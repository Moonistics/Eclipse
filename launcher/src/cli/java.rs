use super::json;
use clap::Subcommand;

#[derive(Subcommand)]
pub(super) enum JavaCmd {
    /// The Java runtimes Eclipse downloaded from Mojang and the ones installed on this computer
    List,
    /// Download Mojang's Java runtime: a major (8, 16, 17, 21, 25) or a component name from a
    /// version's JSON (java-runtime-delta)
    Install { java: String },
}

pub(super) fn java(action: JavaCmd, as_json: bool) -> Result<(), String> {
    use eclipse_launcher::java;
    match action {
        JavaCmd::List => {
            let mut all = java::mojang_runtimes(&java::runtimes_dir());
            all.extend(java::installed());
            if as_json {
                json(serde_json::json!(all));
            } else if all.is_empty() {
                println!("No Java yet. Eclipse downloads Mojang's when an instance needs it.");
            } else {
                for r in &all {
                    let from = match r.source {
                        java::Source::Mojang => "Mojang's, downloaded by Eclipse",
                        java::Source::Installed => "installed",
                    };
                    println!("Java {}  ({from})\n  {}", r.version, r.home.display());
                }
            }
        }
        JavaCmd::Install { java: which } => {
            let need = java::Need::named(&which)?;
            let runtime = java::install(&need, &|line| eprintln!("{line}"))?;
            if as_json {
                json(serde_json::json!(runtime));
            } else {
                println!(
                    "Java {} is ready: {}",
                    runtime.version,
                    runtime.home.display()
                );
            }
        }
    }
    Ok(())
}
