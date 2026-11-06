use super::*;
use crate::glass_ui::{self as ui, white, Tone};
use crate::instances_page::java_line;
use eclipse_launcher::account::Player;
use eclipse_launcher::instances::{Icon, Instance, Kind as InstanceKind};
use eclipse_theme::glass::{fill, ink, radius};
use gpui::{anchored, canvas, deferred, point, ScrollHandle};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub const SIDEBAR: f32 = 236.;

/// The ⋮ menu of a sidebar row while open: whose, and whether Delete… was pressed once.
#[derive(Clone, Default)]
pub struct RowMenu {
    pub open: Option<(String, bool)>,
    /// Where each row was drawn last, by instance id: the menu opens beside its row.
    pub rows: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    list: ScrollHandle,
    shown: Rc<RefCell<Option<(String, Pixels)>>>,
}

/// Records where the element it sits in is drawn, under `id`.
fn measure(rows: &Rc<RefCell<HashMap<String, Bounds<Pixels>>>>, id: String) -> impl IntoElement {
    let rows = rows.clone();
    canvas(
        move |bounds, _, _| {
            rows.borrow_mut().insert(id, bounds);
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

impl Launcher {
    /// An instance's icon on a `size` tile: Eclipse's mark for Eclipse's own instance; else its
    /// picture or the block chosen for it; else its loader's or Minecraft's mark, each two thirds
    /// of the tile on the 8 px grid; else its initial on its colour (the accent when none was
    /// chosen).
    pub(super) fn glass_instance_icon(&mut self, i: &Instance, size: f32) -> AnyElement {
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
        let mut colour = self.look.accent;
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
        let letter = (size * 0.44).round();
        tile.bg(ui::color(colour))
            .font_family(eclipse_theme::glass::DISPLAY_FONT)
            .font_weight(FontWeight::BOLD)
            .text_size(px(letter))
            .line_height(px(letter))
            .text_color(gpui::white())
            .child(instances_page::initial(&i.name))
            .into_any_element()
    }

    /// A sidebar row's state line and its dot (the mockup's `statusOf`), from what the launcher
    /// knows of the instance.
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
                    (Some(java), _, _) => (Tone::Live, java_line(java)),
                    (None, None, Some(own)) => (
                        Tone::Warn,
                        trf("{version} is not installed", &[("version", own)]),
                    ),
                    (None, None, None) => (Tone::Off, tr("Not installed").into()),
                    // Following the launcher's version, which has a newer one waiting.
                    (None, Some(_), None) if self.updates.eclipse.is_some() => (
                        Tone::Accent(self.look.accent_soft),
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

    /// The instances by group (`sections` in the mockup): those without a group first.
    fn glass_instances(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let t = self.theme;
        let selected = self.selected_instance().map(|i| i.id.clone());
        let on_packs = self.page == Page::Mods && self.mods.on_modpacks();
        let mut out = Vec::new();
        let mut group = None;
        let mut list = div().flex().flex_col().gap(px(4.));
        let flush = |list: Div, out: &mut Vec<AnyElement>| {
            out.push(list.into_any_element());
            div().flex().flex_col().gap(px(4.))
        };
        for i in self.instances.list.clone() {
            if i.group != group {
                list = flush(list, &mut out);
                group = i.group.clone();
                if let Some(g) = &group {
                    out.push(
                        ui::label(g.clone())
                            .px(px(ui::SIDEBAR_INSET - ui::SIDEBAR_PAD))
                            .pt(px(16.))
                            .pb(px(6.))
                            .into_any_element(),
                    );
                }
            }
            // As in the mockup, the chosen instance stays marked on every page, unless a ported
            // pack's page is open: that row is the one shown then.
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
            let icon = self.glass_instance_icon(&i, 36.);
            let clicked = i.clone();
            let open = self
                .row_menu
                .open
                .as_ref()
                .is_some_and(|(id, _)| *id == i.id);
            let menu_id = i.id.clone();
            // A right click on the row opens the same menu the ⋮ does.
            let right_id = i.id.clone();
            let more = div()
                .absolute()
                .top_0()
                .bottom_0()
                .right(px(6.))
                .flex()
                .items_center()
                .child(
                    ui::icon_button(
                        format!("side-more-{}", i.id),
                        "more-vertical",
                        32.,
                        18.,
                        radius::CONTROL,
                        open,
                        &t,
                    )
                    .on_click(cx.listener(
                        move |this, event: &ClickEvent, _, cx| {
                            if this.closed_by_this_press(event) {
                                return;
                            }
                            this.row_menu.open =
                                row_menu_after_press(this.row_menu.open.take(), &menu_id);
                            cx.notify();
                        },
                    )),
                );
            list = list.child(
                div()
                    .relative()
                    .child(measure(&self.row_menu.rows, i.id.clone()))
                    .child(
                        ui::row(format!("side-instance-{}", i.id), current, 40., &t)
                            .pl(px(ui::SIDEBAR_INSET - ui::SIDEBAR_PAD))
                            .child(div().relative().flex_none().child(icon))
                            .child(ui::row_text(i.name.clone(), state))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                match clicked.kind {
                                    InstanceKind::Default | InstanceKind::Own => {
                                        this.select_instance(&clicked);
                                        this.go(Page::Play);
                                    }
                                    InstanceKind::Port => {
                                        this.show_modpack(Some(clicked.id["port:".len()..].into()))
                                    }
                                    InstanceKind::Pack => this.show_modpack(None),
                                }
                                cx.notify();
                            }))
                            // The ⋮ menu, from a right click anywhere on the row.
                            .on_mouse_down(
                                gpui::MouseButton::Right,
                                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                    // The open menu's mouse-down-out already closed it on this
                                    // press: reopening here would make the toggle a no-op.
                                    if this.menu_closed_at.take() == Some(event.position) {
                                        cx.notify();
                                        return;
                                    }
                                    this.row_menu.open =
                                        row_menu_after_press(this.row_menu.open.take(), &right_id);
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

    fn nav_item(
        &self,
        page: Page,
        label: &'static str,
        icon: &'static str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        ui::nav_row(
            format!("nav-{label}"),
            icon,
            tr(label),
            self.page == page,
            &self.theme,
        )
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.go(page);
            cx.notify();
        }))
    }

    fn account_block(&mut self, compact: bool, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let (name, how) = self.who_plays();
        let face = match self.accounts.player() {
            Player::Microsoft(_) => self.accounts.selected().map(|a| a.uuid.clone()),
            _ => Some(profile_heads::offline_key(&name)),
        };
        let open = self.account_menu;
        let chip = div()
            .id("account")
            .relative()
            .flex()
            .items_center()
            .gap(px(10.))
            .h(px(if compact { 44. } else { 52. }))
            .px(px(ui::SIDEBAR_INSET - ui::SIDEBAR_PAD))
            .rounded(px(radius::ROW))
            .when(open, |d| d.bg(white(fill::ACTIVE)))
            .when(!open, |d| d.hover(|s| s.bg(white(fill::HOVER))))
            .cursor_pointer()
            .child(
                div()
                    .flex_none()
                    .size(px(32.))
                    .rounded(px(radius::CONTROL))
                    .overflow_hidden()
                    .child(
                        self.profile_heads
                            .draw(face.as_deref(), 32., self.scale, &t),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        ui::text(14., FontWeight::SEMIBOLD, ink::FULL)
                            .line_height(px(18.))
                            .truncate()
                            .child(name),
                    )
                    .child(
                        ui::text(12., FontWeight::NORMAL, ink::MUTED)
                            .line_height(px(16.))
                            .truncate()
                            .child(how),
                    ),
            )
            .child(
                ui::icon("arrow-down-01", 16., white(ink::MUTED)).with_transformation(
                    gpui::Transformation::rotate(gpui::radians(if open {
                        std::f32::consts::PI
                    } else {
                        0.
                    })),
                ),
            )
            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                if this.closed_by_this_press(event) {
                    return;
                }
                this.account_menu = !this.account_menu;
                if this.account_menu {
                    this.accounts = Accounts::load();
                    this.load_profile_heads();
                }
                cx.notify();
            }));
        let sign_in = (!matches!(self.accounts.player(), Player::Microsoft(_))
            && eclipse_launcher::account::client_id().is_some())
        .then(|| {
            keyboard(
                div()
                    .id("side-sign-in")
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(10.))
                    .h(px(36.))
                    .mt(px(6.))
                    .rounded(px(radius::CONTROL))
                    .bg(white(fill::BUTTON))
                    .hover(|s| s.bg(white(fill::ACTIVE)))
                    .cursor_pointer()
                    .child(microsoft::logo(14.))
                    .child(
                        ui::text(13., FontWeight::MEDIUM, ink::FULL)
                            .child(tr("Sign in with Microsoft")),
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.start_sign_in();
                        cx.notify();
                    })),
                &t,
            )
        });
        div()
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .px(px(ui::SIDEBAR_PAD))
            .pt(px(if compact { 8. } else { 10. }))
            .child(keyboard(chip, &t))
            .children(sign_in)
            .children(self.glass_account_menu(compact, cx))
    }

    pub(super) fn glass_sidebar(&mut self, window: &Window, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let soft = self.look.accent_soft;
        self.load_packs();
        let compact = window.viewport_size().height < px(720.);
        let account = self.account_block(compact, cx);
        let mut nav = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(if compact { 0. } else { 2. }))
            .px(px(ui::SIDEBAR_PAD))
            .pt(px(if compact { 8. } else { 14. }));
        for (page, label, icon) in Page::NAV {
            nav = nav.child(
                self.nav_item(page, label, icon, cx)
                    .when(compact, |d| d.h(px(36.))),
            );
        }
        let add = ui::icon_button(
            "add-instance",
            "plus-sign",
            28.,
            15.,
            radius::CONTROL,
            false,
            &t,
        )
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.open_instance_dialog(None);
            cx.notify();
        }));
        let header = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .pl(px(ui::SIDEBAR_INSET))
            .pr(px(14.))
            .pt(px(18.))
            .pb(px(6.))
            .child(ui::label(tr("Instances")))
            .child(add);
        let mut list = div()
            .id("side-list")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.row_menu.list)
            .px(px(ui::SIDEBAR_PAD))
            .children(self.glass_instances(cx));

        let servers: Vec<_> = self
            .servers
            .sidebar_list()
            .map(<[_]>::to_vec)
            .unwrap_or_else(|| self.side_servers.clone());
        if !servers.is_empty() {
            list = list.child(
                ui::label(tr("Servers"))
                    .px(px(ui::SIDEBAR_INSET - ui::SIDEBAR_PAD))
                    .pt(px(16.))
                    .pb(px(6.)),
            );
        }
        let mut server_rows = div().flex().flex_col().gap(px(2.));
        for e in &servers {
            let up = self.servers.is_up(&e.dir);
            let open = self.page == Page::Servers && self.servers.open.as_ref() == Some(&e.dir);
            let dir = e.dir.clone();
            server_rows = server_rows.child(
                ui::row(format!("side-server-{}", e.dir.display()), open, 8., &t)
                    .pl(px(ui::SIDEBAR_INSET - ui::SIDEBAR_PAD))
                    .child(
                        div()
                            .relative()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(32.))
                            .rounded(px(radius::CONTROL))
                            .bg(white(fill::BUTTON))
                            .child(ui::icon("computer-terminal-01", 16., white(ink::STRONG))),
                    )
                    .child(ui::row_text(
                        e.name.clone(),
                        if up {
                            (Tone::Live, tr("Running").into())
                        } else {
                            (Tone::Off, tr("Stopped").into())
                        },
                    ))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.open_server(dir.clone());
                        this.go(Page::Servers);
                        cx.notify();
                    })),
            );
        }
        let scroll = &self.row_menu.list;
        let more_below = -scroll.offset().y < scroll.max_offset().y - px(1.);
        let side = t.side.opacity(0.94);
        let list = div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(list.child(server_rows))
            .child(self.keep_selected_shown())
            .when(more_below, |d| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(px(24.))
                        .bg(gpui::linear_gradient(
                            180.,
                            gpui::linear_color_stop(side.opacity(0.), 0.),
                            gpui::linear_color_stop(side, 1.),
                        )),
                )
            });

        let mut foot = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(2.))
            .px(px(ui::SIDEBAR_PAD))
            .pt(px(10.))
            .pb(px(10.))
            .border_t_1()
            .border_color(white(eclipse_theme::glass::EDGE));
        let card = |title: String, right: Option<String>| {
            div()
                .flex()
                .flex_col()
                .mb(px(8.))
                .rounded(px(radius::ROW))
                .bg(white(fill::FIELD))
                .p(px(12.))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .justify_between()
                        .child(ui::text(13., FontWeight::SEMIBOLD, ink::FULL).child(title))
                        .children(
                            right.map(|r| ui::text(12., FontWeight::NORMAL, ink::MUTED).child(r)),
                        ),
                )
        };
        match &self.self_update {
            SelfUpdate::Downloading(version, f) => {
                foot = foot.child(
                    card(
                        trf("Launcher {version}", &[("version", version)]),
                        Some(format!("{:.0}%", f * 100.)),
                    )
                    .child(div().mt(px(10.)).child(ui::bar(*f, soft))),
                );
            }
            SelfUpdate::Ready(version) => {
                foot = foot.child(
                    card(tr("Launcher update available").to_string(), None)
                        .child(
                            ui::text(12., FontWeight::NORMAL, ink::MUTED)
                                .mt(px(4.))
                                .child(trf("Launcher {version}", &[("version", version)])),
                        )
                        .child(
                            crate::button("update-launcher", "Update launcher", Kind::Primary, &t)
                                .mt(px(10.))
                                .w_full()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.apply_launcher_update(cx);
                                    cx.notify();
                                })),
                        ),
                );
            }
            SelfUpdate::Stuck(error) if !self.self_update_hidden => {
                let why = self.launcher_update_help(error.as_deref());
                let url = backend::download_page(&self.links.website);
                foot = foot.child(
                    card(tr("Launcher update available").to_string(), None)
                        .child(
                            ui::text(12., FontWeight::NORMAL, ink::MUTED)
                                .mt(px(6.))
                                .child(why),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .mt(px(10.))
                                .when(self.launcher_download_available(), |actions| {
                                    actions.child(
                                        crate::button(
                                            "download-launcher",
                                            "Download again",
                                            Kind::Secondary,
                                            &t,
                                        )
                                        .w_full()
                                        .on_click(move |_, _, cx| cx.open_url(&url)),
                                    )
                                })
                                .child(
                                    crate::button(
                                        "dismiss-launcher-update",
                                        "Dismiss",
                                        Kind::Ghost,
                                        &t,
                                    )
                                    .w_full()
                                    .on_click(cx.listener(
                                        |this, _: &ClickEvent, _, cx| {
                                            this.self_update_hidden = true;
                                            cx.notify();
                                        },
                                    )),
                                ),
                        ),
                );
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
                    &t,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.native_log_open();
                    cx.notify();
                })),
            );
        }
        foot = foot
            .child(
                self.nav_item(Page::Settings, "Settings", "sliders-horizontal", cx)
                    .when(compact, |d| d.h(px(36.))),
            )
            .child(
                ui::text(12., FontWeight::NORMAL, ink::DIM)
                    .id("launcher-version")
                    .px(px(ui::SIDEBAR_INSET - ui::SIDEBAR_PAD))
                    .pt(px(6.))
                    .child(trf("Launcher {version}", &[("version", backend::VERSION)]))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.version_clicked();
                        cx.notify();
                    })),
            );

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

    /// The menu under the player (`Switch player`): everyone who can play, then Manage accounts.
    fn glass_account_menu(
        &self,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        if !self.account_menu {
            return None;
        }
        let t = self.theme;
        let soft = self.look.accent_soft;
        let now = self.accounts.player();
        let mut menu = ui::popover(250.)
            .id("account-menu")
            .occlude()
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                this.account_menu = false;
                this.menu_closed_at = Some(event.position);
                cx.notify();
            }));
        for (i, (who, name, how)) in self.players().into_iter().enumerate() {
            let chosen = who == now;
            // The mockup's rows say only "Offline"; the old menu's longer line stays there.
            let how = if who == Player::GameName {
                tr("Offline")
            } else {
                how
            };
            let face = match &who {
                Player::Microsoft(uuid) => Some(uuid.clone()),
                _ => Some(profile_heads::offline_key(&name)),
            };
            menu = menu.child(
                ui::menu_item(
                    format!("account-pick-{i}"),
                    name.into(),
                    how.into(),
                    false,
                    &t,
                )
                .pl(px(54.))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(12.))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .size(px(30.))
                                .rounded(px(radius::CONTROL))
                                .overflow_hidden()
                                .child(self.profile_heads.draw(
                                    face.as_deref(),
                                    30.,
                                    self.scale,
                                    &t,
                                )),
                        ),
                )
                .when(chosen, |d| {
                    d.child(ui::icon("tick-02", 16., ui::color(soft)).relative())
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.play_as(who.clone());
                    cx.notify();
                })),
            );
        }
        menu = menu.child(div().mt(px(4.)).child(
            ui::menu_row("account-manage", "user", tr("Manage accounts"), false, &t).on_click(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.account_menu = false;
                    this.go(Page::Settings);
                    cx.notify();
                }),
            ),
        ));
        Some(deferred(
            div()
                .absolute()
                .top(px(if compact { 56. } else { 66. }))
                .left(px(ui::SIDEBAR_PAD))
                .child(menu),
        ))
    }

    /// The ⋮ menu of a sidebar row, beside it: Edit…, Duplicate…, Open folder, Export…,
    /// launch settings, the desktop shortcut, then Delete… (asked twice). Each acts on that instance.
    pub(super) fn row_menu(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (id, confirm) = self.row_menu.open.clone()?;
        let Some(instance) = self.instances.list.iter().find(|i| i.id == id).cloned() else {
            self.row_menu.open = None;
            return None;
        };
        let row = self.row_menu.rows.borrow().get(&id).copied()?;
        let t = self.theme;
        let height = f32::from(window.viewport_size().height);
        // The mockup's `rect.right + 12`, `min(rect.top - 8, innerHeight - 330)`: its ⋮ button
        // ends 6 px inside the row and sits in the row's middle.
        let x = f32::from(row.origin.x + row.size.width) - 6. + 12.;
        let button_top = f32::from(row.origin.y) + (f32::from(row.size.height) - 32.) / 2.;
        let y = (button_top - 8.).min(height - 330.).max(8.);
        let item = |key: &'static str, icon: &'static str, label: String| {
            ui::menu_row(format!("row-menu-{key}"), icon, label, false, &t)
        };
        let pick = |this: &mut Launcher, i: &Instance| {
            this.row_menu.open = None;
            this.select_instance(i);
            this.go(Page::Play);
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
        let mut menu = ui::popover(250.)
            .id("row-menu")
            .occlude()
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                this.row_menu.open = None;
                this.menu_closed_at = Some(event.position);
                cx.notify();
            }))
            .child(
                item("edit", "pencil-edit-02", tr("Edit…").into()).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.row_menu.open = None;
                        this.open_instance_dialog(Some(edit.clone()));
                        cx.notify();
                    },
                )),
            )
            .child(
                item("duplicate", "copy-01", tr("Duplicate…").into()).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        pick(this, &copy);
                        this.start_copy();
                        cx.notify();
                    },
                )),
            )
            .child(
                item("folder", "folder-01", tr("Open folder").into()).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.row_menu.open = None;
                        let _ = std::fs::create_dir_all(&home);
                        crate::glass_toast::open(cx, &home);
                        cx.notify();
                    },
                )),
            )
            .child(
                item("export", "upload-04", tr("Export…").into()).on_click(cx.listener(
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
                item("launch", "sliders-horizontal", tr("Launch options…").into()).on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        pick(this, &launch);
                        this.home_tab = home_tabs::HomeTab::Overview;
                        // A panel left open for another instance must not turn this into Close.
                        this.launch = None;
                        this.toggle_launch();
                        cx.notify();
                    }),
                ),
            )
            .child(
                item("shortcut", "link-01", shortcut_label.into()).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.row_menu.open = None;
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
        let delete = ui::menu_row(
            "row-menu-delete",
            "delete-02",
            if confirm {
                tr("Click again to delete")
            } else {
                tr("Delete…")
            },
            true,
            &t,
        );
        let delete = if can_delete {
            delete.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Some((_, confirm)) = this.row_menu.open.as_mut().filter(|(_, c)| !*c) {
                    *confirm = true;
                } else {
                    this.row_menu.open = None;
                    this.delete_instance(&gone);
                }
                cx.notify();
            }))
        } else {
            crate::disabled(delete)
        };
        menu = menu.child(div().mt(px(4.)).child(delete));
        Some(
            deferred(anchored().position(point(px(x), px(y))).child(menu))
                .with_priority(1)
                .into_any_element(),
        )
    }
}
