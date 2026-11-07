//! The New instance page, which Edit instance shares.
use crate::fields::Field;
use crate::instance_icons::{IconKey, Mark};
use crate::instance_state::{NewKind, VersionList};
use crate::play_state::PlayState;
use crate::ui::{ButtonKind, Press};
use crate::{
    instance_icons, instance_state, instance_versions, layout, menus, scene, ui, Launcher, Load,
};
use eclipse_launcher::backend::Release;
use eclipse_launcher::i18n;
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::instances::{self, Icon, Instance};
use eclipse_launcher::loaders::Kind as Loader;
use eclipse_theme::glass::space;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, ClickEvent, Div, SharedString, Stateful};

/// The loaders Prism Launcher offers. Those `loaders::Kind` can install are shown; the others
/// join when the launcher learns them.
const LOADERS: [Loader; 4] = [
    Loader::Fabric,
    Loader::Quilt,
    Loader::NeoForge,
    Loader::Forge,
];
/// Logos offered as icons (not Minecraft's, which is Mojang's).
const LOGO_ICONS: [Loader; 4] = [
    Loader::NeoForge,
    Loader::Fabric,
    Loader::Quilt,
    Loader::Forge,
];
const COLOURS: [u32; 7] = [
    0x3b82f6, 0x14b8a6, 0x22c55e, 0xeab308, 0xf97316, 0xef4444, 0x8b8a99,
];
const PREVIEW_ICON: f32 = ui::TILE_LG;
const PREVIEW_ICON_SHORT: f32 = ui::TILE_MD;
const CHOICE_ICON: f32 = ui::TILE_MD;
const CHOICE_NOTE_LINES: usize = 2;
pub(super) const KINDS: [NewKind; 3] = [NewKind::Eclipse, NewKind::Vanilla, NewKind::Modded];
const CHOICE_MIN_WIDTH: f32 = 2. * space::MD + ui::TILE_LG + space::LG + layout::FIELD_SM;

/// The loaders a new modded instance can use now.
pub(super) fn loaders() -> Vec<Loader> {
    LOADERS.to_vec()
}

fn foot(switching: Option<Div>, row: Div) -> Stateful<Div> {
    ui::foot_card()
        .id("instance-foot")
        .flex_none()
        .mb(px(layout::CONTENT_INSET))
        .flex()
        .flex_col()
        .gap(px(space::MD))
        .p(px(ui::PANEL_INSET))
        .children(switching)
        .child(row)
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
        let opening = self
            .instances
            .draft
            .as_ref()
            .is_some_and(|d| d.opened.is_none());
        self.remember_draft();
        let Some(d) = self.instances.draft.as_ref() else {
            return div();
        };
        let (busy, duplicate, scroll) = (d.busy, d.duplicate, d.scroll.clone());
        let editing = d
            .editing
            .as_ref()
            .and_then(|id| self.instances.list.iter().find(|i| &i.id == id))
            .cloned();
        let kind = d.java.kind;
        if let Some(i) = &editing {
            self.sync_launch_options(i, opening);
        }
        let laid_out = scroll.handle.bounds().size.height > px(0.);
        let settled = laid_out
            && match kind {
                NewKind::Eclipse => !matches!(self.versions.releases, Load::Loading),
                _ => matches!(
                    self.instances.versions,
                    VersionList::Ready(_) | VersionList::Failed(_)
                ),
            };
        let at_launch = editing.is_some() && self.reveal_launch_options(settled);
        if at_launch {
            if let Some(d) = &mut self.instances.draft {
                d.reveal_version = false;
            }
        }
        let running = editing
            .as_ref()
            .is_some_and(|i| self.play_state(i) == PlayState::Running);
        let (title, note): (SharedString, _) = match &editing {
            Some(i) => (
                trf("Edit {name}", &[("name", &i.name)]).into(),
                running.then(|| tr("Changes apply the next time it starts.").into()),
            ),
            None if duplicate => {
                let from = self.selected_instance().map_or("", |i| i.name.as_str());
                (trf("Duplicate {name}", &[("name", from)]).into(), None)
            }
            None => (tr("New instance").into(), None),
        };
        let import = (editing.is_none() && !duplicate && !busy).then(|| self.import_button(cx));
        let switching = match &editing {
            Some(i) => self.switch_notice(i),
            None if duplicate => self
                .selected_instance()
                .cloned()
                .and_then(|i| self.switch_notice(&i)),
            None => None,
        };
        let mut cards: Vec<AnyElement> = Vec::new();
        cards.push(self.name_and_group(editing.as_ref(), cx).into_any_element());
        cards.push(
            self.kind_choice(editing.as_ref(), kind, cx)
                .into_any_element(),
        );
        if kind == NewKind::Modded {
            cards.push(self.loader_choice(cx).into_any_element());
        }
        if kind == NewKind::Eclipse {
            let reveal = opening && !at_launch;
            cards.push(self.eclipse_versions(reveal, cx).into_any_element());
        } else {
            self.load_versions(cx);
            cards.push(self.minecraft_versions(cx).into_any_element());
        }
        cards.push(
            self.icon_choice(editing.as_ref(), kind, cx)
                .into_any_element(),
        );
        // The memory it may use, kept with the instance.
        if editing.is_some() {
            cards.extend(self.memory_setting(cx).map(|row| {
                ui::card()
                    .id("instance-memory-card")
                    .p(px(ui::CARD_PADDING - ui::SETTING_ROW_PADDING))
                    .child(row)
                    .into_any_element()
            }));
            if at_launch && laid_out {
                scroll.handle.scroll_to_top_of_item(cards.len());
            } else if at_launch {
                cx.notify();
            }
            cards.push(self.launch_card(cx).into_any_element());
        }
        if let Some(last) = cards.pop() {
            cards.push(
                div()
                    .relative()
                    .flex_none()
                    .child(last)
                    .child(scene::track_content(&self.scene))
                    .into_any_element(),
            );
        }
        let foot = self.instance_foot(editing.as_ref(), kind, switching, cx);
        let scrolled = ui::scrolled(&scroll.handle);
        let pane = ui::scroll_pane("instance-pane", &scroll.handle)
            .on_scroll(cx.listener(move |_, _: &gpui::ScrollEvent, window, cx| {
                cx.on_next_frame(window, move |this, _, cx| {
                    let now = this
                        .instances
                        .draft
                        .as_ref()
                        .is_some_and(|d| ui::scrolled(&d.scroll.handle));
                    if now != scrolled {
                        cx.notify();
                    }
                });
            }))
            .flex()
            .flex_col()
            .size_full()
            .gap(px(layout::CARD_GAP))
            .pb(px(layout::CARD_GAP))
            .children(cards);
        let pane = div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(ui::page_pane(
                pane,
                &scroll,
                layout::page_margin(self.window_width),
            ))
            .children(ui::more_above_fade(
                &scroll.handle,
                scene::sky_under_title(&self.theme.night),
            ));
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .child(ui::page_head_with(title, note, import))
            .child(pane)
            .child(foot)
    }

    pub(super) fn instance_page_compact(&self) -> bool {
        layout::content_width(self.window_width) < layout::INSTANCE_COMPACT_BELOW
    }

    fn choices_stacked(&self) -> bool {
        let inside = layout::content_width(self.window_width) - 2. * ui::CARD_PADDING;
        let columns = KINDS.len() as f32;
        inside < columns * CHOICE_MIN_WIDTH + (columns - 1.) * space::MD
    }

    pub(super) fn choice_icon(&mut self, mark: instance_icons::Mark, name: &str) -> AnyElement {
        let spec = instance_icons::IconSpec {
            picture: instance_icons::Picture::None,
            icon: None,
            mark,
            name: name.to_string(),
        };
        let size = if self.choices_stacked() {
            ui::TILE_MD
        } else {
            ui::TILE_LG
        };
        self.icon_for(spec, size)
    }

    pub(super) fn choices(&self, id: &'static str, columns: u16) -> Stateful<Div> {
        let list = div().id(id);
        if self.choices_stacked() {
            list.flex().flex_col().gap(px(space::XS))
        } else {
            list.grid().grid_cols(columns).gap(px(space::MD))
        }
    }

    pub(super) fn choice_row(
        &self,
        id: String,
        on: bool,
        icon: AnyElement,
        title: SharedString,
        note: SharedString,
    ) -> Stateful<Div> {
        if self.choices_stacked() {
            return ui::choice_card_row(id, on)
                .min_w_0()
                .py(px(space::SM))
                .child(icon)
                .child(ui::item_text(title, note))
                .child(ui::tick_slot(on));
        }
        ui::choice_card(id, on)
            .items_start()
            .gap(px(space::LG))
            .p(px(space::MD))
            .min_w_0()
            .child(icon)
            .child(ui::item_text_wrapped(title, note, CHOICE_NOTE_LINES))
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
        self.draft_version_for(pin.as_ref())
    }

    /// The version an instance that keeps none plays: the launcher's, or with none installed
    /// yet, the newest Nightly, which Create installs.
    fn unpinned_version(&self) -> Option<String> {
        self.draft_version_for(None)
    }

    fn draft_version_for(&self, pin: Option<&String>) -> Option<String> {
        let plays = self.plays_pinned(pin);
        plays
            .tag
            .cloned()
            .or_else(|| self.eclipse_choices().into_iter().next())
    }

    fn eclipse_choices(&self) -> Vec<String> {
        let releases = match &self.versions.releases {
            Load::Ready(r) => r.as_slice(),
            _ => &[],
        };
        eclipse_choices(releases, &self.versions.installed)
    }

    pub(super) fn release_to_install(&self, tag: &str) -> Option<Release> {
        let Load::Ready(releases) = &self.versions.releases else {
            return None;
        };
        (!self.versions.installed.iter().any(|t| t == tag))
            .then(|| releases.iter().find(|r| r.tag == tag && r.asset.is_some()))
            .flatten()
            .cloned()
    }

    /// The mark an instance of `kind` shows when nothing is chosen for it.
    fn auto_mark(kind: NewKind, loader: Loader) -> instance_icons::Mark {
        match kind {
            NewKind::Eclipse => instance_icons::Mark::Eclipse,
            NewKind::Vanilla => instance_icons::Mark::Grass,
            NewKind::Modded => instance_icons::Mark::Loader(loader),
        }
    }

    /// The icon the instance will show, `size` px: the same tile the sidebar draws.
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
        let picture = match &d.picture {
            Some(Some(chosen)) => instance_icons::Picture::Picked(chosen.png.clone()),
            Some(None) => instance_icons::Picture::None,
            None => editing.map_or(instance_icons::Picture::None, |i| {
                instance_icons::Picture::Saved(Box::new(i.clone()))
            }),
        };
        let spec = instance_icons::IconSpec {
            picture,
            icon: d.icon.clone(),
            mark: Self::auto_mark(kind, self.draft_loader()),
            name: name.to_string(),
        };
        self.icon_for(spec, size)
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
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let loader = self.draft_loader();
        // One made in the launcher can switch what it plays; the game's home and modpacks
        // play what they were made with.
        let switches = editing.is_none_or(|i| i.kind == instances::Kind::Own);
        let kept = match editing.map(|i| &i.kind) {
            Some(instances::Kind::Default) => tr("This instance always plays Eclipse"),
            _ => tr("A modpack plays the game it was made for"),
        };
        let mut cards = Vec::new();
        for k in KINDS {
            let (id, title, note) = match k {
                NewKind::Eclipse => ("eclipse", tr("Eclipse"), tr("Faster Minecraft")),
                NewKind::Vanilla => ("vanilla", tr("Vanilla"), tr("The original Minecraft")),
                NewKind::Modded => ("modded", tr("Modded"), tr("Fabric, Forge and more")),
            };
            let icon = match k {
                NewKind::Modded if self.choices_stacked() => ui::tile(ui::TILE_MD)
                    .child(instance_icons::modded_mark())
                    .into_any_element(),
                NewKind::Modded => ui::mark_tile(instance_icons::modded_mark()).into_any_element(),
                _ => self.choice_icon(Self::auto_mark(k, loader), title),
            };
            let card = self.choice_row(
                format!("new-kind-{id}"),
                k == kind,
                icon,
                title.into(),
                note.into(),
            );
            cards.push((
                if switches {
                    card
                } else if k == kind {
                    card.cursor_default()
                } else {
                    ui::unavailable(card, kept)
                },
                title.into(),
                k == kind,
            ));
        }
        let grid = self.choices("instance-game-choices", KINDS.len() as u16);
        let grid = if switches {
            let this = cx.entity().downgrade();
            ui::options(grid, tr("Game"), cards, true, move |i, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.pick_kind(KINDS[i], cx);
                    cx.notify();
                });
            })
        } else {
            grid.children(cards.into_iter().map(|(card, _, _)| card))
        };
        ui::section_card("instance-game", tr("Game"), None, grid)
    }

    fn import_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let importing = self.modpacks.busy;
        let import = ui::button(
            "instance-import",
            Some(crate::icons::OPEN_FOLDER),
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
                layout::FIELD_LG,
                ui::ButtonSize::Lg,
                cx,
            )
            .w_full();
        let group = self
            .field(
                Field::InstanceGroup,
                tr("For example Survival"),
                layout::FIELD_LG,
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
                div().flex().child(ui::open_folder_button(
                    "instance-open-folder",
                    tr("Open instance folder"),
                    home,
                    ui::ButtonSize::Md,
                ))
            }))
    }
}

/// Every Nightly published with a build for this platform, newest first, then the installed
/// versions that list lacks (older Nightlies, imported builds). Picking one not installed installs
/// it on Create.
fn eclipse_choices(releases: &[Release], installed: &[String]) -> Vec<String> {
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
    fn instance_foot(
        &mut self,
        editing: Option<&Instance>,
        kind: NewKind,
        switching: Option<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let Some(d) = self.instances.draft.as_ref() else {
            return div().id("instance-foot");
        };
        let (busy, duplicate, error) = (d.busy, d.duplicate, d.error.clone());
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
        let button = if compact {
            ui::ButtonSize::Md
        } else {
            ui::ButtonSize::Lg
        };
        let cancel = ui::button(
            "instance-cancel",
            None,
            tr("Cancel"),
            ButtonKind::Ghost,
            button,
        );
        let label = match (editing.is_some(), busy) {
            (true, false) => tr("Save changes"),
            (true, true) => tr("Saving"),
            (false, true) if duplicate => tr("Duplicating"),
            (false, false) if duplicate => tr("Duplicate"),
            (false, true) => tr("Creating"),
            (false, false) => tr("Create instance"),
        };
        let creates = editing.is_none() && !duplicate;
        let unchanged = editing.is_some() && !self.draft_changed();
        let save_icon = creates.then_some(crate::icons::NEW_INSTANCE);
        let save = ui::primary("instance-save", save_icon, label, button);
        let named = div()
            .relative()
            .flex()
            .flex_col()
            .flex_auto()
            .min_w_0()
            .gap(px(space::XS))
            .child(ui::card_title(name))
            // Why Create failed takes the summary's place, by the buttons.
            .children(match error {
                Some(e) => Some(ui::error_note(e)),
                None => (!summary.is_empty())
                    .then(|| ui::card_note(summary).truncate().w_0().min_w_full()),
            });
        let lead = div()
            .flex()
            .flex_auto()
            .max_w_full()
            .items_center()
            .gap(px(space::LG));
        let row = div()
            .relative()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(space::LG))
            .min_h(px(button.height()));
        if self.is_asking(&menus::Asking::DiscardDraft) {
            let asking = ui::confirm(
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
            );
            let same_height = lead
                .invisible()
                .children(icon.map(|_| div().flex_none().size(px(size))))
                .child(named);
            let row = row.child(same_height).child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .flex()
                    .items_center()
                    .child(asking.flex_1()),
            );
            return foot(switching, row);
        }
        let buttons = if busy {
            let busy = ui::primary_busy("instance-save", label, None, button);
            ui::dialog_foot(ui::disabled(cancel), Some(busy.into_any_element()))
        } else {
            let cancel = cancel.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.close_instance_page();
                cx.notify();
            }));
            let save = if unchanged {
                ui::primary_unavailable(
                    "instance-save",
                    save_icon,
                    label,
                    button,
                    tr("Nothing changed yet"),
                )
            } else {
                save.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.save_instance(cx);
                    cx.notify();
                }))
            };
            ui::dialog_foot(cancel, Some(save.into_any_element()))
        };
        let lead = lead
            .children(icon.map(|icon| {
                ui::picture_button("instance-preview-icon", icon, tr("Choose image…")).on_press(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.choose_instance_picture(cx);
                    }),
                )
            }))
            .child(named);
        let row = row
            .child(lead)
            .child(buttons.relative().flex_none().ml_auto());
        foot(switching, row)
    }

    /// Eclipse's versions, the newest first: picking the one the launcher plays lets the
    /// instance follow it; any other is kept, and installed on Create when it isn't yet.
    fn eclipse_versions(&mut self, reveal: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let soft = ui::look().accent_soft;
        let follows = self.unpinned_version();
        let chosen = self.draft_eclipse();
        let tags = self.eclipse_choices();
        let rows: Vec<_> = tags
            .iter()
            .enumerate()
            .map(|(i, tag)| {
                let newest = i == 0;
                let on = chosen.as_ref() == Some(tag);
                let label = eclipse_launcher::nightly::display_name(tag);
                let note = newest.then(|| ui::state_line(ui::Tone::Accent(soft), tr("Newest")));
                let row = instance_versions::version_row(
                    format!("eclipse-version-{tag}"),
                    label.clone(),
                    div().children(note),
                    on,
                );
                (row, SharedString::from(label), on)
            })
            .collect();
        if reveal {
            if let Some(at) = tags.iter().position(|t| chosen.as_ref() == Some(t)) {
                instance_versions::reveal_version_row("eclipse-versions", at);
            }
        }
        let list = match (&self.versions.releases, rows.is_empty()) {
            (_, false) => {
                let this = cx.entity().downgrade();
                let list = ui::options(
                    ui::list_box("eclipse-versions", instance_versions::version_list_height()),
                    tr("Eclipse version"),
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
            (Load::Failed(e), true) => Self::releases_failed(
                "eclipse-versions-failed",
                tr("Couldn't load versions"),
                e,
                ButtonKind::Secondary,
                cx,
            )
            .into_any_element(),
            (_, true) => ui::empty_state(
                "eclipse-versions-empty",
                crate::icons::VERSION,
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
                Some(Some(chosen)) => self.icon_image(
                    IconKey::Chosen(instance_state::picture_key(&chosen.png)),
                    CHOICE_ICON,
                    false,
                    || Some(chosen.png.clone()),
                ),
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
        let auto_mark = Self::auto_mark(kind, loader);
        let auto = self
            .mark(&auto_mark, CHOICE_ICON)
            .unwrap_or_else(|| div().into_any_element());
        let auto_on = !pictured && icon.is_none();
        first.push((
            icon_choice("icon-auto", auto_on).child(auto),
            tr("Automatic").into(),
            auto_on,
            IconPick::Icon(None),
        ));
        // Our grass block, then the loaders' logos: each becomes the instance's picture.
        let logos = std::iter::once(Mark::Grass).chain(LOGO_ICONS.map(Mark::Loader));
        for mark in logos.filter(|m| *m != auto_mark || logo == Some(*m)) {
            let (id, name, png, shown) = match mark {
                Mark::Loader(loader) => (
                    loader.id(),
                    SharedString::from(loader.name()),
                    instance_icons::logo_png(loader),
                    self.loader_logo(loader, ui::TILE_MD)
                        .map(|i| ui::image_tile(i, ui::TILE_MD)),
                ),
                _ => (
                    "grass",
                    tr("Grass block").into(),
                    instance_icons::GRASS,
                    self.grass_mark(CHOICE_ICON).map(|i| {
                        instance_icons::centred(i, CHOICE_ICON, self.scale).into_any_element()
                    }),
                ),
            };
            let Some(shown) = shown else {
                continue;
            };
            first.push((
                icon_choice(format!("icon-logo-{id}"), logo == Some(mark)).child(shown),
                name,
                logo == Some(mark),
                IconPick::Logo(mark, png.to_vec()),
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
                      name: &'static str,
                      choices: Vec<(Stateful<Div>, SharedString, bool, IconPick)>,
                      cx: &mut Context<Self>| {
            let (rows, picks): (Vec<_>, Vec<_>) = choices
                .into_iter()
                .map(|(row, label, on, pick)| {
                    ((row.tooltip(ui::tip(label.clone())), label, on), pick)
                })
                .unzip();
            let pick = cx.listener(move |this, i: &usize, _, cx| {
                if let Some(pick) = picks.get(*i) {
                    this.pick_icon(pick, cx);
                }
            });
            ui::options(
                div().id(id).flex().flex_wrap().gap(px(space::SM)),
                name,
                rows,
                false,
                move |i, window, cx| pick(&i, window, cx),
            )
        };
        let first = picker("instance-icons", tr("Icon"), first, cx);
        let colours = picker("instance-colours", tr("Icon colour"), colours, cx);
        let choose = ui::button(
            "instance-picture",
            Some(crate::icons::OPEN_FOLDER),
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
    Logo(Mark, Vec<u8>),
}

fn texture_name(texture: &str) -> String {
    i18n::capitalised(&texture.replace('_', " "))
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
            IconPick::Logo(mark, png) => {
                let made = instance_icons::chosen(png.clone());
                if let (Some(d), Some(made)) = (&mut self.instances.draft, made) {
                    d.picture = Some(Some(made));
                    d.icon = None;
                    d.logo = Some(*mark);
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
