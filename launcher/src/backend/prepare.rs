use super::minecraft::download_for_owner;
use super::net::get_bytes;
use super::paths::{data_dir, linked_minecraft_dir, own_minecraft_dir, version_dir};
use super::resources::PREPARER_CRATE;
use super::versions::{game_version, read_json};
use crate::sources;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(super) fn found(program: &str, arg: &str) -> bool {
    crate::process::command(program)
        .arg(arg)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn python() -> Option<&'static str> {
    let candidates: &[&'static str] = if cfg!(windows) {
        &["py", "python"]
    } else {
        &["python3", "python"]
    };
    candidates.iter().copied().find(|p| found(p, "--version"))
}

/// The server jar the version json `manifest` names, at `jar`: the one kept there when it is
/// Mojang's, else downloaded and checked against the json's sha1.
pub(super) fn server_jar(
    manifest: &Path,
    version: &str,
    jar: &Path,
    fetch: sources::Fetch,
    log: &dyn Fn(String),
) -> Result<(), String> {
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let server = &json["downloads"]["server"];
    let (Some(url), Some(sha1)) = (server["url"].as_str(), server["sha1"].as_str()) else {
        return Err(format!("{} has no server download", manifest.display()));
    };
    if sources::sha1_file(jar).as_deref() == Some(sha1) {
        return Ok(());
    }
    log(format!("Downloading the {version} server jar from Mojang"));
    let bytes = sources::fetch_checked(fetch, url, sha1)
        .map_err(|e| format!("The {version} server jar from Mojang {e}"))?;
    crate::files::write(jar, &bytes)
}

/// How a version's resources are prepared.
pub enum Preparer {
    /// eclipse-prepare, linked into the launcher: the version ships the same preparer sources.
    InProcess(eclipse_prepare::Options),
    /// The version's own preparer: its eclipse-prepare binary when its sources differ from the
    /// launcher's (a newer or older release), or the Python script of versions made before it.
    Process(Command),
}

/// The version's eclipse-prepare binary, when it ships one.
fn preparer_binary(dir: &Path) -> Option<PathBuf> {
    let binary = dir.join(if cfg!(windows) {
        "eclipse-prepare.exe"
    } else {
        "eclipse-prepare"
    });
    binary.is_file().then_some(binary)
}

/// Runs `cmd` with Java at `home`: JAVA_HOME names it and its `bin` leads the PATH, as the
/// versions' own preparers look for Java.
pub(super) fn with_java(cmd: &mut Command, home: &Path) {
    let mut path = vec![home.join("bin")];
    path.extend(
        std::env::var_os("PATH")
            .iter()
            .flat_map(std::env::split_paths),
    );
    cmd.env("JAVA_HOME", home);
    if let Ok(path) = std::env::join_paths(path) {
        cmd.env("PATH", path);
    }
}

/// The Minecraft folder a preparation whose game files are imported already reads: the sounds,
/// the title panorama, the fonts and the languages come from its asset store. It is the one
/// `resolve` found, linked into the official layout for another launcher's files; without one, the
/// chosen folder, and the preparation goes without them, saying so. Before, such a preparation
/// read the chosen folder as it was, which a Prism Launcher or Modrinth App install does not
/// keep its assets in: the game was silent and its title screen grey.
pub(super) fn assets_folder(
    resolved: Result<sources::Resolved, String>,
    chosen: &Path,
    version: &str,
    log: &dyn Fn(String),
) -> PathBuf {
    match resolved {
        Ok(sources::Resolved::Folder(dir)) => dir,
        Ok(sources::Resolved::NotFound(_)) => {
            log(format!(
                "Minecraft {version}'s assets were not found in any launcher folder: the game \
                 will be silent and the title screen plain"
            ));
            chosen.to_path_buf()
        }
        Err(e) => {
            log(format!("Minecraft {version}'s assets: {e}"));
            chosen.to_path_buf()
        }
    }
}

/// Where the preparation takes Minecraft's files from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinecraftFrom {
    /// The player's own install (any launcher's); Mojang only when none has the version and an
    /// account that owns the game is signed in.
    Install,
    /// Mojang, for the signed-in account that owns the game ("Download and prepare").
    Mojang,
}

/// A preparation's steps, in the order they run, as Home names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrepareStep {
    /// Minecraft's files: found in an install, or downloaded from Mojang.
    Files,
    Java,
    Sounds,
    /// eclipse-prepare's own work on those files.
    Prepare,
}

impl PrepareStep {
    pub const ALL: [PrepareStep; 4] = [Self::Files, Self::Java, Self::Sounds, Self::Prepare];

    pub fn name(self) -> &'static str {
        match self {
            Self::Files => "Game files",
            Self::Java => "Java",
            Self::Sounds => "Sounds",
            Self::Prepare => "Prepare",
        }
    }
}

/// The step the preparation under way is at (`PrepareStep::ALL`'s index).
static PREPARE_STEP: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn prepare_step() -> PrepareStep {
    PrepareStep::ALL[PREPARE_STEP.load(std::sync::atomic::Ordering::Relaxed)]
}

pub fn set_prepare_step(step: PrepareStep) {
    PREPARE_STEP.store(step as usize, std::sync::atomic::Ordering::Relaxed);
}

/// Checks what the preparation needs and picks how to run it. PLAY.txt describes the same
/// steps: the player's own launcher install, the server jar from that version's own json, and
/// Java (and Python, for versions before eclipse-prepare). Also returns the Minecraft folder
/// to prepare from: `minecraft`, or the files another launcher keeps, linked into the official
/// launcher's layout (sources.rs).
pub fn preparer(
    tag: &str,
    minecraft: &Path,
    from: MinecraftFrom,
    log: &dyn Fn(String),
) -> Result<(Preparer, PathBuf), String> {
    set_prepare_step(PrepareStep::Files);
    let version = game_version(tag);
    let dir = version_dir(tag);
    let reused = eclipse_prepare::artifacts::reuse_imported(&dir, &|line| log(line.to_string()))?;
    let chosen = minecraft.to_path_buf();
    // Resolved even when the game files are reused: the assets still come from the install.
    let minecraft = {
        let pins = read_json(&dir.join("research-lock.json"))
            .and_then(|lock| sources::Pins::read(&lock))
            .ok_or("The version's research-lock.json doesn't pin the Minecraft files")?;
        let own = own_minecraft_dir();
        let mut known = vec![("Eclipse's own download", own.clone())];
        known.extend(sources::known_folders_here());
        let linked = linked_minecraft_dir();
        if from == MinecraftFrom::Mojang {
            // Straight from Mojang (what is already there and whole is kept), for an owner only.
            download_for_owner(&pins, &own, &get_bytes, log)?;
            own
        } else {
            let resolved = sources::resolve(minecraft, &pins, &known, &linked, &get_bytes, log);
            if reused {
                assets_folder(resolved, &chosen, &version, log)
            } else {
                match resolved? {
                    sources::Resolved::Folder(dir) => dir,
                    sources::Resolved::NotFound(checked) => {
                        let not_found = sources::not_found(&version, &checked, minecraft);
                        download_for_owner(&pins, &own, &get_bytes, log)
                            .map_err(|e| format!("{not_found}\n  • {e}"))?;
                        own
                    }
                }
            }
        }
    };
    let manifest = minecraft
        .join("versions")
        .join(&version)
        .join(format!("{version}.json"));
    let binary = preparer_binary(&dir);
    let python = match binary {
        Some(_) => None,
        None => Some(python().ok_or(
            "This version prepares with Python: Python 3.10 or newer is needed. Install it, or pick a newer version.",
        )?),
    };
    // The Java the version pins, found as eclipse-prepare finds it, so a missing one is
    // reported (or downloaded) before the server jar is downloaded. Versions from before
    // java_major ran Java 25.
    let major = read_json(&dir.join("research-lock.json"))
        .and_then(|lock| lock["minecraft"]["java_major"].as_i64())
        .unwrap_or(25);
    let mut games = vec![chosen, minecraft.clone(), own_minecraft_dir()];
    games.extend(
        sources::known_folders_here()
            .into_iter()
            .map(|(_, dir)| dir),
    );
    set_prepare_step(PrepareStep::Java);
    let java_home = crate::java::for_preparing(
        major,
        !eclipse_prepare::exporters::available(&dir),
        &games,
        || {
            read_json(&manifest)
                .and_then(|json| json["javaVersion"]["component"].as_str().map(String::from))
        },
        log,
    )?;
    let jar = data_dir()
        .join("server-jars")
        .join(format!("{version}.jar"));
    if !reused {
        server_jar(&manifest, &version, &jar, &get_bytes, log)?;
    }
    if let Some(binary) = binary {
        if own_preparer(&dir) {
            let options = eclipse_prepare::Options {
                root: dir,
                import: !reused,
                minecraft_dir: Some(minecraft.clone()),
                server_jar: (!reused).then_some(jar),
                java_home: Some(java_home.clone()),
                inputs: Default::default(),
                output: None,
            };
            return Ok((Preparer::InProcess(options), minecraft));
        }
        let mut cmd = Command::new(binary);
        with_java(&mut cmd, &java_home);
        cmd.current_dir(&dir).arg("--root").arg(&dir);
        if !reused {
            cmd.args(["--prepare", "--server-jar"]).arg(&jar);
        }
        cmd.arg("--minecraft-dir").arg(&minecraft);
        return Ok((Preparer::Process(cmd), minecraft));
    }
    let mut cmd = Command::new(python.unwrap());
    with_java(&mut cmd, &java_home);
    cmd.current_dir(&dir)
        .args(["-u", "tools/prepare_native_client.py"]);
    if !reused {
        cmd.args(["--prepare", "--server-jar"]).arg(&jar);
    }
    cmd.arg("--minecraft-dir").arg(&minecraft);
    Ok((Preparer::Process(cmd), minecraft))
}

/// Copies the pinned version's sounds from the player's own launcher install into the version's
/// resources (`eclipse-prepare sounds`), each line to `log`. Sound is optional, so a launcher
/// install without them is a line in the log, never a failed preparation. The version's own
/// preparer does it when it is not the launcher's and knows how; versions from before it read
/// the sounds where the launcher's preparer puts them.
pub fn sounds(tag: &str, minecraft: &Path, log: &dyn Fn(String)) {
    let dir = version_dir(tag);
    let own = own_preparer(&dir);
    let binary = preparer_binary(&dir).filter(|binary| {
        !own && crate::process::command(binary)
            .arg("--help")
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("eclipse-prepare sounds"))
    });
    set_prepare_step(PrepareStep::Sounds);
    log("Copying the sounds from your Minecraft install".into());
    let result = match binary {
        Some(binary) => crate::process::command(binary)
            .current_dir(&dir)
            .arg("sounds")
            .arg("--root")
            .arg(&dir)
            .arg("--minecraft-dir")
            .arg(minecraft)
            .output()
            .map_err(|e| e.to_string())
            .and_then(|o| {
                let text = |b: &[u8]| String::from_utf8_lossy(b).trim().to_string();
                for line in text(&o.stdout).lines() {
                    log(line.to_string());
                }
                match o.status.success() {
                    true => Ok(()),
                    false => Err(text(&o.stderr).trim_start_matches("error: ").to_string()),
                }
            }),
        None => caught("The sound copy stopped unexpectedly", || {
            eclipse_prepare::sounds(&dir, Some(minecraft), &|line| log(line.to_string()))
        })
        .map(|_| ()),
    };
    if let Err(e) = result {
        log(format!("No sound (the game will be silent): {e}"));
    }
}

/// Runs `Preparer::InProcess` on this thread, each progress line to `log`. A panic is an
/// error like any other, so the window never waits on a preparation that died.
pub fn prepare_in_process(
    options: &eclipse_prepare::Options,
    log: &dyn Fn(String),
) -> Result<(), String> {
    caught("The preparation stopped unexpectedly", || {
        eclipse_prepare::prepare(options, &|line| log(line.to_string()))
    })
    .map(|_| ())
}

/// `run`, a panic in it the error `stopped`: the window never waits on work that died.
pub(super) fn caught<T>(
    stopped: &str,
    run: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
        .unwrap_or_else(|_| Err(stopped.into()))
}

/// Whether the version in `dir` ships the launcher's own preparer, which then runs in-process.
fn own_preparer(dir: &Path) -> bool {
    eclipse_prepare::sources_sha256(&dir.join(PREPARER_CRATE)).as_deref()
        == Some(eclipse_prepare::SOURCES_SHA256)
}
