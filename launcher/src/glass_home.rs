//! Home's head: the instance's icon and name, how long it was played, then Play with its menu,
//! the version, the instance's folder, and joining a friend on the right.
use super::*;
use crate::glass_ui::{self as ui, white};
use eclipse_theme::glass::{fill, ink, radius};
use gpui::deferred;
use std::time::Instant;

const PLAY: f32 = 240.;
const PLAY_WIDE: f32 = 280.;
const PICKER: f32 = 170.;
const PICKER_WIDE: f32 = 220.;
const INVITE_MIN: f32 = 200.;
const INVITE_MAX: f32 = 420.;
const INVITE_ICON_INSET: f32 = 16.;
const ICON: f32 = 48.;

/// Play, the version and the big icons are this tall.
fn tall() -> f32 {
    ui::Size::Hero.height()
}

/// Which of Home's menus is open.
#[derive(Clone, Copy, PartialEq, Default)]
pub(super) enum HomeMenu {
    #[default]
    None,
    Play,
}

impl Launcher {
    /// The head: what Home shows above its tabs. `tag`: the Eclipse version it plays (empty for a
    /// Java instance); `prepared`: whether that version's files are ready.
    pub(super) fn glass_home_head(
        &mut self,
        tag: &str,
        prepared: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let java = self.selected_instance().and_then(|i| i.java.clone());
        // How long this game has run, for "Playing for 5 min now".
        match (&self.client, self.client_since) {
            (Some(_), None) => self.client_since = Some(Instant::now()),
            (None, Some(_)) => self.client_since = None,
            _ => {}
        }
        let actions = div()
            .mt(px(ui::page_gap()))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(10.))
            .child(self.play_group(tag, prepared || java.is_some(), cx))
            .children(java.is_none().then(|| self.glass_picker(cx)))
            .children(self.folder_button())
            .child(self.glass_join_friend(tag, cx));
        div()
            .flex()
            .flex_col()
            .min_w_0()
            .pt(px(ui::page_top()))
            .child(self.home_title(cx))
            .child(self.played_text())
            .child(actions)
    }

    /// The instance's icon tile and its name. The icon opens the instance's Edit page.
    fn home_title(&mut self, cx: &mut Context<Self>) -> Div {
        let Some(instance) = self.selected_instance().cloned() else {
            return div();
        };
        let edit = instance.clone();
        let icon = ui::hover_over(
            "home-instance-icon",
            radius::ROW,
            self.glass_instance_icon(&instance, ICON),
        )
        .cursor_pointer()
        .tooltip(ui::tip(tr("Edit instance")))
        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.open_instance_page(Some(edit.clone()));
            cx.notify();
        }));
        div()
            .flex()
            .items_center()
            .gap(px(16.))
            .min_w_0()
            .child(ui::tip_on_focus(icon, tr("Edit instance").into()))
            .child(
                ui::entity_title(instance.name, eclipse_theme::glass::pixel_text::PAGE_TITLE)
                    .min_w_0()
                    .truncate(),
            )
    }

    /// "41 hours played · last played 1 hour ago" and a Java instance's game ("Vanilla 1.21.4"),
    /// or how long it has run now.
    fn played_text(&self) -> Div {
        let line = match (self.client_since, self.selected_instance()) {
            (Some(since), _) => trf(
                "Playing for {time} now. Have fun!",
                &[("time", &instances::play_time(since.elapsed()))],
            ),
            (None, Some(i)) => {
                let played = match i.last_played {
                    Some(when) => trf(
                        "{time} played · last played {when}",
                        &[
                            ("time", &played_for(i.played)),
                            ("when", &last_played(when)),
                        ],
                    ),
                    None => tr("Not played yet").into(),
                };
                match &i.java {
                    Some(java) => format!("{played} · {}", game_line(java)),
                    None => played,
                }
            }
            (None, None) => String::new(),
        };
        ui::text(
            eclipse_theme::glass::step::LEAD,
            FontWeight::NORMAL,
            ink::STRONG,
        )
        .mt(px(16.))
        .child(line)
    }

    /// Play and its ⋯ menu: Stop while the game runs, Update when the launcher's version has a
    /// newer one, the progress while preparing, updating or starting.
    fn play_group(&mut self, tag: &str, ready: bool, cx: &mut Context<Self>) -> Div {
        let running = self.client.is_some();
        let follows = self
            .selected_instance()
            .is_some_and(|i| i.version.is_none());
        let update = self
            .updates
            .eclipse
            .as_ref()
            .filter(|_| follows && !tag.is_empty())
            .map(|r| r.tag.clone());
        let installing = update
            .as_ref()
            .and_then(|u| self.installing.as_ref().filter(|(t, _)| t == u))
            .map(|(_, p)| *p);
        // The version this instance keeps, downloading since it was made or edited to play it.
        let own = self
            .selected_instance()
            .and_then(|i| i.version.as_ref())
            .and_then(|v| {
                let queued = self.install_next.as_ref().is_some_and(|r| r.tag == *v);
                let running = self.installing.as_ref().filter(|(t, _)| t == v);
                running.map(|(_, p)| Some(*p)).or(queued.then_some(None))
            });
        let busy = match (installing, self.preparing, &self.starting_client) {
            (Some(p), _, _) => Some((tr("Updating"), Some(p))),
            (_, _, _) if own.is_some() => Some((tr("Installing"), own.flatten())),
            (_, true, _) => Some((
                tr("Preparing"),
                eclipse_launcher::mojang::progress().map(|(d, n)| d as f32 / n.max(1) as f32),
            )),
            (_, _, Some(_)) => Some((tr("Starting"), None)),
            _ => None,
        };
        let open = self.home_menu == HomeMenu::Play;
        let kind = if running && busy.is_none() {
            Kind::Secondary
        } else {
            Kind::Primary
        };
        let name = self
            .selected_instance()
            .map(|i| i.name.clone())
            .unwrap_or_default();
        let more = ui::more_button(
            "play-more",
            trf("{name} options", &[("name", &name)]),
            kind,
            ui::Size::Hero,
            open,
        )
        .on_press(cx.listener(|this, event: &ClickEvent, window, cx| {
            if !this.closed_by_this_press(event) {
                this.home_menu = match this.home_menu {
                    HomeMenu::Play => HomeMenu::None,
                    _ => HomeMenu::Play,
                };
                if this.home_menu == HomeMenu::Play {
                    this.opened_menu(window, cx);
                }
            }
            cx.notify();
        }));
        let main = match busy {
            Some((label, progress)) => ui::primary_busy("play", label, progress, ui::Size::Hero),
            None if running => ui::button(
                "play",
                Some("stop"),
                tr("Stop"),
                Kind::Secondary,
                ui::Size::Hero,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(client) = &this.client {
                    client.kill();
                }
                cx.notify();
            })),
            None => {
                let (label, icon) = match &update {
                    Some(_) => (tr("Update"), "download-04"),
                    None => (tr("Play"), "play"),
                };
                let press = play_press(ready, self.owner, false);
                let main = ui::button("play", Some(icon), label, Kind::Primary, ui::Size::Hero);
                match (&update, press) {
                    (Some(_), _) => main.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.install_update();
                        cx.notify();
                    })),
                    (None, PlayPress::Play) => {
                        main.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.picker_open = false;
                            this.play();
                            cx.notify();
                        }))
                    }
                    (None, PlayPress::PrepareThenPlay) => {
                        main.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.picker_open = false;
                            this.play_when_prepared = this.selected().cloned();
                            this.prepare(backend::MinecraftFrom::Install);
                            cx.notify();
                        }))
                    }
                    (None, PlayPress::Nothing) => crate::disabled(main),
                }
            }
        };
        let wide = self.window_width >= layout::HOME_HEAD_WIDE;
        let mut wrap = div()
            .relative()
            .child(ui::split(main, more, kind).min_w(px(if wide { PLAY_WIDE } else { PLAY })));
        if open {
            let locked = running || busy.is_some();
            let mut items: Vec<(&'static str, SharedString, String, bool, PlayItem)> = Vec::new();
            if update.is_some() {
                items.push((
                    "play-skip",
                    tr("Play without updating").into(),
                    String::new(),
                    locked || !ready,
                    PlayItem::Play,
                ));
            }
            if !tag.is_empty() {
                let version = backend::game_version(tag);
                items.push((
                    "play-prepare",
                    tr(if backend::is_prepared(tag) {
                        "Prepare game files again"
                    } else {
                        "Prepare game files"
                    })
                    .into(),
                    trf(
                        "Uses your copy of Minecraft {version}",
                        &[("version", &version)],
                    ),
                    locked,
                    PlayItem::Prepare,
                ));
                items.push((
                    "play-minecraft-folder",
                    tr("Choose the Minecraft folder").into(),
                    self.settings
                        .minecraft_dir
                        .as_deref()
                        .map_or_else(|| tr("Not found").into(), short),
                    false,
                    PlayItem::MinecraftFolder,
                ));
            }
            let instance = self.selected_instance().cloned();
            let id = instance.as_ref().map(|i| (i.id.clone(), i.name.clone()));
            if let Some((id, _)) = &id {
                items.push((
                    "play-edit",
                    tr("Edit instance").into(),
                    String::new(),
                    false,
                    PlayItem::Edit,
                ));
                let has = self.has_shortcut(id);
                items.push((
                    "play-shortcut",
                    tr(if has {
                        "Remove the desktop shortcut"
                    } else {
                        "Create a desktop shortcut"
                    })
                    .into(),
                    String::new(),
                    false,
                    PlayItem::Shortcut,
                ));
                items.push((
                    "play-command",
                    tr("Copy the launch command").into(),
                    launch_command(id),
                    false,
                    PlayItem::Command,
                ));
            }
            let mut menu = ui::popover("play-menu").on_mouse_down_out(cx.listener(
                |this, event: &MouseDownEvent, _, cx| {
                    this.home_menu = HomeMenu::None;
                    this.menu_closed_at = Some(event.position);
                    cx.notify();
                },
            ));
            for (key, label, note, locked, what) in items {
                let (id, edit) = (id.clone(), instance.clone());
                let item = ui::menu_item(key, label, note, locked);
                menu = menu.child(if locked {
                    item
                } else {
                    item.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.home_menu = HomeMenu::None;
                        match what {
                            PlayItem::Prepare => this.prepare(backend::MinecraftFrom::Install),
                            PlayItem::Play => this.play(),
                            PlayItem::MinecraftFolder => this.choose_folder(cx),
                            PlayItem::Edit => this.open_instance_page(edit.clone()),
                            PlayItem::Shortcut => {
                                if let Some((id, name)) = id.clone() {
                                    this.toggle_shortcut(id, name);
                                }
                            }
                            PlayItem::Command => {
                                if let Some((id, _)) = &id {
                                    copy_to_clipboard(
                                        cx,
                                        gpui::ClipboardItem::new_string(launch_command(id)),
                                    );
                                }
                            }
                        }
                        cx.notify();
                    }))
                });
            }
            wrap = wrap.child(
                deferred(
                    div()
                        .absolute()
                        .top(px(tall() + ui::MENU_GAP))
                        .left_0()
                        .child(menu),
                )
                .with_priority(1),
            );
        }
        wrap
    }

    /// The version: "Version" over the one played, a chevron that turns while its list is open;
    /// the list ticks the one played and ends with Manage versions.
    fn glass_picker(&self, cx: &mut Context<Self>) -> Div {
        let label = self.selected().map_or_else(
            || tr("No version").into(),
            |tag| {
                eclipse_launcher::nightly::label(tag)
                    .or_else(|| build_import::label(tag))
                    .unwrap_or_else(|| tag.clone())
            },
        );
        let open = self.picker_open;
        let value = div()
            .flex()
            .flex_col()
            .child(
                ui::text(
                    eclipse_theme::glass::step::CAPTION,
                    FontWeight::NORMAL,
                    ink::MUTED,
                )
                .child(tr("Version")),
            )
            .child(
                ui::text(
                    eclipse_theme::glass::step::LEAD,
                    FontWeight::MEDIUM,
                    ink::FULL,
                )
                .child(label),
            );
        let button = ui::dropdown("picker", None, value, open, tall())
            .gap(px(24.))
            .min_w(px(if self.window_width >= layout::HOME_HEAD_WIDE {
                PICKER_WIDE
            } else {
                PICKER
            }))
            .pl(px(16.))
            .on_press(cx.listener(|this, event: &ClickEvent, window, cx| {
                if !this.closed_by_this_press(event) {
                    this.picker_open = !this.picker_open;
                    if this.picker_open {
                        this.opened_menu(window, cx);
                    }
                }
                cx.notify();
            }));
        let mut wrap = div().relative().child(button);
        if open {
            let follows = self.launcher_version().cloned();
            let mut menu = ui::popover("picker-menu").on_mouse_down_out(cx.listener(
                |this, event: &MouseDownEvent, _, cx| {
                    this.picker_open = false;
                    this.menu_closed_at = Some(event.position);
                    cx.notify();
                },
            ));
            for tag in &self.installed {
                let chosen = self.selected() == Some(tag);
                let latest = follows.as_ref() == Some(tag);
                let name = eclipse_launcher::nightly::label(tag)
                    .or_else(|| build_import::label(tag))
                    .unwrap_or_else(|| tag.clone());
                let note = if latest {
                    tr("Always the newest").to_string()
                } else {
                    tag.clone()
                };
                let pick = tag.clone();
                menu = menu.child(
                    ui::menu_item(format!("pick-{tag}"), name.into(), note, false)
                        .when(chosen, |d| d.child(ui::tick()))
                        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.pin_version((!latest).then(|| pick.clone()));
                            this.picker_open = false;
                            cx.notify();
                        })),
                );
            }
            menu = menu.child(
                ui::menu_item("manage", tr("Manage versions").into(), String::new(), false)
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.go(Page::Versions);
                        this.picker_open = false;
                        cx.notify();
                    })),
            );
            wrap = wrap.child(
                deferred(
                    div()
                        .absolute()
                        .top(px(tall() + ui::MENU_GAP))
                        .left_0()
                        .child(menu),
                )
                .with_priority(1),
            );
        }
        wrap
    }

    fn folder_button(&self) -> Option<Stateful<Div>> {
        let home = self.selected_instance()?.home.clone();
        Some(
            ui::icon_button(
                "home-folder",
                "folder-01",
                tr("Open folder"),
                ui::Size::Hero,
                false,
            )
            .bg(white(fill::BUTTON))
            .on_press(move |_, _, cx| {
                let _ = std::fs::create_dir_all(&home);
                crate::glass_toast::open(cx, &home);
            }),
        )
    }

    /// The invite code field: what its row leaves, up to [`INVITE_MAX`]; the icon inside on the
    /// left, Join inside on the right.
    fn glass_join_friend(&self, tag: &str, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let focused = self.friend_focused;
        let join_label = tr("Join");
        let pr = friend_code_padding_right(join_label);
        let field = ui::field(focused, false)
            .id("friend-code")
            .track_focus(&self.friend_focus)
            .size_full()
            .pl(px(2. * INVITE_ICON_INSET + ui::IconSize::Px16.px()))
            .pr(px(pr))
            .text_color(white(ink::FULL))
            .cursor_text()
            .when(focused, |d| d.child(ui::focus_ring(radius::ROW)))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .child(line_edit::content(
                        &self.friend_code,
                        &t,
                        focused,
                        false,
                        false,
                        "Paste a friend's invite code",
                    )),
            )
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                window.focus(&this.friend_focus, cx);
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                match line_edit::handle(&mut this.friend_code, ev, cx) {
                    line_edit::Key::Pass => return,
                    line_edit::Key::Enter => this.join_friend(),
                    line_edit::Key::Escape => window.blur(cx),
                    _ => {}
                }
                cx.stop_propagation();
                cx.notify();
            }));
        let field = line_edit::mouse(field, cx, |this| &mut this.friend_code);
        let valid = eclipse_friends::invite::parse(self.friend_code.text()).is_some();
        let can = valid && (tag.is_empty() || self.can_start(tag));
        let join = ui::button(
            "join-friend",
            None,
            join_label,
            Kind::Secondary,
            ui::Size::Md,
        );
        let join = if can {
            join.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.join_friend();
                cx.notify();
            }))
        } else {
            crate::disabled(join)
        };
        div()
            .relative()
            .flex_1()
            .min_w(px(INVITE_MIN))
            .max_w(px(INVITE_MAX))
            .h(px(tall()))
            .child(field)
            .child(
                div()
                    .absolute()
                    .left(px(INVITE_ICON_INSET))
                    .top(px((tall() - ui::IconSize::Px16.px()) / 2.))
                    .child(ui::icon(
                        "user-add-01",
                        ui::IconSize::Px16,
                        white(ink::MUTED),
                    )),
            )
            .child(
                div()
                    .absolute()
                    .right(px(8.))
                    .top(px((tall() - ui::Size::Md.height()) / 2.))
                    .child(join),
            )
    }
}

/// Right padding for the invite code text field. In English ("Join"), the button is ~58 px wide,
/// so 84 px leaves a clean margin from the button placed at right: 8 px. In locales with longer
/// labels (French "Rejoindre", German "Beitreten", Ukrainian "Приєднатися"), the button is wider;
/// expand the field's padding so the button never overlaps the entered text or placeholder.
fn friend_code_padding_right(join_label: &str) -> f32 {
    let btn_width = 28. + (join_label.chars().count() as f32 * 8.5).max(30.);
    (8. + btn_width + 12.).max(84.)
}

/// How long an instance was played: "41 hours", "1 hour", "25 min".
fn played_for(played: Duration) -> String {
    match played.as_secs() / 60 {
        0 => tr("under a minute").into(),
        m @ 1..=59 => trf("{count} min", &[("count", &m.to_string())]),
        60..=119 => tr("1 hour").into(),
        m => trf("{count} hours", &[("count", &(m / 60).to_string())]),
    }
}

pub(super) fn game_line(java: &instances::JavaGame) -> String {
    match &java.loader {
        Some(l) => trf(
            "{loader} {version}",
            &[("loader", l.kind.name()), ("version", &java.minecraft)],
        ),
        None => trf("Vanilla {version}", &[("version", &java.minecraft)]),
    }
}

/// When something was last played: "just now", "25 min ago", "2 hours ago" within a day, then
/// "yesterday", then the day ("Oct 31").
pub(super) fn last_played(when: std::time::SystemTime) -> String {
    let local: chrono::DateTime<chrono::Local> = when.into();
    since(
        std::time::SystemTime::now()
            .duration_since(when)
            .unwrap_or_default(),
        (chrono::Local::now().date_naive() - local.date_naive()).num_days(),
        &local.format("%Y-%m-%d").to_string(),
    )
}

/// [`last_played`] from how long ago it was, how many calendar days back, and its date.
fn since(ago: Duration, days: i64, date: &str) -> String {
    match ago.as_secs() / 60 {
        0 => tr("just now").into(),
        m @ 1..=59 => trf("{count} min ago", &[("count", &m.to_string())]),
        60..=119 => tr("1 hour ago").into(),
        m if m < 24 * 60 => trf("{count} hours ago", &[("count", &(m / 60).to_string())]),
        _ if days <= 1 => tr("yesterday").into(),
        _ => glass_versions::published(date, ""),
    }
}

/// What a Play menu row does.
#[derive(Clone, Copy)]
enum PlayItem {
    Prepare,
    Play,
    MinecraftFolder,
    Edit,
    Shortcut,
    Command,
}

/// The command that plays instance `id` without the window, as its desktop shortcut runs it.
fn launch_command(id: &str) -> String {
    format!("eclipse-launcher play --instance {id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn played_time_reads_in_hours_or_minutes() {
        let h = |s: u64| played_for(Duration::from_secs(s));
        assert_eq!(h(148_000), "41 hours");
        assert_eq!(h(5_400), "1 hour");
        assert_eq!(h(1_500), "25 min");
    }

    #[test]
    fn last_played_counts_hours_within_a_day_then_days() {
        let m = |min: u64| Duration::from_secs(min * 60);
        assert_eq!(since(m(0), 0, "2026-11-03"), "just now");
        assert_eq!(since(m(25), 0, "2026-11-03"), "25 min ago");
        assert_eq!(since(m(130), 0, "2026-11-03"), "2 hours ago");
        // Under 24 hours, even across midnight.
        assert_eq!(since(m(20 * 60), 1, "2026-11-02"), "20 hours ago");
        assert_eq!(since(m(30 * 60), 1, "2026-11-02"), "yesterday");
        assert_eq!(since(m(3 * 24 * 60), 3, "2026-10-31"), "Oct 31");
    }

    #[test]
    fn friend_code_padding_right_clears_localized_join_button() {
        for label in [
            "Join",
            "Rejoindre",
            "Beitreten",
            "Приєднатися",
            "Unirse",
            "Dołącz",
            "Reunir-se",
        ] {
            let pr = friend_code_padding_right(label);
            let btn_width = 28. + label.chars().count() as f32 * 8.5;
            let btn_right_extent = 8. + btn_width;
            assert!(
                pr >= btn_right_extent + 10.,
                "padding {pr} must clear button right extent {btn_right_extent} for label '{label}'"
            );
        }
    }
}
