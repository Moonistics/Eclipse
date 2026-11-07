use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

/// A process whose output is kept: the last `LOG_KEEP` lines, and its result once it ends.
pub struct Logged {
    pub running: Running,
    pub log: Arc<Mutex<std::collections::VecDeque<String>>>,
    pub exit: Arc<Mutex<Option<Result<(), String>>>>,
}

pub const LOG_KEEP: usize = 2000;

impl Logged {
    pub fn start(cmd: Command) -> Result<Self, String> {
        Self::start_then(cmd, |_| {})
    }

    pub fn start_then(
        cmd: Command,
        then: impl FnOnce(&Result<(), String>) + Send + 'static,
    ) -> Result<Self, String> {
        let log = Arc::new(Mutex::new(std::collections::VecDeque::new()));
        let exit = Arc::new(Mutex::new(None));
        let (l, e) = (log.clone(), exit.clone());
        let then = Mutex::new(Some(then));
        let running = spawn(cmd, move |out| match out {
            Output::Line(line) => {
                let mut log = l.lock().unwrap();
                log.push_back(line);
                if log.len() > LOG_KEEP {
                    log.pop_front();
                }
            }
            Output::Exit(result) => {
                let result = result.map_err(|e| e.to_string());
                if let Some(then) = then.lock().unwrap().take() {
                    then(&result);
                }
                *e.lock().unwrap() = Some(result);
            }
        })?;
        Ok(Self { running, log, exit })
    }

    pub fn finished(&self) -> Option<Result<(), String>> {
        self.exit.lock().unwrap().clone()
    }

    pub fn tail(&self, lines: usize) -> Vec<String> {
        let log = self.log.lock().unwrap();
        log.iter()
            .skip(log.len().saturating_sub(lines))
            .cloned()
            .collect()
    }

    pub fn wait(&self) -> Result<(), String> {
        loop {
            if let Some(result) = self.finished() {
                return result;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    /// Asks a server to stop, and kills it if it has not after `grace`.
    pub fn stop(&mut self, grace: std::time::Duration) {
        self.running.send("stop");
        let start = std::time::Instant::now();
        while self.finished().is_none() && start.elapsed() < grace {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if self.finished().is_none() {
            self.running.kill();
        }
    }
}

pub enum Output {
    Line(String),
    Exit(Result<(), Ended>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Ended {
    Status(String),
    Failed(String),
}

impl std::fmt::Display for Ended {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ended::Status(status) => write!(f, "Exited with {status}"),
            Ended::Failed(e) => f.write_str(e),
        }
    }
}

/// A running process; its output arrives as `Output::Line`, its end as `Output::Exit`.
pub struct Running {
    pub child: Arc<Mutex<Child>>,
    pub stdin: Option<ChildStdin>,
}

impl Running {
    pub fn send(&mut self, line: &str) {
        if let Some(stdin) = &mut self.stdin {
            let _ = writeln!(stdin, "{line}");
            let _ = stdin.flush();
        }
    }
    pub fn kill(&self) {
        let _ = self.child.lock().unwrap().kill();
    }
}

/// Prepare a command's Windows runtime and keep its guard alive through process exit.
pub struct RuntimeGuard {
    pub(crate) _slot: fs::File,
    pub(crate) _source: fs::File,
}

pub fn prepare_command(cmd: Command) -> Result<(Command, Option<RuntimeGuard>), String> {
    #[cfg(windows)]
    let cmd = crate::windows_runtime::redirect(cmd);
    #[cfg(windows)]
    let guard = crate::windows_runtime::prepare(&cmd).map_err(|e| {
        crate::i18n::trf(
            "Could not run {program}: {error}",
            &[
                ("program", &cmd.get_program().to_string_lossy()),
                ("error", &e.to_string()),
            ],
        )
    })?;
    #[cfg(not(windows))]
    let guard = None;
    Ok((cmd, guard))
}

pub fn spawn(
    cmd: Command,
    report: impl Fn(Output) + Send + Sync + 'static,
) -> Result<Running, String> {
    let (mut cmd, runtime) = prepare_command(cmd)?;
    crate::process::hide_console(&mut cmd);
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start {:?}: {e}", cmd.get_program()))?;
    let stdin = child.stdin.take();
    let report = Arc::new(report);
    let mut readers = Vec::new();
    let out = child
        .stdout
        .take()
        .map(|o| Box::new(o) as Box<dyn Read + Send>);
    let err = child
        .stderr
        .take()
        .map(|o| Box::new(o) as Box<dyn Read + Send>);
    for stream in [out, err].into_iter().flatten() {
        let report = report.clone();
        readers.push(std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                report(Output::Line(line));
            }
        }));
    }
    let child = Arc::new(Mutex::new(child));
    let waited = child.clone();
    std::thread::spawn(move || {
        for r in readers {
            let _ = r.join();
        }
        let status = loop {
            if let Some(s) = waited.lock().unwrap().try_wait().transpose() {
                break s;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        };
        let result = match status {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => Err(Ended::Status(s.to_string())),
            Err(e) => Err(Ended::Failed(e.to_string())),
        };
        drop(runtime);
        report(Output::Exit(result));
    });
    Ok(Running { child, stdin })
}
