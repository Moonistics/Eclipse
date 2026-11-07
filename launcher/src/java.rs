//! Java for the real Minecraft: Java Edition (Java instances): which runtime a version needs, the
//! ones on this computer, and Mojang's own when none fits, as other launchers do.
//!
//! A version's JSON names what it runs on: `javaVersion.majorVersion` (21 for 1.21) and the
//! `component` of Mojang's runtime built for it (`java-runtime-delta`). Versions older than that
//! field ran Java 8 (`jre-legacy`). The game wants that major exactly: older versions and their
//! loaders often fail on a newer Java, so a newer one is never picked by itself.
//!
//! Runtimes are found without starting them: each Java home has a `release` file whose
//! `JAVA_VERSION` gives its version. Mojang's are kept in `minecraft/runtime/` in the launcher's
//! data folder (`mojang::java_runtime` lays them out), the folder preparing uses too.

use crate::{backend, mojang, sources};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// What a version runs on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Need {
    pub major: u32,
    /// Mojang's runtime for it, e.g. `java-runtime-delta`.
    pub component: String,
}

impl Need {
    /// From a version JSON; Java 8 for versions from before `javaVersion`.
    pub fn of(version: &serde_json::Value) -> Need {
        let java = &version["javaVersion"];
        match (java["majorVersion"].as_u64(), java["component"].as_str()) {
            (Some(major), Some(component)) => Need {
                major: major as u32,
                component: component.to_string(),
            },
            _ => Need {
                major: 8,
                component: "jre-legacy".into(),
            },
        }
    }
}

/// Mojang's runtime components by the major they carry, newest component of each.
const COMPONENTS: [(u32, &str); 5] = [
    (8, "jre-legacy"),
    (16, "java-runtime-alpha"),
    (17, "java-runtime-gamma"),
    (21, "java-runtime-delta"),
    (25, "java-runtime-epsilon"),
];

impl Need {
    /// A major (`21`) or a component name (`java-runtime-delta`), as typed.
    pub fn named(name: &str) -> Result<Need, String> {
        let name = name.trim();
        let by_major = name.parse::<u32>().ok().and_then(|major| {
            COMPONENTS
                .iter()
                .find(|(m, _)| *m == major)
                .map(|(m, c)| (*m, *c))
        });
        let by_name = COMPONENTS.iter().find(|(_, c)| *c == name).copied();
        let (major, component) = by_major.or(by_name).ok_or_else(|| {
            let known: Vec<String> = COMPONENTS
                .iter()
                .map(|(m, c)| format!("{m} ({c})"))
                .collect();
            format!("Mojang has no Java {name:?}. Known: {}.", known.join(", "))
        })?;
        Ok(Need {
            major,
            component: component.to_string(),
        })
    }
}

/// Where a runtime comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Mojang's, downloaded by Eclipse.
    Mojang,
    /// Installed on this computer (JAVA_HOME, the PATH, the usual folders).
    Installed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Runtime {
    pub home: PathBuf,
    /// Its full version, e.g. `21.0.5`.
    pub version: String,
    pub major: u32,
    pub source: Source,
}

impl Runtime {
    /// The `java` program to start the game with.
    pub fn java(&self) -> PathBuf {
        self.home
            .join("bin")
            .join(if cfg!(windows) { "javaw.exe" } else { "java" })
    }
}

/// The version a Java home's `release` file names (`JAVA_VERSION="21.0.5"`), with its major.
fn release_version(home: &Path) -> Option<(String, u32)> {
    let text = fs::read_to_string(home.join("release")).ok()?;
    let version = text
        .lines()
        .find_map(|l| l.strip_prefix("JAVA_VERSION="))?
        .trim()
        .trim_matches('"')
        .to_string();
    let mut parts = version.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    let major = if first == 1 {
        parts.next()?.parse().ok()?
    } else {
        first
    };
    Some((version, major))
}

fn runtime(home: PathBuf, source: Source) -> Option<Runtime> {
    let (version, major) = release_version(&home)?;
    let found = Runtime {
        home,
        version,
        major,
        source,
    };
    found.java().is_file().then_some(found)
}

/// Mojang's runtimes Eclipse downloaded.
pub fn runtimes_dir() -> PathBuf {
    backend::own_minecraft_dir().join("runtime")
}

/// The runtimes in a Mojang `runtime` folder.
pub fn mojang_runtimes(dir: &Path) -> Vec<Runtime> {
    eclipse_prepare::java::minecraft_runtimes(dir)
        .into_iter()
        .filter_map(|home| runtime(home, Source::Mojang))
        .collect()
}

/// Java homes installed on this computer: JAVA_HOME, the PATH's, then the folders installers use,
/// each once.
pub fn installed() -> Vec<Runtime> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
    let mut homes: Vec<PathBuf> = var("JAVA_HOME").map(PathBuf::from).into_iter().collect();
    if let Some(path) = var("PATH") {
        homes.extend(
            std::env::split_paths(&path)
                .filter(|dir| dir.file_name().is_some_and(|n| n == "bin"))
                .filter_map(|dir| fs::canonicalize(dir.join("java")).ok())
                .filter_map(|java| Some(java.parent()?.parent()?.to_path_buf())),
        );
    }
    for parent in parents() {
        for entry in fs::read_dir(parent).into_iter().flatten().flatten() {
            let bundle = entry.path().join("Contents/Home");
            homes.push(if bundle.is_dir() {
                bundle
            } else {
                entry.path()
            });
        }
    }
    let mut found: Vec<Runtime> = Vec::new();
    for home in homes {
        let home = fs::canonicalize(&home).unwrap_or(home);
        if !found.iter().any(|r| r.home == home) {
            found.extend(runtime(home, Source::Installed));
        }
    }
    found
}

/// The folders Java installers put homes in, one home per child.
fn parents() -> Vec<PathBuf> {
    let home = dirs::home_dir();
    let mut parents: Vec<PathBuf> = match sources::Os::current() {
        sources::Os::MacOs => {
            let mut p = vec![PathBuf::from("/Library/Java/JavaVirtualMachines")];
            p.extend(
                home.iter()
                    .map(|h| h.join("Library/Java/JavaVirtualMachines")),
            );
            p
        }
        sources::Os::Linux => ["/usr/lib/jvm", "/usr/java", "/usr/local/lib/jvm"]
            .map(PathBuf::from)
            .into(),
        sources::Os::Windows => ["ProgramFiles", "ProgramW6432"]
            .into_iter()
            .filter_map(std::env::var_os)
            .flat_map(|base| {
                let base = PathBuf::from(base);
                ["Eclipse Adoptium", "Java", "Microsoft", "Zulu", "BellSoft"].map(|v| base.join(v))
            })
            .collect(),
    };
    parents.extend(
        home.iter()
            .flat_map(|h| [h.join(".jdks"), h.join(".sdkman/candidates/java")]),
    );
    parents
}

/// The runtime for `need`: the Java home an instance names when it names one, else Mojang's
/// runtime of that major, else an installed one of that major. None: Mojang's is to be downloaded.
pub fn pick(
    need: &Need,
    chosen: Option<&Path>,
    mojang: &[Runtime],
    installed: &[Runtime],
) -> Result<Option<Runtime>, String> {
    if let Some(home) = chosen {
        return runtime(home.to_path_buf(), Source::Installed)
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "{} is not a Java home (no release file or java)",
                    home.display()
                )
            });
    }
    Ok(mojang
        .iter()
        .chain(installed)
        .find(|r| r.major == need.major)
        .cloned())
}

fn download(component: &str, log: &dyn Fn(String)) -> Result<PathBuf, String> {
    mojang::java_runtime(
        component,
        &runtimes_dir(),
        mojang::FILES_AT_A_TIME,
        &backend::get_bytes,
        log,
    )
}

/// The dynamic loader glibc programs on x86-64 Linux start through.
const GLIBC_LOADER: &str = "/lib64/ld-linux-x86-64.so.2";

/// Mojang builds its Linux Java for glibc: without glibc's loader (NixOS without nix-ld, musl
/// distributions) it cannot start, so it isn't downloaded.
fn without_glibc() -> bool {
    cfg!(all(target_os = "linux", target_arch = "x86_64")) && !Path::new(GLIBC_LOADER).exists()
}

/// Downloads Mojang's runtime for `need` into [`runtimes_dir`] (what is already there and whole
/// is kept).
pub fn install(need: &Need, log: &dyn Fn(String)) -> Result<Runtime, String> {
    if without_glibc() {
        return Err(format!(
            "Mojang's Java needs glibc ({GLIBC_LOADER} is missing; on NixOS, enable programs.nix-ld). Install Java {} and pick it instead.",
            need.major
        ));
    }
    let home = download(&need.component, log)?;
    let home = ["jre.bundle/Contents/Home", ""]
        .iter()
        .map(|sub| home.join(sub))
        .find(|h| h.join("release").is_file())
        .unwrap_or(home);
    runtime(home, Source::Mojang)
        .ok_or_else(|| format!("Mojang's {} has no usable java", need.component))
}

/// The Java home a preparation runs with: a JDK of `major` when the version's preparer compiles
/// (`compiler`), else a runtime of it, found as eclipse-prepare finds Java: on this computer, then
/// among the runtimes of the Minecraft folders `games`, then Mojang's `component()` (the one the
/// version's file names), downloaded into [`runtimes_dir`] (Mojang's runtimes are whole JDKs).
pub fn for_preparing(
    major: i64,
    compiler: bool,
    games: &[PathBuf],
    component: impl FnOnce() -> Option<String>,
    log: &dyn Fn(String),
) -> Result<PathBuf, String> {
    let find = if compiler {
        eclipse_prepare::java::find
    } else {
        eclipse_prepare::java::find_runtime
    };
    if let Ok(jdk) = find(major, &[]) {
        return Ok(jdk.home());
    }
    let mut runtimes: Vec<PathBuf> = Vec::new();
    for home in games
        .iter()
        .flat_map(|game| eclipse_prepare::java::minecraft_runtimes(&game.join("runtime")))
    {
        if !runtimes.contains(&home) {
            runtimes.push(home);
        }
    }
    let missing = match find(major, &runtimes) {
        Ok(jdk) => return Ok(jdk.home()),
        Err(e) => e,
    };
    let component = component().ok_or_else(|| missing.clone())?;
    if without_glibc() {
        return Err(format!(
            "{missing}\n  • Minecraft's own Java from Mojang needs glibc ({GLIBC_LOADER} is missing; on NixOS, enable programs.nix-ld), so install compatible Java instead."
        ));
    }
    let home = download(&component, log).map_err(|e| {
        format!("{missing}\n  • Eclipse also tried to download Minecraft's own Java from Mojang, but: {e}")
    })?;
    find(major, &[home]).map(|jdk| jdk.home())
}

#[cfg(test)]
#[path = "java/tests.rs"]
mod tests;
