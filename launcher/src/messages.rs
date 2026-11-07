use super::*;
use eclipse_launcher::backend::{Links, Output, Staged};

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Job {
    Prepare,
    Client,
}

pub(super) enum Msg {
    Releases(Result<Vec<Release>, String>),
    Links(Links),
    Progress(String, f32),
    Installed(String, Result<(), String>),
    /// A version's files are gone (or could not be deleted).
    Removed(String, Result<(), String>),
    /// `prune_old_versions` ended; these versions were deleted.
    Pruned(Vec<String>),
    /// What each installed version takes on disk (`disk_use`).
    DiskUse(Vec<(String, u64)>),
    Line(Job, String),
    Exit(Job, Result<(), backend::Ended>),
    Started(game_run::Started),
    /// Boxed: two releases, the largest message by far.
    Updates(Result<Box<Updates>, String>),
    LauncherProgress(f32),
    LauncherUpdated(Result<Option<Staged>, String>),
    Reused,
    /// None when secrets are kept in the OS keychain, else what to say: they last only until
    /// the launcher closes, and why.
    Keychain(Option<String>),
    /// Whether an account in the launcher owned the game at its last check (the owner gate).
    Owner(bool),
    /// Credential-free result from a background account operation.
    AccountStatus(String, u64, account::session::Status),
    AccountProgress(String, u64, account::session::Phase),
    AccountHead(String, Option<String>, Option<Vec<[u8; 4]>>, bool),
    AccountRemoved(String, u64, Result<bool, String>),
    /// A modpack step's line (`modpacks`).
    PackLine(String),
    /// A modpack step ended: the pack as it now is.
    PackDone(Result<Box<eclipse_launcher::modpacks::Imported>, String>),
    /// Where the native-mods build is (`native_mods::Step::label`).
    NativeProgress(String),
    /// The native-mods build ended: the version built, or why not.
    Native(Result<String, native_mods::BuildError>),
    /// An imported game build is installed (`build_import::import`): its version, or why not.
    Imported(Result<String, String>),
}

#[derive(Clone)]
pub(super) struct Tell(Sender<Msg>);

impl Tell {
    pub(super) fn send(&self, msg: Msg) {
        let _ = self.0.send(msg);
    }

    pub(super) fn output(self, job: Job) -> impl Fn(Output) + Send + Sync + 'static {
        move |out| {
            self.send(match out {
                Output::Line(line) => Msg::Line(job, line),
                Output::Exit(result) => Msg::Exit(job, result),
            })
        }
    }
}

impl Launcher {
    pub(super) fn tell(&self) -> Tell {
        Tell(self.tx.clone())
    }

    pub(super) fn in_background(&self, work: impl FnOnce(Tell) + Send + 'static) {
        let tell = self.tell();
        std::thread::spawn(move || work(tell));
    }

    pub(super) fn drain(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        self.perf.tick();
        match self.bench.as_mut().map(|b| b.tick(&self.perf)) {
            Some(perf::Step::Go(page)) => {
                self.go(Page::NAV[page % Page::NAV.len()]);
                changed = true;
            }
            Some(perf::Step::Scroll) => {
                self.go(Page::Versions);
                changed = true;
            }
            Some(perf::Step::Quit) => cx.quit(),
            Some(perf::Step::Wait) | None => {}
        }
        // The running build's clock moves on each second.
        changed |= self.native.tick();
        while let Ok(msg) = self.rx.try_recv() {
            changed = true;
            match msg {
                Msg::Releases(r) => self.releases_arrived(r),
                Msg::Links(links) => {
                    self.multiplayer.list.stale();
                    self.links = links;
                }
                // The page reads is_prepared when drawn; `changed` redraws it.
                Msg::Reused => self.versions.installed = backend::installed(),
                Msg::Keychain(said) => self.accounts_ui.keychain = said,
                Msg::Owner(owns) => self.owner = Some(owns),
                Msg::AccountHead(uuid, url, face, failed) => {
                    self.profile_heads.loaded(&uuid, &url, face, failed)
                }
                Msg::AccountProgress(uuid, generation, phase) => {
                    self.auth_moved(uuid, generation, phase)
                }
                Msg::AccountStatus(uuid, generation, status) => {
                    self.auth_checked(uuid, generation, status)
                }
                Msg::AccountRemoved(uuid, generation, result) => {
                    self.account_removed(uuid, generation, result)
                }
                Msg::Progress(tag, p) => self.install_moved(tag, p),
                Msg::Installed(tag, result) => self.installed(tag, result),
                Msg::Removed(tag, result) => self.removed(tag, result),
                Msg::DiskUse(disk) => self.versions.disk = disk,
                Msg::Imported(result) => self.imported(result),
                Msg::Pruned(gone) => self.pruned(gone),
                Msg::Updates(updates) => self.updates_arrived(updates),
                Msg::NativeProgress(step) => self.native_moved(step),
                Msg::Native(result) => self.native_built(result),
                Msg::LauncherProgress(p) => self.launcher_update_moved(p),
                Msg::LauncherUpdated(result) => self.launcher_updated(result),
                Msg::PackLine(line) => self.modpacks.log.push(line),
                Msg::PackDone(result) => self.pack_done(result),
                // Old versions log their session; it never reaches the window.
                Msg::Line(_, line) if line.contains("Session ID is") => {}
                Msg::Line(job, line) => {
                    if job == Job::Client && line.starts_with(backend::GAME_FIRST_FRAME) {
                        if let Some(pressed) = self.game.take_pressed() {
                            self.perf
                                .record_since("launcher.play_to_game_frame_ms", pressed);
                        }
                    }
                    self.game.log.push(line);
                }
                Msg::Exit(Job::Prepare, result) => self.prepare_ended(result.err()),
                Msg::Exit(Job::Client, result) => self.run_ended(result.err(), cx),
                Msg::Started(started) => self.game_started(started, cx),
            }
        }
        changed |= self.servers_tick();
        changed |= self.mods_tick();
        changed |= self.github_tick();
        changed |= self.native_log_read(cx);
        changed |= self.toast_tick();
        changed |= self.sign_in_moved();
        if self
            .update_check
            .since()
            .is_some_and(|at| at.elapsed() > backend::CHECK_EVERY)
        {
            self.check_updates();
        }
        if changed {
            cx.notify();
        }
    }
}
