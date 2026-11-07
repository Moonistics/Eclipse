use super::*;

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("eclipse-shortcut-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn names_fit_every_system() {
    assert_eq!(
        file_name("Hardcore", System::Linux),
        "Hardcore (Eclipse).desktop"
    );
    assert_eq!(
        file_name("A/B: C?", System::Windows),
        "A-B- C- (Eclipse).lnk"
    );
    assert_eq!(file_name(" .. ", System::Mac), "Instance (Eclipse).app");
    assert_eq!(slug("port:all the mods"), "port-all-the-mods");
}

#[test]
fn a_linux_entry_plays_its_instance() {
    let entry = desktop_entry(
        Path::new("/opt/My Eclipse/eclipse-launcher"),
        "port:atm",
        "ATM",
    );
    assert!(entry.starts_with("[Desktop Entry]\nType=Application\nName=ATM\n"));
    assert!(entry
        .contains("\nExec=\"/opt/My Eclipse/eclipse-launcher\" play --instance \"port:atm\"\n"));
    assert!(entry.contains("\nTryExec=/opt/My Eclipse/eclipse-launcher\n"));
    assert!(entry.ends_with("\nX-Eclipse-Instance=port:atm\n"));
    let odd = desktop_entry(Path::new("/x"), "a", "Two\nlines");
    assert!(odd.contains("Name=Two lines\n"));
}

#[test]
fn a_mac_app_runs_a_script_that_plays_its_instance() {
    let files = mac_app(
        Path::new("/Applications/Eclipse Launcher.app/Contents/MacOS/eclipse-launcher"),
        "it's",
        "Tom & Jerry",
    );
    let (_, plist, _) = &files[0];
    let plist = String::from_utf8(plist.clone()).unwrap();
    assert!(plist.contains("<string>Tom &amp; Jerry</string>"));
    assert!(plist.contains("<string>dev.eclipse.instance.it-s</string>"));
    assert!(plist.contains("<key>LSUIElement</key>\n\t<true/>"));
    let (path, script, program) = &files[1];
    assert_eq!((*path, *program), ("Contents/MacOS/play", true));
    assert!(String::from_utf8(script.clone()).unwrap().ends_with(
        "exec '/Applications/Eclipse Launcher.app/Contents/MacOS/eclipse-launcher' play \
         --instance 'it'\\''s'\n"
    ));
    assert_eq!(files[2].1, MAC_ICON);
}

/// The shell-written shortcut keeps the instance's arguments, so an uninstall finds it: `plays`
/// reads them from the file's Unicode strings.
#[cfg(windows)]
#[test]
fn the_shell_written_link_keeps_the_instance_it_plays() {
    let root = scratch("plays");
    let exe = root.join("Eclipse Launcher").join("eclipse-launcher.exe");
    fs::create_dir_all(exe.parent().unwrap()).unwrap();
    fs::write(&exe, b"x").unwrap();
    let link = root.join("Hardcore (Eclipse).lnk");
    write_windows_link(
        &link,
        &exe,
        &windows_args("hardcore"),
        "Play Hardcore in Eclipse",
    )
    .unwrap();
    assert!(plays(&link, "hardcore"));
    assert!(!plays(&link, "another"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn shortcuts_are_made_found_renamed_and_removed() {
    let root = scratch("make");
    let (desktop, menu, trash) = (
        root.join("Desktop"),
        root.join("applications"),
        root.join("trash"),
    );
    let bin = trash.clone();
    let trash_fn = move |p: &Path| desktop_seams::to_folder(p, &bin);
    let t: Trash = &trash_fn;
    // A path the shell accepts on the system running the test: Windows writes its shortcuts through
    // IShellLinkW, which refuses a Unix path.
    let exe = if cfg!(windows) {
        Path::new(r"C:\Eclipse\eclipse-launcher.exe")
    } else {
        Path::new("/opt/eclipse-launcher")
    };
    let systems = [System::Linux, System::Mac, System::Windows];
    for system in systems
        .into_iter()
        .filter(|s| cfg!(windows) || *s != System::Windows)
    {
        let made = make_in(
            system,
            &desktop,
            Some(&menu),
            exe,
            "hardcore",
            "Hardcore",
            t,
        )
        .unwrap();
        assert_eq!(made[0], desktop.join(file_name("Hardcore", system)));
        assert_eq!(find_in(&desktop, Some(&menu), "hardcore"), {
            let mut m = made.clone();
            m.sort();
            m
        });
        // Again: rewritten in place, nothing in the trash.
        make_in(
            system,
            &desktop,
            Some(&menu),
            exe,
            "hardcore",
            "Hardcore",
            t,
        )
        .unwrap();
        assert!(!trash.exists());
        // Renamed: the old one goes, the new one is there.
        let made = make_in(system, &desktop, Some(&menu), exe, "hardcore", "Hard", t).unwrap();
        assert!(made[0].ends_with(file_name("Hard", system)));
        assert!(!desktop.join(file_name("Hardcore", system)).exists());
        assert!(find_in(&desktop, Some(&menu), "skyblock").is_empty());
        let gone = remove_in(&desktop, Some(&menu), "hardcore", t).unwrap();
        assert_eq!(gone.len(), made.len());
        assert!(find_in(&desktop, Some(&menu), "hardcore").is_empty());
        fs::remove_dir_all(&trash).unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let made = make_in(System::Mac, &desktop, None, exe, "a", "A", t).unwrap();
        let mode = fs::metadata(made[0].join("Contents/MacOS/play"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
    }
    // Someone else's file of that name stays.
    fs::write(desktop.join("Mine (Eclipse).desktop"), "not ours").unwrap();
    let err = make_in(System::Linux, &desktop, None, exe, "mine", "Mine", t).unwrap_err();
    assert!(err.contains("isn't this instance's shortcut"), "{err}");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn xml_escapes_the_characters_plist_strings_must_not_carry() {
    assert_eq!(
        xml(r#"a & b < c > "d""#),
        r#"a &amp; b &lt; c &gt; &quot;d&quot;"#
    );
}
