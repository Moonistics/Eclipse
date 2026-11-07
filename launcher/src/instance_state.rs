//! Instances in the window (`eclipse_launcher::instances`): the sidebar's list, by group, and the
//! page that makes one (empty, or a copy of the selected one) or changes its name, group and
//! icon.
//!
//! New instance's kinds: Eclipse, Vanilla (Minecraft: Java Edition as Mojang ships it) and Modded
//! (with Fabric, Quilt, Forge or NeoForge). A Java kind picks its version from Mojang's public
//! list (`minecraft_versions`), fetched once when the page first needs it; the game itself comes
//! with the first Play, for an account that owns it.
use super::*;
use eclipse_launcher::instances::{self, Icon, Instance, Kind as InstanceKind};
use eclipse_launcher::loaders::{self, Kind as Loader};
use eclipse_launcher::minecraft_versions::Versions;
use fields::Field;
use gpui::RenderImage;
use std::collections::HashMap;
use std::sync::Arc;
/// Textures offered as icons, when the player's prepared files hold them: block faces and items
/// that read well at sidebar size.
pub(super) const TEXTURES: [&str; 24] = [
    "grass_block_side",
    "oak_planks",
    "oak_log",
    "cobblestone",
    "bricks",
    "bookshelf",
    "crafting_table_front",
    "tnt_side",
    "sand",
    "snow",
    "ice",
    "obsidian",
    "netherrack",
    "end_stone",
    "glowstone",
    "diamond_ore",
    "diamond",
    "emerald",
    "apple",
    "ender_pearl",
    "iron_pickaxe",
    "diamond_sword",
    "bow",
    "book",
];

/// The New instance page: a new instance (`editing` None) or one being changed. Its name and
/// group are the text fields `Field::InstanceName` and `Field::InstanceGroup`.
pub(super) struct InstanceDraft {
    pub(super) editing: Option<String>,
    pub(super) back: Page,
    pub(super) scroll: Scrollbar,
    pub(super) duplicate: bool,
    pub(super) icon: Option<Icon>,
    /// A picture chosen since it opened (None in it: the instance's is removed on Save).
    pub(super) picture: Option<Option<instance_icons::Chosen>>,
    pub(super) error: Option<String>,
    pub(super) busy: bool,
    /// Eclipse, or the Java game it plays.
    pub(super) java: instance_state::JavaChoice,
    /// The Eclipse version it keeps (`Instance::version`); None follows the launcher's.
    pub(super) pin: Option<String>,
    /// The loader logo `picture` was made from, when it was one.
    pub(super) logo: Option<&'static str>,
    pub(super) reveal_version: bool,
    pub(super) opened: Option<DraftShape>,
}

#[derive(PartialEq)]
pub(super) struct DraftShape {
    name: String,
    group: String,
    icon: Option<Icon>,
    picture: Option<Option<u64>>,
    pin: Option<String>,
    kind: NewKind,
    loader: Loader,
    version: Option<String>,
    loader_version: Option<String>,
}

impl InstanceDraft {
    fn shape(&self, name: &str, group: &str) -> DraftShape {
        let j = &self.java;
        DraftShape {
            name: name.to_string(),
            group: group.to_string(),
            icon: self.icon.clone(),
            picture: self
                .picture
                .as_ref()
                .map(|p| p.as_ref().map(|c| picture_key(&c.png))),
            pin: self.pin.clone(),
            kind: j.kind,
            loader: j.loader,
            version: j.version.clone(),
            loader_version: j.loader_version.clone(),
        }
    }
}

pub(super) fn picture_key(png: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    png.hash(&mut h);
    h.finish()
}

/// The instances and their icons as images, kept between frames.
#[derive(Default)]
pub(super) struct Instances {
    pub(super) list: Vec<Instance>,
    pub(super) draft: Option<InstanceDraft>,
    /// Textures, pictures and marks as images, by name and size in device pixels.
    pub(super) icons: HashMap<(String, u32), Option<Arc<RenderImage>>>,
    /// Mojang's versions, for a new Java instance.
    pub(super) versions: instance_state::VersionList,
}

impl Launcher {
    pub(super) fn reload_instances(&mut self) {
        self.instances.list = instances::list();
        // A picture may have come or gone with the change.
        self.instances
            .icons
            .retain(|(key, _), _| !key.starts_with("picture:"));
    }

    /// The instance Play starts.
    pub(super) fn selected_instance(&self) -> Option<&Instance> {
        let id = self
            .settings
            .instance
            .as_deref()
            .unwrap_or(instances::DEFAULT);
        let list = &self.instances.list;
        list.iter().find(|i| i.id == id).or(list.first())
    }

    pub(super) fn plays<'a>(
        &'a self,
        instance: &'a Instance,
    ) -> Option<instances::EclipseVersion<'a>> {
        instance.eclipse_version(
            self.settings.default_version.as_ref(),
            &self.versions.installed,
        )
    }

    pub(super) fn plays_pinned<'a>(
        &'a self,
        pin: Option<&'a String>,
    ) -> instances::EclipseVersion<'a> {
        instances::EclipseVersion::of(
            pin,
            self.settings.default_version.as_ref(),
            &self.versions.installed,
        )
    }

    /// The installed version `instance` plays: its own, else the launcher's.
    pub(super) fn version_of<'a>(&'a self, instance: &'a Instance) -> Option<&'a String> {
        self.plays(instance)?.installed_tag()
    }

    /// Keeps the selected instance on `tag`, or lets it follow the launcher's version.
    pub(super) fn pin_version(&mut self, tag: Option<String>) {
        let Some(id) = self.selected_instance().map(|i| i.id.clone()) else {
            return;
        };
        match instances::edit(&id, |m| m.version = tag) {
            Ok(_) => self.reload_instances(),
            Err(e) => self.game.problem = Some(game_run::GameProblem::Failed(e)),
        }
    }

    pub(super) fn select_instance(&mut self, instance: &Instance) {
        // A native build's warning names the mods of the instance it was opened for.
        self.native.confirm = None;
        self.native.update = None;
        self.settings.instance =
            (instance.kind != InstanceKind::Default).then(|| instance.id.clone());
        self.save_settings();
    }

    /// A texture from the selected version's prepared files, its first frame, drawn as pixels.
    pub(super) fn texture_image(&mut self, name: &str, size: f32) -> Option<Arc<RenderImage>> {
        let tag = self
            .selected_version()
            .filter(|t| backend::is_prepared(t))?
            .clone();
        let scale = self.scale;
        self.instances
            .icons
            .entry((name.to_string(), (size * scale) as u32))
            .or_insert_with(|| {
                let path = Icon::Texture(name.into()).texture(&backend::resources(&tag))?;
                let png = image::open(path).ok()?.to_rgba8();
                let w = png.width() as usize;
                // An animated texture is its frames one under another: the first is square.
                let texels: Vec<[u8; 4]> = png.pixels().take(w * w).map(|p| p.0).collect();
                Some(art::bake(&texels, w, w, size / w as f32, scale))
            })
            .clone()
    }

    /// Opens the page for a new instance, or for changing `editing`.
    pub(super) fn open_instance_page(&mut self, editing: Option<Instance>) {
        if self.instances.draft.as_ref().is_some_and(|d| d.busy) {
            self.go(Page::InstanceEditor);
            return;
        }
        let back = match (self.page, &self.instances.draft) {
            (Page::InstanceEditor, Some(d)) => d.back,
            (Page::InstanceEditor, None) => Page::Home,
            (page, _) => page,
        };
        let editing_one = editing.is_some();
        let (name, group) = editing
            .as_ref()
            .map(|i| (i.name.clone(), i.group.clone().unwrap_or_default()))
            .unwrap_or_default();
        self.fields.text(Field::InstanceName).set(name);
        self.fields.text(Field::InstanceGroup).set(group);
        // What it plays now: Edit can switch it (`instances::switch`).
        let java = editing
            .as_ref()
            .map_or_else(Default::default, java_choice_of);
        self.instances.draft = Some(InstanceDraft {
            icon: editing.as_ref().and_then(|i| i.icon.clone()),
            pin: editing.as_ref().and_then(|i| i.version.clone()),
            logo: None,
            picture: None,
            editing: editing.map(|i| i.id),
            back,
            scroll: Scrollbar::default(),
            duplicate: false,
            error: None,
            busy: false,
            reveal_version: java.version.is_some(),
            java,
            opened: None,
        });
        self.fields.text(Field::VersionSearch).set(String::new());
        self.stop_asking(|a| *a == menus::Asking::DiscardDraft);
        // A new one starts with the name it would get ("My Eclipse"), until the player types one.
        if !editing_one {
            self.change(|_| {});
        }
        if self.page == Page::InstanceEditor {
            self.switches += 1;
        }
        self.go(Page::InstanceEditor);
    }

    /// Opens the page for a copy of the selected instance, named after it.
    pub(super) fn start_copy(&mut self) {
        let Some(source) = self.selected_instance().cloned() else {
            return;
        };
        self.open_instance_page(None);
        self.fields
            .text(Field::InstanceName)
            .set(format!("{} copy", source.name));
        self.fields
            .text(Field::InstanceGroup)
            .set(source.group.clone().unwrap_or_default());
        if let Some(d) = &mut self.instances.draft {
            d.duplicate = true;
            d.java = java_choice_of(&source);
            d.reveal_version = d.java.version.is_some();
            d.pin = source.version.clone();
            d.icon = source.icon.clone();
        }
    }

    pub(super) fn close_instance_page(&mut self) {
        if let Some(d) = self.instances.draft.as_ref().filter(|d| !d.busy) {
            let back = d.back;
            self.go(back);
        }
    }

    /// Escape closes the page. Whether it did.
    pub(super) fn instance_escape(&mut self) -> bool {
        let open = self.page == Page::InstanceEditor
            && self.instances.draft.as_ref().is_some_and(|d| !d.busy);
        if open && self.draft_changed() {
            self.asking = Some(menus::Asking::DiscardDraft);
        } else if open {
            self.close_instance_page();
        }
        open
    }

    pub(super) fn draft_changed(&self) -> bool {
        self.launch_options_changed()
            || self.memory_changed()
            || self
                .instances
                .draft
                .as_ref()
                .is_some_and(|d| d.opened.is_some() && d.opened != self.draft_shape())
    }

    pub(super) fn remember_draft(&mut self) {
        if self
            .instances
            .draft
            .as_ref()
            .is_some_and(|d| d.opened.is_none())
        {
            let shape = self.draft_shape();
            if let Some(d) = &mut self.instances.draft {
                d.opened = shape;
            }
        }
    }

    fn draft_shape(&self) -> Option<DraftShape> {
        let d = self.instances.draft.as_ref()?;
        let typed = self.fields.value(Field::InstanceName).trim();
        let name = if self.default_name().as_deref() == Some(typed) {
            ""
        } else {
            typed
        };
        Some(d.shape(name, self.fields.value(Field::InstanceGroup)))
    }

    /// Creates or changes the instance as the page says. A copy can be gigabytes of worlds, so
    /// the work runs in the background.
    pub(super) fn save_instance(&mut self, cx: &mut Context<Self>) {
        if !self.save_launch_options() {
            return;
        }
        let selected = self.selected_instance().map(|i| i.id.clone());
        let name = self.fields.value(Field::InstanceName).trim().to_string();
        let group = Some(self.fields.value(Field::InstanceGroup).to_string());
        // A Eclipse version picked but not installed downloads once the instance is saved.
        let download = self
            .draft_eclipse()
            .and_then(|tag| self.release_to_install(&tag));
        // An edited instance switched to another game (`instances::switch`).
        let switch_to = self
            .instances
            .draft
            .as_ref()
            .and_then(|d| d.editing.as_ref())
            .and_then(|id| self.instances.list.iter().find(|i| &i.id == id))
            .filter(|i| i.kind == InstanceKind::Own)
            .and_then(|i| {
                self.draft_plays(i)
                    .filter(|to| *to != instances::Plays::of(i))
            });
        let copy_plays = self
            .instances
            .draft
            .as_ref()
            .filter(|d| d.editing.is_none() && d.duplicate)
            .and(self.selected_instance())
            .and_then(|i| {
                self.draft_plays(i)
                    .filter(|to| *to != instances::Plays::of(i))
            });
        let Some(d) = self.instances.draft.as_mut().filter(|d| !d.busy) else {
            return;
        };
        let icon = d.icon.clone();
        let pin = d.pin.clone();
        let picture = d
            .picture
            .as_ref()
            .map(|p| p.as_ref().map(|c| c.png.clone()));
        let editing = d.editing.clone();
        let edited = editing.is_some();
        let copy = selected.filter(|_| d.duplicate);
        // The Eclipse version is Eclipse's instances' alone: a Java one plays its own game.
        let plays_eclipse = d.java.kind == instance_state::NewKind::Eclipse;
        // A new Java instance: what it plays, and its name when none is typed.
        let java = (editing.is_none() && d.java.kind != instance_state::NewKind::Eclipse)
            .then(|| d.java.clone());
        let minecraft = java.as_ref().map(|j| {
            j.minecraft(match &self.instances.versions {
                instance_state::VersionList::Ready(v) => Some(v),
                _ => None,
            })
        });
        let Some(d) = self.instances.draft.as_mut() else {
            return;
        };
        let download = download.filter(|_| plays_eclipse);
        let (java, name) = match (java, minecraft) {
            (Some(_), Some(None)) => {
                d.error =
                    Some(tr("Minecraft versions are still loading. Try again in a moment.").into());
                return;
            }
            (Some(j), Some(Some(mc))) => {
                let name = if name.is_empty() {
                    j.default_name(&mc)
                } else {
                    name
                };
                (Some((j, mc)), name)
            }
            _ => (None, name),
        };
        d.busy = true;
        d.error = None;
        let task = cx.background_executor().spawn(async move {
            let id = match (editing, java) {
                (Some(id), _) => {
                    if let Some(to) = switch_to {
                        instances::switch(&id, instance_state::with_loader(to)?)?;
                    }
                    id
                }
                (None, Some((j, mc))) if copy.is_none() => {
                    instance_state::create_java(&j, &name, &mc)?.id
                }
                (None, _) => {
                    let to = copy_plays.map(instance_state::with_loader).transpose()?;
                    let made = instances::create(&name, copy.as_deref())?.id;
                    if let Some(to) = to {
                        instances::switch(&made, to)?;
                    }
                    made
                }
            };
            let instance = instances::edit(&id, |m| {
                m.name = Some(name);
                m.group = group;
                m.icon = icon;
                if plays_eclipse {
                    m.version = pin;
                }
            })?;
            if let Some(png) = picture {
                instances::set_picture(
                    &instance.home,
                    png.as_deref(),
                    &eclipse_launcher::desktop_seams::to_trash,
                )?;
            }
            Ok(instance)
        });
        cx.spawn(async move |this, cx| {
            let result: Result<Instance, String> = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(instance) => {
                        let name = [("name", instance.name.as_str())];
                        crate::toast::say(if edited {
                            trf("Saved {name}", &name)
                        } else {
                            trf("Created {name}", &name)
                        });
                        let back = this.instances.draft.take().map_or(Page::Home, |d| d.back);
                        this.select_instance(&instance);
                        this.reload_instances();
                        // Play shows the download's progress; Versions shows it too.
                        if let Some(release) = download {
                            this.install_for_instance(release);
                        }
                        if this.page == Page::InstanceEditor {
                            this.go(match instance.kind {
                                InstanceKind::Default | InstanceKind::Own if back != Page::Mods => {
                                    Page::Home
                                }
                                _ => back,
                            });
                        }
                    }
                    Err(e) => {
                        if let Some(d) = &mut this.instances.draft {
                            d.busy = false;
                            d.error = Some(e.clone());
                        }
                        if this.page != Page::InstanceEditor {
                            crate::toast::say_error(e);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Launcher {
    pub(super) fn instance_by_id(&self, id: &str) -> Option<Instance> {
        self.instances.list.iter().find(|i| i.id == id).cloned()
    }

    fn show_on_home(&mut self, instance: &Instance) {
        self.close_menu();
        self.select_instance(instance);
        self.go(Page::Home);
    }

    pub(super) fn show_duplicate(&mut self, instance: &Instance) {
        self.show_on_home(instance);
        self.start_copy();
    }

    pub(super) fn show_export(&mut self, instance: &Instance) {
        self.show_on_home(instance);
        if self.export.is_none() {
            self.toggle_export();
        }
    }

    pub(super) fn show_launch_options(&mut self, instance: &Instance) {
        self.close_menu();
        self.open_instance_page(Some(instance.clone()));
        self.open_at_launch_options(instance);
    }

    pub(super) fn can_delete_instance(&self, instance: &Instance) -> bool {
        instance.kind != InstanceKind::Default && !self.game.active()
    }

    pub(super) fn ask_delete_instance(&mut self, id: String, window: &Window, cx: &App) {
        let Some(instance) = self.instance_by_id(&id) else {
            return;
        };
        self.open_menu_from(menus::Menu::InstanceRow(id.clone()), window, cx);
        if self.can_delete_instance(&instance) {
            self.asking = Some(menus::Asking::InstanceDelete(id));
        }
    }

    pub(super) fn delete_instance(&mut self, id: &str) {
        if self.game.active() {
            return;
        }
        let name = self
            .instances
            .list
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.name.clone());
        match instances::delete_with_selection(id) {
            Ok((_, saved)) => {
                if let Some(name) = name {
                    crate::toast::say(trf("Deleted {name}", &[("name", &name)]));
                }
                // Play starts Eclipse again; the window's copy of the settings says so too.
                if self.settings.instance.as_deref() == Some(id) {
                    self.settings.instance = None;
                    self.save_settings();
                } else if let Err(error) = saved {
                    self.settings_save_error = Some(error);
                }
                self.reload_instances();
            }
            Err(e) => crate::toast::say_error(e),
        }
    }
}

fn java_choice_of(instance: &Instance) -> JavaChoice {
    match &instance.java {
        None => JavaChoice::default(),
        Some(j) => JavaChoice {
            kind: match &j.loader {
                Some(_) => NewKind::Modded,
                None => NewKind::Vanilla,
            },
            loader: j.loader.as_ref().map_or(Loader::Fabric, |l| l.kind),
            version: Some(j.minecraft.clone()),
            ..Default::default()
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NewKind {
    Eclipse,
    Vanilla,
    Modded,
}

/// What a new Java instance plays.
#[derive(Clone, Debug)]
pub(super) struct JavaChoice {
    pub(super) kind: NewKind,
    pub(super) loader: Loader,
    /// None: the newest release.
    pub(super) version: Option<String>,
    pub(super) snapshots: bool,
    pub(super) old: bool,
    /// The loader's version; None: its newest stable one.
    pub(super) loader_version: Option<String>,
}

impl Default for JavaChoice {
    fn default() -> Self {
        JavaChoice {
            kind: NewKind::Eclipse,
            loader: Loader::Fabric,
            version: None,
            snapshots: false,
            old: false,
            loader_version: None,
        }
    }
}

impl JavaChoice {
    /// The Minecraft version it makes: the chosen one, else the newest release.
    pub(super) fn minecraft(&self, versions: Option<&Versions>) -> Option<String> {
        self.version
            .clone()
            .or_else(|| versions.map(|v| v.latest_release.clone()))
    }

    /// The name it gets when none is typed: "Vanilla 1.21.4", "Fabric 1.21.4".
    pub(super) fn default_name(&self, minecraft: &str) -> String {
        match self.kind {
            NewKind::Modded => format!("{} {minecraft}", self.loader.name()),
            _ => format!("Vanilla {minecraft}"),
        }
    }
}

/// `base`, or "base 2", "base 3"… when an instance already has it (names ignore case, as
/// `instances::create` does).
fn free_name(base: String, taken: &[&str]) -> String {
    let free = |n: &String| !taken.iter().any(|t| t.eq_ignore_ascii_case(n));
    if free(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(free)
        .unwrap_or(base)
}

/// Mojang's list as the page has it.
#[derive(Default)]
pub(super) enum VersionList {
    #[default]
    NotAsked,
    Loading,
    Ready(Versions),
    Failed(String),
}

impl Launcher {
    /// The name a new instance gets for what it plays now: "My Eclipse", "Vanilla 1.21.4",
    /// "Fabric 1.21.4", numbered past the ones already taken.
    fn default_name(&self) -> Option<String> {
        let j = &self.instances.draft.as_ref()?.java;
        let versions = match &self.instances.versions {
            VersionList::Ready(v) => Some(v),
            _ => None,
        };
        let base = match j.kind {
            NewKind::Eclipse => tr("My Eclipse").to_string(),
            _ => j.default_name(&j.minecraft(versions)?),
        };
        let taken: Vec<&str> = self
            .instances
            .list
            .iter()
            .map(|i| i.name.as_str())
            .collect();
        Some(free_name(base, &taken))
    }

    /// Changes what the new instance plays. Its name follows while it is the one the page gave
    /// (or none), never one the player typed.
    pub(super) fn change(&mut self, f: impl FnOnce(&mut JavaChoice)) {
        let before = self.default_name();
        if let Some(d) = self.instances.draft.as_mut() {
            let was = (d.java.loader, d.java.version.clone());
            f(&mut d.java);
            if was.1 != d.java.version {
                d.reveal_version = true;
            }
            if was != (d.java.loader, d.java.version.clone()) {
                d.java.loader_version = None;
            }
        }
        // An instance being edited keeps its name.
        if self
            .instances
            .draft
            .as_ref()
            .is_some_and(|d| d.editing.is_some())
        {
            return;
        }
        let typed = self.fields.value(Field::InstanceName).trim().to_string();
        if typed.is_empty() || Some(&typed) == before.as_ref() {
            let after = self.default_name().unwrap_or_default();
            self.fields.text(Field::InstanceName).set(after);
        }
    }

    /// Picks the page's kind; a Java one fetches Mojang's list the first time.
    pub(super) fn pick_kind(&mut self, kind: NewKind, cx: &mut Context<Self>) {
        self.change(|j| j.kind = kind);
        if kind != NewKind::Eclipse {
            self.load_versions(cx);
        }
    }

    /// Asks Mojang for its versions, the first time the page needs them.
    pub(super) fn load_versions(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.instances.versions, VersionList::NotAsked) {
            return;
        }
        self.instances.versions = VersionList::Loading;
        let task = cx
            .background_executor()
            .spawn(async { eclipse_launcher::minecraft_versions::load() });
        cx.spawn(async move |this, cx| {
            let list = task.await;
            let _ = this.update(cx, |this, cx| {
                let before = this.fields.value(Field::InstanceName).trim().is_empty();
                this.instances.versions = match list {
                    Ok(v) => VersionList::Ready(v),
                    Err(e) => VersionList::Failed(e),
                };
                // The newest release is known now: so is the name.
                if before {
                    this.change(|_| {});
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// Makes a Java instance: `name`, playing `minecraft`, with the loader version chosen for Modded
/// (none: its newest stable one), asked of its service now, so a version it doesn't support
/// says so here.
pub(super) fn create_java(
    choice: &JavaChoice,
    name: &str,
    minecraft: &str,
) -> Result<instances::Instance, String> {
    let loader = (choice.kind == NewKind::Modded)
        .then(|| {
            loaders::choose(
                choice.loader.id(),
                minecraft,
                choice.loader_version.as_deref(),
            )
        })
        .transpose()?;
    let made = instances::create_java(name, minecraft)?;
    match loader {
        Some(loader) => instances::edit(&made.id, |m| {
            m.java.get_or_insert_with(Default::default).loader = Some(loader)
        }),
        None => Ok(made),
    }
}

/// `to`, with the newest stable version of a loader it names without one (asked of its service
/// now, so a Minecraft version the loader doesn't support says so here).
pub(super) fn with_loader(to: instances::Plays) -> Result<instances::Plays, String> {
    match to {
        instances::Plays::Java {
            minecraft,
            loader: Some(l),
        } if l.version.is_empty() => Ok(instances::Plays::Java {
            loader: Some(loaders::choose(l.kind.id(), &minecraft, None)?),
            minecraft,
        }),
        other => Ok(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_java_instance_is_named_after_what_it_plays() {
        let mut choice = JavaChoice {
            kind: NewKind::Vanilla,
            ..Default::default()
        };
        assert_eq!(choice.default_name("1.21.4"), "Vanilla 1.21.4");
        choice.kind = NewKind::Modded;
        choice.loader = Loader::Quilt;
        assert_eq!(choice.default_name("1.21.4"), "Quilt 1.21.4");
        assert_eq!(choice.minecraft(None), None);
        choice.version = Some("1.20.1".into());
        assert_eq!(choice.minecraft(None).as_deref(), Some("1.20.1"));
    }

    #[test]
    fn a_default_name_skips_the_ones_taken() {
        assert_eq!(free_name("My Eclipse".into(), &["Eclipse"]), "My Eclipse");
        assert_eq!(
            free_name("My Eclipse".into(), &["my eclipse", "My Eclipse 2"]),
            "My Eclipse 3"
        );
    }
}
