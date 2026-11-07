use super::{json, progress};
use eclipse_launcher::backend::{self, Release};
use eclipse_launcher::build_import;
use std::path::PathBuf;

pub(super) fn versions(
    installed: bool,
    as_json: bool,
    settings: &backend::Settings,
) -> Result<(), String> {
    reuse_prepared(as_json, settings);
    if installed {
        let installed = backend::installed();
        let selected = backend::resolve_version(None, settings).ok();
        if as_json {
            json(serde_json::json!({ "installed": installed, "selected": selected }));
        } else if installed.is_empty() {
            println!("No version is installed.");
        }
        for tag in installed.iter().filter(|_| !as_json) {
            let mark = if selected.as_ref() == Some(tag) {
                "*"
            } else {
                " "
            };
            let state = if !backend::is_prepared(tag) {
                "not prepared"
            } else if backend::has_sounds(tag) {
                "prepared"
            } else {
                "prepared, no sounds (prepare again)"
            };
            println!("{mark} {tag}  {state}");
        }
    } else {
        let releases = backend::releases(settings)?;
        if as_json {
            json(releases.iter().map(release_json).collect());
            return Ok(());
        }
        let installed = backend::installed();
        for r in &releases {
            let state = match (installed.contains(&r.tag), r.asset.is_some()) {
                (true, _) => "installed",
                (false, true) => "",
                (false, false) => "no build for this platform",
            };
            println!("{:<15} {}  {}", r.tag, r.date, state);
        }
    }
    Ok(())
}

/// Versions and updates first give a version that isn't prepared the resources of one that is.
fn reuse_prepared(as_json: bool, settings: &backend::Settings) {
    for (tag, from) in backend::reuse_prepared_everywhere(settings) {
        if !as_json {
            eprintln!("Prepared {tag} with the resources of {from}.");
        }
    }
}

pub(super) fn install(
    tag: String,
    as_json: bool,
    settings: &backend::Settings,
) -> Result<(), String> {
    let label = format!("Installing {tag}");
    let tag = backend::install_tag(&tag, false, settings, progress(label))?;
    if as_json {
        json(serde_json::json!({ "installed": tag, "prepared": backend::is_prepared(&tag) }));
    } else if backend::is_prepared(&tag) {
        println!("Installed {tag}. It is prepared already, from another installed version.");
    } else {
        println!("Installed {tag}. Prepare it next: eclipse-launcher prepare --version {tag}");
    }
    Ok(())
}

pub(super) fn remove(tag: String) -> Result<(), String> {
    backend::remove_version(&tag)?;
    println!("Removed {tag}.");
    Ok(())
}

pub(super) fn import_build(
    path: PathBuf,
    version: Option<String>,
    trust_unsigned: bool,
    as_json: bool,
    settings: &backend::Settings,
) -> Result<(), String> {
    if !trust_unsigned {
        return Err("An imported build is unsigned: Eclipse can't check where it came from. If you trust it, pass --trust-unsigned.".into());
    }
    let tag = build_import::import(&path, version.as_deref(), settings)?;
    let prepared = backend::is_prepared(&tag);
    if as_json {
        json(serde_json::json!({ "imported": tag, "prepared": prepared }));
    } else if prepared {
        println!("Imported {tag} and selected it. It is prepared already, from another installed version.");
    } else {
        println!("Imported {tag} and selected it. Prepare it next: eclipse-launcher prepare --version {tag}");
    }
    Ok(())
}

pub(super) fn notes(
    tag: Option<String>,
    as_json: bool,
    settings: &backend::Settings,
) -> Result<(), String> {
    let r = backend::release_notes(tag.as_deref(), settings)?;
    if as_json {
        json(serde_json::json!({ "tag": r.tag, "name": r.name, "date": r.date, "notes": r.body }));
    } else {
        let title = if r.name.is_empty() { &r.tag } else { &r.name };
        println!(
            "{title}\n{}  {}\n\n{}",
            r.tag,
            r.date,
            backend::markdown_to_text(&r.body)
        );
    }
    Ok(())
}

pub(super) fn update(
    check: bool,
    as_json: bool,
    settings: &backend::Settings,
) -> Result<(), String> {
    reuse_prepared(as_json, settings);
    let updates = backend::check_updates(settings)?;
    if as_json && check {
        json(serde_json::json!({
            "eclipse": updates.eclipse.as_ref().map(|r| &r.tag),
            "launcher": updates.launcher.as_ref().and_then(|r| backend::launcher_version(&r.tag)),
            "launcher_current": backend::VERSION,
        }));
        return Ok(());
    }
    match &updates.eclipse {
        None => println!("Eclipse: up to date."),
        Some(r) if check => println!("Eclipse: {} is available.", r.tag),
        Some(r) => {
            let tag = backend::install_tag(
                &r.tag,
                true,
                settings,
                progress(format!("Installing {}", r.tag)),
            )?;
            println!("Eclipse: installed {tag}.");
        }
    }
    match &updates.launcher {
        None => println!("Launcher: {} is up to date.", backend::VERSION),
        Some(r) => {
            let version = backend::launcher_version(&r.tag).unwrap_or_default();
            if check {
                println!(
                    "Launcher: {version} is available (this is {}).",
                    backend::VERSION
                );
            } else if !backend::self_update_allowed() {
                println!("Launcher: {version} is available; this build does not replace itself.");
            } else {
                backend::self_update(r, progress(format!("Updating the launcher to {version}")))?;
                println!("Launcher: updated to {version}. Restart the launcher to use it.");
            }
        }
    }
    Ok(())
}

fn release_json(r: &Release) -> serde_json::Value {
    serde_json::json!({
        "tag": r.tag, "name": r.name, "date": r.date, "prerelease": r.prerelease,
        "available_here": r.asset.is_some(), "installed": backend::installed().contains(&r.tag),
    })
}
