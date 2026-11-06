use super::*;

impl Launcher {
    pub(super) fn drain(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        self.perf.tick();
        match self.bench.as_mut().map(|b| b.tick(&self.perf)) {
            Some(perf::Step::Go(page)) => {
                let pages: Vec<Page> = Page::ALL.iter().map(|p| p.0).collect();
                self.go(pages[page % pages.len()]);
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
        if let Some((started, shown)) = &mut self.native_started {
            if started.elapsed().as_secs() != *shown {
                *shown = started.elapsed().as_secs();
                changed = true;
            }
        }
        while let Ok(msg) = self.rx.try_recv() {
            changed = true;
            match msg {
                Msg::Releases(r) => {
                    if let (Ok(_), Some(checked)) = (&r, self.checked) {
                        self.perf.record_since("launcher.release_list_ms", checked);
                    }
                    self.releases = r.map_or_else(Load::Failed, Load::Ready)
                }
                Msg::Links(links) => {
                    self.multiplayer.stale();
                    self.links = links;
                }
                // The page reads is_prepared when drawn; `changed` redraws it.
                Msg::Reused(_) => self.installed = backend::installed(),
                Msg::Keychain(said) => self.keychain = said,
                Msg::Owner(owns) => self.owner = Some(owns),
                Msg::AccountHead(uuid, url, face, failed) => {
                    self.profile_heads.loaded(&uuid, &url, face, failed)
                }
                Msg::AccountProgress(uuid, generation, phase) => {
                    if self.auth_busy.get(&uuid) == Some(&generation) {
                        self.auth_progress.insert(uuid, phase);
                    }
                }
                Msg::AccountStatus(uuid, generation, status) => {
                    if self.auth_busy.get(&uuid) != Some(&generation) {
                        continue;
                    }
                    self.auth_busy.remove(&uuid);
                    self.auth_progress.remove(&uuid);
                    if self.accounts.find(&uuid).is_some() {
                        if status.state == account::session::State::Ready {
                            self.owner = Some(true);
                        }
                        self.auth_status.insert(uuid, status);
                        // A refresh may have saved a new name or skin: show it now.
                        let saved = Accounts::load();
                        if saved != self.accounts {
                            self.accounts = saved;
                            self.load_profile_heads();
                        }
                    }
                }
                Msg::AccountRemoved(uuid, generation, result) => {
                    if self.auth_busy.get(&uuid) != Some(&generation) {
                        continue;
                    }
                    self.auth_busy.remove(&uuid);
                    self.auth_progress.remove(&uuid);
                    match result {
                        Ok(owns) => {
                            self.accounts = Accounts::load();
                            self.auth_status.remove(&uuid);
                            self.owner = Some(owns);
                        }
                        Err(e) => self.sign_in_state = Some(SignInState::Done(Err(e))),
                    }
                }
                Msg::JavaGame(id, home, cmd) => {
                    self.starting_client = None;
                    let spawned = cmd.and_then(|(cmd, vars)| {
                        self.client_vars = vars;
                        backend::spawn(Job::Client, cmd, self.tx.clone())
                    });
                    match spawned {
                        Ok(r) => {
                            self.client = Some(r);
                            self.client_home = Some(home);
                            self.client_run = Some((id, Instant::now()));
                            self.client_error = account::take_unsaved();
                            self.step_aside(cx);
                        }
                        Err(e) => {
                            if e == account::NoOwner.to_string() {
                                self.owner = Some(false);
                                self.owner_screen_open = true;
                                self.client_error = Some(account::NoOwner::in_window());
                            } else {
                                self.client_error = Some(e);
                            }
                        }
                    }
                }
                Msg::Session(session) => {
                    self.load_auth_statuses(false);
                    if let Some(tag) = self.starting_client.take() {
                        let mut cmd = backend::client_command(&tag);
                        let account::GameSession { file, offline, env } = match session {
                            Ok(pass) => pass,
                            Err(account::NoOwner) => {
                                self.owner = Some(false);
                                self.owner_screen_open = true;
                                self.client_error = Some(account::NoOwner::in_window());
                                self.starting_pack = None;
                                self.joining = None;
                                self.quick_play = None;
                                continue;
                            }
                        };
                        // Past the gate by the developer option alone is no owner.
                        if !self.settings.dev_offline {
                            self.owner = Some(true);
                        }
                        if let Some(file) = file {
                            cmd.arg("--session-file").arg(file);
                        }
                        cmd.envs(env);
                        // Still play, offline, and say why.
                        if let Some(e) = offline {
                            self.client_error = Some(format!("Playing offline: {e}"));
                        } else if let Some(e) = account::take_unsaved() {
                            self.client_error = Some(e);
                        }
                        self.accounts = Accounts::load();
                        // An offline name the player picked (the quick switch).
                        if let Some(name) = self.accounts.name_arg() {
                            cmd.arg("--name").arg(name);
                        }
                        cmd.envs(self.accounts.look_env());
                        // A pack's Play button plays that pack; Play plays the selected instance.
                        let (profile, run) = match self.starting_pack.take() {
                            Some(work) => {
                                let id = instances::list()
                                    .into_iter()
                                    .find(|i| i.work == work)
                                    .map(|i| i.id);
                                let applied =
                                    eclipse_launcher::modpacks::apply_profile(&mut cmd, &work);
                                (applied, id)
                            }
                            None => {
                                let instance = instances::selected();
                                (instances::apply(&mut cmd, &instance), Some(instance.id))
                            }
                        };
                        if let Err(e) = profile {
                            self.client_error = Some(e);
                            account::discard_unread_sessions();
                            continue;
                        }
                        if let Some(code) = self.joining.take() {
                            if let Err(e) = backend::join_friend(&mut cmd, &code) {
                                self.client_error = Some(e);
                            }
                        }
                        if let Some((flag, value)) = self.quick_play.take() {
                            cmd.arg(flag).arg(value);
                        }
                        backend::append_game_args(&mut cmd);
                        // Last, around every argument: the instance's pre-launch command,
                        // wrapper and variables.
                        let instance = run
                            .as_ref()
                            .and_then(|id| instances::list().into_iter().find(|i| &i.id == id));
                        self.client_vars = instance
                            .as_ref()
                            .map(|i| i.vars(cmd.get_program()))
                            .unwrap_or_default();
                        cmd.envs(self.client_vars.iter().map(|(k, v)| (k, v)));
                        let launch = instance.map(|i| i.launch).unwrap_or_default();
                        let cmd = eclipse_launcher::launch::wrap(cmd, &launch);
                        self.client_home = cmd.get_current_dir().map(|d| d.to_path_buf());
                        match backend::spawn(Job::Client, cmd, self.tx.clone()) {
                            Ok(r) => {
                                self.client = Some(r);
                                self.client_run = run.map(|id| (id, Instant::now()));
                                self.step_aside(cx);
                            }
                            Err(e) => {
                                self.client_error = Some(e);
                                account::discard_unread_sessions();
                            }
                        }
                    } else {
                        // The start was called off: the sessions written for it go.
                        account::discard_unread_sessions();
                    }
                }
                Msg::Progress(tag, p) => {
                    self.bar = (self.bar.1, p);
                    self.installing = Some((tag, p));
                }
                Msg::Installed(tag, result) => {
                    self.installing = None;
                    self.bar = (0., 0.);
                    self.installed = backend::installed();
                    let as_update = self.update_install.take().filter(|(t, _)| *t == tag);
                    let for_instance = self.instance_install.take_if(|t| *t == tag).is_some();
                    if let (Ok(()), Some(started)) = (&result, self.install_started.take()) {
                        let name = if as_update.is_some() {
                            "launcher.update_ms"
                        } else {
                            "launcher.install_ms"
                        };
                        self.perf.record_since(name, started);
                    }
                    match result {
                        Ok(()) => {
                            if self.updates.eclipse.as_ref().is_some_and(|r| r.tag == tag) {
                                self.updates.eclipse = None;
                            }
                            // An instance's own version leaves the other instances on theirs.
                            let select = as_update.is_none_or(|(_, select)| select)
                                && (!for_instance || self.launcher_version().is_none());
                            if select {
                                self.settings.selected = Some(tag);
                            }
                            // Pruning reads settings from disk: protect the selection only once
                            // it has been saved there. A failed save leaves all versions in place.
                            if self.save_settings() {
                                let tx = self.tx.clone();
                                std::thread::spawn(move || {
                                    let _ = tx.send(Msg::Pruned(backend::prune_old_versions()));
                                });
                            }
                        }
                        Err(e) => self.install_error = Some((tag, e)),
                    }
                    if let Some(next) = self.install_next.take() {
                        self.install_for_instance(next);
                    }
                }
                Msg::Removed(tag, result) => {
                    self.removing.retain(|t| *t != tag);
                    self.installed = backend::installed();
                    let selected = if self.settings_save_error.is_some() {
                        self.settings.selected.take()
                    } else {
                        Settings::load().selected
                    };
                    self.settings.selected = selection_after_removal(&self.installed, selected);
                    if let Err(e) = result {
                        self.install_error = Some((tag, e));
                    }
                }
                Msg::DiskUse(disk) => self.disk = disk,
                Msg::Imported(result) => {
                    self.importing_build = false;
                    self.installed = backend::installed();
                    match result {
                        // `build_import::import` saved the selection; this keeps the window's copy.
                        Ok(tag) => self.settings.selected = Some(tag),
                        Err(e) => self.import_error = Some(e),
                    }
                }
                Msg::Pruned(gone) => {
                    if !gone.is_empty() {
                        self.installed = backend::installed();
                    }
                }
                Msg::Updates(Ok(updates)) => {
                    self.update_error = None;
                    self.updates = *updates;
                    if self.settings.auto_install() {
                        self.install_update();
                    }
                    if let Some(release) = self.updates.launcher.clone() {
                        self.update_launcher(release);
                    }
                }
                Msg::Updates(Err(e)) => self.update_error = Some(e),
                Msg::NativeProgress(step) => self.native_step = Some(step),
                Msg::Native(result) => {
                    // The last step of a build that worked says how it went.
                    let last = self.native_step.take();
                    self.native_started = None;
                    self.native_done = result.is_ok().then_some(last).flatten();
                    self.installed = backend::installed();
                    self.reload_instances();
                    // A build attempt ended: an open panel reads the log it left.
                    self.native_log.stale = true;
                    match result.map_err(|e| match e.strip_prefix(native_mods::MISMATCH) {
                        Some(list) => Ok(list.to_owned()),
                        None => Err(e),
                    }) {
                        Err(Ok(list)) => self.native_mismatch = Some(list),
                        Err(Err(e)) => self.native_error = Some(e),
                        // Play waited for this build (`Mods::rebuild`).
                        Ok(_) if std::mem::take(&mut self.mods.play_after_build) => self.play(),
                        Ok(_) => {}
                    }
                    self.mods.play_after_build = false;
                }
                Msg::LauncherProgress(p) => {
                    if let SelfUpdate::Downloading(_, f) = &mut self.self_update {
                        *f = p;
                    }
                }
                Msg::LauncherUpdated(result) => {
                    let version = match &self.self_update {
                        SelfUpdate::Downloading(v, _) => v.clone(),
                        _ => String::new(),
                    };
                    self.self_update = match result {
                        Ok(staged) => {
                            eclipse_launcher::update_handoff::set_pending(staged);
                            SelfUpdate::Ready(version)
                        }
                        Err(e) => SelfUpdate::Failed(e),
                    };
                }
                Msg::PackLine(line) => {
                    self.pack_log.push_back(line);
                    if self.pack_log.len() > LOG_LINES {
                        self.pack_log.pop_front();
                    }
                }
                Msg::PackDone(result) => self.pack_done(result),
                // Old versions log their session; it never reaches the window.
                Msg::Line(_, line) if line.contains("Session ID is") => {}
                Msg::Line(job, line) => {
                    if job == Job::Client && line.starts_with(perf::GAME_FIRST_FRAME) {
                        if let Some(pressed) = self.play_pressed.take() {
                            self.perf
                                .record_since("launcher.play_to_game_frame_ms", pressed);
                        }
                    }
                    // The MCP server keeps its own server's log; the launcher's are the preparer's
                    // and the game's.
                    if job != Job::Server {
                        self.prepare_log.push_back(line);
                        if self.prepare_log.len() > LOG_LINES {
                            self.prepare_log.pop_front();
                        }
                    }
                }
                Msg::Exit(job, result) => {
                    let error = result.err();
                    match job {
                        Job::Prepare => {
                            self.preparing = false;
                            // Play pressed before the version was prepared plays now.
                            if let Some(tag) = self.play_when_prepared.take() {
                                if error.is_none() && backend::is_prepared(&tag) {
                                    self.starting_pack = None;
                                    self.start_client(tag);
                                }
                            }
                            if let (None, Some(started)) = (&error, self.prepare_started.take()) {
                                self.perf.record_since("launcher.prepare_ms", started);
                            }
                            // A preparer process's exit status says little, so its last line
                            // follows; the launcher's own errors are full sentences already.
                            self.prepare_error = error.map(|e| {
                                let tail = self
                                    .prepare_log
                                    .iter()
                                    .rev()
                                    .find(|l| !l.trim().is_empty())
                                    .filter(|_| e.starts_with("Exited with"));
                                tail.map_or(e.clone(), |t| format!("{e}: {t}"))
                            });
                        }
                        Job::Client => {
                            self.client = None;
                            // Any session file the game didn't read (it crashed first).
                            account::discard_unread_sessions();
                            self.come_back(cx);
                            // The game saved: last played, sizes and pictures changed.
                            self.worlds.stale();
                            self.multiplayer.stale();
                            self.screenshots.stale();
                            self.game_packs.stale();
                            let vars = std::mem::take(&mut self.client_vars);
                            if let Some((id, started)) = self.client_run.take() {
                                if let Err(e) = instances::record_play(&id, started.elapsed()) {
                                    eprintln!("Play time not saved: {e}");
                                }
                                if let Some(i) = instances::list().into_iter().find(|i| i.id == id)
                                {
                                    eclipse_launcher::launch::run_after_exit(
                                        &i.launch, &i.home, &vars,
                                    );
                                }
                                self.reload_instances();
                            }
                            // The run's own game folder: an instance's or a pack's keeps its reports.
                            let home = self.client_home.clone().unwrap_or_else(backend::game_home);
                            let report = self
                                .client_started
                                .take()
                                .and_then(|since| backend::crash_report_since(&home, since));
                            let switched = if error.is_some() && report.is_none() {
                                self.switched_from_vulkan()
                            } else {
                                None
                            };
                            if let Some(api) = switched {
                                self.client_error = Some(trf(
                                    "Eclipse's Vulkan renderer crashed while starting on this \
                                     graphics card, so the game now uses {api}. Press Play to \
                                     start it again. Video Settings > Graphics API switches back.",
                                    &[("api", api)],
                                ));
                            } else if error.is_some() || report.is_some() {
                                let summary = report
                                    .as_ref()
                                    .and_then(|p| std::fs::read_to_string(p).ok())
                                    .and_then(|text| backend::crash_summary(&text))
                                    .or_else(|| {
                                        let tail = self
                                            .prepare_log
                                            .iter()
                                            .rev()
                                            .find(|l| !l.trim().is_empty());
                                        error.as_ref().map(|e| {
                                            tail.map_or(e.clone(), |t| format!("{e}: {t}"))
                                        })
                                    })
                                    .unwrap_or_default();
                                self.crash = Some(Crash {
                                    report,
                                    summary,
                                    copied: false,
                                });
                            }
                        }
                        Job::Server => {}
                    }
                }
            }
        }
        changed |= self.servers_tick();
        changed |= self.mods_tick();
        changed |= self.github_tick();
        changed |= self.native_log_read(cx);
        changed |= self.toast_tick();
        if let Some(sign_in) = &self.sign_in {
            if let Some(state) = sign_in_change(&mut self.sign_in_state, sign_in.state()) {
                if let SignInState::Done(Ok(ref account)) = state {
                    self.auth_busy.remove(&account.uuid);
                    self.sign_in_target = None;
                    self.accounts = Accounts::load();
                    self.load_profile_heads();
                    self.load_auth_statuses(false);
                    // Sign-in refuses accounts that don't own the game.
                    self.owner = Some(true);
                    self.owner_screen_open = false;
                    self.client_error = None;
                    if !self.settings.owner_notice_seen {
                        self.settings.owner_notice_seen = true;
                        self.save_settings();
                    }
                }
                changed = true;
            }
        }
        if self
            .checked
            .is_some_and(|at| at.elapsed() > backend::CHECK_EVERY)
        {
            self.check_updates();
        }
        if changed {
            cx.notify();
        }
    }
}
