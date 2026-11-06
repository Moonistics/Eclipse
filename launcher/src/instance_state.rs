//! Instances in the window (`eclipse_launcher::instances`): the sidebar's list, by group, and the
//! page that makes one (empty, or a copy of the selected one) or changes its name, group and
//! icon.
use super::*;
use eclipse_launcher::instances::{self, Icon, Instance, Kind as InstanceKind};
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
    pub(super) scroll: gpui::ScrollHandle,
    /// A new instance copies the selected one's worlds, options and packs.
    pub(super) copy: bool,
    pub(super) icon: Option<Icon>,
    /// A picture chosen since it opened (None in it: the instance's is removed on Save).
    pub(super) picture: Option<Option<instance_icons::Chosen>>,
    pub(super) error: Option<String>,
    pub(super) busy: bool,
    /// Eclipse, or the Java game it plays.
    pub(super) java: new_instance::JavaChoice,
    /// The Eclipse version it keeps (`Instance::version`); None follows the launcher's.
    pub(super) pin: Option<String>,
    /// The loader logo `picture` was made from, when it was one.
    pub(super) logo: Option<&'static str>,
    pub(super) reveal_version: bool,
}

/// The instances and their icons as images, kept between frames.
#[derive(Default)]
pub(super) struct Instances {
    pub(super) list: Vec<Instance>,
    pub(super) draft: Option<InstanceDraft>,
    /// Textures, pictures and marks as images, by name and size in device pixels.
    pub(super) icons: HashMap<(String, u32), Option<Arc<RenderImage>>>,
    /// Mojang's versions, for a new Java instance.
    pub(super) versions: new_instance::VersionList,
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

    /// The installed version `instance` plays: its own, else the launcher's.
    pub(super) fn version_of(&self, instance: &Instance) -> Option<&String> {
        match &instance.version {
            Some(own) => self.installed.iter().find(|t| *t == own),
            None => self.launcher_version(),
        }
    }

    /// Keeps the selected instance on `tag`, or lets it follow the launcher's version.
    pub(super) fn pin_version(&mut self, tag: Option<String>) {
        let Some(id) = self.selected_instance().map(|i| i.id.clone()) else {
            return;
        };
        match instances::edit(&id, |m| m.version = tag) {
            Ok(_) => self.reload_instances(),
            Err(e) => self.client_error = Some(e),
        }
    }

    pub(super) fn select_instance(&mut self, instance: &Instance) {
        // A native build's warning names the mods of the instance it was opened for.
        self.native_confirm = None;
        self.native_update = None;
        self.settings.instance =
            (instance.kind != InstanceKind::Default).then(|| instance.id.clone());
        self.save_settings();
    }

    /// A texture from the selected version's prepared files, its first frame, drawn as pixels.
    pub(super) fn texture_image(&mut self, name: &str, size: f32) -> Option<Arc<RenderImage>> {
        let tag = self.selected().filter(|t| backend::is_prepared(t))?.clone();
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
        self.fields.focus_soon = Some(Field::InstanceName);
        // What it plays now: Edit can switch it (`instances::switch`).
        let java = match editing.as_ref().and_then(|i| i.java.as_ref()) {
            None => Default::default(),
            Some(j) => new_instance::JavaChoice {
                kind: match &j.loader {
                    Some(_) => new_instance::NewKind::Modded,
                    None => new_instance::NewKind::Vanilla,
                },
                loader: j
                    .loader
                    .as_ref()
                    .map_or(eclipse_launcher::loaders::Kind::Fabric, |l| l.kind),
                version: Some(j.minecraft.clone()),
                ..Default::default()
            },
        };
        self.instances.draft = Some(InstanceDraft {
            icon: editing.as_ref().and_then(|i| i.icon.clone()),
            pin: editing.as_ref().and_then(|i| i.version.clone()),
            logo: None,
            picture: None,
            editing: editing.map(|i| i.id),
            back,
            scroll: gpui::ScrollHandle::new(),
            copy: false,
            error: None,
            busy: false,
            reveal_version: java.version.is_some(),
            java,
        });
        self.fields.text(Field::VersionSearch).set(String::new());
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
        let Some(name) = self.selected_instance().map(|i| i.name.clone()) else {
            return;
        };
        self.open_instance_page(None);
        self.fields
            .text(Field::InstanceName)
            .set(format!("{name} copy"));
        if let Some(d) = &mut self.instances.draft {
            d.copy = true;
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
        if open {
            self.close_instance_page();
        }
        open
    }

    /// Creates or changes the instance as the page says. A copy can be gigabytes of worlds, so
    /// the work runs in the background.
    pub(super) fn save_instance(&mut self, cx: &mut Context<Self>) {
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
        let copy = selected.filter(|_| d.copy);
        // The Eclipse version is Eclipse's instances' alone: a Java one plays its own game.
        let plays_eclipse = d.java.kind == new_instance::NewKind::Eclipse;
        // A new Java instance: what it plays, and its name when none is typed.
        let java = (editing.is_none() && d.java.kind != new_instance::NewKind::Eclipse)
            .then(|| d.java.clone());
        let minecraft = java.as_ref().map(|j| {
            j.minecraft(match &self.instances.versions {
                new_instance::VersionList::Ready(v) => Some(v),
                _ => None,
            })
        });
        let Some(d) = self.instances.draft.as_mut() else {
            return;
        };
        let download = download.filter(|_| plays_eclipse);
        let (java, name) = match (java, minecraft) {
            (Some(_), Some(None)) => {
                d.error = Some(tr("Mojang's version list isn't here yet.").into());
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
                        instances::switch(&id, new_instance::with_loader(to)?)?;
                    }
                    id
                }
                (None, Some((j, mc))) => new_instance::create_java(&j, &name, &mc)?.id,
                (None, None) => instances::create(&name, copy.as_deref())?.id,
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
                                InstanceKind::Default | InstanceKind::Own => Page::Home,
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
                            crate::toast::say(e);
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
    pub(super) fn delete_instance(&mut self, id: &str) {
        if self.client.is_some() || self.starting_client.is_some() {
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
            Err(e) => crate::toast::say(e),
        }
    }
}

/// The first letter or digit of a name, for an icon without a picture.
pub(super) fn initial(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .unwrap_or('?')
        .to_uppercase()
        .to_string()
}
