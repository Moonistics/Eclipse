use super::json;
use eclipse_launcher::backend;
use eclipse_launcher::instances;

pub(super) fn logs(lines: usize, crash: bool, as_json: bool) -> Result<(), String> {
    let (path, text) = backend::read_logs(lines, crash)?;
    if as_json {
        json(serde_json::json!({ "path": path, "text": text }));
    } else {
        print!("{text}");
    }
    Ok(())
}

pub(super) fn share(
    instance: Option<String>,
    as_json: bool,
    settings: &backend::Settings,
) -> Result<(), String> {
    use eclipse_launcher::game_log;
    let i = match instance {
        Some(name) => instances::find(&name)?,
        None => instances::selected(settings),
    };
    let path = backend::latest_log_in(&i.home);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let shared = game_log::share_private(&text, &i.home)?;
    if as_json {
        json(serde_json::json!({
            "url": shared.url, "delete": format!("{}:{}", shared.id, shared.token),
        }));
    } else {
        println!("{}", shared.url);
        println!(
            "Delete it: eclipse-launcher unshare-log {}:{}",
            shared.id, shared.token
        );
    }
    Ok(())
}

pub(super) fn unshare(shared: String) -> Result<(), String> {
    let (id, token) = shared
        .split_once(':')
        .ok_or("Give the \"<id>:<token>\" share-log printed.")?;
    eclipse_launcher::game_log::unshare(&eclipse_launcher::game_log::Shared {
        url: String::new(),
        id: id.into(),
        token: token.into(),
    })?;
    println!("Deleted the shared log {id}.");
    Ok(())
}
