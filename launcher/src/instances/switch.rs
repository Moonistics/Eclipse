//! Switching what an instance plays (Edit's kinds, the mockup's `InstanceModal` for an existing
//! instance): Eclipse, or Minecraft: Java Edition of one version with or without a loader. Only
//! its entry in `instances.json` changes. Its folder, with its worlds, mods and options, is never
//! touched, and a Java instance's own settings (memory, Java, arguments, window) are put aside
//! while it plays Eclipse, to come back with Java.
use super::*;
use crate::game_folder;
use crate::loaders::Loader;

/// What an instance plays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plays {
    Eclipse,
    Java {
        minecraft: String,
        loader: Option<Loader>,
    },
}

impl Plays {
    /// What `instance` plays now.
    pub fn of(instance: &Instance) -> Plays {
        match &instance.java {
            None => Plays::Eclipse,
            Some(j) => Plays::Java {
                minecraft: j.minecraft.clone(),
                loader: j.loader.clone(),
            },
        }
    }

    /// The Minecraft version its worlds are saved by; `eclipse` is the one Eclipse plays.
    fn minecraft<'a>(&'a self, eclipse: &'a str) -> &'a str {
        match self {
            Plays::Eclipse => eclipse,
            Plays::Java { minecraft, .. } => minecraft,
        }
    }

    /// Whose mods it loads: Eclipse's, a loader's, or none (Vanilla).
    fn mods_for(&self) -> Option<(&'static str, &'static str)> {
        match self {
            Plays::Eclipse => Some(("eclipse", "Eclipse")),
            Plays::Java { loader, .. } => loader.as_ref().map(|l| (l.kind.id(), l.kind.name())),
        }
    }

    /// Whether it loads mods made for `id` (a Quilt instance takes Fabric's too).
    fn loads(&self, id: &str) -> bool {
        match self {
            Plays::Eclipse => id == "eclipse",
            Plays::Java { loader, .. } => loader
                .as_ref()
                .is_some_and(|l| l.kind.loads().iter().any(|m| m.id() == id)),
        }
    }
}

/// Something in an instance's folder that a switch keeps on disk but can't keep working, for the
/// player to read before it applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unkept {
    /// `count` mods made for `made_for` (Eclipse, Fabric…), which the new game doesn't load.
    Mods { count: usize, made_for: String },
    /// `count` mods made for Minecraft `from`: some may not load in `to`.
    ModsVersion {
        count: usize,
        from: String,
        to: String,
    },
    /// `count` worlds saved by Minecraft `from`: `to` upgrades them for good, or, older, can't
    /// open them.
    Worlds {
        count: usize,
        from: String,
        to: String,
    },
}

/// What switching `instance` to `to` can't keep. `eclipse`: the Minecraft version Eclipse plays.
pub fn unkept(instance: &Instance, to: &Plays, eclipse: &str) -> Vec<Unkept> {
    let from = Plays::of(instance);
    let mut out = Vec::new();
    let (was, now) = (from.minecraft(eclipse), to.minecraft(eclipse));
    if let Some((id, name)) = from.mods_for() {
        let count = mod_files(&instance.home, id == "eclipse");
        if count > 0 && !to.loads(id) {
            out.push(Unkept::Mods {
                count,
                made_for: name.into(),
            });
        } else if count > 0 && was != now {
            out.push(Unkept::ModsVersion {
                count,
                from: was.into(),
                to: now.into(),
            });
        }
    }
    let worlds = game_folder::worlds(&instance.home).len();
    if worlds > 0 && was != now {
        out.push(Unkept::Worlds {
            count: worlds,
            from: was.into(),
            to: now.into(),
        });
    }
    out
}

/// The mods in `home`: a Eclipse instance's content mods (`eclipse-mods/<id>.emod`, and the
/// folder mods ported from Java) and native mods, a Java one's jars, on or off.
fn mod_files(home: &Path, eclipse: bool) -> usize {
    let count = |dir: &str, counted: &dyn Fn(&str, bool) -> bool| {
        fs::read_dir(home.join(dir)).map_or(0, |d| {
            d.flatten()
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    !name.starts_with('.') && counted(&name, e.path().is_dir())
                })
                .count()
        })
    };
    if eclipse {
        count(game_folder::ECLIPSE_MODS, &|n, dir| {
            dir || n.ends_with(".emod")
        }) + count(crate::native_mods::INSTALLED, &|n, dir| {
            !dir && n.ends_with(".hnmod")
        })
    } else {
        count(game_folder::JAVA_MODS, &|n, dir| {
            !dir && (n.ends_with(".jar") || n.ends_with(".jar.disabled"))
        })
    }
}

/// Switches what the instance `query` plays. Only an instance made in the launcher can: the
/// game's home and modpacks play what they were made with.
pub fn switch(query: &str, to: Plays) -> Result<Instance, String> {
    switch_in(&backend::game_home(), &backend::data_dir(), query, to)
}

pub(super) fn switch_in(
    game_home: &Path,
    data: &Path,
    query: &str,
    to: Plays,
) -> Result<Instance, String> {
    let instance = find_in(&list_in(game_home, data), query)?;
    if instance.kind != Kind::Own {
        return Err(format!("{} plays what it was made with.", instance.name));
    }
    edit_in(game_home, data, query, |m| match to {
        Plays::Eclipse => {
            if let Some(java) = m.java.take() {
                m.java_kept = Some(java);
            }
        }
        Plays::Java { minecraft, loader } => {
            let mut java = m
                .java
                .take()
                .or_else(|| m.java_kept.take())
                .unwrap_or_default();
            java.minecraft = minecraft;
            java.loader = loader;
            m.java = Some(java);
        }
    })
}
