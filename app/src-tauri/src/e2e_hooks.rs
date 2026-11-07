//! Test-only hooks for the end-to-end suite. Compiled into debug builds only (`cfg(debug_assertions)` on the module),
//! so a release build has neither the command nor the argument parsing. Even in a debug build the hook stays off
//! unless the environment variable `GT_E2E=1` is set, so a developer's `tauri dev` session is never affected.
//!
//! The page asks `e2e_hooks` once at start. When enabled it learns the dialog results given on the command line
//! (`--open-dialog-result <path>`, repeatable) and then drives the native-dialog and drag-drop `Platform` itself.

use serde::Serialize;
use tauri::Runtime;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct E2eHooks {
    /// Answers for the next file/folder/save dialogs, in order.
    pub dialog_results: Vec<String>,
}

/// Collects every value after `--open-dialog-result` (also `--open-dialog-result=<value>`).
pub fn parse_dialog_results<I: IntoIterator<Item = String>>(args: I) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--open-dialog-result" {
            if let Some(v) = it.next() {
                out.push(v);
            }
        } else if let Some(v) = a.strip_prefix("--open-dialog-result=") {
            out.push(v.to_string());
        }
    }
    out
}

/// Are the test hooks on? Only `GT_E2E=1` turns them on; anything else leaves the app exactly as it is.
pub fn enabled() -> bool {
    flag_on(std::env::var("GT_E2E").ok().as_deref())
}

fn flag_on(value: Option<&str>) -> bool {
    value == Some("1")
}

/// With hooks on, `GT_E2E_CONFIG_DIR` replaces the settings/recents folder, so a test run never touches the user's own.
pub fn config_dir_override() -> Option<std::path::PathBuf> {
    if !enabled() {
        return None;
    }
    std::env::var_os("GT_E2E_CONFIG_DIR")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
}

/// With hooks on, the windows of the config start without taking the keyboard focus, so a test window never pulls
/// the focus away from the owner's work. `GT_E2E_FOCUS=1` turns this off, for watching a test run.
pub fn keep_windows_quiet<R: Runtime>(context: &mut tauri::Context<R>) {
    if !enabled() || std::env::var("GT_E2E_FOCUS").is_ok_and(|v| v == "1") {
        return;
    }
    for window in &mut context.config_mut().app.windows {
        window.focus = false;
    }
}

/// `None` when the hooks are off.
#[tauri::command]
pub fn e2e_hooks() -> Option<E2eHooks> {
    enabled().then(|| E2eHooks {
        dialog_results: parse_dialog_results(std::env::args().skip(1)),
    })
}

pub fn handler<R: Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![e2e_hooks]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn collects_repeated_and_equals_forms() {
        let got = parse_dialog_results(v(&[
            "--open-dialog-result",
            "a.ged",
            "--x",
            "--open-dialog-result=C:/b",
        ]));
        assert_eq!(got, v(&["a.ged", "C:/b"]));
    }

    #[test]
    fn only_the_exact_value_one_turns_the_hooks_on() {
        assert!(flag_on(Some("1")));
        for off in [
            None,
            Some(""),
            Some("0"),
            Some("true"),
            Some("11"),
            Some(" 1"),
        ] {
            assert!(!flag_on(off), "{off:?}");
        }
    }

    #[test]
    fn no_flag_no_results_and_trailing_flag_is_ignored() {
        assert!(parse_dialog_results(v(&["--other"])).is_empty());
        assert!(parse_dialog_results(v(&["--open-dialog-result"])).is_empty());
    }
}
