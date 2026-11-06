use super::*;
use crate::home_head::game_line;
use crate::menus::{Asking, Menu};
use crate::ui::{self, white, Tone};
use eclipse_launcher::account::Player;
use eclipse_launcher::instances::{Icon, Instance, Kind as InstanceKind};
use eclipse_theme::glass::{fill, ink, radius, space, step};
use gpui::{anchored, canvas, deferred, point, ScrollHandle};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

pub use crate::layout::SIDEBAR;

const ACCOUNT_CHIP: f32 = 52.;
const ACCOUNT_CHIP_SHORT: f32 = 44.;
const ACCOUNT_TOP: f32 = 10.;
const ACCOUNT_TOP_SHORT: f32 = 8.;
const ACCOUNT_HEAD: f32 = 32.;
const MENU_HEAD: f32 = 30.;
const NAV_TOP: f32 = 14.;
const NAV_TOP_SHORT: f32 = 8.;
const NAV_GAP: f32 = 2.;
const LIST_TOP: f32 = 18.;
const LIST_TOP_SHORT: f32 = 6.;
const GROUP_TOP: f32 = 16.;
const GROUP_BOTTOM: f32 = 6.;
const ROW_TILE: f32 = ui::TILE_MD;
const MORE_INSET: f32 = 6.;
const WINDOW_MARGIN: f32 = 8.;

const LONE_LABEL_PEEK: f32 = 20.;
const NAV_INSET: f32 = ui::SIDEBAR_INSET - ui::SIDEBAR_PAD - 1.;

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
    list: ScrollHandle,
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

fn group_label(text: impl Into<SharedString>, labels: &Rc<RefCell<Vec<Bounds<Pixels>>>>) -> Div {
    ui::label(text)
        .relative()
        .px(px(ui::SIDEBAR_INSET - ui::SIDEBAR_PAD))
        .pt(px(GROUP_TOP))
        .pb(px(GROUP_BOTTOM))
        .child(measure_label(labels))
}

impl Launcher {
    /// An instance's icon on a `size` tile: Eclipse's mark for Eclipse's own instance; else its
    /// picture or the block chosen for it; else its loader's or Minecraft's mark, each two thirds
    /// of the tile on the 8 px grid; else its initial on its colour (the accent when none was
    /// chosen).
    pub(super) fn instance_icon(&mut self, i: &Instance, size: f32) -> AnyElement {
        let inner = (size * 2. / 3. / 8.).round() * 8.;
        let tile = ui::tile(size);
        if i.kind == InstanceKind::Default && i.icon.is_none() {
            return tile.child(self.eclipse_mark(inner)).into_any_element();
        }
        if let Some(picture) = self.instance_picture(i, inner) {
            return tile
                .child(instance_icons::centred(picture, inner, self.scale))
                .into_any_element();
        }
        let mut colour = ui::look().accent;
        match &i.icon {
            Some(Icon::Colour(hex)) => {
                colour = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(colour);
            }
            Some(Icon::Texture(name)) => {
                if let Some(image) = self.texture_image(name, inner) {
                    return tile
                        .child(gpui::img(image).flex_none().size(px(inner)))
                        .into_any_element();
                }
            }
            None => {
                if let Some(mark) = self.instance_mark(i, inner) {
                    return tile
                        .child(instance_icons::centred(mark, inner, self.scale))
                        .into_any_element();
                }
            }
        }
        ui::letter_tile(ui::color(colour), &i.name, size).into_any_element()
    }

    /// A sidebar row's state line and its dot, from what the launcher knows of the instance.
    fn row_state(&self, i: &Instance) -> (Tone, String) {
        let running =
            self.client.is_some() && self.selected_instance().is_some_and(|s| s.id == i.id);
        if running {
            return (Tone::Live, tr("Running").into());
        }
        let starting = self.starting_client.is_some()
            && self.selected_instance().is_some_and(|s| s.id == i.id);
        if starting {
            return (Tone::Busy, tr("Starting").into());
        }
        match i.kind {
            InstanceKind::Default | InstanceKind::Own => {
                let tag = self.version_of(i).cloned();
                let prepared = tag.as_deref().is_some_and(backend::is_prepared);
                match (&i.java, &tag, &i.version) {
                    (Some(java), _, _) => (Tone::Live, game_line(java)),
                    (None, None, Some(own)) => (
                        Tone::Warn,
                        trf("{version} is not installed", &[("version", own)]),
                    ),
                    (None, None, None) => (Tone::Off, tr("Not installed").into()),
                    // Following the launcher's version, which has a newer one waiting.
                    (None, Some(_), None) if self.updates.eclipse.is_some() => (
                        Tone::Accent(ui::look().accent_soft),
                        tr("Update available").into(),
                    ),
                    (None, Some(_), _) if !prepared => (Tone::Warn, tr("Needs preparing").into()),
                    (None, Some(tag), _) if self.native_build_of(tag).is_some() => {
                        let build = self.native_build_of(tag).unwrap_or_default();
                        (
                            Tone::Live,
                            trf(
                                "Built with mods · {version}",
                                &[("version", &build.eclipse_short())],
                            ),
                        )
                    }
                    (None, Some(tag), Some(_)) => (Tone::Live, format!("{} · {tag}", tr("Ready"))),
                    (None, Some(_), None) => (Tone::Live, tr("Ready").into()),
                }
            }
            InstanceKind::Port => {
                let slug = &i.id["port:".len()..];
                let summary = self.packs.iter().find(|p| p.slug == slug);
                (
                    Tone::Live,
                    summary.map_or_else(|| tr("Modpack").into(), |p| p.summary()),
                )
            }
            InstanceKind::Pack => (Tone::Off, tr("Eclipse modpack").into()),
        }
    }

    /// The instances by group: those without a group first.
    fn sidebar_instances(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.row_menu.labels.borrow_mut().clear();
        let editing = self
            .instances
            .draft
            .as_ref()
            .filter(|_| self.page == Page::InstanceEditor)
            .and_then(|d| d.editing.clone());
        let selected = editing.or_else(|| self.selected_instance().map(|i| i.id.clone()));
        let on_packs = self.page == Page::Mods && self.mods.on_modpacks();
        let mut out = Vec::new();
        let mut group = None;
        let row_end = ui::ButtonSize::Sm.height() + 2. * MORE_INSET;
        let mut list = div().flex().flex_col().gap(px(space::XS));
        let flush = |list: Div, out: &mut Vec<AnyElement>| {
            out.push(list.into_any_element());
            div().flex().flex_col().gap(px(space::XS))
        };
        for i in self.instances.list.clone() {
            if i.group != group {
                list = flush(list, &mut out);
                group = i.group.clone();
                if let Some(g) = &group {
                    out.push(group_label(g.clone(), &self.row_menu.labels).into_any_element());
                }
            }
            // The chosen instance stays marked on every page, unless a ported pack's page is
            // open: that row is the one shown then.
            let current = match i.kind {
                InstanceKind::Default | InstanceKind::Own => {
                    !(on_packs && self.pack_open.is_some()) && selected.as_ref() == Some(&i.id)
                }
                InstanceKind::Port => {
                    on_packs && self.pack_open.as_deref() == Some(&i.id["port:".len()..])
                }
                InstanceKind::Pack => false,
            };
            let state = self.row_state(&i);
            let icon = self.instance_icon(&i, ROW_TILE);
            let clicked = i.clone();
            let open = self.menu_is(&Menu::InstanceRow(i.id.clone()));
            let menu_id = i.id.clone();
            // A right click on the row opens the same menu the ⋯ does.
            let right_id = i.id.clone();
            let more = div()
                .absolute()
                .top_0()
                .bottom_0()
                .right(px(MORE_INSET))
                .flex()
                .items_center()
                .child(
                    ui::more_button(
                        format!("side-more-{}", i.id),
                        trf("{name} options", &[("name", &i.name)]),
                        ButtonKind::Ghost,
                        ui::ButtonSize::Sm,
                        open,
                    )
                    .on_press(cx.listener(
                        move |this, event: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            if this.closed_by_this_press(event) {
                                return;
                            }
                            this.toggle_menu(Menu::InstanceRow(menu_id.clone()), window, cx);
                            cx.notify();
                        },
                    )),
                );
            list = list.child(
                div()
                    .relative()
                    .child(measure(&self.row_menu.rows, i.id.clone()))
                    .child(
                        ui::row(format!("side-instance-{}", i.id), current)
                            .pr(px(row_end))
                            .pl(px(NAV_INSET))
                            .child(div().relative().flex_none().child(icon))
                            .child(ui::item_text(
                                i.name.clone(),
                                ui::state_line(state.0, state.1),
                            ))
                            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                match clicked.kind {
                                    InstanceKind::Default | InstanceKind::Own => {
                                        this.select_instance(&clicked);
                                        this.go(Page::Home);
                                    }
                                    InstanceKind::Port => {
                                        this.show_modpack(Some(clicked.id["port:".len()..].into()))
                                    }
                                    InstanceKind::Pack => this.show_modpack(None),
                                }
                                cx.notify();
                            }))
                            // The ⋯ menu, from a right click anywhere on the row.
                            .on_mouse_down(
                                gpui::MouseButton::Right,
                                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                    // The open menu's mouse-down-out already closed it on this
                                    // press: reopening here would make the toggle a no-op.
                                    if this.menu_closed_at.take() == Some(event.position) {
                                        cx.notify();
                                        return;
                                    }
                                    this.open_menu = menus::menu_after_press(
                                        this.open_menu.take(),
                                        Menu::InstanceRow(right_id.clone()),
                                    );
                                    this.stop_asking(|a| matches!(a, Asking::InstanceDelete(_)));
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(more),
            );
        }
        out.push(list.into_any_element());
        out
    }

    fn keep_selected_shown(&self) -> impl IntoElement {
        let id = self.selected_instance().map(|i| i.id.clone());
        let rows = self.row_menu.rows.clone();
        let list = self.row_menu.list.clone();
        let shown = self.row_menu.shown.clone();
        canvas(
            move |view, window, _| {
                let Some(id) = id else {
                    return;
                };
                let key = (id, view.size.height);
                if shown.borrow().as_ref() == Some(&key) {
                    return;
                }
                let Some(row) = rows.borrow().get(&key.0).copied() else {
                    return;
                };
                let mut offset = list.offset();
                if row.top() < view.top() {
                    offset.y += view.top() - row.top();
                } else if row.bottom() > view.bottom() {
                    offset.y -= row.bottom() - view.bottom();
                }
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
        let list = self.row_menu.list.clone();
        let cover = self.row_menu.lone_label.clone();
        canvas(
            move |_, window, _| {
                let foot = list.bounds().bottom();
                let lone = labels
                    .borrow()
                    .iter()
                    .filter(|l| l.top() < foot && l.bottom() + px(LONE_LABEL_PEEK) > foot)
                    .map(|l| f32::from(foot - l.top()))
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

    fn nav_item(
        &self,
        page: Page,
        label: &'static str,
        icon: &'static str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        ui::nav_row(format!("nav-{page:?}"), icon, tr(label), self.page == page)
            .px(px(NAV_INSET))
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.go(page);
                cx.notify();
            }))
    }

    fn account_block(&mut self, compact: bool, cx: &mut Context<Self>) -> Div {
        let (name, how) = self.who_plays();
        let face = match self.accounts.player() {
            Player::Microsoft(_) => self.accounts.selected().map(|a| a.uuid.clone()),
            _ => Some(profile_heads::offline_key(&name)),
        };
        let open = self.menu_is(&Menu::Account);
        let chip =
            ui::hoverable("account", radius::ROW, ui::Hover::Row)
                .flex()
                .items_center()
                .gap(px(space::MD))
                .h(px(account_chip(compact)))
                .px(px(ui::SIDEBAR_INSET - ui::SIDEBAR_PAD))
                .rounded(px(radius::ROW))
                .when(open, |d| d.bg(white(fill::ACTIVE)))
                .cursor_pointer()
                .child(
                    div()
                        .relative()
                        .flex_none()
                        .size(px(ACCOUNT_HEAD))
                        .rounded(px(radius::CONTROL))
                        .overflow_hidden()
                        .child(
                            self.profile_heads
                                .draw(face.as_deref(), ACCOUNT_HEAD, self.scale),
                        ),
                )
                .child(ui::item_text(name, how))
                .child(
                    ui::icon("arrow-down-01", ui::IconSize::Px16, white(ink::MUTED))
                        .relative()
                        .with_transformation(gpui::Transformation::rotate(gpui::radians(
                            if open { std::f32::consts::PI } else { 0. },
                        ))),
                )
                .on_press(cx.listener(|this, event: &ClickEvent, window, cx| {
                    if this.closed_by_this_press(event) {
                        return;
                    }
                    this.toggle_menu(Menu::Account, window, cx);
                    if this.menu_is(&Menu::Account) {
                        this.accounts = Accounts::load();
                        this.load_profile_heads();
                    }
                    cx.notify();
                }));
        div()
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .px(px(ui::SIDEBAR_PAD))
            .pt(px(account_top(compact)))
            .child(chip)
            .children(self.account_popover(compact, cx))
    }

    pub(super) fn sidebar(&mut self, cx: &mut Context<Self>) -> Div {
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
        for (page, label, icon) in Page::NAV {
            nav = nav.child(
                self.nav_item(page, label, icon, cx)
                    .when(compact, |d| d.h(px(ui::NAV_ROW_SHORT))),
            );
        }
        let adding = self.page == Page::InstanceEditor
            && self
                .instances
                .draft
                .as_ref()
                .is_some_and(|d| d.editing.is_none());
        let add = ui::icon_button(
            "add-instance",
            "plus-sign",
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
            .child(ui::label(tr("Instances")))
            .child(add);
        let mut list = ui::scroll_pane("side-list", &self.row_menu.list)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .px(px(ui::SIDEBAR_PAD))
            .children(self.sidebar_instances(cx));

        let servers: Vec<_> = self
            .servers
            .sidebar_list()
            .map(<[_]>::to_vec)
            .unwrap_or_else(|| self.side_servers.clone());
        if !servers.is_empty() {
            list = list.child(group_label(tr("Servers"), &self.row_menu.labels));
        }
        let mut server_rows = div().flex().flex_col().gap(px(space::XXS));
        for e in &servers {
            let up = self.servers.is_up(&e.dir);
            let open = self.page == Page::Servers && self.servers.open.as_ref() == Some(&e.dir);
            let dir = e.dir.clone();
            server_rows = server_rows.child(
                ui::row(format!("side-server-{}", e.dir.display()), open)
                    .pr(px(space::SM))
                    .pl(px(NAV_INSET))
                    .child(ui::tile(ROW_TILE).relative().child(ui::icon(
                        "computer-terminal-01",
                        ui::IconSize::Px16,
                        ui::icon_tone(open),
                    )))
                    .child(ui::item_text(
                        e.name.clone(),
                        if up {
                            ui::state_line(Tone::Live, tr("Running"))
                        } else {
                            ui::state_line(Tone::Off, tr("Stopped"))
                        },
                    ))
                    .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.open_server(dir.clone());
                        this.go(Page::Servers);
                        cx.notify();
                    })),
            );
        }
        let side = self.theme.side;
        let lone_label = self.row_menu.lone_label.get();
        let list = div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(list.child(server_rows))
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
                    .children(ui::more_below_fade(&self.row_menu.list, side)),
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
            .border_color(white(eclipse_theme::glass::EDGE));
        let caption = |text: String| {
            ui::text(step::CAPTION, FontWeight::NORMAL, ink::MUTED)
                .mt(px(space::XS))
                .child(text)
                .into_any_element()
        };
        let card = |title: String, right: Option<String>, body: AnyElement, actions| {
            let head = div()
                .flex()
                .items_baseline()
                .justify_between()
                .child(ui::text(step::NOTE, FontWeight::SEMIBOLD, ink::FULL).child(title))
                .children(
                    right.map(|r| ui::text(step::CAPTION, FontWeight::NORMAL, ink::MUTED).child(r)),
                );
            ui::notice(
                "download-04",
                div().flex().flex_col().child(head).child(body),
                actions,
            )
            .mb(px(space::SM))
        };
        match &self.self_update {
            SelfUpdate::Downloading(version, f) => {
                foot = foot.child(card(
                    trf("Launcher {version}", &[("version", version)]),
                    Some(format!("{:.0}%", f * 100.)),
                    div()
                        .mt(px(space::SM))
                        .child(ui::bar(*f))
                        .into_any_element(),
                    Vec::new(),
                ));
            }
            SelfUpdate::Ready(version) => {
                let update = ui::button(
                    "update-launcher",
                    None,
                    tr("Update launcher"),
                    ButtonKind::Primary,
                    ui::ButtonSize::Md,
                )
                .w_full()
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.apply_launcher_update(cx);
                    cx.notify();
                }));
                foot = foot.child(card(
                    tr("Launcher update available").to_string(),
                    None,
                    caption(trf("Launcher {version}", &[("version", version)])),
                    vec![update.into_any_element()],
                ));
            }
            SelfUpdate::Stuck(error) if !self.self_update_hidden => {
                let why = self.launcher_update_help(error.as_deref());
                let url = backend::download_page(&self.links.website);
                let mut actions = Vec::new();
                if self.launcher_download_available() {
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
                    tr("Launcher update available").to_string(),
                    None,
                    caption(why),
                    actions,
                ));
            }
            _ => {}
        }
        if self.native_log.available {
            foot = foot.child(
                ui::nav_row(
                    "side-native-log",
                    "computer-terminal-01",
                    tr("Mod build log"),
                    false,
                )
                .px(px(NAV_INSET))
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.native_log_open();
                    cx.notify();
                })),
            );
        }
        foot = foot
            .child(
                self.nav_item(Page::Settings, "Settings", "sliders-horizontal", cx)
                    .when(compact, |d| d.h(px(ui::NAV_ROW_SHORT))),
            )
            .when(!compact, |d| {
                d.child(
                    ui::text(step::CAPTION, FontWeight::NORMAL, ink::DIM)
                        .id("launcher-version")
                        .px(px(ui::SIDEBAR_INSET - ui::SIDEBAR_PAD))
                        .pt(px(space::SM))
                        .child(trf("Launcher {version}", &[("version", backend::VERSION)]))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.version_clicked();
                            cx.notify();
                        })),
                )
            });

        div()
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(SIDEBAR))
            .h_full()
            .bg(side)
            .border_r_1()
            .border_color(white(eclipse_theme::glass::EDGE))
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
        let mut menu = ui::popover("account-menu").on_mouse_down_out(cx.listener(
            |this, event: &MouseDownEvent, _, cx| {
                this.close_menu();
                this.menu_closed_at = Some(event.position);
                cx.notify();
            },
        ));
        for (who, name, how) in self.players() {
            let chosen = who == now;
            let face = match &who {
                Player::Microsoft(uuid) => Some(uuid.clone()),
                _ => Some(profile_heads::offline_key(&name)),
            };
            menu = menu.child(
                ui::menu_item(
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
                        .child(
                            div()
                                .size(px(MENU_HEAD))
                                .rounded(px(radius::CONTROL))
                                .overflow_hidden()
                                .child(self.profile_heads.draw(
                                    face.as_deref(),
                                    MENU_HEAD,
                                    self.scale,
                                )),
                        ),
                )
                .when(chosen, |d| d.child(ui::tick()))
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.play_as(who.clone());
                    cx.notify();
                })),
            );
        }
        let sign_in = !matches!(now, Player::Microsoft(_))
            && eclipse_launcher::account::client_id().is_some();
        let mut more = div().mt(px(space::XS)).flex().flex_col();
        if sign_in {
            more = more.child(
                ui::menu_row(
                    "account-sign-in",
                    "user-add-01",
                    tr("Sign in with Microsoft"),
                    false,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.close_menu();
                    this.start_sign_in();
                    cx.notify();
                })),
            );
        }
        menu = menu.child(more.child(
            ui::menu_row("account-manage", "user", tr("Manage accounts"), false).on_press(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.close_menu();
                    this.go(Page::Settings);
                    cx.notify();
                }),
            ),
        ));
        Some(deferred(
            div()
                .absolute()
                .top(px(account_top(compact)
                    + account_chip(compact)
                    + ui::MENU_GAP / 2.))
                .left(px(ui::SIDEBAR_PAD))
                .child(menu),
        ))
    }

    /// The ⋯ menu of a sidebar row, beside it: Edit…, Duplicate…, Open folder, Export…,
    /// launch settings, the desktop shortcut, then Delete… (then asked to confirm). Each acts on
    /// that instance.
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
        let at = point(row.right() - px(MORE_INSET) + px(ui::MENU_GAP), row.top());
        let item = |key: &'static str, icon: &'static str, label: String| {
            ui::menu_row(format!("row-menu-{key}"), icon, label, false)
        };
        let pick = |this: &mut Launcher, i: &Instance| {
            this.close_menu();
            this.select_instance(i);
            this.go(Page::Home);
        };
        let (edit, copy, export, launch, shortcut) = (
            instance.clone(),
            instance.clone(),
            instance.clone(),
            instance.clone(),
            instance.clone(),
        );
        let home = instance.home.clone();
        let shortcut_label = if self.has_shortcut(&instance.id) {
            tr("Remove the desktop shortcut")
        } else {
            tr("Create a desktop shortcut")
        };
        let mut menu = ui::popover("row-menu")
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                this.close_menu();
                this.menu_closed_at = Some(event.position);
                cx.notify();
            }))
            .child(
                item("edit", "pencil-edit-02", tr("Edit…").into()).on_press(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.close_menu();
                        this.open_instance_page(Some(edit.clone()));
                        cx.notify();
                    },
                )),
            )
            .child(
                item("duplicate", "copy-01", tr("Duplicate…").into()).on_press(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        pick(this, &copy);
                        this.start_copy();
                        cx.notify();
                    },
                )),
            )
            .child(
                item("folder", "folder-01", tr("Open folder").into()).on_press(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.close_menu();
                        let _ = std::fs::create_dir_all(&home);
                        crate::toast::open(cx, &home);
                        cx.notify();
                    },
                )),
            )
            .child(
                item("export", "upload-04", tr("Export…").into()).on_press(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        pick(this, &export);
                        if this.export.is_none() {
                            this.toggle_export();
                        }
                        cx.notify();
                    },
                )),
            )
            .child(
                item("launch", "sliders-horizontal", tr("Launch options…").into()).on_press(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        pick(this, &launch);
                        // A panel left open for another instance must not turn this into Close.
                        this.launch = None;
                        this.toggle_launch();
                        cx.notify();
                    }),
                ),
            )
            .child(
                item("shortcut", "link-01", shortcut_label.into()).on_press(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.close_menu();
                        this.toggle_shortcut(shortcut.id.clone(), shortcut.name.clone());
                        cx.notify();
                    },
                )),
            );
        // Eclipse's own instance can't be deleted, nor one while a game runs.
        let can_delete = instance.kind != InstanceKind::Default
            && self.client.is_none()
            && self.starting_client.is_none();
        let gone = instance.id.clone();
        let delete = if confirm {
            ui::menu_confirm(
                "row-menu-delete",
                trf("Delete {name}?", &[("name", &instance.name)]),
                tr("Delete"),
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
            let ask = ask.mt(px(space::XS));
            if can_delete {
                let asked = instance.id.clone();
                ask.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.asking = Some(Asking::InstanceDelete(asked.clone()));
                    cx.notify();
                }))
                .into_any_element()
            } else {
                ui::disabled(ask).into_any_element()
            }
        };
        menu = menu.child(delete);
        Some(
            deferred(
                anchored()
                    .position(at)
                    .snap_to_window_with_margin(px(WINDOW_MARGIN))
                    .child(menu),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}
