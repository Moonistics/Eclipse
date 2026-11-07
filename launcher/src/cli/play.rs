use super::attached;
use eclipse_launcher::account::{self, Accounts};
use eclipse_launcher::backend;
use eclipse_launcher::instances;
use std::path::PathBuf;

#[derive(clap::Args)]
pub(super) struct PlayArgs {
    #[arg(long)]
    pub(super) version: Option<String>,
    /// The instance to play (its name or id; default: the selected one). A Java instance
    /// starts its Minecraft: Java Edition, downloaded with its Java the first time
    #[arg(long)]
    pub(super) instance: Option<String>,
    /// Open or create this world (a folder under saves) at once
    #[arg(long)]
    pub(super) world: Option<String>,
    /// Creative mode for a new world
    #[arg(long)]
    pub(super) creative: bool,
    /// Join a friend's world with its invite code
    #[arg(long)]
    pub(super) join: Option<String>,
    /// Join this server at once: an address, or a server of the list by name or position
    #[arg(long, conflicts_with_all = ["world", "join"])]
    pub(super) server: Option<String>,
    /// An extra argument for the game, passed through as-is (repeatable), for example
    /// --server-thread
    #[arg(long = "arg", value_name = "ARG", allow_hyphen_values = true)]
    pub(super) arg: Vec<String>,
    /// Play this launch as this signed-in account (its name or UUID), without changing
    /// the one `account select` picked: several games can play as several accounts at once
    #[arg(long, value_name = "WHO")]
    pub(super) account: Option<String>,
}

pub(super) fn prepare(
    minecraft: Option<PathBuf>,
    download: bool,
    version: Option<String>,
) -> Result<(), String> {
    let tag = backend::resolve_version(version.as_deref())?;
    let minecraft = backend::minecraft_dir(minecraft)?;
    let from = if download {
        backend::MinecraftFrom::Mojang
    } else {
        backend::MinecraftFrom::Install
    };
    let (preparer, minecraft) =
        backend::preparer(&tag, &minecraft, from, &|line| eprintln!("{line}"))?;
    backend::sounds(&tag, &minecraft, &|line| println!("{line}"));
    match preparer {
        backend::Preparer::Process(cmd) => attached(cmd)?,
        backend::Preparer::InProcess(options) => {
            backend::prepare_in_process(&options, &|line| println!("{line}"))?
        }
    }
    println!("{tag} is prepared.");
    Ok(())
}

pub(super) fn play(args: PlayArgs) -> Result<(), String> {
    let PlayArgs {
        version,
        instance,
        world,
        creative,
        join,
        server,
        arg,
        account,
    } = args;
    let instance = match instance {
        Some(name) => instances::find(&name)?,
        None => instances::selected(),
    };
    if instance.java.is_some()
        && (version.is_some() || world.is_some() || join.is_some() || creative || account.is_some())
    {
        return Err(format!(
            "{} plays Minecraft: Java Edition: --version, --world, --creative, \
                 --join and --account are for Eclipse instances.",
            instance.name
        ));
    }
    let who = match account {
        Some(who) => Some(account::Player::Microsoft(
            Accounts::load()
                .find(&who)
                .ok_or_else(|| {
                    format!("No signed-in account {who}. See `eclipse-launcher account`.")
                })?
                .uuid
                .clone(),
        )),
        None => None,
    };
    launch(backend::Play {
        version,
        who,
        join,
        world,
        server,
        creative,
        args: arg,
        ..backend::Play::of(instance)
    })?;
    Ok(())
}

pub(super) fn server(
    dir: Option<PathBuf>,
    version: Option<String>,
    accept_eula: bool,
) -> Result<(), String> {
    let tag = backend::resolve_version(version.as_deref())?;
    let dir = backend::server_dir(dir);
    // The server inherits this terminal, so typed lines reach its console.
    attached(backend::server(&tag, &dir, accept_eula)?)?;
    Ok(())
}

pub(super) fn launch(request: backend::Play) -> Result<(), String> {
    let instance = request.instance.clone();
    let launched = backend::launch(request, &|line| eprintln!("{line}")).map_err(|e| e.hinted())?;
    for notice in launched.notices() {
        eprintln!("{}", notice.said());
    }
    let started = std::time::Instant::now();
    let played = attached(launched.cmd);
    if let Err(e) = instances::record_play(&instance.id, started.elapsed()) {
        eprintln!("Play time not saved: {e}");
    }
    after_exit(&instance, &launched.vars);
    played
}

fn after_exit(instance: &instances::Instance, vars: &[(&'static str, std::ffi::OsString)]) {
    let Some(mut cmd) =
        eclipse_launcher::launch::after_exit(&instance.launch, &instance.home, vars)
    else {
        return;
    };
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("The post-exit command failed ({s})."),
        Err(e) => eprintln!("The post-exit command didn't run: {e}"),
    }
}
