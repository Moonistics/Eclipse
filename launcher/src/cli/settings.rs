use super::json;
use eclipse_launcher::backend;
use eclipse_launcher::i18n::{tr, Language};
use eclipse_launcher::native_mods;
use eclipse_launcher::themes::{self, Accent};
use serde_json::{json, Value};
use std::path::PathBuf;

/// A launcher setting as `settings` shows it, and how a value sets it (None: its own command
/// sets it).
struct Key {
    name: &'static str,
    show: fn(&backend::Settings) -> Value,
    set: Option<Set>,
}

type Set = fn(&mut backend::Settings, &str) -> Result<(), String>;

const KEYS: [Key; 17] = [
    Key {
        name: "selected-version",
        show: |_| json!(backend::resolve_version(None).ok()),
        set: Some(|s, value| {
            s.default_version = Some(backend::resolve_version(Some(value))?);
            Ok(())
        }),
    },
    Key {
        name: "game-architecture",
        show: |s| json!(s.game_architecture.name()),
        set: Some(|s, value| {
            let choice = backend::GameArchitecture::parse(value).ok_or_else(|| {
                tr("Say auto, x64 or arm64 for the game architecture.").to_string()
            })?;
            if choice != backend::GameArchitecture::Auto && !backend::can_choose_game_architecture()
            {
                return Err(
                    tr("Manual game architectures are available on Windows ARM only.").into(),
                );
            }
            s.game_architecture = choice;
            Ok(())
        }),
    },
    Key {
        name: "minecraft-dir",
        show: |s| json!(s.minecraft_dir),
        set: Some(|s, value| {
            s.minecraft_dir = folder(value);
            Ok(())
        }),
    },
    Key {
        name: "server-dir",
        show: |_| json!(backend::server_dir(None)),
        set: Some(|s, value| {
            s.server_dir = folder(value);
            Ok(())
        }),
    },
    Key {
        name: "install-new-versions",
        show: |s| json!(s.auto_install()),
        set: Some(|s, value| {
            s.install_new_versions = Some(on_off(value)?);
            Ok(())
        }),
    },
    Key {
        name: "prune-old-versions",
        show: |s| json!(s.auto_prune()),
        set: Some(|s, value| {
            s.prune_old_versions = Some(on_off(value)?);
            Ok(())
        }),
    },
    Key {
        name: "keep-versions",
        show: |s| json!(s.keep_newest()),
        set: Some(|s, value| {
            s.keep_versions = match value {
                "default" => None,
                n => Some(
                    n.parse::<usize>()
                        .ok()
                        .filter(|n| *n > 0)
                        .ok_or_else(|| format!("{value}: say a number from 1, or default"))?,
                ),
            };
            Ok(())
        }),
    },
    Key {
        name: "game-args",
        show: |s| json!(s.game_args),
        set: None,
    },
    Key {
        name: "proxy",
        show: |s| json!(s.proxy),
        set: Some(|s, value| {
            if value == "default" || value.is_empty() {
                s.proxy = None;
            } else if let Some(why) = eclipse_launcher::proxy::refuse(value) {
                return Err(why);
            } else {
                s.proxy = Some(value.trim().to_string());
            }
            Ok(())
        }),
    },
    Key {
        name: "desktop-entry",
        show: |s| json!(s.desktop_entry()),
        set: None,
    },
    Key {
        name: "while-playing",
        show: |s| json!(s.while_playing.name()),
        set: Some(|s, value| {
            s.while_playing = backend::WhilePlaying::parse(value).ok_or_else(|| {
                let names = backend::WhilePlaying::ALL.map(backend::WhilePlaying::name);
                format!("{value}: say {}", either(&names))
            })?;
            Ok(())
        }),
    },
    Key {
        name: "language",
        show: |s| json!(s.language.map_or("system", |l| l.code())),
        set: Some(|s, value| {
            s.language = match value.trim() {
                "system" | "default" => None,
                code => Some(Language::parse(code).ok_or_else(|| {
                    let codes: Vec<&str> = Language::ALL.iter().map(|l| l.code()).collect();
                    format!("{value}: say system, {}", either(&codes))
                })?),
            };
            Ok(())
        }),
    },
    Key {
        name: "accent",
        show: |s| json!(s.accent.name()),
        set: Some(|s, value| {
            s.accent = Accent::parse(value).ok_or_else(|| {
                let names: Vec<_> = Accent::ALL.iter().map(|a| a.name()).collect();
                format!(
                    "{value}: say {}, a colour like #2563eb or a hue in degrees (0 to 360)",
                    names.join(", ")
                )
            })?;
            Ok(())
        }),
    },
    Key {
        name: "background",
        show: |s| {
            json!(themes::background(
                &backend::data_dir(),
                s.background.as_deref()
            ))
        },
        set: Some(|s, value| {
            s.background = match value.trim() {
                "default" | "" => None,
                file => Some(themes::keep_background(
                    &backend::data_dir(),
                    std::path::Path::new(file),
                )?),
            };
            Ok(())
        }),
    },
    Key {
        name: "native-profile",
        show: |s| json!(native_mods::Profile::chosen(s).name()),
        // "default", as for the other settings, goes back to `performance`.
        set: Some(|s, value| {
            s.set_native_profile(match value {
                "default" => None,
                _ => Some(
                    native_mods::Profile::parse(value)
                        .ok_or_else(|| format!("{value}: say performance, dev or release"))?,
                ),
            });
            Ok(())
        }),
    },
    Key {
        name: "native-cache-gib",
        show: |s| json!(eclipse_launcher::native_cache::limit_gib(s)),
        set: Some(|s, value| {
            s.native_cache_gib = match value {
                "default" => None,
                n => Some(
                    n.parse::<u64>()
                        .ok()
                        .filter(|n| n.checked_mul(1 << 30).is_some())
                        .ok_or_else(|| {
                            tr("Enter a cache limit in GiB; 0 disables automatic cleanup.")
                                .to_string()
                        })?,
                ),
            };
            Ok(())
        }),
    },
    Key {
        name: "no-ai",
        show: |_| json!(backend::no_ai()),
        set: None,
    },
];

/// The keys `settings` sets, in the order its help lists them.
pub(super) const SETTABLE: [&str; 14] = [
    "selected-version",
    "minecraft-dir",
    "server-dir",
    "install-new-versions",
    "prune-old-versions",
    "keep-versions",
    "game-architecture",
    "proxy",
    "while-playing",
    "language",
    "accent",
    "background",
    "native-profile",
    "native-cache-gib",
];

/// "a, b or c".
fn either(names: &[&str]) -> String {
    let (last, rest) = names.split_last().expect("a list to choose from");
    format!("{} or {last}", rest.join(", "))
}

fn folder(value: &str) -> Option<PathBuf> {
    (value != "default").then(|| PathBuf::from(value))
}

pub(super) fn on_off(value: &str) -> Result<bool, String> {
    match value {
        "on" | "true" => Ok(true),
        "off" | "false" => Ok(false),
        _ => Err(format!("{value}: say on or off")),
    }
}

pub(super) fn set_setting(s: &mut backend::Settings, key: &str, value: &str) -> Result<(), String> {
    match KEYS.iter().find(|k| k.name == key).and_then(|k| k.set) {
        Some(set) => set(s, value),
        None => Err(format!("No setting {key}.")),
    }
}

fn settings_json(s: &backend::Settings) -> Value {
    Value::Object(
        KEYS.iter()
            .map(|k| (k.name.replace('-', "_"), (k.show)(s)))
            .collect(),
    )
}

pub(super) fn settings(
    key: Option<String>,
    value: Option<String>,
    as_json: bool,
) -> Result<(), String> {
    let mut settings = backend::Settings::load();
    if let Some(key) = key {
        let value = value.ok_or("settings KEY VALUE: the value is missing")?;
        set_setting(&mut settings, &key, &value)?;
        settings.save()?;
    }
    let all = settings_json(&settings);
    if as_json {
        json(all);
    } else {
        for (key, value) in all.as_object().into_iter().flatten() {
            println!("{key}: {value}");
        }
    }
    Ok(())
}

pub(super) fn game_args(args: Option<String>, as_json: bool) -> Result<(), String> {
    let mut settings = backend::Settings::load();
    if let Some(line) = args {
        settings.game_args = line;
        settings.save()?;
    }
    let line = settings.game_args;
    if as_json {
        json(serde_json::json!({
            "game_args": line,
            "parsed": backend::split_args(&line),
        }));
    } else if line.is_empty() {
        println!("No extra game arguments.");
    } else {
        println!("Game arguments: {line}");
    }
    Ok(())
}

pub(super) fn desktop_entry(remove: bool) -> Result<(), String> {
    let mut settings = backend::Settings::load();
    settings.desktop_entry = Some(!remove);
    settings.save()?;
    if remove {
        eclipse_launcher::desktop::remove()?;
        println!("Removed Eclipse from the applications menu.");
    } else {
        eclipse_launcher::desktop::install(backend::exe_path())?;
        println!("Eclipse is in the applications menu.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_help_lists_every_key_that_sets_and_only_those() {
        let mut setters: Vec<&str> = KEYS
            .iter()
            .filter(|k| k.set.is_some())
            .map(|k| k.name)
            .collect();
        let mut listed = SETTABLE.to_vec();
        setters.sort();
        listed.sort();
        assert_eq!(setters, listed);
    }
}
