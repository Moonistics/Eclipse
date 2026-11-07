use super::architecture::GameArchitecture;
use super::paths::data_dir;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

const FILE: &str = "settings.json";

/// How many of the newest release versions stay installed when the player has not said.
const KEEP_NEWEST: usize = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    Dark,
    Light,
    /// Follow the system's light or dark.
    System,
}

/// What the launcher's window does while the game runs, as the player picks it in Settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WhilePlaying {
    #[default]
    Stay,
    /// The window goes to the Dock or the taskbar, and comes back when the game closes.
    Minimize,
    /// The launcher hides (macOS only: elsewhere it minimises), and comes back when the game
    /// closes.
    Hide,
}

impl WhilePlaying {
    pub const ALL: [WhilePlaying; 3] = [Self::Stay, Self::Minimize, Self::Hide];

    /// Its name in `settings.json` and on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Stay => "stay",
            Self::Minimize => "minimize",
            Self::Hide => "hide",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        let name = match name.as_str() {
            "minimise" => "minimize",
            "keep" | "open" | "off" => "stay",
            n => n,
        };
        Self::ALL.into_iter().find(|w| w.name() == name)
    }

    /// What this computer does for it: only macOS can hide a whole app.
    pub fn here(self) -> Self {
        match self {
            Self::Hide if !cfg!(target_os = "macos") => Self::Minimize,
            w => w,
        }
    }
}

/// What native builds are compiled for, as the player picks it (`native_mods::Profile::cargo`
/// says how each one compiles).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeProfile {
    #[default]
    Performance,
    Dev,
    Release,
}

impl NativeProfile {
    pub fn name(self) -> &'static str {
        match self {
            Self::Performance => "performance",
            Self::Dev => "dev",
            Self::Release => "release",
        }
    }

    /// The profile named `text`; `fast`, `Performance`'s name before `Dev`, still names it.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "performance" | "fast" => Some(Self::Performance),
            "dev" => Some(Self::Dev),
            "release" => Some(Self::Release),
            _ => None,
        }
    }

    /// The settings' profile: the one picked, else `Release` for an older file's release switch.
    pub fn chosen(settings: &Settings) -> Self {
        settings
            .native_profile
            .unwrap_or(if settings.native_release {
                Self::Release
            } else {
                Self::Performance
            })
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    #[serde(rename = "selected")]
    pub default_version: Option<String>,
    /// Game asset architecture. Auto preserves the launcher's process-architecture behavior.
    #[serde(default)]
    pub game_architecture: GameArchitecture,
    /// The instance Play starts (`instances`), by id. Unset means Eclipse, the game's home.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    pub minecraft_dir: Option<PathBuf>,
    pub server_dir: Option<PathBuf>,
    /// Unset means on: only a player who turned it off has `false` here. 0.1.1 wrote its
    /// `auto_install` (off by default) for everyone, so this is a new key and that one is ignored.
    #[serde(default)]
    pub install_new_versions: Option<bool>,
    /// Unset means on: after an install the launcher deletes the release versions nothing needs
    /// (`prune_old_versions`). Only a player who turned it off has `false` here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prune_old_versions: Option<bool>,
    /// How many of the newest release versions pruning keeps. Unset means [`KEEP_NEWEST`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_versions: Option<usize>,
    /// Linux: whether the launcher keeps its entry in the applications menu (`desktop`). Unset
    /// means yes; removing the entry stores `false`, so it doesn't come back on the next start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desktop_entry: Option<bool>,
    /// Extra arguments for the game's command line, as one line (see [`split_args`]). Empty by
    /// default. Appended after the launcher's own arguments, so they can add to or override them.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub game_args: String,
    /// The proxy every download goes through (`proxy`), such as `http://127.0.0.1:7890` or
    /// `socks5://127.0.0.1:1080`.
    /// Unset means none: the environment's `HTTPS_PROXY` and `HTTP_PROXY` apply instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    #[serde(default)]
    pub appearance: Appearance,
    /// The player has read what the "I'm playing" ping holds: on the first-run question, or on the
    /// Play page once if they answered that before it existed.
    #[serde(default)]
    pub ping_notice_seen: bool,
    /// The player without an account that owns the game has read, once, why the launcher needs
    /// one now (the owner gate's screen).
    #[serde(default)]
    pub owner_notice_seen: bool,
    #[serde(default)]
    pub while_playing: WhilePlaying,
    /// The accent colour on top of the appearance (`themes::Accent`). Eclipse's amber by default.
    #[serde(default)]
    pub accent: crate::themes::Accent,
    /// The launcher theme (`themes::theme`): its sky behind the light appearance. Unset is
    /// Eclipse's own.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::themes::stored_theme"
    )]
    pub theme: Option<String>,
    /// Home's background: a picture the player picked, kept in the data folder under this name
    /// (`themes::keep_background`). Unset shows Eclipse's landscape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// The window's language. Unset follows the system's (`i18n::Language::system`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<crate::i18n::Language>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduce_motion: Option<bool>,
    /// Whether what the browser installed from Eclipse's registry updates by itself, at start
    /// and before Play (`browser::eclipse::auto`). Unset is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mods_auto_update: Option<bool>,
    /// The player has read the notice before the first native mod (`native_status::NOTICE`).
    /// Native mods are always on: an older file's `native_mods` switch is ignored.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub native_mods_noticed: bool,
    /// The profile native builds compile with ([`NativeProfile::chosen`]), set through
    /// [`Settings::set_native_profile`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    native_profile: Option<NativeProfile>,
    /// Combined fast/release compilation cache limit in GiB. Unset is 30; zero disables
    /// automatic cleanup. Installed native versions and player data are not this cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_cache_gib: Option<u64>,
    /// An older file's switch for the release profile, read when `native_profile` is unset.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    native_release: bool,
    /// Developer options (hidden: seven clicks on the sidebar's launcher version reveal them):
    /// offline names play without an account that owns the game, in the window only
    /// (`account::Accounts::play_as_with`, `account::launch_for_game`). Never a download: the game still
    /// comes from the player's own install. Off by default, and absent from the file while off.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dev_offline: bool,
}

impl Settings {
    pub fn load() -> Self {
        Self::load_from(&data_dir().join(FILE))
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&data_dir().join(FILE))
    }

    pub fn update<T>(change: impl FnOnce(&mut Settings) -> T) -> Result<T, String> {
        Self::update_at(&data_dir().join(FILE), change)
    }

    /// The settings file's write time and size, to notice a change made in the launcher.
    pub fn stamp() -> Option<(std::time::SystemTime, u64)> {
        let meta = fs::metadata(data_dir().join(FILE)).ok()?;
        Some((meta.modified().ok()?, meta.len()))
    }

    /// Every key whose value this launcher can read; one it can't (a newer launcher's appearance, a
    /// language it doesn't have yet) is left at its default and the others stay.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        if text
            .trim_matches(|c: char| c.is_whitespace() || c == '\0')
            .is_empty()
        {
            return Ok(Self::default());
        }
        let fields: serde_json::Map<String, serde_json::Value> = serde_json::from_str(text)?;
        Ok(
            serde_json::from_value(fields.clone().into()).unwrap_or_else(|_| {
                let readable: serde_json::Map<_, _> = fields
                    .into_iter()
                    .filter(|(key, value)| {
                        serde_json::from_value::<Self>(serde_json::json!({ key: value })).is_ok()
                    })
                    .collect();
                serde_json::from_value(readable.into()).unwrap_or_default()
            }),
        )
    }

    pub(super) fn load_from(path: &Path) -> Self {
        Self::read(path).unwrap_or_default()
    }

    fn read(path: &Path) -> Result<Self, String> {
        let unread = |why: String| format!("{why}. Fix or remove it to save settings again.");
        match fs::read(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(unread(format!("{} can't be read ({e})", path.display()))),
            Ok(bytes) => Self::parse(&String::from_utf8_lossy(&bytes)).map_err(|e| {
                unread(format!(
                    "{} can't be read ({:?} error at line {}, column {})",
                    path.display(),
                    e.classify(),
                    e.line(),
                    e.column()
                ))
            }),
        }
    }

    pub(super) fn save_to(&self, path: &Path) -> Result<(), String> {
        let _held = hold(path)?;
        Self::read(path)?;
        crate::files::write_json(path, self)?;
        crate::proxy::announce(self.proxy.clone());
        Ok(())
    }

    pub(super) fn update_at<T>(
        path: &Path,
        change: impl FnOnce(&mut Settings) -> T,
    ) -> Result<T, String> {
        let _held = hold(path)?;
        let mut settings = Self::read(path)?;
        let answer = change(&mut settings);
        crate::files::write_json(path, &settings)?;
        crate::proxy::announce(settings.proxy.clone());
        Ok(answer)
    }

    /// Picks the profile native builds compile with (None: the default); an older file's release
    /// switch goes with it.
    pub fn set_native_profile(&mut self, profile: Option<NativeProfile>) {
        self.native_profile = profile;
        self.native_release = false;
    }

    pub fn desktop_entry(&self) -> bool {
        self.desktop_entry.unwrap_or(true)
    }
    pub fn language(&self) -> crate::i18n::Language {
        self.language.unwrap_or_else(crate::i18n::Language::system)
    }
    pub fn auto_install(&self) -> bool {
        self.install_new_versions.unwrap_or(true)
    }
    pub fn auto_prune(&self) -> bool {
        self.prune_old_versions.unwrap_or(true)
    }
    pub fn auto_updates_mods(&self) -> bool {
        self.mods_auto_update.unwrap_or(true)
    }
    pub fn keep_newest(&self) -> usize {
        self.keep_versions.unwrap_or(KEEP_NEWEST).max(1)
    }
}

fn hold(path: &Path) -> Result<(MutexGuard<'static, ()>, fs::File), String> {
    static WRITES: Mutex<()> = Mutex::new(());
    let process = WRITES.lock().unwrap_or_else(|e| e.into_inner());
    let lock = path.with_extension("lock");
    let shown = |e: std::io::Error| format!("{}: {e}", lock.display());
    if let Some(dir) = lock.parent() {
        fs::create_dir_all(dir).map_err(shown)?;
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock)
        .map_err(shown)?;
    file.lock().map_err(shown)?;
    Ok((process, file))
}
