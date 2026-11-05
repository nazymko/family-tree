#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use gt_app_lib::commands::{self, COMMAND_NAMES};
use gt_app_lib::protocol;
use gt_app_lib::state::AppState;
use gt_core::date::calendar::{Calendar, first_jdn};
use gt_gedcom::{ImportOptions, import};
use gt_session::{ApiError, ErrorCode, OwnerKind, OwnerRef, PersonQuery, Session, SessionClock};
use gt_store::Project;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponseBody};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{App, Listener, WebviewWindow};

const NOW_MS: i64 = 1_793_664_000_000; // 2026-11-03

fn today() -> i32 {
    first_jdn(Calendar::Gregorian, 2026, Some(11), Some(3)).unwrap()
}

fn state_in(root: &Path) -> AppState {
    let config = root.join("config");
    let hashes = gt_app_lib::agent_access::FileHashes::new(&config);
    AppState::with_clock(
        config,
        Box::new(|| SessionClock::fixed(today(), NOW_MS)),
        Box::new(hashes),
    )
}

const HEAD: &str = "0 HEAD\n1 SOUR TEST\n2 VERS 1.0\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n";

fn ged(n: usize, born: &str) -> String {
    let mut s = HEAD.to_string();
    for i in 0..n {
        s += &format!("0 @I{i}@ INDI\n1 NAME Person{i:04} /Test/\n1 BIRT\n2 DATE {born}\n");
    }
    s + "0 TRLR\n"
}

/// A project folder built from GEDCOM text.
fn project_from(root: &Path, name: &str, ged: &str) -> PathBuf {
    let out = import(ged.as_bytes(), &ImportOptions::default());
    let dir = root.join(name);
    let mut p = Project::create(&dir).unwrap();
    p.save_tree(&out.tree, today()).unwrap();
    dir
}

fn first_id(state: &AppState) -> uuid::Uuid {
    state
        .with_session(|s| Ok(s.list_persons(&PersonQuery::default(), 0, 1)))
        .unwrap()
        .items[0]
        .person
        .id
}

// ---- a mock app with the real handler, for tests that go through IPC ----

fn app_with(state: AppState) -> (App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let app = protocol::register(tauri::test::mock_builder())
        .manage(state)
        .invoke_handler(commands::handler())
        .build(mock_context(noop_assets()))
        .unwrap();
    let w = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    (app, w)
}

fn invoke(w: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
    let req = InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "http://tauri.localhost".parse().unwrap(),
        body: InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    };
    get_ipc_response(w, req).map(|b| match b {
        InvokeResponseBody::Json(s) => serde_json::from_str(&s).unwrap(),
        InvokeResponseBody::Raw(_) => Value::Null,
    })
}

fn query_json() -> Value {
    serde_json::to_value(PersonQuery::default()).unwrap()
}

// ---- tests ----

#[test]
fn every_api_method_has_a_command() {
    let api = std::fs::read_to_string(format!(
        "{}/../src/lib/api/api.ts",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let body = api
        .split("export interface Api {")
        .nth(1)
        .unwrap()
        .split("\n}")
        .next()
        .unwrap();
    let mut methods = Vec::new();
    for line in body.lines() {
        // Method lines start with two spaces and a name followed by `(`.
        let Some(rest) = line.strip_prefix("  ") else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if name.is_empty() || !rest[name.len()..].starts_with('(') {
            continue;
        }
        methods.push(name);
    }
    assert!(methods.len() > 30, "parsed {methods:?}");
    // `mediaUrl` is a synchronous string builder (the gtmedia: scheme, not a command).
    let exempt = ["mediaUrl", "traceImageUrl"];
    for m in methods.iter().filter(|m| !exempt.contains(&m.as_str())) {
        let mut snake = String::new();
        for c in m.chars() {
            if c.is_ascii_uppercase() {
                snake.push('_');
                snake.push(c.to_ascii_lowercase());
            } else {
                snake.push(c);
            }
        }
        assert!(
            COMMAND_NAMES.contains(&snake.as_str()),
            "no command {snake}"
        );
    }
    assert_eq!(COMMAND_NAMES.len(), 65);
    let unique: std::collections::HashSet<&&str> = COMMAND_NAMES.iter().collect();
    assert_eq!(unique.len(), COMMAND_NAMES.len(), "a command listed twice");
}

#[test]
fn no_project_error_code() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let err = invoke(
        &w,
        "list_persons",
        json!({"q": query_json(), "offset": 0, "limit": 10}),
    )
    .unwrap_err();
    assert_eq!(err["code"], "no_project");
    for cmd in ["undo", "undo_state", "last_citation"] {
        assert_eq!(
            invoke(&w, cmd, json!({})).unwrap_err()["code"],
            "no_project"
        );
    }
}

#[test]
fn errors_serialise_as_code_message() {
    let e = ApiError::new(ErrorCode::NotFound, "gone");
    assert_eq!(
        serde_json::to_value(&e).unwrap(),
        json!({"code": "not_found", "message": "gone"})
    );
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let err = invoke(
        &w,
        "open_project",
        json!({"dir": dir.path().join("nothing").to_str().unwrap()}),
    )
    .unwrap_err();
    let obj = err.as_object().unwrap();
    assert_eq!(obj.len(), 2);
    assert!(obj["code"].is_string() && !obj["message"].as_str().unwrap().is_empty());
}

#[test]
fn open_then_list_then_close() {
    let dir = tempfile::tempdir().unwrap();
    let proj = project_from(dir.path(), "tree", &ged(5, "1900"));
    let (_app, w) = app_with(state_in(dir.path()));
    let info = invoke(&w, "open_project", json!({"dir": proj.to_str().unwrap()})).unwrap();
    assert_eq!(info["personCount"], 5);
    let list = json!({"q": query_json(), "offset": 0, "limit": 10});
    let page = invoke(&w, "list_persons", list.clone()).unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 5);
    let recent = invoke(&w, "recent_projects", json!({})).unwrap();
    assert_eq!(recent.as_array().unwrap().len(), 1);
    assert_eq!(recent[0]["missing"], false);
    invoke(&w, "close_project", json!({})).unwrap();
    let err = invoke(&w, "list_persons", list).unwrap_err();
    assert_eq!(err["code"], "no_project");
}

#[test]
fn list_persons_rejects_nothing_over_200() {
    let dir = tempfile::tempdir().unwrap();
    let proj = project_from(dir.path(), "tree", &ged(250, "1900"));
    let (_app, w) = app_with(state_in(dir.path()));
    invoke(&w, "open_project", json!({"dir": proj.to_str().unwrap()})).unwrap();
    let page = invoke(
        &w,
        "list_persons",
        json!({"q": query_json(), "offset": 0, "limit": 10000}),
    )
    .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 200);
    assert_eq!(page["total"], 250);
}

#[test]
fn import_emits_progress_events_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("in.ged");
    std::fs::write(&file, ged(20, "1900")).unwrap();
    let (app, w) = app_with(state_in(dir.path()));
    let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
    let sink = seen.clone();
    app.listen("import-progress", move |e| {
        sink.lock()
            .unwrap()
            .push(serde_json::from_str(e.payload()).unwrap());
    });
    let req = json!({
        "file": file.to_str().unwrap(),
        "projectDir": dir.path().join("imported").to_str().unwrap(),
        "projectName": "Imported",
        "mediaRoots": [],
        "dayOrder": "dmy",
    });
    let report = invoke(&w, "import_gedcom", json!({ "req": req })).unwrap();
    assert_eq!(report["project"]["personCount"], 20);
    let events = seen.lock().unwrap().clone();
    let phases = ["reading", "parsing", "copying_media", "saving", "indexing"];
    let rank = |e: &Value| phases.iter().position(|p| e["phase"] == *p).unwrap();
    assert!(events.len() >= 3, "{events:?}");
    assert!(
        events.windows(2).all(|p| rank(&p[0]) <= rank(&p[1])),
        "{events:?}"
    );
    assert_eq!(events[0]["phase"], "reading");
    // The import is the open project now, and the report can be saved.
    let out = dir.path().join("report.txt");
    invoke(
        &w,
        "save_import_report",
        json!({"dest": out.to_str().unwrap()}),
    )
    .unwrap();
    assert!(!std::fs::read_to_string(out).unwrap().is_empty());
}

#[test]
fn living_threshold_change_recomputes() {
    let dir = tempfile::tempdir().unwrap();
    let proj = project_from(dir.path(), "tree", &ged(1, "1930"));
    let (_app, w) = app_with(state_in(dir.path()));
    invoke(&w, "open_project", json!({"dir": proj.to_str().unwrap()})).unwrap();
    let living = |w: &WebviewWindow<MockRuntime>| {
        let page = invoke(
            w,
            "list_persons",
            json!({"q": query_json(), "offset": 0, "limit": 10}),
        )
        .unwrap();
        page["items"][0]["living"].as_bool().unwrap()
    };
    assert!(living(&w), "aged 96 with a 100-year rule");
    let s = invoke(&w, "set_settings", json!({"patch": {"livingMaxAge": 80}})).unwrap();
    assert_eq!(s["livingMaxAge"], 80);
    assert!(!living(&w), "aged 96 with an 80-year rule");
    invoke(&w, "close_project", json!({})).unwrap();
    // The stored flag follows too.
    let p = Project::open(&proj).unwrap();
    let flag: i64 = p
        .conn()
        .query_row("SELECT living_cached FROM person LIMIT 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(flag, 0);
}

#[test]
fn home_person_persists_in_project() {
    let dir = tempfile::tempdir().unwrap();
    let proj = project_from(dir.path(), "tree", &ged(3, "1900"));
    let state = state_in(dir.path());
    commands::open_project_impl(&state, proj.to_str().unwrap()).unwrap();
    let id = first_id(&state);
    let (_app, w) = app_with(state);
    let s = invoke(&w, "set_settings", json!({"patch": {"homePerson": id}})).unwrap();
    assert_eq!(s["homePerson"], json!(id));
    invoke(&w, "close_project", json!({})).unwrap();
    let info = invoke(&w, "open_project", json!({"dir": proj.to_str().unwrap()})).unwrap();
    assert_eq!(info["homePerson"], json!(id));
    // `null` clears it; an unknown id is refused.
    let s = invoke(&w, "set_settings", json!({"patch": {"homePerson": null}})).unwrap();
    assert!(s["homePerson"].is_null());
    let err = invoke(
        &w,
        "set_settings",
        json!({"patch": {"homePerson": uuid::Uuid::now_v7()}}),
    )
    .unwrap_err();
    assert_eq!(err["code"], "not_found");
}

#[test]
fn app_settings_apply_without_a_project_and_reach_the_next_one() {
    let dir = tempfile::tempdir().unwrap();
    let proj = project_from(dir.path(), "tree", &ged(1, "1900"));
    let state = state_in(dir.path());
    let s = commands::set_settings_impl(
        &state,
        serde_json::from_value(json!({"theme": "dark", "dayOrder": "mdy"})).unwrap(),
    )
    .unwrap();
    assert_eq!(s.backups_to_keep, 10);
    let refused = commands::set_settings_impl(
        &state,
        serde_json::from_value(json!({"livingMaxAge": 90})).unwrap(),
    );
    assert_eq!(refused.unwrap_err().code, ErrorCode::NoProject);
    commands::open_project_impl(&state, proj.to_str().unwrap()).unwrap();
    let now = commands::get_settings_impl(&state);
    assert_eq!(format!("{:?}", now.theme), "Dark");
    assert_eq!(format!("{:?}", now.day_order), "Mdy");
    // And it survived on disk for the next run.
    let again = state_in(dir.path());
    assert_eq!(
        format!("{:?}", commands::get_settings_impl(&again).theme),
        "Dark"
    );
}

fn backups(proj: &Path) -> usize {
    std::fs::read_dir(proj.join("backups"))
        .map(|d| d.filter_map(Result::ok).count())
        .unwrap_or(0)
}

#[test]
fn daily_backup_created_once_per_day() {
    let dir = tempfile::tempdir().unwrap();
    let proj = project_from(dir.path(), "tree", &ged(2, "1900"));
    let state = state_in(dir.path());
    let p = proj.to_str().unwrap();
    commands::open_project_impl(&state, p).unwrap();
    assert_eq!(backups(&proj), 1);
    commands::open_project_impl(&state, p).unwrap();
    assert_eq!(backups(&proj), 1, "a second open the same day adds none");
    // Age the only backup past 24 hours: the next open takes a new one.
    let old = std::fs::read_dir(proj.join("backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let two_days = std::time::SystemTime::now() - std::time::Duration::from_secs(48 * 3600);
    std::fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(two_days)
        .unwrap();
    commands::open_project_impl(&state, p).unwrap();
    assert_eq!(backups(&proj), 2);
}

// ---- gtmedia ----

fn png() -> Vec<u8> {
    let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        40,
        30,
        image::Rgb([200, 30, 30]),
    ));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

/// An open project with one picture attached; returns the state and the media id.
fn state_with_media(root: &Path) -> (AppState, uuid::Uuid) {
    let proj = project_from(root, "tree", &ged(1, "1900"));
    let state = state_in(root);
    commands::open_project_impl(&state, proj.to_str().unwrap()).unwrap();
    let pic = root.join("pic.png");
    std::fs::write(&pic, png()).unwrap();
    let owner = OwnerRef {
        kind: OwnerKind::Person,
        id: first_id(&state),
    };
    let res = state
        .with_session(|s| s.attach_files(owner, &[pic]))
        .unwrap();
    (state, res.attached[0].id)
}

#[test]
fn gtmedia_protocol_serves_thumb_and_404() {
    let dir = tempfile::tempdir().unwrap();
    let (state, id) = state_with_media(dir.path());
    let ok = protocol::respond(&state, &format!("/{id}/thumb"));
    assert_eq!(ok.status(), 200);
    assert_eq!(ok.headers()["content-type"], "image/jpeg");
    assert_eq!(
        ok.headers()["cache-control"],
        "private, max-age=31536000, immutable"
    );
    assert!(ok.body().starts_with(&[0xFF, 0xD8]));
    let orig = protocol::respond(&state, &format!("/{id}/original"));
    assert_eq!(orig.status(), 200);
    assert_eq!(orig.headers()["content-type"], "image/png");
    assert_eq!(
        protocol::respond(&state, &format!("/{id}/preview")).status(),
        200
    );

    let missing = protocol::respond(&state, &format!("/{}/thumb", uuid::Uuid::now_v7()));
    assert_eq!(missing.status(), 404);
    assert!(missing.headers().get("cache-control").is_none());

    // No project open: nothing to serve.
    commands::close_project_impl(&state);
    assert_eq!(
        protocol::respond(&state, &format!("/{id}/thumb")).status(),
        404
    );
}

#[test]
fn gtmedia_url_built_by_the_ui_is_served_and_encoded_slash_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let (state, id) = state_with_media(dir.path());
    // TS: convertFileSrc(id, 'gtmedia') + '/' + size  ->  http://gtmedia.localhost/<id>/thumb
    // (gtmedia://localhost/<id>/thumb elsewhere); the webview hands the URI path to the handler.
    for url in [
        format!("http://gtmedia.localhost/{id}/thumb"),
        format!("gtmedia://localhost/{id}/thumb"),
    ] {
        let path = url.parse::<tauri::http::Uri>().unwrap().path().to_string();
        assert_eq!(protocol::respond(&state, &path).status(), 200, "{url}");
    }
    // The old form: convertFileSrc("<id>/thumb") percent-encodes the slash.
    let bad = format!("http://gtmedia.localhost/{id}%2Fthumb");
    let path = bad.parse::<tauri::http::Uri>().unwrap().path().to_string();
    assert_eq!(protocol::respond(&state, &path).status(), 400, "{bad}");
}

#[test]
fn open_media_externally_resolves_only_stored_media() {
    let dir = tempfile::tempdir().unwrap();
    let (state, id) = state_with_media(dir.path());
    let p = state.with_session(|s| s.media_original_path(id)).unwrap();
    assert!(p.is_file());
    assert!(p.to_string_lossy().contains("media"));
    let unknown = state.with_session(|s| s.media_original_path(uuid::Uuid::now_v7()));
    assert!(unknown.is_err());
}

#[test]
fn gtmedia_rejects_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let (state, id) = state_with_media(dir.path());
    for bad in [
        "/../../etc/passwd".to_string(),
        "/..%2F..%2Fetc%2Fpasswd/thumb".to_string(),
        "/%2e%2e/thumb".to_string(),
        format!("/{id}/../thumb"),
        format!("/{id}/thumb/extra"),
        format!("/{id}/thumb.jpg"),
        format!("/{id}/THUMB"),
        format!("/{id}"),
        format!("//{id}/thumb"),
        format!("/{{{id}}}/thumb"),
        format!("/urn:uuid:{id}/thumb"),
        format!("/{}/thumb", id.simple()),
        "/".to_string(),
        String::new(),
        "/C:/Windows/win.ini/thumb".to_string(),
    ] {
        let r = protocol::respond(&state, &bad);
        assert_eq!(r.status(), 400, "{bad}");
        assert!(r.headers().get("cache-control").is_none());
    }
    assert!(protocol::parse_path(&format!("/{id}/thumb")).is_some());
}

#[test]
fn session_is_send_so_it_can_live_in_the_mutex() {
    fn needs_send<T: Send>() {}
    needs_send::<Session>();
}

#[test]
fn get_family_reads_a_union_and_refuses_unknown_ids() {
    let dir = tempfile::tempdir().unwrap();
    let ged = format!(
        "{HEAD}0 @I1@ INDI\n1 NAME A /X/\n1 FAMS @F1@\n0 @I2@ INDI\n1 NAME B /X/\n1 FAMS @F1@\n\
         0 @I3@ INDI\n1 NAME C /X/\n1 FAMC @F1@\n0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n0 TRLR\n"
    );
    let proj = project_from(dir.path(), "tree", &ged);
    let state = state_in(dir.path());
    commands::open_project_impl(&state, proj.to_str().unwrap()).unwrap();
    let fid = state
        .with_session(|s| Ok(*s.tree().families.keys().next().unwrap()))
        .unwrap();
    let (_app, w) = app_with(state);
    let f = invoke(&w, "get_family", json!({"id": fid})).unwrap();
    assert_eq!(f["id"], json!(fid));
    assert!(f["partners"][0]["name"].is_string() && f["partners"][1]["name"].is_string());
    assert_eq!(f["children"].as_array().unwrap().len(), 1);
    assert_eq!(f["children"][0]["preferred"], true);
    let err = invoke(&w, "get_family", json!({"id": uuid::Uuid::now_v7()})).unwrap_err();
    assert_eq!(err["code"], "not_found");
}

#[test]
fn open_media_externally_refuses_files_that_are_not_images_or_pdfs() {
    let dir = tempfile::tempdir().unwrap();
    let (state, id) = state_with_media(dir.path());
    // A genuine picture with a matching extension passes.
    let path = commands::checked_original_path(&state, id).unwrap();
    assert!(path.is_file());
    // The stored file is replaced by a Windows executable (what a hostile GEDCOM could bring in),
    // keeping a harmless-looking name: the content is checked, not just the extension.
    std::fs::write(&path, b"MZ\x90\x00\x03\x00\x00\x00 not a picture").unwrap();
    let err = commands::checked_original_path(&state, id).unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
    assert!(
        err.message.contains("only open photos and PDF"),
        "{}",
        err.message
    );
    // Content and extension must agree: the OS picks the program by extension, so a file with
    // PDF or GIF magic bytes under another name (a script disguised as a picture) is refused.
    std::fs::write(&path, b"%PDF-1.7\n%%EOF").unwrap();
    assert_eq!(
        commands::checked_original_path(&state, id)
            .unwrap_err()
            .code,
        ErrorCode::Invalid
    );
    std::fs::write(&path, b"GIF89a<script>alert(1)</script>").unwrap();
    assert_eq!(
        commands::checked_original_path(&state, id)
            .unwrap_err()
            .code,
        ErrorCode::Invalid
    );
    // Unknown ids still fail as not found.
    let e = commands::checked_original_path(&state, uuid::Uuid::now_v7()).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

// ---- save_picture ----

const PNG_HEAD: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

fn png_bytes() -> Vec<u8> {
    let mut v = PNG_HEAD.to_vec();
    v.extend_from_slice(b"rest of a picture");
    v
}

fn invoke_save_picture(
    w: &WebviewWindow<MockRuntime>,
    path: &str,
    bytes: Vec<u8>,
) -> Result<Value, Value> {
    use base64::Engine as _;
    let mut headers = tauri::http::HeaderMap::new();
    headers.insert(
        "x-gt-path",
        base64::engine::general_purpose::STANDARD
            .encode(path.as_bytes())
            .parse()
            .unwrap(),
    );
    let req = InvokeRequest {
        cmd: "save_picture".into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "http://tauri.localhost".parse().unwrap(),
        body: InvokeBody::Raw(bytes),
        headers,
        invoke_key: INVOKE_KEY.to_string(),
    };
    get_ipc_response(w, req).map(|b| match b {
        InvokeResponseBody::Json(s) => serde_json::from_str(&s).unwrap(),
        InvokeResponseBody::Raw(_) => Value::Null,
    })
}

#[test]
fn save_picture_writes_the_bytes_through_ipc() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let dest = dir.path().join("Pedigree - Ivan Koval.png");
    let out = invoke_save_picture(&w, dest.to_str().unwrap(), png_bytes()).unwrap();
    assert_eq!(out["bytes"], png_bytes().len());
    assert_eq!(std::fs::read(&dest).unwrap(), png_bytes());
}

#[test]
fn save_picture_rejects_bad_input_with_a_plain_error() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let missing = dir.path().join("no such folder").join("a.png");
    for (path, bytes) in [
        ("".to_string(), png_bytes()),
        (dir.path().to_str().unwrap().to_string(), png_bytes()),
        (missing.to_str().unwrap().to_string(), png_bytes()),
        (
            dir.path().join("b.png").to_str().unwrap().to_string(),
            b"not a picture".to_vec(),
        ),
        (
            dir.path().join("c.png").to_str().unwrap().to_string(),
            vec![],
        ),
    ] {
        let err = invoke_save_picture(&w, &path, bytes).unwrap_err();
        assert!(
            err["code"] == "invalid" || err["code"] == "io",
            "{path}: {err}"
        );
        let msg = err["message"].as_str().unwrap();
        assert!(msg.ends_with('.') && !msg.contains("Os {"), "{msg}");
    }
    assert!(!missing.exists());
    assert!(!dir.path().join("b.png").exists());
    assert!(!dir.path().join("c.png").exists());
}

#[test]
fn save_picture_writes_an_svg_and_a_non_ascii_name() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"></svg>".to_vec();
    let dest = dir.path().join("Предки - Іван Коваль.svg");
    let out = invoke_save_picture(&w, dest.to_str().unwrap(), svg.clone()).unwrap();
    assert_eq!(out["bytes"], svg.len());
    assert_eq!(std::fs::read(&dest).unwrap(), svg);
}

#[test]
fn save_picture_checks_the_extension_against_the_content() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let svg = b"<svg></svg>".to_vec();
    for (name, bytes) in [
        ("a.txt", png_bytes()),
        ("b.png", svg.clone()),
        ("c.svg", png_bytes()),
        ("noext", png_bytes()),
    ] {
        let p = dir.path().join(name);
        let err = invoke_save_picture(&w, p.to_str().unwrap(), bytes).unwrap_err();
        assert_eq!(err["code"], "invalid", "{name}");
        assert!(!p.exists(), "{name}");
    }
}

#[test]
fn save_picture_without_a_path_header_or_with_a_json_body_is_a_plain_error() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let req = |body: InvokeBody| InvokeRequest {
        cmd: "save_picture".into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "http://tauri.localhost".parse().unwrap(),
        body,
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    };
    let err = get_ipc_response(&w, req(InvokeBody::Raw(png_bytes())))
        .map(|_| ())
        .unwrap_err();
    assert_eq!(err["code"], "invalid");
    let err = get_ipc_response(&w, req(InvokeBody::Json(json!([1, 2, 3]))))
        .map(|_| ())
        .unwrap_err();
    assert_eq!(err["code"], "invalid");
}
