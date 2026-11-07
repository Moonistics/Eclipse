//! A mod that alone stops a build (it hooks code the build's Eclipse doesn't have, or declares
//! the same support module as another mod) is left out, with the mods that need it, and the
//! build runs again without them: the player still gets a game with the others, and is told
//! which mods were left out and why.
use super::manifest::Bundle;
use super::mixins::Diagnostic;
use crate::i18n::trf;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Why {
    /// It doesn't apply to, or compile on, the Eclipse the build uses.
    OtherVersion,
    /// It declares the same support module, in the same file, as this mod.
    Clash(String),
    /// It needs this mod, which was left out.
    Needs(String),
}

/// A mod left out of a build, kept in its `native-mods.json` as `{id, version, why, with}`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(into = "Kept", try_from = "Kept")]
pub struct LeftOut {
    pub id: String,
    pub version: String,
    pub why: Why,
}

impl LeftOut {
    pub(super) fn new(bundle: &Bundle, why: Why) -> Self {
        Self {
            id: bundle.id.clone(),
            version: bundle.version.clone(),
            why,
        }
    }

    /// What the player reads.
    pub fn label(&self) -> String {
        match &self.why {
            Why::OtherVersion => trf(
                "{name} was left out: made for another Eclipse version",
                &[("name", &self.id)],
            ),
            Why::Clash(other) => trf(
                "{name} was left out: clashes with {other}",
                &[("name", &self.id), ("other", other)],
            ),
            Why::Needs(other) => trf(
                "{name} was left out: it needs {other}, which was left out",
                &[("name", &self.id), ("other", other)],
            ),
        }
    }
}

/// How a [`LeftOut`] is kept: a reason a later launcher adds reads as [`Why::OtherVersion`].
#[derive(serde::Serialize, serde::Deserialize)]
struct Kept {
    id: String,
    version: String,
    why: String,
    with: Option<String>,
}

impl From<LeftOut> for Kept {
    fn from(left: LeftOut) -> Self {
        let (why, with) = match left.why {
            Why::OtherVersion => ("other-version", None),
            Why::Clash(other) => ("clash", Some(other)),
            Why::Needs(other) => ("needs", Some(other)),
        };
        Self {
            id: left.id,
            version: left.version,
            why: why.into(),
            with,
        }
    }
}

impl TryFrom<Kept> for LeftOut {
    type Error = &'static str;

    fn try_from(kept: Kept) -> Result<Self, Self::Error> {
        let with = || kept.with.clone().ok_or("no other mod named");
        Ok(Self {
            why: match kept.why.as_str() {
                "clash" => Why::Clash(with()?),
                "needs" => Why::Needs(with()?),
                _ => Why::OtherVersion,
            },
            id: kept.id,
            version: kept.version,
        })
    }
}

/// The mod of `mods` (in build order) a horizon-mixins diagnostic at the end of its `output`
/// (`{"code", "message", "mod_id", "file"}`) blames, and why; None when it names none of them,
/// so the error is the build's. A support module's diagnostic names no mod (horizon-mixins
/// e8cd0aa) but the module (`support module 'x' collides …`) or its destination (`file`, which
/// this Eclipse may not have); between two mods, neither: the later one is left out.
pub(super) fn from_mixins(output: &str, mods: &[Bundle]) -> Option<LeftOut> {
    let found = Diagnostic::read(output)?;
    let message = found.message.as_deref().unwrap_or_default();
    let declared: Vec<(&Bundle, &[(String, String)])> =
        mods.iter().map(|b| (b, b.modules.as_slice())).collect();
    // The first mod declaring the same module as `bundle`, other than itself.
    let clash = |bundle: &Bundle| {
        declared
            .iter()
            .find(|(other, theirs)| {
                other.id != bundle.id && theirs.iter().any(|m| bundle.modules.contains(m))
            })
            .map(|(other, _)| other.id.clone())
    };
    let named = found
        .mod_id()
        .and_then(|id| mods.iter().find(|b| b.id == id))
        .or_else(|| {
            let name = message
                .split("support module '")
                .nth(1)?
                .split('\'')
                .next()?;
            declared
                .iter()
                .find(|(_, theirs)| theirs.iter().any(|(_, n)| n == name))
                .map(|(b, _)| *b)
        })
        .or_else(|| {
            let file = found.file.as_deref()?;
            let mut declaring = declared
                .iter()
                .filter(|(_, theirs)| theirs.iter().any(|(f, _)| f == file));
            match (declaring.next(), declaring.next()) {
                (Some((bundle, _)), None) => Some(*bundle),
                _ => None,
            }
        });
    if let Some(bundle) = named {
        let why = match found.code.as_deref() {
            Some("E_MODULE_CONFLICT") => clash(bundle).map_or(Why::OtherVersion, Why::Clash),
            _ => Why::OtherVersion,
        };
        return Some(LeftOut::new(bundle, why));
    }
    if found.code.as_deref() != Some("E_MODULE_CONFLICT") {
        return None;
    }
    // Two mods' modules: the later of the first pair declaring the same one.
    declared.iter().enumerate().find_map(|(i, (bundle, _))| {
        clash(bundle)
            .filter(|first| declared[..i].iter().any(|(b, _)| b.id == *first))
            .map(|first| LeftOut::new(bundle, Why::Clash(first)))
    })
}

/// The mods the compiler's errors were traced to (`owners`, as `id version`), among `mods`.
pub(super) fn from_compiler(owners: &[String], mods: &[Bundle]) -> Vec<LeftOut> {
    mods.iter()
        .filter(|b| owners.iter().any(|o| o.split(' ').next() == Some(&b.id)))
        .map(|b| LeftOut::new(b, Why::OtherVersion))
        .collect()
}

/// `left` and, after them, every mod of `mods` that needs one of them (not optionally),
/// transitively, each once.
pub(super) fn with_dependents(left: &[LeftOut], mods: &[Bundle]) -> Vec<LeftOut> {
    let mut out = left.to_vec();
    let mut gone: BTreeSet<String> = left.iter().map(|l| l.id.clone()).collect();
    loop {
        let next = mods.iter().find_map(|bundle| {
            let needed = bundle
                .dependencies
                .iter()
                .find(|d| !d.optional && gone.contains(&d.id))?;
            (!gone.contains(&bundle.id))
                .then(|| LeftOut::new(bundle, Why::Needs(needed.id.clone())))
        });
        let Some(next) = next else {
            return out;
        };
        gone.insert(next.id.clone());
        out.push(next);
    }
}

#[cfg(test)]
mod tests {
    use super::super::dependencies::Dependency;
    use super::*;
    use std::fs;
    use std::path::Path;

    fn bundle(root: &Path, id: &str, modules: &[(&str, &str)]) -> Bundle {
        let dir = root.join(id);
        fs::create_dir_all(&dir).unwrap();
        let mut text = format!("[mod]\nid = \"{id}\"\nversion = \"1.0.0\"\n");
        for (file, name) in modules {
            text += &format!(
                "[[module]]\nid = \"m\"\nfile = \"{file}\"\nname = \"{name}\"\nsource = \"m.rs\"\n"
            );
        }
        fs::write(dir.join(super::super::manifest::MANIFEST), text).unwrap();
        super::super::bundles(&dir).unwrap().remove(0)
    }

    #[test]
    fn a_target_this_eclipse_lacks_leaves_its_mod_out() {
        let root = super::super::tests::dir("isolate-target");
        let mods = [
            bundle(&root, "zoom", &[]),
            bundle(&root, "eatinganimation", &[]),
        ];
        // horizon-mixins e8cd0aa on Eclipse 6475f62, a player's build of 2026-11-05.
        let output = r#"error: plan failed
{
  "code": "E_TARGET_NOT_FOUND",
  "message": "no source function 'eclipse_client::item_render::ItemRenderer::resolve'",
  "mod_id": "eatinganimation",
  "injection_id": "held-frame"
}"#;
        let left = from_mixins(output, &mods).unwrap();
        assert_eq!(
            (left.id.as_str(), &left.why),
            ("eatinganimation", &Why::OtherVersion)
        );
        assert_eq!(
            left.label(),
            "eatinganimation was left out: made for another Eclipse version"
        );
        // A diagnostic naming no mod of the build is the build's own error.
        assert_eq!(
            from_mixins(r#"{"code": "E_METADATA", "message": "cargo"}"#, &mods),
            None
        );
        assert_eq!(from_mixins("no diagnostic", &mods), None);
    }

    #[test]
    fn a_module_clash_names_its_mod_by_the_module() {
        let root = super::super::tests::dir("isolate-module");
        let lib = "crates/eclipse-client/src/lib.rs";
        let mods = [
            bundle(&root, "gyro", &[(lib, "gyro")]),
            bundle(&root, "hitbox_plus", &[(lib, "official_hitbox_plus")]),
        ];
        // horizon-mixins e8cd0aa: no mod_id on a module's diagnostic.
        let output = r#"{"code": "E_MODULE_CONFLICT", "message": "support module 'official_hitbox_plus' collides with an existing type/module/import or unresolved glob import"}"#;
        let left = from_mixins(output, &mods).unwrap();
        assert_eq!(
            (left.id.as_str(), left.why),
            ("hitbox_plus", Why::OtherVersion)
        );
        // Or by its destination, a file this Eclipse doesn't have (worldedit 1.2.1 on 6475f62).
        let chat = "crates/eclipse-client/src/chat.rs";
        let mods = [
            bundle(&root, "zoom", &[]),
            bundle(&root, "worldedit", &[(chat, "worldedit_chat")]),
        ];
        let output = r#"{"code": "E_MODULE", "message": "module destination must be an indexed Rust source file", "file": "crates/eclipse-client/src/chat.rs"}"#;
        assert_eq!(from_mixins(output, &mods).unwrap().id, "worldedit");
    }

    #[test]
    fn two_mods_declaring_one_module_leave_the_later_out() {
        let root = super::super::tests::dir("isolate-clash");
        let game = "crates/eclipse-client/src/game.rs";
        let mods = [
            bundle(&root, "a_zoom", &[(game, "zoom")]),
            bundle(&root, "fps", &[(game, "fps")]),
            bundle(&root, "b_zoom", &[(game, "zoom")]),
        ];
        let output = r#"{"code": "E_MODULE_CONFLICT", "message": "competing support modules", "file": "crates/eclipse-client/src/game.rs"}"#;
        let left = from_mixins(output, &mods).unwrap();
        assert_eq!(left.id, "b_zoom");
        assert_eq!(left.label(), "b_zoom was left out: clashes with a_zoom");
        // Named, it says which mod it clashes with too.
        let output = r#"{"code": "E_MODULE_CONFLICT", "message": "competing support modules", "mod_id": "a_zoom"}"#;
        assert_eq!(
            from_mixins(output, &mods).unwrap().why,
            Why::Clash("b_zoom".into())
        );
    }

    #[test]
    fn the_mods_needing_a_left_out_mod_go_too_but_not_optional_ones() {
        let root = super::super::tests::dir("isolate-deps");
        let [lib, addon, extra, maybe, other] =
            ["lib", "addon", "extra", "maybe", "other"].map(|id| bundle(&root, id, &[]));
        let dep = |id: &str, optional| Dependency {
            id: id.into(),
            requirement: semver::VersionReq::STAR,
            optional,
        };
        let (on_lib, on_addon, maybe_lib) = (
            [dep("lib", false)],
            [dep("addon", false)],
            [dep("lib", true)],
        );
        let needing = |bundle: &Bundle, dependencies: &[Dependency]| Bundle {
            dependencies: dependencies.to_vec(),
            ..bundle.clone()
        };
        let mods = [
            needing(&extra, &on_addon),
            lib.clone(),
            needing(&addon, &on_lib),
            needing(&maybe, &maybe_lib),
            other,
        ];
        let left = with_dependents(&[LeftOut::new(&lib, Why::OtherVersion)], &mods);
        let ids: Vec<&str> = left.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids, ["lib", "addon", "extra"]);
        assert_eq!(
            left[2].label(),
            "extra was left out: it needs addon, which was left out"
        );
        // What a build's record keeps reads back the same.
        let kept = serde_json::to_string(&left).unwrap();
        assert_eq!(serde_json::from_str::<Vec<LeftOut>>(&kept).unwrap(), left);
    }
}
