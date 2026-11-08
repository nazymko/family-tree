//! What the About section of Settings shows: the build's version, and the one optional network call of the app,
//! "Check for a newer version". Nothing here runs by itself: the check happens only when the person presses the button.
//!
//! The version comes from git at build time (`build.rs`): the newest release tag `vX.Y.Z` reachable from the commit, and
//! how many commits lie after it. The files in the repository keep an old number (a release writes its version only into
//! its own copy), so `CARGO_PKG_VERSION` is only the fallback for a build made without git.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Runtime};
use tauri_plugin_opener::OpenerExt;

pub const REPO_URL: &str = "https://github.com/nazymko/family-tree";
const LATEST_API: &str = "https://api.github.com/repos/nazymko/family-tree/releases/latest";
const TIMEOUT: Duration = Duration::from_secs(8);

/// The facts `build.rs` collected; each may be missing.
#[derive(Debug, Clone, Copy, Default)]
pub struct BuildFacts<'a> {
    /// Version in the manifests (`CARGO_PKG_VERSION`).
    pub pkg: &'a str,
    /// `git describe --tags --long`: `v0.3.0-12-g1a2b3c4`.
    pub describe: Option<&'a str>,
    pub commit: Option<&'a str>,
    /// `YYYY-MM-DD`.
    pub date: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    /// What to show: `0.3.0` for a release, `0.3.0+12` for a build twelve commits after it.
    pub version: String,
    /// The release this build is based on, `0.3.0`; compared with the newest release.
    pub base: String,
    /// True when the build is exactly a release tag.
    pub release: bool,
    pub commit: Option<String>,
    pub date: Option<String>,
}

/// `v0.3.0-12-g1a2b3c4` to (`0.3.0`, 12, `1a2b3c4`). The tag may itself hold dashes (`v0.4.0-rc.1`).
fn split_describe(d: &str) -> Option<(&str, u32, &str)> {
    let (rest, hash) = d.rsplit_once('-')?;
    let hash = hash.strip_prefix('g')?;
    let (tag, ahead) = rest.rsplit_once('-')?;
    let version = tag.strip_prefix('v')?;
    Some((version, ahead.parse().ok()?, hash))
}

pub fn info_from(f: BuildFacts) -> AppInfo {
    let from_git = f
        .describe
        .and_then(split_describe)
        .filter(|(v, _, _)| Semver::parse(v).is_some());
    let (base, ahead, hash) = match from_git {
        Some((v, n, h)) => (v.to_string(), Some(n), Some(h.to_string())),
        None => (f.pkg.to_string(), None, f.commit.map(str::to_string)),
    };
    let release = ahead == Some(0);
    let version = match ahead {
        Some(n) if n > 0 => format!("{base}+{n}"),
        _ => base.clone(),
    };
    AppInfo {
        version,
        base,
        release,
        commit: hash.or_else(|| f.commit.map(str::to_string)),
        date: f.date.map(str::to_string),
    }
}

/// The info of this very build.
pub fn current() -> AppInfo {
    info_from(BuildFacts {
        pkg: env!("CARGO_PKG_VERSION"),
        describe: option_env!("GT_GIT_DESCRIBE"),
        commit: option_env!("GT_GIT_COMMIT"),
        date: option_env!("GT_GIT_DATE"),
    })
}

/// `major.minor.patch` with an optional pre-release part; a pre-release is older than the same release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Semver {
    nums: [u64; 3],
    pre: Option<String>,
}

impl Semver {
    pub fn parse(text: &str) -> Option<Semver> {
        let text = text.trim().trim_start_matches('v');
        let text = text.split('+').next()?;
        let (core, pre) = match text.split_once('-') {
            Some((c, p)) => (c, Some(p.to_string())),
            None => (text, None),
        };
        let mut parts = core.split('.');
        let mut nums = [0u64; 3];
        for n in &mut nums {
            *n = parts.next()?.parse().ok()?;
        }
        parts.next().is_none().then_some(Semver { nums, pre })
    }
}

impl PartialOrd for Semver {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Semver {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        self.nums
            .cmp(&other.nums)
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Equal,
                (Some(_), None) => Less,
                (None, Some(_)) => Greater,
                (Some(a), Some(b)) => a.cmp(b),
            })
    }
}

/// The three fields of GitHub's answer the app reads.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub html_url: String,
    pub published_at: Option<String>,
}

pub fn parse_release(json: &str) -> Option<Release> {
    serde_json::from_str(json).ok()
}

/// Only pages of this project's repository are ever opened from the app.
pub fn is_project_url(url: &str) -> bool {
    url == REPO_URL
        || url
            .strip_prefix(REPO_URL)
            .is_some_and(|r| r.starts_with('/') || r.starts_with('?'))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FailReason {
    /// No connection, a timeout, or a blocked request.
    Offline,
    /// GitHub answered with an error status.
    Server,
    /// The answer was not a release.
    BadReply,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum UpdateStatus {
    Latest {
        latest: String,
    },
    Newer {
        version: String,
        url: String,
        published: Option<String>,
    },
    Failed {
        reason: FailReason,
    },
}

/// What came back from the network: the body of a 200 answer, or why there is none.
pub type Fetched = Result<String, FailReason>;

/// Compares the running build with GitHub's newest release. `fetched` is the answer of the request, made by the caller.
pub fn status_from(running: &AppInfo, fetched: Fetched) -> UpdateStatus {
    let body = match fetched {
        Ok(b) => b,
        Err(reason) => return UpdateStatus::Failed { reason },
    };
    let bad = UpdateStatus::Failed {
        reason: FailReason::BadReply,
    };
    let Some(rel) = parse_release(&body) else {
        return bad;
    };
    let (Some(latest), Some(mine)) = (Semver::parse(&rel.tag_name), Semver::parse(&running.base))
    else {
        return bad;
    };
    let version = rel.tag_name.trim_start_matches('v').to_string();
    if latest > mine {
        let url = if is_project_url(&rel.html_url) {
            rel.html_url
        } else {
            format!("{REPO_URL}/releases/latest")
        };
        UpdateStatus::Newer {
            version,
            url,
            published: rel.published_at,
        }
    } else {
        UpdateStatus::Latest { latest: version }
    }
}

async fn fetch_latest() -> Fetched {
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(format!("Family-Tree/{}", current().version))
        .build()
        .map_err(|_| FailReason::Offline)?;
    let resp = client
        .get(LATEST_API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| FailReason::Offline)?;
    if !resp.status().is_success() {
        return Err(FailReason::Server);
    }
    resp.text().await.map_err(|_| FailReason::Offline)
}

/// The one network call nobody asked for by importing or reading a picture: it runs only when the button is pressed.
pub async fn check_for_update() -> UpdateStatus {
    status_from(&current(), fetch_latest().await)
}

/// Opens a page of the project's repository in the system browser. Returns false for any other address.
pub fn open_project_link<R: Runtime>(app: AppHandle<R>, url: String) -> bool {
    is_project_url(&url) && app.opener().open_url(url, None::<&str>).is_ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn facts(describe: Option<&str>) -> BuildFacts<'_> {
        BuildFacts {
            pkg: "0.1.0",
            describe,
            commit: Some("abc1234"),
            date: Some("2026-11-08"),
        }
    }

    #[test]
    fn exact_tag_is_a_release() {
        let i = info_from(facts(Some("v0.3.0-0-gabc1234")));
        assert_eq!(
            (i.version.as_str(), i.base.as_str(), i.release),
            ("0.3.0", "0.3.0", true)
        );
        assert_eq!(i.commit.as_deref(), Some("abc1234"));
    }

    #[test]
    fn commits_after_a_tag_show_the_distance() {
        let i = info_from(facts(Some("v0.3.0-12-g1a2b3c4")));
        assert_eq!(
            (i.version.as_str(), i.base.as_str(), i.release),
            ("0.3.0+12", "0.3.0", false)
        );
        assert_eq!(i.commit.as_deref(), Some("1a2b3c4"));
        assert_eq!(i.date.as_deref(), Some("2026-11-08"));
    }

    #[test]
    fn prerelease_tag_keeps_its_dashes() {
        let i = info_from(facts(Some("v0.4.0-rc.1-3-gdeadbee")));
        assert_eq!(
            (i.version.as_str(), i.base.as_str()),
            ("0.4.0-rc.1+3", "0.4.0-rc.1")
        );
    }

    #[test]
    fn no_git_falls_back_to_the_manifest_version() {
        let i = info_from(BuildFacts {
            pkg: "0.1.0",
            ..Default::default()
        });
        assert_eq!((i.version.as_str(), i.release), ("0.1.0", false));
        assert_eq!((i.commit, i.date), (None, None));
        let i = info_from(facts(None));
        assert_eq!((i.version.as_str(), i.release), ("0.1.0", false));
        assert_eq!(i.commit.as_deref(), Some("abc1234"));
        let i = info_from(facts(Some("nonsense")));
        assert_eq!(i.version, "0.1.0");
    }

    #[test]
    fn semver_orders_numbers_not_text() {
        let v = |s: &str| Semver::parse(s).unwrap();
        assert!(v("0.10.0") > v("0.9.9"));
        assert!(v("v1.0.0") > v("0.99.99"));
        assert!(v("0.3.0") > v("0.3.0-rc.1"));
        assert!(v("0.3.0+12") == v("0.3.0"));
        assert!(Semver::parse("0.3").is_none());
        assert!(Semver::parse("nightly").is_none());
        assert!(Semver::parse("1.2.3.4").is_none());
    }

    const REPLY: &str = r#"{"tag_name":"v0.4.0","html_url":"https://github.com/nazymko/family-tree/releases/tag/v0.4.0","published_at":"2026-12-01T06:00:00Z","assets":[{"name":"x"}],"body":"notes"}"#;

    #[test]
    fn github_json_reads_three_fields() {
        let r = parse_release(REPLY).unwrap();
        assert_eq!(r.tag_name, "v0.4.0");
        assert_eq!(r.published_at.as_deref(), Some("2026-12-01T06:00:00Z"));
        assert!(parse_release("{}").is_none());
        assert!(parse_release("<html>").is_none());
    }

    fn running(describe: &str) -> AppInfo {
        info_from(facts(Some(describe)))
    }

    #[test]
    fn newer_release_is_offered_with_its_page() {
        let s = status_from(&running("v0.3.0-12-g1a2b3c4"), Ok(REPLY.into()));
        assert_eq!(
            s,
            UpdateStatus::Newer {
                version: "0.4.0".into(),
                url: "https://github.com/nazymko/family-tree/releases/tag/v0.4.0".into(),
                published: Some("2026-12-01T06:00:00Z".into())
            }
        );
    }

    #[test]
    fn same_or_older_release_means_latest() {
        let latest = UpdateStatus::Latest {
            latest: "0.4.0".into(),
        };
        assert_eq!(
            status_from(&running("v0.4.0-0-gabc1234"), Ok(REPLY.into())),
            latest
        );
        // a build after the newest release is ahead of it, and still has the latest
        assert_eq!(
            status_from(&running("v0.4.0-9-gabc1234"), Ok(REPLY.into())),
            latest
        );
    }

    #[test]
    fn failures_are_told_apart() {
        let r = running("v0.3.0-0-gabc1234");
        let failed = |reason| UpdateStatus::Failed { reason };
        assert_eq!(
            status_from(&r, Err(FailReason::Offline)),
            failed(FailReason::Offline)
        );
        assert_eq!(
            status_from(&r, Err(FailReason::Server)),
            failed(FailReason::Server)
        );
        assert_eq!(
            status_from(&r, Ok("not json".into())),
            failed(FailReason::BadReply)
        );
        let odd = r#"{"tag_name":"nightly","html_url":"https://github.com/nazymko/family-tree/releases/tag/nightly"}"#;
        assert_eq!(
            status_from(&r, Ok(odd.into())),
            failed(FailReason::BadReply)
        );
    }

    #[test]
    fn a_foreign_release_page_is_replaced_by_the_project_page() {
        let evil =
            r#"{"tag_name":"v9.0.0","html_url":"https://evil.example/x","published_at":null}"#;
        match status_from(&running("v0.3.0-0-gabc1234"), Ok(evil.into())) {
            UpdateStatus::Newer { url, .. } => {
                assert_eq!(url, format!("{REPO_URL}/releases/latest"))
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn only_project_links_open() {
        assert!(is_project_url("https://github.com/nazymko/family-tree"));
        assert!(is_project_url(
            "https://github.com/nazymko/family-tree/releases"
        ));
        assert!(is_project_url(
            "https://github.com/nazymko/family-tree/issues/new/choose"
        ));
        assert!(!is_project_url(
            "https://github.com/nazymko/family-tree-evil"
        ));
        assert!(!is_project_url("https://github.com/other/family-tree"));
        assert!(!is_project_url("file:///C:/Windows/System32/calc.exe"));
    }

    #[test]
    fn serialises_for_the_window() {
        let s = serde_json::to_value(UpdateStatus::Failed {
            reason: FailReason::BadReply,
        })
        .unwrap();
        assert_eq!(
            s,
            serde_json::json!({"kind": "failed", "reason": "badReply"})
        );
        let n = serde_json::to_value(UpdateStatus::Newer {
            version: "1.0.0".into(),
            url: "u".into(),
            published: None,
        })
        .unwrap();
        assert_eq!(n["kind"], "newer");
        assert!(n["published"].is_null());
    }

    #[test]
    fn this_build_reports_something() {
        assert!(Semver::parse(&current().base).is_some());
    }
}
