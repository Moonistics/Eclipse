use super::*;
use super::{
    architecture::*, game_output::*, links::*, minecraft::*, net::*, paths::*, platform::*,
    play::*, prepare::*, releases::*, resources::*, self_update::*, settings::*, updates::*,
    versions::*,
};
use crate::{account, sources};
use std::cmp::Ordering::*;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[test]
#[cfg(windows)]
fn failed_removal_stays_listed_and_can_be_retried() {
    use std::os::windows::fs::OpenOptionsExt;
    let versions =
        std::env::temp_dir().join(format!("eclipse-remove-locked-{}", std::process::id()));
    let dir = versions.join("v0.2.0");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(INSTALL_MARK), "").unwrap();
    fs::write(dir.join(CLIENT), "locked executable").unwrap();
    // Refuse delete sharing, as a running Windows executable does.
    let running = fs::File::options()
        .read(true)
        .share_mode(1)
        .open(dir.join(CLIENT))
        .unwrap();
    assert!(remove_in(&dir).is_err());
    assert_eq!(installed_in(&versions), ["v0.2.0"]);
    drop(running);
    remove_in(&dir).unwrap();
    assert!(installed_in(&versions).is_empty());
    assert!(!dir.exists());
    fs::remove_dir_all(versions).unwrap();
}

/// [#5189](https://github.com/Moonistics/Eclipse/issues/5189): MinGW-w64's tools can't
/// read a non-ASCII path, so the build root is the data folder's resolved ASCII spelling —
/// which a link's target or a short (8.3) name already is.
#[test]
fn the_build_dir_is_a_path_the_tools_can_read() {
    assert!(ascii(Path::new(
        r"C:\Users\me\AppData\Roaming\eclipse-launcher"
    )));
    assert!(!ascii(Path::new(
        r"C:\Users\Кириченко Павло\AppData\Roaming\eclipse-launcher"
    )));
    assert!(!ascii(Path::new("/høme/me/eclipse-launcher")));
    // A space splits windres' include folders, which matters on Windows only.
    let spaced = Path::new(r"C:\Users\Jane Doe\AppData\Roaming\eclipse-launcher");
    assert!(ascii(spaced));
    assert_eq!(tool_safe(spaced), !cfg!(windows));
    let built = build_dir();
    assert!(ascii(&built) || cfg!(windows), "{}", built.display());
    // Resolved: the build never sees a link in the path (where the answer is the data
    // folder itself, so already resolved — Windows' other spellings aren't its own).
    if ascii(&built) {
        assert_eq!(
            built,
            built
                .canonicalize()
                .map(crate::files::plain)
                .unwrap_or_else(|_| built.clone())
        );
    }
}

/// [#6009](https://github.com/Moonistics/Eclipse/issues/6009): with a Cyrillic user name the
/// build folder is a real ASCII folder of ProgramData's (not a short name of the data folder,
/// which rustc resolves back to the Cyrillic one), its own for each data folder.
#[test]
fn a_cyrillic_data_folder_builds_in_an_ascii_folder_of_its_own() {
    let pavlo = Path::new(r"C:\Users\Кириченко Павло\AppData\Roaming\eclipse-launcher");
    let other = Path::new(r"C:\Users\Олена\AppData\Roaming\eclipse-launcher");
    let public = PathBuf::from(if cfg!(windows) { r"D:\Data" } else { "/data" });
    let dir = ascii_build_dir(pavlo, Some(public.clone()));
    assert!(ascii(&dir), "{}", dir.display());
    assert_eq!(dir.parent(), Some(public.as_path()));
    let name = dir.file_name().unwrap().to_str().unwrap();
    assert!(
        name.starts_with("eclipse-launcher-") && name.len() == 25,
        "{name}"
    );
    assert_eq!(dir, ascii_build_dir(pavlo, Some(public.clone())), "stable");
    assert_ne!(dir, ascii_build_dir(other, Some(public)), "one per user");
    // %ProgramData% unset, relative or itself not ASCII: C:\ProgramData.
    let default = PathBuf::from(r"C:\ProgramData");
    for pd in [None, Some("ProgramData".into()), Some(r"C:\Дані".into())] {
        let dir = ascii_build_dir(pavlo, pd);
        assert_eq!(dir.parent(), Some(default.as_path()));
        assert!(ascii(&dir));
    }
}

/// The game program's mod API level is its mark's; a program from before the mark (0.2.68
/// and older) or none at all serves level 0.
#[test]
fn a_game_program_says_its_mod_api_level() {
    let dir = std::env::temp_dir().join(format!("eclipse-api-level-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let marked = dir.join("marked");
    let mut program = b"\x7fELF code code".to_vec();
    program.extend_from_slice(b"eclipse-mod-api-level:0007;");
    fs::write(&marked, &program).unwrap();
    let old = dir.join("old");
    fs::write(&old, b"\x7fELF code without a mark").unwrap();
    assert_eq!(program_mod_api_level(&marked), Some(7));
    assert_eq!(program_mod_api_level(&old), Some(0));
    assert_eq!(program_mod_api_level(&dir.join("missing")), None);
    fs::remove_dir_all(&dir).unwrap();
}

/// A program that couldn't be read once (not there yet, held by a scan) isn't level 0 for the
/// rest of the run.
#[test]
fn a_program_read_later_says_its_own_level() {
    let tag = format!("v0.0.{}-level", std::process::id());
    let dir = version_dir(&tag);
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(mod_api_level(&tag), 0);
    fs::create_dir_all(&dir).unwrap();
    let mut program = b"\x7fELF code code".to_vec();
    program.extend_from_slice(b"eclipse-mod-api-level:0007;");
    fs::write(dir.join(CLIENT), &program).unwrap();
    assert_eq!(mod_api_level(&tag), 7);
    fs::remove_dir_all(&dir).unwrap();
}

/// The launcher downloads Mojang's game files (client jar, libraries, assets) only for an
/// account the services said owns the game: never without one, and never past the gate by
/// the developer option (`Settings::dev_offline`), whose pass has no owner.
#[test]
fn mojang_downloads_only_for_an_owner() {
    let dir = std::env::temp_dir().join(format!("eclipse-owner-only-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let pins = sources::Pins::read(&serde_json::json!({ "minecraft": {
        "version": "26.3",
        "client_sha1": "00",
        "manifest_sha1": "00",
    }}))
    .unwrap();
    let asked = std::sync::Mutex::new(Vec::<String>::new());
    let fetch = |url: &str| -> Result<Vec<u8>, String> {
        asked.lock().unwrap().push(url.to_string());
        Err("no network in tests".into())
    };
    let log = |_: String| {};
    let no_owner = || Ok(account::Launch::default());
    for (pass, signed_in) in [
        (Err(account::NoOwner), false),
        (Err(account::NoOwner), true),
        (no_owner(), false),
        (no_owner(), true),
    ] {
        assert!(download_for(&pins, &dir, pass, signed_in, &fetch, &log).is_err());
    }
    assert!(asked.lock().unwrap().is_empty());
    assert!(!dir.exists());
    // An owner: Mojang is asked (and the download fails here, without network).
    let owner = account::Account {
        uuid: "069a79f444e94726a5befca90e38aaf5".into(),
        name: "Notch".into(),
        skin: None,
        cape: None,
    };
    let secrets = serde_json::from_str(
        r#"{"refresh_token":"r","minecraft_token":"t","minecraft_expires_at":1,"owns":true}"#,
    )
    .unwrap();
    let pass = Ok(account::Launch {
        owner: Some((owner, secrets)),
        ..Default::default()
    });
    assert!(download_for(&pins, &dir, pass, true, &fetch, &log).is_err());
    assert!(!asked.lock().unwrap().is_empty());
    let _ = fs::remove_dir_all(&dir);

    // A store that lacks assets is not filled for a non-owner either: nothing is asked.
    fs::create_dir_all(dir.join("assets/indexes")).unwrap();
    fs::write(
        dir.join("assets/indexes/34.json"),
        r#"{"objects":{"a":{"hash":"00","size":1}}}"#,
    )
    .unwrap();
    asked.lock().unwrap().clear();
    for (pass, signed_in) in [(Err(account::NoOwner), true), (no_owner(), true)] {
        assert!(download_for(&pins, &dir, pass, signed_in, &fetch, &log).is_err());
    }
    assert!(asked.lock().unwrap().is_empty());
    let _ = fs::remove_dir_all(&dir);

    // In the sources: one call downloads the game, in `download_for`, and the only caller
    // of that asks the strict gate. Game-file hosts appear nowhere else.
    let mut files = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    let mut sources = Vec::new();
    while let Some(path) = files.pop() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if path.is_dir() {
            files.extend(fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
        } else if path.extension().is_some_and(|e| e == "rs") {
            let in_tests =
                name.contains("tests") || path.components().any(|c| c.as_os_str() == "tests");
            // The code alone: test modules (this one included) come after it.
            // A Windows checkout has CRLF line ends: the searches below look for "\n".
            let mut text = fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
            text.truncate(text.find("#[cfg(test)]").unwrap_or(text.len()));
            sources.push((name, in_tests, text));
        }
    }
    let code: Vec<_> = sources.iter().filter(|(_, tests, _)| !tests).collect();
    let calls: Vec<_> = code
        .iter()
        .flat_map(|(name, _, text)| {
            text.matches("mojang::download(")
                .map(move |_| name.as_str())
        })
        .collect();
    assert_eq!(calls, ["minecraft.rs"]);
    let body = |f: &str| {
        code.iter()
            .find_map(|(_, _, text)| {
                let start = text
                    .match_indices(&format!("fn {f}("))
                    .map(|(at, _)| at)
                    .find(|&at| {
                        text[..at]
                            .rsplit('\n')
                            .next()
                            .is_some_and(|line| !line.starts_with(' '))
                    })?;
                Some(&text[start..start + text[start..].find("\n}\n").unwrap()])
            })
            .unwrap_or_else(|| panic!("no fn {f}"))
    };
    assert!(body("download_for").contains("mojang::download("));
    assert!(body("download_for_owner").contains("account::launch("));
    assert!(!body("download_for_owner").contains("launch_for_game("));
    // `download_for` is defined once and called once, by `download_for_owner`.
    let mentions: usize = code
        .iter()
        .map(|(_, _, text)| text.matches("download_for(").count())
        .sum();
    assert_eq!(mentions, 2);
    assert!(body("download_for_owner").contains("download_for("));
    // Refilling an incomplete store asks the same gate and names no host or download itself.
    assert!(body("refill_own_store").contains("download_for_owner("));
    assert!(!body("refill_own_store").contains("mojang::"));
    // It runs at start-up, never on Play, which also runs `complete_assets`.
    assert!(body("reuse_prepared_everywhere").contains("refill_own_store("));
    assert!(!body("complete_assets").contains("refill_own_store("));
    for host in [
        "piston-data",
        "libraries.minecraft.net",
        "resources.download.minecraft.net",
    ] {
        for (name, tests, text) in &sources {
            assert!(
                *tests || name == "mojang.rs" || !text.contains(host),
                "{name} names {host}"
            );
        }
    }
}

/// A preparation that reuses the imported game files still reads the assets from the
/// launcher that has them: a Prism Launcher instance's are in the launcher root, linked into
/// the official layout (jan.sonki on Discord: silent, grey title screen since 0.2.48).
#[test]
fn reused_imports_take_the_assets_from_the_launcher_that_has_them() {
    let dir = std::env::temp_dir().join(format!("eclipse-reused-assets-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let client = b"fake client jar";
    let manifest = br#"{"id": "26.3"}"#;
    let write = |path: &str, data: &[u8]| {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    };
    let prism = "PrismLauncher";
    write(
        &format!("{prism}/libraries/com/mojang/minecraft/26.3/minecraft-26.3-client.jar"),
        client,
    );
    write(&format!("{prism}/assets/indexes/34.json"), b"{}");
    let instance = dir.join(prism).join("instances/26.3/minecraft");
    fs::create_dir_all(&instance).unwrap();
    let pins = sources::Pins::read(&serde_json::json!({ "minecraft": {
        "version": "26.3",
        "client_sha1": sources::sha1_hex(client),
        "manifest_sha1": sources::sha1_hex(manifest),
    }}))
    .unwrap();
    let linked = dir.join("linked");
    let fetch = |_: &str| Ok(manifest.to_vec());
    let lines = std::cell::RefCell::new(Vec::new());
    let log = |line: String| lines.borrow_mut().push(line);
    let resolved = sources::resolve(&instance, &pins, &[], &linked, &fetch, &log);
    let folder = assets_folder(resolved, &instance, "26.3", &log);
    assert_eq!(folder, linked);
    assert!(folder.join("assets/indexes/34.json").is_file());

    // Nothing found: the chosen folder, and a line saying what the game will lack.
    let empty = dir.join("empty");
    fs::create_dir_all(&empty).unwrap();
    let resolved = sources::resolve(&empty, &pins, &[], &linked, &fetch, &log);
    assert_eq!(assets_folder(resolved, &empty, "26.3", &log), empty);
    assert!(lines.borrow().last().unwrap().contains("will be silent"));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn resources_without_sounds_are_reported() {
    let dir = std::env::temp_dir().join(format!("eclipse-no-sounds-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("assets/minecraft/textures")).unwrap();
    fs::write(dir.join(RESOURCES_MANIFEST), "{}").unwrap();
    assert!(!sounds_in(&dir));
    fs::write(dir.join("assets/minecraft/sounds.json"), "{}").unwrap();
    assert!(sounds_in(&dir));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn each_mac_picks_the_build_for_its_processor() {
    assert_eq!(
        platform_assets("macos", "aarch64"),
        Some((
            "eclipse-macos-arm64.zip",
            "eclipse-launcher-macos-arm64.zip"
        ))
    );
    assert_eq!(
        platform_assets("macos", "x86_64"),
        Some(("eclipse-macos-x64.zip", "eclipse-launcher-macos-x64.zip"))
    );
    assert_eq!(
        platform_assets("windows", "x86_64"),
        Some((
            "eclipse-windows-x64.zip",
            "eclipse-launcher-windows-x64.zip"
        ))
    );
    assert_eq!(
        platform_assets("windows", "aarch64"),
        Some((
            "eclipse-windows-arm64.zip",
            "eclipse-launcher-windows-arm64.zip"
        ))
    );
    assert_eq!(platform_assets("linux", "aarch64"), None);
    assert_eq!(platform_assets("macos", "x86"), None);
    assert_eq!(
        ASSET,
        platform_assets(std::env::consts::OS, std::env::consts::ARCH).map(|(game, _)| game)
    );
    assert_eq!(
        game_fallback_for("windows", "aarch64"),
        Some("eclipse-windows-x64.zip")
    );
    assert_eq!(game_fallback_for("windows", "x86_64"), None);
    assert_eq!(game_fallback_for("macos", "aarch64"), None);
}

/// The ARM64 Windows launcher installs the ARM64 build where a release has one, and the x64
/// build (under emulation) from the releases that have only that, checked against that file's
/// signed checksum.
#[test]
fn an_arm64_windows_launcher_falls_back_to_the_x64_game() {
    let (arm, x64) = ("eclipse-windows-arm64.zip", "eclipse-windows-x64.zip");
    let asset = |name: &str, url: &str| serde_json::json!({"name": name, "browser_download_url": url, "size": 1});
    let list = serde_json::json!([
        {"tag_name": "v2026.11.05", "assets": [
            asset(x64, "https://example.test/v2026.11.05/eclipse-windows-x64.zip"),
            asset(arm, "https://example.test/v2026.11.05/eclipse-windows-arm64.zip")]},
        {"tag_name": "v2026.11.04", "assets": [
            asset(x64, "https://example.test/v2026.11.04/eclipse-windows-x64.zip")]},
        {"tag_name": "v2026.11.03", "assets": []},
    ]);
    let releases = game_releases_with(&list, Some(arm), Some(x64));
    let urls: Vec<_> = releases
        .iter()
        .map(|r| {
            r.asset
                .as_ref()
                .map(|(url, _)| url.rsplit('/').next().unwrap())
        })
        .collect();
    assert_eq!(urls, [Some(arm), Some(x64), None]);
    let installs: Vec<_> = releases
        .iter()
        .map(|r| asset_installed(r, Some(arm), Some(x64)))
        .collect();
    assert_eq!(installs, [Some(arm), Some(x64), None]);

    // An x64 launcher, on any Windows, never looks at the ARM64 build.
    let x64_only = game_releases_with(&list, Some(x64), None);
    assert!(x64_only[..2]
        .iter()
        .all(|r| r.asset.as_ref().unwrap().0.ends_with(x64)));
}

#[test]
fn a_game_architecture_choice_is_independent_of_the_launcher_process() {
    let (arm, x64) = ("eclipse-windows-arm64.zip", "eclipse-windows-x64.zip");
    // GetNativeSystemInfo reports ARM64 even when std::env::consts::ARCH is x86_64.
    assert!(supports_game_architecture_choice(
        "windows",
        Some("aarch64")
    ));
    assert!(!supports_game_architecture_choice(
        "windows",
        Some("x86_64")
    ));
    assert!(!supports_game_architecture_choice("linux", Some("aarch64")));

    // An x64 launcher keeps the legacy x64 default, but can opt into ARM64.
    assert_eq!(
        game_assets_for("windows", Some("aarch64"), "x86_64", GameArchitecture::Auto),
        Some((x64, None))
    );
    assert_eq!(
        game_assets_for(
            "windows",
            Some("aarch64"),
            "x86_64",
            GameArchitecture::Arm64
        ),
        Some((arm, Some(x64)))
    );

    // The ARM64 launcher defaults to ARM64 and can choose x64 for compatibility.
    assert_eq!(
        game_assets_for(
            "windows",
            Some("aarch64"),
            "aarch64",
            GameArchitecture::Auto
        ),
        Some((arm, Some(x64)))
    );
    assert_eq!(
        game_assets_for("windows", Some("aarch64"), "aarch64", GameArchitecture::X64),
        Some((x64, None))
    );

    // Native x64 Windows and the other platforms keep their old process-based selection.
    assert_eq!(
        game_assets_for("windows", Some("x86_64"), "x86_64", GameArchitecture::Arm64),
        Some((x64, None))
    );
    assert_eq!(
        game_assets_for("linux", None, "x86_64", GameArchitecture::Arm64),
        Some(("eclipse-linux-x86_64.tar.gz", None))
    );
}

#[cfg(windows)]
#[test]
fn windows_native_architecture_detection_sees_the_host_under_emulation() {
    let native = native_windows_architecture().expect("supported Windows machine");
    assert!(matches!(native, "x86_64" | "aarch64"));
    if std::env::var_os("ECLIPSE_TEST_EXPECT_NATIVE_ARM64").is_some() {
        assert_eq!(std::env::consts::ARCH, "x86_64");
        assert_eq!(native, "aarch64");
    }
    if native == "aarch64" {
        assert!(supports_game_architecture_choice("windows", Some(native)));
        let (asset, fallback) = game_assets_for(
            "windows",
            Some(native),
            std::env::consts::ARCH,
            GameArchitecture::Auto,
        )
        .unwrap();
        if std::env::consts::ARCH == "x86_64" {
            assert_eq!((asset, fallback), ("eclipse-windows-x64.zip", None));
        } else {
            assert_eq!(
                (asset, fallback),
                ("eclipse-windows-arm64.zip", Some("eclipse-windows-x64.zip"))
            );
        }
    }
}

#[test]
fn selected_architecture_uses_available_assets_and_only_the_safe_fallback() {
    let arm = "eclipse-windows-arm64.zip";
    let x64 = "eclipse-windows-x64.zip";
    let asset = |name: &str| serde_json::json!({"name": name, "browser_download_url": format!("https://example.test/{name}"), "size": 1});
    let list = serde_json::json!([
        {"tag_name": "v2026.11.05", "assets": [asset(x64), asset(arm)]},
        {"tag_name": "v2026.11.04", "assets": [asset(x64)]},
        {"tag_name": "v2026.11.03", "assets": []}
    ]);

    let arm_choice = game_releases_with(&list, Some(arm), Some(x64));
    assert_eq!(
        arm_choice
            .iter()
            .map(|release| release
                .asset
                .as_ref()
                .and_then(|(url, _)| url.rsplit('/').next()))
            .collect::<Vec<_>>(),
        [Some(arm), Some(x64), None]
    );
    assert_eq!(
        asset_installed(&arm_choice[0], Some(arm), Some(x64)),
        Some(arm)
    );
    assert_eq!(
        asset_installed(&arm_choice[1], Some(arm), Some(x64)),
        Some(x64)
    );
    assert_eq!(asset_installed(&arm_choice[2], Some(arm), Some(x64)), None);

    let x64_choice = game_releases_with(&list, Some(x64), None);
    assert!(x64_choice.iter().take(2).all(|release| release
        .asset
        .as_ref()
        .unwrap()
        .0
        .ends_with(x64)));
    assert_eq!(
        asset_installed(&arm_choice[0], Some(x64), None),
        None,
        "a stale ARM64 release selection must not install under an x64 choice"
    );
}

#[test]
fn selected_game_build_and_safe_fallback_satisfy_the_saved_choice() {
    let fallback = InstalledGameArchitecture {
        requested: GameArchitecture::Arm64,
        binary: GameArchitecture::X64,
    };
    assert!(installed_architecture_matches(
        GameArchitecture::Arm64,
        fallback
    ));
    assert!(installed_architecture_matches(
        GameArchitecture::X64,
        fallback
    ));
    assert!(!installed_architecture_matches(
        GameArchitecture::Arm64,
        InstalledGameArchitecture {
            requested: GameArchitecture::X64,
            binary: GameArchitecture::X64,
        }
    ));
}

#[test]
fn updates_reinstall_only_when_the_selected_game_architecture_needs_a_new_build() {
    let arm_fallback = InstalledGameArchitecture {
        requested: GameArchitecture::Arm64,
        binary: GameArchitecture::X64,
    };
    let old_x64 = InstalledGameArchitecture {
        requested: GameArchitecture::X64,
        binary: GameArchitecture::X64,
    };
    let arm = InstalledGameArchitecture {
        requested: GameArchitecture::Arm64,
        binary: GameArchitecture::Arm64,
    };

    assert!(!game_architecture_install_needed(
        GameArchitecture::Arm64,
        GameArchitecture::X64,
        Some(arm_fallback),
    ));
    assert!(game_architecture_install_needed(
        GameArchitecture::Arm64,
        GameArchitecture::X64,
        Some(old_x64),
    ));
    assert!(game_architecture_install_needed(
        GameArchitecture::Arm64,
        GameArchitecture::Arm64,
        Some(arm_fallback),
    ));
    assert!(!game_architecture_install_needed(
        GameArchitecture::Arm64,
        GameArchitecture::Arm64,
        Some(arm),
    ));
}

#[test]
fn native_mod_builds_report_when_the_selected_game_has_no_matching_compiler() {
    assert_eq!(
        native_mod_architecture_message(GameArchitecture::X64, GameArchitecture::Arm64),
        Some("Native mods cannot build for the selected ARM64 game yet. The available compiler builds x64 only.")
    );
    assert_eq!(
        native_mod_architecture_message(GameArchitecture::Arm64, GameArchitecture::X64),
        Some("Native mods cannot build for the selected x64 game yet. This ARM64 launcher has no matching compiler.")
    );
    assert_eq!(
        native_mod_architecture_message(GameArchitecture::Arm64, GameArchitecture::Arm64),
        None
    );
}

#[test]
fn windows_pe_machine_types_identify_the_installed_game_architecture() {
    let pe = |machine: u16| {
        let mut bytes = vec![0; 0x86];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\x00\x00");
        bytes[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
        bytes
    };
    let dir = std::env::temp_dir().join(format!("eclipse-pe-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let program = dir.join("eclipse-client.exe");
    let read = |bytes: &[u8]| {
        fs::write(&program, bytes).unwrap();
        pe_architecture_file(&program)
    };
    assert_eq!(read(&pe(0x8664)), Some(GameArchitecture::X64));
    assert_eq!(read(&pe(0xaa64)), Some(GameArchitecture::Arm64));
    assert_eq!(read(b"not a PE executable"), None);
    fs::remove_dir_all(&dir).unwrap();
}

/// launcher.yml publishes a release even when its ARM64 build failed. An ARM64 launcher then
/// stays on its version until a release has its own zip: it never takes the x64 one.
#[test]
fn an_arm64_launcher_skips_a_release_without_its_zip() {
    let (arm, x64) = (
        "eclipse-launcher-windows-arm64.zip",
        "eclipse-launcher-windows-x64.zip",
    );
    let asset = |name: &str| serde_json::json!({"name": name, "browser_download_url": format!("https://example.test/{name}"), "size": 1});
    let list = serde_json::json!([
        {"tag_name": "launcher@0.30.0", "assets": [asset(x64)]},
        {"tag_name": "launcher@0.29.0", "assets": [asset(x64), asset(arm)]},
    ]);
    let releases = parse_releases(&list, Some(arm), is_launcher_tag);
    assert!(launcher_update_from(&releases, "launcher@0.29.0").is_none());
    let update = launcher_update_from(&releases, "launcher@0.28.0").unwrap();
    assert_eq!(update.tag, "launcher@0.29.0");
    assert!(update.asset.unwrap().0.ends_with(arm));
}

#[test]
fn the_game_plays_under_its_own_audio_stream_name() {
    let cmd = client_command("v2026.10.13");
    let props = cmd
        .get_envs()
        .find(|(k, _)| *k == "PIPEWIRE_PROPS")
        .map(|(_, v)| v);
    match std::env::var_os("PIPEWIRE_PROPS") {
        Some(_) => assert_eq!(props, None),
        None => assert!(props.flatten().is_some_and(|v| v
            .to_string_lossy()
            .contains("application.name = \"Eclipse\""))),
    }
}

#[test]
fn a_crash_after_the_vulkan_adapter_line_and_before_the_first_frame_is_a_start_crash() {
    let start = [
        "Eclipse: resources loaded in 11.3s; 35723 block states, 1658 items, 67 biomes",
        "Eclipse renderer: Intel(R) HD Graphics 530 (Vulkan, IntegratedGpu)",
    ];
    assert!(vulkan_start_crashed(start));
    // A step line after the adapter keeps it a start crash.
    let step = [start[1], "Eclipse renderer: creating the pipelines"];
    assert!(vulkan_start_crashed(step));
    // A crash after the first frame is the game's, not the renderer start's.
    let later = [start[1], "Eclipse: first frame after 812 ms"];
    assert!(!vulkan_start_crashed(later));
    // Already on DirectX 12, or no renderer yet: nothing to switch.
    let dx12 = ["Eclipse renderer: Intel(R) HD Graphics 530 (Dx12, IntegratedGpu)"];
    assert!(!vulkan_start_crashed(dx12));
    assert!(!vulkan_start_crashed(&start[..1]));
    // Vulkan failed to open and DirectX 12 took over: its line is the last adapter.
    assert!(!vulkan_start_crashed([start[1], dx12[0]]));
}

#[test]
fn a_renderer_start_crash_switches_auto_to_dx12_once() {
    let home = std::env::temp_dir().join(format!("eclipse-dx12-switch-{}", std::process::id()));
    let _ = fs::remove_dir_all(&home);
    // No settings file yet: one with the API alone.
    assert_eq!(switch_from_auto(&home, "dx12"), Ok(true));
    assert_eq!(
        fs::read_to_string(home.join(GRAPHICS_FILE)).unwrap(),
        "graphicsApi:dx12\n"
    );
    // Already DirectX 12, or the player's own Vulkan: left alone.
    assert_eq!(switch_from_auto(&home, "dx12"), Ok(false));
    fs::write(home.join(GRAPHICS_FILE), "sky:false\ngraphicsApi:vulkan\n").unwrap();
    assert_eq!(switch_from_auto(&home, "dx12"), Ok(false));
    // Auto becomes DirectX 12 and the other settings stay.
    fs::write(
        home.join(GRAPHICS_FILE),
        "sky:false\ngraphicsApi:auto\nfog:true\n",
    )
    .unwrap();
    assert_eq!(switch_from_auto(&home, "dx12"), Ok(true));
    assert_eq!(
        fs::read_to_string(home.join(GRAPHICS_FILE)).unwrap(),
        "sky:false\ngraphicsApi:dx12\nfog:true\n"
    );
    let _ = fs::remove_dir_all(&home);
}

/// #4293: an Ivy Bridge on Linux whose Vulkan driver (Mesa's hasvk) crashes at the renderer's
/// start plays on OpenGL next time; Windows keeps DirectX 12 (#746), macOS has nothing else.
#[test]
fn a_vulkan_start_crash_on_linux_switches_auto_to_opengl() {
    assert_eq!(vulkan_crash_fallback("windows").unwrap().0, "dx12");
    assert_eq!(vulkan_crash_fallback("macos"), None);
    let (api, _) = vulkan_crash_fallback("linux").unwrap();
    let home = std::env::temp_dir().join(format!("eclipse-gl-switch-{}", std::process::id()));
    let _ = fs::remove_dir_all(&home);
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join(GRAPHICS_FILE), "graphicsApi:auto\nvsync:true\n").unwrap();
    assert_eq!(switch_from_auto(&home, api), Ok(true));
    assert_eq!(
        fs::read_to_string(home.join(GRAPHICS_FILE)).unwrap(),
        "graphicsApi:opengl\nvsync:true\n"
    );
    // The next crash (OpenGL had no GPU either and Vulkan ran again) leaves it: no loop.
    assert_eq!(switch_from_auto(&home, api), Ok(false));
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn extra_game_arguments_split_like_a_shell() {
    assert!(split_args("").is_empty());
    assert!(split_args("   ").is_empty());
    assert_eq!(split_args("--server-thread"), ["--server-thread"]);
    assert_eq!(
        split_args("--render-distance 32 --server-thread"),
        ["--render-distance", "32", "--server-thread"]
    );
    // Quotes keep a word with spaces together; either kind works.
    assert_eq!(
        split_args("--name 'Two Words' --world \"My World\""),
        ["--name", "Two Words", "--world", "My World"]
    );
    assert_eq!(split_args("--name \"\""), ["--name", ""]);
    // A backslash escapes a space outside quotes, and " and \ inside double quotes.
    assert_eq!(split_args("--name My\\ World"), ["--name", "My World"]);
    assert_eq!(split_args("a\\tb c"), ["atb", "c"]);
    assert_eq!(split_args("\"a\\\"b\" 'c\\d'"), ["a\"b", "c\\d"]);
}

#[test]
fn the_game_arguments_setting_round_trips() {
    let settings = Settings::parse(r#"{"game_args":"--server-thread"}"#).unwrap();
    assert_eq!(settings.game_args, "--server-thread");
    // Unset in an older settings.json: no arguments, and nothing is written for them.
    let old = Settings::parse(r#"{"installed":true}"#).unwrap();
    assert_eq!(old.game_args, "");
    assert!(!serde_json::to_string(&old).unwrap().contains("game_args"));
}

#[test]
fn tags_never_mix() {
    assert!(is_game_tag("v2026.10.13"));
    assert!(!is_game_tag("launcher-v0.2.0"));
    assert!(!is_game_tag("vnext"));
    assert_eq!(launcher_version("launcher-v0.2.0"), Some("0.2.0"));
    assert_eq!(launcher_version("launcher@0.2.0"), Some("0.2.0"));
    assert!(!is_game_tag("launcher@0.2.0"));
    assert!(is_game_tag("v0.2.0"));
    assert!(is_launcher_tag("launcher@0.2.0"));
    assert_eq!(launcher_version("v2026.10.13"), None);
    assert_eq!(launcher_version("launcher-vx"), None);
    let list = serde_json::json!([
        {"tag_name": "v2026.10.12", "assets": [{"name": "eclipse-linux-x86_64.tar.gz", "browser_download_url": "u", "size": 1}]},
        {"tag_name": "launcher-v9.0.0", "assets": [{"name": "eclipse-launcher-linux-x86_64.tar.gz", "browser_download_url": "u", "size": 1}]},
        {"tag_name": "v2026.10.13", "assets": []},
        {"tag_name": "launcher-v0.0.1", "draft": true, "assets": []},
    ]);
    let game: Vec<_> = parse_releases(&list, ASSET, is_game_tag)
        .into_iter()
        .map(|r| r.tag)
        .collect();
    assert_eq!(game, ["v2026.10.13", "v2026.10.12"]);
    let launcher: Vec<_> = parse_releases(&list, LAUNCHER_ASSET, is_launcher_tag)
        .into_iter()
        .map(|r| r.tag)
        .collect();
    assert_eq!(launcher, ["launcher-v9.0.0"]);
}

/// Rolling a release back (`gh release edit <tag> --draft`, docs/release-flow.md) makes the
/// previous one the newest again: it is what `install latest` and the update check offer.
#[test]
fn withdrawn_release_is_never_offered() {
    let asset = ASSET.unwrap_or("eclipse-linux-x86_64.tar.gz");
    let list = serde_json::json!([
        {"tag_name": "v2026.10.15.1", "draft": true, "assets": [{"name": asset, "browser_download_url": "bad", "size": 1}]},
        {"tag_name": "v2026.10.15", "assets": [{"name": asset, "browser_download_url": "good", "size": 1}]},
        {"tag_name": "v2026.10.14", "assets": [{"name": asset, "browser_download_url": "old", "size": 1}]},
    ]);
    let releases = parse_releases(&list, Some(asset), is_game_tag);
    assert!(releases.iter().all(|r| r.tag != "v2026.10.15.1"));
    assert_eq!(
        latest(&releases).map(|r| r.tag.as_str()),
        Some("v2026.10.15")
    );
    let offered = |installed: &[&str]| {
        let installed: Vec<String> = installed.iter().map(|t| t.to_string()).collect();
        eclipse_update(&releases, &installed).map(|r| r.tag)
    };
    assert_eq!(offered(&["v2026.10.14"]), Some("v2026.10.15".into()));
    assert_eq!(offered(&[]), Some("v2026.10.15".into()));
    // Who installed the withdrawn one is not told to "update" to an older version.
    assert_eq!(offered(&["v2026.10.15.1", "v2026.10.14"]), None);
}

/// Every release is published as the release, and launchers update to it. The pre-releases
/// of the old Alpha channel (v0.2.1 on) are only installed by hand: a launcher that
/// had one installed waits for the next release instead of going back.
#[test]
fn updates_follow_every_release_and_leave_the_old_alpha_ones_out() {
    let asset = ASSET.unwrap_or("eclipse-linux-x86_64.tar.gz");
    let list = serde_json::json!([
        {"tag_name": "v0.2.13", "assets": [{"name": asset, "browser_download_url": "n", "size": 1}]},
        {"tag_name": "v0.2.12", "prerelease": true, "assets": [{"name": asset, "browser_download_url": "a", "size": 1}]},
        {"tag_name": "v0.2.0", "assets": [{"name": asset, "browser_download_url": "s", "size": 1}]},
    ]);
    let releases = parse_releases(&list, Some(asset), is_game_tag);
    assert_eq!(
        releases.iter().map(|r| r.prerelease).collect::<Vec<_>>(),
        [false, true, false]
    );
    let offered = |releases: &[Release], installed: &[&str]| {
        let installed: Vec<String> = installed.iter().map(|t| t.to_string()).collect();
        eclipse_update(&installable_updates(releases), &installed).map(|r| r.tag)
    };
    assert_eq!(offered(&releases, &[]), Some("v0.2.13".into()));
    assert_eq!(offered(&releases, &["v0.2.0"]), Some("v0.2.13".into()));
    assert_eq!(offered(&releases, &["v0.2.12"]), Some("v0.2.13".into()));
    assert_eq!(offered(&releases, &["v0.2.13"]), None);
    // Before the first release of the single channel: the old Alpha one stays where it is.
    let before = &releases[1..];
    assert_eq!(offered(before, &["v0.2.0"]), None);
    assert_eq!(offered(before, &["v0.2.12", "v0.2.0"]), None);
}

#[test]
fn the_window_stays_while_playing_unless_picked() {
    assert_eq!(
        Settings::parse("{}").unwrap().while_playing,
        WhilePlaying::Stay
    );
    let hide = Settings::parse(r#"{"while_playing": "hide", "game_args": "--x"}"#).unwrap();
    assert_eq!(hide.while_playing, WhilePlaying::Hide);
    assert_eq!(
        WhilePlaying::parse("Minimise"),
        Some(WhilePlaying::Minimize)
    );
    assert_eq!(WhilePlaying::parse("off"), Some(WhilePlaying::Stay));
    assert_eq!(WhilePlaying::parse("close"), None);
    for w in WhilePlaying::ALL {
        let text = serde_json::to_string(&w).unwrap();
        assert_eq!(text, format!("\"{}\"", w.name()));
    }
    let here = WhilePlaying::Hide.here();
    assert_eq!(here == WhilePlaying::Hide, cfg!(target_os = "macos"));
}

#[test]
fn the_game_architecture_defaults_to_auto_and_persists_manual_choices() {
    assert_eq!(
        Settings::parse("{}").unwrap().game_architecture,
        GameArchitecture::Auto
    );
    let arm = Settings::parse(r#"{"game_architecture":"arm64","game_args":"--x"}"#).unwrap();
    assert_eq!(
        (arm.game_architecture, arm.game_args.as_str()),
        (GameArchitecture::Arm64, "--x")
    );
    let saved = serde_json::to_value(&arm).unwrap();
    assert_eq!(saved["game_architecture"], "arm64");
    assert_eq!(GameArchitecture::parse("X64"), Some(GameArchitecture::X64));
    assert_eq!(GameArchitecture::parse("other"), None);
}

#[test]
fn the_language_follows_the_system_unless_picked() {
    use crate::i18n::Language;
    assert_eq!(Settings::parse("{}").unwrap().language, None);
    let fr = Settings::parse(r#"{"language": "fr"}"#).unwrap();
    assert_eq!(fr.language(), Language::French);
    let saved = serde_json::to_value(Settings::default()).unwrap();
    assert!(saved.get("language").is_none(), "unset is left out");
}

#[test]
fn an_older_files_glass_and_blur_are_dropped_and_the_rest_kept() {
    let old = Settings::parse(r#"{"glass": "frosty", "blur": 8.5, "game_args": "--x"}"#).unwrap();
    assert_eq!(old.game_args, "--x");
    let saved = serde_json::to_value(&old).unwrap();
    assert!(saved.get("glass").is_none() && saved.get("blur").is_none());
}

#[test]
fn the_accent_is_amber_unless_picked() {
    use crate::themes::Accent;
    assert_eq!(Settings::parse("{}").unwrap().accent, Accent::AMBER);
    let teal =
        Settings::parse(r#"{"accent": "teal", "background": "background-0123456789ab.png"}"#)
            .unwrap();
    assert_eq!(
        (teal.accent, teal.background.as_deref()),
        (Accent::TEAL, Some("background-0123456789ab.png"))
    );
}

#[test]
fn the_appearance_is_dark_unless_picked() {
    assert_eq!(Settings::parse("{}").unwrap().appearance, Appearance::Dark);
    let light = Settings::parse(r#"{"appearance": "light", "game_args": "--x"}"#).unwrap();
    assert_eq!(light.appearance, Appearance::Light);
    let mut settings = Settings::default();
    settings.appearance = Appearance::System;
    let text = serde_json::to_string(&settings).unwrap();
    assert!(text.contains(r#""appearance":"system""#), "{text}");
}

#[test]
fn a_value_this_launcher_cant_read_falls_back_alone() {
    let future = Settings::parse(
        r#"{"appearance": "sepia", "while_playing": "fade", "game_architecture": "future",
            "accent": "plaid", "language": "tlh", "keep_versions": "two",
            "game_args": "--x", "selected": "v0.2.0"}"#,
    )
    .unwrap();
    assert_eq!(future.appearance, Appearance::Dark);
    assert_eq!(future.while_playing, WhilePlaying::Stay);
    assert_eq!(future.game_architecture, GameArchitecture::Auto);
    assert_eq!(future.accent, crate::themes::Accent::AMBER);
    assert_eq!(future.language, None);
    assert_eq!(future.keep_versions, None);
    assert_eq!(
        (future.game_args.as_str(), future.default_version.as_deref()),
        ("--x", Some("v0.2.0"))
    );
}

#[test]
fn settings_that_cant_be_read_are_kept_and_updates_keep_every_change() {
    let dir = std::env::temp_dir().join(format!("eclipse-settings-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    let damaged = r#"{"selected": "v0.2.0", "game_args": "--x""#;
    fs::write(&path, damaged).unwrap();
    let loaded = Settings::load_from(&path);
    assert_eq!(loaded.default_version, None);
    assert!(loaded.save_to(&path).is_err());
    assert!(Settings::update_at(&path, |s| s.game_args = "--y".into()).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), damaged);

    fs::write(&path, r#"{"selected": "v0.2.0", "keep_versions": 0}"#).unwrap();
    std::thread::scope(|threads| {
        for _ in 0..8 {
            threads.spawn(|| {
                Settings::update_at(&path, |s| {
                    s.keep_versions = Some(s.keep_versions.unwrap_or_default() + 1)
                })
                .unwrap()
            });
        }
    });
    let after = Settings::load_from(&path);
    assert_eq!(
        (after.default_version.as_deref(), after.keep_versions),
        (Some("v0.2.0"), Some(8))
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_old_update_channel_settings_are_dropped_and_the_rest_kept() {
    for old in [
        r#"{"update_channel": "alpha", "game_args": "--x", "selected": "v0.2.0"}"#,
        r#"{"update_channel": "stable", "game_args": "--x", "selected": "v0.2.0"}"#,
        r#"{"beta_builds": true, "game_args": "--x", "selected": "v0.2.0"}"#,
        r#"{"beta_builds": false, "update_channel": "nightly", "game_args": "--x", "selected": "v0.2.0"}"#,
    ] {
        let settings = Settings::parse(old).unwrap();
        assert_eq!(settings.game_args, "--x", "{old}");
        assert_eq!(settings.default_version.as_deref(), Some("v0.2.0"), "{old}");
        let saved = serde_json::to_value(&settings).unwrap();
        assert!(saved.get("update_channel").is_none(), "{old}");
        assert!(saved.get("beta_builds").is_none(), "{old}");
    }
}

#[test]
fn the_version_is_the_one_the_release_workflow_injects() {
    assert_eq!(version_or(Some("0.4.2")), "0.4.2");
    assert_eq!(version_or(Some("")), env!("CARGO_PKG_VERSION"));
    assert_eq!(version_or(None), env!("CARGO_PKG_VERSION"));
    // Self-update compares releases with that version: this launcher's own release is no update.
    let r = |tag: String| Release {
        tag,
        name: String::new(),
        body: String::new(),
        date: String::new(),
        time: String::new(),
        asset: Some(("u".into(), 1)),
        prerelease: false,
        nightly: None,
    };
    assert!(launcher_update(&[r(own_tag())]).is_none());
    assert_eq!(launcher_version(&own_tag()), Some(VERSION));
    assert_eq!(
        parse_version_output(&format!("eclipse-launcher {VERSION}\n")),
        Some(VERSION)
    );
}

#[test]
fn an_older_nightly_installs_as_a_rollback_with_a_warning() {
    let nightly = |tag: &str| Release {
        tag: tag.into(),
        name: String::new(),
        body: String::new(),
        date: String::new(),
        time: String::new(),
        asset: Some(("u".into(), 1)),
        prerelease: false,
        nightly: Some("nightly".into()),
    };
    let installed = [
        "v0.2.81".to_owned(),
        "nightly-2026-10-28-0c02067".to_owned(),
    ];
    let warning = nightly_downgrade(&nightly("nightly-2026-10-21-aaaaaaa"), &installed).unwrap();
    assert!(
        warning.contains("older than nightly-2026-10-28-0c02067"),
        "{warning}"
    );
    assert!(nightly_downgrade(&nightly("nightly-2026-10-29-bbbbbbb"), &installed).is_none());
    assert!(nightly_downgrade(&nightly("nightly-2026-10-28-0c02067"), &installed).is_none());
    // No Nightly installed, or a release: nothing to warn about.
    assert!(nightly_downgrade(&nightly("nightly-2026-10-21-aaaaaaa"), &installed[..1]).is_none());
    let mut release = nightly("v0.2.80");
    release.nightly = None;
    assert!(nightly_downgrade(&release, &installed).is_none());
}

#[test]
fn a_launcher_knows_which_line_it_was_released_on() {
    // launcher.yml gives the tag; before it did, a release had only its `launcher-v` version.
    assert_eq!(
        own_tag_of(Some("launcher@0.2.3"), Some("0.2.3")),
        "launcher@0.2.3"
    );
    assert_eq!(
        own_tag_of(Some("launcher-v0.17.0"), Some("0.17.0")),
        "launcher-v0.17.0"
    );
    assert_eq!(own_tag_of(None, Some("0.16.0")), "launcher-v0.16.0");
    assert_eq!(own_tag_of(Some(""), Some("0.16.0")), "launcher-v0.16.0");
    // A build from source is on the 0.2 line.
    assert_eq!(
        own_tag_of(None, None),
        format!("launcher@{}", env!("CARGO_PKG_VERSION"))
    );
}

/// The update path to 0.2 (docs/versioning.md): launchers from before it take the last
/// `launcher-v` release, which takes every `launcher@` one.
#[test]
fn launchers_from_before_0_2_update_to_it() {
    let r = |tag: &str| Release {
        tag: tag.into(),
        name: String::new(),
        body: String::new(),
        date: String::new(),
        time: String::new(),
        asset: Some(("u".into(), 1)),
        prerelease: false,
        nightly: None,
    };
    let mut list = vec![
        r("launcher-v0.16.0"),
        r("launcher@0.2.0"),
        r("launcher-v0.17.0"),
        r("launcher@0.2.1"),
    ];
    list.sort_by(|a, b| compare_versions(&b.tag, &a.tag));
    let tags: Vec<_> = list.iter().map(|r| r.tag.as_str()).collect();
    assert_eq!(
        tags,
        [
            "launcher@0.2.1",
            "launcher@0.2.0",
            "launcher-v0.17.0",
            "launcher-v0.16.0"
        ]
    );
    let offered = |own: &str| launcher_update_from(&list, own).map(|r| r.tag);
    assert_eq!(offered("launcher-v0.16.0"), Some("launcher@0.2.1".into()));
    assert_eq!(offered("launcher-v0.17.0"), Some("launcher@0.2.1".into()));
    assert_eq!(offered("launcher@0.2.0"), Some("launcher@0.2.1".into()));
    assert_eq!(offered("launcher@0.2.1"), None);
    assert_eq!(offered("launcher@0.3.0-rc.1"), None);
    // The old launchers' own check only sees `launcher-v` tags: 0.17.0 is their way over.
    let old_scheme: Vec<_> = list
        .iter()
        .filter(|r| r.tag.starts_with(OLD_LAUNCHER_TAG_PREFIX))
        .cloned()
        .collect();
    assert_eq!(
        launcher_update_from(&old_scheme, "launcher-v0.16.0").map(|r| r.tag),
        Some("launcher-v0.17.0".into())
    );
}

/// A launcher in an AppImage updates with the AppImage's own file: the `.tar.gz` binary would
/// be written inside the AppImage's read-only mount.
#[test]
fn an_appimage_updates_with_the_appimage() {
    assert_eq!(launcher_asset_running_from(false), LAUNCHER_ASSET);
    assert_eq!(launcher_asset_running_from(true), LAUNCHER_APPIMAGE_ASSET);
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    assert_eq!(
        launcher_asset_running_from(true),
        Some("eclipse-launcher-linux-x86_64.AppImage")
    );
}

/// A program opened inside a `.zip` runs from the temporary folder, under a name that depends on
/// the tool, and is deleted later: an update written there is lost, so such a copy is told to
/// download the launcher again (#9287). A folder of the same look elsewhere is a real install.
#[test]
fn a_launcher_run_from_the_temp_folder_is_a_temporary_copy() {
    let temp = Path::new("/tmp/t");
    let inside = |dir: &str| in_temp_folder(Path::new(dir), temp);
    assert!(inside("/tmp/t"));
    assert!(inside(
        "/tmp/t/Temp12_eclipse-launcher-windows-x64.zip/inner"
    ));
    assert!(inside("/tmp/t/7zO8A1B2C3D"));
    assert!(!inside("/tmp/tools"));
    assert!(!inside("/home/me/Temp1_eclipse.zip"));
}

/// The same folder spelled two ways, as %TEMP% and the launcher's own path can be on an account
/// with a short name: `temp_dir()` as the system gives it against its resolved form, and a
/// spelling through `..`, which a literal comparison misses on every system.
#[test]
fn the_temp_folder_is_recognized_however_it_is_spelled() {
    let root = scratch("temp-spelling");
    let temp = root.join("temp");
    let app = temp.join("Temp1_eclipse.zip");
    let outside = root.join("installed");
    fs::create_dir_all(&app).unwrap();
    fs::create_dir_all(&outside).unwrap();

    assert!(in_temp_folder(
        &app,
        &root.join("installed").join("..").join("temp")
    ));
    assert!(!in_temp_folder(&outside, &temp));

    let system = std::env::temp_dir();
    let long = system.canonicalize().unwrap();
    let copy = long.join(format!("eclipse-temp-copy-{}", std::process::id()));
    fs::create_dir_all(&copy).unwrap();
    let found = in_temp_folder(&copy, &system);
    let _ = fs::remove_dir_all(&copy);
    assert!(found);
    let _ = fs::remove_dir_all(root);
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("eclipse-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn stage_named(work: &Path, pid: u32) -> PathBuf {
    let stage = work.join(format!("{pid}-0123456789abcdef0123456789abcdef"));
    fs::create_dir_all(&stage).unwrap();
    fs::write(stage.join("download.zip"), b"archive").unwrap();
    stage
}

/// A crashed launcher's stage goes, and nothing another process may still use does: a live
/// process's however old, a recent one whose process has gone, and one handed to an installer
/// after sitting in a long-lived window, though its folder is old.
#[test]
fn only_abandoned_stages_are_cleaned_up() {
    let work = scratch("stage-cleanup");
    let live = stage_named(&work, 100);
    let crashed = stage_named(&work, 200);
    let handed_over = stage_named(&work, 300);
    let old_launcher_file = work.join("download");
    fs::write(&old_launcher_file, b"partial").unwrap();
    let is_alive = |pid: u32| pid == 100;
    let later = std::time::SystemTime::now() + 2 * STAGE_GRACE;
    let marker = fs::File::create(handed_over.join(HANDED_OVER)).unwrap();
    marker.set_modified(later).unwrap();
    drop(marker);

    remove_abandoned_stages(&work, std::time::SystemTime::now(), is_alive);
    assert!(
        crashed.exists(),
        "gone only just now: its installer may be working"
    );

    remove_abandoned_stages(&work, later, is_alive);
    assert_eq!(fs::read(live.join("download.zip")).unwrap(), b"archive");
    assert!(handed_over.join("download.zip").exists());
    assert!(!crashed.exists());
    assert!(!old_launcher_file.exists());
    let _ = fs::remove_dir_all(work);
}

/// Each stage is its own folder, and removing one leaves another process's as it was. Only a
/// path inside a stage folder removes anything.
#[test]
fn removing_a_stage_leaves_the_others_alone() {
    let work = scratch("stage-remove");
    let mine = new_stage_folder(&work).unwrap();
    let theirs = new_stage_folder(&work).unwrap();
    assert_ne!(mine, theirs);
    fs::write(theirs.join("download.zip"), b"their download").unwrap();
    fs::write(mine.join("download.zip"), b"mine").unwrap();

    remove_stage(&work, &work.join("unpacked").join("eclipse-launcher"));
    remove_stage(&work, &mine.join("..").join("eclipse-launcher"));
    remove_stage(&work, &work);
    assert!(mine.exists() && theirs.exists());

    remove_stage(&work, &mine.join("download.zip"));
    assert!(!mine.exists());
    assert_eq!(
        fs::read(theirs.join("download.zip")).unwrap(),
        b"their download"
    );
    let _ = fs::remove_dir_all(work);
}

/// A download that doesn't match keeps nothing, not even a temporary file, and doesn't touch a
/// finished file already there. One that matches appears under its name only.
#[test]
fn a_download_takes_its_name_only_after_the_hash_check() {
    let dir = scratch("save-checked");
    let to = dir.join("download.zip");
    let bytes: &[u8] = b"the launcher";
    let right = eclipse_release_sign::sha256_hex(bytes);
    let wrong = eclipse_release_sign::sha256_hex(b"something else");
    fs::write(&to, b"finished earlier").unwrap();

    assert!(save_checked(bytes, 0, "asset", &wrong, &to, &|_: f32| {}).is_err());
    assert_eq!(fs::read(&to).unwrap(), b"finished earlier");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);

    save_checked(bytes, 0, "asset", &right, &to, &|_: f32| {}).unwrap();
    assert_eq!(fs::read(&to).unwrap(), bytes);
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_process_that_has_exited_is_not_alive() {
    assert!(process_alive(std::process::id()));
    // The test binary itself, which exits at once on an option it doesn't know.
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid = child.id();
    child.wait().unwrap();
    drop(child);
    assert!(!process_alive(pid));
}

/// A launcher whose folder takes no new file (a disk image, macOS's translocated copy of an
/// app it was not moved from) can't put the new one in place.
#[cfg(unix)]
#[test]
fn a_read_only_folder_takes_no_update() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("eclipse-read-only-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    assert!(writable(&dir));
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    // root writes anywhere.
    if unsafe { libc::geteuid() } != 0 {
        assert!(!writable(&dir));
    }
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        fs::read_dir(&dir).unwrap().count(),
        0,
        "the probe is removed"
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// `$APPIMAGE` names the file the runtime mounted; a path that is not there (moved or deleted
/// since the launcher started) is no AppImage, and the launcher updates its own binary again.
/// Nor is one inherited from another AppImage's program: the binary has to run from `$APPDIR`.
// AppImages are Linux's, and the paths below are absolute only on Unix.
#[cfg(unix)]
#[test]
fn only_an_appimage_file_counts() {
    use std::ffi::OsStr;
    let mount = OsStr::new("/tmp/.mount_Eclipse123");
    let exe = Path::new("/tmp/.mount_Eclipse123/usr/bin/eclipse-launcher");
    assert_eq!(appimage_path_in(None, Some(mount), exe), None);
    assert_eq!(
        appimage_path_in(Some(OsStr::new("Eclipse.AppImage")), Some(mount), exe),
        None,
        "a relative path is no $APPIMAGE"
    );
    assert_eq!(
        appimage_path_in(
            Some(OsStr::new("/nonexistent/Eclipse.AppImage")),
            Some(mount),
            exe
        ),
        None
    );
    let file = std::env::temp_dir().join(format!("eclipse-appimage-{}", std::process::id()));
    std::fs::write(&file, b"#!AppImage").unwrap();
    assert_eq!(
        appimage_path_in(Some(file.as_os_str()), Some(mount), exe),
        Some(file.clone())
    );
    // Inherited from another AppImage: this launcher is installed elsewhere.
    let elsewhere = Path::new("/home/a/.local/bin/eclipse-launcher");
    assert_eq!(
        appimage_path_in(Some(file.as_os_str()), Some(mount), elsewhere),
        None
    );
    assert_eq!(appimage_path_in(Some(file.as_os_str()), None, exe), None);
    let _ = std::fs::remove_file(&file);
}

#[test]
fn game_0_2_follows_every_date_release() {
    assert_eq!(compare_versions("v0.2.0", "v2026.10.19.15"), Greater);
    assert_eq!(compare_versions("v0.2.0", "v2026.10.21"), Greater);
    assert_eq!(compare_versions("v2099.12.31.9", "v0.2.0"), Less);
    assert_eq!(compare_versions("v0.2.1", "v0.2.0"), Greater);
    assert_eq!(compare_versions("v0.2.10", "v0.2.9"), Greater);
    assert_eq!(compare_versions("v0.3.0", "v0.2.57"), Greater);
    assert_eq!(compare_versions("v0.2.0", "v0.2.0"), Equal);
    // Pre-releases come before their release and after the one before.
    assert_eq!(compare_versions("v0.3.0-rc.1", "v0.3.0"), Less);
    assert_eq!(compare_versions("v0.3.0-rc.1", "v0.2.9"), Greater);
    assert_eq!(compare_versions("v0.3.0-beta.11", "v0.3.0-beta.2"), Greater);
    assert_eq!(compare_versions("v0.3.0-rc.1", "v0.3.0-beta.9"), Greater);
    assert_eq!(compare_versions("v0.3.0-beta.1", "v0.3.0-beta"), Greater);
    assert_eq!(compare_versions("v0.3.0-alpha", "v0.3.0-1"), Greater);
    assert_eq!(compare_versions("v0.2.1-beta.1", "v2026.10.21"), Greater);
    // Launcher and game tags of 0.2 compare alike; old launcher tags stay below.
    assert_eq!(
        compare_versions("launcher@0.2.0", "launcher-v0.16.0"),
        Greater
    );
    assert_eq!(compare_versions("launcher@0.2.0", "v0.2.0"), Equal);

    let asset = ASSET.unwrap_or("eclipse-linux-x86_64.tar.gz");
    let list = serde_json::json!([
        {"tag_name": "v0.2.0", "prerelease": true, "assets": [{"name": asset, "browser_download_url": "b", "size": 1}]},
        {"tag_name": "v2026.10.21", "prerelease": true, "assets": [{"name": asset, "browser_download_url": "d", "size": 1}]},
        {"tag_name": "v2026.10.19.15", "assets": [{"name": asset, "browser_download_url": "s", "size": 1}]},
    ]);
    let releases = parse_releases(&list, Some(asset), is_game_tag);
    let installed = ["v2026.10.19.15".to_string(), "v2026.10.12".to_string()];
    // A pre-release 0.2.0 is not an update; once published as the release, it is one.
    let offered = |releases: &[Release]| {
        eclipse_update(&installable_updates(releases), &installed).map(|r| r.tag)
    };
    assert_eq!(offered(&releases), None);
    let mut released = releases.clone();
    released[0].prerelease = false;
    assert_eq!(offered(&released), Some("v0.2.0".into()));
    let mut sorted = vec![
        "v2026.10.19.15".to_string(),
        "v0.2.1".into(),
        "v0.2.0".into(),
        "v2026.10.21".into(),
    ];
    sorted.sort_by(|a, b| compare_versions(b, a));
    assert_eq!(
        sorted,
        ["v0.2.1", "v0.2.0", "v2026.10.21", "v2026.10.19.15"]
    );
}

#[test]
fn the_fallback_looks_for_0_2_launchers_first() {
    // The asset the launcher reading this list installs with (the AppImage's own when it runs
    // from one), as launcher_releases_in_with looks for it.
    let asset = launcher_asset().unwrap_or("unsupported");
    let release = |tag: &str| serde_json::json!({"tag_name": tag, "assets": [{"name": asset, "browser_download_url": "u", "size": 1}]});
    let asked = std::cell::RefCell::new(Vec::new());
    let lookup = |with_new: bool| {
        asked.borrow_mut().clear();
        launcher_releases_in_with(&serde_json::json!([]), |url| {
            asked
                .borrow_mut()
                .push(url.rsplit('/').next().unwrap_or("").to_string());
            if url.ends_with("matching-refs/tags/launcher@") {
                Ok(serde_json::json!(if with_new {
                    vec![serde_json::json!({"ref": "refs/tags/launcher@0.2.0"})]
                } else {
                    Vec::new()
                }))
            } else if url.ends_with("matching-refs/tags/launcher-v") {
                Ok(serde_json::json!([{"ref": "refs/tags/launcher-v0.17.0"}]))
            } else {
                Ok(release(url.rsplit('/').next().unwrap_or("")))
            }
        })
        .into_iter()
        .map(|r| r.tag)
        .collect::<Vec<_>>()
    };
    assert_eq!(lookup(true), ["launcher@0.2.0"]);
    assert_eq!(*asked.borrow(), ["launcher@", "launcher@0.2.0"]);
    // Before the first `launcher@` release, the `launcher-v` ones.
    assert_eq!(lookup(false), ["launcher-v0.17.0"]);
    assert_eq!(
        *asked.borrow(),
        ["launcher@", "launcher-v", "launcher-v0.17.0"]
    );
}

#[test]
fn launcher_tags_from_matching_refs_newest_first() {
    let refs = serde_json::json!([
        {"ref": "refs/tags/launcher-v0.1.9"},
        {"ref": "refs/tags/launcher-v0.1.10"},
        {"ref": "refs/tags/launcher-vnext"},
        {"ref": "refs/tags/v2026.10.15"},
        {"ref": "refs/tags/launcher-v0.2.0"},
        {"ref": "refs/tags/launcher@0.2.0"},
    ]);
    assert_eq!(
        newest_launcher_tags(&refs),
        [
            "launcher@0.2.0",
            "launcher-v0.2.0",
            "launcher-v0.1.10",
            "launcher-v0.1.9"
        ]
    );
}

#[test]
fn versions_compare_by_number() {
    assert_eq!(compare_versions("v2026.10.13", "v2026.10.12"), Greater);
    assert_eq!(compare_versions("v2026.9.30", "v2026.10.1"), Less);
    assert_eq!(compare_versions("v2026.10.13.1", "v2026.10.13"), Greater);
    assert_eq!(compare_versions("0.10.0", "0.9.9"), Greater);
    assert_eq!(compare_versions("launcher-v1.2", "1.2.0"), Equal);
    assert_eq!(compare_versions("1.0.0-rc1", "1.0.0"), Equal);
}

#[test]
fn updates() {
    let r = |tag: &str| Release {
        tag: tag.into(),
        name: String::new(),
        body: String::new(),
        date: String::new(),
        time: String::new(),
        asset: Some(("u".into(), 1)),
        prerelease: false,
        nightly: None,
    };
    let releases = [r("v2026.10.13"), r("v2026.10.12")];
    assert_eq!(
        eclipse_update(&releases, &["v2026.10.12".into()]).map(|r| r.tag),
        Some("v2026.10.13".into())
    );
    assert!(eclipse_update(&releases, &["v2026.10.13".into()]).is_none());
    assert!(launcher_update(&[r("launcher-v0.0.1")]).is_none());
    assert!(launcher_update(&[r("launcher@99.0.0")]).is_some());
    assert!(launcher_update_from(&[r("launcher-v99.0.0")], "launcher-v0.16.0").is_some());
    let installed = ["v2026.10.12".to_string(), "v2026.10.1".to_string()];
    assert!(select_after_update(Some("v2026.10.12"), &installed));
    assert!(!select_after_update(Some("v2026.10.1"), &installed));
    assert!(select_after_update(None, &installed));
    assert!(!select_after_update(
        Some("imported-2026-11-01-abcdef0"),
        &["imported-2026-11-01-abcdef0".to_string()]
    ));
}

/// The Nightly is the only build Eclipse ships (docs/release-flow.md): every launcher takes the
/// current one as its update, over every release; the last week's are listed, never offered.
#[test]
fn every_launcher_updates_to_the_current_nightly() {
    let asset = ASSET.unwrap_or("eclipse-linux-x86_64.tar.gz");
    let build =
        |url: &str| serde_json::json!([{"name": asset, "browser_download_url": url, "size": 1}]);
    let list = serde_json::json!([
        {"tag_name": "nightly", "name": "Nightly 2026-10-28 · 9b772b2", "prerelease": true,
         "published_at": "2026-10-28T09:58:01Z", "assets": build("n")},
        {"tag_name": "launcher@0.2.21", "assets": []},
        {"tag_name": "v0.2.95", "assets": build("r")},
        {"tag_name": "nightly-2026-10-28", "name": "Nightly 2026-10-28 · 83f15fa", "prerelease": true,
         "published_at": "2026-10-28T07:54:57Z", "assets": build("d")},
        {"tag_name": "nightly-2026-10-27", "name": "Nightly 2026-10-27 · 7954902", "prerelease": true,
         "published_at": "2026-10-28T00:15:40Z", "assets": build("e")},
    ]);
    let releases = game_releases(&list);
    let listed: Vec<_> = releases
        .iter()
        .map(|r| (r.tag.as_str(), r.nightly.as_deref(), r.prerelease))
        .collect();
    assert_eq!(
        listed,
        [
            ("nightly-2026-10-28-9b772b2", Some("nightly"), false),
            (
                "nightly-2026-10-28-83f15fa",
                Some("nightly-2026-10-28"),
                true
            ),
            (
                "nightly-2026-10-27-7954902",
                Some("nightly-2026-10-27"),
                true
            ),
            ("v0.2.95", None, false),
        ]
    );
    let offered = |installed: &[&str]| {
        let installed: Vec<String> = installed.iter().map(|t| t.to_string()).collect();
        eclipse_update(&installable_updates(&releases), &installed).map(|r| r.tag)
    };
    let current = Some("nightly-2026-10-28-9b772b2".to_string());
    // A player of the release channel, a new player, one on an older Nightly of the same day.
    assert_eq!(offered(&["v0.2.95", "v0.2.94"]), current);
    assert_eq!(offered(&[]), current);
    assert_eq!(offered(&["nightly-2026-10-28-83f15fa", "v0.2.95"]), current);
    assert_eq!(offered(&["nightly-2026-10-28-9b772b2"]), None);
    // Nightlies come after every release in every list.
    assert!(compare_versions("nightly-2026-10-27-7954902", "v0.2.95").is_gt());
    assert!(compare_versions("nightly-2026-10-28-9b772b2", "nightly-2026-10-27-7954902").is_gt());
}

/// Players move to the Nightly at their next check: whoever was on the newest release, on a
/// Nightly, or on nothing. A release picked by hand stays picked.
#[test]
fn players_of_the_newest_release_move_to_the_nightly() {
    let installed = [
        "nightly-2026-10-27-7954902".to_string(),
        "v0.2.95".into(),
        "v0.2.90".into(),
    ];
    assert!(select_after_update(Some("v0.2.95"), &installed));
    assert!(select_after_update(
        Some("nightly-2026-10-27-7954902"),
        &installed
    ));
    assert!(select_after_update(None, &installed));
    assert!(!select_after_update(Some("v0.2.90"), &installed));
    assert!(select_after_update(
        Some("v0.2.95"),
        &["v0.2.95".to_string()]
    ));
}

#[test]
fn new_versions_install_automatically_unless_turned_off() {
    // No settings file, an old one that never mentioned it, and 0.1.1's default `false`.
    assert!(Settings::parse("").unwrap().auto_install());
    assert!(Settings::parse("{}").unwrap().auto_install());
    assert!(
        Settings::parse(r#"{"selected": "v2026.10.13", "auto_install": false}"#)
            .unwrap()
            .auto_install()
    );
    assert!(Settings::parse(r#"{"install_new_versions": true}"#)
        .unwrap()
        .auto_install());
    assert!(!Settings::parse(r#"{"install_new_versions": false}"#)
        .unwrap()
        .auto_install());
    // Saving after the player turned it off keeps it off.
    let mut off = Settings::default();
    off.install_new_versions = Some(false);
    assert!(!Settings::parse(&serde_json::to_string(&off).unwrap())
        .unwrap()
        .auto_install());
    // The selection rule is unchanged: the new version is selected only from the newest.
    let installed = ["v2026.10.12".to_string(), "v2026.10.1".to_string()];
    assert!(select_after_update(Some("v2026.10.12"), &installed));
    assert!(!select_after_update(Some("v2026.10.1"), &installed));
}

#[test]
fn links_fall_back_to_the_built_in_ones() {
    let defaults = Links::default();
    assert_eq!(defaults.discord, "");
    assert_eq!(defaults.sponsor, "");
    assert_eq!(defaults.website, "");
    // The file on main says the same thing as the defaults.
    assert_eq!(Links::parse(include_str!("../../links.json")), defaults);
    // Unreadable or missing, offline included: the defaults.
    assert_eq!(Links::parse(""), defaults);
    assert_eq!(Links::parse("<html>404</html>"), defaults);
    assert_eq!(Links::parse("[1]"), defaults);
    assert_eq!(Links::parse(r#"{"discord": 5}"#), defaults);
    // A key that is missing keeps its default; a new address replaces it; empty hides it.
    assert_eq!(
        Links::parse(r#"{"discord": "https://x.y/z"}"#),
        Links {
            discord: "https://x.y/z".into(),
            ..defaults.clone()
        }
    );
    assert_eq!(
        Links::parse(r#"{"discord": "", "sponsor": " ", "website": "https://x.y"}"#),
        Links {
            website: "https://x.y".into(),
            ..defaults.clone()
        }
    );
    // A key this launcher does not know is ignored.
    assert_eq!(Links::parse(r#"{"mastodon": "https://x.y/z"}"#), defaults);
}

#[test]
fn a_new_version_reuses_resources_prepared_by_a_compatible_one() {
    let versions =
        std::env::temp_dir().join(format!("eclipse-launcher-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&versions);
    let oracle = "mods/eclipse/src/test/java/dev/eclipselab/parity/VanillaHeightmapOracle.java";
    let camera = "mods/eclipse/src/test/java/dev/eclipselab/parity/VanillaCameraOracle.java";
    let collision = "mods/eclipse/src/test/java/dev/eclipselab/parity/VanillaCollisionOracle.java";
    let write = |tag: &str, path: &str, text: &str| {
        let path = versions.join(tag).join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    let release = |tag: &str, minecraft: &str| {
        write(tag, INSTALL_MARK, "");
        let lock = serde_json::json!({"minecraft": {"version": minecraft, "client_sha1": "c", "server_sha1": "s"}});
        write(tag, "research-lock.json", &lock.to_string());
        write(tag, "tools/prepare_native_client.py", "schema 7");
        write(tag, oracle, "heightmap");
        write(tag, collision, "collision");
        write(tag, camera, tag); // Not an exporter: it may change freely.
    };
    let prepared = |tag: &str, minecraft: &str| {
        let manifest =
            serde_json::json!({"schema": 7, "collision_exporters_sha256": {collision: "x"}});
        write(
            tag,
            &format!(".local/eclipse-client/{minecraft}/{RESOURCES_MANIFEST}"),
            &manifest.to_string(),
        );
        write(
            tag,
            &format!(".local/eclipse-client/{minecraft}/worldgen/biomes.json"),
            "{}",
        );
    };
    let is_prepared = |tag: &str| {
        resources_at(&versions.join(tag))
            .join(RESOURCES_MANIFEST)
            .exists()
    };

    release("v2026.10.13", "26.3");
    prepared("v2026.10.13", "26.3");
    release("v2026.10.12", "26.2");
    prepared("v2026.10.12", "26.2");
    release("v2026.10.13.1", "26.3");
    assert_eq!(
        reuse_prepared(&versions, "v2026.10.13.1"),
        Some("v2026.10.13".into())
    );
    assert!(is_prepared("v2026.10.13.1"));
    assert!(resources_at(&versions.join("v2026.10.13.1"))
        .join("worldgen/biomes.json")
        .exists());
    let left: Vec<_> = fs::read_dir(versions.join("v2026.10.13.1/.local/eclipse-client"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(left, ["26.3"]);
    assert_eq!(reuse_prepared(&versions, "v2026.10.13.1"), None);

    // Another Minecraft version, a changed exporter or a changed prepare script: prepare again.
    release("v2026.10.14", "26.4");
    assert_eq!(reuse_prepared(&versions, "v2026.10.14"), None);
    release("v2026.10.15", "26.3");
    write("v2026.10.15", collision, "collision, fixed");
    assert_eq!(reuse_prepared(&versions, "v2026.10.15"), None);
    release("v2026.10.16", "26.3");
    write("v2026.10.16", "tools/prepare_native_client.py", "schema 8");
    assert_eq!(reuse_prepared(&versions, "v2026.10.16"), None);
    assert!(!is_prepared("v2026.10.15") && !is_prepared("v2026.10.16"));

    // A version an older launcher installed without preparing it is caught up by the sweep
    // that starting the window, `versions` and `update` run; the incompatible ones stay as
    // they are, and a second sweep has nothing to do.
    release("v2026.10.17", "26.3");
    assert_eq!(
        reuse_prepared_in(&versions),
        [("v2026.10.17".to_string(), "v2026.10.13.1".to_string())]
    );
    assert!(is_prepared("v2026.10.17"));
    assert!(!is_prepared("v2026.10.15") && !is_prepared("v2026.10.16"));
    assert!(reuse_prepared_in(&versions).is_empty());

    // Versions with eclipse-prepare: its sources are the preparer, so the same sources
    // reuse each other's resources, and a change or a version without them prepares again.
    let crate_file = "crates/eclipse-prepare/src/lib.rs";
    release("v2026.10.18", "26.3");
    write("v2026.10.18", crate_file, "rust preparer");
    assert_eq!(reuse_prepared(&versions, "v2026.10.18"), None);
    prepared("v2026.10.18", "26.3");
    release("v2026.10.19", "26.3");
    write("v2026.10.19", crate_file, "rust preparer");
    assert_eq!(
        reuse_prepared(&versions, "v2026.10.19"),
        Some("v2026.10.18".into())
    );
    release("v2026.10.20", "26.3");
    write("v2026.10.20", crate_file, "rust preparer, changed");
    assert_eq!(reuse_prepared(&versions, "v2026.10.20"), None);
    let _ = fs::remove_dir_all(&versions);
}

#[test]
fn prepared_resources_follow_the_preparers_output_version() {
    let versions = std::env::temp_dir().join(format!(
        "eclipse-launcher-output-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&versions);
    let write = |tag: &str, path: &str, text: &str| {
        let path = versions.join(tag).join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    let lib = "crates/eclipse-prepare/src/lib.rs";
    // A preparer with its sources (as releases ship them) and the exporters it runs.
    let release = |tag: &str, code: &str| {
        write(tag, INSTALL_MARK, "");
        let lock = serde_json::json!({"minecraft": {"version": "26.3", "client_sha1": "c", "server_sha1": "s"}});
        write(tag, "research-lock.json", &lock.to_string());
        write(tag, "crates/eclipse-prepare/Cargo.toml", "[package]");
        write(tag, "crates/eclipse-prepare/build.rs", "fn main() {}");
        write(tag, lib, code);
        for name in LEGACY_EXPORTERS {
            write(
                tag,
                &format!("mods/eclipse/src/test/java/dev/eclipselab/parity/{name}"),
                name,
            );
        }
    };
    let prepared = |tag: &str| {
        write(
            tag,
            &format!(".local/eclipse-client/26.3/{RESOURCES_MANIFEST}"),
            r#"{"schema": 7}"#,
        );
        write(tag, ".local/eclipse-client/26.3/facts.json", tag);
    };
    let facts =
        |tag: &str| fs::read_to_string(resources_at(&versions.join(tag)).join("facts.json")).ok();

    // The preparer's code changed, not what it makes: the resources are kept.
    release(
        "v0.3.0",
        "pub const OUTPUT_VERSION: u32 = 1;\nfn prepare() {}",
    );
    prepared("v0.3.0");
    release(
        "v0.3.1",
        "pub const OUTPUT_VERSION: u32 = 1;\nfn prepare() { faster() }",
    );
    assert_eq!(reuse_prepared(&versions, "v0.3.1"), Some("v0.3.0".into()));
    assert_eq!(facts("v0.3.1").as_deref(), Some("v0.3.0"));
    // Its output changed, so the number did: prepare again.
    release(
        "v0.3.2",
        "pub const OUTPUT_VERSION: u32 = 2;\nfn prepare() {}",
    );
    assert_eq!(reuse_prepared(&versions, "v0.3.2"), None);
    assert_eq!(facts("v0.3.2"), None);
    // Another Minecraft jar, same number: prepare again.
    release(
        "v0.3.3",
        "pub const OUTPUT_VERSION: u32 = 1;\nfn prepare() {}",
    );
    let lock = serde_json::json!({"minecraft": {"version": "26.3", "client_sha1": "other", "server_sha1": "s"}});
    write("v0.3.3", "research-lock.json", &lock.to_string());
    assert_eq!(reuse_prepared(&versions, "v0.3.3"), None);

    // A preparer from before the number is known by its fingerprint.
    release("v0.2.64", "fn prepare() {}");
    let fingerprint = legacy_fingerprint(&versions.join("v0.2.64")).unwrap();
    let legacy = [(fingerprint.as_str(), 1)];
    let at = |tag: &str| output_version_in(&versions.join(tag), &legacy);
    assert_eq!(at("v0.2.64"), Some(1));
    assert_eq!(at("v0.3.2"), Some(2));
    // Line endings as checked out don't count; an exporter's or the sources' change does.
    write(
        "v0.2.64",
        "mods/eclipse/src/test/java/dev/eclipselab/parity/ParityData.java",
        "ParityData.java\r",
    );
    assert_eq!(at("v0.2.64"), Some(1));
    write(
        "v0.2.64",
        "mods/eclipse/src/test/java/dev/eclipselab/parity/VanillaCollisionOracle.java",
        "changed",
    );
    assert_eq!(at("v0.2.64"), None);
    release("v0.2.65", "fn prepare() { changed() }");
    assert_eq!(at("v0.2.65"), None);
    assert_eq!(output_version_in(&versions.join("missing"), &legacy), None);
    let _ = fs::remove_dir_all(&versions);
}

#[test]
fn prepared_versions_get_the_run_time_reports_without_preparing_again() {
    let versions = std::env::temp_dir().join(format!(
        "eclipse-launcher-reports-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&versions);
    let write = |tag: &str, path: &str, text: &str| {
        let path = versions.join(tag).join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    let release = |tag: &str, server: &str| {
        write(tag, INSTALL_MARK, "");
        let lock = serde_json::json!({"minecraft": {"version": "26.3", "client_sha1": "c", "server_sha1": server}});
        write(tag, "research-lock.json", &lock.to_string());
        write(
            tag,
            &format!(".local/eclipse-client/26.3/{RESOURCES_MANIFEST}"),
            "{}",
        );
    };
    let resources = |tag: &str| resources_at(&versions.join(tag));
    let datagen = |tag: &str, name: &str| {
        write(tag, &format!(".local/datagen/out/reports/{name}"), tag);
    };

    // Prepared before the reports shipped with the resources: its own scratch output.
    release("v2026.10.13", "s");
    datagen("v2026.10.13", "registries.json");
    datagen("v2026.10.13", "commands.json");
    // Resources copied from it: the reports come from the version that ran the generator.
    release("v2026.10.14", "s");
    // Another server jar: the reports of the others would be wrong, and it has none.
    release("v2026.10.15", "other");
    assert!(reuse_prepared_in(&versions).is_empty());
    for tag in ["v2026.10.13", "v2026.10.14"] {
        for name in RUNTIME_REPORTS {
            assert_eq!(
                fs::read_to_string(resources(tag).join(name)).unwrap(),
                "v2026.10.13"
            );
        }
    }
    assert!(!resources("v2026.10.15").join("registries.json").exists());
    assert!(!complete_reports(&versions, "v2026.10.15"));
    // Once there they stay as they are, even when the scratch output is gone.
    fs::remove_dir_all(versions.join("v2026.10.13/.local/datagen")).unwrap();
    assert!(complete_reports(&versions, "v2026.10.13"));
    let left: Vec<_> = fs::read_dir(resources("v2026.10.14"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().into_string().unwrap())
        .filter(|n| n.contains("partial"))
        .collect();
    assert!(left.is_empty());
    let _ = fs::remove_dir_all(&versions);
}

#[test]
fn prepared_versions_get_what_the_asset_store_adds_without_preparing_again() {
    let dir = std::env::temp_dir().join(format!(
        "eclipse-launcher-packs-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    let (versions, store, empty) = (dir.join("versions"), dir.join("store"), dir.join("none"));
    let write = |path: PathBuf, data: &[u8]| {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    };
    // An official launcher folder whose asset store holds the two packs, a panorama (fake
    // 2x2 PNG headers), Unifont, a language and the sound list.
    let background = "minecraft/textures/gui/title/background";
    let png = |n: u8| {
        let mut data = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR\0\0\0\x02\0\0\0\x02".to_vec();
        data.push(n);
        data
    };
    let mut assets: Vec<(String, Vec<u8>)> = eclipse_prepare::BUILT_IN_PACKS
        .iter()
        .map(|name| (name.to_string(), name.as_bytes().to_vec()))
        .collect();
    for face in ["0", "1", "2", "3", "4", "5", "overlay"] {
        assets.push((
            format!("{background}/panorama_{face}.png"),
            png(face.as_bytes()[0]),
        ));
    }
    for name in [
        "minecraft/font/include/unifont.json",
        "minecraft/font/unifont.zip",
        "pack.mcmeta",
        "minecraft/lang/fr_fr.json",
        "minecraft/sounds.json",
    ] {
        assets.push((name.into(), format!("{{\"{name}\": 1}}").into_bytes()));
    }
    let mut index = serde_json::Map::new();
    for (name, data) in &assets {
        let hash = sources::sha1_hex(data);
        write(
            store.join("assets/objects").join(&hash[..2]).join(&hash),
            data,
        );
        index.insert(
            name.clone(),
            serde_json::json!({"hash": hash, "size": data.len()}),
        );
    }
    let index = serde_json::json!({ "objects": index }).to_string();
    write(store.join("assets/indexes/7.json"), index.as_bytes());
    let manifest = serde_json::json!({
        "id": "9.9",
        "assetIndex": {"id": "7", "sha1": sources::sha1_hex(index.as_bytes())},
        "downloads": {"client": {"sha1": "c"}, "server": {"sha1": "s"}},
        "javaVersion": {"majorVersion": 25},
    })
    .to_string();
    write(store.join("versions/9.9/9.9.json"), manifest.as_bytes());
    fs::create_dir_all(&empty).unwrap();
    // The client jar's 1x1 placeholder.
    let placeholder = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR\0\0\0\x01\0\0\0\x01";
    let release = |tag: &str, prepared: bool| {
        let root = versions.join(tag);
        write(root.join(INSTALL_MARK), b"");
        let lock = serde_json::json!({"minecraft": {"version": "9.9", "java_major": 25,
            "client_sha1": "c", "server_sha1": "s", "manifest_sha1": sources::sha1_hex(manifest.as_bytes())}});
        write(root.join("research-lock.json"), lock.to_string().as_bytes());
        if prepared {
            let resources = resources_at(&root);
            write(resources.join(RESOURCES_MANIFEST), b"{}");
            write(resources.join("assets/minecraft/lang/en_us.json"), b"{}");
            write(
                resources.join("assets/minecraft/lang/deprecated.json"),
                b"{}",
            );
            write(
                resources
                    .join("assets")
                    .join(background)
                    .join("panorama_0.png"),
                placeholder,
            );
        }
    };
    let lines = std::cell::RefCell::new(Vec::new());
    let log = |line: String| lines.borrow_mut().push(line);
    let asset = |tag: &str, name: &str| {
        fs::read(resources_at(&versions.join(tag)).join("assets").join(name)).ok()
    };

    // Resources prepared without any of them: a folder without an asset store is skipped.
    release("v0.2.1", true);
    complete_assets_in(
        &versions,
        "v0.2.1",
        || vec![empty.clone(), store.clone()],
        &log,
    );
    assert_eq!(
        *lines.borrow(),
        [
            "Built-in pack assets ready: 2 files, 0.0 MB",
            "Panorama assets ready: 7 files, 0.0 MB",
            "Font assets ready: 2 files, 0.0 MB",
            "Language assets ready: 2 files, 0.0 MB",
            "Sound assets ready: 1 files, 0.0 MB",
        ]
    );
    for (name, data) in &assets {
        assert_eq!(asset("v0.2.1", name).as_ref(), Some(data), "{name}");
    }
    // Nothing else is prepared again.
    let resources = resources_at(&versions.join("v0.2.1"));
    assert_eq!(fs::read(resources.join(RESOURCES_MANIFEST)).unwrap(), b"{}");

    // No asset store anywhere: one line each, the version stays prepared, and no second try.
    lines.borrow_mut().clear();
    release("v0.2.2", true);
    complete_assets_in(&versions, "v0.2.2", || vec![empty.clone()], &log);
    complete_assets_in(&versions, "v0.2.2", || vec![empty.clone()], &log);
    let what = lines
        .borrow()
        .iter()
        .map(|line| line.split(" not added: ").next().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        what,
        [
            "v0.2.2: Programmer Art and High Contrast",
            "v0.2.2: The title panorama",
            "v0.2.2: The Unifont glyphs",
            "v0.2.2: The languages",
            "v0.2.2: The sounds",
        ]
    );
    assert!(resources_at(&versions.join("v0.2.2"))
        .join(RESOURCES_MANIFEST)
        .exists());
    assert_eq!(
        asset("v0.2.2", &format!("{background}/panorama_0.png")).as_deref(),
        Some(&placeholder[..])
    );

    // Not prepared, or complete: nothing to do, without even looking for a Minecraft folder.
    lines.borrow_mut().clear();
    release("v0.2.3", false);
    let unasked = || -> Vec<PathBuf> { unreachable!("no folder is needed") };
    complete_assets_in(&versions, "v0.2.3", unasked, &log);
    complete_assets_in(&versions, "v0.2.1", unasked, &log);
    assert!(lines.borrow().is_empty());
    assert!(!resources_at(&versions.join("v0.2.3")).exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn finds_the_app_bundle_around_the_binary() {
    let exe = Path::new("/Applications/Eclipse.app/Contents/MacOS/eclipse-launcher");
    assert_eq!(
        app_bundle(exe),
        Some(Path::new("/Applications/Eclipse.app"))
    );
    assert_eq!(app_bundle(Path::new("/Users/me/eclipse-launcher")), None);
    assert_eq!(
        app_bundle(Path::new("/x/Contents/MacOS/eclipse-launcher")),
        None
    );
    let work = std::env::temp_dir().join(format!("eclipse-find-app-{}", std::process::id()));
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(work.join("dist/Eclipse.app/Contents/MacOS")).unwrap();
    assert_eq!(find_app(&work), Some(work.join("dist/Eclipse.app")));
    let _ = fs::remove_dir_all(&work);
}

/// The whole update of an app bundle, as the release ships it: signed, zipped with `ditto`,
/// unpacked by the launcher, swapped in place of the installed one, still valid.
#[cfg(target_os = "macos")]
#[test]
fn an_app_bundle_updates_in_place() {
    use std::os::unix::fs::PermissionsExt;
    let work = std::env::temp_dir().join(format!("eclipse-bundle-{}", std::process::id()));
    let _ = fs::remove_dir_all(&work);
    let make = |dir: &Path, version: &str| {
        let app = dir.join("Eclipse.app");
        fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        fs::create_dir_all(app.join("Contents/Resources")).unwrap();
        let bin = app.join("Contents/MacOS/eclipse-launcher");
        fs::copy("/usr/bin/true", &bin).unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(app.join("Contents/Resources/version"), version).unwrap();
        fs::write(
            app.join("Contents/Info.plist"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\
             <key>CFBundleExecutable</key><string>eclipse-launcher</string>\
             <key>CFBundleIdentifier</key><string>dev.eclipse.test</string>\
             <key>CFBundlePackageType</key><string>APPL</string></dict></plist>\n",
        )
        .unwrap();
        let signed = Command::new("codesign")
            .args(["--force", "--sign", "-"])
            .arg(&app)
            .status()
            .unwrap();
        assert!(signed.success());
        app
    };
    let installed = make(&work.join("Applications"), "1.0.0");
    let release = make(&work.join("release"), "2.0.0");
    let zip = work.join("release.zip");
    let zipped = Command::new("ditto")
        .args(["-c", "-k", "--norsrc", "--noextattr", "--keepParent"])
        .arg(&release)
        .arg(&zip)
        .status()
        .unwrap();
    assert!(zipped.success());
    unpack(&zip, "release.zip", &work.join("unpacked")).unwrap();
    let new = find_app(&work.join("unpacked")).unwrap();
    replace_bundle(&new, &installed).unwrap();
    let version = fs::read_to_string(installed.join("Contents/Resources/version")).unwrap();
    assert_eq!(version, "2.0.0");
    let bin = installed.join("Contents/MacOS/eclipse-launcher");
    assert!(
        Command::new(&bin).status().unwrap().success(),
        "still executable"
    );
    let valid = Command::new("codesign")
        .args(["--verify", "--strict"])
        .arg(&installed)
        .status()
        .unwrap();
    assert!(valid.success(), "the updated bundle's signature holds");
    let left: Vec<_> = fs::read_dir(work.join("Applications"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(
        left,
        ["Eclipse.app"],
        "nothing staged or moved aside is left"
    );

    // A bundle changed after it was signed is refused, and the installed one stays.
    let tampered = make(&work.join("tampered"), "3.0.0");
    fs::write(tampered.join("Contents/Resources/version"), "3.0.1").unwrap();
    assert!(replace_bundle(&tampered, &installed).is_err());
    let version = fs::read_to_string(installed.join("Contents/Resources/version")).unwrap();
    assert_eq!(version, "2.0.0");
    let left = fs::read_dir(work.join("Applications")).unwrap().count();
    assert_eq!(left, 1, "nothing staged is left");
    let _ = fs::remove_dir_all(&work);
}

#[test]
fn launcher_versions_read_from_the_binary() {
    assert_eq!(
        parse_version_output("eclipse-launcher 0.1.5\n"),
        Some("0.1.5")
    );
    assert_eq!(parse_version_output("something else"), None);
    assert_eq!(parse_version_output(""), None);
    // This test binary is not a launcher: no version, so an update goes ahead.
    assert_eq!(
        installed_launcher_version(&std::env::current_exe().unwrap()),
        None
    );
    assert_eq!(
        installed_launcher_version(Path::new("/nonexistent/eclipse-launcher")),
        None
    );
}

/// The launcher's file was replaced while it ran (an update from the command line, or an
/// earlier one): the next replacement still lands at the path it started from.
#[cfg(unix)]
#[test]
fn a_replaced_binary_is_replaced_again_at_its_path() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("eclipse-launcher-replace-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let exe = dir.join("eclipse-launcher");
    fs::write(&exe, "old").unwrap();
    // Another update swaps the file under the running launcher.
    fs::write(dir.join("other"), "other update").unwrap();
    fs::rename(dir.join("other"), &exe).unwrap();
    fs::write(dir.join("new"), "new").unwrap();
    replace_binary(&dir.join("new"), &exe).unwrap();
    assert_eq!(fs::read_to_string(&exe).unwrap(), "new");
    // The file there keeps its mode, made executable (the swapped-in one was 0644).
    assert_eq!(
        fs::metadata(&exe).unwrap().permissions().mode() & 0o777,
        0o755
    );
    // Gone altogether: it is put back, executable.
    fs::remove_file(&exe).unwrap();
    replace_binary(&dir.join("new"), &exe).unwrap();
    assert_eq!(fs::read_to_string(&exe).unwrap(), "new");
    assert_eq!(
        fs::metadata(&exe).unwrap().permissions().mode() & 0o777,
        0o755
    );
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["eclipse-launcher", "new"]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn pruning_keeps_the_newest_releases() {
    let tags = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let installed = tags(&[
        "v0.2.91", "v0.2.90", "v0.2.88", "v0.2.85", "v0.2.80", "nightly",
    ]);
    assert_eq!(
        stale_versions(&installed, 2),
        tags(&["v0.2.88", "v0.2.85", "v0.2.80"])
    );
    assert!(stale_versions(&installed, 9).is_empty());
}

#[test]
fn crash_reports_are_found_and_summarised() {
    let dir = std::env::temp_dir().join(format!("eclipse-launcher-crash-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let start = std::time::SystemTime::now() - std::time::Duration::from_secs(2);
    assert!(crash_report_in_since(&dir, start).is_none());
    fs::create_dir_all(&dir).unwrap();
    let report = "---- Eclipse Crash Report ----\n// Details below.\n\nTime: 2026-10-14 09:05:07\nDescription: Unexpected error\n\npanicked at src/game.rs:10:5:\nboom\n\t0: eclipse_client::game::frame\n";
    fs::write(dir.join("crash-2026-10-14_09.05.07-client.txt"), report).unwrap();
    fs::write(dir.join("notes.txt"), "x").unwrap();
    assert_eq!(
        crash_report_in_since(&dir, start).unwrap(),
        dir.join("crash-2026-10-14_09.05.07-client.txt")
    );
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
    assert!(crash_report_in_since(&dir, later).is_none());
    assert_eq!(
        crash_summary(report).as_deref(),
        Some("panicked at src/game.rs:10:5: boom")
    );
    assert_eq!(crash_summary("no report"), None);
    let log = dir.join("latest.log");
    fs::write(&log, "a\nb\nc\n").unwrap();
    assert_eq!(tail_of(&log, 2).unwrap(), "b\nc\n");
    assert_eq!(tail_of(&log, 100).unwrap(), "a\nb\nc\n");
    assert!(tail_of(&dir.join("missing.log"), 5).is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn plain_notes() {
    let text =
        markdown_to_text("# Title\n\nSome *words* and [a link](https://x.y).\n\n- one\n- two\n");
    assert_eq!(
        text,
        "Title\n\nSome words and a link (https://x.y).\n\n- one\n- two\n"
    );
}

#[test]
fn the_game_does_not_inherit_the_launchers_renderer_workarounds() {
    let mut cmd = Command::new("true");
    cmd.env("GALLIUM_DRIVER", "llvmpipe")
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .env("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
        .env("WEBKIT_DISABLE_COMPOSITING_MODE", "1")
        .env("PIPEWIRE_PROPS", "kept");
    strip_launcher_render_env(&mut cmd);
    let envs = cmd.get_envs().collect::<std::collections::HashMap<_, _>>();
    for key in LAUNCHER_RENDER_ENV {
        assert_eq!(envs.get(OsStr::new(key)), Some(&None), "{key} is removed");
    }
    // Only the launcher's own workarounds go: the rest of the environment still reaches the game.
    assert_eq!(
        envs.get(OsStr::new("PIPEWIRE_PROPS")),
        Some(&Some(OsStr::new("kept")))
    );
}

#[test]
fn package_roots_are_matched_as_path_components() {
    for path in [
        "/usr/bin/eclipse-launcher",
        "/nix/store/hash-eclipse/bin/eclipse-launcher",
        "/app/bin/eclipse-launcher",
    ] {
        assert!(package_managed(Path::new(path)), "{path}");
    }
    for path in [
        "/home/player/Downloads/Eclipse.AppImage",
        "/usr/local/bin/eclipse-launcher",
        "/usr-backup/eclipse-launcher",
        "/nix/storehouse/eclipse-launcher",
        "/application/eclipse-launcher",
        "/tmp/.mount_Eclipse/usr/bin/eclipse-launcher",
    ] {
        assert!(!package_managed(Path::new(path)), "{path}");
    }
}

#[test]
fn every_request_names_the_project_and_where_to_reach_it() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = vec![0; 4096];
        let n = stream.read(&mut request).unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
        String::from_utf8_lossy(&request[..n]).to_lowercase()
    });
    get(&url).unwrap();
    let request = server.join().unwrap();
    assert!(
        request.contains(
            &format!(
                "user-agent: {} eclipse-launcher/{VERSION} (github.com/{})",
                REPO, REPO
            )
            .to_lowercase()
        ),
        "{request}"
    );
}

#[test]
fn a_failure_on_githubs_pages_names_github() {
    let error = NetError::new(
        "https://horizon-rust.github.io/horizon-mods/index.json",
        ureq::Error::Status(503, ureq::Response::new(503, "Unavailable", "").unwrap()),
    );
    assert!(
        error.to_string().starts_with("GitHub isn't answering"),
        "{error}"
    );
}

/// Settings files from before native mods were on by default still read; their
/// `native_mods` key is ignored, and the notice stays read.
#[test]
fn old_settings_with_the_native_mods_switch_still_read() {
    let s: Settings =
        serde_json::from_str(r#"{"native_mods": true, "native_mods_noticed": true}"#).unwrap();
    assert!(s.native_mods_noticed);
    let s: Settings = serde_json::from_str(r#"{"native_mods": false}"#).unwrap();
    assert!(!s.native_mods_noticed);
    let saved = serde_json::to_string(&s).unwrap();
    assert!(!saved.contains("native_mods"), "{saved}");
}

#[test]
fn the_step_under_way_reads_back_as_it_was_set() {
    for step in PrepareStep::ALL {
        set_prepare_step(step);
        assert_eq!(prepare_step(), step);
    }
}

#[test]
fn the_setting_picks_the_profile_and_the_old_switch_still_picks_release() {
    let read = |text: &str| Settings::parse(text).unwrap();
    assert_eq!(
        NativeProfile::chosen(&read("{}")),
        NativeProfile::Performance
    );
    let mut old = read(r#"{"native_release": true}"#);
    assert_eq!(NativeProfile::chosen(&old), NativeProfile::Release);
    old.set_native_profile(Some(NativeProfile::Dev));
    assert_eq!(NativeProfile::chosen(&old), NativeProfile::Dev);
    let text = serde_json::to_string(&old).unwrap();
    assert!(text.contains("\"native_profile\":\"dev\""), "{text}");
    old.set_native_profile(None);
    assert_eq!(
        NativeProfile::chosen(&old),
        NativeProfile::Performance,
        "the old switch is gone"
    );
    assert_eq!(
        NativeProfile::parse("performance"),
        Some(NativeProfile::Performance)
    );
    assert_eq!(
        NativeProfile::parse("fast"),
        Some(NativeProfile::Performance)
    );
    assert_eq!(
        NativeProfile::parse(" release "),
        Some(NativeProfile::Release)
    );
    assert_eq!(NativeProfile::parse("debug"), None);
}

/// The window opens its owner screen on this refusal.
#[test]
fn a_launch_with_no_account_that_owns_the_game_is_refused_as_such() {
    let tag = format!("v0.0.{}-gate", std::process::id());
    let dir = version_dir(&tag);
    fs::create_dir_all(&dir).unwrap();
    mark_installed(&dir).unwrap();
    let mut eclipse = crate::instances::list().remove(0);
    eclipse.version = Some(tag.clone());
    let refused = launch(Play::of(eclipse), &|_| {}).err();
    fs::remove_dir_all(&dir).unwrap();
    assert_eq!(refused, Some(PlayError::NoOwner));
}

/// A registry mod taken down is turned off in the folder that plays, an instance's own too: Play
/// once checked only the game's default folder.
#[test]
fn play_turns_a_taken_down_mod_off_in_the_instance_it_plays() {
    use crate::browser::{installed, java, Kind, SourceId};
    let tag = format!("v0.0.{}-taken-down", std::process::id());
    let dir = version_dir(&tag);
    let resources = resources(&tag);
    fs::create_dir_all(&resources).unwrap();
    fs::write(resources.join("eclipse-resources.json"), "{}").unwrap();
    mark_installed(&dir).unwrap();
    let home = std::env::temp_dir().join(format!("eclipse-taken-down-{}", std::process::id()));
    let _ = fs::remove_dir_all(&home);
    fs::create_dir_all(home.join("eclipse-mods")).unwrap();
    fs::write(home.join("eclipse-mods/coords_hud.emod"), "mod").unwrap();
    installed::Installed::open(&home)
        .unwrap()
        .record(installed::Entry {
            source: SourceId::Eclipse,
            project: "coords_hud".into(),
            slug: "coords_hud".into(),
            title: "Coordinates HUD".into(),
            kind: Kind::Mod,
            version: "0.1.0".into(),
            number: "0.1.0".into(),
            file: "eclipse-mods/coords_hud.emod".into(),
            checksum: String::new(),
            page: String::new(),
            dependency: false,
            verdict: None,
            skipped: vec![],
            namespaces: vec![],
        })
        .unwrap();
    java::set_enabled(&home, "coords_hud", true).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/eclipse-mods/tests/index/index-v1.json");
    let mut index: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fixture).unwrap()).unwrap();
    index["revoked"] =
        serde_json::json!([{ "id": "coords_hud", "reason": "Test.", "date": "2026-11-06" }]);
    let kept = game_home().join("mod-browser/eclipse-index.json");
    let before = fs::read(&kept).ok();
    fs::create_dir_all(kept.parent().unwrap()).unwrap();
    fs::write(&kept, index.to_string()).unwrap();
    let instance = crate::instances::Instance {
        id: "taken-down".into(),
        name: "Taken down".into(),
        kind: crate::instances::Kind::Own,
        work: home.clone(),
        home: home.clone(),
        icon: None,
        group: None,
        version: Some(tag),
        played: std::time::Duration::ZERO,
        last_played: None,
        launch: Default::default(),
        game_args: Some("--from-the-instance".into()),
        java: None,
    };
    let launched = launch(
        Play {
            offline_without_owner: true,
            ..Play::of(instance)
        },
        &|_| {},
    );
    let args: Vec<String> = launched.as_ref().map_or_else(
        |_| Vec::new(),
        |l| {
            l.cmd
                .get_args()
                .map(|a| a.to_string_lossy().into_owned())
                .collect()
        },
    );
    let on = java::is_enabled(&home, "coords_hud");
    match before {
        Some(text) => fs::write(&kept, text).unwrap(),
        None => fs::remove_file(&kept).unwrap(),
    }
    fs::remove_dir_all(&dir).unwrap();
    fs::remove_dir_all(&home).unwrap();
    assert_eq!(launched.err(), None);
    assert!(!on, "the taken-down mod is off in the instance's folder");
    assert!(args.iter().any(|a| a == "--from-the-instance"), "{args:?}");
}

#[cfg(unix)]
#[test]
fn a_logged_process_says_how_it_ended_once_it_has() {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "echo done; exit 3"]);
    let logged = Logged::start_then(cmd, move |result| {
        let _ = tx.send(result.clone());
    })
    .unwrap();
    let told = rx.recv_timeout(std::time::Duration::from_secs(20)).unwrap();
    assert!(told.is_err(), "{told:?}");
    assert_eq!(logged.wait(), told);
    assert_eq!(logged.tail(1), ["done"]);
}

#[test]
fn a_server_jar_that_isnt_mojangs_is_refused_and_not_kept() {
    let dir = std::env::temp_dir().join(format!("eclipse-server-jar-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("26.3.json");
    let sha1 = sources::sha1_hex(b"mojang's server");
    fs::write(
        &manifest,
        serde_json::json!({ "downloads": { "server": { "url": "https://piston-data.mojang.com/server.jar", "sha1": sha1 } } })
            .to_string(),
    )
    .unwrap();
    let jar = dir.join("server-jars/26.3.jar");
    let swapped = |_: &str| Ok(b"someone else's server".to_vec());
    let refused = server_jar(&manifest, "26.3", &jar, &swapped, &|_| {}).unwrap_err();
    assert!(
        refused.contains("isn't") || refused.contains("is not"),
        "{refused}"
    );
    assert!(!jar.exists());

    let mojang = |_: &str| Ok(b"mojang's server".to_vec());
    server_jar(&manifest, "26.3", &jar, &mojang, &|_| {}).unwrap();
    assert_eq!(fs::read(&jar).unwrap(), b"mojang's server");

    fs::write(&jar, b"cut short").unwrap();
    server_jar(&manifest, "26.3", &jar, &mojang, &|_| {}).unwrap();
    assert_eq!(fs::read(&jar).unwrap(), b"mojang's server");

    let offline = |_: &str| Err("offline".to_string());
    server_jar(&manifest, "26.3", &jar, &offline, &|_| {}).unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_install_moves_the_selection_by_why_it_ran() {
    let newer = "nightly-2026-11-01-4aa6854";
    let older = "nightly-2026-10-20-1b2c3d4";
    let selects = |tag, why, selected| install_selects(tag, why, selected);
    assert!(selects(older, InstallWhy::Added, None));
    assert!(selects(newer, InstallWhy::Added, Some(older)));
    assert!(!selects(older, InstallWhy::Added, Some(newer)));
    assert!(!selects(newer, InstallWhy::ForInstance, Some(older)));
    assert!(selects(newer, InstallWhy::ForInstance, None));
    assert!(selects(older, InstallWhy::Picked, Some(newer)));
    let update = |select| InstallWhy::Update { select };
    assert!(selects(newer, update(true), Some(older)));
    assert!(!selects(newer, update(false), Some(older)));
}

#[test]
fn a_release_body_becomes_its_notes_without_refs_thanks_or_footer() {
    let notes = |body: &str| {
        Release {
            tag: "v2026.10.13".into(),
            name: String::new(),
            body: body.into(),
            date: String::new(),
            time: String::new(),
            asset: None,
            prerelease: false,
            nightly: None,
        }
        .notes()
    };
    let body = "<h1>Want to play?</h1>\n\n### New\n- Native mods: export a headless engine (#6462)\n\n### Fixed\n- Play with friends: any port (#6363, fixes #6342)\n- Keep (this) aside\n\n### Contributors\nThanks to @a.\n\nYour launcher updates itself. New here?\n\n---\nNot an official Minecraft product.";
    assert_eq!(
        notes(body),
        "### New\n- Native mods: export a headless engine\n\n### Fixed\n- Play with friends: any port\n- Keep (this) aside"
    );
    assert_eq!(
        notes("Rebuilt from the latest main.\n\n---\nNot official."),
        "Rebuilt from the latest main."
    );
    assert_eq!(notes("### Contributors\nThanks to @a."), "");
}
