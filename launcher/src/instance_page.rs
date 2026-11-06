//! The New instance page: what the instance plays, its name and group, its version and its icon
//! in cards; at the foot the instance as it will be, with Cancel and Create.
use super::*;
use crate::ui::{self, white};
use eclipse_launcher::instances::{self, Icon, Instance};
use eclipse_launcher::loaders::Kind as Loader;
use eclipse_theme::glass::{fill, ink, pixel_text, space};
use fields::Field;
use instance_state::{NewKind, VersionList};

/// The loaders Prism Launcher offers. Those `loaders::Kind` can install are shown; the others
/// join when the launcher learns them.
const LOADERS: [&str; 5] = ["fabric", "quilt", "neoforge", "forge", "liteloader"];
/// Logos offered as icons (not Minecraft's, which is Mojang's).
const LOGO_ICONS: [&str; 4] = ["neoforge", "fabric", "quilt", "forge"];
/// The colours offered.
const COLOURS: [u32; 7] = [
    0x3b82f6, 0x14b8a6, 0x22c55e, 0xeab308, 0xf97316, 0xef4444, 0x8b8a99,
];
const PREVIEW_ICON: f32 = ui::TILE_LG;
const PREVIEW_ICON_SHORT: f32 = ui::TILE_MD;
const CHOICE_ICON: f32 = ui::TILE_MD;
const NAME_FIELD_WIDTH: f32 = 340.;
const ECLIPSE_LIST: f32 = 248.;
const ECLIPSE_COLUMN: f32 = 240.;
pub(super) const CHOICE_NOTE_LINES: usize = 2;

/// The loaders a new modded instance can use now.
pub(super) fn loaders() -> Vec<Loader> {
    LOADERS
        .iter()
        .filter_map(|id| Loader::parse(id).ok())
        .collect()
}

fn foot_height() -> f32 {
    let icon = if ui::short_window() {
        PREVIEW_ICON_SHORT
    } else {
        PREVIEW_ICON
    };
    icon + 2. * ui::PANEL_INSET
}

fn icon_choice(id: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    ui::choice(id, on)
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(ui::TILE_LG))
}

impl Launcher {
    pub(super) fn instance_page(&mut self, cx: &mut Context<Self>) -> Div {
        self.remember_draft();
        let Some(d) = self.instances.draft.as_ref() else {
            return div();
        };
        let (busy, duplicate) = (d.busy, d.duplicate);
        let (error, scroll) = (d.error.clone(), d.scroll.clone());
        let editing = d
            .editing
            .as_ref()
            .and_then(|id| self.instances.list.iter().find(|i| &i.id == id))
            .cloned();
        let kind = d.java.kind;
        let running = editing.as_ref().is_some_and(|i| {
            self.game.client.is_some() && self.selected_instance().is_some_and(|s| s.id == i.id)
        });
        let (title, note): (SharedString, _) = match &editing {
            Some(i) => (
                trf("Edit {name}", &[("name", &i.name)]).into(),
                if running {
                    tr("Changes apply the next time it starts.")
                } else {
                    ""
                },
            ),
            None if duplicate => {
                let from = self.selected_instance().map_or("", |i| i.name.as_str());
                (trf("Duplicate {name}", &[("name", from)]).into(), "")
            }
            None => (tr("New instance").into(), ""),
        };
        let mut cards: Vec<AnyElement> = Vec::new();
        if let Some(i) = &editing {
            cards.extend(self.switch_notice(i).map(IntoElement::into_any_element));
        }
        cards.push(
            self.kind_choice(editing.as_ref(), kind, !busy && !duplicate, cx)
                .into_any_element(),
        );
        if kind == NewKind::Modded {
            cards.push(self.loader_choice(cx).into_any_element());
        }
        if kind == NewKind::Eclipse {
            cards.push(self.eclipse_versions(cx).into_any_element());
        } else {
            self.load_versions(cx);
            cards.push(self.minecraft_versions(cx).into_any_element());
        }
        cards.push(self.name_and_group(editing.as_ref(), cx).into_any_element());
        cards.push(
            self.icon_choice(editing.as_ref(), kind, cx)
                .into_any_element(),
        );
        // The memory it may use, kept with the instance.
        if editing.is_some() {
            cards.extend(self.memory_setting(cx).map(|row| {
                ui::card()
                    .id("instance-memory")
                    .p(px(ui::CARD_PADDING - ui::SETTING_ROW_PADDING))
                    .child(row)
                    .into_any_element()
            }));
        }
        let foot = self.instance_foot(editing.as_ref(), kind, duplicate, busy, error, cx);
        let pane = ui::scroll_pane("instance-pane", &scroll.handle)
            .flex()
            .flex_col()
            .size_full()
            .gap(px(layout::CARD_GAP))
            .children(cards);
        let pane = div()
            .relative()
            .flex_1()
            .min_h_0()
            .pb(px(layout::CONTENT_INSET + foot_height() + layout::CARD_GAP))
            .child(ui::page_pane(
                pane,
                &scroll,
                layout::page_margin(self.window_width),
            ))
            .child(foot);
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .child(ui::page_head(title, note))
            .child(pane)
    }

    pub(super) fn instance_page_compact(&self) -> bool {
        layout::content_width(self.window_width) < layout::INSTANCE_COMPACT_BELOW
    }

    /// The loader a modded instance plays (or will).
    pub(super) fn draft_loader(&self) -> Loader {
        self.instances
            .draft
            .as_ref()
            .map_or(Loader::Fabric, |d| d.java.loader)
    }

    /// The Minecraft version a Java instance plays (or will), once Mojang's list says.
    pub(super) fn draft_minecraft(&self) -> Option<String> {
        let versions = match &self.instances.versions {
            VersionList::Ready(v) => Some(v),
            _ => None,
        };
        self.instances.draft.as_ref()?.java.minecraft(versions)
    }

    /// The Eclipse version an instance plays: the one it keeps, else the one it follows.
    pub(super) fn draft_eclipse(&self) -> Option<String> {
        let pin = self.instances.draft.as_ref()?.pin.clone();
        pin.or_else(|| self.followed_version())
    }

    /// The version an instance that keeps none plays: the launcher's, or with none installed
    /// yet, the newest Nightly, which Create installs.
    fn followed_version(&self) -> Option<String> {
        self.launcher_version()
            .cloned()
            .or_else(|| self.eclipse_choices().into_iter().next())
    }

    /// The Eclipse versions the page offers: see [`eclipse_choices`].
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
            Some(image) => ui::image_tile(image, size),
            None => {
                let name = match kind {
                    NewKind::Vanilla => "Vanilla",
                    _ => loader.name(),
                };
                ui::letter_tile(white(fill::BUTTON), name, size).into_any_element()
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
        let Some(d) = self.instances.draft.as_ref() else {
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
                Some(_) => ui::image_tile(image, size),
                None => instance_icons::centred(image, size, self.scale).into_any_element(),
            };
        }
        match icon {
            Some(Icon::Colour(hex)) => {
                let rgb = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
                ui::letter_tile(ui::color(rgb), name, size).into_any_element()
            }
            Some(Icon::Texture(texture)) => match self.texture_image(&texture, size) {
                Some(image) => ui::image_tile(image, size),
                None => {
                    ui::letter_tile(ui::color(ui::look().accent), name, size).into_any_element()
                }
            },
            None => {
                let loader = self.draft_loader();
                self.auto_mark(kind, loader, size)
            }
        }
    }

    fn draft_summary(&self, kind: NewKind, name: &str) -> String {
        let version = self.draft_minecraft().unwrap_or_else(|| "…".into());
        let plays = match kind {
            NewKind::Eclipse => match self.draft_eclipse() {
                Some(tag) => format!(
                    "Eclipse · {}",
                    eclipse_launcher::nightly::display_name(&tag)
                ),
                None => "Eclipse".into(),
            },
            NewKind::Vanilla => trf("Vanilla {version}", &[("version", &version)]),
            NewKind::Modded => trf(
                "{loader} {version}",
                &[
                    ("loader", self.draft_loader().name()),
                    ("version", &version),
                ],
            ),
        };
        let group = self.fields.value(Field::InstanceGroup).trim().to_string();
        [plays, group]
            .into_iter()
            .filter(|part| !part.is_empty() && part != name)
            .collect::<Vec<_>>()
            .join(" · ")
    }

    fn kind_choice(
        &mut self,
        editing: Option<&Instance>,
        kind: NewKind,
        offers_import: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let loader = self.draft_loader();
        // One made in the launcher can switch what it plays; the game's home and modpacks
        // play what they were made with.
        let switches = editing.is_none_or(|i| i.kind == instances::Kind::Own);
        let marks = !self.instance_page_compact();
        let mut cards = Vec::new();
        let kinds = [NewKind::Eclipse, NewKind::Vanilla, NewKind::Modded];
        for k in kinds {
            let (id, title, note) = match k {
                NewKind::Eclipse => ("eclipse", tr("Eclipse"), tr("Faster Minecraft")),
                NewKind::Vanilla => ("vanilla", tr("Vanilla"), tr("The original Minecraft")),
                NewKind::Modded => ("modded", tr("Modded"), tr("Fabric, Forge and more")),
            };
            let mark = marks.then(|| match k {
                NewKind::Modded => self.modded_mark(ui::MARK),
                _ => self.auto_mark(k, loader, ui::MARK),
            });
            let card = ui::choice_card(format!("new-kind-{id}"), k == kind)
                .items_start()
                .gap(px(space::LG))
                .p(px(space::MD))
                .min_w_0()
                .children(mark.map(ui::mark_tile))
                .child(ui::item_text_wrapped(title, note, CHOICE_NOTE_LINES));
            cards.push((
                if switches {
                    card
                } else {
                    card.cursor_default().when(k != kind, ui::disabled)
                },
                title.into(),
                k == kind,
            ));
        }
        let importing = self.modpacks.busy;
        let import = (offers_import && editing.is_none()).then(|| {
            let import = ui::button(
                "instance-import",
                None,
                if importing {
                    tr("Importing…")
                } else {
                    tr("Import a modpack…")
                },
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            );
            if importing {
                return ui::disabled(import).into_any_element();
            }
            import
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.show_modpack(None);
                    this.choose_pack(false, cx);
                    cx.notify();
                }))
                .into_any_element()
        });
        let grid = div()
            .id("instance-game-choices")
            .grid()
            .grid_cols(cards.len() as u16)
            .gap(px(space::MD));
        let grid = if switches {
            let this = cx.entity().downgrade();
            ui::options(grid, cards, true, move |i, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.pick_kind(kinds[i], cx);
                    cx.notify();
                });
            })
        } else {
            grid.children(cards.into_iter().map(|(card, _, _)| card))
        };
        ui::section_card("instance-game", tr("Game"), import, grid)
    }

    fn name_and_group(
        &mut self,
        editing: Option<&Instance>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let name = self
            .field(
                Field::InstanceName,
                tr("My instance"),
                NAME_FIELD_WIDTH,
                ui::ButtonSize::Lg,
                cx,
            )
            .w_full();
        let group = self
            .field(
                Field::InstanceGroup,
                tr("For example Survival"),
                NAME_FIELD_WIDTH,
                ui::ButtonSize::Lg,
                cx,
            )
            .w_full();
        let labelled = |label: &'static str, field: Stateful<Div>| {
            ui::labelled_field(tr(label), field.aria_label(tr(label)))
                .flex_1()
                .min_w_0()
        };
        ui::card()
            .id("instance-name")
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(space::MD))
            .p(px(ui::CARD_PADDING))
            .child(
                div()
                    .flex()
                    .gap(px(space::MD))
                    .child(labelled("Name", name))
                    .child(labelled("Group (optional)", group)),
            )
            .children(editing.map(|i| {
                let home = i.home.clone();
                div().flex().child(
                    ui::button(
                        "instance-open-folder",
                        Some("folder-01"),
                        tr("Open instance folder"),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
                    )
                    .on_press(move |_, _, cx| crate::toast::open(cx, &home)),
                )
            }))
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

impl Launcher {
    /// The foot: the instance as it will be, Cancel and Create.
    fn instance_foot(
        &mut self,
        editing: Option<&Instance>,
        kind: NewKind,
        duplicate: bool,
        busy: bool,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let typed = self.fields.value(Field::InstanceName).trim().to_string();
        let name = if typed.is_empty() {
            tr("New instance").to_string()
        } else {
            typed
        };
        let summary = self.draft_summary(kind, &name);
        let size = if ui::short_window() {
            PREVIEW_ICON_SHORT
        } else {
            PREVIEW_ICON
        };
        let compact = self.instance_page_compact();
        let icon = (!compact).then(|| self.draft_icon(editing, kind, &name, size));
        let (button, title) = if compact {
            (ui::ButtonSize::Md, pixel_text::PRESS)
        } else {
            (ui::ButtonSize::Lg, pixel_text::CARD_TITLE)
        };
        let cancel = ui::button(
            "instance-cancel",
            None,
            tr("Cancel"),
            ButtonKind::Ghost,
            button,
        );
        let label = match (editing.is_some(), busy) {
            (true, _) => tr("Save changes"),
            (false, true) if duplicate => tr("Duplicating…"),
            (false, false) if duplicate => tr("Duplicate"),
            (false, _) => tr("Create instance"),
        };
        let save = ui::primary("instance-save", label, button);
        let buttons = if self.is_asking(&menus::Asking::DiscardDraft) {
            div().child(ui::confirm(
                "instance-cancel",
                ui::Confirm::DiscardChanges,
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = None;
                    cx.notify();
                }),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = None;
                    this.close_instance_page();
                    cx.notify();
                }),
            ))
        } else if busy {
            div()
                .flex()
                .items_center()
                .gap(px(space::SM))
                .child(ui::disabled(cancel))
                .child(ui::disabled(save))
        } else {
            div()
                .flex()
                .items_center()
                .gap(px(space::SM))
                .child(cancel.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.close_instance_page();
                    cx.notify();
                })))
                .child(save.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.save_instance(cx);
                    cx.notify();
                })))
        };
        ui::card()
            .id("instance-foot")
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(layout::CONTENT_INSET))
            .flex()
            .items_center()
            .gap(px(space::LG))
            .p(px(ui::PANEL_INSET))
            .children(icon.map(|icon| {
                ui::picture_button("instance-preview-icon", icon, tr("Choose image…")).on_press(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.choose_instance_picture(cx);
                    }),
                )
            }))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(space::XS))
                    .child(ui::entity_title(name, title).truncate())
                    // Why Create failed takes the summary's place, by the buttons.
                    .children(match error {
                        Some(e) => Some(ui::error_note(e)),
                        None => (!summary.is_empty()).then(|| ui::card_note(summary).truncate()),
                    }),
            )
            .child(buttons.relative().flex_none())
    }

    /// Eclipse's versions, the newest first: picking the one the launcher plays lets the
    /// instance follow it; any other is kept, and installed on Create when it isn't yet.
    fn eclipse_versions(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let soft = ui::look().accent_soft;
        let follows = self.followed_version();
        let chosen = self.draft_eclipse();
        let tags = self.eclipse_choices();
        let rows: Vec<_> = tags
            .iter()
            .enumerate()
            .map(|(i, tag)| {
                let newest = i == 0;
                let on = chosen.as_ref() == Some(tag);
                let date = eclipse_launcher::nightly::date_of(tag).map(|d| dates::published(d, ""));
                let label = eclipse_launcher::nightly::display_name(tag);
                let row = ui::list_choice(format!("eclipse-version-{tag}"), on)
                    .py(px(space::MD))
                    .child(
                        ui::item_text(label.clone(), date.unwrap_or_default())
                            .flex_none()
                            .w(px(ECLIPSE_COLUMN)),
                    )
                    .children(newest.then(|| {
                        ui::text(
                            eclipse_theme::glass::step::NOTE,
                            FontWeight::NORMAL,
                            ink::FULL,
                        )
                        .child(ui::state_line(ui::Tone::Accent(soft), tr("Newest")))
                    }))
                    .child(ui::tick_slot(on));
                (row, SharedString::from(label), on)
            })
            .collect();
        let list = match (&self.releases, rows.is_empty()) {
            (_, false) => {
                let this = cx.entity().downgrade();
                let list = ui::options(
                    ui::list_box("eclipse-versions", ECLIPSE_LIST),
                    rows,
                    true,
                    move |i, _, cx| {
                        let pick = tags[i].clone();
                        let _ = this.update(cx, |this, cx| {
                            let latest = follows.as_ref() == Some(&pick);
                            if let Some(d) = &mut this.instances.draft {
                                d.pin = (!latest).then(|| pick.clone());
                            }
                            cx.notify();
                        });
                    },
                );
                ui::with_scrollbar(list).into_any_element()
            }
            (Load::Loading, true) => {
                ui::loading_state("eclipse-versions-loading", tr("Loading versions…"))
                    .into_any_element()
            }
            (Load::Failed(e), true) => ui::error_state_with(
                "eclipse-versions-failed",
                tr("Couldn't load versions"),
                e,
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.fetch_releases();
                    cx.notify();
                }),
                vec![Self::proxy_settings_button("eclipse-versions-failed", cx)],
                ButtonKind::Secondary,
            )
            .into_any_element(),
            (_, true) => ui::empty_state(
                "eclipse-versions-empty",
                "package-01",
                tr("No version installed"),
                tr("Install one on the Versions page first."),
                vec![],
            )
            .into_any_element(),
        };
        ui::section_card(
            "instance-eclipse-version",
            tr("Eclipse version"),
            None,
            list,
        )
    }

    /// The icons: the instance's own mark, the loaders' logos and the prepared files' blocks and
    /// items, then the colours and Choose image….
    fn icon_choice(
        &mut self,
        editing: Option<&Instance>,
        kind: NewKind,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let (icon, logo, d_picture) = {
            let Some(d) = self.instances.draft.as_ref() else {
                return div().id("instance-icon");
            };
            (d.icon.clone(), d.logo, d.picture.clone())
        };
        let has_saved_picture = editing.is_some_and(|i| i.home.join(instances::PICTURE).is_file());
        let file_picture = matches!(&d_picture, Some(Some(_))) && logo.is_none();
        let own_picture = d_picture.is_none() && has_saved_picture;
        let pictured = matches!(&d_picture, Some(Some(_))) || own_picture;
        let typed = self.fields.value(Field::InstanceName).trim().to_string();
        let name = if typed.is_empty() { "N".into() } else { typed };
        let mut first: Vec<(Stateful<Div>, SharedString, bool, IconPick)> = Vec::new();
        if file_picture || has_saved_picture {
            let picture_img = match &d_picture {
                Some(Some(chosen)) => {
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    chosen.png.hash(&mut h);
                    self.icon_image(
                        format!("chosen:{:x}", h.finish()),
                        CHOICE_ICON,
                        false,
                        || Some(chosen.png.clone()),
                    )
                }
                _ => editing.and_then(|i| self.instance_picture(i, CHOICE_ICON)),
            };
            let picture_active = (file_picture || own_picture) && icon.is_none();
            if let Some(image) = picture_img {
                first.push((
                    icon_choice("icon-picture", picture_active).child(instance_icons::centred(
                        image,
                        CHOICE_ICON,
                        self.scale,
                    )),
                    tr("Your picture").into(),
                    picture_active,
                    IconPick::Picture {
                        active: picture_active,
                        saved: has_saved_picture,
                    },
                ));
            }
        }
        let loader = self.draft_loader();
        let auto = self.auto_mark(kind, loader, CHOICE_ICON);
        let auto_on = !pictured && icon.is_none();
        first.push((
            icon_choice("icon-auto", auto_on).child(auto),
            tr("Automatic").into(),
            auto_on,
            IconPick::Icon(None),
        ));
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
                "grass" => {
                    self.icon_image("mark:grass".into(), CHOICE_ICON, false, || {
                        Some(png.clone())
                    })
                    .map(|i| instance_icons::centred(i, CHOICE_ICON, self.scale).into_any_element())
                }
                _ => self
                    .loader_logo(id, ui::TILE_MD)
                    .map(|i| ui::image_tile(i, ui::TILE_MD)),
            };
            let Some(shown) = shown else {
                continue;
            };
            let name: SharedString = match id {
                "grass" => tr("Grass block").into(),
                _ => Loader::parse(id).map_or(id.into(), |l| l.name().into()),
            };
            first.push((
                icon_choice(format!("icon-logo-{id}"), logo == Some(id)).child(shown),
                name,
                logo == Some(id),
                IconPick::Logo(id, png),
            ));
        }
        for texture in instance_state::TEXTURES {
            let Some(image) = self.texture_image(texture, CHOICE_ICON) else {
                continue;
            };
            let on = !pictured && icon == Some(Icon::Texture(texture.into()));
            first.push((
                icon_choice(format!("icon-{texture}"), on)
                    .child(ui::image_tile(image, ui::TILE_MD)),
                texture_name(texture).into(),
                on,
                IconPick::Icon(Some(Icon::Texture(texture.into()))),
            ));
        }
        let colours: Vec<_> = COLOURS
            .iter()
            .map(|rgb| {
                let hex = format!("#{rgb:06x}");
                let on = !pictured && icon == Some(Icon::Colour(hex.clone()));
                (
                    icon_choice(format!("icon-{hex}"), on).child(ui::letter_tile(
                        ui::color(*rgb),
                        &name,
                        CHOICE_ICON,
                    )),
                    SharedString::from(trf("Colour {hex}", &[("hex", &hex)])),
                    on,
                    IconPick::Icon(Some(Icon::Colour(hex))),
                )
            })
            .collect();
        let picker = |id: &'static str,
                      choices: Vec<(Stateful<Div>, SharedString, bool, IconPick)>,
                      cx: &mut Context<Self>| {
            let (rows, picks): (Vec<_>, Vec<_>) = choices
                .into_iter()
                .map(|(row, label, on, pick)| ((row, label, on), pick))
                .unzip();
            let pick = cx.listener(move |this, i: &usize, _, cx| {
                if let Some(pick) = picks.get(*i) {
                    this.pick_icon(pick, cx);
                }
            });
            ui::options(
                div()
                    .id(id)
                    .aria_label(tr("Icon"))
                    .flex()
                    .flex_wrap()
                    .gap(px(space::SM)),
                rows,
                false,
                move |i, window, cx| pick(&i, window, cx),
            )
        };
        let first = picker("instance-icons", first, cx);
        let colours = picker("instance-colours", colours, cx);
        let choose = ui::button(
            "instance-picture",
            Some("image-01"),
            tr("Choose image…"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .ml(px(space::SM))
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.choose_instance_picture(cx);
        }));
        let body = div().flex().flex_col().child(first).child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(space::SM))
                .mt(px(space::SM))
                .child(colours)
                .child(choose),
        );
        ui::section_card("instance-icon", tr("Icon"), None, body)
    }
}

enum IconPick {
    Picture { active: bool, saved: bool },
    Icon(Option<Icon>),
    Logo(&'static str, Vec<u8>),
}

fn texture_name(texture: &str) -> String {
    let words = texture.replace('_', " ");
    let mut chars = words.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

impl Launcher {
    fn pick_icon(&mut self, pick: &IconPick, cx: &mut Context<Self>) {
        match pick {
            IconPick::Picture { active: true, .. } => self.choose_instance_picture(cx),
            IconPick::Picture { saved, .. } => {
                if let Some(d) = &mut self.instances.draft {
                    d.icon = None;
                    d.logo = None;
                    if *saved {
                        d.picture = None;
                    }
                    d.error = None;
                }
            }
            IconPick::Icon(icon) => {
                if let Some(d) = &mut self.instances.draft {
                    d.icon = icon.clone();
                    d.picture = Some(None);
                    d.logo = None;
                }
            }
            IconPick::Logo(id, png) => {
                let made = instance_icons::chosen(png.clone());
                if let (Some(d), Some(made)) = (&mut self.instances.draft, made) {
                    d.picture = Some(Some(made));
                    d.icon = None;
                    d.logo = Some(id);
                    d.error = None;
                }
            }
        }
        cx.notify();
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
    fn new_instance_offers_every_published_nightly_then_what_only_this_computer_has() {
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
