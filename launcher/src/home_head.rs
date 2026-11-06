//! Home's head: the instance's icon and name, how long it was played, then Play with its menu,
//! the version, the instance's folder, and joining a friend on the right.
use super::*;
use crate::menus::Menu;
use crate::ui::{self};
use eclipse_theme::glass::space;
use std::time::Instant;

const PLAY: f32 = 240.;
const PLAY_WIDE: f32 = 280.;
const PICKER: f32 = 280.;
const INVITE_MIN: f32 = 360.;
const ICON: f32 = ui::TILE_LG;

/// Play, the version and the big icons are this tall.
fn tall() -> f32 {
    ui::ButtonSize::Hero.height()
}

impl Launcher {
    /// The head: what Home shows above its tabs. `tag`: the Eclipse version it plays (empty for a
    /// Java instance); `prepared`: whether that version's files are ready.
    pub(super) fn home_head(&mut self, tag: &str, prepared: bool, cx: &mut Context<Self>) -> Div {
        let java = self.selected_instance().and_then(|i| i.java.clone());
        // How long this game has run, for "Playing for 5 min now".
        match (&self.game.client, self.game.since) {
            (Some(_), None) => self.game.since = Some(Instant::now()),
            (None, Some(_)) => self.game.since = None,
            _ => {}
        }
        let play = self.play_group(tag, cx);
        let invite = java.is_none().then(|| {
            let width = self.invite_width(tag);
            self.invite_field(tag, width, cx)
        });
        let actions = div()
            .mt(px(ui::page_gap()))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(space::MD))
            .child(play)
            .children(java.is_none().then(|| self.version_picker(cx)))
            .children(self.folder_button())
            .children(invite);
        div()
            .relative()
            .flex()
            .flex_col()
            .min_w_0()
            .pt(px(ui::page_top()))
            .child(self.home_title(cx))
            .child(self.played_text(tag, prepared))
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
        div()
            .flex()
            .items_center()
            .gap(px(space::LG))
            .min_w_0()
            .child(icon)
            .child(ui::page_title(instance.name.into()))
    }

    /// "41 hours played · last played 1 hour ago" and a Java instance's game ("Vanilla 1.21.4"),
    /// or how long it has run now.
    fn played_text(&self, tag: &str, prepared: bool) -> Div {
        let line = match (self.game.since, self.selected_instance()) {
            (Some(since), _) => trf(
                "Playing for {time} now. Have fun!",
                &[("time", &instances::play_time(since.elapsed()))],
            ),
            (None, Some(i)) if i.java.is_none() && !prepared => {
                if self.shown_prepare_error().is_some() {
                    return ui::page_note("home-played", "");
                }
                let version = backend::game_version(tag);
                if self.owner == Some(true) {
                    trf(
                        "The first Play downloads Minecraft {version} from Mojang.",
                        &[("version", &version)],
                    )
                } else {
                    trf(
                        "Before your first game, play Minecraft {version} once in your Minecraft launcher.",
                        &[("version", &version)],
                    )
                }
            }
            (None, Some(i)) => {
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
            (None, None) => String::new(),
        };
        ui::page_note("home-played", line)
    }

    /// Play and its ⋯ menu: Stop while the game runs, Update when the launcher's version has a
    /// newer one, the progress while preparing, updating or starting.
    fn play_group(&mut self, tag: &str, cx: &mut Context<Self>) -> Div {
        let state = self.selected_play_state();
        let running = state == PlayState::Running;
        let update = self.play_update(tag);
        let busy = state.busy();
        let open = self.menu_is(&Menu::Play) && !tag.is_empty();
        let kind = if running && busy.is_none() {
            ButtonKind::Secondary
        } else {
            ButtonKind::Primary
        };
        let focus = self.play_focus.clone();
        let main = match busy {
            Some((label, progress)) => {
                ui::primary_busy("play", label, progress, ui::ButtonSize::Hero)
            }
            None if running => ui::button(
                "play",
                Some("stop"),
                tr("Stop"),
                ButtonKind::Secondary,
                ui::ButtonSize::Hero,
            )
            .track_focus(&focus)
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(client) = &this.game.client {
                    client.kill();
                }
                cx.notify();
            })),
            None => {
                let (label, icon) = match &update {
                    Some(_) => (tr("Update"), "download-04"),
                    None => (tr("Play"), "play"),
                };
                let main = ui::button(
                    "play",
                    Some(icon),
                    label,
                    ButtonKind::Primary,
                    ui::ButtonSize::Hero,
                )
                .track_focus(&focus);
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
        let play = if tag.is_empty() {
            main.min_w(width).into_any_element()
        } else {
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
        if open {
            let locked = running || busy.is_some();
            let mut items: Vec<(&'static str, &'static str, &'static str, bool, PlayItem)> =
                Vec::new();
            if update.is_some() {
                items.push((
                    "play-skip",
                    "play",
                    tr("Play without updating"),
                    locked,
                    PlayItem::Play,
                ));
            }
            items.push((
                "play-prepare",
                crate::icons::VERSION,
                tr(if backend::is_prepared(tag) {
                    "Prepare game files again"
                } else {
                    "Prepare game files"
                }),
                locked,
                PlayItem::Prepare,
            ));
            items.push((
                "play-minecraft-folder",
                "folder-01",
                tr("Choose Minecraft folder…"),
                false,
                PlayItem::MinecraftFolder,
            ));
            let mut menu = self.menu_popover("play-menu", tr("Play options"), cx);
            for (key, icon, label, locked, what) in items {
                let row = ui::menu_row(key, icon, label, false);
                menu = menu.child(if locked {
                    ui::disabled(row)
                } else {
                    row.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.close_menu();
                        match what {
                            PlayItem::Prepare => this.prepare(backend::MinecraftFrom::Install),
                            PlayItem::Play => this.press_play(),
                            PlayItem::MinecraftFolder => this.choose_folder(cx),
                        }
                        cx.notify();
                    }))
                });
            }
            wrap = wrap.child(ui::menu_under(tall(), ui::Align::Start, menu));
        }
        wrap
    }

    pub(super) fn press_play(&mut self) {
        let ready = self.selected_instance().is_some_and(|i| i.java.is_some())
            || self
                .selected_version()
                .is_some_and(|tag| backend::is_prepared(tag));
        match play_press(ready, self.game.busy()) {
            PlayPress::Play => self.play(),
            PlayPress::PrepareThenPlay => {
                self.game.play_when_prepared = self.selected_version().cloned();
                self.prepare(backend::MinecraftFrom::Install);
            }
            PlayPress::Nothing => {
                if let Some(why) = self.busy_reason() {
                    crate::toast::say(why);
                }
            }
        }
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

    fn folder_button(&self) -> Option<Stateful<Div>> {
        let home = self.selected_instance()?.home.clone();
        Some(
            ui::icon_button(
                "home-folder",
                "folder-01",
                tr("Open instance folder"),
                ButtonKind::Secondary,
                ui::ButtonSize::Hero,
                false,
            )
            .on_press(move |_, _, cx| ui::open_folder(&home, cx)),
        )
    }

    fn play_update(&self, tag: &str) -> Option<String> {
        self.selected_instance()
            .and_then(|i| self.update_for(i))
            .filter(|_| !tag.is_empty())
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

    fn invite_width(&self, tag: &str) -> Option<f32> {
        let room = layout::content_width(self.window_width);
        let pair = self.play_width(self.play_update(tag).is_some()) + space::MD + PICKER;
        let row = pair + space::MD + tall();
        if row + space::MD + INVITE_MIN <= room {
            None
        } else if row <= room {
            Some(row)
        } else if pair <= room {
            Some(pair - tall() - space::MD)
        } else {
            Some(PICKER + space::MD + tall())
        }
    }

    /// The invite code field, `width` wide or what its row leaves; the icon inside on the left,
    /// Join inside on the right.
    fn invite_field(&mut self, tag: &str, width: Option<f32>, cx: &mut Context<Self>) -> Div {
        let code = self.fields.value(Field::InviteCode);
        let valid = eclipse_friends::invite::parse(code).is_some();
        let can = valid && self.can_start(tag);
        let join = ui::button(
            "join-friend",
            None,
            tr("Join"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        let join = if can {
            join.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.join_friend();
                cx.notify();
            }))
        } else if !valid {
            ui::unavailable(join, tr("Paste a friend's invite code first"))
        } else {
            ui::unavailable(join, self.why_not_start())
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

/// What a Play menu row does.
#[derive(Clone, Copy)]
enum PlayItem {
    Prepare,
    Play,
    MinecraftFolder,
}
