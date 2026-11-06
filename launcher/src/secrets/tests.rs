//! The stores against fakes of the OS keychain, and the secrets file on disk.
use super::*;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const KEY: &str = "069a79f444e94726a5befca90e38aaf5";

/// The OS keychain itself: Credential Manager and the Keychain take one secret per item, and a
/// sign-in's secrets are longer than one item holds. Ignored by default: it writes to the keychain
/// (a test key, removed again).
#[cfg(any(target_os = "macos", target_os = "windows"))]
#[test]
#[ignore = "writes to the OS keychain"]
fn system_keychain_keeps_long_secrets() {
    let key = "test-long-secret";
    let secret = format!("{{\"refresh_token\":\"{}\"}}", "r".repeat(3000));
    SystemStore.set(key, &secret).unwrap();
    assert_eq!(SystemStore.get(key).unwrap(), Some(secret.clone()));
    SystemStore.delete(key).unwrap();
    assert_eq!(SystemStore.get(key).unwrap(), None);
}

#[test]
fn long_secrets_split_into_keychain_sized_pieces() {
    // Mixed 1-, 2- and 4-byte characters, so the cuts land inside some of them.
    let secret = "aé🔑".repeat(300);
    let pieces = split_secret(&secret);
    assert!(pieces.len() > 1);
    assert_eq!(pieces.concat(), secret);
    for piece in &pieces {
        assert!(piece.len() <= PIECE);
        assert!(piece.encode_utf16().count() * 2 <= 2560);
    }
    assert_eq!(split_secret("short"), vec!["short"]);
}

/// Keychain items in memory; writes fail once `room` items are kept, as a full Credential Manager
/// refuses new items while it still overwrites the ones it has.
struct FullItems {
    items: Mutex<HashMap<String, String>>,
    room: usize,
}
impl Items for FullItems {
    fn read(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self.items.lock().unwrap().get(key).cloned())
    }
    fn write(&self, key: &str, piece: &str) -> Result<(), String> {
        let mut items = self.items.lock().unwrap();
        if !items.contains_key(key) && items.len() >= self.room {
            return Err(credential_manager_says(8).unwrap().into());
        }
        items.insert(key.into(), piece.into());
        Ok(())
    }
    fn remove(&self, key: &str) -> Result<bool, String> {
        Ok(self.items.lock().unwrap().remove(key).is_some())
    }
}

#[test]
fn a_secret_the_keychain_cant_finish_keeps_the_old_one() {
    let items = FullItems {
        items: Mutex::default(),
        room: 2,
    };
    let old = "o".repeat(PIECE + 1);
    set_pieces(&items, KEY, &old).unwrap();
    assert_eq!(get_pieces(&items, KEY).unwrap(), Some(old.clone()));
    // A renewal three pieces long: the first two overwrite, the third finds the store full.
    let new = "n".repeat(2 * PIECE + 1);
    let said = set_pieces(&items, KEY, &new).unwrap_err();
    assert_eq!(said, CREDENTIAL_MANAGER_FULL);
    assert_eq!(get_pieces(&items, KEY).unwrap(), Some(old));
    // A sign-in into a full store keeps nothing, and says why.
    let full = FullItems {
        items: Mutex::default(),
        room: 0,
    };
    assert_eq!(
        set_pieces(&full, KEY, "s").unwrap_err(),
        CREDENTIAL_MANAGER_FULL
    );
    assert_eq!(get_pieces(&full, KEY).unwrap(), None);
    // Room again: the longer secret replaces the old one whole.
    let items = FullItems { room: 3, ..items };
    set_pieces(&items, KEY, &new).unwrap();
    assert_eq!(get_pieces(&items, KEY).unwrap(), Some(new));
    set_pieces(&items, KEY, "short").unwrap();
    assert_eq!(get_pieces(&items, KEY).unwrap(), Some("short".into()));
    assert_eq!(items.items.lock().unwrap().len(), 1);
}

#[test]
fn a_split_secret_folds_into_one_item() {
    // The layout launchers before #4912 wrote, in memory.
    let items = FullItems {
        items: Mutex::default(),
        room: 99,
    };
    let secret = "s".repeat(2 * PIECE + 1);
    set_pieces(&items, KEY, &secret).unwrap();
    assert_eq!(items.items.lock().unwrap().len(), 3);
    // The first read answers the whole secret and leaves a single item behind.
    assert_eq!(get_item(&items, KEY).unwrap(), Some(secret.clone()));
    assert_eq!(items.items.lock().unwrap().len(), 1);
    assert_eq!(items.read(KEY).unwrap(), Some(secret));
    // A rewrite stays one item; a shorter secret leaves no pieces behind.
    set_item(&items, KEY, "short").unwrap();
    assert_eq!(items.items.lock().unwrap().len(), 1);
    assert_eq!(get_item(&items, KEY).unwrap(), Some("short".into()));
    delete_pieces(&items, KEY).unwrap();
    assert_eq!(get_item(&items, KEY).unwrap(), None);
}

/// Items that answer `left` removals, then refuse them, as a keychain denying a delete does.
struct RefusedDeletes {
    items: Mutex<HashMap<String, String>>,
    left: Mutex<usize>,
}
impl Items for RefusedDeletes {
    fn read(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self.items.lock().unwrap().get(key).cloned())
    }
    fn write(&self, key: &str, piece: &str) -> Result<(), String> {
        self.items.lock().unwrap().insert(key.into(), piece.into());
        Ok(())
    }
    fn remove(&self, key: &str) -> Result<bool, String> {
        let mut left = self.left.lock().unwrap();
        if *left == 0 {
            return Err("the keychain refused the delete".into());
        }
        *left -= 1;
        Ok(self.items.lock().unwrap().remove(key).is_some())
    }
}

#[test]
fn a_refused_delete_puts_the_split_secret_back() {
    // The layout launchers before #4912 wrote, in memory.
    let old = "o".repeat(2 * PIECE + 1);
    let pieces: Vec<String> = split_secret(&old).iter().map(|p| p.to_string()).collect();
    for left in [0, 1] {
        let items = RefusedDeletes {
            items: Mutex::new(
                pieces
                    .iter()
                    .enumerate()
                    .map(|(n, p)| (piece_key(KEY, n), p.clone()))
                    .collect(),
            ),
            left: Mutex::new(left),
        };
        // The fold's clean-up is refused, halfway or at once: the read still answers the whole
        // secret, and every old item is back behind it.
        assert_eq!(get_item(&items, KEY).unwrap(), Some(old.clone()));
        assert_eq!(items.items.lock().unwrap().len(), pieces.len());
        assert_eq!(get_item(&items, KEY).unwrap(), Some(old.clone()));
        // A writer's set over the split layout fails the same way, and reads back the same.
        *items.left.lock().unwrap() = left;
        assert_eq!(
            set_item(&items, KEY, "new").unwrap_err(),
            "the keychain refused the delete"
        );
        assert_eq!(get_item(&items, KEY).unwrap(), Some(old.clone()));
        assert_eq!(items.items.lock().unwrap().len(), pieces.len());
    }
    // Windows' set_pieces rolls a refused leftover delete back the same way.
    let items = RefusedDeletes {
        items: Mutex::new(
            pieces
                .iter()
                .enumerate()
                .map(|(n, p)| (piece_key(KEY, n), p.clone()))
                .collect(),
        ),
        left: Mutex::new(0),
    };
    assert!(set_pieces(&items, KEY, "shorter").is_err());
    assert_eq!(get_pieces(&items, KEY).unwrap(), Some(old));
}

#[test]
fn credential_manager_error_8_says_the_store_is_full() {
    let said = credential_manager_says(8).unwrap();
    assert!(said.contains("Credential Manager is full"));
    assert!(said.contains("Windows Credentials"));
    // Other codes keep the keyring crate's own words.
    assert_eq!(credential_manager_says(5), None);
    assert_eq!(credential_manager_says(1312), None);
}

#[test]
fn macos_removed_executable_error_requests_a_restart_and_new_sign_in() {
    let said = macos_keychain_says(100002).unwrap();
    assert_eq!(said, MACOS_LAUNCHER_RESTART);
    assert!(said.contains("Restart the launcher"));
    assert!(said.contains("sign in with Microsoft again"));
    for code in [0, 2, -25300, -25293, -25291, 100001, 100003] {
        assert_eq!(macos_keychain_says(code), None, "OSStatus {code}");
    }
}

/// Constructs native errors, without creating or reading any Keychain entry.
#[cfg(target_os = "macos")]
#[test]
fn only_the_typed_macos_removed_executable_failure_gets_the_restart_message() {
    let failure = |code| {
        keyring::Error::PlatformFailure(Box::new(security_framework::base::Error::from_code(code)))
    };
    assert_eq!(write_error(failure(100002)), MACOS_LAUNCHER_RESTART);
    for code in [-25300, -25293, -25291, 100003] {
        let error = failure(code);
        let original = error.to_string();
        assert_ne!(original, MACOS_LAUNCHER_RESTART);
        assert_eq!(write_error(error), original);
    }
    let unrelated = keyring::Error::PlatformFailure(Box::new(std::io::Error::other(
        "OSStatus 100002: unrelated provider",
    )));
    let original = unrelated.to_string();
    assert_eq!(write_error(unrelated), original, "not a string match");
    assert_eq!(
        write_error(keyring::Error::NoEntry),
        keyring::Error::NoEntry.to_string()
    );
}

/// A secrets file of the test's own, gone first.
pub(crate) fn secrets_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("eclipse-secrets-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("launcher").join(SECRETS_FILE)
}

pub(crate) fn remove_secrets_dir(path: &Path) {
    let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
}

/// The files beside the secrets file, by name.
pub(crate) fn files_beside(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_broken_secrets_file_is_an_error_that_never_shows_it() {
    let path = secrets_file("broken");
    let store = FileStore::new(path.clone());
    // Cut short, and a lone string: serde's own messages quote the latter.
    for broken in [
        "{\"a\":\"leaked-secret",
        "\"leaked-secret\"",
        "[\"leaked-secret\"]",
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let sealed = super::seal(broken.as_bytes()).unwrap();
        std::fs::write(&path, &sealed).unwrap();
        let said = store.get("a").unwrap_err();
        assert!(!said.contains("leaked-secret"), "{said}");
        assert!(said.contains("line 1"), "{said}");
        // Never written over: the secrets in it aren't lost in silence.
        assert!(store.set("c", "four").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), sealed);
    }
    remove_secrets_dir(&path);
}

#[test]
fn the_secrets_file_is_never_plain_text() {
    let path = secrets_file("sealed");
    let store = FileStore::new(path.clone());
    store.set("a", "kept-secret").unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.starts_with(super::SEALED));
    assert!(
        !bytes.windows(11).any(|w| w == b"kept-secret"),
        "the secret is encrypted"
    );
    assert_eq!(store.get("a").unwrap().as_deref(), Some("kept-secret"));
    // Plain JSON, or one byte changed in the sealed file: refused, never quoted, never written over.
    let mut changed = bytes.clone();
    *changed.last_mut().unwrap() ^= 1;
    for other in [b"{\"a\":\"leaked-secret\"}".to_vec(), changed] {
        std::fs::write(&path, &other).unwrap();
        let said = store.get("a").unwrap_err();
        assert!(!said.contains("leaked-secret"), "{said}");
        assert!(said.contains("Remove it and sign in again"), "{said}");
        assert!(store.set("c", "four").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), other);
    }
    remove_secrets_dir(&path);
}

const WRITER: &str = "ECLIPSE_TEST_SECRETS_WRITER";

/// One of the processes [`processes_sharing_the_secrets_file_lose_nothing`] starts: does nothing
/// unless it gives the file and the writer's number.
#[test]
#[ignore = "a child process of processes_sharing_the_secrets_file_lose_nothing"]
fn secrets_file_writer() {
    let Some(spec) = std::env::var_os(WRITER) else {
        return;
    };
    let spec = spec.into_string().unwrap();
    let (w, path) = spec.split_once(':').unwrap();
    let store = FileStore::new(PathBuf::from(path));
    for i in 0..25 {
        store
            .set(&format!("{w}-{i}"), &format!("secret-{w}-{i}"))
            .unwrap();
    }
    if w == "0" {
        store.delete("signed-out").unwrap();
    }
}

#[test]
fn processes_sharing_the_secrets_file_lose_nothing() {
    let path = secrets_file("processes");
    FileStore::new(path.clone()).set("signed-out", "x").unwrap();
    let exe = std::env::current_exe().unwrap();
    let writers: Vec<_> = (0..6)
        .map(|w| {
            std::process::Command::new(&exe)
                .args([
                    "--ignored",
                    "--exact",
                    "secrets::tests::secrets_file_writer",
                    "--test-threads=1",
                ])
                .env(WRITER, format!("{w}:{}", path.display()))
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    for mut writer in writers {
        assert!(writer.wait().unwrap().success());
    }
    let store = FileStore::new(path.clone());
    for w in 0..6 {
        for i in 0..25 {
            assert_eq!(
                store.get(&format!("{w}-{i}")).unwrap(),
                Some(format!("secret-{w}-{i}"))
            );
        }
    }
    assert_eq!(store.get("signed-out").unwrap(), None);
    assert_eq!(
        files_beside(&path),
        ["account-secrets.json", "account-secrets.lock"]
    );
    remove_secrets_dir(&path);
}

/// A keychain that can be away (a Secret Service that didn't start, a dismissed unlock prompt),
/// refuse some keys, or hold every call while `prompt` is held (an unlock prompt left open).
#[derive(Default)]
struct Keychain {
    kept: MemoryStore,
    away: std::sync::atomic::AtomicBool,
    refuses: Mutex<Vec<String>>,
    prompt: Mutex<()>,
    /// Calls that reached it: each one could have been a prompt.
    asked: std::sync::atomic::AtomicUsize,
    /// Run once, right after the next call it didn't answer.
    after_away: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    /// Run once, while it takes the next secret.
    while_taking: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl Keychain {
    fn answer(&self) -> Result<(), String> {
        self.asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        drop(self.prompt.lock().unwrap());
        if self.away.load(std::sync::atomic::Ordering::Relaxed) {
            if let Some(then) = self.after_away.lock().unwrap().take() {
                then();
            }
            return Err("The name org.freedesktop.secrets was not provided".into());
        }
        Ok(())
    }
    fn set_away(&self, away: bool) {
        self.away.store(away, std::sync::atomic::Ordering::Relaxed);
    }
    fn asked(&self) -> usize {
        self.asked.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// One keychain seen by several launchers.
struct Shared(Arc<Keychain>);

impl SecretStore for Shared {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        self.0.answer()?;
        self.0.kept.get(key)
    }
    fn set(&self, key: &str, secret: &str) -> Result<(), String> {
        self.0.answer()?;
        if self.0.refuses.lock().unwrap().iter().any(|k| k == key) {
            return Err("the keychain is full".into());
        }
        if let Some(meanwhile) = self.0.while_taking.lock().unwrap().take() {
            meanwhile();
        }
        self.0.kept.set(key, secret)
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        self.0.answer()?;
        self.0.kept.delete(key)
    }
    fn persistent(&self) -> bool {
        true
    }
}

/// A launcher starting with this keychain and secrets file.
fn launcher_with(keychain: &Arc<Keychain>, path: &Path) -> Layered {
    Layered::new(
        Box::new(Shared(keychain.clone())),
        FileStore::new(path.to_path_buf()),
    )
}

#[test]
fn the_keychain_takes_back_what_the_file_kept_while_it_was_away() {
    let path = secrets_file("recovery");
    let keychain = Arc::new(Keychain::default());
    // A is signed in while the keychain answers: no file.
    launcher_with(&keychain, &path)
        .set("A", "secret-a")
        .unwrap();
    assert!(!path.exists());
    // A start the keychain misses: B goes to the file, and A can't be read.
    keychain.set_away(true);
    let missed = launcher_with(&keychain, &path);
    missed.set("B", "secret-b").unwrap();
    assert!(missed.get("A").is_err());
    assert_eq!(missed.get("B").unwrap(), Some("secret-b".into()));
    // The keychain is back: the next start hands it B, and the file is gone.
    keychain.set_away(false);
    let back = launcher_with(&keychain, &path);
    assert_eq!(
        (
            keychain.kept.get("A").unwrap(),
            keychain.kept.get("B").unwrap()
        ),
        (Some("secret-a".into()), Some("secret-b".into()))
    );
    assert_eq!(back.get("B").unwrap(), Some("secret-b".into()));
    assert!(!path.exists());
    remove_secrets_dir(&path);
}

#[test]
fn a_launcher_on_the_file_keeps_working_after_another_hands_it_to_the_keychain() {
    let path = secrets_file("running");
    let keychain = Arc::new(Keychain::default());
    keychain.set_away(true);
    let running = launcher_with(&keychain, &path);
    running.set("A", "secret-a").unwrap();
    // The keychain comes back and a second launcher (or the CLI) starts.
    keychain.set_away(false);
    let second = launcher_with(&keychain, &path);
    assert!(!path.exists());
    // The first one still reads, renews and signs out.
    assert_eq!(running.get("A").unwrap(), Some("secret-a".into()));
    running.set("A", "renewed-a").unwrap();
    assert_eq!(second.get("A").unwrap(), Some("renewed-a".into()));
    running.delete("A").unwrap();
    assert_eq!(
        (second.get("A").unwrap(), keychain.kept.get("A").unwrap()),
        (None, None)
    );
    assert!(!path.exists());
    remove_secrets_dir(&path);
}

#[test]
fn launchers_started_on_an_old_file_find_it_after_another_hands_it_over() {
    let path = secrets_file("inherited");
    // A previous run kept A in the file.
    FileStore::new(path.clone()).set("A", "secret-a").unwrap();
    // Two launchers start while the keychain is away: their hand-overs fail. One reads A then.
    let keychain = Arc::new(Keychain::default());
    keychain.set_away(true);
    let read_before = launcher_with(&keychain, &path);
    let never_read = launcher_with(&keychain, &path);
    assert_eq!(read_before.get("A").unwrap(), Some("secret-a".into()));
    // The keychain is back and a third launcher hands A over.
    keychain.set_away(false);
    launcher_with(&keychain, &path);
    assert!(!path.exists());
    // Both running launchers still read A.
    assert_eq!(read_before.get("A").unwrap(), Some("secret-a".into()));
    assert_eq!(never_read.get("A").unwrap(), Some("secret-a".into()));
    remove_secrets_dir(&path);
}

#[test]
fn a_hand_over_by_another_process_right_after_a_start_missed_the_keychain() {
    let path = secrets_file("inherited-race");
    FileStore::new(path.clone()).set("A", "secret-a").unwrap();
    let keychain = Arc::new(Keychain::default());
    keychain.set_away(true);
    // The moment this start's hand-over fails, the keychain is back and another process hands A
    // over (the keychain takes it, the file lets it go), before this start has done anything else.
    *keychain.after_away.lock().unwrap() = Some(Box::new({
        let (keychain, path) = (keychain.clone(), path.clone());
        move || {
            keychain.set_away(false);
            keychain.kept.set("A", "secret-a").unwrap();
            FileStore::new(path).delete("A").unwrap();
        }
    }));
    let started = launcher_with(&keychain, &path);
    assert!(!path.exists());
    assert_eq!(started.get("A").unwrap(), Some("secret-a".into()));
    remove_secrets_dir(&path);
}

#[test]
fn a_secret_renewed_in_the_file_during_the_hand_over_is_found_after_the_next() {
    let path = secrets_file("renewed-in-file");
    let file = FileStore::new(path.clone());
    file.set("A", "old-a").unwrap();
    file.set("B", "secret-b").unwrap();
    let keychain = Arc::new(Keychain::default());
    // While this start hands A over, a launcher on the file renews A there; then the keychain
    // refuses B, and this run stops asking it.
    *keychain.while_taking.lock().unwrap() = Some(Box::new({
        let path = path.clone();
        move || FileStore::new(path).set("A", "renewed-a").unwrap()
    }));
    keychain.refuses.lock().unwrap().push("B".into());
    let started = launcher_with(&keychain, &path);
    assert_eq!(keychain.kept.get("A").unwrap(), Some("old-a".into()));
    // Another start hands both over: this run, which hasn't read A since, still reads both, the
    // renewed A.
    keychain.refuses.lock().unwrap().clear();
    launcher_with(&keychain, &path);
    assert!(!path.exists());
    assert_eq!(
        (started.get("A").unwrap(), started.get("B").unwrap()),
        (Some("renewed-a".into()), Some("secret-b".into()))
    );
    remove_secrets_dir(&path);
}

#[test]
fn a_move_the_keychain_half_refuses_then_a_sign_out_leaves_no_copy() {
    let path = secrets_file("half");
    let keychain = Arc::new(Keychain::default());
    keychain.set_away(true);
    let away = launcher_with(&keychain, &path);
    away.set("A", "secret-a").unwrap();
    away.set("B", "secret-b").unwrap();
    // Back, but it refuses B: A moves, B stays in the file.
    keychain.set_away(false);
    keychain.refuses.lock().unwrap().push("B".into());
    let back = launcher_with(&keychain, &path);
    assert_eq!(
        (
            keychain.kept.get("A").unwrap(),
            keychain.kept.get("B").unwrap()
        ),
        (Some("secret-a".into()), None)
    );
    assert_eq!(back.get("B").unwrap(), Some("secret-b".into()));
    // A signs out on that run, which stopped asking the keychain: no launcher reads A again...
    back.delete("A").unwrap();
    assert_eq!(back.get("A").unwrap(), None);
    // (A start where the keychain still refuses B then reads A as "away": no secret either.)
    assert_eq!(
        launcher_with(&keychain, &path).get("A").ok().flatten(),
        None
    );
    // ...and the keychain's copy goes at the first start it takes everything.
    keychain.refuses.lock().unwrap().clear();
    let later = launcher_with(&keychain, &path);
    assert_eq!(
        (keychain.kept.get("A").unwrap(), later.get("B").unwrap()),
        (None, Some("secret-b".into()))
    );
    assert!(!path.exists());
    remove_secrets_dir(&path);
}

#[test]
fn a_keychain_waiting_on_a_prompt_never_holds_the_file_up() {
    let path = secrets_file("prompt");
    let keychain = Arc::new(Keychain::default());
    keychain.set_away(true);
    launcher_with(&keychain, &path)
        .set("B", "secret-b")
        .unwrap();
    keychain.set_away(false);
    // An unlock prompt nobody answers yet, while one launcher signs A in.
    let prompt = keychain.prompt.lock().unwrap();
    let asked = keychain.asked();
    let signing_in = {
        let (keychain, path) = (keychain.clone(), path.clone());
        std::thread::spawn(move || {
            Layered {
                keychain: Box::new(Shared(keychain)),
                file: FileStore::new(path),
                away: Default::default(),
                parked: Default::default(),
            }
            .set("A", "secret-a")
        })
    };
    while keychain.asked() == asked {
        std::thread::sleep(Duration::from_millis(1));
    }
    // Another process still reads and writes the file meanwhile.
    let (done, said) = std::sync::mpsc::channel();
    let other = path.clone();
    std::thread::spawn(move || {
        let file = FileStore::new(other);
        let _ = done.send((file.get("B"), file.set("C", "secret-c")));
    });
    let (read, written) = said
        .recv_timeout(Duration::from_secs(10))
        .expect("the file waited on the keychain's prompt");
    assert_eq!((read.unwrap(), written), (Some("secret-b".into()), Ok(())));
    // The prompt is answered: A goes to the keychain, B and C are still in the file.
    drop(prompt);
    signing_in.join().unwrap().unwrap();
    let file = FileStore::new(path.clone());
    assert_eq!(keychain.kept.get("A").unwrap(), Some("secret-a".into()));
    assert_eq!(
        (
            file.get("A").unwrap(),
            file.get("B").unwrap(),
            file.get("C").unwrap()
        ),
        (None, Some("secret-b".into()), Some("secret-c".into()))
    );
    remove_secrets_dir(&path);
}

/// A start handing the file's A to the keychain, held up mid-way by an unlock prompt, while a
/// launcher already running does `meanwhile` to A. The running one waits for the hand-over instead
/// of racing it, and goes on once the prompt is answered.
fn hand_over_racing(
    name: &str,
    meanwhile: impl FnOnce(&Layered) + Send + 'static,
) -> (Arc<Keychain>, PathBuf) {
    let path = secrets_file(name);
    let keychain = Arc::new(Keychain::default());
    // Started before the file had anything: it asked the keychain nothing.
    let running = launcher_with(&keychain, &path);
    // A run the keychain missed kept A in the file.
    FileStore::new(path.clone()).set("A", "old-a").unwrap();
    let prompt = keychain.prompt.lock().unwrap();
    let asked = keychain.asked();
    let starting = {
        let (keychain, path) = (keychain.clone(), path.clone());
        std::thread::spawn(move || drop(launcher_with(&keychain, &path)))
    };
    while keychain.asked() == asked {
        std::thread::sleep(Duration::from_millis(1));
    }
    let racing = std::thread::spawn(move || meanwhile(&running));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        keychain.asked(),
        asked + 1,
        "the running launcher reached the keychain during the hand-over"
    );
    drop(prompt);
    starting.join().unwrap();
    racing.join().unwrap();
    (keychain, path)
}

#[test]
fn a_hand_over_never_brings_back_an_account_signed_out_meanwhile() {
    let (keychain, path) = hand_over_racing("race-sign-out", |running| {
        running.delete("A").unwrap();
    });
    assert_eq!(keychain.kept.get("A").unwrap(), None);
    assert_eq!(launcher_with(&keychain, &path).get("A").unwrap(), None);
    assert!(!path.exists());
    remove_secrets_dir(&path);
}

#[test]
fn a_hand_over_never_writes_over_a_token_renewed_meanwhile() {
    let (keychain, path) = hand_over_racing("race-renewal", |running| {
        running.set("A", "renewed-a").unwrap();
    });
    assert_eq!(keychain.kept.get("A").unwrap(), Some("renewed-a".into()));
    assert_eq!(
        launcher_with(&keychain, &path).get("A").unwrap(),
        Some("renewed-a".into())
    );
    assert!(!path.exists());
    remove_secrets_dir(&path);
}

#[test]
fn the_secrets_file_counts_only_sign_ins() {
    let path = secrets_file("counts");
    let file = FileStore::new(path.clone());
    assert!(!file.holds_secrets());
    file.put("A", SIGNED_OUT).unwrap();
    assert!(!file.holds_secrets(), "a sign-out mark alone");
    file.put("B", "secret-b").unwrap();
    assert!(file.holds_secrets());
    std::fs::write(&path, "{\"B\":\"secret-b").unwrap();
    assert!(!file.holds_secrets(), "a file that can't be read");
    remove_secrets_dir(&path);
}

#[test]
fn a_dismissed_prompt_is_asked_once_a_run() {
    let path = secrets_file("dismissed");
    FileStore::new(path.clone()).set("A", "secret-a").unwrap();
    let keychain = Arc::new(Keychain::default());
    keychain.set_away(true);
    // The start asks once to hand A over; nothing after asks again.
    let run = launcher_with(&keychain, &path);
    assert_eq!(keychain.asked(), 1);
    run.set("B", "secret-b").unwrap();
    run.set("A", "renewed-a").unwrap();
    assert_eq!(run.get("A").unwrap(), Some("renewed-a".into()));
    assert!(run.get("never-signed-in").is_err());
    run.delete("B").unwrap();
    assert_eq!(run.get("B").unwrap(), None);
    assert_eq!(keychain.asked(), 1);
    remove_secrets_dir(&path);
}

#[test]
fn signing_out_while_the_keychain_is_away_forgets_its_copy() {
    let path = secrets_file("sign-out");
    let keychain = Arc::new(Keychain::default());
    launcher_with(&keychain, &path)
        .set("A", "secret-a")
        .unwrap();
    keychain.set_away(true);
    let away = launcher_with(&keychain, &path);
    away.delete("A").unwrap();
    assert_eq!(away.get("A").unwrap(), None);
    // The keychain's copy goes at the first start it answers, and the file with it.
    keychain.set_away(false);
    let back = launcher_with(&keychain, &path);
    assert_eq!(
        (back.get("A").unwrap(), keychain.kept.get("A").unwrap()),
        (None, None)
    );
    assert!(!path.exists());
    remove_secrets_dir(&path);
}
