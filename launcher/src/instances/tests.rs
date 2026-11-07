use super::*;
use std::collections::BTreeMap;

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "eclipse-launcher-instances-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn put(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// Every file under `dir`, relative, with its bytes.
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                out.insert(p.strip_prefix(root).unwrap().into(), fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// A game home as a player has it before instances: worlds, options, packs, a session, an
/// imported modpack and a registry modpack with their own worlds.
fn player_home(home: &Path) {
    put(&home.join("saves/World/level.dat"), "world");
    put(&home.join("saves/World/region/r.0.0.mca"), "region");
    put(&home.join("saves/Other/level.dat"), "other");
    put(&home.join("options.txt"), "fov:90\n");
    put(&home.join("eclipse-player.txt"), "name:Player123\n");
    put(&home.join("resourcepacks/faithful.zip"), "pack");
    put(&home.join("logs/latest.log"), "log");
    put(&home.join("sessions/a.json"), "token");
    put(&home.join("telemetry/consent.json"), "{}");
    let pack = eclipse_port_pack::import::Pack {
        name: "Small Pack".into(),
        ..Default::default()
    };
    put(
        &home.join("ports/small-pack/pack.json"),
        &serde_json::to_string(&pack).unwrap(),
    );
    put(
        &home.join("ports/small-pack/home/saves/Modded/level.dat"),
        "modded",
    );
    put(
        &home.join("eclipse-packs/cozy/pack.json"),
        r#"{"id":"cozy","name":"Cozy","version":"1.0.0","sha256":"00","mods":[]}"#,
    );
    put(
        &home.join("eclipse-packs/cozy/home/saves/Cozy/level.dat"),
        "cozy",
    );
}

#[test]
fn a_new_player_name_replaces_the_kept_one_everywhere() {
    let root = scratch("newname");
    let (home, data) = (root.join("eclipse"), root.join("launcher"));
    player_home(&home);
    // An own instance that has the old name, and one with no file yet.
    put(
        &home.join("instances/one/eclipse-player.txt"),
        "name:Player123\n",
    );
    fs::create_dir_all(home.join("instances/two")).unwrap();
    let name = new_player_name_in(&home, &data).unwrap();
    assert_ne!(name, "Player123");
    assert!(name.starts_with("Player") && name.len() == 11);
    let kept = |home: &Path| fs::read_to_string(home.join("eclipse-player.txt")).unwrap();
    for h in [
        &home,
        &home.join("instances/one"),
        &home.join("instances/two"),
        &home.join("ports/small-pack/home"),
        &home.join("eclipse-packs/cozy/home"),
    ] {
        assert_eq!(kept(h), format!("name:{name}\n"), "{}", h.display());
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn existing_worlds_stay_where_they_are_and_every_folder_becomes_an_instance() {
    let root = scratch("migrate");
    let (home, data) = (root.join("eclipse"), root.join("launcher"));
    player_home(&home);
    let before = files(&home);

    // A launcher with instances, on a player's existing folders: no instances.json yet.
    let all = list_in(&home, &data);
    let ids: Vec<&str> = all.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, [DEFAULT, "pack:cozy", "port:small-pack"]);
    assert_eq!(all[0].name, "Eclipse");
    assert_eq!(all[0].home, home);
    assert_eq!(all[1].home, home.join("eclipse-packs/cozy/home"));
    assert_eq!(all[2].name, "Small Pack");
    assert_eq!(all[2].home, home.join("ports/small-pack/home"));

    // Everything the player can do with instances, then not one byte of theirs has moved.
    let empty = create_in(&home, &data, "Survival", None, None).unwrap();
    let copy = create_in(&home, &data, "My copy", Some(&all[0]), None).unwrap();
    edit_in(&home, &data, DEFAULT, |m| m.name = Some("Main".into())).unwrap();
    edit_in(&home, &data, "Small Pack", |m| {
        m.icon = Some(Icon::Texture("diamond".into()));
        m.group = Some("Modded".into());
    })
    .unwrap();
    let after = files(&home);
    for (path, bytes) in &before {
        assert_eq!(after.get(path), Some(bytes), "{} changed", path.display());
    }
    assert!(!data.join("settings.json").exists());

    // The empty one has only the player's options and name; the copy has the worlds and
    // packs, never the sessions, logs, data collection or other instances.
    assert_eq!(empty.home, home.join("instances/survival"));
    let empty_files: Vec<PathBuf> = files(&empty.home).into_keys().collect();
    assert_eq!(
        empty_files,
        [PathBuf::from("eclipse-player.txt"), "options.txt".into()]
    );
    let copied = files(&copy.home);
    for kept in [
        "saves/World/region/r.0.0.mca",
        "saves/Other/level.dat",
        "resourcepacks/faithful.zip",
    ] {
        assert_eq!(copied[Path::new(kept)], before[Path::new(kept)]);
    }
    for left_out in [
        "sessions",
        "logs",
        "telemetry",
        "instances",
        "ports",
        "eclipse-packs",
    ] {
        assert!(!copy.home.join(left_out).exists(), "{left_out} was copied");
    }

    let all = list_in(&home, &data);
    assert_eq!(all[0].name, "Main");
    let port = find_in(&all, "port:small-pack").unwrap();
    assert_eq!(port.group.as_deref(), Some("Modded"));
    assert_eq!(port.home, home.join("ports/small-pack/home"));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn names_stay_unique_and_packs_are_not_copied() {
    let root = scratch("names");
    let (home, data) = (root.join("eclipse"), root.join("launcher"));
    player_home(&home);
    create_in(&home, &data, "Survival", None, None).unwrap();
    assert_eq!(
        create_in(&home, &data, "survival", None, None).unwrap_err(),
        "There is already an instance called survival. Choose another name."
    );
    assert!(create_in(&home, &data, "  ", None, None).is_err());
    // Another name with the same folder name gets the next free folder.
    let second = create_in(&home, &data, "Survival!", None, None).unwrap();
    assert_eq!(second.id, "survival-2");
    assert_eq!(second.name, "Survival!");
    assert!(edit_in(&home, &data, "survival-2", |m| m.name =
        Some("SURVIVAL".into()))
    .is_err());
    let port = find_in(&list_in(&home, &data), "small-pack").unwrap();
    assert!(create_in(&home, &data, "Pack copy", Some(&port), None).is_err());
    assert!(!home.join("instances/pack-copy").exists());
    // An empty group is no group.
    let grouped = edit_in(&home, &data, "Survival", |m| m.group = Some(" ".into())).unwrap();
    assert_eq!(grouped.group, None);
    // An import named like an instance: both can still change everything but their names.
    Store::change(&data, |store| {
        store.instances.entry(port.id.clone()).or_default().name = Some("Survival".into());
        Ok(())
    })
    .unwrap();
    let launched = edit_in(&home, &data, "survival", |m| {
        m.launch.wrapper = "nice".into()
    })
    .unwrap();
    assert_eq!(launched.launch.wrapper, "nice");
    assert!(edit_in(&home, &data, "survival-2", |m| m.name =
        Some("survival".into()))
    .is_err());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn an_instance_follows_the_players_answers() {
    let root = scratch("follow");
    let (main, own) = (root.join("main"), root.join("own"));
    put(&main.join("options.txt"), "fov:90\n");
    put(&main.join("eclipse-player.txt"), "name:Player123\n");
    put(&main.join("telemetry/consent.json"), r#"{"version":1}"#);
    eclipse_core::no_ai::set(&main, true).unwrap();
    put(&own.join("options.txt"), "fov:70\n");
    follow_player(&main, &own);
    assert_eq!(
        fs::read_to_string(own.join("options.txt")).unwrap(),
        "fov:70\n"
    );
    assert_eq!(
        fs::read_to_string(own.join("eclipse-player.txt")).unwrap(),
        "name:Player123\n"
    );
    assert_eq!(
        fs::read_to_string(own.join("telemetry/consent.json")).unwrap(),
        r#"{"version":1}"#
    );
    assert!(eclipse_core::no_ai::is_on(&own));
    eclipse_core::no_ai::set(&main, false).unwrap();
    follow_player(&main, &own);
    assert!(!eclipse_core::no_ai::is_on(&own));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn icons_are_a_texture_name_or_a_colour() {
    assert_eq!(Icon::parse("#3FA7D6"), Ok(Icon::Colour("#3fa7d6".into())));
    assert_eq!(
        Icon::parse("grass_block"),
        Ok(Icon::Texture("grass_block".into()))
    );
    for bad in ["#12345", "../x", "Grass", "", "#zzzzzz"] {
        assert!(Icon::parse(bad).is_err(), "{bad}");
    }
    let root = scratch("icon");
    put(
        &root.join("assets/minecraft/textures/item/diamond.png"),
        "png",
    );
    let diamond = Icon::Texture("diamond".into()).texture(&root).unwrap();
    assert!(diamond.ends_with("item/diamond.png"));
    assert_eq!(Icon::Texture("stone".into()).texture(&root), None);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn an_instance_keeps_its_own_version_until_it_follows_updates_again() {
    let root = scratch("version");
    let (home, data) = (root.join("eclipse"), root.join("launcher"));
    player_home(&home);
    create_in(&home, &data, "Old world", None, None).unwrap();
    let pinned = edit_in(&home, &data, "Old world", |m| {
        m.version = Some("v0.1.0".into())
    })
    .unwrap();
    assert_eq!(pinned.version.as_deref(), Some("v0.1.0"));
    // Renaming keeps it; the other instances follow the launcher's version.
    let renamed = edit_in(&home, &data, "old-world", |m| m.name = Some("Older".into())).unwrap();
    assert_eq!(renamed.version.as_deref(), Some("v0.1.0"));
    assert!(list_in(&home, &data)
        .iter()
        .filter(|i| i.id != "old-world")
        .all(|i| i.version.is_none()));
    let follows = edit_in(&home, &data, "Older", |m| m.version = None).unwrap();
    assert_eq!(follows.version, None);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn play_time_adds_up_per_instance_and_survives_a_rename() {
    let root = scratch("play-time");
    let (home, data) = (root.join("home"), root.join("data"));
    fs::create_dir_all(home.join(OWN).join("skyblock")).unwrap();
    let ended = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    record_play_in(&data, "skyblock", Duration::from_secs(25 * 60 + 10), ended).unwrap();
    edit_in(&home, &data, "skyblock", |m| m.name = Some("Sky".into())).unwrap();
    let later = ended + Duration::from_secs(3600);
    record_play_in(&data, "skyblock", Duration::from_secs(40 * 60), later).unwrap();
    let all = list_in(&home, &data);
    let sky = find_in(&all, "Sky").unwrap();
    assert_eq!(sky.played, Duration::from_secs(65 * 60 + 10));
    assert_eq!(sky.last_played, Some(later));
    let default = find_in(&all, DEFAULT).unwrap();
    assert_eq!(
        (default.played, default.last_played),
        (Duration::ZERO, None)
    );
    // Never played: nothing written for it.
    let text = fs::read_to_string(data.join(FILE)).unwrap();
    assert!(!text.contains("eclipse"), "{text}");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn an_instances_json_that_cant_be_read_is_never_saved_over() {
    let root = scratch("torn");
    let (home, data) = (root.join("home"), root.join("data"));
    player_home(&home);
    create_in(&home, &data, "Skyblock", None, None).unwrap();
    let torn = &fs::read(data.join(FILE)).unwrap()[..20];
    fs::write(data.join(FILE), torn).unwrap();
    assert!(record_play_in(&data, "skyblock", Duration::from_secs(60), UNIX_EPOCH).is_err());
    assert!(edit_in(&home, &data, "skyblock", |m| m.group = Some("A".into())).is_err());
    assert!(create_in(&home, &data, "Other", None, None).is_err());
    assert!(!home.join(OWN).join("other").exists());
    assert_eq!(fs::read(data.join(FILE)).unwrap(), torn);
    assert!(find_in(&list_in(&home, &data), "skyblock").is_ok());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn play_time_reads_as_minutes_then_hours() {
    let t = |s| play_time(Duration::from_secs(s));
    assert_eq!(t(59), "under a minute");
    assert_eq!(t(12 * 60 + 59), "12 min");
    assert_eq!(t(3 * 3600 + 5 * 60), "3 h 05 min");
    assert_eq!(t(140 * 3600 + 7 * 60), "140 h");
}

/// A Java instance: an empty folder of its own (the real game keeps its own options), its
/// version in instances.json, kept by a copy, and refused by Play until the launcher starts
/// the real game; Eclipse instances' entries stay as they were.
#[test]
fn a_java_instance_plays_one_minecraft_version_in_its_own_folder() {
    let root = scratch("java");
    let (home, data) = (root.join("eclipse"), root.join("launcher"));
    player_home(&home);
    let java = create_in(&home, &data, "Vanilla 1.21.4", None, Some("1.21.4")).unwrap();
    assert_eq!(java.home, home.join(OWN).join("vanilla-1-21-4"));
    assert_eq!(fs::read_dir(&java.home).unwrap().count(), 0);
    assert_eq!(java.java.as_ref().unwrap().minecraft, "1.21.4");
    let copy = create_in(&home, &data, "Second", Some(&java), None).unwrap();
    assert_eq!(copy.java, java.java);
    let eclipse = create_in(&home, &data, "Eclipse two", None, None).unwrap();
    assert_eq!(eclipse.java, None);
    let store = fs::read_to_string(data.join(FILE)).unwrap();
    assert_eq!(
        store.matches("\"minecraft\": \"1.21.4\"").count(),
        2,
        "{store}"
    );
    let mut cmd = Command::new("game");
    let refused = apply(&mut cmd, &java).unwrap_err();
    assert!(
        refused.contains("plays Minecraft 1.21.4 (Java Edition)"),
        "{refused}"
    );
    edit_in(&home, &data, &java.id, |m| {
        m.java.as_mut().unwrap().memory_mb = Some(6144)
    })
    .unwrap();
    let edited = find_in(&list_in(&home, &data), &java.id).unwrap();
    assert_eq!(edited.java.unwrap().memory_mb, Some(6144));
    fs::remove_dir_all(&root).unwrap();
}

/// Delete moves an instance's folder to the Trash and forgets it; after one deleted outside the
/// launcher, a new one of its name starts clean (#eclipse-bugs: an instance that stayed).
#[test]
fn deleted_instances_go_to_the_trash_and_are_forgotten() {
    let root = scratch("delete");
    let (home, data, trash) = (root.join("home"), root.join("data"), root.join("trash"));
    player_home(&home);
    let to_trash = |p: &Path| crate::desktop_seams::to_folder(p, &trash);
    let survival = create_in(&home, &data, "Survival", None, None).unwrap();
    put(&survival.home.join("saves/Base/level.dat"), "base");
    edit_in(&home, &data, "Survival", |m| m.group = Some("Mine".into())).unwrap();
    let gone = delete_in(&home, &data, "Survival", &to_trash).unwrap();
    assert_eq!(gone.id, "survival");
    assert_eq!(
        fs::read(trash.join("survival/saves/Base/level.dat")).unwrap(),
        b"base"
    );
    assert!(find_in(&list_in(&home, &data), "Survival").is_err());
    assert!(!Store::read(&data)
        .unwrap()
        .instances
        .contains_key("survival"));
    // The game's home stays; an imported pack goes like an instance.
    assert!(delete_in(&home, &data, DEFAULT, &to_trash)
        .unwrap_err()
        .contains("can't be deleted"));
    assert!(home.join("saves/World/level.dat").is_file());
    delete_in(&home, &data, "small-pack", &to_trash).unwrap();
    assert!(trash
        .join("small-pack/home/saves/Modded/level.dat")
        .is_file());
    // A folder removed by hand: its choices are dropped, a new one of that name starts clean.
    create_in(&home, &data, "Skyblock", None, None).unwrap();
    edit_in(&home, &data, "Skyblock", |m| m.group = Some("Old".into())).unwrap();
    fs::remove_dir_all(home.join(OWN).join("skyblock")).unwrap();
    assert!(find_in(&list_in(&home, &data), "Skyblock").is_err());
    let again = create_in(&home, &data, "Skyblock", None, None).unwrap();
    assert_eq!(again.group, None);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn the_mod_browser_installs_for_the_instance_loader() {
    let vanilla = JavaGame {
        minecraft: "1.21.1".into(),
        ..JavaGame::default()
    };
    assert_eq!(vanilla.target().loader, None);
    let quilt = JavaGame {
        loader: Some(crate::loaders::Loader {
            kind: crate::loaders::Kind::Quilt,
            version: "0.30.1".into(),
        }),
        ..vanilla
    };
    let target = quilt.target();
    assert_eq!(
        (target.minecraft.as_str(), target.loader.as_deref()),
        ("1.21.1", Some("quilt"))
    );
    // A Quilt instance takes Fabric mods too.
    assert!(target.accepts("fabric") && !target.accepts("neoforge"));
}

/// A chosen picture is kept scaled to 128 pixels in the instance's folder; removing it moves it
/// to the trash; a file that isn't an image is refused.
#[test]
fn a_picture_is_kept_small_and_goes_to_the_trash() {
    let root = scratch("picture");
    let (home, trash) = (root.join("home"), root.join("trash"));
    let mut photo = Vec::new();
    image::RgbaImage::from_pixel(600, 400, image::Rgba([200, 90, 40, 255]))
        .write_to(
            &mut std::io::Cursor::new(&mut photo),
            image::ImageFormat::Png,
        )
        .unwrap();
    let png = picture(photo).unwrap();
    let to_trash = |p: &Path| crate::desktop_seams::to_folder(p, &trash);
    set_picture(&home, Some(&png), &to_trash).unwrap();
    let kept = image::open(home.join(PICTURE)).unwrap();
    assert_eq!((kept.width(), kept.height()), (128, 85));
    set_picture(&home, None, &to_trash).unwrap();
    assert!(!home.join(PICTURE).exists());
    assert!(trash.join(PICTURE).is_file());
    // Nothing to remove is fine.
    set_picture(&home, None, &to_trash).unwrap();
    assert!(picture(b"not an image".to_vec()).is_err());
    fs::remove_dir_all(&root).unwrap();
}

/// Edit switches what an instance plays (Eclipse, Vanilla, a loader, another Minecraft version):
/// its folder, with its worlds, mods and options, stays byte for byte as it was, a Java
/// instance's settings come back after a turn as Eclipse, and what can't keep working is named
/// first. The game's home plays what it was made with.
#[test]
fn switching_what_an_instance_plays_keeps_its_worlds_mods_and_settings() {
    use crate::loaders::{Kind as L, Loader};
    let root = scratch("switch");
    let (home, data) = (root.join("eclipse"), root.join("launcher"));
    player_home(&home);
    let made = create_in(&home, &data, "Mine", None, None).unwrap();
    put(&made.home.join("saves/Base/level.dat"), "base");
    put(&made.home.join("saves/Base/region/r.0.0.mca"), "region");
    put(&made.home.join("eclipse-mods/minimap.emod"), "emod");
    put(
        &made.home.join("eclipse-mods/ported/eclipse-mod.toml"),
        "ported",
    );
    put(
        &made.home.join("eclipse-mods/.eclipse-mod-data/minimap/x"),
        "data",
    );
    put(&made.home.join("options.txt"), "fov:90");
    edit_in(&home, &data, &made.id, |m| {
        m.version = Some("nightly-2026-11-01-c9d474d".into())
    })
    .unwrap();
    let before = files(&made.home);
    let fabric = |v: &str| Plays::Java {
        minecraft: v.into(),
        loader: Some(Loader {
            kind: L::Fabric,
            version: "0.16.0".into(),
        }),
    };

    // Eclipse to Fabric: Eclipse's mod stops loading; its world is opened by another version.
    let now = find_in(&list_in(&home, &data), &made.id).unwrap();
    assert_eq!(
        unkept(&now, &fabric("1.21.1"), "26.3"),
        [
            Unkept::Mods {
                count: 2,
                made_for: "Eclipse".into()
            },
            Unkept::Worlds {
                count: 1,
                from: "26.3".into(),
                to: "1.21.1".into()
            },
        ]
    );
    let java = switch::switch_in(&home, &data, &made.id, fabric("1.21.1")).unwrap();
    assert_eq!(java.java.as_ref().unwrap().minecraft, "1.21.1");
    assert_eq!(java.version.as_deref(), Some("nightly-2026-11-01-c9d474d"));
    edit_in(&home, &data, &made.id, |m| {
        m.java.as_mut().unwrap().memory_mb = Some(6144);
        m.java.as_mut().unwrap().jvm_args = "-XX:+UseZGC".into();
    })
    .unwrap();
    put(&made.home.join("mods/sodium.jar"), "jar");
    let with_jar = files(&made.home);

    // Fabric to Quilt keeps Fabric's mods working; another Minecraft version may not.
    let quilt = Plays::Java {
        minecraft: "1.21.1".into(),
        loader: Some(Loader {
            kind: L::Quilt,
            version: "0.27.0".into(),
        }),
    };
    let java = find_in(&list_in(&home, &data), &made.id).unwrap();
    assert_eq!(unkept(&java, &quilt, "26.3"), []);
    assert_eq!(
        unkept(&java, &fabric("1.20.1"), "26.3")[0],
        Unkept::ModsVersion {
            count: 1,
            from: "1.21.1".into(),
            to: "1.20.1".into()
        }
    );

    // A turn as Eclipse, then Vanilla: the Java settings come back, nothing on disk moved.
    switch::switch_in(&home, &data, &made.id, Plays::Eclipse).unwrap();
    let eclipse = find_in(&list_in(&home, &data), &made.id).unwrap();
    assert_eq!(eclipse.java, None);
    let vanilla = Plays::Java {
        minecraft: "1.21.4".into(),
        loader: None,
    };
    let back = switch::switch_in(&home, &data, &made.id, vanilla).unwrap();
    let back = back.java.unwrap();
    assert_eq!(
        (
            back.minecraft.as_str(),
            back.memory_mb,
            back.jvm_args.as_str()
        ),
        ("1.21.4", Some(6144), "-XX:+UseZGC")
    );
    assert_eq!(back.loader, None);
    assert_eq!(files(&made.home), with_jar);
    assert_eq!(
        with_jar.len(),
        before.len() + 1,
        "only the jar put there is new"
    );

    // The game's home can't switch, and is left as it was.
    let home_files = files(&home.join("saves"));
    let refused = switch::switch_in(&home, &data, DEFAULT, Plays::Eclipse).unwrap_err();
    assert!(refused.contains("plays what it was made with"), "{refused}");
    assert_eq!(files(&home.join("saves")), home_files);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn every_reader_of_the_version_an_instance_plays_gets_the_same_answer() {
    let root = scratch("plays");
    let (home, data) = (root.join("eclipse"), root.join("launcher"));
    player_home(&home);
    create_in(&home, &data, "Pinned", None, None).unwrap();
    create_in(&home, &data, "Vanilla", None, Some("1.21.4")).unwrap();
    edit_in(&home, &data, "Pinned", |m| {
        m.version = Some("v0.2.0".into())
    })
    .unwrap();
    let all = list_in(&home, &data);
    let eclipse = find_in(&all, DEFAULT).unwrap();
    let pinned = find_in(&all, "Pinned").unwrap();
    let vanilla = find_in(&all, "Vanilla").unwrap();
    let (newer, older, gone) = (
        "v0.3.0".to_string(),
        "v0.2.0".to_string(),
        "v0.1.0".to_string(),
    );
    let both = [newer.clone(), older.clone()];
    let only_newer = [newer.clone()];
    let check = |instance: &Instance,
                 default: Option<&String>,
                 installed: &[String],
                 expected: EclipseVersion| {
        let plays = instance.eclipse_version(default, installed).unwrap();
        let case = format!("{} with {default:?} of {installed:?}", instance.name);
        assert_eq!(plays, expected, "{case}");
        let resolved = instance.installed_version(default, installed);
        assert_eq!(resolved.as_ref().ok(), plays.installed_tag(), "{case}");
    };
    let plays = |tag, follows, installed| EclipseVersion {
        tag,
        follows,
        installed,
    };
    check(
        &eclipse,
        Some(&older),
        &both,
        plays(Some(&older), true, true),
    );
    check(
        &eclipse,
        Some(&gone),
        &both,
        plays(Some(&newer), true, true),
    );
    check(&eclipse, None, &[], plays(None, true, false));
    check(
        &pinned,
        Some(&newer),
        &both,
        plays(Some(&older), false, true),
    );
    check(
        &pinned,
        Some(&newer),
        &only_newer,
        plays(Some(&older), false, false),
    );
    check(&pinned, None, &[], plays(Some(&older), false, false));
    let unplayable = pinned.installed_version(Some(&newer), &only_newer);
    assert!(
        unplayable
            .unwrap_err()
            .contains("v0.2.0, which is not installed"),
        "a pinned version that isn't installed never plays the launcher's"
    );
    assert_eq!(
        eclipse.installed_version(None, &[]),
        Err(backend::no_version_installed())
    );
    assert_eq!(vanilla.eclipse_version(Some(&newer), &both), None);
    assert!(vanilla.resolve_version(None).is_err());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn an_instance_without_its_own_game_arguments_uses_the_launchers() {
    let root = scratch("game-args");
    let (home, data) = (root.join("eclipse"), root.join("launcher"));
    player_home(&home);
    fs::create_dir_all(home.join("instances/survival")).unwrap();
    put(
        &data.join(FILE),
        r#"{"instances": {"survival": {"name": "Survival", "played_secs": 60}}}"#,
    );
    let survival = find_in(&list_in(&home, &data), "Survival").unwrap();
    assert_eq!(survival.game_args, None);
    assert_eq!(survival.game_args_or("--fps 120"), "--fps 120");
    edit_in(&home, &data, "Survival", |m| m.group = Some("Old".into())).unwrap();
    let kept = fs::read_to_string(data.join(FILE)).unwrap();
    assert!(!kept.contains("game_args"), "{kept}");

    let own = edit_in(&home, &data, "Survival", |m| {
        m.game_args = Some("--quick".into())
    })
    .unwrap();
    assert_eq!(own.game_args_or("--fps 120"), "--quick");
    let none = edit_in(&home, &data, "Survival", |m| {
        m.game_args = Some(String::new())
    })
    .unwrap();
    assert_eq!(none.game_args_or("--fps 120"), "");
    let back = edit_in(&home, &data, "Survival", |m| m.game_args = None).unwrap();
    assert_eq!(back.game_args_or("--fps 120"), "--fps 120");
    assert_eq!(back.played, Duration::from_secs(60));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn registry_mods_install_for_the_game_their_instance_plays() {
    let instance = |id: &str, version: Option<&str>| Instance {
        id: id.into(),
        name: id.into(),
        kind: Kind::Own,
        work: PathBuf::from(id),
        home: PathBuf::from(id),
        icon: None,
        group: None,
        version: version.map(String::from),
        played: Duration::ZERO,
        last_played: None,
        launch: Default::default(),
        game_args: None,
        java: None,
    };
    let list = [instance("eclipse", None), instance("old", Some("v1"))];
    let installed = ["v2".to_string(), "v1".to_string()];
    let selected = Some(&installed[0]);
    let at = |home: &str| playing_at(&list, Path::new(home), selected, &installed).cloned();
    assert_eq!(at("old").as_deref(), Some("v1"));
    assert_eq!(at("eclipse").as_deref(), Some("v2"));
    assert_eq!(at("elsewhere").as_deref(), Some("v2"));
    let gone = [instance("gone", Some("v0"))];
    assert_eq!(
        playing_at(&gone, Path::new("gone"), selected, &installed),
        None
    );
}
