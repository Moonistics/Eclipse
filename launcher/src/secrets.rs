//! Where the launcher keeps secrets (Microsoft sign-ins, the Tailscale access token): the OS
//! keychain, a file only the player can read where there is none (Linux and FreeBSD), or memory in
//! tests. Each store also names the lock every process takes around a read, renew and write.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// The keychain service name every secret is stored under.
pub const SERVICE: &str = "dev.eclipse.launcher";

/// Where secrets are kept: the OS keychain, a file of the launcher's own when there is none, or
/// memory in tests.
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>, String>;
    fn set(&self, key: &str, secret: &str) -> Result<(), String>;
    fn delete(&self, key: &str) -> Result<(), String>;
    /// False for memory: secrets are gone when the launcher closes.
    fn persistent(&self) -> bool;
    /// The lock every launcher and CLI process takes around a read, renew and write of these
    /// secrets; memory stores need only the process lock.
    fn lock_file(&self) -> Option<PathBuf> {
        self.persistent()
            .then(|| crate::backend::data_dir().join("account-session.lock"))
    }
    /// The in-process half of that lock. The system keychain is one per computer, so every
    /// handle to it shares the process-wide lock; a memory store's secrets are its own, and so
    /// is its lock: two of them (one per unit test) never wait for each other.
    fn operations(&self) -> &Mutex<()> {
        static OPERATIONS: Mutex<()> = Mutex::new(());
        &OPERATIONS
    }
}

#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>, Mutex<()>);
impl SecretStore for MemoryStore {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
    fn set(&self, key: &str, secret: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(key.into(), secret.into());
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
    fn persistent(&self) -> bool {
        false
    }
    fn operations(&self) -> &Mutex<()> {
        &self.1
    }
}

/// The secrets file in the launcher's data folder, when no keychain answers (#5095): many Linux
/// systems run no Secret Service, and sign-ins would otherwise last one run. One JSON object, key
/// to secret, encrypted with a key of this computer's ([`seal`]), readable and writable by the
/// player only (0600 on Unix), replaced whole on every write. A lock file beside it keeps each read-modify-replace whole across every launcher and CLI
/// process. The keychain takes its secrets back at the first start it answers ([`Layered`]).
pub struct FileStore {
    path: PathBuf,
}

pub const SECRETS_FILE: &str = "account-secrets.json";

impl FileStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// The launcher's own: [`SECRETS_FILE`] in its data folder.
    pub fn launcher() -> Self {
        Self::new(crate::backend::data_dir().join(SECRETS_FILE))
    }

    fn shown(&self, e: std::io::Error) -> String {
        format!("{}: {e}", self.path.display())
    }

    /// Runs `f` holding the file's lock (`account-secrets.lock`): other processes wait.
    fn locked<T>(&self, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        self.holding("lock", f)
    }

    /// Runs `f` holding the lock file `account-secrets.<name>`, made if need be.
    fn holding<T>(&self, name: &str, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| self.shown(e))?;
        }
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.path.with_extension(name))
            .map_err(|e| self.shown(e))?;
        lock.lock().map_err(|e| self.shown(e))?;
        // Closing the file releases the lock, after `f` whatever it answers.
        f()
    }

    /// Whether the file holds a sign-in: not only sign-out marks, and readable.
    fn holds_secrets(&self) -> bool {
        self.entries()
            .is_ok_and(|secrets| secrets.values().any(|secret| secret != SIGNED_OUT))
    }

    fn read(&self) -> Result<std::collections::BTreeMap<String, String>, String> {
        match std::fs::read(&self.path) {
            // Only where the file breaks: serde's own message can quote what the file holds.
            Ok(bytes) => serde_json::from_slice(&unseal(&bytes).ok_or_else(|| {
                format!(
                    "{} can't be read on this computer: it was written on another one, or changed. \
                     Remove it and sign in again.",
                    self.path.display()
                )
            })?)
            .map_err(|e| {
                format!(
                    "{} can't be read ({:?} error at line {}, column {}).",
                    self.path.display(),
                    e.classify(),
                    e.line(),
                    e.column()
                )
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
            Err(e) => Err(self.shown(e)),
        }
    }

    /// Replaces the file with `secrets` through a temporary file of this write's own beside it, so
    /// a crash leaves the old file or the new one; removes it when there are none left.
    fn write(&self, secrets: &std::collections::BTreeMap<String, String>) -> Result<(), String> {
        if secrets.is_empty() {
            return match std::fs::remove_file(&self.path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(self.shown(e)),
                _ => Ok(()),
            };
        }
        let json = serde_json::to_vec(secrets).map_err(|e| e.to_string())?;
        crate::files::write_private(&self.path, &seal(&json)?)
    }

    fn entry(&self, key: &str) -> Result<Option<String>, String> {
        if !self.path.exists() {
            return Ok(None);
        }
        self.locked(|| Ok(self.read()?.remove(key)))
    }

    /// Everything the file holds, if it is there.
    fn entries(&self) -> Result<std::collections::BTreeMap<String, String>, String> {
        if !self.path.exists() {
            return Ok(Default::default());
        }
        self.locked(|| self.read())
    }

    fn put(&self, key: &str, value: &str) -> Result<(), String> {
        self.locked(|| {
            let mut secrets = self.read()?;
            secrets.insert(key.into(), value.into());
            self.write(&secrets)
        })
    }

    /// The keychain took `key`: removes its entry if it still holds `before`, what was read before
    /// the keychain call. Whether the file still has an entry for it: one another process wrote
    /// since stays, and is read first.
    fn settle(&self, key: &str, before: Option<&str>) -> Result<bool, String> {
        if !self.path.exists() {
            return Ok(false);
        }
        self.locked(|| {
            let mut secrets = self.read()?;
            if before.is_some() && secrets.get(key).map(String::as_str) == before {
                secrets.remove(key);
                self.write(&secrets)?;
                return Ok(false);
            }
            Ok(secrets.contains_key(key))
        })
    }
}

/// The secrets file's first bytes: format 1, the JSON sealed with ChaCha20-Poly1305 under
/// [`file_key`], then a random 12-byte nonce and the ciphertext with its tag.
const SEALED: &[u8] = b"HZSECv1\n";

/// The secrets file's key: this computer's identity (`/etc/machine-id` on Linux, `/etc/hostid` on
/// FreeBSD) hashed with a constant of the launcher's. The tokens are never on disk in plain text,
/// and a copy of the file taken off the computer (a backup, a synced home folder) can't be read
/// elsewhere. On the computer itself, the file's mode (0600) is what keeps other users out.
fn file_key() -> chacha20poly1305::Key {
    use sha2::Digest;
    let mut hash = sha2::Sha256::new();
    hash.update(b"eclipse-launcher account-secrets v1");
    if let Some(id) = ["/etc/machine-id", "/var/lib/dbus/machine-id", "/etc/hostid"]
        .iter()
        .find_map(|path| std::fs::read(path).ok())
    {
        hash.update(id.trim_ascii());
    }
    hash.finalize()
}

/// `json` sealed for the secrets file ([`SEALED`]).
fn seal(json: &[u8]) -> Result<Vec<u8>, String> {
    use chacha20poly1305::{aead::Aead, KeyInit};
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
    let sealed = chacha20poly1305::ChaCha20Poly1305::new(&file_key())
        .encrypt(&nonce.into(), json)
        .map_err(|_| "the secrets couldn't be sealed".to_string())?;
    Ok([SEALED, &nonce, &sealed].concat())
}

/// What [`seal`] sealed, or `None` for a file in another format, changed, or sealed on another
/// computer.
fn unseal(bytes: &[u8]) -> Option<Vec<u8>> {
    use chacha20poly1305::{aead::Aead, KeyInit};
    let rest = bytes.strip_prefix(SEALED)?;
    let (nonce, sealed) = rest.split_at_checked(12)?;
    let nonce: [u8; 12] = nonce.try_into().ok()?;
    chacha20poly1305::ChaCha20Poly1305::new(&file_key())
        .decrypt(&nonce.into(), sealed)
        .ok()
}

/// A file entry for an account signed out while the keychain was away: the keychain's copy, if any,
/// counts as gone, and goes at the first start it answers.
const SIGNED_OUT: &str = "";

impl SecretStore for FileStore {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        self.locked(|| Ok(self.read()?.remove(key)))
    }
    fn set(&self, key: &str, secret: &str) -> Result<(), String> {
        self.locked(|| {
            let mut secrets = self.read()?;
            secrets.insert(key.into(), secret.into());
            self.write(&secrets)
        })
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        self.locked(|| {
            let mut secrets = self.read()?;
            if secrets.remove(key).is_none() {
                return Ok(());
            }
            self.write(&secrets)
        })
    }
    fn persistent(&self) -> bool {
        true
    }
}

/// The Secret Service (GNOME Keyring, KWallet) through oo7, as gpui itself uses it.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub struct SystemStore;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
impl SystemStore {
    fn keyring() -> Result<oo7::Keyring, String> {
        futures_lite::future::block_on(async {
            // `oo7::Keyring::new` takes `GTK_USE_PORTAL=1` (set by many KDE desktops) for a
            // sandbox, and then keeps the secrets in a file under a key the Secret portal gives
            // per app id. Outside a sandbox that id comes from how the launcher was started (the
            // menu or a terminal), so the file opened in one start and not the next (#3249).
            // Flatpak (`/.flatpak-info`) and Snap (`$SNAP`) keep the portal's file.
            let sandboxed = std::path::Path::new("/.flatpak-info").exists()
                || std::env::var_os("SNAP").is_some();
            let keyring = if sandboxed {
                oo7::Keyring::new().await.map_err(|e| e.to_string())?
            } else {
                let service = oo7::dbus::Service::new().await.map_err(|e| e.to_string())?;
                oo7::Keyring::DBus(
                    service
                        .default_collection()
                        .await
                        .map_err(|e| e.to_string())?,
                )
            };
            keyring.unlock().await.map_err(|e| e.to_string())?;
            Ok(keyring)
        })
    }
}
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
impl SecretStore for SystemStore {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        let keyring = Self::keyring()?;
        futures_lite::future::block_on(async {
            let attributes = vec![("service", SERVICE), ("account", key)];
            let items = keyring
                .search_items(&attributes)
                .await
                .map_err(|e| e.to_string())?;
            let Some(item) = items.into_iter().next() else {
                return Ok(None);
            };
            item.unlock().await.map_err(|e| e.to_string())?;
            let secret = item.secret().await.map_err(|e| e.to_string())?;
            Ok(String::from_utf8(secret.to_vec()).ok())
        })
    }
    fn set(&self, key: &str, secret: &str) -> Result<(), String> {
        let keyring = Self::keyring()?;
        futures_lite::future::block_on(async {
            let attributes = vec![("service", SERVICE), ("account", key)];
            keyring
                .create_item(
                    "Eclipse launcher: Minecraft account",
                    &attributes,
                    secret.as_bytes(),
                    true,
                )
                .await
                .map_err(|e| e.to_string())
        })
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        let keyring = Self::keyring()?;
        futures_lite::future::block_on(async {
            let attributes = vec![("service", SERVICE), ("account", key)];
            keyring.delete(&attributes).await.map_err(|e| e.to_string())
        })
    }
    fn persistent(&self) -> bool {
        true
    }
}

/// Keychain on macOS, Credential Manager on Windows (the keyring crate). Credential Manager takes at
/// most 2560 bytes of UTF-16 per item and a sign-in's secrets are longer, so on Windows a secret is
/// kept in pieces: the first under the account's key, the rest under `key.1`, `key.2`... The
/// Keychain has no such limit, and every item is a consent prompt for a launcher signature it
/// doesn't know yet — pieces meant several prompts after every self-update (#4912) — so on macOS a
/// secret is one item, and pieces an older launcher left fold into it as they are read.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub struct SystemStore;

/// Bytes of UTF-8 per keychain item: at most twice as many in UTF-16, well under 2560.
#[cfg(any(target_os = "windows", test))]
const PIECE: usize = 1000;

#[cfg(any(target_os = "macos", target_os = "windows", test))]
fn piece_key(key: &str, n: usize) -> String {
    if n == 0 {
        key.to_string()
    } else {
        format!("{key}.{n}")
    }
}

/// `secret` cut into pieces of at most `PIECE` bytes, on character boundaries.
#[cfg(any(target_os = "windows", test))]
fn split_secret(mut secret: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    while secret.len() > PIECE {
        let mut cut = PIECE;
        while !secret.is_char_boundary(cut) {
            cut -= 1;
        }
        let (head, tail) = secret.split_at(cut);
        pieces.push(head);
        secret = tail;
    }
    pieces.push(secret);
    pieces
}

/// One keychain item per key: what a secret's pieces are kept in.
#[cfg(any(target_os = "macos", target_os = "windows", test))]
trait Items {
    fn read(&self, key: &str) -> Result<Option<String>, String>;
    fn write(&self, key: &str, piece: &str) -> Result<(), String>;
    /// Removes one item; false when there was none.
    fn remove(&self, key: &str) -> Result<bool, String>;
}

#[cfg(any(target_os = "windows", test))]
fn get_pieces(items: &dyn Items, key: &str) -> Result<Option<String>, String> {
    let Some(mut secret) = items.read(key)? else {
        return Ok(None);
    };
    for n in 1.. {
        match items.read(&piece_key(key, n))? {
            Some(piece) => secret.push_str(&piece),
            None => break,
        }
    }
    Ok(Some(secret))
}

#[cfg(any(target_os = "windows", test))]
fn set_pieces(items: &dyn Items, key: &str, secret: &str) -> Result<(), String> {
    let before = get_pieces(items, key).ok().flatten();
    let pieces = split_secret(secret);
    for (n, piece) in pieces.iter().enumerate() {
        if let Err(e) = items.write(&piece_key(key, n), piece) {
            // New pieces before old ones read back as no secret at all (the account would be
            // signed out at the next start): the old secret goes back, which fit before.
            if n > 0 {
                let _ = delete_pieces(items, key);
                if let Some(before) = &before {
                    for (n, piece) in split_secret(before).iter().enumerate() {
                        let _ = items.write(&piece_key(key, n), piece);
                    }
                }
            }
            return Err(e);
        }
    }
    // Pieces left over from a longer secret would be read back as part of this one.
    for n in pieces.len().. {
        match items.remove(&piece_key(key, n)) {
            Ok(true) => {}
            Ok(false) => break,
            Err(e) => {
                // Some leftovers are already gone, some are still there: the old secret goes back
                // whole, as on a refused write above.
                let _ = delete_pieces(items, key);
                if let Some(before) = &before {
                    for (n, piece) in split_secret(before).iter().enumerate() {
                        let _ = items.write(&piece_key(key, n), piece);
                    }
                }
                return Err(e);
            }
        }
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows", test))]
fn delete_pieces(items: &dyn Items, key: &str) -> Result<(), String> {
    for n in 0.. {
        if !items.remove(&piece_key(key, n))? {
            break;
        }
    }
    Ok(())
}

/// One item per key, the whole secret in it (macOS). A secret an older launcher split over `key.1`,
/// `key.2`, ... reads whole and folds into the one item, so a read touches a single item — and asks
/// once — from then on. A merged item also reads fine as "piece 0" to launchers that still split.
#[cfg(any(target_os = "macos", test))]
fn get_item(items: &dyn Items, key: &str) -> Result<Option<String>, String> {
    let Some(mut secret) = items.read(key)? else {
        return Ok(None);
    };
    let mut split = false;
    for n in 1.. {
        match items.read(&piece_key(key, n))? {
            Some(piece) => {
                secret.push_str(&piece);
                split = true;
            }
            None => break,
        }
    }
    if split {
        let _ = set_item(items, key, &secret);
    }
    Ok(Some(secret))
}

/// Writes `secret` to `key` and removes `key.1`, `key.2`, ... pieces an older, split secret left:
/// they would read back as part of this one. A removal the keychain refuses puts the old items
/// back: the merged secret beside the pieces left would read as a longer, unparseable one, and
/// pieces removed before the failure can't be had again except from here.
#[cfg(any(target_os = "macos", test))]
fn set_item(items: &dyn Items, key: &str, secret: &str) -> Result<(), String> {
    let mut before = Vec::new();
    if let Some(piece) = items.read(key)? {
        before.push((key.to_string(), piece));
        for n in 1.. {
            match items.read(&piece_key(key, n))? {
                Some(piece) => before.push((piece_key(key, n), piece)),
                None => break,
            }
        }
    }
    items.write(key, secret)?;
    for n in 1.. {
        match items.remove(&piece_key(key, n)) {
            Ok(true) => {}
            Ok(false) => break,
            Err(e) => {
                if before.is_empty() {
                    let _ = items.remove(key);
                } else {
                    for (key, piece) in &before {
                        let _ = items.write(key, piece);
                    }
                }
                return Err(e);
            }
        }
    }
    Ok(())
}

/// Credential Manager's error 8 (`ERROR_NOT_ENOUGH_MEMORY`) on a write: not the computer's memory,
/// the player's credential store is full (an undocumented limit of its total size). The Xbox app
/// and the Minecraft Launcher are known to leave hundreds of items there (#4313, #4366).
#[cfg(any(target_os = "windows", test))]
pub const CREDENTIAL_MANAGER_FULL: &str = "Windows Credential Manager is full, so Eclipse can't \
     keep your sign-in (Windows error 8; it isn't your computer's memory). Open Credential Manager \
     from the Start menu, go to Windows Credentials and remove entries you no longer use, then sign \
     in again. The Xbox app and the Minecraft Launcher often leave many XblGrts and Xbl| entries \
     there; removing an entry can sign the app that made it out.";

/// What the launcher says for a Windows error code Credential Manager gave when writing an item,
/// when it says more than the code.
#[cfg(any(target_os = "windows", test))]
fn credential_manager_says(code: u32) -> Option<&'static str> {
    const ERROR_NOT_ENOUGH_MEMORY: u32 = 8;
    (code == ERROR_NOT_ENOUGH_MEMORY).then_some(CREDENTIAL_MANAGER_FULL)
}

/// A running executable removed by an older macOS updater cannot create Keychain credentials.
#[cfg(any(target_os = "macos", test))]
pub const MACOS_LAUNCHER_RESTART: &str = "The launcher was updated while it was running. Restart the launcher, then sign in with Microsoft again.";

#[cfg(any(target_os = "macos", test))]
fn macos_keychain_says(code: i32) -> Option<&'static str> {
    // CSSM's Unix ENOENT: Keychain cannot identify an executable removed by an older updater.
    (code == 100002).then_some(MACOS_LAUNCHER_RESTART)
}

/// The keyring crate's error as the player reads it.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn write_error(e: keyring::Error) -> String {
    #[cfg(target_os = "macos")]
    if let keyring::Error::PlatformFailure(inner) = &e {
        if let Some(said) = inner
            .downcast_ref::<security_framework::base::Error>()
            .and_then(|error| macos_keychain_says(error.code()))
        {
            return said.to_string();
        }
    }
    #[cfg(target_os = "windows")]
    if let keyring::Error::PlatformFailure(inner) = &e {
        if let Some(said) = inner
            .downcast_ref::<keyring::windows::Error>()
            .and_then(|code| credential_manager_says(code.0))
        {
            return said.to_string();
        }
    }
    e.to_string()
}

/// The OS keychain's items, under [`SERVICE`].
#[cfg(any(target_os = "macos", target_os = "windows"))]
struct KeyringItems;

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl Items for KeyringItems {
    fn read(&self, key: &str) -> Result<Option<String>, String> {
        match keyring::Entry::new(SERVICE, key).and_then(|e| e.get_password()) {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
    fn write(&self, key: &str, piece: &str) -> Result<(), String> {
        keyring::Entry::new(SERVICE, key)
            .and_then(|e| e.set_password(piece))
            .map_err(write_error)
    }
    fn remove(&self, key: &str) -> Result<bool, String> {
        match keyring::Entry::new(SERVICE, key).and_then(|e| e.delete_credential()) {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl SecretStore for SystemStore {
    #[cfg(target_os = "macos")]
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        get_item(&KeyringItems, key)
    }
    #[cfg(target_os = "windows")]
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        get_pieces(&KeyringItems, key)
    }
    #[cfg(target_os = "macos")]
    fn set(&self, key: &str, secret: &str) -> Result<(), String> {
        set_item(&KeyringItems, key, secret)
    }
    #[cfg(target_os = "windows")]
    fn set(&self, key: &str, secret: &str) -> Result<(), String> {
        set_pieces(&KeyringItems, key, secret)
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        delete_pieces(&KeyringItems, key)
    }
    fn persistent(&self) -> bool {
        true
    }
}

pub const NO_KEYCHAIN: &str =
    "Your sign-in can't be saved on this computer. You'll need to sign in again next time.";

/// What the keychain answered when [`store`] fell back to memory.
static NO_KEYCHAIN_WHY: OnceLock<String> = OnceLock::new();

/// What the window and the CLI say when sign-ins last only this run: [`NO_KEYCHAIN`] and why.
/// None while the keychain keeps them.
pub fn no_keychain() -> Option<String> {
    if store().persistent() {
        return None;
    }
    Some(match NO_KEYCHAIN_WHY.get() {
        Some(why) => format!("{NO_KEYCHAIN} The keychain said: {why}"),
        None => NO_KEYCHAIN.into(),
    })
}

/// What a keychain this run stopped asking answers.
const KEYCHAIN_AWAY: &str =
    "The keychain didn't answer earlier: sign in again to keep this account.";

/// The keychain, with the secrets file for what it can't keep (#5095). Each call asks both, never a
/// choice made once: a launcher that wrote to the file while the keychain was away still finds its
/// secrets after another process handed them to the keychain. Reads take the file's copy first (it
/// is the newer one), writes go to the keychain and to the file only when it refuses.
///
/// The file's lock is never held across a keychain call, which can wait on an unlock prompt: other
/// processes keep reading and writing the file meanwhile. Every write to the keychain holds a lock of
/// its own instead (`account-secrets.keychain.lock`), with what it writes read under it, so a hand-over
/// never puts back a secret another process renewed or signed out since; only keychain writes wait
/// on a prompt, as they would on the keychain itself. A file entry is only removed if it still holds
/// what was read before the keychain call, so another process's newer write stays. And once the
/// keychain refuses or doesn't answer, this run stops asking it (no prompt after a dismissed
/// prompt), until another process hands it a secret this run saw in the file.
pub struct Layered {
    keychain: Box<dyn SecretStore>,
    file: FileStore,
    away: std::sync::atomic::AtomicBool,
    /// What this run saw in the file and didn't hand over itself: put there, read there, or there
    /// when it started. Gone from the file later, the keychain answers again (another process
    /// handed it).
    parked: Mutex<std::collections::HashSet<String>>,
}

impl Layered {
    /// Starts by handing the keychain what the file kept while it was away, one secret at a time,
    /// up to the first it refuses; sign-outs it missed are done there too. Each is read again under
    /// the keychain's lock: one another process renewed or signed out meanwhile isn't handed over.
    pub fn new(keychain: Box<dyn SecretStore>, file: FileStore) -> Self {
        let layered = Self {
            keychain,
            file,
            away: Default::default(),
            parked: Default::default(),
        };
        let keys: Vec<String> = layered
            .file
            .entries()
            .unwrap_or_default()
            .into_keys()
            .collect();
        // Seen before the hand-over starts: whatever another process hands over meanwhile, this
        // run asks the keychain for it again.
        layered.parked.lock().unwrap().extend(keys.iter().cloned());
        for key in keys {
            let handed = layered.keychain_writes(|| {
                let Some(secret) = layered.file.entry(&key)? else {
                    return Ok(());
                };
                layered.ask(|keychain| match secret.as_str() {
                    SIGNED_OUT => keychain.delete(&key),
                    _ => keychain.set(&key, &secret),
                })?;
                layered.unpark(&key, Some(secret))
            });
            if handed.is_err() {
                break;
            }
        }
        layered
    }

    /// Runs `f`, a write to the keychain and what it reads first, holding the keychain's lock.
    fn keychain_writes<T>(&self, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        self.file.holding("keychain.lock", f)
    }

    fn is_away(&self) -> bool {
        self.away.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Asks the keychain unless it was away; an error means it is away for the rest of the run.
    fn ask<T>(
        &self,
        call: impl FnOnce(&dyn SecretStore) -> Result<T, String>,
    ) -> Result<T, String> {
        if self.is_away() {
            return Err(KEYCHAIN_AWAY.into());
        }
        let answer = call(self.keychain.as_ref());
        if answer.is_err() {
            self.away.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        answer
    }

    /// Puts `value` in the file for `key`, the keychain being away.
    fn park(&self, key: &str, value: &str) -> Result<(), String> {
        self.file.put(key, value)?;
        self.parked.lock().unwrap().insert(key.into());
        Ok(())
    }

    /// The keychain took `key`: the file's copy read before goes, unless another process wrote since.
    /// Still in the file, it stays counted as seen there, so its hand-over by another process is
    /// noticed.
    fn unpark(&self, key: &str, before: Option<String>) -> Result<(), String> {
        let still_there = self.file.settle(key, before.as_deref())?;
        let mut parked = self.parked.lock().unwrap();
        if still_there {
            parked.insert(key.into());
        } else {
            parked.remove(key);
        }
        Ok(())
    }
}

impl SecretStore for Layered {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        match self.file.entry(key)? {
            Some(secret) if secret == SIGNED_OUT => return Ok(None),
            Some(secret) => {
                self.parked.lock().unwrap().insert(key.into());
                return Ok(Some(secret));
            }
            None => {}
        }
        // Seen in the file and gone from it: another process handed it to the keychain, which
        // answers again.
        if self.is_away() && self.parked.lock().unwrap().remove(key) {
            self.away.store(false, std::sync::atomic::Ordering::Relaxed);
        }
        self.ask(|keychain| keychain.get(key))
    }
    fn set(&self, key: &str, secret: &str) -> Result<(), String> {
        if self.is_away() {
            return self.park(key, secret);
        }
        self.keychain_writes(|| {
            let before = self.file.entry(key)?;
            match self.ask(|keychain| keychain.set(key, secret)) {
                Ok(()) => self.unpark(key, before),
                Err(_) => self.park(key, secret),
            }
        })
    }
    fn delete(&self, key: &str) -> Result<(), String> {
        // The keychain may hold a copy: the mark hides it until it can go.
        if self.is_away() {
            return self.park(key, SIGNED_OUT);
        }
        self.keychain_writes(|| {
            let before = self.file.entry(key)?;
            match self.ask(|keychain| keychain.delete(key)) {
                Ok(()) => self.unpark(key, before),
                Err(_) => self.park(key, SIGNED_OUT),
            }
        })
    }
    fn persistent(&self) -> bool {
        true
    }
}

/// The OS keychain if it answers, else why not.
fn keychain_or_why() -> Result<Box<dyn SecretStore>, String> {
    // Unit tests never reach the player's keychain.
    #[cfg(all(
        not(test),
        any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "macos",
            target_os = "windows"
        )
    ))]
    {
        if std::env::var_os("ECLIPSE_NO_KEYCHAIN").is_some() {
            return Err("Saving sign-ins is turned off.".into());
        }
        SystemStore.get("probe").map(|_| Box::new(SystemStore) as _)
    }
    #[cfg(not(all(
        not(test),
        any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "macos",
            target_os = "windows"
        )
    )))]
    Err("This system has no keychain.".into())
}

/// Microsoft sign-ins. Linux and FreeBSD: the Secret Service with the secrets file beside it
/// ([`Layered`], #5095). macOS and Windows: the keychain if it answers. Memory with
/// `ECLIPSE_NO_KEYCHAIN` (the launcher's own tests) or no keychain on macOS and Windows (said in the
/// UI with [`no_keychain`]).
pub fn store() -> &'static dyn SecretStore {
    static STORE: OnceLock<Box<dyn SecretStore>> = OnceLock::new();
    STORE
        .get_or_init(|| {
            #[cfg(all(not(test), any(target_os = "linux", target_os = "freebsd")))]
            if std::env::var_os("ECLIPSE_NO_KEYCHAIN").is_none() {
                return Box::new(Layered::new(Box::new(SystemStore), FileStore::launcher()));
            }
            keychain_or_why().unwrap_or_else(|why| {
                let _ = NO_KEYCHAIN_WHY.set(why);
                Box::<MemoryStore>::default()
            })
        })
        .as_ref()
}

/// Secrets never written to a file of the launcher's own, the Tailscale access token (it
/// administers the player's whole tailnet): the keychain if it answers, else memory for this run.
/// [`store`] itself on macOS and Windows, where that is what it is.
pub fn keychain() -> &'static dyn SecretStore {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        static KEYCHAIN: OnceLock<Box<dyn SecretStore>> = OnceLock::new();
        KEYCHAIN
            .get_or_init(|| keychain_or_why().unwrap_or_else(|_| Box::<MemoryStore>::default()))
            .as_ref()
    }
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    store()
}

/// Whether sign-ins sit in the secrets file now, the keychain having refused them (#5095): sign-out
/// marks alone, or a file that can't be read, don't count.
pub fn in_secrets_file() -> bool {
    cfg!(any(target_os = "linux", target_os = "freebsd"))
        && store().persistent()
        && FileStore::launcher().holds_secrets()
}

#[cfg(test)]
#[path = "secrets/tests.rs"]
pub(crate) mod tests;
