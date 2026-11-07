use eclipse_launcher::backend;
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::instances;
use eclipse_launcher::native_mods;
use std::path::PathBuf;

#[derive(clap::Args)]
pub(super) struct BuildArgs {
    /// A mod folder (eclipse-mixin.toml, or an unpacked .hnmod), or a folder of them
    /// (default: the native mods installed and turned on in the instance)
    pub(super) folder: Option<PathBuf>,
    /// Export mode: build a standalone dedicated-server engine into this folder instead of
    /// an instance's build. Needs a mod FOLDER; no instance is read or changed, and no
    /// launcher config is written
    #[arg(
        long = "server-output",
        value_name = "DIR",
        conflicts_with = "instance",
        requires = "folder"
    )]
    pub(super) server_output: Option<PathBuf>,
    /// The instance whose installed native mods to build, and which then plays the build
    /// (default: the selected one)
    #[arg(long)]
    pub(super) instance: Option<String>,
    /// The installed version whose game files the build reuses, and whose commit it builds on
    /// when no mod pins one (default: the one the instance plays, so a rebuild stays on its
    /// Eclipse)
    #[arg(long)]
    pub(super) version: Option<String>,
    /// Move to the newest installed release or Nightly instead of the Eclipse the instance
    /// plays (a full rebuild)
    #[arg(long, conflicts_with = "version")]
    pub(super) update: bool,
    /// You read the mods' source and trust it with full access to this computer
    #[arg(long)]
    pub(super) trust_native_code: bool,
    /// Build mods made for different Eclipse versions together anyway, on the newest of them
    #[arg(long)]
    pub(super) allow_version_mismatch: bool,
    /// Build every mod on the commit of the version the build starts from, even the mods that
    /// pin another Eclipse commit (they may not apply or compile there)
    #[arg(long)]
    pub(super) ignore_pins: bool,
    /// The build profile: performance (the default: incremental, quick to rebuild), dev (quickest
    /// to rebuild a mod, slow game) or release (thin LTO: slowest to build, slightly faster
    /// game). Default: the native-profile setting
    #[arg(long, value_parser = ["performance", "dev", "release"])]
    pub(super) profile: Option<String>,
}

pub(super) fn cache(clear: bool) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(&eclipse_launcher::native_cache::info(clear)?).unwrap()
    );
    Ok(())
}

pub(super) fn build(args: BuildArgs) -> Result<(), String> {
    let BuildArgs {
        folder,
        server_output,
        instance,
        version,
        update,
        trust_native_code,
        allow_version_mismatch,
        ignore_pins,
        profile,
    } = args;
    if !trust_native_code {
        return Err("Native mods run with full access to this computer. Read their source, then pass --trust-native-code.".into());
    }
    let started = std::time::Instant::now();
    let profile = match profile.as_deref().and_then(native_mods::Profile::parse) {
        Some(profile) => profile,
        None => native_mods::Profile::chosen(&backend::Settings::load()),
    };
    let steps = |step: native_mods::Step| {
        let secs = started.elapsed().as_secs();
        eprintln!("[{:>2}:{:02}] {}", secs / 60, secs % 60, step.label())
    };
    if let Some(output) = server_output {
        // Export mode: a standalone server engine in `output`. No instance is read
        // or changed, and no launcher config is written; `folder` is required (clap).
        let folder = folder.unwrap();
        let base = match &version {
            Some(v) => backend::resolve_version(Some(v))?,
            // The instance's version is never looked at here: the newest installed
            // release or Nightly, or that one with `--update` (never a native build).
            None => native_mods::base_for(None, &backend::installed(), update)
                .ok_or_else(backend::no_version_installed)?,
        };
        let built = native_mods::build_server(
            &base,
            &folder,
            &output,
            allow_version_mismatch,
            ignore_pins,
            profile,
            steps,
        )
        .map_err(|e| match e {
            native_mods::BuildError::Mismatch(_) => trf(
                "{error} They often still build: pass --allow-version-mismatch to try.",
                &[("error", &e.to_string())],
            ),
            native_mods::BuildError::Failed(e) => e,
        })?;
        let built = std::path::absolute(&built).unwrap_or(built);
        let path = built.display().to_string();
        println!(
            "{}",
            trf("Built the server engine: {path}", &[("path", &path)])
        );
        println!(
            "{}",
            tr(
                "Point the server at it: set engine.path to this file in the server's \
                    server.toml (docs/hosting.md); the launcher writes nothing itself."
            )
        );
        return Ok(());
    }
    let target = match &instance {
        Some(name) => instances::find(name)?,
        None => instances::selected(),
    };
    let base = match version {
        Some(v) => backend::resolve_version(Some(&v))?,
        None => {
            let plays = target.resolve_version(None).ok();
            native_mods::base_for(plays.as_deref(), &backend::installed(), update)
                .ok_or_else(backend::no_version_installed)?
        }
    };
    let unpacked;
    let folder = match folder {
        Some(folder) => folder,
        None => {
            unpacked = native_mods::installed_mods(&target.home)?;
            unpacked.path().to_owned()
        }
    };
    let built = native_mods::build(
        &base,
        &folder,
        &target,
        allow_version_mismatch,
        ignore_pins,
        profile,
        steps,
    )
    .map_err(|e| match e {
        native_mods::BuildError::Mismatch(_) => {
            format!("{e} They often still build: pass --allow-version-mismatch to try.")
        }
        native_mods::BuildError::Failed(e) => e,
    })?;
    println!(
        "{built} built and selected for its instance ({:.0} s).",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}
