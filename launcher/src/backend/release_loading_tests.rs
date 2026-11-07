use super::{platform::*, releases::*, updates::*};
use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn errors_reach_releases_and_updates_without_fallback() {
    for error in [
        "No connection: offline",
        "GitHub's rate limit was reached.",
        "invalid JSON",
    ] {
        let mut published = None;
        let result = releases_and_updates_with(
            || Err(error.into()),
            || panic!("no installed scan after failure"),
            |_| panic!("no fallback after failure"),
            |r| published = Some(r),
        );
        assert_eq!(published.unwrap().unwrap_err(), error);
        assert_eq!(result.unwrap_err(), error);
    }
}

#[test]
fn publish_games_before_older_launcher_fallback() {
    let events = RefCell::new(Vec::new());
    let result = releases_and_updates_with(
        || Ok(serde_json::json!([{"tag_name": "v2026.10.14", "assets": []}])),
        Vec::new,
        |list| {
            launcher_releases_in_with(list, |url| {
                assert_eq!(events.borrow().first(), Some(&"releases"));
                events.borrow_mut().push("fallback");
                if url.contains("matching-refs") {
                    Ok(serde_json::json!([{"ref": "refs/tags/launcher@99.0.0"}]))
                } else {
                    Ok(
                        serde_json::json!({"tag_name": "launcher@99.0.0", "assets": [{
                            "name": LAUNCHER_ASSET.unwrap_or("unsupported"),
                            "browser_download_url": "u", "size": 1
                        }]}),
                    )
                }
            })
        },
        |r| {
            assert_eq!(r.unwrap()[0].tag, "v2026.10.14");
            events.borrow_mut().push("releases");
        },
    )
    .unwrap();
    assert_eq!(*events.borrow(), ["releases", "fallback", "fallback"]);
    assert_eq!(result.launcher.is_some(), LAUNCHER_ASSET.is_some());
}

#[test]
fn failed_launcher_fallback_keeps_game_result_and_checks_stay_fresh() {
    let requests = AtomicUsize::new(0);
    for day in [14, 15] {
        let mut published = None;
        let updates = releases_and_updates_with(
            || {
                requests.fetch_add(1, Ordering::Relaxed);
                Ok(serde_json::json!([{"tag_name": format!("v2026.10.{day}")}]))
            },
            Vec::new,
            |list| launcher_releases_in_with(list, |_| Err("offline".into())),
            |r| published = Some(r.unwrap()),
        )
        .unwrap();
        assert_eq!(published.unwrap()[0].tag, format!("v2026.10.{day}"));
        assert!(updates.launcher.is_none());
    }
    assert_eq!(requests.load(Ordering::Relaxed), 2);
}
