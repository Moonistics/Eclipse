//! Home's head: the selected instance, Play with its version, and joining a friend.
use crate::fields::Field;
use crate::game_run::QuickPlay;
use crate::menus::Menu;
use crate::page::Page;
use crate::play_state::PlayState;
use crate::ui::{self, ButtonKind, Press};
use crate::{dates, layout, play_state, scene, Launcher};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::{backend, instances};
use eclipse_theme::glass::space;
use gpui::prelude::*;
use gpui::{div, px, ClickEvent, Div, SharedString, Stateful};

const PLAY: f32 = 240.;
const PLAY_WIDE: f32 = 280.;
const PICKER: f32 = 280.;
const INVITE_MIN: f32 = 360.;
const ICON_BUTTONS: f32 = 2.;
const ICON: f32 = ui::TILE_LG;

/// Play, the version and the big icons are this tall.
fn tall() -> f32 {
    ui::ButtonSize::Hero.height()
}

impl Launcher {
    /// The head: what Home shows above its tabs. `eclipse`: the Eclipse version it plays (none
    /// for a Java instance); `prepared`: whether that version's files are ready.
    pub(super) fn home_head(
        &mut self,
        eclipse: Option<&str>,
        prepared: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let play = self.play_group(eclipse, cx);
        let invite = eclipse.map(|_| {
            let width = self.invite_width();
            self.invite_field(width, cx)
        });
        let icons: Vec<_> = [self.edit_button(cx), self.folder_button()]
            .into_iter()
            .flatten()
            .collect();
        let icons = (!icons.is_empty()).then(|| div().flex().gap(px(space::MD)).children(icons));
        let actions = div()
            .mt(px(ui::page_gap()))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(space::MD))
            .child(play)
            .children(eclipse.is_some().then(|| self.version_picker(cx)))
            .children(icons)
            .children(invite);
        div()
            .relative()
            .flex()
            .flex_col()
            .min_w_0()
            .pt(px(ui::page_top()))
            .child(self.home_title(cx))
            .children(self.played_text(eclipse.filter(|_| !prepared)))
            .child(actions)
            .child(scene::track_home_head(&self.scene))
    }

    /// The instance's icon tile and its name. The icon opens the instance's Edit page.
    pub(super) fn home_title(&mut self, cx: &mut Context<Self>) -> Div {
        let Some(instance) = self.selected_instance().cloned() else {
            return div();
        };
        let edit = instance.clone();
        let icon = ui::picture_button(
            "home-instance-icon",
            self.instance_icon(&instance, ICON),
            tr("Edit instance"),
        )
        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.open_instance_page(Some(edit.clone()));
            cx.notify();
        }));
        let icon = ui::skipped_by_tab(icon);
        div()
            .flex()
            .items_center()
            .gap(px(space::LG))
            .min_w_0()
            .child(icon)
            .child(ui::page_title(instance.name.into()))
    }

    fn head_note_says_no_sign_in(&self) -> bool {
        self.game.run().is_none()
            && self.selected_instance().is_some()
            && self.cant_sign_in_to_play()
    }

    /// "41 hours played · last played 1 hour ago" and a Java instance's game ("Vanilla 1.21.4"),
    /// or how long it has run now.
    fn played_text(&self, unprepared: Option<&str>) -> Option<Div> {
        let run = self.game.run().map(|r| r.since);
        let line = match (run, self.selected_instance(), unprepared) {
            (Some(since), _, _) => trf(
                "Playing for {time} now. Have fun!",
                &[("time", &instances::play_time(since.elapsed()))],
            ),
            (None, Some(_), _) if self.head_note_says_no_sign_in() => tr(
                "Playing needs Microsoft sign-in, which this copy of the launcher can't do yet.",
            )
            .into(),
            (None, Some(_), Some(tag)) => {
                let version = backend::game_version(tag);
                let said_below = self
                    .shown_prepare_error()
                    .is_some_and(|e| play_state::minecraft_missing(e, &version));
                if self.owner == Some(true) {
                    trf(
                        "The first Play downloads Minecraft {version} from Mojang.",
                        &[("version", &version)],
                    )
                } else if said_below {
                    return None;
                } else {
                    trf(
                        "Before your first game, play Minecraft {version} once in your Minecraft launcher.",
                        &[("version", &version)],
                    )
                }
            }
            (None, Some(i), None) => {
                let played = match i.last_played {
                    Some(when) => trf(
                        "{time} played · last played {when}",
                        &[
                            ("time", &dates::duration(i.played)),
                            ("when", &dates::ago(when)),
                        ],
                    ),
                    None => tr("Not played yet").into(),
                };
                match &i.java {
                    Some(java) => format!("{played} · {}", java.line()),
                    None => played,
                }
            }
            (None, None, _) => return None,
        };
        Some(ui::page_note("home-played", line))
    }

    /// Play and its ⋯ menu: Stop while the game runs, Update when the launcher's version has a
    /// newer one, the progress while preparing, updating or starting.
    fn play_group(&mut self, eclipse: Option<&str>, cx: &mut Context<Self>) -> Div {
        let state = self.selected_play_state();
        let running = state == PlayState::Running;
        let update = eclipse.and_then(|_| self.play_update());
        let busy = state.busy();
        let menu = eclipse.filter(|_| self.menu_is(&Menu::Play));
        let no_sign_in = self.why_no_sign_in();
        let held = no_sign_in.filter(|_| busy.is_none() && !running && update.is_none());
        let kind = if (running && busy.is_none()) || held.is_some() {
            ButtonKind::Secondary
        } else {
            ButtonKind::Primary
        };
        let focus = self.play_focus.clone();
        let main = match (busy, held) {
            (Some((label, progress)), _) => ui::keep_focus(
                ui::primary_busy("play", label, progress, ui::ButtonSize::Hero),
                &focus,
            ),
            (None, _) if running => ui::keep_focus(
                ui::button(
                    "play",
                    Some("stop"),
                    tr("Stop"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Hero,
                ),
                &focus,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(run) = this.game.run() {
                    run.client.kill();
                }
                cx.notify();
            })),
            (None, Some(why)) => {
                ui::keep_focus_for("play", &focus);
                let (icon, label, size) = (Some("play"), tr("Play"), ui::ButtonSize::Hero);
                if self.head_note_says_no_sign_in() {
                    ui::primary_unavailable_said("play", icon, label, size, why)
                } else {
                    ui::primary_unavailable("play", icon, label, size, why)
                }
            }
            (None, None) => {
                let (label, icon) = match &update {
                    Some(_) => (tr("Update"), crate::icons::DOWNLOAD),
                    None => (tr("Play"), "play"),
                };
                let main = ui::keep_focus(
                    ui::primary("play", Some(icon), label, ui::ButtonSize::Hero),
                    &focus,
                );
                match &update {
                    Some(_) => main.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.install_update();
                        cx.notify();
                    })),
                    None => main.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.close_menu();
                        this.press_play();
                        cx.notify();
                    })),
                }
            }
        };
        let width = px(self.play_width(update.is_some()));
        let play = if eclipse.is_none() {
            main.min_w(width).into_any_element()
        } else {
            let open = menu.is_some();
            let more = if busy.is_some() {
                ui::more_busy("play-more", tr("Play options"), ui::ButtonSize::Hero, open)
            } else {
                ui::more_button(
                    "play-more",
                    tr("Play options"),
                    kind,
                    ui::ButtonSize::Hero,
                    open,
                )
            };
            let more = self.menu_opener(more, Menu::Play, cx);
            ui::split(main, more, kind).min_w(width).into_any_element()
        };
        let mut wrap = div().relative().child(play);
        if let Some(tag) = menu {
            let locked = running || busy.is_some();
            let wait = self.busy_reason().filter(|_| locked);
            let set_up: SharedString = if backend::is_prepared(tag) {
                tr("Repair game files").into()
            } else {
                trf(
                    "Set up Minecraft {version}",
                    &[("version", &backend::game_version(tag))],
                )
                .into()
            };
            let mut items = Vec::new();
            if update.is_some() {
                items.push(PlayRow {
                    key: "play-skip",
                    icon: "play",
                    label: tr("Play without updating").into(),
                    locked,
                    why: no_sign_in.map(String::from).or_else(|| wait.clone()),
                    what: PlayItem::Play,
                });
            }
            items.push(PlayRow {
                key: "play-prepare",
                icon: crate::icons::VERSION,
                label: set_up,
                locked,
                why: wait,
                what: PlayItem::Prepare,
            });
            items.push(PlayRow {
                key: "play-minecraft-folder",
                icon: crate::icons::OPEN_FOLDER,
                label: tr("Choose Minecraft folder…").into(),
                locked: false,
                why: None,
                what: PlayItem::MinecraftFolder,
            });
            let mut menu = self.menu_popover("play-menu", tr("Play options"), cx);
            for item in items {
                let row = ui::menu_row(item.key, item.icon, item.label, false);
                let what = item.what;
                menu = menu.child(match item.why {
                    Some(why) => ui::unavailable(row, why),
                    None if item.locked => ui::disabled(row),
                    None => row.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.close_menu();
                        match what {
                            PlayItem::Prepare => {
                                this.prepare(backend::MinecraftFrom::Install, false)
                            }
                            PlayItem::Play => this.press_play(),
                            PlayItem::MinecraftFolder => this.choose_folder(cx),
                        }
                        cx.notify();
                    })),
                });
            }
            wrap = wrap.child(ui::menu_under(tall(), ui::Align::Start, menu));
        }
        wrap
    }

    /// The version: "Version" over the one played, a chevron that turns while its list is open;
    /// the list ticks the one played and ends with Manage versions.
    fn version_picker(&self, cx: &mut Context<Self>) -> Div {
        let label = self.selected_version().map_or_else(
            || tr("No version").into(),
            |tag| eclipse_launcher::nightly::display_name(tag),
        );
        let open = self.menu_is(&Menu::Version);
        let button = ui::labelled_dropdown(
            "version-picker",
            tr("Version"),
            None,
            label,
            open,
            ui::ButtonSize::Hero,
        )
        .w(px(PICKER));
        let button = self.menu_opener(button, Menu::Version, cx);
        let mut wrap = div().relative().child(button);
        if open {
            let follows = self.default_version().cloned();
            let newest =
                follows.as_ref() == backend::newest_release_or_nightly(&self.versions.installed);
            let mut menu = self.menu_popover("version-picker-menu", tr("Version"), cx);
            for tag in &self.versions.installed {
                let chosen = self.selected_version() == Some(tag);
                let name = eclipse_launcher::nightly::display_name(tag);
                let note = match follows.as_ref() == Some(tag) {
                    true if newest => tr("Always the newest"),
                    true => tr("Default version"),
                    false => tr("Stays on this version"),
                };
                let pick = tag.clone();
                let row = ui::pick_item(format!("version-pick-{tag}"), name, note, chosen);
                menu = menu.child(
                    row.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.use_version(pick.clone());
                        this.close_menu();
                        cx.notify();
                    })),
                );
            }
            menu = menu.child(
                ui::menu_item(
                    "version-manage",
                    tr("Manage versions").into(),
                    String::new(),
                    false,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.go(Page::Versions);
                    this.close_menu();
                    cx.notify();
                })),
            );
            wrap = wrap.child(ui::menu_under(tall(), ui::Align::Start, menu));
        }
        wrap
    }

    fn edit_button(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let instance = self.selected_instance()?.clone();
        Some(
            ui::icon_button(
                "home-edit",
                "pencil-edit-02",
                tr("Edit instance"),
                ButtonKind::Secondary,
                ui::ButtonSize::Hero,
                false,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.open_instance_page(Some(instance.clone()));
                cx.notify();
            })),
        )
    }

    fn folder_button(&self) -> Option<Stateful<Div>> {
        let home = self.selected_instance()?.home.clone();
        Some(ui::open_folder_icon(
            "home-folder",
            tr("Open instance folder"),
            home,
            ButtonKind::Secondary,
            ui::ButtonSize::Hero,
        ))
    }

    fn play_update(&self) -> Option<String> {
        self.selected_instance()
            .and_then(|i| self.update_for(i))
            .map(|r| r.tag.clone())
    }

    fn play_width(&self, update: bool) -> f32 {
        let least = if self.window_width >= layout::HOME_HEAD_WIDE {
            PLAY_WIDE
        } else {
            PLAY
        };
        match update {
            true => least.max(
                ui::button_width(
                    tr("Update"),
                    true,
                    ButtonKind::Primary,
                    ui::ButtonSize::Hero,
                ) + tall(),
            ),
            false => least,
        }
    }

    fn invite_width(&self) -> Option<f32> {
        let room = layout::content_width(self.window_width);
        let pair = self.play_width(self.play_update().is_some()) + space::MD + PICKER;
        let icon = space::MD + tall();
        let row = pair + ICON_BUTTONS * icon;
        if row + space::MD + INVITE_MIN <= room {
            None
        } else if row <= room {
            Some(row)
        } else if pair <= room {
            Some(pair - ICON_BUTTONS * icon)
        } else {
            Some(PICKER + ICON_BUTTONS * icon)
        }
    }

    /// The invite code field, `width` wide or what its row leaves; the icon inside on the left,
    /// Join inside on the right.
    fn invite_field(&mut self, width: Option<f32>, cx: &mut Context<Self>) -> Div {
        let code = self.fields.value(Field::InviteCode);
        let valid = eclipse_friends::invite::parse(code).is_some();
        let refused = self.start_refusal(&QuickPlay::Friend(code.to_string()));
        let join = ui::button(
            "join-friend",
            None,
            tr("Join"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        let join = if !valid {
            ui::unavailable(join, tr("Paste a friend's invite code first"))
        } else if let Some(why) = refused {
            ui::unavailable(join, why)
        } else {
            join.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.join_friend();
                cx.notify();
            }))
        };
        let field = self
            .field_with(
                Field::InviteCode,
                Some(ui::field_icon("user-add-01")),
                tr("Paste a friend's invite code"),
                width.unwrap_or(INVITE_MIN),
                ui::ButtonSize::Hero,
                Some(join.into_any_element()),
                cx,
            )
            .w_full();
        div()
            .relative()
            .map(|d| match width {
                Some(w) => d.w(px(w)),
                None => d.flex_1().min_w(px(INVITE_MIN)),
            })
            .child(field)
            .children(self.field_error(Field::InviteCode).map(|note| {
                note.absolute()
                    .top(px(tall() + space::XS))
                    .left_0()
                    .right_0()
            }))
    }
}

#[derive(Clone, Copy)]
enum PlayItem {
    Prepare,
    Play,
    MinecraftFolder,
}

struct PlayRow {
    key: &'static str,
    icon: &'static str,
    label: SharedString,
    locked: bool,
    why: Option<String>,
    what: PlayItem,
}
