//! `eclipse-launcher <subcommand>`: the same steps as the window, from a terminal. Every
//! command goes through the library; this module only parses arguments and prints.
use clap::{Parser, Subcommand};
use eclipse_launcher::backend;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::process::Command;

pub(crate) mod account;
mod agents;
mod data;
mod instance;
mod java;
mod logs;
mod modpack;
mod mods;
mod multiplayer;
mod native;
mod packs;
mod play;
mod rust;
mod screenshots;
mod servers;
mod settings;
mod versions;
mod worlds;

#[derive(Parser)]
#[command(
    name = "eclipse-launcher",
    version = backend::VERSION,
    about = "Installs, prepares and starts Eclipse. Without a subcommand, opens the window."
)]
struct Cli {
    /// Print JSON instead of text, where a command has something to report
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Write a shell completion script to stdout
    Completions { shell: clap_complete::Shell },
    /// List Eclipse releases, or the installed versions
    Versions {
        /// Only the versions installed here (no network)
        #[arg(long)]
        installed: bool,
    },
    /// Download and install a release (a tag such as v2026.10.13, or "latest")
    Install { tag: String },
    /// Remove an installed version
    Remove { tag: String },
    /// Install a game program you have, such as a eclipse-client built on your fork's Actions,
    /// as its own version on an installed release or Nightly's game files, and select it.
    /// Nothing signed it: it runs with your rights, and updates never replace it
    ImportBuild {
        /// The program (eclipse-client, eclipse-client.exe on Windows), or a zip holding it
        path: PathBuf,
        /// The installed release or Nightly whose game files it plays with (default: the
        /// selected one, else the newest)
        #[arg(long)]
        version: Option<String>,
        /// You picked this build yourself and trust it with full access to this computer
        #[arg(long)]
        trust_unsigned: bool,
    },
    /// Prepare a version's resources from your own Minecraft install, or from Mojang
    Prepare {
        /// The game folder of the official launcher, Prism Launcher, MultiMC, the Modrinth App,
        /// ATLauncher or GDLauncher (default: the launcher's, then every launcher's default)
        #[arg(long, conflicts_with = "download")]
        minecraft: Option<PathBuf>,
        /// Download Minecraft from Mojang for the signed-in account that owns it, instead of
        /// using an install
        #[arg(long)]
        download: bool,
        /// The installed version (default: the selected one)
        #[arg(long)]
        version: Option<String>,
    },
    /// Start the client (needs a signed-in Microsoft account that owns Minecraft: Java Edition)
    Play(play::PlayArgs),
    /// Run a dedicated server here; type commands into its console
    Server {
        /// The server's folder (default: the one the launcher uses)
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        version: Option<String>,
        /// Agree to the Minecraft EULA (https://aka.ms/MinecraftEULA)
        #[arg(long)]
        accept_eula: bool,
    },
    /// Show the native compilation cache's size and limit as JSON. Installed games and saves
    /// are never removed; builds must be idle
    NativeCache {
        /// Clear only fast/release compiler outputs. The next changed build recompiles
        #[arg(long)]
        clear: bool,
    },
    /// Native mods (docs/native-mods.md): compile mixin bundles on the Eclipse revision they
    /// pin, else the one the instance plays, with the launcher's own Rust, as the version the
    /// instance then plays. Not sandboxed: the mods' code runs with your rights
    NativeBuild(native::BuildArgs),
    /// Show a release's patch notes (default: the newest)
    Notes { tag: Option<String> },
    /// Show the Discord, sponsor and website links
    Links,
    /// Microsoft accounts: list them (the default), sign in, sign out, or pick the one that plays
    Account {
        #[command(subcommand)]
        action: Option<account::AccountCmd>,
    },
    /// Show the launcher's settings, or set one (the Settings page): selected-version TAG,
    /// game-architecture auto|x64|arm64,
    /// minecraft-dir PATH, server-dir PATH ("default" resets a folder), install-new-versions
    /// on|off, prune-old-versions on|off (delete old versions after an install, keeping the
    /// selected one, the ones instances use and the newest few), keep-versions N (how many of the
    /// newest to keep; "default" is 2), proxy http://host:port|socks5://host:port
    /// ("default" removes it), while-playing stay|minimize|hide (what the window does while the game runs), language
    /// system|en|fr|ru|de|zh|es|uk|pl (the window's language; system follows the computer's), accent
    /// violet|blue|teal|green|amber|red|pink|#rrggbb|DEGREES (a named accent, any colour, or any
    /// hue), background PATH (a picture behind Home; "default"
    /// shows Eclipse's landscape), native-cache-gib N (combined native compilation cache limit
    /// in GiB, default 30; 0 disables automatic cleanup), native-profile performance|dev|release (the profile native
    /// mods build with: performance is incremental, dev rebuilds a mod quickest for a slow game,
    /// release is slowest to build and slightly faster to play)
    Settings {
        #[arg(value_parser = settings::SETTABLE)]
        key: Option<String>,
        /// The new value (with KEY)
        #[arg(requires = "key")]
        value: Option<String>,
    },
    /// Show or set the extra arguments the game starts with (for example --server-thread);
    /// quotes group a word with spaces in it
    GameArgs {
        /// Omit to show the current arguments
        #[arg(allow_hyphen_values = true)]
        args: Option<String>,
    },
    /// Upload an instance's latest.log to mclo.gs (a paste service for Minecraft logs) and print
    /// its link, for a bug report: only when the player asks. The home folder, the user name and
    /// the account names are taken out first; the link expires after 90 days unread
    ShareLog {
        /// The instance (default: the selected one)
        #[arg(long)]
        instance: Option<String>,
    },
    /// Delete a log shared with share-log: the "<id>:<token>" it printed
    UnshareLog { shared: String },
    /// Print the end of the game's log (logs/latest.log in the game folder), or its newest crash
    /// report
    Logs {
        /// How many lines of latest.log
        #[arg(long, default_value_t = 100)]
        lines: usize,
        /// The newest crash report instead
        #[arg(long)]
        crash: bool,
    },
    /// Look for a newer Eclipse release and launcher, and install them
    Update {
        /// Only report what is available
        #[arg(long)]
        check: bool,
    },
    /// Linux: put Eclipse in the applications menu (an entry and the icon, under
    /// $XDG_DATA_HOME), or take it out with --remove
    DesktopEntry {
        /// Remove the entry and the icon, and stop adding them back
        #[arg(long)]
        remove: bool,
    },
    /// Opt-in data collection (docs/telemetry.md): what is allowed, what would be sent, and
    /// deleting it; all off until you allow a kind
    Data {
        #[command(subcommand)]
        action: Option<data::DataCmd>,
    },
    /// Instances: separate game folders, each with its own worlds, options and packs
    Instance {
        #[command(subcommand)]
        action: Option<instance::InstanceCmd>,
    },
    /// An instance's single-player worlds: list them, back one up, copy or delete it
    Worlds {
        #[command(subcommand)]
        action: Option<worlds::WorldsCmd>,
    },
    /// An instance's resource and shader packs: list, add, turn on or off, remove
    Packs {
        #[command(subcommand)]
        action: Option<packs::PacksCmd>,
    },
    /// Java for Minecraft: Java Edition instances: list the runtimes, download Mojang's
    Java {
        #[command(subcommand)]
        action: Option<java::JavaCmd>,
    },
    /// The screenshots F2 took in an instance: list, delete
    Screenshots {
        #[command(subcommand)]
        action: Option<screenshots::ScreenshotsCmd>,
    },
    /// The servers in an instance's Multiplayer list (servers.dat): list, add, remove
    Multiplayer {
        #[command(subcommand)]
        action: Option<multiplayer::MultiplayerCmd>,
    },
    /// Modpacks: import a Java pack, see its coverage report, port what this computer can and
    /// play it as its own profile (docs/modpacks.md)
    Modpack {
        #[command(subcommand)]
        action: Option<modpack::ModpackCmd>,
    },
    /// The mod browser: Java mods, resource packs and modpacks from Modrinth
    /// (CurseForge support is coming soon), each file checked against its checksum
    /// (docs/mod-browser.md)
    Mods {
        #[command(subcommand)]
        action: Option<mods::ModsCmd>,
    },
    /// The portable Rust toolchain native mods compile with (docs/native-mods.md): the launcher's
    /// own, in its data folder, never installed on the system
    Rust {
        #[command(subcommand)]
        action: Option<rust::RustCmd>,
    },
    /// Servers hosted with eclipse-server (the Servers page): list them, create one, or run any
    /// eclipse-server command on one
    Servers {
        #[command(subcommand)]
        action: Option<servers::ServersCmd>,
    },
    /// Serve the Model Context Protocol on stdin and stdout, for AI agents (refused while No AI
    /// is on)
    Mcp,
    /// How to connect an AI agent (Claude Code, Codex, Cursor, any MCP client) to `mcp`: the
    /// config to paste and the command that adds it (refused while No AI is on)
    AgentConfig {
        /// Only this client
        #[arg(long, value_parser = ["claude-code", "codex", "cursor", "generic"])]
        client: Option<String>,
        /// Start the MCP server as a client would and say whether it answered
        #[arg(long)]
        test: bool,
    },
    /// Show or set No AI: on hides and turns off every AI feature (the companion, agent control,
    /// eclipse-ctl and both MCP servers); off, the default, makes them available
    NoAi {
        /// Omit to show the current state
        #[arg(value_parser = ["on", "off"])]
        state: Option<String>,
    },
    /// Measure the launcher: starts (the first is the cold one), page switching, idle CPU and
    /// memory, the release list fetch; writes a benchmark report to benchmarks/ in its data folder
    Benchmark {
        /// How many times to start it
        #[arg(long, default_value_t = 5)]
        repeats: usize,
    },
}

/// The CLI's definition, for the MCP tools generated from it (cli_tools.rs).
pub fn command() -> clap::Command {
    <Cli as clap::CommandFactory>::command()
}

/// The commands that exist for AI agents: hidden and refused while No AI is on. Every other
/// command stays: the launcher, packagers and scripts need them, and none drives the game.
pub const AI_COMMANDS: [&str; 2] = ["mcp", "agent-config"];

/// Runs a subcommand and returns the exit code, or None when the window should open.
pub fn run() -> Option<i32> {
    use clap::FromArgMatches;
    let no_ai = backend::no_ai();
    let mut definition = command();
    if no_ai {
        // Hidden from the help too; a call still gets the refusal below.
        for name in AI_COMMANDS {
            definition = definition.mut_subcommand(name, |c| c.hide(true));
        }
    }
    let cli = Cli::from_arg_matches(&definition.get_matches()).unwrap_or_else(|e| e.exit());
    let command = cli.command?;
    let result = match command {
        Cmd::Mcp | Cmd::AgentConfig { .. } if no_ai => Err(eclipse_core::no_ai::REFUSAL.into()),
        Cmd::Mcp => crate::mcp::serve(),
        command => {
            let result = execute(command, cli.json);
            // A game it started has ended (attached) or none did (an error): a session file it
            // didn't read goes now.
            eclipse_launcher::account::discard_unread_sessions();
            result
        }
    };
    Some(match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    })
}

/// Download progress on one terminal line, only when stderr is a terminal and the percentage moves.
fn progress(label: String) -> impl Fn(f32) {
    let steps = steps();
    move |p| steps(&label, p)
}

/// [`progress`] for work in steps, each saying what it downloads.
fn steps() -> impl Fn(&str, f32) {
    let last = std::cell::RefCell::new((String::new(), -1));
    let tty = std::io::stderr().is_terminal();
    move |label, p| {
        let pct = (p * 100.) as i32;
        {
            let mut last = last.borrow_mut();
            if !tty || (last.0 == label && last.1 == pct) {
                return;
            }
            *last = (label.to_string(), pct);
        }
        eprint!("\r{label} {pct:>3}%");
        let _ = std::io::stderr().flush();
        if p >= 1.0 {
            eprintln!();
        }
    }
}

fn json(value: serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(&value).unwrap());
}

fn attached(cmd: Command) -> Result<(), String> {
    let (mut cmd, _runtime) = backend::prepare_command(cmd)?;
    let status = cmd
        .status()
        .map_err(|e| format!("Could not start {:?}: {e}", cmd.get_program()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Exited with {status}"))
    }
}

fn execute(command: Cmd, as_json: bool) -> Result<(), String> {
    match command {
        Cmd::Completions { shell } => completions(shell),
        Cmd::Versions { installed } => versions::versions(installed, as_json),
        Cmd::Install { tag } => versions::install(tag, as_json),
        Cmd::Remove { tag } => versions::remove(tag),
        Cmd::ImportBuild {
            path,
            version,
            trust_unsigned,
        } => versions::import_build(path, version, trust_unsigned, as_json),
        Cmd::Update { check } => versions::update(check, as_json),
        Cmd::Notes { tag } => versions::notes(tag, as_json),
        Cmd::Prepare {
            minecraft,
            download,
            version,
        } => play::prepare(minecraft, download, version),
        Cmd::Play(args) => play::play(args),
        Cmd::Server {
            dir,
            version,
            accept_eula,
        } => play::server(dir, version, accept_eula),
        Cmd::NativeCache { clear } => native::cache(clear),
        Cmd::NativeBuild(args) => native::build(args),
        Cmd::Links => links(as_json),
        Cmd::Benchmark { repeats } => benchmark(repeats, as_json),
        Cmd::NoAi { state } => agents::no_ai(state, as_json),
        Cmd::AgentConfig { client, test } => agents::agent_config(client, test, as_json),
        Cmd::Settings { key, value } => settings::settings(key, value, as_json),
        Cmd::GameArgs { args } => settings::game_args(args, as_json),
        Cmd::DesktopEntry { remove } => settings::desktop_entry(remove),
        Cmd::ShareLog { instance } => logs::share(instance, as_json),
        Cmd::UnshareLog { shared } => logs::unshare(shared),
        Cmd::Logs { lines, crash } => logs::logs(lines, crash, as_json),
        Cmd::Account { action } => {
            account::account(action.unwrap_or(account::AccountCmd::List), as_json)
        }
        Cmd::Data { action } => data::data(action.unwrap_or(data::DataCmd::Status), as_json),
        Cmd::Worlds { action } => worlds::worlds(
            action.unwrap_or(worlds::WorldsCmd::List { instance: None }),
            as_json,
        ),
        Cmd::Packs { action } => packs::packs(
            action.unwrap_or(packs::PacksCmd::List {
                kind: None,
                instance: None,
            }),
            as_json,
        ),
        Cmd::Screenshots { action } => screenshots::screenshots(
            action.unwrap_or(screenshots::ScreenshotsCmd::List { instance: None }),
            as_json,
        ),
        Cmd::Multiplayer { action } => multiplayer::multiplayer(
            action.unwrap_or(multiplayer::MultiplayerCmd::List { instance: None }),
            as_json,
        ),
        Cmd::Instance { action } => {
            instance::instance(action.unwrap_or(instance::InstanceCmd::List), as_json)
        }
        Cmd::Modpack { action } => {
            modpack::modpack(action.unwrap_or(modpack::ModpackCmd::List), as_json)
        }
        Cmd::Mods { action } => mods::mods(
            action.unwrap_or(mods::ModsCmd::List { instance: None }),
            as_json,
        ),
        Cmd::Servers { action } => {
            servers::servers(action.unwrap_or(servers::ServersCmd::List), as_json)
        }
        Cmd::Rust { action } => rust::rust(action.unwrap_or(rust::RustCmd::Status), as_json),
        Cmd::Java { action } => java::java(action.unwrap_or(java::JavaCmd::List), as_json),
        Cmd::Mcp => unreachable!(),
    }
}

fn completions(shell: clap_complete::Shell) -> Result<(), String> {
    clap_complete::generate(
        shell,
        &mut self::command(),
        "eclipse-launcher",
        &mut std::io::stdout(),
    );
    Ok(())
}

fn links(as_json: bool) -> Result<(), String> {
    let links = backend::links();
    if as_json {
        json(serde_json::to_value(&links).unwrap());
    } else {
        for (name, url) in [
            ("Discord", &links.discord),
            ("Sponsor", &links.sponsor),
            ("Website", &links.website),
        ] {
            println!("{name:<8} {}", if url.is_empty() { "(none)" } else { url });
        }
    }
    Ok(())
}

fn benchmark(repeats: usize, as_json: bool) -> Result<(), String> {
    let path = crate::perf::benchmark(repeats.max(1))?;
    if as_json {
        json(serde_json::json!({ "report": path }));
    } else {
        println!("{}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use eclipse_launcher::native_mods;
    use std::path::Path;

    #[test]
    fn completions_cover_each_shell_and_nested_commands() {
        for shell in [
            clap_complete::Shell::Bash,
            clap_complete::Shell::Zsh,
            clap_complete::Shell::Fish,
            clap_complete::Shell::Elvish,
            clap_complete::Shell::PowerShell,
        ] {
            let shell_name = shell.to_string();
            let args = ["eclipse-launcher", "completions", shell_name.as_str()].map(str::to_owned);
            let cli = Cli::try_parse_from(args).unwrap();
            assert!(matches!(cli.command, Some(Cmd::Completions { .. })));
            let mut output = Vec::new();
            clap_complete::generate(shell, &mut command(), "eclipse-launcher", &mut output);
            let script = String::from_utf8(output).unwrap();
            assert!(script.contains("eclipse-launcher"));
            assert!(script.contains("versions"));
            assert!(script.contains("multiplayer"));
        }
        assert!(Cli::try_parse_from(["eclipse-launcher", "completions"]).is_err());
        assert!(Cli::try_parse_from(["eclipse-launcher", "completions", "unknown"]).is_err());
    }

    #[test]
    fn settings_takes_every_language_and_names_them_all_when_refusing() {
        use crate::i18n::Language;
        let mut settings = backend::Settings::default();
        let refused = settings::set_setting(&mut settings, "language", "xx").unwrap_err();
        let named: Vec<&str> = refused
            .split([',', ' '])
            .filter(|w| !w.is_empty())
            .collect();
        for language in Language::ALL {
            settings::set_setting(&mut settings, "language", language.code()).unwrap();
            assert_eq!(settings.language, Some(language));
            assert!(named.contains(&language.code()), "{refused}");
        }
        settings::set_setting(&mut settings, "language", "system").unwrap();
        assert_eq!(settings.language, None);
    }

    #[test]
    fn play_takes_an_account_for_one_launch() {
        let cli = Cli::try_parse_from(["eclipse-launcher", "play", "--account", "Alex"]).unwrap();
        match cli.command {
            Some(Cmd::Play(play::PlayArgs { account, .. })) => {
                assert_eq!(account.as_deref(), Some("Alex"))
            }
            _ => panic!("not play"),
        }
    }

    #[test]
    fn play_takes_extra_arguments_that_start_with_hyphens() {
        let cli = Cli::try_parse_from([
            "eclipse-launcher",
            "play",
            "--arg",
            "--server-thread",
            "--arg",
            "40",
        ])
        .unwrap();
        match cli.command {
            Some(Cmd::Play(play::PlayArgs { arg, .. })) => {
                assert_eq!(arg, ["--server-thread", "40"])
            }
            _ => panic!("expected play"),
        }
    }

    #[test]
    fn game_args_takes_one_line_that_starts_with_hyphens() {
        let cli = Cli::try_parse_from([
            "eclipse-launcher",
            "game-args",
            "--server-thread --render-distance 40",
        ])
        .unwrap();
        match cli.command {
            Some(Cmd::GameArgs { args }) => {
                assert_eq!(
                    args.as_deref(),
                    Some("--server-thread --render-distance 40")
                );
            }
            _ => panic!("expected game-args"),
        }
    }

    #[test]
    fn mods_commands_name_their_instance() {
        let parse = |args: &[&str]| {
            let cli = Cli::try_parse_from([&["eclipse-launcher", "mods"], args].concat()).unwrap();
            match cli.command {
                Some(Cmd::Mods { action }) => action,
                _ => panic!("expected mods"),
            }
        };
        match parse(&["install", "fresh-animations", "--instance", "Skyblock"]) {
            Some(mods::ModsCmd::Install {
                project, instance, ..
            }) => assert_eq!(
                (project.as_str(), instance.as_deref()),
                ("fresh-animations", Some("Skyblock"))
            ),
            _ => panic!("expected mods install"),
        }
        assert!(matches!(
            parse(&["list"]),
            Some(mods::ModsCmd::List { instance: None })
        ));
        assert!(parse(&[]).is_none());
        match parse(&[
            "add-github",
            "https://github.com/a/b",
            "--mod",
            "zoom",
            "--all",
        ]) {
            Some(mods::ModsCmd::AddGithub {
                url,
                mod_id,
                all,
                trust_native_code,
                ..
            }) => assert_eq!(
                (url.as_str(), mod_id.as_deref(), all, trust_native_code),
                ("https://github.com/a/b", Some("zoom"), true, false)
            ),
            _ => panic!("expected mods add-github"),
        }
        assert!(matches!(
            parse(&["update-github", "--check"]),
            Some(mods::ModsCmd::UpdateGithub {
                check: true,
                id: None,
                ..
            })
        ));
        // Search names one for a Java instance's version and loader; Check reads none: checks
        // are shared.
        assert!(matches!(
            parse(&["search", "x", "--instance", "A"]),
            Some(mods::ModsCmd::Search { instance: Some(i), .. }) if i == "A"
        ));
        assert!(
            Cli::try_parse_from(["eclipse-launcher", "mods", "check", "x", "--instance", "A"])
                .is_err()
        );
    }

    #[test]
    fn native_build_export_mode_needs_a_folder_and_refuses_an_instance() {
        let parse = |args: &[&str]| {
            Cli::try_parse_from(
                [
                    &["eclipse-launcher", "native-build", "--trust-native-code"],
                    args,
                ]
                .concat(),
            )
        };
        // Export mode: a mod folder and an output folder.
        match parse(&["mods", "--server-output", "engine-out"])
            .unwrap()
            .command
        {
            Some(Cmd::NativeBuild(native::BuildArgs {
                folder,
                server_output,
                instance,
                version,
                update,
                profile,
                ..
            })) => {
                assert_eq!(folder.as_deref(), Some(Path::new("mods")));
                assert_eq!(server_output.as_deref(), Some(Path::new("engine-out")));
                assert_eq!(instance, None);
                assert_eq!(version, None);
                assert!(!update);
                assert_eq!(profile, None);
            }
            _ => panic!("expected native-build"),
        }
        // The mod folder is explicit in export mode: no default from an instance.
        assert!(parse(&["--server-output", "engine-out"]).is_err());
        // Export mode never touches an instance.
        assert!(parse(&["mods", "--server-output", "engine-out", "--instance", "A"]).is_err());
        // The options export mode keeps: an explicit version, the profile, mismatch; a version
        // and --update refuse each other, as in the game mode.
        match parse(&[
            "mods",
            "--server-output",
            "engine-out",
            "--version",
            "v2026.10.13",
            "--allow-version-mismatch",
            "--profile",
            "release",
        ])
        .unwrap()
        .command
        {
            Some(Cmd::NativeBuild(native::BuildArgs {
                version,
                update,
                allow_version_mismatch,
                profile,
                ..
            })) => {
                assert_eq!(version.as_deref(), Some("v2026.10.13"));
                assert!(!update);
                assert!(allow_version_mismatch);
                assert_eq!(profile.as_deref(), Some("release"));
            }
            _ => panic!("expected native-build"),
        }
        // --update on its own: the newest installed release or Nightly, never a native build.
        match parse(&["mods", "--server-output", "engine-out", "--update"])
            .unwrap()
            .command
        {
            Some(Cmd::NativeBuild(native::BuildArgs { update, .. })) => assert!(update),
            _ => panic!("expected native-build"),
        }
        // A version refuses --update, as in the game mode.
        assert!(parse(&[
            "mods",
            "--server-output",
            "engine-out",
            "--version",
            "v2026.10.13",
            "--update"
        ])
        .is_err());
        // The game mode is unchanged: an instance build without --server-output.
        assert!(matches!(
            parse(&["--instance", "A"]).unwrap().command,
            Some(Cmd::NativeBuild(native::BuildArgs { instance: Some(i), server_output: None, .. })) if i == "A"
        ));
    }

    #[test]
    fn export_mode_base_is_an_installed_release_not_a_native_build() {
        // The export mode picks the newest installed release or Nightly, never a native build
        // (`native-*`), and `--update` changes nothing when it already is the newest.
        let installed = vec![
            "v2026.10.13".to_string(),
            "v2026.09.01".to_string(),
            "native-2b03c1".to_string(),
        ];
        for update in [false, true] {
            assert_eq!(
                native_mods::base_for(None, &installed, update).as_deref(),
                Some("v2026.10.13")
            );
        }
        assert_eq!(native_mods::base_for(None, &[], false), None);
    }

    #[test]
    fn native_cache_command_and_persisted_limit() {
        for (args, clear) in [
            (vec!["eclipse-launcher", "native-cache"], false),
            (vec!["eclipse-launcher", "native-cache", "--clear"], true),
        ] {
            assert!(matches!(Cli::try_parse_from(args).unwrap().command,
                Some(Cmd::NativeCache { clear: parsed }) if parsed == clear));
        }
        let mut settings = backend::Settings::default();
        settings::set_setting(&mut settings, "native-cache-gib", "12").unwrap();
        let saved = serde_json::to_string(&settings).unwrap();
        let loaded: backend::Settings = serde_json::from_str(&saved).unwrap();
        assert_eq!(loaded.native_cache_gib, Some(12));
        assert!(
            settings::set_setting(&mut settings, "native-cache-gib", "18446744073709551615")
                .is_err()
        );
        assert!(settings::set_setting(&mut settings, "native-cache-gib", "-1").is_err());
        settings::set_setting(&mut settings, "native-cache-gib", "0").unwrap();
        assert_eq!(settings.native_cache_gib, Some(0));
        settings::set_setting(&mut settings, "native-cache-gib", "default").unwrap();
        assert_eq!(settings.native_cache_gib, None);
    }
}
