use super::*;
use crate::menus::{Asking, Menu};
use crate::ui::{self, Tone};
use eclipse_launcher::account::Player;
use eclipse_launcher::instances::{Instance, Kind as InstanceKind};
use eclipse_theme::glass::{ink, space, step};
use gpui::{canvas, point};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

const ACCOUNT_CHIP: f32 = 52.;
const ACCOUNT_CHIP_SHORT: f32 = 40.;
const ACCOUNT_TOP: f32 = ui::SIDEBAR_PAD;
const ACCOUNT_TOP_SHORT: f32 = space::SM;
const ACCOUNT_HEAD: f32 = ui::TILE_MD;
const MENU_HEAD: f32 = ui::TILE_SM;
const NAV_TOP: f32 = space::MD + space::XXS;
const NAV_TOP_SHORT: f32 = space::SM;
const NAV_GAP: f32 = space::XXS;
const LIST_TOP: f32 = space::LG + space::XXS;
const LIST_TOP_SHORT: f32 = space::XS + space::XXS;
const GROUP_TOP: f32 = space::LG;
const GROUP_BOTTOM: f32 = space::XS + space::XXS;
const ROW_TILE: f32 = ui::TILE_MD;
const MORE_INSET: f32 = space::XXS;

const NAV_INSET: f32 = ui::SIDEBAR_INSET - ui::SIDEBAR_PAD - 1.;
const LIST_END: f32 = ui::SIDEBAR_PAD + space::XS;

fn row_width(window: f32) -> f32 {
    layout::sidebar(window) - 2. * ui::SIDEBAR_PAD
}

fn list_row_width(window: f32) -> f32 {
    row_width(window) - (LIST_END - ui::SIDEBAR_PAD)
}

const SIDE_ROWS: &str = "side-rows";

fn account_chip(compact: bool) -> f32 {
    if compact {
        ACCOUNT_CHIP_SHORT
    } else {
        ACCOUNT_CHIP
    }
}

fn account_top(compact: bool) -> f32 {
    if compact {
        ACCOUNT_TOP_SHORT
    } else {
        ACCOUNT_TOP
    }
}

/// Where the sidebar drew its rows and labels, for the ⋯ menu beside a row and the list's foot.
#[derive(Clone, Default)]
pub struct RowMenu {
    /// Where each row was drawn last, by instance id: the menu opens beside its row.
    pub rows: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    list: Scrollbar,
    shown: Rc<RefCell<Option<(String, Pixels)>>>,
    labels: Rc<RefCell<Vec<Bounds<Pixels>>>>,
    lone_label: Rc<Cell<f32>>,
}

/// Records where the element it sits in is drawn, under `id`.
fn measure(rows: &Rc<RefCell<HashMap<String, Bounds<Pixels>>>>, id: String) -> impl IntoElement {
    let rows = rows.clone();
    ui::measure(move |bounds| {
        rows.borrow_mut().insert(id, bounds);
    })
}

fn measure_label(labels: &Rc<RefCell<Vec<Bounds<Pixels>>>>) -> impl IntoElement {
    let labels = labels.clone();
    ui::measure(move |bounds| labels.borrow_mut().push(bounds))
}

fn group_label(
    id: impl Into<SharedString>,
    text: impl Into<SharedString>,
    labels: &Rc<RefCell<Vec<Bounds<Pixels>>>>,
) -> Stateful<Div> {
    let text: SharedString = text.into();
    ui::heading(id, text.clone(), 2)
        .relative()
        .px(px(ui::SIDEBAR_INSET - ui::SIDEBAR_PAD))
        .pt(px(GROUP_TOP))
        .pb(px(GROUP_BOTTOM))
        .child(ui::label(text))
        .child(measure_label(labels))
}

impl Launcher {
    /// A sidebar row's state line and its dot, from what the launcher knows of the instance;
    /// `short` where the narrow sidebar has no room for the whole line.
    fn row_state(&self, i: &Instance, short: bool) -> (Tone, String) {
        let state = self.play_state(i);
        if state == PlayState::Running {
            return (Tone::Live, tr("Running").into());
        }
        if let Some(line) = state.busy_line() {
            return (Tone::Busy, line);
        }
        match &i.kind {
            InstanceKind::Default | InstanceKind::Own => match &i.java {
                Some(java) => (Tone::Off, java.line()),
                None if short => self.install_state(i).short_line(),
                None => self.install_state(i).line(),
            },
            InstanceKind::Port { slug } => {
                let summary = self.modpacks.imported.iter().find(|p| p.slug == *slug);
                (
                    Tone::Off,
                    summary.map_or_else(|| tr("Modpack").into(), |p| p.summary()),
                )
            }
            InstanceKind::Pack { .. } => (Tone::Off, tr("Eclipse modpack").into()),
        }
    }

    /// The sidebar's instances by group, those without a group first, then the servers: one
    /// list, one Tab stop, the arrows between rows.
    fn sidebar_rows(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        self.row_menu.labels.borrow_mut().clear();
        let editing = self
            .instances
            .draft
            .as_ref()
            .filter(|_| self.page == Page::InstanceEditor)
            .and_then(|d| d.editing.clone());
        let selected = editing.or_else(|| {
            (self.page == Page::Home)
                .then(|| self.selected_instance().map(|i| i.id.clone()))
                .flatten()
        });
        let on_packs = self.page == Page::Mods && self.mods.on_modpacks();
        let keyboard = ui::options_has_keyboard(SIDE_ROWS);
        let cursor = ui::options_cursor(SIDE_ROWS);
        let mut rows = Vec::new();
        let mut heads = Vec::new();
        let mut group = None;
        let row_end = ui::ButtonSize::Sm.height() + 2. * MORE_INSET;
        let roomy = layout::sidebar(self.window_width) >= layout::SIDEBAR;
        let instances = self.instances.list.clone();
        for (at, i) in instances.iter().enumerate() {
            if i.group != group {
                group = i.group.clone();
                if let Some(g) = &group {
                    heads.push((
                        at,
                        group_label(format!("side-group-{g}"), g.clone(), &self.row_menu.labels)
                            .into_any_element(),
                    ));
                }
            }
            let current = match &i.kind {
                InstanceKind::Default | InstanceKind::Own => selected.as_ref() == Some(&i.id),
                InstanceKind::Port { slug } => {
                    on_packs && self.modpacks.open.as_deref() == Some(slug.as_str())
                }
                InstanceKind::Pack { .. } => false,
            };
            let state = self.row_state(i, false);
            let shown = if roomy {
                state.1.clone()
            } else {
                self.row_state(i, true).1
            };
            let icon = self.instance_icon(i, ROW_TILE);
            let open = self.menu_is(&Menu::InstanceRow(i.id.clone()));
            let right_id = i.id.clone();
            let (row_id, more_id): (SharedString, SharedString) = (
                format!("side-instance-{}", i.id).into(),
                format!("side-more-{}", i.id).into(),
            );
            let lit = open
                || (keyboard && cursor == at)
                || ui::hover_is(&row_id)
                || ui::hover_is(&more_id);
            let more = div()
                .absolute()
                .top_0()
                .bottom_0()
                .right(px(MORE_INSET))
                .flex()
                .items_center()
                .child(ui::shown_on_focus(
                    ui::skipped_by_tab(self.menu_opener(
                        ui::more_button(
                            more_id,
                            trf("{name} options", &[("name", &i.name)]),
                            ButtonKind::Ghost,
                            ui::ButtonSize::Sm,
                            open,
                        ),
                        Menu::InstanceRow(i.id.clone()),
                        cx,
                    )),
                    lit,
                ));
            let end = if lit { row_end } else { space::SM };
            let room = ui::side_row_text_room(list_row_width(self.window_width), NAV_INSET, end);
            let clipped = ui::text_width(&i.name, step::BODY)
                .max(ui::DOT_ROOM + ui::text_width(&state.1, step::NOTE));
            let full = format!("{} · {}", i.name, state.1);
            let row = ui::tip_when_clipped(
                ui::side_row(row_id, i.name.clone(), current)
                    .aria_description(state.1.clone())
                    .aria_keyshortcuts("Shift+F10"),
                full,
                clipped,
                room,
            )
            .pr(px(end))
            .pl(px(NAV_INSET))
            .child(measure(&self.row_menu.rows, i.id.clone()))
            .child(div().relative().flex_none().child(icon))
            .child(ui::item_text(
                i.name.clone(),
                ui::state_line(state.0, shown),
            ))
            .child(more)
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    // The open menu's mouse-down-out already closed it on this press:
                    // reopening here would make the toggle a no-op.
                    if this.menu_closed_at.take() == Some(event.position) {
                        cx.notify();
                        return;
                    }
                    this.toggle_menu(Menu::InstanceRow(right_id.clone()), window, cx);
                    cx.notify();
                }),
            );
            rows.push((row, SharedString::from(i.name.clone()), current));
        }
        let servers = self.servers.listed().to_vec();
        if !servers.is_empty() {
            heads.push((
                rows.len(),
                group_label("side-group-servers", tr("Servers"), &self.row_menu.labels)
                    .into_any_element(),
            ));
        }
        for e in &servers {
            let up = self.servers.is_up(&e.dir);
            let open = self.page == Page::Servers && self.servers.open.as_ref() == Some(&e.dir);
            let state = if up { tr("Running") } else { tr("Stopped") };
            let row = ui::side_row(
                format!("side-server-{}", e.dir.display()),
                e.name.clone(),
                open,
            )
            .aria_description(state)
            .pr(px(space::SM))
            .pl(px(NAV_INSET))
            .child(ui::tile(ROW_TILE).relative().child(ui::icon(
                Page::Servers.icon(),
                ui::IconSize::Px16,
                ui::icon_tone(open),
            )))
            .child(ui::item_text(
                e.name.clone(),
                ui::state_line(if up { Tone::Live } else { Tone::Off }, state),
            ));
            rows.push((row, SharedString::from(e.name.clone()), open));
        }
        let picks: Vec<Result<Instance, std::path::PathBuf>> = instances
            .into_iter()
            .map(Ok)
            .chain(servers.iter().map(|e| Err(e.dir.clone())))
            .collect();
        let menus: Vec<Option<String>> = picks
            .iter()
            .map(|p| p.as_ref().ok().map(|i| i.id.clone()))
            .collect();
        let this = cx.entity().downgrade();
        ui::options_with_heads(
            div().id(SIDE_ROWS).flex().flex_col().gap(px(space::XS)),
            tr("Instances"),
            rows,
            heads,
            false,
            move |i, _, cx| {
                let Some(pick) = picks.get(i).cloned() else {
                    return;
                };
                let _ = this.update(cx, |this, cx| {
                    match pick {
                        Ok(clicked) => match &clicked.kind {
                            InstanceKind::Default | InstanceKind::Own => {
                                this.select_instance(&clicked);
                                this.go(Page::Home);
                            }
                            InstanceKind::Port { slug } => this.show_modpack(Some(slug.clone())),
                            InstanceKind::Pack { .. } => this.show_modpack(None),
                        },
                        Err(dir) => {
                            this.open_server(dir);
                            this.go(Page::Servers);
                        }
                    }
                    cx.notify();
                });
            },
        )
        .on_key_down(
            cx.listener(move |this, ev: &gpui::KeyDownEvent, window, cx| {
                if !ui::opens_row_menu(&ev.keystroke) {
                    return;
                }
                let Some(Some(id)) = menus.get(ui::options_cursor(SIDE_ROWS)).cloned() else {
                    return;
                };
                this.open_menu_from(Menu::InstanceRow(id), window, cx);
                cx.stop_propagation();
                cx.notify();
            }),
        )
    }

    fn keep_selected_shown(&self) -> impl IntoElement {
        let id = self.selected_instance().map(|i| i.id.clone());
        let rows = self.row_menu.rows.clone();
        let list = self.row_menu.list.handle.clone();
        let shown = self.row_menu.shown.clone();
        let covered = px(self.row_menu.lone_label.get());
        let side = self.side_focus.clone();
        let margin = px(ui::fade_height());
        canvas(
            move |view, window, cx| {
                let Some(id) = id else {
                    return;
                };
                if window.last_input_was_keyboard() && side.contains_focused(window, cx) {
                    return;
                }
                let key = (id, view.size.height);
                if shown.borrow().as_ref() == Some(&key) {
                    return;
                }
                let Some(row) = rows.borrow().get(&key.0).copied() else {
                    return;
                };
                let mut offset = list.offset();
                if row.top() < view.top() + margin {
                    offset.y += view.top() + margin - row.top();
                } else if row.bottom() > view.bottom() - covered - margin {
                    offset.y -= row.bottom() - (view.bottom() - covered - margin);
                    let moved = offset.y - list.offset().y;
                    let sliced = rows.borrow().values().find_map(|r| {
                        let (top, bottom) = (r.top() + moved, r.bottom() + moved);
                        (top < view.top() && bottom > view.top()).then_some(bottom - view.top())
                    });
                    if let Some(rest) = sliced {
                        offset.y -= rest;
                    }
                }
                offset.y = offset.y.min(px(0.)).max(-list.max_offset().y);
                if offset != list.offset() {
                    list.set_offset(offset);
                    window.request_animation_frame();
                }
                *shown.borrow_mut() = Some(key);
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }

    fn cover_lone_label(&self) -> impl IntoElement {
        let labels = self.row_menu.labels.clone();
        let list = self.row_menu.list.handle.clone();
        let cover = self.row_menu.lone_label.clone();
        let peek = px(ui::fade_height());
        canvas(
            move |_, window, _| {
                let foot = list.bounds().bottom();
                let lone = labels
                    .borrow()
                    .iter()
                    .map(|l| (l.top() + px(GROUP_TOP), l.bottom()))
                    .filter(|&(top, bottom)| top < foot && bottom + peek > foot)
                    .map(|(top, _)| f32::from(foot - top))
                    .fold(0., f32::max);
                if lone != cover.get() {
                    cover.set(lone);
                    window.refresh();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
    }

    fn nav_item(&self, page: Page, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = page.label();
        let row = ui::nav_row(
            format!("nav-{page:?}"),
            page.icon(),
            label,
            self.page == page,
            ui::NavDensity::Sidebar,
        );
        ui::tip_when_clipped(
            row,
            label,
            ui::press_width(&label.to_uppercase()),
            ui::nav_text_room(
                row_width(self.window_width),
                NAV_INSET,
                ui::NavDensity::Sidebar,
            ),
        )
        .px(px(NAV_INSET))
        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.go(page);
            cx.notify();
        }))
    }

    fn account_block(&mut self, compact: bool, cx: &mut Context<Self>) -> Div {
        let who = self.who_plays();
        let (name, how, face) = (who.name, who.how, Some(who.face));
        let open = self.menu_is(&Menu::Account);
        let chip = ui::selected(
            ui::hoverable("account", eclipse_theme::glass::radius::ROW, ui::Hover::Row)
                .aria_label(name.clone())
                .aria_description(how)
                .flex()
                .items_center()
                .gap(px(space::SM))
                .h(px(account_chip(compact)))
                .pl(px(NAV_INSET))
                .pr(px(MORE_INSET
                    + (ui::ButtonSize::Sm.height() - ui::IconSize::Px16.px())
                        / 2.))
                .cursor_pointer()
                .child(ui::face(
                    self.profile_heads
                        .draw(face.as_deref(), ACCOUNT_HEAD, self.scale),
                    ACCOUNT_HEAD,
                ))
                .child(ui::item_text(name, how).ml(px(space::XS)))
                .child(ui::chevron(open).relative()),
            open,
        );
        let chip = self.menu_opener(chip, Menu::Account, cx);
        div()
            .flex()
            .flex_col()
            .flex_none()
            .px(px(ui::SIDEBAR_PAD))
            .pt(px(account_top(compact)))
            .child(
                div()
                    .relative()
                    .child(chip)
                    .children(self.account_popover(compact, cx)),
            )
    }

    pub(super) fn sidebar(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        self.load_packs();
        let compact = ui::short_window();
        let account = self.account_block(compact, cx);
        let mut nav = div()
            .flex()
            .flex_col()
            .flex_none()
            .when(!compact, |d| d.gap(px(NAV_GAP)))
            .px(px(ui::SIDEBAR_PAD))
            .pt(px(if compact { NAV_TOP_SHORT } else { NAV_TOP }));
        for page in Page::NAV {
            if page != Page::Settings {
                nav = nav.child(self.nav_item(page, cx));
            }
        }
        let adding = self.page == Page::InstanceEditor
            && self
                .instances
                .draft
                .as_ref()
                .is_some_and(|d| d.editing.is_none() && !d.duplicate);
        let add = ui::icon_button(
            "add-instance",
            crate::icons::NEW_INSTANCE,
            tr("New instance"),
            ButtonKind::Ghost,
            ui::ButtonSize::Sm,
            adding,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.open_instance_page(None);
            cx.notify();
        }));
        let header = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .pl(px(ui::SIDEBAR_INSET))
            .pr(px(ui::SIDEBAR_PAD + MORE_INSET))
            .pt(px(if compact { LIST_TOP_SHORT } else { LIST_TOP }))
            .pb(px(GROUP_BOTTOM))
            .child(
                ui::heading("side-instances-title", tr("Instances").into(), 2)
                    .child(ui::label(tr("Instances"))),
            )
            .child(add);
        let rows = self.sidebar_rows(cx);
        let list = self
            .row_menu
            .list
            .wrap(
                ui::scroll_pane("side-list", &self.row_menu.list.handle)
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .pl(px(ui::SIDEBAR_PAD))
                    .pr(px(LIST_END))
                    .child(rows),
            )
            .flex_1()
            .min_h_0();
        let side = self.theme.side;
        let lone_label = self.row_menu.lone_label.get();
        let list = div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(list)
            .child(self.keep_selected_shown())
            .child(self.cover_lone_label())
            .when(lone_label > 0., |d| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(px(lone_label))
                        .bg(side),
                )
            })
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top_0()
                    .bottom(px(lone_label))
                    .children(ui::more_above_fade(&self.row_menu.list.handle, side))
                    .children(ui::more_below_fade(&self.row_menu.list.handle, side)),
            );

        let mut foot = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(space::XXS))
            .px(px(ui::SIDEBAR_PAD))
            .pt(px(space::MD))
            .pb(px(space::MD))
            .border_t_1()
            .border_color(ui::edge());
        let caption = |text: String| {
            ui::text(step::CAPTION, FontWeight::NORMAL, ink::MUTED)
                .mt(px(space::XS))
                .child(text)
                .into_any_element()
        };
        let card = |body: AnyElement, actions| {
            ui::notice(ui::Notice::Info, "download-04", body, actions).mb(px(space::SM))
        };
        let titled = |title: String, body: AnyElement| {
            div()
                .flex()
                .flex_col()
                .child(ui::text(step::NOTE, FontWeight::SEMIBOLD, ink::FULL).child(title))
                .child(body)
                .into_any_element()
        };
        match &self.self_update {
            SelfUpdate::Downloading(version, f) => {
                foot = foot.child(card(
                    ui::progress_line(
                        trf("Launcher {version}", &[("version", version)]),
                        Some((*f, *f)),
                        "",
                    )
                    .into_any_element(),
                    Vec::new(),
                ));
            }
            SelfUpdate::Ready(version) => {
                let update = ui::button(
                    "update-launcher",
                    None,
                    tr("Update launcher"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .w_full()
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.apply_launcher_update(cx);
                    cx.notify();
                }));
                foot = foot.child(card(
                    titled(
                        tr("Launcher update available").to_string(),
                        caption(trf("Launcher {version}", &[("version", version)])),
                    ),
                    vec![update.into_any_element()],
                ));
            }
            SelfUpdate::Stuck(error) if !self.self_update_hidden => {
                let why = self.launcher_update_help(error.as_deref());
                let url = backend::download_page(&self.links.website);
                let mut actions = Vec::new();
                if !backend::package_managed_launcher() {
                    actions.push(
                        ui::button(
                            "download-launcher",
                            None,
                            tr("Download again"),
                            ButtonKind::Secondary,
                            ui::ButtonSize::Md,
                        )
                        .w_full()
                        .on_press(move |_, _, cx| cx.open_url(&url))
                        .into_any_element(),
                    );
                }
                actions.push(
                    ui::button(
                        "dismiss-launcher-update",
                        None,
                        tr("Dismiss"),
                        ButtonKind::Ghost,
                        ui::ButtonSize::Md,
                    )
                    .w_full()
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.self_update_hidden = true;
                        cx.notify();
                    }))
                    .into_any_element(),
                );
                foot = foot.child(card(
                    titled(tr("Launcher update available").to_string(), caption(why)),
                    actions,
                ));
            }
            _ => {}
        }
        if self.native_log.available {
            foot = foot.child(
                ui::nav_row(
                    "side-native-log",
                    crate::icons::CONSOLE,
                    tr("Mod build log"),
                    false,
                    ui::NavDensity::Sidebar,
                )
                .px(px(NAV_INSET))
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.native_log_open();
                    cx.notify();
                })),
            );
        }
        foot = foot
            .child(self.nav_item(Page::Settings, cx))
            .when(!compact, |d| {
                let version = trf("Launcher {version}", &[("version", backend::VERSION)]);
                d.child(
                    ui::text(step::CAPTION, FontWeight::NORMAL, ink::DIM)
                        .id("launcher-version")
                        .role(gpui::Role::Label)
                        .aria_label(version.clone())
                        .px(px(ui::SIDEBAR_INSET - ui::SIDEBAR_PAD))
                        .pt(px(space::SM))
                        .child(version)
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.version_clicked();
                            cx.notify();
                        })),
                )
            });

        div()
            .id("sidebar")
            .role(gpui::Role::Navigation)
            .aria_label(tr("Launcher"))
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(layout::sidebar(self.window_width)))
            .h_full()
            .bg(side)
            .border_r_1()
            .border_color(ui::edge())
            .child(account)
            .child(nav)
            .child(header)
            .child(list)
            .child(foot)
    }

    /// The menu under the player: everyone who can play, Sign in with Microsoft while playing
    /// offline, then Manage accounts.
    fn account_popover(&self, compact: bool, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.menu_is(&Menu::Account) {
            return None;
        }
        let now = self.accounts.player();
        let mut menu = self.menu_popover("account-menu", tr("Account"), cx);
        for (who, name, how) in self.players() {
            let chosen = who == now;
            let face = match &who {
                Player::Microsoft(uuid) => Some(uuid.clone()),
                _ => Some(profile_heads::offline_key(&name)),
            };
            let row = ui::menu_item(
                format!("account-pick-{name}"),
                name.into(),
                how.into(),
                false,
            )
            .pl(px(ui::PANEL_INSET + MENU_HEAD + ui::PANEL_INSET))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(ui::PANEL_INSET))
                    .flex()
                    .items_center()
                    .child(ui::face(
                        self.profile_heads
                            .draw(face.as_deref(), MENU_HEAD, self.scale),
                        MENU_HEAD,
                    )),
            )
            .when(chosen, |d| d.child(ui::tick()));
            menu = menu.child(if chosen { ui::starts_menu(row) } else { row }.on_press(
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.play_as(who.clone());
                    cx.notify();
                }),
            ));
        }
        let mut more = div().mt(px(space::XS)).flex().flex_col();
        if self.can_sign_in() {
            more = more.child(
                ui::face_menu_row(
                    "account-sign-in",
                    "user-add-01",
                    tr("Sign in with Microsoft"),
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.close_menu();
                    this.open_sign_in();
                    cx.notify();
                })),
            );
        }
        menu = menu.child(
            more.child(
                ui::face_menu_row(
                    "account-manage",
                    crate::icons::SETTINGS,
                    tr("Manage accounts"),
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.close_menu();
                    this.go(Page::Settings);
                    cx.notify();
                })),
            ),
        );
        Some(ui::menu_under(
            account_chip(compact),
            ui::Align::Start,
            menu,
        ))
    }

    /// The ⋯ menu of a sidebar row, beside it: Edit instance, Duplicate…, Open instance folder,
    /// Export…, launch settings, the desktop shortcut and its command, then Delete… (then asked
    /// to confirm). Each acts on that instance.
    pub(super) fn row_menu(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Some(Menu::InstanceRow(id)) = self.open_menu.clone() else {
            return None;
        };
        let confirm = self.is_asking(&Asking::InstanceDelete(id.clone()));
        let Some(instance) = self.instances.list.iter().find(|i| i.id == id).cloned() else {
            self.close_menu();
            return None;
        };
        let row = self.row_menu.rows.borrow().get(&id).copied()?;
        let beside = row.right().max(px(layout::sidebar(self.window_width))) + px(ui::MENU_GAP);
        let at = point(beside, row.top());
        let item = |key: &'static str, icon: &'static str, label: String| {
            ui::menu_row(format!("row-menu-{key}"), icon, label, false)
        };
        let (edit, copy, export, launch, shortcut) = (
            instance.clone(),
            instance.clone(),
            instance.clone(),
            instance.clone(),
            instance.clone(),
        );
        let home = instance.home.clone();
        let command = instance.id.clone();
        let shortcut_label = if self.has_shortcut(&instance.id) {
            tr("Remove the desktop shortcut")
        } else {
            tr("Create a desktop shortcut")
        };
        let mut menu = self
            .menu_popover(
                "row-menu",
                trf("{name} options", &[("name", &instance.name)]),
                cx,
            )
            .child(
                item("edit", "pencil-edit-02", tr("Edit…").into()).on_press(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.close_menu();
                        this.open_instance_page(Some(edit.clone()));
                        cx.notify();
                    },
                )),
            )
            .when(instance_state::duplicable(&instance), |menu| {
                menu.child(
                    item("duplicate", "copy-01", tr("Duplicate…").into()).on_press(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.show_duplicate(&copy);
                            cx.notify();
                        },
                    )),
                )
            })
            .child(
                item("folder", "folder-01", tr("Open instance folder").into()).on_press(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.close_menu();
                        ui::open_folder(&home, cx);
                        cx.notify();
                    }),
                ),
            )
            .child(
                item("export", "upload-04", tr("Export…").into()).on_press(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.show_export(&export);
                        cx.notify();
                    },
                )),
            )
            .child(
                item(
                    "launch",
                    crate::icons::LAUNCH_OPTIONS,
                    tr("Launch options…").into(),
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.show_launch_options(&launch);
                    cx.notify();
                })),
            )
            .child(
                item("shortcut", "link-01", shortcut_label.into()).on_press(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.close_menu();
                        this.toggle_shortcut(shortcut.id.clone(), shortcut.name.clone());
                        cx.notify();
                    },
                )),
            )
            .child(
                item(
                    "command",
                    crate::icons::CONSOLE,
                    tr("Copy the launch command").into(),
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.close_menu();
                    clipboard::copy_to_clipboard(
                        cx,
                        gpui::ClipboardItem::new_string(launch_command(&command)),
                    );
                    cx.notify();
                })),
            );
        let delete_block = self.delete_block(&instance);
        let gone = instance.id.clone();
        let delete = if confirm {
            ui::menu_confirm(
                "row-menu-delete",
                ui::Confirm::Delete(&instance.name),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.stop_asking(|a| matches!(a, Asking::InstanceDelete(_)));
                    cx.notify();
                }),
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.close_menu();
                    this.delete_instance(&gone);
                    cx.notify();
                }),
            )
            .into_any_element()
        } else {
            let ask = ui::menu_row("row-menu-delete", "delete-02", tr("Delete…"), true);
            match delete_block {
                None => {
                    let asked = instance.id.clone();
                    ask.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.asking = Some(Asking::InstanceDelete(asked.clone()));
                        cx.notify();
                    }))
                    .into_any_element()
                }
                Some(why) => ui::unavailable(ask, why).into_any_element(),
            }
        };
        menu = menu.child(delete);
        Some(ui::menu_beside(at, menu).into_any_element())
    }
}

/// The command that plays instance `id` without the window, as its desktop shortcut runs it.
fn launch_command(id: &str) -> String {
    format!("eclipse-launcher play --instance {id}")
}
