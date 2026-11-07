use super::manifest::{Bundle, Injection, InjectionSite};
use std::collections::BTreeMap;

fn names_in_order<'a, T>(pairs: &[(&'a str, T)]) -> Vec<&'a str> {
    let mut names: Vec<&str> = Vec::new();
    for (name, _) in pairs {
        if !names.contains(name) {
            names.push(name);
        }
    }
    names
}

/// The build's warnings about the mods' injections (#6490): two mods on one
/// place, mods wrapping one function at its head and its return, and a
/// cancelling injection on a native hook point. Neither stops the build —
/// they say what the player and the author could not otherwise see. The
/// places are the manifest's: whether two selectors pick the same call, or
/// a raw `target` names a hook point's function, only horizon-mixins
/// resolves, so the warnings stay to what the manifest says for sure.
pub(super) fn warn_conflicts(mods: &[Bundle]) -> Vec<String> {
    let mut out = Vec::new();
    let mut places: BTreeMap<_, Vec<(&str, &Injection)>> = BTreeMap::new();
    for b in mods {
        for i in &b.injections {
            places.entry(i.place()).or_default().push((&b.id, i));
        }
    }
    for (_, list) in places {
        let mut list = list;
        list.sort_by_key(|(mod_id, i)| {
            (
                std::cmp::Reverse(i.priority),
                (*mod_id).to_owned(),
                i.id.clone(),
            )
        });
        let names = names_in_order(&list);
        if names.len() < 2 {
            continue;
        }
        let first = list[0].1;
        let both = if names.len() == 2 { "both" } else { "all" };
        out.push(format!(
            "mods {} {both} inject at {}; {} runs first (priority descending, mod id ascending).",
            names.join(", "),
            first.place_as_text(),
            names[0]
        ));
    }
    let mut functions: BTreeMap<&InjectionSite, Vec<(&str, &Injection)>> = BTreeMap::new();
    for b in mods {
        for i in &b.injections {
            if i.kind.as_deref() == Some("HEAD") || i.kind.as_deref() == Some("RETURN") {
                functions.entry(&i.site).or_default().push((&b.id, i));
            }
        }
    }
    for (site, list) in functions {
        let kinds: std::collections::BTreeSet<&str> =
            list.iter().filter_map(|(_, i)| i.kind.as_deref()).collect();
        if kinds.len() < 2 || names_in_order(&list).len() < 2 {
            continue;
        }
        let sides: Vec<String> = list
            .iter()
            .map(|(mod_id, i)| format!("{mod_id} ({})", i.kind.as_deref().unwrap_or("")))
            .collect();
        out.push(format!(
            "mods {} inject into {}: HEAD runs on entry, RETURN on return, whatever their priorities.",
            sides.join(", "),
            site.as_text()
        ));
    }
    // A cancelling `HEAD` on a hook point: a `Break` skips the later
    // injections at the place and the function's body. A raw `target` is
    // only known to be a hook point through the catalog, which the launcher
    // does not read; a `hook` name is one by definition. Only `HEAD` is
    // said: the interior kinds' cancellation skips less than the body.
    for b in mods {
        for i in &b.injections {
            if i.cancellable
                && i.kind.as_deref() == Some("HEAD")
                && matches!(i.site, InjectionSite::Hook(_))
            {
                out.push(format!(
                    "{}'s `{}` can cancel at {}: a Break skips the later injections and the function's body.",
                    b.id,
                    i.id,
                    i.place_as_text()
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_mods::tests::bundle_of;

    #[test]
    fn two_mods_on_one_hook_warn_in_run_order() {
        let mods = [
            bundle_of(
                "alpha",
                "[[injection]]\nid = \"a\"\nhook = \"frame\"\nkind = \"HEAD\"\n",
            ),
            bundle_of(
                "beta",
                "[mod]\nid = \"beta\"\nversion = \"0.1.0\"\npriority = 200\n\
                 [[injection]]\nid = \"b\"\nhook = \"frame\"\nkind = \"HEAD\"\n",
            ),
        ];
        assert_eq!(
            warn_conflicts(&mods),
            ["mods beta, alpha both inject at hook `frame` HEAD; beta runs first (priority descending, mod id ascending)."]
        );
    }

    /// horizon-mixins defaults a mod's priority to 0, not to the README
    /// example's 100: a mod without one runs after `priority = 50`.
    #[test]
    fn the_default_priority_is_zero() {
        let mods = [
            bundle_of(
                "alpha",
                "[[injection]]\nid = \"a\"\nhook = \"frame\"\nkind = \"HEAD\"\n",
            ),
            bundle_of(
                "beta",
                "[[injection]]\nid = \"b\"\nhook = \"frame\"\nkind = \"HEAD\"\npriority = 50\n",
            ),
        ];
        assert_eq!(
            warn_conflicts(&mods),
            ["mods beta, alpha both inject at hook `frame` HEAD; beta runs first (priority descending, mod id ascending)."]
        );
    }

    #[test]
    fn a_priority_tie_orders_by_mod_id() {
        let mods = [
            bundle_of(
                "beta",
                "[[injection]]\nid = \"b\"\nhook = \"tick\"\nkind = \"HEAD\"\n",
            ),
            bundle_of(
                "alpha",
                "[[injection]]\nid = \"a\"\nhook = \"tick\"\nkind = \"HEAD\"\n",
            ),
        ];
        assert_eq!(
            warn_conflicts(&mods),
            ["mods alpha, beta both inject at hook `tick` HEAD; alpha runs first (priority descending, mod id ascending)."]
        );
    }

    #[test]
    fn an_injections_priority_overrides_its_mods() {
        let mods = [
            bundle_of(
                "alpha",
                "[[injection]]\nid = \"a\"\nhook = \"tick\"\nkind = \"HEAD\"\npriority = 50\n",
            ),
            bundle_of(
                "beta",
                "[[injection]]\nid = \"b\"\nhook = \"tick\"\nkind = \"HEAD\"\n",
            ),
        ];
        // beta's is 0 (the default), alpha's 50: alpha first.
        assert_eq!(
            warn_conflicts(&mods),
            ["mods alpha, beta both inject at hook `tick` HEAD; alpha runs first (priority descending, mod id ascending)."]
        );
    }

    #[test]
    fn two_mods_on_one_raw_target_warn_too() {
        let mods = [
            bundle_of(
                "alpha",
                "[[injection]]\nid = \"a\"\ntarget = \"game::calls::compute\"\nkind = \"HEAD\"\n",
            ),
            bundle_of(
                "beta",
                "[[injection]]\nid = \"b\"\ntarget = \"game::calls::compute\"\nkind = \"HEAD\"\n",
            ),
        ];
        assert_eq!(
            warn_conflicts(&mods),
            ["mods alpha, beta both inject at `game::calls::compute` HEAD; alpha runs first (priority descending, mod id ascending)."]
        );
    }

    /// HEAD and RETURN wrap the whole function: HEAD runs on entry, RETURN
    /// on return, whatever the priorities, so no "runs first".
    #[test]
    fn head_and_return_on_one_function_warn_without_a_runs_first() {
        let mods = [
            bundle_of(
                "alpha",
                "[[injection]]\nid = \"a\"\nhook = \"tick\"\nkind = \"HEAD\"\n",
            ),
            bundle_of(
                "beta",
                "[[injection]]\nid = \"b\"\nhook = \"tick\"\nkind = \"RETURN\"\n",
            ),
        ];
        assert_eq!(
            warn_conflicts(&mods),
            ["mods alpha (HEAD), beta (RETURN) inject into hook `tick`: HEAD runs on entry, RETURN on return, whatever their priorities."]
        );
    }

    /// Two interior injections of one function with different selectors are
    /// different places: horizon-mixins resolves the selector against the
    /// source, the launcher does not guess it.
    #[test]
    fn different_selectors_are_different_places() {
        let mods = [
            bundle_of(
                "alpha",
                "[[injection]]\nid = \"a\"\nhook = \"tick\"\nkind = \"BEFORE_CALL\"\n\
                 selector = \"x\"\n",
            ),
            bundle_of(
                "beta",
                "[[injection]]\nid = \"b\"\nhook = \"tick\"\nkind = \"BEFORE_CALL\"\n\
                 selector = \"y\"\n",
            ),
        ];
        assert!(warn_conflicts(&mods).is_empty());
    }

    #[test]
    fn same_selector_and_ordinal_is_one_place() {
        let mods = [
            bundle_of(
                "alpha",
                "[[injection]]\nid = \"a\"\nhook = \"tick\"\nkind = \"BEFORE_CALL\"\n\
                 selector = \"x\"\nordinal = 1\n",
            ),
            bundle_of(
                "beta",
                "[[injection]]\nid = \"b\"\nhook = \"tick\"\nkind = \"BEFORE_CALL\"\n\
                 selector = \"x\"\nordinal = 1\n",
            ),
        ];
        assert_eq!(
            warn_conflicts(&mods),
            ["mods alpha, beta both inject at hook `tick` BEFORE_CALL selector `x` ordinal 1; alpha runs first (priority descending, mod id ascending)."]
        );
    }

    #[test]
    fn different_sites_stay_quiet() {
        let mods = [
            bundle_of(
                "alpha",
                "[[injection]]\nid = \"a\"\nhook = \"frame\"\nkind = \"HEAD\"\n",
            ),
            bundle_of(
                "beta",
                "[[injection]]\nid = \"b\"\nhook = \"tick\"\nkind = \"HEAD\"\n",
            ),
        ];
        assert!(warn_conflicts(&mods).is_empty());
    }

    #[test]
    fn a_cancelling_head_on_a_hook_point_warns() {
        let mods = [bundle_of(
            "alpha",
            "[[injection]]\nid = \"stop\"\nhook = \"frame\"\nkind = \"HEAD\"\ncancellable = true\n",
        )];
        assert_eq!(
            warn_conflicts(&mods),
            ["alpha's `stop` can cancel at hook `frame` HEAD: a Break skips the later injections and the function's body."]
        );
    }

    /// Cancellation is allowed on the interior kinds too, but what a Break
    /// skips there is less than the body: only `HEAD` is said.
    #[test]
    fn a_cancelling_interior_kind_stays_quiet() {
        let mods = [bundle_of(
            "alpha",
            "[[injection]]\nid = \"stop\"\nhook = \"tick\"\nkind = \"BEFORE_CALL\"\n\
             cancellable = true\n",
        )];
        assert!(warn_conflicts(&mods).is_empty());
    }

    #[test]
    fn a_cancelling_injection_on_a_raw_target_stays_quiet() {
        // Whether a raw target fires a hook point only the catalog knows,
        // which the launcher does not read.
        let mods = [bundle_of(
            "alpha",
            "[[injection]]\nid = \"stop\"\ntarget = \"game::tick\"\nkind = \"HEAD\"\n\
             cancellable = true\n",
        )];
        assert!(warn_conflicts(&mods).is_empty());
    }

    #[test]
    fn one_mod_with_two_injections_on_one_site_stays_quiet() {
        let mods = [bundle_of(
            "alpha",
            "[[injection]]\nid = \"first\"\nhook = \"frame\"\nkind = \"HEAD\"\n\
             [[injection]]\nid = \"second\"\nhook = \"frame\"\nkind = \"HEAD\"\n",
        )];
        assert!(warn_conflicts(&mods).is_empty());
    }
}
