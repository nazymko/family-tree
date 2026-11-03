#![allow(clippy::unwrap_used)]

use gt_app_lib::settings_file::*;
use gt_session::{DayOrder, NameOrder, Theme};

#[test]
fn settings_atomic_write_and_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config");

    // No file: the defaults of the brief.
    let d = load_settings(&cfg);
    assert_eq!(d.day_order, DayOrder::Dmy);
    assert_eq!(d.name_order, NameOrder::GivenFirst);
    assert_eq!(d.theme, Theme::System);
    assert_eq!(d.backups_to_keep, 10);

    let mut s = d.clone();
    s.theme = Theme::Dark;
    s.backups_to_keep = 3;
    s.day_order = DayOrder::Mdy;
    save_settings(&cfg, &s).unwrap();
    assert_eq!(load_settings(&cfg), s);
    // The temp file was renamed away, and the JSON is camelCase on disk.
    assert!(!cfg.join("settings.json.tmp").exists());
    let text = std::fs::read_to_string(cfg.join(SETTINGS_FILE)).unwrap();
    assert!(text.contains("\"dayOrder\"") && text.contains("\"backupsToKeep\""));

    // A partial file keeps the other defaults; a corrupt one falls back entirely.
    std::fs::write(cfg.join(SETTINGS_FILE), r#"{"theme":"light"}"#).unwrap();
    let p = load_settings(&cfg);
    assert_eq!(p.theme, Theme::Light);
    assert_eq!(p.backups_to_keep, 10);
    std::fs::write(cfg.join(SETTINGS_FILE), "{ not json").unwrap();
    assert_eq!(load_settings(&cfg), AppSettings::default());
}

#[test]
fn recent_list_dedups_caps_and_marks_missing() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config");
    let mk = |n: usize| {
        let d = dir.path().join(format!("p{n}"));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("tree.db"), b"x").unwrap();
        d
    };
    for n in 0..15 {
        let d = mk(n);
        touch_recent(&cfg, d.to_str().unwrap(), &format!("P{n}"), n as i64).unwrap();
    }
    let list = load_recent(&cfg);
    assert_eq!(list.len(), MAX_RECENT);
    assert_eq!(list[0].name, "P14");
    assert_eq!(list[11].name, "P3");

    // The same folder spelled differently ("dir/./") replaces its entry and moves to the top.
    let again = format!("{}/./", mk(5).display());
    touch_recent(&cfg, &again, "Five", 99).unwrap();
    let list = load_recent(&cfg);
    assert_eq!(list.len(), MAX_RECENT);
    assert_eq!(list[0].name, "Five");
    assert!(list.iter().all(|r| r.name != "P5"));

    // A folder that has gone is listed but marked missing.
    std::fs::remove_file(mk(14).join("tree.db")).unwrap();
    let list = load_recent(&cfg);
    let p14 = list.iter().find(|r| r.name == "P14").unwrap();
    assert!(p14.missing);
    assert!(!list[0].missing);

    remove_recent(&cfg, mk(14).to_str().unwrap()).unwrap();
    assert!(load_recent(&cfg).iter().all(|r| r.name != "P14"));
}
