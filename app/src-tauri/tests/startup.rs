#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use gt_app_lib::settings_file;
use gt_app_lib::startup;
use gt_app_lib::state::AppState;
use gt_core::date::calendar::{Calendar, first_jdn};
use gt_session::SessionClock;
use gt_store::Project;
use std::path::{Path, PathBuf};

fn state_in(root: &Path) -> AppState {
    let config = root.join("config");
    let hashes = gt_app_lib::agent_access::FileHashes::new(&config);
    let today = first_jdn(Calendar::Gregorian, 2026, Some(11), Some(3)).unwrap();
    AppState::with_clock(
        config,
        Box::new(move || SessionClock::fixed(today, 1_793_664_000_000)),
        Box::new(hashes),
    )
}

/// An empty synthetic tree folder.
fn tree(root: &Path, name: &str) -> PathBuf {
    let dir = root.join(name);
    drop(Project::create(&dir).unwrap());
    dir
}

fn is_open(state: &AppState) -> bool {
    AppState::lock(&state.session).is_some()
}

#[test]
fn setting_on_opens_the_most_recent_tree_and_agents_see_it() {
    let root = tempfile::tempdir().unwrap();
    let a = tree(root.path(), "a");
    let b = tree(root.path(), "b");
    let state = state_in(root.path());
    settings_file::touch_recent(&state.config_dir, a.to_str().unwrap(), "A", 1).unwrap();
    settings_file::touch_recent(&state.config_dir, b.to_str().unwrap(), "B", 2).unwrap();
    let out = startup::run(&state, None);
    assert_eq!(out.opened.unwrap().dir, b.to_str().unwrap());
    assert!(out.notice.is_none());
    // The same session an agent host asks on every call.
    assert!(is_open(&state));
    assert!(AppState::lock(&state.startup).is_some());
}

#[test]
fn setting_off_leaves_the_start_screen() {
    let root = tempfile::tempdir().unwrap();
    let a = tree(root.path(), "a");
    let state = state_in(root.path());
    settings_file::touch_recent(&state.config_dir, a.to_str().unwrap(), "A", 1).unwrap();
    AppState::lock(&state.settings).reopen_last = false;
    let out = startup::run(&state, None);
    assert!(out.opened.is_none() && out.notice.is_none());
    assert!(!is_open(&state));
}

#[test]
fn a_missing_folder_gives_a_message_and_keeps_the_recent_entry() {
    let root = tempfile::tempdir().unwrap();
    let gone = root.path().join("gone");
    let state = state_in(root.path());
    settings_file::touch_recent(&state.config_dir, gone.to_str().unwrap(), "Gone", 1).unwrap();
    let out = startup::run(&state, None);
    assert!(out.opened.is_none());
    assert_eq!(out.notice.unwrap().kind, "missing");
    assert_eq!(settings_file::load_recent(&state.config_dir).len(), 1);
    assert!(!is_open(&state));
}

#[test]
fn a_tree_held_by_another_instance_falls_back_with_a_message() {
    let root = tempfile::tempdir().unwrap();
    let a = tree(root.path(), "a");
    let state = state_in(root.path());
    settings_file::touch_recent(&state.config_dir, a.to_str().unwrap(), "A", 1).unwrap();
    // Another opener holds the lock.
    let _held = Project::open(&a).unwrap();
    let out = startup::run(&state, None);
    assert!(out.opened.is_none());
    assert_eq!(out.notice.unwrap().kind, "failed");
    assert_eq!(settings_file::load_recent(&state.config_dir).len(), 1);
    assert!(!is_open(&state));
}

#[test]
fn the_command_line_path_wins_over_the_recent_entry() {
    let root = tempfile::tempdir().unwrap();
    let a = tree(root.path(), "a");
    let cli = tree(root.path(), "cli");
    let state = state_in(root.path());
    settings_file::touch_recent(&state.config_dir, a.to_str().unwrap(), "A", 1).unwrap();
    AppState::lock(&state.settings).reopen_last = false;
    let out = startup::run(&state, cli.to_str());
    assert_eq!(out.opened.unwrap().dir, cli.to_str().unwrap());
}
