//! The instance dialog: one plate over the dimmed page. On the left, what the instance will be
//! (its icon, name and what it plays) and its kind; on the right, what it plays, its name and
//! group, and its icon; Cancel and Create at the foot.
use super::*;
use crate::glass_ui::{self as ui, white};
use eclipse_launcher::instances::{self, Icon, Instance};
use eclipse_launcher::loaders::Kind as Loader;
use eclipse_theme::glass::{fill, ink, radius, EDGE, FLOAT};
use new_instance::{NewKind, VersionList};
use servers_page::Field;
use std::sync::Arc;

/// The loaders Prism Launcher offers, in the mockup's order. Those `loaders::Kind` can install
/// are shown; the others join when the launcher learns them.
const LOADERS: [&str; 5] = ["fabric", "quilt", "neoforge", "forge", "liteloader"];
/// Logos offered as icons (the mockup's `LOGO_ICONS`, but Minecraft's, which is Mojang's).
const LOGO_ICONS: [&str; 4] = ["neoforge", "fabric", "quilt", "forge"];
/// The colours offered.
const COLOURS: [u32; 7] = [
    0x3b82f6, 0x14b8a6, 0x22c55e, 0xeab308, 0xf97316, 0xef4444, 0x8b8a99,
];

/// The loaders a new modded instance can use now.
pub(super) fn loaders() -> Vec<Loader> {
    LOADERS
        .iter()
        .filter_map(|id| Loader::parse(id).ok())
        .collect()
}

pub(super) fn choice(id: impl Into<SharedString>, on: bool, t: &Theme) -> Stateful<Div> {
    keyboard(
        div()
            .id(id.into())
            .flex()
            .rounded(px(radius::POPOVER))
            .cursor_pointer()
            .border_1()
            .border_color(white(EDGE))
            .when(on, |d| d.bg(white(fill::SELECTED)))
            .when(!on, |d| d.hover(|s| s.bg(white(fill::HOVER)))),
        t,
    )
}

/// A choice's two lines: its title at 15 px, semibold, its note at 13 px.
pub(super) fn choice_text(title: impl Into<SharedString>, note: impl Into<SharedString>) -> Div {
    let note: SharedString = note.into();
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .child(
            ui::text(15., FontWeight::SEMIBOLD, ink::FULL)
                .truncate()
                .child(title.into()),
        )
        .when(!note.is_empty(), |d| {
            d.child(
                ui::text(13., FontWeight::NORMAL, ink::MUTED)
                    .line_height(px(18.))
                    .child(note),
            )
        })
}

/// A field group: its heading, anything `aside` at its right, then `body`.
pub(super) fn section(title: SharedString, aside: Option<Div>, body: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .child(
            div()
                .flex()
                .items_end()
                .justify_between()
                .gap(px(16.))
                .mb(px(12.))
                .child(ui::card_heading(title))
                .children(aside),
        )
        .child(body)
}

pub(super) fn list_choice(id: impl Into<SharedString>, on: bool, t: &Theme) -> Stateful<Div> {
    keyboard(
        div()
            .id(id.into())
            .flex()
            .flex_none()
            .items_center()
            .gap(px(16.))
            .px(px(16.))
            .rounded(px(radius::ROW))
            .cursor_pointer()
            .border_1()
            .when(on, |d| {
                d.bg(white(fill::SELECTED)).border_color(white(EDGE))
            })
            .when(!on, |d| {
                d.border_color(gpui::transparent_black())
                    .hover(|s| s.bg(white(fill::HOVER)))
            }),
        t,
    )
}

/// The tick at a list row's end, the accent's soft shade, in a 20 px column.
pub(super) fn tick(on: bool, soft: u32) -> Div {
    div()
        .flex()
        .flex_none()
        .justify_center()
        .w(px(20.))
        .children(on.then(|| ui::icon("tick-02", 18., ui::color(soft))))
}

pub(super) fn list_box(id: &'static str, max: f32) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_col()
        .max_h(px(max))
        .overflow_y_scroll()
        .on_scroll(|_, _, cx| cx.stop_propagation())
        .p(px(6.))
        .rounded(px(radius::POPOVER))
        .border_1()
        .border_color(white(EDGE))
}

pub(super) fn letter_tile(colour: Hsla, name: &str, size: f32) -> Div {
    let letter = (size * 0.46).round();
    ui::tile(size)
        .bg(colour)
        .font_family(if letter >= 40. {
            eclipse_theme::glass::DISPLAY_FONT
        } else {
            eclipse_theme::glass::HEADING_FONT
        })
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(letter))
        .line_height(px(letter))
        .text_color(gpui::white())
        .child(instances_page::initial(name))
}

/// An image `size` px square, round at 26 % (`LogoIcon`).
pub(super) fn tile_image(image: Arc<gpui::RenderImage>, size: f32) -> AnyElement {
    gpui::img(image)
        .flex_none()
        .size(px(size))
        .rounded(px((size * 0.26).round()))
        .into_any_element()
}

fn icon_choice(id: impl Into<SharedString>, on: bool, t: &Theme) -> Stateful<Div> {
    keyboard(
        div()
            .id(id.into())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(52.))
            .rounded(px(radius::POPOVER))
            .cursor_pointer()
            .border_1()
            .when(on, |d| {
                d.bg(white(fill::SELECTED)).border_color(white(EDGE))
            })
            .when(!on, |d| {
                d.border_color(gpui::transparent_black())
                    .hover(|s| s.bg(white(fill::HOVER)))
            }),
        t,
    )
}

impl Launcher {
    pub(super) fn glass_instance_dialog(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let d = self.instances.dialog.as_ref()?;
        let busy = d.busy;
        let editing = d
            .editing
            .as_ref()
            .and_then(|id| self.instances.list.iter().find(|i| &i.id == id))
            .cloned();
        let kind = d.java.kind;
        let aside = self.instance_aside(editing.as_ref(), kind, cx);
        let main = self.instance_main(editing.as_ref(), kind, cx);
        let close = cx.listener(move |this, _: &MouseDownEvent, _, cx| {
            if !busy {
                this.instances.dialog = None;
            }
            cx.notify();
        });
        let height = self.instances.dialog_height.clone();
        let measure = gpui::canvas(
            move |bounds, window, _| {
                let h = f32::from(bounds.size.height);
                if height.get() != h {
                    height.set(h);
                    window.request_animation_frame();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let card = crate::glass::float(radius::MODAL)
            .id("instance-card")
            .occlude()
            .flex()
            .w_full()
            .max_w(px(1180.))
            .h_full()
            .max_h(px(820.))
            .child(aside)
            .child(main);
        Some(
            div()
                .id("instance-dialog")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p(px(24.))
                .occlude()
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .child(ui::scrim_backdrop())
                .child(measure)
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .bg(ui::scrim())
                        .on_mouse_down(gpui::MouseButton::Left, close),
                )
                .child(card),
        )
    }

    /// The loader a modded instance plays (or will).
    pub(super) fn draft_loader(&self) -> Loader {
        self.instances
            .dialog
            .as_ref()
            .map_or(Loader::Fabric, |d| d.java.loader)
    }

    /// The Minecraft version a Java instance plays (or will), once Mojang's list says.
    pub(super) fn draft_minecraft(&self) -> Option<String> {
        let versions = match &self.instances.versions {
            VersionList::Ready(v) => Some(v),
            _ => None,
        };
        self.instances.dialog.as_ref()?.java.minecraft(versions)
    }

    /// The Eclipse version an instance plays: the one it keeps, else the one it follows.
    pub(super) fn draft_eclipse(&self) -> Option<String> {
        let pin = self.instances.dialog.as_ref()?.pin.clone();
        pin.or_else(|| self.followed_version())
    }

    /// The version an instance that keeps none plays: the launcher's, or with none installed
    /// yet, the newest Nightly, which Create installs.
    fn followed_version(&self) -> Option<String> {
        self.launcher_version()
            .cloned()
            .or_else(|| self.eclipse_choices().into_iter().next())
    }

    /// The Eclipse versions the dialog offers (the mockup's `data.nightlies`): see
    /// [`eclipse_choices`].
    pub(super) fn eclipse_choices(&self) -> Vec<String> {
        let releases = match &self.releases {
            Load::Ready(r) => r.as_slice(),
            _ => &[],
        };
        eclipse_choices(releases, &self.installed)
    }

    /// The release to download for the Eclipse version `tag`, when it isn't installed.
    pub(super) fn release_to_install(&self, tag: &str) -> Option<Release> {
        let Load::Ready(releases) = &self.releases else {
            return None;
        };
        (!self.installed.iter().any(|t| t == tag))
            .then(|| releases.iter().find(|r| r.tag == tag && r.asset.is_some()))
            .flatten()
            .cloned()
    }

    /// The mark an instance shows when nothing is chosen: Eclipse's, the grass block, its
    /// loader's logo.
    fn auto_mark(&mut self, kind: NewKind, loader: Loader, size: f32) -> AnyElement {
        let mark = match kind {
            NewKind::Eclipse => return self.eclipse_mark(size),
            NewKind::Vanilla => self.grass_mark(size),
            NewKind::Modded => self.loader_logo(loader.id(), size),
        };
        match mark {
            Some(image) if kind == NewKind::Vanilla => {
                instance_icons::centred(image, size, self.scale).into_any_element()
            }
            Some(image) => tile_image(image, size),
            None => {
                let name = match kind {
                    NewKind::Vanilla => "Vanilla",
                    _ => loader.name(),
                };
                letter_tile(white(fill::BUTTON), name, size).into_any_element()
            }
        }
    }

    /// The icon the instance will show, `size` px.
    fn draft_icon(
        &mut self,
        editing: Option<&Instance>,
        kind: NewKind,
        name: &str,
        size: f32,
    ) -> AnyElement {
        let Some(d) = self.instances.dialog.as_ref() else {
            return div().into_any_element();
        };
        let d_logo = d.logo;
        let (icon, picture) = (
            d.icon.clone(),
            d.picture
                .as_ref()
                .map(|p| p.as_ref().map(|c| c.png.clone())),
        );
        let image = match picture {
            Some(Some(png)) => {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                png.hash(&mut h);
                self.icon_image(format!("chosen:{:x}", h.finish()), size, false, || {
                    Some(png)
                })
            }
            Some(None) => None,
            None => editing.and_then(|i| self.instance_picture(i, size)),
        };
        if let Some(image) = image {
            // A loader's logo is a tile, round as the others; a picture keeps its own shape.
            return match d_logo {
                Some(_) => tile_image(image, size),
                None => instance_icons::centred(image, size, self.scale).into_any_element(),
            };
        }
        match icon {
            Some(Icon::Colour(hex)) => {
                let rgb = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
                letter_tile(ui::color(rgb), name, size).into_any_element()
            }
            Some(Icon::Texture(texture)) => match self.texture_image(&texture, size) {
                Some(image) => tile_image(image, size),
                None => letter_tile(ui::color(self.look.accent), name, size).into_any_element(),
            },
            None => {
                let loader = self.draft_loader();
                self.auto_mark(kind, loader, size)
            }
        }
    }

    /// The left column: the instance as it will be, and its kind.
    fn instance_aside(
        &mut self,
        editing: Option<&Instance>,
        kind: NewKind,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let t = self.theme;
        let typed = self.servers.value(Field::InstanceName).trim().to_string();
        let name = if typed.is_empty() {
            tr("New instance").to_string()
        } else {
            typed
        };
        let loader = self.draft_loader();
        let minecraft = trf(
            "Minecraft {version}",
            &[(
                "version",
                &self.draft_minecraft().unwrap_or_else(|| "…".into()),
            )],
        );
        let mut summary = match kind {
            NewKind::Eclipse => match self.draft_eclipse() {
                Some(tag) => format!("Eclipse · {}", version_label(&tag)),
                None => "Eclipse".into(),
            },
            NewKind::Vanilla => format!("{} · {minecraft}", tr("Vanilla")),
            NewKind::Modded => format!("{} · {minecraft}", loader.name()),
        };
        let group = self.servers.value(Field::InstanceGroup).trim().to_string();
        if !group.is_empty() {
            summary = format!("{summary} · {group}");
        }
        let short = (1. ..700.).contains(&self.instances.dialog_height.get());
        let icon = self.draft_icon(editing, kind, &name, if short { 64. } else { 88. });
        // One made in the launcher can switch what it plays; the game's home and modpacks
        // play what they were made with.
        let switches = editing.is_none_or(|i| i.kind == instances::Kind::Own);
        let kinds = [NewKind::Eclipse, NewKind::Vanilla, NewKind::Modded];
        let names: Vec<&str> = loaders().iter().map(|l| l.name()).collect();
        let modded = match names.split_last() {
            Some((last, [])) => last.to_string(),
            Some((last, rest)) => trf(
                "{first} or {last}",
                &[("first", &rest.join(", ")), ("last", last)],
            ),
            None => String::new(),
        };
        let mut cards = Vec::new();
        for k in kinds {
            let (id, title, note) = match k {
                NewKind::Eclipse => (
                    "eclipse",
                    tr("Eclipse"),
                    tr("Plays Eclipse mods").to_string(),
                ),
                NewKind::Vanilla => (
                    "vanilla",
                    tr("Vanilla"),
                    tr("Minecraft as Mojang ships it").to_string(),
                ),
                NewKind::Modded => ("modded", tr("Modded"), modded.clone()),
            };
            let tile = match k {
                NewKind::Modded => ui::tile(42.)
                    .child(ui::icon("puzzle", 21., white(ink::STRONG)))
                    .into_any_element(),
                _ => self.auto_mark(k, loader, 42.),
            };
            let card = choice(format!("new-kind-{id}"), k == kind, &t)
                .items_center()
                .gap(px(14.))
                .p(px(12.))
                .child(tile)
                .child(choice_text(title, if short { String::new() } else { note }));
            cards.push(if !switches {
                card.cursor_default().when(k != kind, crate::disabled)
            } else {
                card.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.pick_kind(k, cx);
                    cx.notify();
                }))
            });
        }
        // A modpack becomes an instance by being imported, on the Mods page. Edit offers it too,
        // as the mockup's does.
        let import = (!self.instances.dialog.as_ref().is_some_and(|d| d.busy)).then(|| {
            ui::button(
                "instance-import",
                (!short).then_some("package-01"),
                tr("Import a modpack…"),
                Kind::Secondary,
                ui::Size::Md,
                &t,
            )
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.instances.dialog = None;
                this.show_modpack(None);
                cx.notify();
            }))
        });
        div()
            .id("instance-aside")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(if short { 220. } else { 340. }))
            .h_full()
            .p(px(if short { 20. } else { 28. }))
            .overflow_y_scroll()
            .border_r_1()
            .border_color(white(EDGE))
            .child(ui::label(tr(if editing.is_some() {
                "Edit instance"
            } else {
                "New instance"
            })))
            .child(
                div()
                    .id("aside-instance-icon")
                    .mt(px(if short { 16. } else { 24. }))
                    .cursor_pointer()
                    .hover(|s| s.opacity(crate::glass_ui::HOVER_OPACITY))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.choose_instance_picture(cx);
                    }))
                    .child(icon),
            )
            .child(
                ui::entity_title(name, 24.)
                    .flex_none()
                    .mt(px(if short { 12. } else { 20. }))
                    .truncate(),
            )
            .child(
                ui::text(14., FontWeight::NORMAL, ink::MUTED)
                    .mt(px(8.))
                    .child(summary),
            )
            .child(
                div()
                    .mt(px(if short { 20. } else { 32. }))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .children(cards),
            )
            .child(div().flex_1().min_h(px(if short { 16. } else { 32. })))
            .children(import)
    }
}

/// Every Nightly published with a build for this platform, newest first, then the installed
/// versions that list lacks (older Nightlies, imported builds). Picking one not installed installs
/// it on Create.
pub(super) fn eclipse_choices(releases: &[Release], installed: &[String]) -> Vec<String> {
    let published = releases
        .iter()
        .filter(|r| r.nightly.is_some() && r.asset.is_some())
        .map(|r| &r.tag);
    let mut out: Vec<String> = Vec::new();
    for tag in published.chain(installed) {
        if !out.contains(tag) {
            out.push(tag.clone());
        }
    }
    out
}

/// "Nightly · c9d474d", "Imported · 2026-10-30", or the tag.
fn version_label(tag: &str) -> String {
    eclipse_launcher::nightly::label(tag)
        .or_else(|| build_import::label(tag))
        .unwrap_or_else(|| tag.to_string())
}

impl Launcher {
    /// The right column: the title, what the instance plays, its name and group, its icon, and
    /// the foot.
    fn instance_main(
        &mut self,
        editing: Option<&Instance>,
        kind: NewKind,
        cx: &mut Context<Self>,
    ) -> Div {
        let t = self.theme;
        let Some(d) = self.instances.dialog.as_ref() else {
            return div();
        };
        let (busy, copy, error) = (d.busy, d.copy, d.error.clone());
        let title = match (editing, kind) {
            (Some(i), _) => trf("Edit {name}", &[("name", &i.name)]),
            (None, NewKind::Eclipse) => tr("New Eclipse instance").into(),
            (None, NewKind::Vanilla) => tr("New vanilla instance").into(),
            (None, NewKind::Modded) => tr("New modded instance").into(),
        };
        let sub = editing.map(|_| tr("Changes apply the next time it starts."));
        // The mockup's `clamp(1.6rem, 2.4vw, 2.1rem)`.
        let size = (self.window_width * 0.024).clamp(25.6, 33.6);
        let head = div()
            .flex()
            .flex_none()
            .items_start()
            .justify_between()
            .gap(px(16.))
            .px(px(36.))
            .pt(px(28.))
            .pb(px(16.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(ui::entity_title(title, size).truncate())
                    .children(sub.map(|sub| {
                        ui::text(14., FontWeight::NORMAL, ink::MUTED)
                            .mt(px(6.))
                            .child(sub)
                    })),
            )
            .child(
                ui::icon_button(
                    "instance-close",
                    "cancel-01",
                    36.,
                    18.,
                    radius::CONTROL,
                    false,
                    &t,
                )
                .when(!busy, |b| {
                    b.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.instances.dialog = None;
                        cx.notify();
                    }))
                }),
            );
        let mut body = div()
            .id("instance-body")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(24.))
            .px(px(36.))
            .pb(px(32.))
            .overflow_y_scroll();
        if let Some(i) = editing {
            body = body.children(self.switch_notice(i));
        }
        let name = self
            .field(Field::InstanceName, tr("My instance"), 340., cx)
            .flex_1()
            .min_w_0();
        let group = self
            .field(Field::InstanceGroup, tr("No group"), 340., cx)
            .flex_1()
            .min_w_0();
        body = body.child(section(
            tr("Name and group").into(),
            None,
            div().flex().gap(px(10.)).child(name).child(group),
        ));
        if kind == NewKind::Eclipse {
            body = body.child(self.eclipse_versions(cx));
            if editing.is_none() {
                body = body.child(self.start_from(copy, cx));
            }
        } else {
            self.load_versions(cx);
            body = body.child(self.minecraft_versions(cx));
            if kind == NewKind::Modded {
                body = body.child(self.loader_choice(cx));
            }
        }
        body = body.child(self.icon_choice(editing, kind, cx));
        // The memory it may use, kept with the instance (not in the mockup, kept here).
        if editing.is_some() {
            body = body.children(self.glass_memory_row(cx));
        }
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(head)
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(body)
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .h(px(16.))
                            .bg(gpui::linear_gradient(
                                180.,
                                gpui::linear_color_stop(ui::color(FLOAT).opacity(0.), 0.),
                                gpui::linear_color_stop(ui::color(FLOAT), 1.),
                            )),
                    ),
            )
            .child(self.instance_foot(editing, copy, busy, error, cx))
    }

    /// The foot: where the instance lives, Cancel and Create.
    fn instance_foot(
        &mut self,
        editing: Option<&Instance>,
        copy: bool,
        busy: bool,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) -> Div {
        let t = self.theme;
        let place = match editing {
            Some(i) => short(&i.home),
            None => String::new(),
        };
        let cancel = button("instance-cancel", "Cancel", Kind::Ghost, &t);
        let label = match (editing.is_some(), busy) {
            (true, _) => tr("Save changes"),
            (false, true) if copy => tr("Copying…"),
            (false, _) => tr("Create instance"),
        };
        let save = ui::solid("instance-save", label, 44., 14., &t);
        let buttons = if busy {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(disabled(cancel))
                .child(disabled(save))
        } else {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(cancel.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.instances.dialog = None;
                    cx.notify();
                })))
                .child(save.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.save_instance(cx);
                    cx.notify();
                })))
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .px(px(36.))
            .py(px(20.))
            .border_t_1()
            .border_color(white(EDGE))
            // Why Create failed takes the folder's place, by the buttons.
            .child(match error {
                Some(e) => ui::text(13., FontWeight::NORMAL, ink::FULL)
                    .min_w_0()
                    .text_color(ui::danger())
                    .child(e),
                None => ui::text(13., FontWeight::NORMAL, ink::MUTED)
                    .min_w_0()
                    .truncate()
                    .child(place),
            })
            .child(buttons.flex_none())
    }

    /// Eclipse's versions, the newest first: picking the one the launcher plays lets the
    /// instance follow it; any other is kept, and installed on Create when it isn't yet.
    fn eclipse_versions(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let soft = self.look.accent_soft;
        let follows = self.followed_version();
        let chosen = self.draft_eclipse();
        let rows: Vec<_> = self
            .eclipse_choices()
            .into_iter()
            .enumerate()
            .map(|(i, tag)| {
                let latest = follows.as_ref() == Some(&tag);
                let newest = i == 0;
                let on = chosen.as_ref() == Some(&tag);
                let date = eclipse_launcher::nightly::date_of(&tag)
                    .map(|d| glass_versions::published(d, ""));
                let pick = tag.clone();
                list_choice(format!("eclipse-version-{tag}"), on, &t)
                    .py(px(12.))
                    .child(choice_text(version_label(&tag), date.unwrap_or_default()).flex_1())
                    .children(newest.then(|| {
                        ui::text(13., FontWeight::NORMAL, ink::FULL)
                            .text_color(ui::color(soft))
                            .child(tr("Newest"))
                    }))
                    .child(tick(on, soft))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.instances.dialog {
                            d.pin = (!latest).then(|| pick.clone());
                        }
                        cx.notify();
                    }))
            })
            .collect();
        let list = if rows.is_empty() {
            let empty = match &self.releases {
                Load::Loading => tr("Loading releases…"),
                _ => tr("No version installed"),
            };
            list_box("eclipse-versions", 248.).child(
                ui::text(14., FontWeight::NORMAL, ink::MUTED)
                    .px(px(16.))
                    .py(px(24.))
                    .child(empty),
            )
        } else {
            list_box("eclipse-versions", 248.).children(rows)
        };
        section(tr("Version").into(), None, list)
    }

    /// Empty, or a copy of the selected instance.
    fn start_from(&mut self, copy: bool, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let selected = self.selected_instance().cloned();
        let from = selected
            .as_ref()
            .map_or_else(|| "Eclipse".into(), |i| i.name.clone());
        let icon = match &selected {
            Some(i) => self.glass_instance_icon(i, 36.),
            None => self.eclipse_mark(36.),
        };
        let card = |on: bool, id: &'static str| {
            choice(id, on, &t)
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(px(12.))
                .p(px(16.))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.instances.dialog {
                        d.copy = id == "instance-from-copy";
                    }
                    cx.notify();
                }))
        };
        let empty = card(!copy, "instance-from-empty")
            .child(ui::tile(36.).child(ui::icon("plus-sign", 18., white(ink::BODY))))
            .child(choice_text(tr("Empty"), tr("A fresh game folder")));
        let copied = card(copy, "instance-from-copy")
            .child(icon)
            .child(choice_text(
                trf("A copy of {name}", &[("name", &from)]),
                tr("Its mods, worlds and options"),
            ));
        section(
            tr("Start from").into(),
            None,
            div().flex().gap(px(10.)).child(empty).child(copied),
        )
    }

    /// The icons: the instance's own mark, the loaders' logos and the prepared files' blocks and
    /// items, then the colours and Choose image….
    fn icon_choice(
        &mut self,
        editing: Option<&Instance>,
        kind: NewKind,
        cx: &mut Context<Self>,
    ) -> Div {
        let t = self.theme;
        let (icon, logo, d_picture) = {
            let Some(d) = self.instances.dialog.as_ref() else {
                return div();
            };
            (d.icon.clone(), d.logo, d.picture.clone())
        };
        let has_saved_picture = editing.is_some_and(|i| i.home.join(instances::PICTURE).is_file());
        let file_picture = matches!(&d_picture, Some(Some(_))) && logo.is_none();
        let own_picture = d_picture.is_none() && has_saved_picture;
        let pictured = matches!(&d_picture, Some(Some(_))) || own_picture;
        let typed = self.servers.value(Field::InstanceName).trim().to_string();
        let name = if typed.is_empty() { "N".into() } else { typed };
        let pick = |icon: Option<Icon>| {
            cx.listener(
                move |this: &mut Self, _: &ClickEvent, _, cx: &mut Context<Self>| {
                    if let Some(d) = &mut this.instances.dialog {
                        d.icon = icon.clone();
                        d.picture = Some(None);
                        d.logo = None;
                    }
                    cx.notify();
                },
            )
        };
        let mut first: Vec<AnyElement> = Vec::new();
        if file_picture || has_saved_picture {
            let picture_img = match &d_picture {
                Some(Some(chosen)) => {
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    chosen.png.hash(&mut h);
                    self.icon_image(format!("chosen:{:x}", h.finish()), 40., false, || {
                        Some(chosen.png.clone())
                    })
                }
                _ => editing.and_then(|i| self.instance_picture(i, 40.)),
            };
            let picture_active = (file_picture || own_picture) && icon.is_none();
            if let Some(image) = picture_img {
                first.push(
                    icon_choice("icon-picture", picture_active, &t)
                        .child(instance_icons::centred(image, 40., self.scale))
                        .on_click(cx.listener(
                            move |this: &mut Self, _: &ClickEvent, _, cx: &mut Context<Self>| {
                                if picture_active {
                                    this.choose_instance_picture(cx);
                                } else if let Some(d) = &mut this.instances.dialog {
                                    d.icon = None;
                                    d.logo = None;
                                    if has_saved_picture {
                                        d.picture = None;
                                    }
                                    d.error = None;
                                }
                                cx.notify();
                            },
                        ))
                        .into_any_element(),
                );
            }
        }
        let loader = self.draft_loader();
        let auto = self.auto_mark(kind, loader, 40.);
        first.push(
            icon_choice("icon-auto", !pictured && icon.is_none(), &t)
                .child(auto)
                .on_click(pick(None))
                .into_any_element(),
        );
        // Our grass block, then the loaders' logos: each becomes the instance's picture.
        let mut logos: Vec<(&'static str, Vec<u8>)> =
            vec![("grass", instance_icons::GRASS.to_vec())];
        for id in LOGO_ICONS {
            if let Some(png) = instance_icons::logo_png(id) {
                logos.push((id, png.to_vec()));
            }
        }
        for (id, png) in logos {
            let shown = match id {
                "grass" => self
                    .icon_image("mark:grass".into(), 40., false, || Some(png.clone()))
                    .map(|i| instance_icons::centred(i, 40., self.scale).into_any_element()),
                _ => self.loader_logo(id, 40.).map(|i| tile_image(i, 40.)),
            };
            let Some(shown) = shown else {
                continue;
            };
            first.push(
                icon_choice(format!("icon-logo-{id}"), logo == Some(id), &t)
                    .child(shown)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let made = instance_icons::chosen(png.clone());
                        if let (Some(d), Some(made)) = (&mut this.instances.dialog, made) {
                            d.picture = Some(Some(made));
                            d.icon = None;
                            d.logo = Some(id);
                            d.error = None;
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        for texture in instances_page::TEXTURES {
            let Some(image) = self.texture_image(texture, 40.) else {
                continue;
            };
            let on = !pictured && icon == Some(Icon::Texture(texture.into()));
            first.push(
                icon_choice(format!("icon-{texture}"), on, &t)
                    .child(tile_image(image, 40.))
                    .on_click(pick(Some(Icon::Texture(texture.into()))))
                    .into_any_element(),
            );
        }
        let colours = COLOURS.map(|rgb| {
            let hex = format!("#{rgb:06x}");
            let on = !pictured && icon == Some(Icon::Colour(hex.clone()));
            icon_choice(format!("icon-{hex}"), on, &t)
                .child(letter_tile(ui::color(rgb), &name, 40.))
                .on_click(pick(Some(Icon::Colour(hex))))
        });
        let choose = ui::button(
            "instance-picture",
            Some("image-01"),
            tr("Choose image…"),
            Kind::Secondary,
            ui::Size::Md,
            &t,
        )
        .ml(px(6.))
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.choose_instance_picture(cx);
        }));
        let mut body = div()
            .flex()
            .flex_col()
            .child(div().flex().flex_wrap().gap(px(6.)).children(first))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .mt(px(6.))
                    .children(colours)
                    .child(choose),
            );
        if self.selected().is_none_or(|t| !backend::is_prepared(t)) {
            body = body.child(
                ui::text(13., FontWeight::NORMAL, ink::MUTED)
                    .mt(px(12.))
                    .child(tr("Prepare a version to use block and item icons.")),
            );
        }
        section(tr("Icon").into(), None, body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, nightly: bool, asset: bool) -> Release {
        Release {
            tag: tag.into(),
            name: String::new(),
            body: String::new(),
            date: String::new(),
            time: String::new(),
            asset: asset.then(|| ("eclipse.tar.gz".into(), 1)),
            prerelease: false,
            nightly: nightly.then(|| "nightly".into()),
        }
    }

    #[test]
    fn the_dialog_offers_every_published_nightly_then_what_only_this_computer_has() {
        let releases = [
            release("nightly-2026-11-03-aaaaaaa", true, true),
            // No build for this platform: nothing to install.
            release("nightly-2026-11-02-bbbbbbb", true, false),
            release("nightly-2026-11-01-ccccccc", true, true),
            // The old Alpha channel's releases aren't Nightlies.
            release("v0.2.1", false, true),
        ];
        let installed = [
            "nightly-2026-11-01-ccccccc".to_string(),
            "nightly-2026-10-20-ddddddd".to_string(),
            "imported-2026-10-30".to_string(),
        ];
        assert_eq!(
            eclipse_choices(&releases, &installed),
            [
                "nightly-2026-11-03-aaaaaaa",
                "nightly-2026-11-01-ccccccc",
                "nightly-2026-10-20-ddddddd",
                "imported-2026-10-30",
            ]
        );
        // Offline, or before the list arrives: what is installed.
        assert_eq!(eclipse_choices(&[], &installed), installed);
    }
}
