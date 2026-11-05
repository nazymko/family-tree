use serde_json::Value;

fn read(path: &str) -> Value {
    let p = format!("{}/{path}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(p).expect("read")).expect("json")
}

#[test]
fn csp_is_locked_down() {
    let conf = read("tauri.conf.json");
    let csp = conf["app"]["security"]["csp"].as_str().expect("csp");
    assert!(csp.contains("default-src 'self'"));
    let rest = csp
        .replace("http://ipc.localhost", "")
        .replace("http://gtmedia.localhost", "");
    assert!(
        !rest.contains("http://") && !rest.contains("https://"),
        "{csp}"
    );
}

#[test]
fn csp_lets_the_player_load_recordings_from_the_media_scheme_only() {
    let conf = read("tauri.conf.json");
    let csp = conf["app"]["security"]["csp"].as_str().expect("csp");
    let media = csp
        .split(';')
        .map(str::trim)
        .find(|d| d.starts_with("media-src"))
        .expect("a media-src directive: without it default-src 'self' blocks gtmedia: audio");
    let sources: Vec<_> = media.split_whitespace().skip(1).collect();
    assert_eq!(
        sources,
        ["'self'", "gtmedia:", "http://gtmedia.localhost"],
        "{csp}"
    );
}

#[test]
fn window_and_identity() {
    let conf = read("tauri.conf.json");
    assert_eq!(conf["identifier"], "app.gene-tree.desktop");
    let w = &conf["app"]["windows"][0];
    assert_eq!(w["label"], "main");
    assert_eq!(
        (w["width"].as_u64(), w["height"].as_u64()),
        (Some(1280), Some(800))
    );
    assert_eq!(
        (w["minWidth"].as_u64(), w["minHeight"].as_u64()),
        (Some(960), Some(640))
    );
    assert_eq!(w["dragDropEnabled"], true);
}

#[test]
fn capabilities_are_exact() {
    let cap = read("capabilities/default.json");
    let perms: Vec<&str> = cap["permissions"]
        .as_array()
        .expect("perms")
        .iter()
        .map(|v| v.as_str().expect("str"))
        .collect();
    assert_eq!(
        perms,
        [
            "core:default",
            "dialog:allow-open",
            "dialog:allow-save",
            "opener:allow-reveal-item-in-dir"
        ]
    );
    assert!(
        perms
            .iter()
            .all(|p| !p.starts_with("fs:") && !p.starts_with("http:"))
    );
    let conf = read("tauri.conf.json");
    assert!(conf["plugins"].get("updater").is_none());
}
