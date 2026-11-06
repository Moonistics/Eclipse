use super::*;

impl Launcher {
    /// The selected instance's native build (docs/native-mods.md): which build it plays, the
    /// Eclipse it was built on, and whether it holds the native mods turned on (Up to date,
    /// Needs rebuild, Not built), with Build, and Update when a newer Eclipse is installed. Both
    /// open the trust warning first (`native_warning`). A build under way, or its error, shows
    /// only on the instance it is for.
    pub(crate) fn native_build(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        use native_status::State;
        let instance = self.selected_instance()?;
        let version = self.version_of(instance).cloned();
        let home = self.mods.home();
        let key = native_status::fingerprint(&home, version.as_deref());
        let build = version.as_deref().and_then(|v| self.native_build_of(v));
        let mut memo = self.mods.native_state.borrow_mut();
        let (state, pinned) = match memo.as_ref() {
            Some((k, state, pinned)) if *k == key => (*state, pinned.clone()),
            _ => {
                let built = version.as_deref().and_then(native_status::built);
                let state = native_status::state(&native_status::enabled(&home), built.as_deref());
                let pinned = match &build {
                    Some(b) if b.pinned_unknown => native_status::pinning(&home, &b.eclipse),
                    _ => Vec::new(),
                };
                *memo = Some((key, state, pinned.clone()));
                (state, pinned)
            }
        };
        let pinned = match &build {
            Some(b) if !b.pinned_by.is_empty() => b.pinned_by.clone(),
            _ => pinned,
        };
        // Rebuilding on a newer Eclipse, or on the build's own one when mods pinned another
        // commit, builds every mod on that Eclipse's commit (`on_base`).
        let update = build
            .as_ref()
            .filter(|_| state != State::NoMods)
            .and_then(|b| {
                native_mods::newer_than(&b.base, &self.installed).or_else(|| {
                    (!pinned.is_empty() && self.installed.contains(&b.base)).then(|| b.base.clone())
                })
            });
        let mine = self.native.for_instance.as_deref() == Some(instance.id.as_str());
        let step = self.native.step.as_ref().filter(|_| mine);
        let error = self.native.error.as_ref().filter(|_| mine);
        let name = [("instance", instance.name.as_str())];
        let line = |s: String| div().child(s);
        let (status, badge) = match (step, error, state) {
            (Some(step), _, _) => (
                line(format!(
                    "{step} - {}",
                    native_mods::duration_label(
                        self.native.started.map_or(0, |(s, _)| s.elapsed().as_secs())
                    )
                ))
                .text_color(ui::color(ui::look().accent_soft)),
                Some((ui::Tone::Busy, tr("Building"))),
            ),
            (None, Some(e), _) => (ui::error_note(e.clone()), None),
            (None, None, State::NoMods) => (line(tr("No native mod is turned on.").into()), None),
            (None, None, State::NotBuilt) => (
                line(trf(
                    "{instance} doesn't play its native mods yet: build the game with them.",
                    &name,
                )),
                Some((ui::Tone::Warn, tr("Not built"))),
            ),
            (None, None, State::Outdated) => (
                line(trf(
                    "The native mods turned on changed since {instance} was built: build again to play them.",
                    &name,
                )),
                Some((ui::Tone::Warn, tr("Needs rebuild"))),
            ),
            (None, None, State::UpToDate) => (
                line(trf("Up to date: {instance} plays these mods.", &name)),
                Some((ui::Tone::Live, tr("Up to date"))),
            ),
        };
        let mut detail = ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY)
            .flex()
            .flex_col()
            .gap(px(space::XS))
            .child(status);
        if let Some(done) = self.native.done.as_ref().filter(|_| mine && step.is_none()) {
            detail = detail.child(div().text_color(white(ink::MUTED)).child(done.clone()));
        }
        if let (Some(tag), Some(build)) = (&version, &build) {
            let said = match pinned.is_empty() {
                true => trf(
                    "Plays {build}, built on Eclipse {version}.",
                    &[("build", tag), ("version", &build.eclipse())],
                ),
                false => trf(
                    "Plays {build}, built on Eclipse {commit}, the commit pinned by {mods}, not on {version}.",
                    &[
                        ("build", tag),
                        ("commit", &build.eclipse[..build.eclipse.len().min(7)]),
                        ("mods", &pinned.join(", ")),
                        ("version", &build.eclipse()),
                    ],
                ),
            };
            detail = detail.child(div().text_color(white(ink::MUTED)).child(said));
            for left in &build.left_out {
                detail = detail.child(ui::state_note(ui::Tone::Warn, left.label()));
            }
        }
        if self.native.step.is_some() && !mine {
            let other = self
                .native
                .for_instance
                .as_deref()
                .and_then(|id| self.instances.list.iter().find(|i| i.id == id));
            detail = detail.child(div().text_color(white(ink::MUTED)).child(trf(
                "{instance} is building. Wait for it to finish.",
                &[("instance", other.map_or("", |i| i.name.as_str()))],
            )));
        }
        let mismatch = self
            .native
            .step
            .is_none()
            .then_some(self.native.mismatch.as_ref())
            .flatten();
        let idle =
            self.native.step.is_none() && self.native.confirm.is_none() && mismatch.is_none();
        let can_build =
            idle && (error.is_some() || matches!(state, State::NotBuilt | State::Outdated));
        let mut right = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(space::MD))
            .children(badge.map(|(tone, label)| {
                ui::text(step::NOTE, FontWeight::MEDIUM, ink::BODY)
                    .child(ui::state_line(tone, label))
            }));
        if let Some(newer) = update.filter(|_| idle) {
            right = right.child(
                ui::button(
                    "native-update",
                    None,
                    trf(
                        "Rebuild on Eclipse {version}",
                        &[("version", &native_status::eclipse_label(&newer))],
                    ),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.native.update = Some(newer.clone());
                    this.native.on_base = true;
                    this.native.confirm = Some(this.native_mods_to_build().map(|(_, b)| b));
                    cx.notify();
                })),
            );
        }
        if can_build {
            right = right.child(
                ui::button(
                    "native-build",
                    None,
                    tr("Build"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.native.update = None;
                    this.native.on_base = false;
                    this.native.confirm = Some(this.native_mods_to_build().map(|(_, b)| b));
                    cx.notify();
                })),
            );
        }
        let (picker, said) = self.native_profile_picker("native-profile", cx);
        detail = detail.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(space::SM))
                .mt(px(space::SM))
                .child(picker)
                .child(said),
        );
        if let Some(list) = mismatch {
            detail = detail.child(self.native_mismatch_panel(list, cx));
        }
        if let Some(mods) = &self.native.confirm {
            detail = detail.child(self.native_warning(mods, cx));
        }
        Some(
            ui::well()
                .id("native-build")
                .flex()
                .items_start()
                .gap(px(space::LG))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(space::XS))
                        .child(
                            ui::text(step::BODY, FontWeight::SEMIBOLD, ink::FULL)
                                .child(trf("Native build of {instance}", &name)),
                        )
                        .child(detail),
                )
                .child(right),
        )
    }

    /// The warning before a native build: what the mods can do and which mods. The build starts
    /// only from its button.
    pub(super) fn native_warning(
        &self,
        mods: &Result<Vec<native_mods::Bundle>, String>,
        cx: &mut Context<Self>,
    ) -> Div {
        let list = match mods {
            Ok(found) => found
                .iter()
                .map(|b| format!("{} {}", b.id, b.version))
                .collect::<Vec<_>>()
                .join(", "),
            Err(e) => e.clone(),
        };
        let said = div()
            .flex()
            .flex_col()
            .gap(px(space::XS))
            .child(
                ui::text(step::BODY, FontWeight::SEMIBOLD, ink::FULL)
                    .child(tr("These mods will run with full access to your computer")),
            )
            .child(tr("They can read, change or delete your files and use your accounts. Build them only if you trust their authors."))
            .child(trf("Mods: {mods}", &[("mods", &list)]))
            .child(tr("The first build takes 20 to 30 minutes and a few GB of disk. Later ones take a few minutes."))
            .children(self.native.update.as_ref().map(|newer| {
                ui::state_note(
                    ui::Tone::Warn,
                    trf(
                        "This moves the instance to Eclipse {version}. It compiles the game from scratch, so it takes longer than a normal rebuild: up to 30 minutes.",
                        &[("version", &native_status::eclipse_label(newer))],
                    ),
                )
            }));
        let cancel = ui::button(
            "native-cancel",
            None,
            tr("Cancel"),
            ButtonKind::Ghost,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.native.confirm = None;
            this.native.update = None;
            this.native.on_base = false;
            cx.notify();
        }));
        let mut actions = vec![cancel.into_any_element()];
        if mods.is_ok() {
            actions.push(
                ui::button(
                    "native-trust",
                    None,
                    tr("Trust and build"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.build_native(false);
                    cx.notify();
                }))
                .into_any_element(),
            );
        }
        ui::notice(ui::Notice::Warn, "alert-circle", said, actions).mt(px(space::MD))
    }

    /// Mods made for other Eclipse versions: which, and the version the build uses. They often
    /// still work, so the player may build anyway.
    pub(super) fn native_mismatch_panel(&self, list: &str, cx: &mut Context<Self>) -> Div {
        let said = div()
            .flex()
            .flex_col()
            .gap(px(space::XS))
            .child(
                ui::text(step::BODY, FontWeight::SEMIBOLD, ink::FULL)
                    .child(tr("These mods were made for different Eclipse versions")),
            )
            .child(list.to_owned())
            .child(tr("They often still work together. If the build fails, install versions made for the same Nightly."));
        let cancel = ui::button(
            "native-mismatch-cancel",
            None,
            tr("Cancel"),
            ButtonKind::Ghost,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.native.mismatch = None;
            cx.notify();
        }));
        let anyway = ui::button(
            "native-anyway",
            None,
            tr("Build anyway"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.build_native(true);
            cx.notify();
        }));
        ui::notice(
            ui::Notice::Warn,
            "alert-circle",
            said,
            vec![cancel.into_any_element(), anyway.into_any_element()],
        )
        .mt(px(space::MD))
    }
}
