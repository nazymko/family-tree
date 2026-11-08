/// Runs git in the crate folder; None when git is missing, this is not a checkout (a source tarball) or the command fails.
fn git(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(std::env::var("CARGO_MANIFEST_DIR").ok()?)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !text.is_empty()).then_some(text)
}

/// Hands the build's git facts to `about.rs` as GT_GIT_DESCRIBE (`v0.3.0-12-g1a2b3c4`), GT_GIT_COMMIT and GT_GIT_DATE
/// (`2026-11-08`). Each is simply left out when git cannot tell; the app then reports the version in its manifest.
fn build_info() {
    // Re-run after a commit, a checkout or a new tag. In a worktree these live in the shared git dir; `--git-path` knows.
    let mut watch = vec![
        "HEAD".to_string(),
        "packed-refs".to_string(),
        "refs/tags".to_string(),
    ];
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
        watch.push(branch);
    }
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    for w in watch {
        if let Some(path) = git(&["rev-parse", "--git-path", &w]) {
            let full = std::path::Path::new(&manifest).join(path);
            if full.exists() {
                println!("cargo:rerun-if-changed={}", full.display());
            }
        }
    }
    let facts = [
        (
            "GT_GIT_DESCRIBE",
            git(&["describe", "--tags", "--long", "--match", "v[0-9]*"]),
        ),
        ("GT_GIT_COMMIT", git(&["rev-parse", "--short=7", "HEAD"])),
        ("GT_GIT_DATE", git(&["log", "-1", "--format=%cs"])),
    ];
    for (name, value) in facts {
        if let Some(v) = value {
            println!("cargo:rustc-env={name}={v}");
        }
    }
}

fn main() {
    // The icon is compiled into the exe, but changing it does not re-run this script by itself: the app would keep
    // showing the old one in the taskbar.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build();
    build_info();
    // The integration tests link the real WebView2 loader and need the Common Controls v6
    // manifest, or they fail to start (STATUS_ENTRYPOINT_NOT_FOUND). Tests only: the app's own
    // binary gets its manifest from tauri-build.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        println!("cargo:rerun-if-changed=test.manifest");
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-tests=/MANIFESTINPUT:{dir}/test.manifest");
    }
}
