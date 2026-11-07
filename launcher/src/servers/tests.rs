use super::*;
use std::io::{BufRead, BufReader, Write};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("eclipse-servers-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn folder_names_are_plain() {
    assert_eq!(slug("Ludo's Survival!"), "ludo-s-survival");
    assert_eq!(slug("  "), "server");
    assert_eq!(slug("Été 2026"), "t-2026");
    assert_eq!(service_suffix(Path::new("/x/My World")), "my-world");
}

#[test]
fn create_asks_init_and_config_set() {
    let spec = NewServer {
        name: "Friends".into(),
        world: World::New {
            seed: " 12345 ".into(),
        },
        players: vec!["Alex".into(), "Steve".into()],
        eula: true,
        check_accounts: false,
        ..NewServer::default()
    };
    assert_eq!(
        init_args(&spec, "world", 25566).join(" "),
        "init --yes --eula --world world --seed 12345 --port 25566 --whitelist --offline \
         --player Alex --player Steve --op Alex"
    );
    let open = NewServer {
        friends_only: false,
        world: World::Copy("/saves/Old".into()),
        ..NewServer::default()
    };
    assert_eq!(
        init_args(&open, "old", 25565).join(" "),
        "init --yes --world old --port 25565 --no-whitelist"
    );
    assert_eq!(
        settings_args(&spec, 8131).join(" "),
        "config set world.game_mode=survival world.difficulty=normal server.max_players=10 \
         server.motd=Friends panel.bind=127.0.0.1:8131"
    );
}

#[cfg(unix)]
#[test]
fn programs_are_laid_out_as_install_sh_does() {
    let root = scratch("runtime");
    let version = root.join("versions/v1");
    fs::create_dir_all(version.join(".local/eclipse-client/26.3")).unwrap();
    fs::write(version.join(PROGRAM), "server").unwrap();
    fs::write(version.join("eclipse-client"), "game").unwrap();
    fs::write(version.join(".installed"), "").unwrap();
    crate::game_app::ensure(&version).unwrap();
    fs::write(
        version.join(".local/eclipse-client/26.3/eclipse-resources.json"),
        "{}",
    )
    .unwrap();
    let rt = root.join("runtime");
    set_up_in(&rt, &version, "v1", "26.3").unwrap();
    assert_eq!(
        fs::read_to_string(rt.join("current").join(PROGRAM)).unwrap(),
        "server"
    );
    assert!(!rt.join("releases/v1/.installed").exists());
    // The game's macOS bundle is the launcher's, not the server's.
    assert!(!rt.join("releases/v1/Eclipse.app").exists());
    // The release finds the game files where eclipse-server looks: .local beside it.
    assert!(rt
        .join("current/.local/eclipse-client/26.3/eclipse-resources.json")
        .is_file());
    // Again with newer game files: only they change, swapped in whole.
    fs::write(
        version.join(".local/eclipse-client/26.3/eclipse-resources.json"),
        "{\"new\":1}",
    )
    .unwrap();
    set_up_in(&rt, &version, "v1", "26.3").unwrap();
    assert_eq!(
        fs::read_to_string(game_files(&rt, "26.3").join("eclipse-resources.json")).unwrap(),
        "{\"new\":1}"
    );
    assert!(!game_files(&rt, "26.3").with_extension("old").exists());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn problems_say_what_to_do() {
    let dir = scratch("problems");
    let port = Problem::explain_with(
        "",
        "[Server thread/WARN]: **** FAILED TO BIND TO PORT!\nThe exception was: Address already in use (os error 48)",
        &dir,
    );
    assert!(port.message.contains("already used"), "{}", port.message);
    assert!(matches!(port.fix, Some(Fix::UsePort(p)) if p > 25565));
    let files = Problem::explain_with(
        "eclipse-server: no prepared game files. Eclipse reads them from",
        "",
        &dir,
    );
    assert_eq!(files.fix, Some(Fix::Prepare));
    let reports = Problem::explain_with(
        "",
        "**** FAILED TO BIND TO PORT!\nThe exception was: registries.json not found next to the resources",
        &dir,
    );
    assert_eq!(reports.fix, Some(Fix::Prepare));
    let stopped = Problem::explain_with("eclipse-server: the server stopped (exit 1)", "", &dir);
    assert_eq!(stopped.fix, Some(Fix::ShowLog));
    assert!(
        stopped.message.starts_with("The server"),
        "{}",
        stopped.message
    );
    let other = Problem::explain_with("eclipse-server: a server is already running in x", "", &dir);
    assert_eq!(other, Problem::plain("A server is already running in x"));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn lists_and_status_read_as_vanilla_and_the_supervisor_write_them() {
    let dir = scratch("lists");
    fs::write(
        dir.join("whitelist.json"),
        r#"[{"uuid": "a", "name": "Alex"}, {"uuid": "b", "name": "Steve"}]"#,
    )
    .unwrap();
    assert_eq!(names(&dir, "whitelist.json"), ["Alex", "Steve"]);
    assert!(names(&dir, "ops.json").is_empty());
    fs::remove_dir_all(&dir).unwrap();

    let s = Status::parse(&json!({
        "phase": "running", "engine_uptime_seconds": 61, "crashes": 1, "last_crash": "signal: 9",
        "sample": { "rss_bytes": 800_000_000u64, "cpu_percent": 3.5, "game": {
            "tps": 20.0, "mspt": { "average": 4.2 }, "max_players": 10, "paused": false,
            "players": [{ "name": "Alex", "uuid": "u1", "ping": 12 }] } },
        "update": { "installed": "v1", "available": "v2", "busy": null, "last": "Not updated: x", "ok": false }
    }));
    assert_eq!(s.phase, "running");
    assert_eq!(s.players[0].name, "Alex");
    assert_eq!(s.players[0].ping, Some(12));
    assert_eq!(
        (s.tps, s.mspt, s.max_players),
        (Some(20.0), Some(4.2), Some(10))
    );
    assert_eq!(s.update_available.as_deref(), Some("v2"));
    assert_eq!(s.update_last, Some(("Not updated: x".into(), false)));
    assert!(!Status::stopped().running());
}

/// The control API: the token goes with every call, and errors come back as the server words them.
#[test]
fn the_api_sends_the_token() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = std::thread::spawn(move || {
        let mut seen = Vec::new();
        for reply in [
            r#"{"phase":"running","sample":{"game":{"players":[]}}}"#,
            r#"{"error":"the game didn't answer"}"#,
        ] {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                head.push_str(&line);
            }
            // Read the body too: closing with it unread would reset the connection.
            let length = head
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|n| n.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            let mut body = vec![0; length];
            std::io::Read::read_exact(&mut reader, &mut body).unwrap();
            seen.push(head);
            let code = if reply.contains("error") {
                "409 Conflict"
            } else {
                "200 OK"
            };
            let mut stream = stream;
            write!(
                stream,
                "HTTP/1.1 {code}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                reply.len()
            )
            .unwrap();
        }
        seen
    });
    let dir = scratch("api");
    fs::create_dir_all(dir.join(".eclipse")).unwrap();
    fs::write(
        dir.join(".eclipse/api.json"),
        json!({ "address": address, "token": "t0k", "pid": 1 }).to_string(),
    )
    .unwrap();
    assert_eq!(status(&dir).phase, "running");
    let api = Api::open(&dir).unwrap();
    assert_eq!(
        api.command("list").unwrap_err(),
        "the game didn't answer".to_string()
    );
    let seen = server.join().unwrap();
    assert!(
        seen[0].starts_with("GET /api/v1/status HTTP/1.1"),
        "{}",
        seen[0]
    );
    assert!(seen
        .iter()
        .all(|h| h.to_ascii_lowercase().contains("authorization: bearer t0k")));
    assert!(seen[1].starts_with("POST /api/v1/command"));
    fs::remove_dir_all(&dir).unwrap();
    // Nothing answers: stopped.
    assert_eq!(status(Path::new("/nonexistent")).phase, "stopped");
}

#[test]
fn deleting_a_stopped_server_moves_its_world_and_backups_to_trash() {
    let root = scratch("delete-stopped");
    let dir = root.join("server");
    let trash = root.join("trash");
    fs::create_dir_all(dir.join("world/region")).unwrap();
    fs::create_dir_all(dir.join("backups")).unwrap();
    fs::write(dir.join("server.toml"), "[server]\n").unwrap();
    fs::write(dir.join("world/region/region.mca"), b"world").unwrap();
    fs::write(dir.join("backups/backup.zip"), b"backup").unwrap();
    trash_stopped_server(&dir, |path| crate::desktop_seams::to_folder(path, &trash)).unwrap();
    assert!(!dir.exists());
    assert_eq!(
        fs::read(trash.join("server/world/region/region.mca")).unwrap(),
        b"world"
    );
    assert_eq!(
        fs::read(trash.join("server/backups/backup.zip")).unwrap(),
        b"backup"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn deleting_refuses_a_live_supervisor_even_when_its_api_is_unavailable() {
    let root = scratch("delete-running");
    fs::create_dir_all(root.join(".eclipse")).unwrap();
    fs::write(root.join("server.toml"), "[server]\n").unwrap();
    let lock = fs::File::create(root.join(".eclipse/lock")).unwrap();
    lock.try_lock().unwrap();
    let result = trash_stopped_server(&root, |_| panic!("must not move a running server"));
    assert!(result.unwrap_err().contains("Stop the server"));
    assert!(root.join("server.toml").exists());
    drop(lock);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn deleting_refuses_an_unrecognized_folder_and_keeps_files_on_trash_failure() {
    let root = scratch("delete-invalid");
    assert!(trash_stopped_server(&root, |_| panic!("not a server")).is_err());
    fs::write(root.join("server.toml"), "[server]\n").unwrap();
    assert!(
        trash_stopped_server(&root, |_| Err("trash unavailable".into()))
            .unwrap_err()
            .contains("trash unavailable")
    );
    assert!(root.join("server.toml").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_game_mode_comes_from_server_toml() {
    let dir = scratch("game-mode");
    assert_eq!(game_mode(&dir), None);
    fs::write(
        dir.join("server.toml"),
        "[server]\nport = 25565\n\n[world]\nname = \"world\"\ngame_mode = \"creative\"\n",
    )
    .unwrap();
    assert_eq!(game_mode(&dir).as_deref(), Some("creative"));
    fs::write(dir.join("server.toml"), "[server]\nport = 25565\n").unwrap();
    assert_eq!(game_mode(&dir), None);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_server_list_that_cant_be_read_is_never_saved_over() {
    let root = scratch("torn-list");
    let file = root.join("servers.json");
    let dir = root.join("Mine");
    fs::create_dir_all(&dir).unwrap();
    let entry = Entry {
        name: "Mine".into(),
        dir,
    };
    change_list(&file, |all| all.push(entry.clone())).unwrap();
    assert_eq!(read_list(&file).unwrap(), std::slice::from_ref(&entry));
    fs::write(&file, "[{\"name\": \"Mine\"").unwrap();
    assert!(change_list(&file, |all| all.push(entry.clone())).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "[{\"name\": \"Mine\"");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_backup_says_when_and_why_it_was_made() {
    let made = |name: &str| {
        Backup {
            name: name.into(),
            bytes: 0,
            modified: None,
        }
        .made()
    };
    let at = "2026-10-19 11:28:57 UTC".to_string();
    assert_eq!(
        made("2026-10-19_11-28-57-manual.zip"),
        Some((at.clone(), BackupKind::Manual))
    );
    assert_eq!(
        made("2026-10-19_11-28-57-auto.zip"),
        Some((at.clone(), BackupKind::Auto))
    );
    assert_eq!(
        made("2026-10-19_11-28-57-before-v2026-10-19-10.zip"),
        Some((
            at.clone(),
            BackupKind::BeforeRelease("v2026.10.19.10".into())
        ))
    );
    assert_eq!(
        made("2026-10-19_11-28-57-before-nightly.zip"),
        Some((at, BackupKind::BeforeRelease("nightly".into())))
    );
    assert_eq!(made("my-copy.zip"), None);
}

#[test]
fn settings_are_named_as_server_toml_has_them() {
    assert_eq!(Key::MaxPlayers.dotted(), "server.max_players");
    assert_eq!(Key::GameMode.dotted(), "world.game_mode");
    let args = settings_args(&NewServer::default(), 8080);
    assert!(
        args.contains(&"world.game_mode=survival".to_string()),
        "{args:?}"
    );
    let phase = |p: &str| {
        Status {
            phase: p.into(),
            ..Status::default()
        }
        .phase()
    };
    assert_eq!(phase("running"), Phase::Running);
    assert_eq!(phase("restarting"), Phase::Restarting);
    assert_eq!(phase("gone"), Phase::Stopped);
}
