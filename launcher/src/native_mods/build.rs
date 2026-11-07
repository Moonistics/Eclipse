use super::compile::{cargo, compile, lock_crates, post_build_programs, profile_of, GAME_BIN};
use super::conflicts::warn_conflicts;
use super::install::{already_exists, copy_out, export, install, made, prune, stale};
use super::isolate::LeftOut;
use super::key::{mod_hash, source_digest, version_for};
use super::manifest::{bundles, repin, Bundle};
use super::mixins::{apply, tool};
use super::record::{about, classify, last_record, save_record, About, BuiltMod, Plan, Record};
use super::revision::{decide, short, Base, Decided, GitHub, Undecided};
use super::run::{append_log, clear_log, log_warning, parse_cargo, run, run_checked, Cargo};
use super::stage::{copy_bundle, source, stage_mixins, stage_trait_data, sync};
use super::step::Step;
use super::target::{architecture_mismatch, platform_for_game, unreadable, BuildPlatform, Target};
use super::{api, crates, dependencies, diagnostics, inject, inject_cache, isolate, Profile};
use crate::backend;
use crate::i18n::{tr, trf};
use crate::instances::{self, Instance};
use crate::native_cache::{self, Dirs};
use crate::rust_toolchain::{self, Toolchain};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Records the whole attempt, including selection or export after compilation.
pub(super) fn logged<T, E: From<String> + std::fmt::Display>(
    cache_limit: Option<u64>,
    build: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    // Another build or a cleanup holds the lease on the same target/: refuse. Any other
    // failure (a filesystem without file locks, a `native` folder linked to another drive)
    // only stops the cleanup: the build goes ahead without the lease, as it did before.
    let (cache, unavailable) = match native_cache::Cache::acquire(&backend::build_dir()) {
        Ok(cache) => (Some(cache), None),
        Err(native_cache::Error::Busy) => {
            return Err(String::from(native_cache::Error::Busy).into())
        }
        Err(error) => (None, Some(error.to_string())),
    };
    clear_log()?;
    if let Some(warning) = &unavailable {
        log_warning(warning);
    }
    let result = build();
    match &result {
        Ok(_) => {
            let _ = append_log("Build completed.");
        }
        Err(error) => {
            let _ = append_log(&format!("The build failed: {error}"));
        }
    }
    // The installed binary and post-build programs have left target/ before trimming. Keep
    // this lease through installation/export, including failed builds, and never wait on Cargo.
    let Some(cache) = cache else {
        return result;
    };
    match cache.trim(cache_limit) {
        Ok(bytes) if bytes > 0 => {
            let _ = append_log(&trf(
                "Cleared {bytes} bytes of native build cache. The next changed build will compile from scratch.",
                &[("bytes", &bytes.to_string())],
            ));
        }
        Err(error) => log_warning(&error.to_string()),
        _ => {}
    }
    result
}

#[derive(Clone, Debug, PartialEq)]
pub enum BuildError {
    /// Mods pin other Eclipse revisions than the one the build would use, and the player hasn't
    /// agreed to try (`anyway`): which, as `id → pin`, and the build's.
    Mismatch(String),
    Failed(String),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mismatch(list) => write!(f, "Made for other Eclipse versions: {list}"),
            Self::Failed(error) => f.write_str(error),
        }
    }
}

impl From<String> for BuildError {
    fn from(error: String) -> Self {
        Self::Failed(error)
    }
}

/// What a build of `folder`'s mods came out as: the version it installed and what it did.
#[derive(Clone, Debug)]
struct Built {
    version: String,
    /// Which case the build was, said before anything ran.
    plan: Plan,
    /// The profile it compiled with: the chosen one, unless the source has no `[profile.fast]`.
    profile: Profile,
    secs: u64,
    /// The mods turned on that it was built without.
    left_out: Vec<LeftOut>,
}

/// Why [`pipeline`] stopped.
enum Failed {
    /// These mods stopped it (`error` says how), while `compiling` or before: it can run again
    /// without them.
    Mods {
        left: Vec<LeftOut>,
        error: String,
        compiling: bool,
    },
    /// Every mod was left out.
    Nothing,
    Stopped(BuildError),
}

impl From<String> for Failed {
    fn from(error: String) -> Self {
        Self::Stopped(BuildError::Failed(error))
    }
}

impl From<Undecided> for Failed {
    fn from(undecided: Undecided) -> Self {
        match undecided {
            Undecided::Mismatch(list) => Self::Stopped(BuildError::Mismatch(format!("{list}."))),
            Undecided::Unfit(error) | Undecided::Failed(error) => error.into(),
        }
    }
}

impl From<Failed> for BuildError {
    fn from(failed: Failed) -> Self {
        match failed {
            Failed::Mods { error, .. } => Self::Failed(error),
            Failed::Nothing => Self::Failed("Every native mod was left out of the build.".into()),
            Failed::Stopped(error) => error,
        }
    }
}

/// What a build was asked for: `folder`'s mods built on `base`, as `target` with `profile`;
/// `anyway` builds mods made for other Eclipse versions together, `on_base` builds every mod on
/// `base`'s own commit.
#[derive(Clone, Copy)]
struct Request<'a> {
    base: &'a str,
    folder: &'a Path,
    anyway: bool,
    on_base: bool,
    profile: Profile,
    target: Target,
}

/// The pipeline both targets build with ([`build`], [`build_server`]): `folder`'s mods on the
/// Eclipse revision they pin (else `base`'s, the installed version whose game files the build
/// reuses: a native build's own revision when it is one, [`base_for`]), compiled as `target`
/// (the game, or the headless engine) and installed as its own version,
/// `native-<revision>-<key>` (the key hashes the revision, the mods' files, the tool's commit,
/// the profile and the target). A build already made is used again. Mods pinning different
/// revisions build on the newest one, with the newest horizon-mixins they name, only when the
/// player agreed to try (`anyway`): else it stops with [`BuildError::Mismatch`].
/// With `on_base`, every mod builds on `base`'s own commit, pinned or not (the player's choice).
/// The mods in `skip` and those needing them are left out; with `isolate`, a mod that alone
/// stops horizon-mixins or the compiler ends it with [`Failed::Mods`], so it can be left out too.
/// Nothing of the player's outside the launcher's data is touched: no instance is edited and
/// no version selected here.
fn pipeline(
    request: Request,
    progress: &impl Fn(Step),
    skip: &[LeftOut],
    isolate: bool,
) -> Result<Built, Failed> {
    let Request {
        base,
        folder,
        anyway,
        on_base,
        profile,
        target,
    } = request;
    let started = std::time::Instant::now();
    backend::need_prepared(base)?;
    let platform = platform_for_game(base)?;
    let dirs = Dirs::current();
    fs::create_dir_all(dirs.root()).map_err(|e| e.to_string())?;
    // The mods are copied first: the build and its key use the copy, whatever happens to the
    // folder meanwhile.
    let incoming = dirs.mods_incoming();
    let _ = fs::remove_dir_all(&incoming);
    let found = bundles(folder)?;
    for warning in warn_conflicts(&found) {
        log_warning(&warning);
    }
    let mut ids = std::collections::HashSet::new();
    for b in &found {
        if !ids.insert(&b.id) {
            return Err(format!("Duplicate native mod id {:?}.", b.id).into());
        }
        copy_bundle(b, &incoming.join(&b.id))?;
    }
    // Read before `repin` rewrites a copy's manifest.
    let digests: HashMap<String, Option<String>> = found
        .iter()
        .map(|b| (b.id.clone(), source_digest(b)))
        .collect();
    let mods: Vec<Bundle> = found
        .into_iter()
        .map(|b| Bundle {
            dir: incoming.join(&b.id),
            ..b
        })
        .collect();
    let left_out = isolate::with_dependents(skip, &mods);
    let mut mods: Vec<Bundle> = mods
        .into_iter()
        .filter(|b| !left_out.iter().any(|l| l.id == b.id))
        .collect();
    if mods.is_empty() {
        return Err(Failed::Nothing);
    }
    let inputs: Vec<_> = mods
        .iter()
        .map(|bundle| dependencies::Mod {
            id: &bundle.id,
            version: &bundle.version,
            has_api: bundle.api.is_some(),
            dependencies: &bundle.dependencies,
        })
        .collect();
    let resolution = dependencies::resolve(&inputs).map_err(|e| e.to_string())?;
    let built_on = about(base);
    let start = Base {
        version: base,
        built: built_on.as_ref(),
        on_base,
        anyway,
    };
    let decided = decide(&mods, &start, &GitHub)?;
    let revision = &decided.revision;
    if let Some(list) = &decided.mismatch {
        log_warning(&format!(
            "Some mods were made for other Eclipse versions ({list}). Building anyway on {}.",
            short(revision)
        ));
    }
    for b in mods
        .iter_mut()
        .filter(|b| b.hooks.is_some() || b.eclipse.as_ref().is_some_and(|r| r != revision))
    {
        repin(b, revision)?;
    }
    let ordered: Vec<Bundle> = resolution.order.iter().map(|&i| mods[i].clone()).collect();
    let version = version_for(
        revision,
        &decided.mixins,
        profile,
        target,
        platform.cache_dir,
        &mods,
    )?;
    let now = Record {
        eclipse: revision.clone(),
        mixins: decided.mixins.clone(),
        mods: mods
            .iter()
            .map(|m| Ok((m.id.clone(), mod_hash(m)?)))
            .collect::<Result<_, String>>()?,
    };
    let exists = backend::is_installed(&target.directory(&version));
    let plan = classify(
        exists,
        last_record(target, profile, platform.cache_dir).as_ref(),
        &now,
    );
    progress(Step::Plan(plan.clone(), profile));
    let attempt = Attempt {
        request,
        origin: start.origin().to_owned(),
        platform,
        dirs,
        decided,
        mods,
        ordered,
        resolution,
        digests,
        left_out,
        isolate,
        progress,
    };
    let mut built = Built {
        version: version.clone(),
        plan: plan.clone(),
        profile,
        secs: 0,
        left_out: attempt.left_out.clone(),
    };
    if !exists {
        built.profile = attempt.make(&version, &now)?;
    } else {
        // Used again: it counts as the newest of the builds kept (`prune`).
        let _ = backend::mark_installed(&target.directory(&version));
    }
    let _ = fs::remove_dir_all(&incoming);
    built.secs = started.elapsed().as_secs();
    Ok(built)
}

struct Attempt<'a> {
    request: Request<'a>,
    origin: String,
    platform: BuildPlatform,
    dirs: Dirs,
    decided: Decided,
    mods: Vec<Bundle>,
    ordered: Vec<Bundle>,
    resolution: dependencies::Resolution,
    digests: HashMap<String, Option<String>>,
    left_out: Vec<LeftOut>,
    isolate: bool,
    progress: &'a dyn Fn(Step),
}

impl Attempt<'_> {
    // Said with a failure: a mod made for another revision may not apply or compile on this one.
    fn blame(&self, e: String) -> String {
        let Decided {
            mismatch,
            pinned_by,
            ..
        } = &self.decided;
        match mismatch {
            Some(list) => {
                let by = match pinned_by.as_slice() {
                    [] => String::new(),
                    [single] => format!(" (pinned by {single})"),
                    many => format!(" (pinned by {})", many.join(", ")),
                };
                format!(
                    "{e}\n\nSome mods were made for other Eclipse versions ({list}{by}), which may be why. Install versions made for the same Nightly."
                )
            }
            None => e,
        }
    }

    fn make(&self, version: &str, now: &Record) -> Result<Profile, Failed> {
        let Request {
            profile, target, ..
        } = self.request;
        let (revision, progress) = (&self.decided.revision, self.progress);
        rust_toolchain::linker(target == Target::Client)?;
        let rust = rust_toolchain::install(|f| progress(Step::Rust(f)))?;
        progress(Step::Source(revision.clone()));
        let source = source(revision)?;
        let (stage, has_api) = self.staged(&rust, &source)?;
        let (target_dir, compiled, game) = self.compiled(&rust, &source, &stage, has_api)?;
        save_record(target, profile, self.platform.cache_dir, now);
        self.installed(&rust, &stage, &target_dir, &game, compiled, version)?;
        Ok(compiled)
    }

    fn staged(&self, rust: &Toolchain, source: &Path) -> Result<(PathBuf, bool), Failed> {
        let Attempt {
            dirs,
            mods,
            ordered,
            resolution,
            progress,
            ..
        } = self;
        let isolate = self.isolate;
        let (revision, mixins, profile) = (
            &self.decided.revision,
            &self.decided.mixins,
            self.request.profile,
        );
        let blame = |e| self.blame(e);
        let needs_mixins = mods.iter().any(|m| m.mixins_content);
        // `dirs` is already the form horizon-mixins and MinGW take: resolved (no link, which
        // it refuses) and, on Windows, ASCII (which they can't read past), both by build_dir.
        let fresh = dirs.stage_new();
        let _ = fs::remove_dir_all(&fresh);
        if needs_mixins {
            let key = inject_cache::key(revision, mixins, profile, ordered)?;
            let snapshot = dirs.patched().join(&key);
            if snapshot.is_dir() {
                progress(Step::CachedMixins);
                sync(&snapshot, &fresh)?;
            } else {
                let injections = inject::prepare(ordered, source)?;
                let tool = tool(rust, mixins, &progress)?;
                let mixin_input = dirs.mixins_incoming();
                stage_mixins(ordered, &mixin_input)?;
                progress(Step::Mixins);
                let result = run_checked(
                    apply(rust, &tool, source, &mixin_input, &fresh),
                    "horizon-mixins",
                    |_| {},
                    None,
                );
                let _ = fs::remove_dir_all(&mixin_input);
                if let Err(ran) = result {
                    let error = blame(ran.error);
                    return Err(match isolate::from_mixins(&ran.tail, ordered) {
                        Some(culprit) if isolate => Failed::Mods {
                            left: vec![culprit],
                            error,
                            compiling: false,
                        },
                        _ => error.into(),
                    });
                }
                inject::stage(&fresh, &injections)?;
                let parent = snapshot.parent().ok_or("invalid cache path".to_owned())?;
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                let partial = parent.join(format!(".{key}.partial"));
                let _ = fs::remove_dir_all(&partial);
                sync(&fresh, &partial)?;
                fs::rename(&partial, &snapshot).map_err(|e| e.to_string())?;
            }
            inject_cache::touch_and_prune(&dirs.patched(), &key)?;
        } else {
            sync(source, &fresh)?;
        }
        stage_trait_data(mods, &fresh)?;
        if mods.iter().any(|m| m.api.is_some()) {
            // Offline metadata resolves all targets, including packages not fetched for this host.
            let mut fetch = cargo(rust, source, "fetch");
            fetch.arg("--locked");
            run(fetch, "Downloading Eclipse's crates", |_| {})?;
        }
        let has_api = api::stage(&fresh, mods, resolution)?;
        if has_api {
            // Add path packages to Cargo.lock before the locked dependency check.
            let mut metadata = cargo(rust, &fresh, "metadata");
            metadata.args(["--offline", "--format-version", "1"]);
            run(metadata, "Resolving trait mod crates", |_| {})?;
        }
        // Absolute: the compiler runs from elsewhere (`compile`).
        let stage = dirs.stage();
        sync(&fresh, &stage)?;
        let _ = fs::remove_dir_all(&fresh);
        if crates::any(&stage) || has_api {
            progress(Step::Crates);
            lock_crates(rust, source, &stage, mods)?;
        }
        Ok((stage, has_api))
    }

    fn compiled(
        &self,
        rust: &Toolchain,
        source: &Path,
        stage: &Path,
        has_api: bool,
    ) -> Result<(PathBuf, Profile, PathBuf), Failed> {
        let Attempt {
            dirs,
            platform,
            mods,
            progress,
            ..
        } = self;
        let isolate = self.isolate;
        let Request {
            profile, target, ..
        } = self.request;
        let blame = |e| self.blame(e);
        let target_dir = dirs.target(platform.cache_dir);
        let compiled = profile_of(stage, profile);
        let game = target_dir.join(compiled.cargo().0).join(backend::CLIENT);
        let (mut crates, mut total, mut name) = (0, None, String::new());
        let identities: Vec<_> = mods
            .iter()
            .map(|m| (m.id.clone(), m.version.clone()))
            .collect();
        run_checked(
            compile(
                rust,
                stage,
                target,
                &target_dir,
                compiled,
                if has_api { api::PACKAGE } else { GAME_BIN },
            ),
            "Compiling the game",
            |line| {
                for event in parse_cargo(line) {
                    match event {
                        Cargo::Compiling(n) => {
                            crates += 1;
                            name = n;
                        }
                        Cargo::Progress(done, all) => {
                            crates = crates.max(done);
                            total = Some(all);
                        }
                    }
                    if !name.is_empty() {
                        progress(Step::Compile {
                            done: crates,
                            total,
                            name: name.clone(),
                            profile: compiled,
                        });
                    }
                }
            },
            Some(diagnostics::Compiler {
                stage,
                source,
                mods: &identities,
            }),
        )
        .map_err(|ran| {
            let error = blame(crates::locked(ran.error));
            match isolate::from_compiler(&ran.owners, mods) {
                left if isolate && !left.is_empty() => Failed::Mods {
                    left,
                    error,
                    compiling: true,
                },
                _ => error.into(),
            }
        })?;
        if let Some(expected) = platform.machine {
            let actual = backend::pe_machine(&game).map_err(|error| unreadable(&game, &error))?;
            if actual != expected {
                return Err(architecture_mismatch(
                    &format!("PE machine 0x{expected:04x}"),
                    &format!("PE machine 0x{actual:04x}"),
                )
                .into());
            }
        }
        Ok((target_dir, compiled, game))
    }

    fn installed(
        &self,
        rust: &Toolchain,
        stage: &Path,
        target_dir: &Path,
        game: &Path,
        compiled: Profile,
        version: &str,
    ) -> Result<(), Failed> {
        let Attempt {
            origin,
            dirs,
            platform,
            mods,
            digests,
            left_out,
            progress,
            ..
        } = self;
        let Request { base, target, .. } = self.request;
        let Decided {
            revision,
            pinned_by,
            base_eclipse,
            on_base,
            mixins,
            ..
        } = &self.decided;
        // Out of `target_dir` before any post-build bin is built there (`post_build_programs`).
        let programs = dirs.programs();
        let binary = programs.join(target.binary());
        copy_out(game, &binary)?;
        let post = post_build_programs(
            rust,
            stage,
            target_dir,
            &programs.join("post-build"),
            compiled,
            progress,
        )?;
        progress(Step::Install);
        let about = About {
            base: origin.clone(),
            eclipse: revision.clone(),
            base_eclipse: base_eclipse.clone(),
            pinned_by: pinned_by.clone(),
            on_base: *on_base,
            horizon_mixins: mixins.clone(),
            hooks: mods.iter().find_map(|m| m.hooks),
            rust: rust_toolchain::VERSION.into(),
            profile: compiled.name().into(),
            target,
            rust_target: platform.target.map(str::to_owned),
            mods: mods
                .iter()
                .map(|m| BuiltMod {
                    id: m.id.clone(),
                    version: m.version.clone(),
                    sha256: digests.get(&m.id).cloned().flatten(),
                })
                .collect(),
            left_out: left_out.clone(),
        };
        let cache = dirs.post_build_cache();
        install(base, version, target, &binary, &about, |dir| {
            for (step, program) in &post {
                progress(Step::PostBuild(step.mod_id.clone()));
                let cmd = crates::post_build(program, step, dir, &cache, stage)?;
                run(cmd, &format!("{}'s post-build step", step.mod_id), |_| {})?;
            }
            Ok(())
        })?;
        let _ = fs::remove_dir_all(&programs);
        Ok(())
    }
}

/// How many times a build compiles again without the mods the compiler's errors were traced to:
/// each costs a compile, where a mod that doesn't apply costs only horizon-mixins' run.
const COMPILE_RETRIES: usize = 3;

/// Runs `pipeline` until it builds, each time without the mods that stopped it before (they
/// are said with `progress` as they are left out, and those needing them once it built). The
/// error, when it can't, starts with the mods already left out.
fn isolated(
    mut pipeline: impl FnMut(&[LeftOut]) -> Result<Built, Failed>,
    progress: &impl Fn(Step),
) -> Result<Built, BuildError> {
    let mut skip: Vec<LeftOut> = Vec::new();
    let mut compiles = 0;
    let mut last = String::new();
    let said = |skip: &[LeftOut], error: String| {
        let labels: Vec<String> = skip.iter().map(LeftOut::label).collect();
        BuildError::Failed(match labels.is_empty() {
            true => error,
            false => format!("{}\n\n{error}", labels.join("\n")),
        })
    };
    loop {
        match pipeline(&skip) {
            Ok(built) => {
                for left in built.left_out.iter().filter(|l| !skip.contains(l)) {
                    log_warning(&left.label());
                    progress(Step::LeftOut(left.clone()));
                }
                return Ok(built);
            }
            Err(Failed::Mods {
                left,
                error,
                compiling,
            }) => {
                compiles += usize::from(compiling);
                let new: Vec<LeftOut> = left
                    .into_iter()
                    .filter(|l| !skip.iter().any(|s| s.id == l.id))
                    .collect();
                if new.is_empty() || compiles > COMPILE_RETRIES {
                    return Err(said(&skip, error));
                }
                for left in new {
                    log_warning(&left.label());
                    progress(Step::LeftOut(left.clone()));
                    skip.push(left);
                }
                last = error;
            }
            Err(Failed::Nothing) => return Err(said(&skip, last)),
            Err(Failed::Stopped(BuildError::Failed(error))) => return Err(said(&skip, error)),
            Err(Failed::Stopped(mismatch)) => return Err(mismatch),
        }
    }
}

/// Builds `folder`'s mods on the Eclipse revision they pin (else `base`'s, the installed version
/// whose game files the build reuses: a native build's own revision when it is one, [`base_for`])
/// and points `instance` at the build, the default one included. Returns the build's version; a
/// build already made is used again. Mods pinning different revisions build on the newest one,
/// with the newest horizon-mixins they name, only when the player agreed to try (`anyway`): else
/// it stops with [`BuildError::Mismatch`]. With `on_base`, every mod builds on `base`'s own
/// commit, pinned or not: the player's Rebuild on Eclipse.
pub fn build(
    base: &str,
    folder: &Path,
    instance: &Instance,
    anyway: bool,
    on_base: bool,
    settings: &backend::Settings,
    progress: impl Fn(Step),
) -> Result<String, BuildError> {
    if let Some(error) = backend::native_mod_architecture_error(settings) {
        return Err(error.into());
    }
    let request = Request {
        base,
        folder,
        anyway,
        on_base,
        profile: Profile::chosen(settings),
        target: Target::Client,
    };
    logged(native_cache::limit_bytes(settings), || {
        let built = isolated(|skip| pipeline(request, &progress, skip, true), &progress)?;
        instances::edit(&instance.id, |meta| {
            meta.version = Some(built.version.clone())
        })?;
        prune(&built.version, settings);
        if built.plan != Plan::UpToDate {
            progress(Step::Done {
                secs: built.secs,
                profile: built.profile,
                plan: built.plan,
            });
        }
        Ok(built.version)
    })
}

/// Builds `folder`'s mods as the headless engine a dedicated server runs (`eclipse-engine`,
/// docs/server.md): the same pipeline as [`build`] on the same revision, compiled with
/// `--no-default-features`, installed as its own version (its key distinct from the game's, so
/// the two never stand in for each other) and exported into `output` as a standalone runtime
/// folder: the built engine, the base version's resources, the mods' post-build data and the
/// build's record. The folder lives outside the launcher's managed versions, where nothing of
/// the launcher's can remove it; nothing of it is overwritten, and a folder already at `output`
/// is refused before anything is downloaded or compiled. No instance is edited and no version
/// selected: a server build changes nothing the launcher's window plays. Returns the absolute
/// path of the exported engine binary.
pub fn build_server(
    base: &str,
    folder: &Path,
    output: &Path,
    anyway: bool,
    on_base: bool,
    settings: &backend::Settings,
    progress: impl Fn(Step),
) -> Result<PathBuf, BuildError> {
    let request = Request {
        base,
        folder,
        anyway,
        on_base,
        profile: Profile::chosen(settings),
        target: Target::Server,
    };
    logged(native_cache::limit_bytes(settings), || {
        if output.symlink_metadata().is_ok() {
            return Err(already_exists(output).into());
        }
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let output = parent.canonicalize().map_err(|e| e.to_string())?.join(
            output
                .file_name()
                .ok_or_else(|| tr("Choose a new folder for the server export.").to_string())?,
        );
        let dirs = Dirs::current();
        for managed in [backend::version_dir(""), dirs.root().to_owned()] {
            if managed
                .canonicalize()
                .is_ok_and(|dir| output.starts_with(dir))
            {
                return Err(String::from(tr(
                    "Export the server outside the launcher's managed folders.",
                ))
                .into());
            }
        }
        // A server plays exactly the mods it was given: none is left out.
        let built = pipeline(request, &progress, &[], false)?;
        export(&Target::Server.directory(&built.version), &output)?;
        let cached = dirs.server_builds();
        let builds = made(
            fs::read_dir(&cached)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path())),
        );
        for old in stale(builds, &built.version, &[]) {
            if let Err(e) = fs::remove_dir_all(cached.join(&old)) {
                log_warning(&format!("{old}: {e}"));
            }
        }
        if built.plan != Plan::UpToDate {
            progress(Step::Done {
                secs: built.secs,
                profile: built.profile,
                plan: built.plan,
            });
        }
        let engine = output.join(Target::Server.binary());
        Ok(std::path::absolute(&engine).unwrap_or(engine))
    })
}

#[cfg(test)]
mod tests {
    use super::super::run::log_path;
    use super::*;
    use crate::native_mods::tests::{dir, write_bundle, LOG_TEST};

    /// The export's own gate: an output that already exists is refused before anything is
    /// downloaded or compiled (a base that isn't installed would refuse the build otherwise),
    /// and no instance is edited or version selected.
    #[test]
    fn a_server_build_refuses_an_existing_output_before_building() {
        let _guard = LOG_TEST.lock().unwrap();
        let output = dir("server-output");
        let folder = dir("server-mods");
        write_bundle(&folder, "hello", "fn hook() {}");
        let instances_before = instances::list();
        let selected_before = backend::Settings::load().default_version;
        let err = build_server(
            "v0.0.0-not-installed",
            &folder,
            &output,
            true,
            false,
            &backend::Settings::default(),
            |_| {},
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("already exists"), "{err}");
        assert!(fs::read_to_string(log_path()).unwrap().contains(&err));
        let managed = Dirs::current().root().join("forbidden-export");
        let err = build_server(
            "v0.0.0-not-installed",
            &folder,
            &managed,
            true,
            false,
            &backend::Settings::default(),
            |_| {},
        )
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("outside the launcher's managed folders"),
            "{err}"
        );
        assert!(!managed.exists());
        assert_eq!(instances::list(), instances_before, "no instance is edited");
        assert_eq!(
            backend::Settings::load().default_version,
            selected_before,
            "no version is selected"
        );
    }

    /// The build log ([`clear_log`], [`append_log`]): fresh for every attempt, and an early
    /// failure (before any command ran) recorded in it, in one test because they share the
    /// one log file.
    #[test]
    fn the_log_is_fresh_per_attempt_and_keeps_early_failures() {
        let _guard = LOG_TEST.lock().unwrap();
        clear_log().unwrap();
        append_log("the last build's output").unwrap();
        clear_log().unwrap();
        assert_eq!(fs::read(log_path()).unwrap(), b"");
        append_log("a line").unwrap();
        append_log("another").unwrap();
        assert_eq!(fs::read_to_string(log_path()).unwrap(), "a line\nanother\n");
        // A build that fails before anything runs empties the old log and says why.
        let folder = dir("log-early");
        write_bundle(&folder, "hello", "fn hook() {}");
        let out = dir("log-early-out");
        let err = build_server(
            "v0.0.0-not-installed",
            &folder,
            &out.join("server"),
            true,
            false,
            &backend::Settings::default(),
            |_| {},
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("is not prepared yet"), "{err}");
        let log = fs::read_to_string(log_path()).unwrap();
        assert!(log.contains("is not prepared yet"), "{log}");
        assert!(!log.contains("the last build's output"), "{log}");
        // A failure after compiler output keeps those diagnostics and the final reason.
        let failure: Result<(), String> = logged(
            native_cache::limit_bytes(&backend::Settings::default()),
            || {
                append_log("error[E0425]: cannot find value `missing` in this scope")?;
                Err("Could not export the engine".into())
            },
        );
        assert!(failure.is_err());
        let log = fs::read_to_string(log_path()).unwrap();
        assert!(log.starts_with("error[E0425]"), "{log}");
        assert!(
            log.ends_with("The build failed: Could not export the engine\n"),
            "{log}"
        );
        assert!(!log.contains("Build completed."), "{log}");
        let success: Result<(), String> = logged(
            native_cache::limit_bytes(&backend::Settings::default()),
            || Ok(()),
        );
        success.unwrap();
        assert_eq!(
            fs::read_to_string(log_path()).unwrap(),
            "Build completed.\n"
        );
    }
}
