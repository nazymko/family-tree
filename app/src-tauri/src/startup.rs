//! What the app opens by itself at start: the folder named on the command line, else the most recent
//! tree when the setting "Reopen the last tree at startup" is on. The outcome waits in `AppState` until
//! the page asks for it (`startup_outcome`); the tree is already open by then, so an agent connected
//! over MCP sees it at once.
//!
//! A second launch with a path (the exe started again with a folder while the app runs) is IGNORED on
//! purpose: the single-instance plugin only brings the running window forward. Replacing the tree the
//! owner is editing because another program started the exe is not safe.

use crate::commands;
use crate::state::AppState;
use gt_session::{ProjectInfo, RecentProject};
use serde::Serialize;

/// What to do at start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Show the start screen.
    Nothing,
    /// Open this folder.
    Open(String),
    /// The most recent project's folder is gone: start screen plus a message.
    Missing { dir: String, name: String },
}

/// The command-line path wins over the setting; the setting opens the first (most recent) entry only.
pub fn decide(cli_path: Option<&str>, reopen_last: bool, recent: &[RecentProject]) -> Plan {
    if let Some(p) = cli_path.filter(|p| !p.trim().is_empty()) {
        return Plan::Open(p.to_string());
    }
    if !reopen_last {
        return Plan::Nothing;
    }
    match recent.first() {
        None => Plan::Nothing,
        Some(r) if r.missing => Plan::Missing {
            dir: r.dir.clone(),
            name: r.name.clone(),
        },
        Some(r) => Plan::Open(r.dir.clone()),
    }
}

/// The tree folder named on the command line: the first argument that is not a `--flag` and not the
/// value of `--open-dialog-result` (the test hook).
pub fn cli_path<I: IntoIterator<Item = String>>(args: I) -> Option<String> {
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--open-dialog-result" {
            it.next();
        } else if !a.starts_with("--") && !a.trim().is_empty() {
            return Some(a);
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupNotice {
    /// `missing`: the folder is gone. `failed`: it could not be opened (locked, newer schema, damaged).
    pub kind: String,
    pub dir: String,
    /// For `failed`, the reason; for `missing`, the tree's name.
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupOutcome {
    pub opened: Option<ProjectInfo>,
    pub notice: Option<StartupNotice>,
}

/// Runs the plan. Never fails and never edits the recent list beyond what a normal open does.
pub fn run(state: &AppState, cli: Option<&str>) -> StartupOutcome {
    let reopen = AppState::lock(&state.settings).reopen_last;
    let recent = crate::settings_file::load_recent(&state.config_dir);
    let outcome = match decide(cli, reopen, &recent) {
        Plan::Nothing => StartupOutcome {
            opened: None,
            notice: None,
        },
        Plan::Missing { dir, name } => StartupOutcome {
            opened: None,
            notice: Some(StartupNotice {
                kind: "missing".into(),
                dir,
                message: name,
            }),
        },
        Plan::Open(dir) => match commands::open_project_impl(state, &dir) {
            Ok(info) => StartupOutcome {
                opened: Some(info),
                notice: None,
            },
            Err(e) => StartupOutcome {
                opened: None,
                notice: Some(StartupNotice {
                    kind: "failed".into(),
                    dir,
                    message: e.message,
                }),
            },
        },
    };
    *AppState::lock(&state.startup) = Some(outcome.clone());
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recent(dir: &str, missing: bool) -> RecentProject {
        RecentProject {
            dir: dir.into(),
            name: "Synthetic".into(),
            opened_at: 1,
            missing,
        }
    }

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn setting_on_opens_the_most_recent() {
        let r = [recent("/t/a", false), recent("/t/b", false)];
        assert_eq!(decide(None, true, &r), Plan::Open("/t/a".into()));
    }

    #[test]
    fn setting_off_opens_nothing() {
        assert_eq!(decide(None, false, &[recent("/t/a", false)]), Plan::Nothing);
    }

    #[test]
    fn no_recent_entry_opens_nothing() {
        assert_eq!(decide(None, true, &[]), Plan::Nothing);
    }

    #[test]
    fn missing_recent_entry_gives_a_notice_plan() {
        let r = [recent("/t/gone", true), recent("/t/b", false)];
        assert_eq!(
            decide(None, true, &r),
            Plan::Missing {
                dir: "/t/gone".into(),
                name: "Synthetic".into()
            }
        );
    }

    #[test]
    fn cli_path_wins_over_setting_and_recent() {
        let r = [recent("/t/a", false)];
        assert_eq!(
            decide(Some("/t/cli"), true, &r),
            Plan::Open("/t/cli".into())
        );
        assert_eq!(
            decide(Some("/t/cli"), false, &r),
            Plan::Open("/t/cli".into())
        );
        assert_eq!(decide(Some("  "), false, &r), Plan::Nothing);
    }

    #[test]
    fn cli_parsing_skips_flags_and_hook_values() {
        assert_eq!(cli_path(v(&[])), None);
        assert_eq!(cli_path(v(&["--open-dialog-result", "x.ged"])), None);
        assert_eq!(
            cli_path(v(&["--open-dialog-result", "x.ged", "C:/trees/a"])),
            Some("C:/trees/a".into())
        );
        assert_eq!(cli_path(v(&["--other", "/t/a"])), Some("/t/a".into()));
    }
}
