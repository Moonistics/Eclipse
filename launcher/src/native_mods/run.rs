use super::diagnostics;
use super::mixins::diagnostic;
use crate::native_cache::Dirs;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// What a cargo output line says about the compile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cargo {
    /// `Compiling name vX`: a crate started.
    Compiling(String),
    /// Cargo's progress bar, `Building [===>   ] 12/430: names`: units done, and in all.
    Progress(usize, usize),
}

/// `line` without terminal escape sequences (`ESC [ ... letter`).
fn without_escapes(line: &str) -> String {
    let mut plain = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                for d in chars.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            plain.push(c);
        }
    }
    plain
}

/// Cargo's events in one line of its standard error. With `CARGO_TERM_PROGRESS_WHEN=always` the
/// progress bar is redrawn with carriage returns, so a line holds several pieces.
pub(super) fn parse_cargo(line: &str) -> Vec<Cargo> {
    let mut out = Vec::new();
    for piece in without_escapes(line).split('\r') {
        let piece = piece.trim();
        if let Some(name) = piece.strip_prefix("Compiling ") {
            out.push(Cargo::Compiling(
                name.split_whitespace().next().unwrap_or(name).to_owned(),
            ));
        } else if let Some(bar) = piece.strip_prefix("Building [") {
            let counts = bar.split_once("] ").map(|(_, rest)| rest).unwrap_or("");
            let counts = counts.split(':').next().unwrap_or("");
            if let Some((done, total)) = counts.split_once('/') {
                if let (Ok(done), Ok(total)) = (done.trim().parse(), total.trim().parse()) {
                    out.push(Cargo::Progress(done, total));
                }
            }
        }
    }
    out
}

/// What of `line` the build log keeps: all of it but cargo's progress bar, whose redraws are
/// separated by carriage returns and may be followed on the same line by a real message
/// (`Building [=>] 3/9: a\r\x1b[Kerror[E0425]: ...`). None when nothing else is left.
fn log_text(line: &str) -> Option<String> {
    let kept: Vec<String> = without_escapes(line)
        .split('\r')
        .filter(|piece| {
            let piece = piece.trim();
            !piece.is_empty() && !piece.starts_with("Building [")
        })
        .map(str::to_owned)
        .collect();
    (!kept.is_empty()).then(|| kept.join("\n"))
}

/// The build's log, kept until the next build.
pub fn log_path() -> PathBuf {
    Dirs::current().log()
}

/// Makes the build log fresh for a new attempt: its folder made, the last build's log
/// emptied. The error says why it couldn't, so the Build log page can show it instead of
/// the old file's contents.
pub fn clear_log() -> Result<(), String> {
    fs::create_dir_all(Dirs::current().root()).map_err(|e| e.to_string())?;
    fs::write(log_path(), "").map_err(|e| format!("{}: {e}", log_path().display()))
}

/// Appends `message` (with a newline) to the build log, so a failure that happens before any
/// command runs is still recorded.
pub fn append_log(message: &str) -> Result<(), String> {
    let mut log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
        .map_err(|e| format!("{}: {e}", log_path().display()))?;
    writeln!(log, "{message}").map_err(|e| e.to_string())
}

/// Appends `text` to the build log, the way a compiler warning reads. A log that can't be
/// written loses only the warning: the build goes on.
pub(super) fn log_warning(text: &str) {
    let _ = append_log(&format!("warning: {text}"));
}

/// Runs `cmd`, its output appended to the build log; `line` sees each line of its standard error.
/// An error ends with its first `error` lines and the last lines: a failed build script's output
/// alone could push the line naming its crate out of the last ones.
pub(super) fn run(cmd: Command, what: &str, line: impl FnMut(&str)) -> Result<(), String> {
    run_checked(cmd, what, line, None).map_err(|e| e.error)
}

/// A [`run_checked`] that failed: its error, the last lines of its standard error, and the mods
/// the compiler's errors were traced to (`id version`).
pub(super) struct Ran {
    pub(super) error: String,
    pub(super) tail: String,
    pub(super) owners: Vec<String>,
}

pub(super) fn run_checked(
    mut cmd: Command,
    what: &str,
    mut line: impl FnMut(&str),
    compiler: Option<diagnostics::Compiler<'_>>,
) -> Result<(), Ran> {
    let ran = |error: String| Ran {
        error,
        tail: String::new(),
        owners: Vec::new(),
    };
    let mut log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
        .map_err(|e| ran(e.to_string()))?;
    let _ = writeln!(log, "$ {cmd:?}");
    let stdout = if compiler.is_some() {
        cmd.stdout(Stdio::piped());
        Some(log.try_clone().map_err(|e| ran(e.to_string()))?)
    } else {
        cmd.stdout(log.try_clone().map_err(|e| ran(e.to_string()))?);
        None
    };
    let mut child = cmd
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ran(app_control(format!("Could not start {what}: {e}"))))?;
    std::thread::scope(|scope| {
        let messages = compiler.zip(stdout).map(|(compiler, log)| {
            let input = BufReader::new(child.stdout.take().expect("piped"));
            scope.spawn(move || diagnostics::capture(&compiler, input, log))
        });
        let mut tail = std::collections::VecDeque::new();
        let mut errors = Vec::new();
        let mut blocked = false;
        for text in BufReader::new(child.stderr.take().expect("piped"))
            .lines()
            .map_while(Result::ok)
        {
            if let Some(kept) = log_text(&text) {
                let _ = writeln!(log, "{kept}");
            }
            line(&text);
            blocked |= blocked_by_app_control(&text);
            if text.starts_with("error") && errors.len() < 4 {
                errors.push(text.clone());
            }
            if tail.len() == 12 {
                tail.pop_front();
            }
            tail.push_back(text);
        }
        let status = child.wait().map_err(|e| ran(e.to_string()))?;
        let messages = messages
            .map(|m| m.join().expect("compiler output reader"))
            .unwrap_or_default();
        if status.success() {
            return Ok(());
        }
        let tail: Vec<String> = tail.into();
        let owners = messages.owners();
        let error = messages.explain(failure(what, &status.to_string(), &errors, &tail));
        Err(Ran {
            error: match blocked {
                true => with_app_control(error),
                false => app_control(error),
            },
            tail: tail.join("\n"),
            owners,
        })
    })
}

/// What to do when Windows' Smart App Control refused one of the build's programs (#5911). It
/// lets only signed programs or ones Microsoft knows run, and a build runs programs it has just
/// compiled (every crate's build script, horizon-mixins, the game) and the toolchain's own
/// (`dlltool.exe`). Windows refuses them with os error 4551 ("An Application Control policy has
/// blocked this file"). Nothing is checked before the build: with Smart App Control on, a build
/// whose programs already ran before it was turned on still succeeds.
fn smart_app_control() -> String {
    crate::i18n::tr("Windows' Smart App Control is blocking the build (os error 4551). \
It only lets signed programs run, and a native build runs programs no one has signed: the ones it \
compiles on this computer and some of the compiler's own. To build native mods, turn it off in \
Windows Security > App & browser control > Smart App Control settings, then build again. On some \
Windows versions it can't be turned back on without reinstalling Windows. Content mods (.emod) need \
no build and work with it on.")
    .to_string()
}

/// Whether `text` (a build's output, or the error starting one of its programs) is Windows
/// refusing a program under an Application Control policy, Smart App Control's.
fn blocked_by_app_control(text: &str) -> bool {
    text.contains("os error 4551")
}

/// `error` led by what to do when Windows' Smart App Control refused one of the build's
/// programs (#5911): cargo's own lines only say a build script was "never executed".
fn app_control(error: String) -> String {
    match blocked_by_app_control(&error) {
        true => with_app_control(error),
        false => error,
    }
}

fn with_app_control(error: String) -> String {
    format!("{}\n\n{error}", smart_app_control())
}

/// [`run`]'s error: the `errors` lines not already among the last ones (`tail`), then those.
fn failure(what: &str, status: &str, errors: &[String], tail: &[String]) -> String {
    let end = diagnostic(&tail.join("\n")).unwrap_or_else(|| tail.join("\n"));
    let first: Vec<&str> = errors
        .iter()
        .filter(|e| !tail.contains(e))
        .map(String::as_str)
        .collect();
    let (shown, first) = if first.is_empty() {
        ("The end of its output", String::new())
    } else {
        (
            "Its first errors and the end of its output",
            format!("{}\n…\n", first.join("\n")),
        )
    };
    format!(
        "{what} failed ({status}). {shown}, the whole of it in {}:\n{first}{end}",
        log_path().display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargos_output_gives_the_crate_and_the_progress() {
        assert_eq!(
            parse_cargo("   Compiling serde v1.0.228"),
            vec![Cargo::Compiling("serde".into())]
        );
        assert_eq!(
            parse_cargo("   Building [==>        ] 12/430: serde, syn"),
            vec![Cargo::Progress(12, 430)]
        );
        // The bar is redrawn with carriage returns and escapes, then the next line follows.
        assert_eq!(
            parse_cargo(
                "   Building [=>   ] 3/9: a\r\x1b[K   Building [==>  ] 4/9: b\r      \r   Compiling b v0.1.0"
            ),
            vec![
                Cargo::Progress(3, 9),
                Cargo::Progress(4, 9),
                Cargo::Compiling("b".into())
            ]
        );
        assert!(parse_cargo("warning: unused variable").is_empty());
        assert!(parse_cargo("    Finished `fast` profile in 1m").is_empty());
        // The log leaves out the progress bar, never what follows it on the same line.
        assert_eq!(log_text("   Building [=>   ] 3/9: a"), None);
        assert_eq!(log_text("\x1b[K"), None);
        assert_eq!(
            log_text("   Building [=>   ] 3/9: a\r   Compiling b v1").as_deref(),
            Some("   Compiling b v1")
        );
        assert_eq!(
            log_text("   Building [=>   ] 3/9: a\r\x1b[Kerror[E0425]: cannot find `x`").as_deref(),
            Some("error[E0425]: cannot find `x`")
        );
        assert_eq!(
            log_text("\x1b[K\x1b[33mwarning\x1b[0m: unused").as_deref(),
            Some("warning: unused")
        );
        assert_eq!(log_text("warning: x").as_deref(), Some("warning: x"));
    }

    #[test]
    fn a_failed_build_names_the_crate_its_output_pushed_out() {
        // cargo's output on Linux without ALSA's development files (Rust 1.99.1, 2026-10-31):
        // the build script's report fills the last lines.
        let errors = ["error: failed to run custom build command for `alsa-sys v0.3.1`".to_owned()];
        let tail: Vec<String> = [
            "  The system library `alsa` required by crate `alsa-sys` was not found.",
            "  HINT: you may need to install a package such as alsa, alsa-dev or alsa-devel.",
            "warning: build failed, waiting for other jobs to finish...",
        ]
        .map(str::to_owned)
        .into();
        let e = failure("Compiling the game", "exit status: 101", &errors, &tail);
        assert!(e.starts_with("Compiling the game failed (exit status: 101). Its first errors"));
        assert!(
            e.ends_with(&format!("{}\n…\n{}", errors[0], tail.join("\n"))),
            "{e}"
        );
        // An error still among the last lines isn't said twice.
        let e = failure("horizon-mixins", "exit status: 1", &errors, &errors);
        assert!(e.contains("The end of its output") && e.matches("alsa-sys").count() == 1);
    }

    /// Smart App Control refusing a build script (#5911): cargo names the crate, Windows' reason
    /// is only in a `Caused by` line in the middle of its output, so the error leads with what to do.
    #[cfg(unix)]
    #[test]
    fn a_build_smart_app_control_blocked_says_how_to_build() {
        let _guard = crate::native_mods::tests::LOG_TEST.lock().unwrap();
        clear_log().unwrap();
        // cargo's output on Windows 11 25H2 with Smart App Control on (the report in #5911).
        let mut cmd = Command::new("sh");
        cmd.args(["-c", r#"
            echo 'error: failed to run custom build command for `quote v1.0.47`' >&2
            echo 'Caused by:' >&2
            echo '  could not execute process `C:\target\build-script-build` (never executed)' >&2
            echo 'Caused by:' >&2
            echo '  An Application Control policy has blocked this file. (os error 4551)' >&2
            for i in 1 2 3 4 5 6 7 8 9 10 11 12 13; do echo "warning: build failed, waiting ($i)" >&2; done
            exit 101
        "#]);
        let e = run(cmd, "Building horizon-mixins", |_| {}).unwrap_err();
        assert!(e.starts_with(&smart_app_control()), "{e}");
        assert!(e.contains("quote v1.0.47"), "{e}");
        // Another failure says nothing about it.
        let mut cmd = Command::new("sh");
        cmd.args([
            "-c",
            "echo 'error: could not compile `eclipse-client`' >&2; exit 101",
        ]);
        let e = run(cmd, "Compiling the game", |_| {}).unwrap_err();
        assert!(!e.contains("Smart App Control"), "{e}");
    }
}
