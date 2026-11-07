use super::isolate::LeftOut;
use super::record::Plan;
use super::revision::short;
use super::Profile;
use crate::i18n::{tr, trf};
use crate::rust_toolchain;

/// "1 min 5 s" style time, for the build's status.
pub fn duration_label(secs: u64) -> String {
    if secs >= 60 {
        format!("{} min {} s", secs / 60, secs % 60)
    } else {
        format!("{secs} s")
    }
}

/// Where a build is.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Rust(f32),
    Tool,
    Source(String),
    Mixins,
    /// A previously patched source snapshot was reused.
    CachedMixins,
    /// Which case the build is, before anything runs.
    Plan(Plan, Profile),
    /// Crates done, the total once cargo says it, and the one just started.
    Compile {
        done: usize,
        total: Option<usize>,
        name: String,
        profile: Profile,
    },
    /// The mods' own crates checked against Eclipse's `Cargo.lock`.
    Crates,
    /// A mod's post-build step being built.
    PostBuildCompile(String),
    /// A mod's post-build step running in the new build's folder.
    PostBuild(String),
    Install,
    /// This mod was left out of the build.
    LeftOut(LeftOut),
    /// The build ended: seconds it took.
    Done {
        secs: u64,
        profile: Profile,
        plan: Plan,
    },
}

impl Step {
    pub fn label(&self) -> String {
        match self {
            Self::Rust(done) => trf(
                "Downloading Rust {version} (once): {done}%",
                &[
                    ("version", rust_toolchain::VERSION),
                    ("done", &format!("{:.0}", done * 100.0)),
                ],
            ),
            Self::Tool => tr("Preparing the mod tools (once)").into(),
            Self::Source(revision) => trf(
                "Downloading Eclipse's source at {revision}",
                &[("revision", short(revision))],
            ),
            Self::Mixins => tr("Applying the mixins").into(),
            Self::CachedMixins => tr("Reusing patched game source").into(),
            Self::Crates => tr("Checking the mods' crates against Eclipse's Cargo.lock").into(),
            Self::Plan(plan, profile) => format!("{} ({})", plan.label(), profile.name()),
            Self::Compile {
                done,
                total,
                name,
                profile,
            } => match total {
                Some(total) => trf(
                    "Compiling the game ({profile}): {done} of {total} crates, now {name}",
                    &[
                        ("profile", profile.name()),
                        ("done", &done.to_string()),
                        ("total", &total.to_string()),
                        ("name", name),
                    ],
                ),
                None => trf(
                    "Compiling the game ({profile}): {done} crates, now {name}",
                    &[
                        ("profile", profile.name()),
                        ("done", &done.to_string()),
                        ("name", name),
                    ],
                ),
            },
            Self::PostBuildCompile(id) => trf("Building {name}'s post-build step", &[("name", id)]),
            Self::PostBuild(id) => trf("Running {name}'s post-build step", &[("name", id)]),
            Self::Install => tr("Installing the build").into(),
            Self::LeftOut(left) => left.label(),
            Self::Done {
                secs,
                profile,
                plan,
            } => trf(
                "Built in {duration} ({profile}, {plan})",
                &[
                    ("duration", &duration_label(*secs)),
                    ("profile", profile.name()),
                    ("plan", tr(plan.kind())),
                ],
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_steps_name_the_profile_and_the_outcome() {
        assert_eq!(
            Step::Plan(Plan::UpToDate, Profile::Performance).label(),
            "Up to date (no build needed) (performance)"
        );
        let compile = Step::Compile {
            done: 12,
            total: Some(430),
            name: "serde".into(),
            profile: Profile::Performance,
        };
        assert_eq!(
            compile.label(),
            "Compiling the game (performance): 12 of 430 crates, now serde"
        );
        let done = |secs, profile, plan| Step::Done {
            secs,
            profile,
            plan,
        };
        assert_eq!(
            done(42, Profile::Performance, Plan::Incremental(vec![])).label(),
            "Built in 42 s (performance, incremental)"
        );
        assert_eq!(
            done(125, Profile::Release, Plan::First).label(),
            "Built in 2 min 5 s (release, first build)"
        );
    }
}
