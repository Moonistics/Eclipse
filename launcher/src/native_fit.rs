//! Whether a native mod fits the Eclipse the player plays, said before it is installed (#9860),
//! so a mod made for another Eclipse is no surprise at build time. A mod naming a hook points'
//! version fits any Eclipse with that `NATIVE_HOOKS_API` or a later one; a pinned mod fits the
//! commit it pins (docs/native-mods.md, "Pinned to a Nightly").
use crate::backend;
use crate::i18n::{tr, trf};
use crate::instances::Instance;
use crate::native_mods::{self, Base, Bundle, HookCatalog, Lookup, Pinned, Undecided};
use eclipse_mods::index::BrokenOn;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The Eclipse the selected instance plays: the commit its game was compiled from and that
/// commit's hook points' version (None when it has none, or it couldn't be read).
#[derive(Clone, Debug, PartialEq)]
pub struct Plays {
    pub revision: String,
    pub hooks: Option<u32>,
}

/// How a native mod fits [`Plays`].
#[derive(Clone, Debug, PartialEq)]
pub enum Fit {
    /// It builds on the Eclipse the player plays.
    Fits,
    /// It needs hook points the player's Eclipse doesn't have yet.
    NeedsNewer { needs: u32, has: u32 },
    /// It pins another commit, and mods turned on pin yet another one: the build asks for
    /// Build anyway, and the mods made for the other commit may not compile.
    Clashes { pin: String, with: Vec<String> },
    /// It pins another commit, alone: the build compiles the game on that commit.
    MovesTo { pin: String },
    /// The registry found it doesn't apply on the Eclipse the player plays (horizon-mixins'
    /// `code`): a build leaves it out.
    Broken { code: String },
}

/// How mod `id` fits: it names hook points' version `hooks`, else pins commit `pin`. `pins` are
/// the mods turned on that pin a commit (id, commit); the mod's own older version among them is
/// replaced by this one. `broken_on` are the commits the registry found it doesn't apply on: one
/// it would build on makes it [`Fit::Broken`]. None when the hook points aren't known.
pub fn fit(
    id: &str,
    hooks: Option<u32>,
    pin: &str,
    plays: &Plays,
    pins: &[(String, String)],
    broken_on: &[BrokenOn],
    hooks_at: impl Fn(&str) -> Result<Option<u32>, String>,
) -> Option<Fit> {
    let alone = [Bundle::listed(id, hooks, pin)];
    let base = Base {
        version: &plays.revision,
        built: None,
        on_base: false,
        anyway: true,
    };
    let builds_on = match native_mods::decide(&alone, &base, &Played { plays, hooks_at }) {
        Ok(decided) => decided.revision,
        Err(Undecided::Unfit(_)) => {
            return Some(Fit::NeedsNewer {
                needs: hooks?,
                has: plays.hooks?,
            })
        }
        Err(_) => return None,
    };
    if let Some(broken) = broken_on.iter().find(|b| b.eclipse == builds_on) {
        return Some(Fit::Broken {
            code: broken.code.clone(),
        });
    }
    if builds_on == plays.revision {
        return Some(Fit::Fits);
    }
    if hooks.is_some() {
        return Some(Fit::MovesTo { pin: builds_on });
    }
    let with: Vec<String> = pins
        .iter()
        .filter(|(other, revision)| other != id && revision != pin)
        .map(|(other, _)| other.clone())
        .collect();
    Some(match with.is_empty() {
        true => Fit::MovesTo { pin: pin.into() },
        false => Fit::Clashes {
            pin: pin.into(),
            with,
        },
    })
}

struct Played<'a, H> {
    plays: &'a Plays,
    hooks_at: H,
}

const UNKNOWN: &str = "Not known before a build.";

impl<H: Fn(&str) -> Result<Option<u32>, String>> Lookup for Played<'_, H> {
    fn commit_of(&self, version: &str) -> Result<String, String> {
        Ok(version.to_owned())
    }

    fn date(&self, _: Pinned, _: &str) -> Result<String, String> {
        Err(UNKNOWN.into())
    }

    fn hooks(&self, revision: &str) -> Result<Option<u32>, String> {
        match revision == self.plays.revision {
            true => self.plays.hooks.map(Some).ok_or_else(|| UNKNOWN.into()),
            false => (self.hooks_at)(revision),
        }
    }

    fn catalog(&self, _: &str) -> Result<Option<HookCatalog>, String> {
        Err(UNKNOWN.into())
    }
}

pub fn describe(fit: &Fit, plays: &Plays) -> String {
    match fit {
        Fit::Fits => tr("Builds on the Eclipse you play").to_owned(),
        Fit::NeedsNewer { needs, has } => trf(
            "Needs native hook points v{needs}, the Eclipse you play has v{has}: update Eclipse first",
            &[("needs", &needs.to_string()), ("has", &has.to_string())],
        ),
        Fit::Clashes { pin, with } => trf(
            "Made for Eclipse {pin}, but {mods} pin another commit: the build will ask you to Build anyway and may not compile",
            &[("pin", native_mods::short(pin)), ("mods", &with.join(", "))],
        ),
        Fit::MovesTo { pin } => trf(
            "Made for Eclipse {pin}, you play {yours}: the launcher builds your game on {pin}",
            &[
                ("pin", native_mods::short(pin)),
                ("yours", native_mods::short(&plays.revision)),
            ],
        ),
        Fit::Broken { code } => trf(
            "Doesn't build on the Eclipse you play ({code}): a build leaves it out until its author updates it",
            &[("code", code)],
        ),
    }
}

/// [`Plays`] by the version an instance plays (kept for good once GitHub answered, a failure or
/// nothing installed for [`RETRY`]), and hook points' versions by commit: reading them asks
/// GitHub, and a search asks for every native mod it finds.
type CacheKey = (String, Option<String>);
type Known = HashMap<CacheKey, Read>;
static PLAYS: Mutex<Option<Known>> = Mutex::new(None);
static HOOKS: Mutex<Option<HashMap<String, Option<u32>>>> = Mutex::new(None);
const RETRY: Duration = Duration::from_secs(60);

#[derive(Clone)]
struct Read {
    at: Instant,
    plays: Option<Plays>,
    settled: bool,
}

fn cached(cache: &Known, key: &CacheKey) -> Option<Option<Plays>> {
    cache
        .get(key)
        .filter(|read| read.settled || read.at.elapsed() < RETRY)
        .map(|read| read.plays.clone())
}

/// The Eclipse `instance` plays; None for a Java instance, when nothing is installed, or when
/// GitHub can't say its commit.
pub fn plays(instance: &Instance, settings: &backend::Settings) -> Option<Plays> {
    if instance.java.is_some() {
        return None;
    }
    let version = instance.resolve_version(None, settings).ok()?;
    // Native builds can be rebuilt under the same version name on a different commit.
    // Read the build record before consulting the cache so the cached answer follows it.
    let native_revision = native_mods::about(&version).map(|about| about.eclipse);
    let key = (version.clone(), native_revision.clone());
    let known = cached(PLAYS.lock().unwrap().get_or_insert_default(), &key);
    if let Some(plays) = known {
        return plays;
    }
    // A native build is the commit it compiled; anything else, the commit of the release or
    // Nightly a build would start from.
    let revision = match native_revision {
        Some(revision) => Some(revision),
        None => native_mods::base_for(Some(&version), &backend::installed(), false)
            .and_then(|base| native_mods::revision_of(&base).ok()),
    };
    let read = match revision {
        Some(revision) => {
            let hooks = hooks_at(&revision);
            Read {
                at: Instant::now(),
                settled: hooks.is_ok(),
                plays: Some(Plays {
                    hooks: hooks.ok().flatten(),
                    revision,
                }),
            }
        }
        None => Read {
            at: Instant::now(),
            plays: None,
            settled: false,
        },
    };
    let plays = read.plays.clone();
    PLAYS
        .lock()
        .unwrap()
        .get_or_insert_default()
        .insert(key, read);
    plays
}

fn hooks_at(revision: &str) -> Result<Option<u32>, String> {
    if let Some(&hooks) = HOOKS.lock().unwrap().get_or_insert_default().get(revision) {
        return Ok(hooks);
    }
    let hooks = native_mods::hooks_at(revision)?;
    HOOKS
        .lock()
        .unwrap()
        .get_or_insert_default()
        .insert(revision.into(), hooks);
    Ok(hooks)
}

/// The native mods turned on in game folder `home` that pin a commit, as (id, commit).
pub fn pins(home: &Path) -> Vec<(String, String)> {
    crate::native_status::bundles_on(home)
        .into_iter()
        .filter_map(|b| {
            let pin = native_mods::pin(b.manifest.hooks, Some(&b.manifest.eclipse))?.to_owned();
            Some((b.manifest.id, pin))
        })
        .collect()
}

/// The line the registry browser shows for a native mod's version naming `hooks`, or pinning
/// `pin`, in `instance`; None when it can't be told.
pub fn line(
    instance: &Instance,
    settings: &backend::Settings,
    id: &str,
    hooks: Option<u32>,
    pin: &str,
    broken_on: &[BrokenOn],
) -> Option<String> {
    let plays = plays(instance, settings)?;
    fit(
        id,
        hooks,
        pin,
        &plays,
        &pins(&instance.home),
        broken_on,
        hooks_at,
    )
    .map(|fit| describe(&fit, &plays))
}

#[cfg(test)]
mod tests {
    use super::*;

    const YOURS: &str = "1111111111111111111111111111111111111111";
    const OTHER: &str = "2222222222222222222222222222222222222222";
    const THIRD: &str = "3333333333333333333333333333333333333333";

    fn plays(hooks: Option<u32>) -> Plays {
        Plays {
            revision: YOURS.into(),
            hooks,
        }
    }

    fn pin(id: &str, revision: &str) -> (String, String) {
        (id.into(), revision.into())
    }

    fn unasked(revision: &str) -> Result<Option<u32>, String> {
        panic!("asked the hook points of {revision}")
    }

    #[test]
    fn rebuilding_a_native_version_does_not_reuse_its_previous_compatibility() {
        let version = "native-example".to_string();
        let old_key = (version.clone(), Some(YOURS.to_string()));
        let rebuilt_key = (version.clone(), Some(OTHER.to_string()));
        let unbuilt_key = (version, None);
        let old = plays(Some(28));
        let mut cache = Known::new();
        cache.insert(
            old_key.clone(),
            Read {
                at: Instant::now(),
                plays: Some(old.clone()),
                settled: true,
            },
        );
        assert_eq!(cached(&cache, &old_key), Some(Some(old)));
        assert_eq!(cached(&cache, &rebuilt_key), None);
        assert_eq!(cached(&cache, &unbuilt_key), None);
    }

    #[test]
    fn a_mod_the_registry_found_broken_on_the_eclipse_played_says_so() {
        let broken = [BrokenOn {
            eclipse: YOURS.into(),
            code: "E_TARGET_NOT_FOUND".into(),
        }];
        let at_28 = plays(Some(28));
        let found = fit(
            "eatinganimation",
            Some(1),
            OTHER,
            &at_28,
            &[],
            &broken,
            unasked,
        );
        assert_eq!(
            found,
            Some(Fit::Broken {
                code: "E_TARGET_NOT_FOUND".into()
            })
        );
        assert_eq!(
            describe(&found.unwrap(), &at_28),
            "Doesn't build on the Eclipse you play (E_TARGET_NOT_FOUND): a build leaves it out until its author updates it"
        );
        // Broken on another commit than the one played: as before.
        let elsewhere = [BrokenOn {
            eclipse: THIRD.into(),
            code: "E_TARGET_NOT_FOUND".into(),
        }];
        assert_eq!(
            fit(
                "eatinganimation",
                Some(1),
                OTHER,
                &at_28,
                &[],
                &elsewhere,
                unasked
            ),
            Some(Fit::Fits)
        );
        // A mod pinning another commit builds there, not on the Eclipse played.
        assert_eq!(
            fit("jei", None, OTHER, &at_28, &[], &broken, unasked),
            Some(Fit::MovesTo { pin: OTHER.into() })
        );
    }

    #[test]
    fn a_hook_points_mod_fits_that_version_or_a_later_one() {
        let at_28 = plays(Some(28));
        assert_eq!(
            fit("zoom", Some(28), OTHER, &at_28, &[], &[], unasked),
            Some(Fit::Fits)
        );
        assert_eq!(
            fit("zoom", Some(3), OTHER, &at_28, &[], &[], unasked),
            Some(Fit::Fits)
        );
        assert_eq!(
            fit("zoom", Some(30), YOURS, &at_28, &[], &[], unasked),
            Some(Fit::NeedsNewer { needs: 30, has: 28 })
        );
        // Unknown hook points: nothing is said rather than a guess.
        assert_eq!(
            fit("zoom", Some(1), OTHER, &plays(None), &[], &[], unasked),
            None
        );
    }

    #[test]
    fn a_hook_points_mod_ignores_the_others_pins() {
        let pins = [pin("veinminer", THIRD)];
        assert_eq!(
            fit("zoom", Some(5), OTHER, &plays(Some(9)), &pins, &[], unasked),
            Some(Fit::Fits)
        );
    }

    #[test]
    fn a_pinned_mod_fits_only_its_commit() {
        let p = plays(None);
        assert_eq!(
            fit("jei", None, YOURS, &p, &[], &[], unasked),
            Some(Fit::Fits)
        );
        assert_eq!(
            fit("jei", None, OTHER, &p, &[], &[], unasked),
            Some(Fit::MovesTo { pin: OTHER.into() })
        );
    }

    #[test]
    fn mods_pinning_another_commit_clash() {
        let pins = [
            pin("veinminer", THIRD),
            pin("chop", OTHER),
            pin("jei", THIRD),
        ];
        // `chop` agrees with the pin and `jei` is the mod being replaced: only veinminer clashes.
        assert_eq!(
            fit("jei", None, OTHER, &plays(None), &pins, &[], unasked),
            Some(Fit::Clashes {
                pin: OTHER.into(),
                with: vec!["veinminer".into()],
            })
        );
        let agreeing = [pin("chop", OTHER)];
        assert_eq!(
            fit("jei", None, OTHER, &plays(None), &agreeing, &[], unasked),
            Some(Fit::MovesTo { pin: OTHER.into() })
        );
    }

    #[test]
    fn a_hook_points_mod_moves_to_its_tested_commit_when_yours_lacks_them() {
        let at_28 = plays(Some(28));
        let tested = |revision: &str| Ok((revision == OTHER).then_some(30));
        assert_eq!(
            fit("recipes", Some(30), OTHER, &at_28, &[], &[], tested),
            Some(Fit::MovesTo { pin: OTHER.into() }),
            "the build compiles it on the commit it was tested on"
        );
        let older = |_: &str| Ok(Some(29));
        assert_eq!(
            fit("recipes", Some(30), OTHER, &at_28, &[], &[], older),
            Some(Fit::NeedsNewer { needs: 30, has: 28 })
        );
        let offline = |_: &str| Err("No connection".to_string());
        assert_eq!(
            fit("recipes", Some(30), OTHER, &at_28, &[], &[], offline),
            None
        );
    }
}
