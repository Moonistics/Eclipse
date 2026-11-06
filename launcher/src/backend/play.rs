use super::architecture::game_architecture_launch_error;
use super::minecraft::download_version_for_owner;
use super::net::get_bytes;
use super::paths::{data_dir, game_home, version_dir, versions_dir};
use super::platform::CLIENT;
use super::resources::{complete_assets, complete_reports, is_prepared, resources};
use super::settings::Settings;
use super::versions::game_version;
use crate::{account, mojang};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The command that starts a Java instance's Minecraft, for an account that owns the game: its
/// version downloaded from Mojang the first time (then it starts offline too), the Java it needs
/// (the instance's own, else one found here, else Mojang's, downloaded), and the game in the
/// instance's folder. The caller wraps it (`launch::wrap`) and never logs it: it holds the token.
/// A Java instance's game, ready to start; `server` (`host[:port]`) is joined once it has.
pub fn java_game(
    instance: &crate::instances::Instance,
    server: Option<&str>,
    log: &dyn Fn(String),
) -> Result<Command, String> {
    let game = instance.java.as_ref().ok_or_else(|| {
        format!(
            "{} plays Eclipse, not Minecraft: Java Edition",
            instance.name
        )
    })?;
    let player = account::java_player()?;
    let minecraft = data_dir().join("minecraft");
    let id = mojang::relative(&game.minecraft)?;
    let natives = minecraft.join("natives").join(&id);
    // Each version and loader profile is fetched once; it then starts offline too.
    let downloaded = |id: &str| -> Result<PathBuf, String> {
        Ok(minecraft
            .join("versions")
            .join(mojang::relative(id)?)
            .join(".downloaded"))
    };
    let marker = downloaded(&game.minecraft)?;
    if !marker.is_file() {
        let listed = crate::minecraft_versions::load()?;
        let version = listed.find(&game.minecraft)?;
        download_version_for_owner(&version.id, &version.sha1, log)?;
        fs::write(&marker, "").map_err(|e| format!("{}: {e}", marker.display()))?;
    }
    // The Java the game needs, which also runs a loader's installer.
    let file = |id: &str| -> Result<PathBuf, String> {
        Ok(minecraft
            .join("versions")
            .join(mojang::relative(id)?)
            .join(format!("{id}.json")))
    };
    let vanilla = crate::java_launch::read(&file(&game.minecraft)?)?;
    let need = crate::java::Need::of(&crate::java_launch::resolve(vanilla, &minecraft)?);
    let found = crate::java::pick(
        &need,
        game.home.as_deref(),
        &crate::java::mojang_runtimes(&crate::java::runtimes_dir()),
        &crate::java::installed(),
    )?;
    let runtime = match found {
        Some(runtime) => runtime,
        None => {
            log(format!("Downloading Java {} from Mojang", need.major));
            crate::java::install(&need, log)?
        }
    };
    let mut start = game.minecraft.clone();
    if let Some(loader) = &game.loader {
        start = loader.kind.profile_id(&loader.version, &game.minecraft);
        let marker = downloaded(&start)?;
        if !marker.is_file() {
            start = crate::loaders::install(
                loader,
                &game.minecraft,
                &minecraft,
                &runtime.java(),
                &get_bytes,
                log,
            )?;
            fs::write(downloaded(&start)?, "").map_err(|e| e.to_string())?;
        }
    }
    let version =
        crate::java_launch::resolve(crate::java_launch::read(&file(&start)?)?, &minecraft)?;
    let jvm_args: Vec<String> = game.jvm_args.split_whitespace().map(String::from).collect();
    fs::create_dir_all(&instance.home).map_err(|e| format!("{}: {e}", instance.home.display()))?;
    crate::java_launch::command(
        &version,
        &crate::java_launch::Launch {
            minecraft: &minecraft,
            game_dir: &instance.home,
            natives: &natives,
            java: &runtime.java(),
            memory_mb: game
                .memory_mb
                .unwrap_or(crate::instances::DEFAULT_MEMORY_MB),
            jvm_args: &jvm_args,
            player: &player,
            os: crate::java_launch::os_name(),
            window: game.window,
            server,
        },
    )
}

/// Whether No AI is on: every AI feature hidden and off (`eclipse_core::no_ai`, kept in the
/// game's `eclipse-extras.txt` so the game reads the same switch).
pub fn no_ai() -> bool {
    eclipse_core::no_ai::is_on(&game_home())
}

/// Turns No AI on or off for the launcher and the game.
pub fn set_no_ai(on: bool) -> Result<(), String> {
    eclipse_core::no_ai::set(&game_home(), on).map_err(|e| format!("Could not save No AI: {e}"))
}

// ----- Logs and crash reports ---------------------------------------------------------------------

/// Splits the player's extra game arguments (Settings' one line) into words, the way a shell
/// would: spaces separate them, single or double quotes group a word with spaces in it, and a
/// backslash escapes the next character (outside quotes, or inside double quotes).
pub fn split_args(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else if c == '\\' && q == '"' {
                    // Inside double quotes only `"` and `\` are escaped, as a shell does.
                    match chars.next() {
                        Some(n @ ('"' | '\\')) => word.push(n),
                        Some(n) => {
                            word.push('\\');
                            word.push(n);
                        }
                        None => word.push('\\'),
                    }
                } else {
                    word.push(c);
                }
            }
            None => match c {
                '\'' | '"' => {
                    quote = Some(c);
                    started = true;
                }
                '\\' => {
                    if let Some(n) = chars.next() {
                        word.push(n);
                        started = true;
                    }
                }
                c if c.is_whitespace() => {
                    if started {
                        words.push(std::mem::take(&mut word));
                        started = false;
                    }
                }
                _ => {
                    word.push(c);
                    started = true;
                }
            },
        }
    }
    if started {
        words.push(word);
    }
    words
}

/// The extra game arguments the player set in Settings, split into words.
pub fn game_args() -> Vec<String> {
    split_args(&Settings::load().game_args)
}

/// Appends the player's extra game arguments, so they add to the launcher's own or override them.
pub fn append_game_args(cmd: &mut Command) {
    cmd.args(game_args());
}

/// Renderer workarounds a player may have exported to start the launcher itself (a transparent
/// window on old Mesa drivers): they belong to the launcher's own webview and must not reach the
/// game, which picks its own backend. Inherited, they left the client on CPU rendering at 2-3 FPS
/// (#7832).
pub const LAUNCHER_RENDER_ENV: [&str; 4] = [
    "LIBGL_ALWAYS_SOFTWARE",
    "GALLIUM_DRIVER",
    "WEBKIT_DISABLE_DMABUF_RENDERER",
    "WEBKIT_DISABLE_COMPOSITING_MODE",
];

pub(super) fn strip_launcher_render_env(cmd: &mut Command) {
    for key in LAUNCHER_RENDER_ENV {
        cmd.env_remove(key);
    }
}

pub fn client_command(tag: &str) -> Command {
    let home = game_home();
    let _ = fs::create_dir_all(&home);
    let mut cmd = Command::new(client_program(tag));
    #[cfg(windows)]
    crate::windows_runtime::mark(&mut cmd, &version_dir(tag));
    strip_launcher_render_env(&mut cmd);
    cmd.current_dir(&home)
        .env("ECLIPSE_HOME", &home)
        .env_remove("ECLIPSE_ACCOUNT_SESSIONS")
        .arg("--resources")
        .arg(resources(tag));
    // How the game asks for a renewed session (`account session`) while it runs.
    if let Ok(exe) = std::env::current_exe() {
        cmd.env("ECLIPSE_LAUNCHER_EXE", exe);
    }
    // PipeWire remembers volume and mute per application name. The game's own name is
    // "PipeWire ALSA [eclipse-client]", shared with development and test runs, where
    // muting it once left every later game silent: played from here it is "Eclipse".
    if std::env::var_os("PIPEWIRE_PROPS").is_none() {
        cmd.env(
            "PIPEWIRE_PROPS",
            "{ application.name = \"Eclipse\" media.role = \"Game\" }",
        );
    }
    cmd
}

/// The game's program: on macOS the one inside the version's Eclipse.app, so the Dock and Cmd+Tab
/// show Eclipse's name and icon (game_app.rs); the bare binary elsewhere, or if that fails.
fn client_program(tag: &str) -> PathBuf {
    let dir = version_dir(tag);
    #[cfg(target_os = "macos")]
    match crate::game_app::ensure(&dir) {
        Ok(program) => return program,
        Err(e) => eprintln!("Starting the game outside {}: {e}", crate::game_app::APP),
    }
    dir.join(CLIENT)
}

pub fn server_command(tag: &str, folder: &Path) -> Command {
    let mut cmd = Command::new(version_dir(tag).join(CLIENT));
    cmd.current_dir(folder)
        .arg("--resources")
        .arg(resources(tag))
        .args(["--dedicated", "--nogui"]);
    cmd
}

pub fn eula_agreed(folder: &Path) -> bool {
    fs::read_to_string(folder.join("eula.txt"))
        .is_ok_and(|s| s.lines().any(|l| l.trim() == "eula=true"))
}

/// Writes eula.txt as vanilla's server expects it, once the player agreed in the launcher.
pub fn agree_eula(folder: &Path) -> Result<(), String> {
    fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let text = "#By changing the setting below to TRUE you are indicating your agreement to our EULA (https://aka.ms/MinecraftEULA).\neula=true\n";
    fs::write(folder.join("eula.txt"), text).map_err(|e| e.to_string())
}

// ----- Shared steps for the front ends -----------------------------------------------------------

/// The server folder: the one given, the one chosen in the launcher, or `server/` in the data folder.
pub fn server_dir(requested: Option<PathBuf>) -> PathBuf {
    requested
        .or(Settings::load().server_dir)
        .unwrap_or_else(|| data_dir().join("server"))
}

/// Whether `tag` is prepared, with the run-time reports it needs completed when another copy has them.
pub fn need_prepared(tag: &str) -> Result<(), String> {
    if is_prepared(tag) {
        complete_reports(&versions_dir(), tag);
        Ok(())
    } else {
        Err(format!(
            "{tag} is not prepared yet. Prepare it first: `eclipse-launcher prepare --version {tag}` \
             uses your own Minecraft {v}, or downloads it from Mojang for the signed-in account \
             that owns it.",
            v = game_version(tag)
        ))
    }
}

/// The client's command, once the owner gate lets it start and its resources are prepared. Past
/// the gate, a sign-in that can't be renewed (no network, expired) still lets the game start,
/// offline.
pub fn client(tag: &str) -> Result<Command, String> {
    client_as(tag, None)
}

/// What the command line adds to a refusal to play for want of an account.
pub const SIGN_IN_HINT: &str = "Sign in with `eclipse-launcher account sign-in`.";

/// [`client`], playing as `who` for this launch only (`play --account`).
pub fn client_as(tag: &str, who: Option<&account::Player>) -> Result<Command, String> {
    if let Some(error) = game_architecture_launch_error(tag) {
        return Err(error);
    }
    let session = match who {
        None => account::session_for_game(&game_home(), false).map_err(|e| e.to_string()),
        Some(who) => account::session_for_game_as(&game_home(), who),
    }
    .map_err(|refused| format!("{refused} {}", crate::i18n::tr(SIGN_IN_HINT)))?;
    if let Err(e) = need_prepared(tag) {
        account::discard_unread_sessions();
        return Err(e);
    }
    complete_assets(tag);
    crate::browser::eclipse::before_play(&game_home());
    let mut cmd = client_command(tag);
    if let Some(file) = session.file {
        cmd.arg("--session-file").arg(file);
    }
    cmd.envs(session.env);
    if let Some(e) = session.offline {
        eprintln!("Playing offline: {e}");
    }
    if let Some(e) = account::take_unsaved() {
        eprintln!("{e}");
    }
    append_game_args(&mut cmd);
    Ok(cmd)
}

/// Start the game straight into a friend's world: `--server friend:CODE`.
pub fn join_friend(cmd: &mut Command, code: &str) -> Result<(), String> {
    eclipse_friends::invite::parse_detailed(code).map_err(|e| e.to_string())?;
    let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    cmd.arg("--server")
        .arg(format!("{}{code}", eclipse_friends::ADDRESS_PREFIX));
    Ok(())
}

/// The dedicated server's command in `folder`, once prepared and the EULA is agreed to.
pub fn server(tag: &str, folder: &Path, accept_eula: bool) -> Result<Command, String> {
    if let Some(error) = game_architecture_launch_error(tag) {
        return Err(error);
    }
    need_prepared(tag)?;
    if accept_eula {
        agree_eula(folder)?;
    }
    if !eula_agreed(folder) {
        return Err("Running a server needs your agreement to the Minecraft EULA (https://aka.ms/MinecraftEULA). Accept it to continue.".into());
    }
    fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    Ok(server_command(tag, folder))
}
