#![allow(clippy::unwrap_used)]

use assert_cmd::Command;
use std::path::{Path, PathBuf};

fn gt() -> Command {
    Command::cargo_bin("gt").unwrap()
}

fn repo_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/gedcom")
        .join(name)
}

fn local_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn run_ok(cmd: &mut Command) -> String {
    let out = cmd.assert().success().get_output().clone();
    String::from_utf8(out.stdout).unwrap()
}

fn import_into(ged: &Path, dir: &Path) {
    run_ok(gt().arg("import").arg(ged).arg("--into").arg(dir));
}

#[test]
fn check_prints_report() {
    let out = run_ok(gt().arg("check").arg(repo_fixture("basic551.ged")));
    assert!(out.contains("ANCESTRY"), "{out}");
    assert!(out.contains("_MTTAG"), "{out}");
}

#[test]
fn import_then_search() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    import_into(&repo_fixture("basic551.ged"), &dir);
    let out = run_ok(gt().arg("search").arg(&dir).arg("muller"));
    let line = out.lines().next().unwrap_or_default();
    assert!(line.starts_with("person"), "{out}");
    assert!(line.contains("Mueller"), "{out}");
}

fn export_text(dir: &Path, extra: &[&str]) -> String {
    let out = dir.parent().unwrap().join("out.ged");
    run_ok(
        gt().arg("export")
            .arg(dir)
            .arg("-o")
            .arg(&out)
            .arg("--force")
            .args(extra),
    );
    String::from_utf8(std::fs::read(out).unwrap()).unwrap()
}

#[test]
fn export_hides_living_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    import_into(&local_fixture("living.ged"), &dir);
    let text = export_text(&dir, &[]);
    assert!(!text.contains("Livingstone"), "{text}");
    assert!(text.contains("Living"), "{text}");
    assert!(text.contains("Mueller"), "{text}");
}

#[test]
fn export_include_all() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    import_into(&local_fixture("living.ged"), &dir);
    let text = export_text(&dir, &["--living", "all"]);
    assert!(text.contains("Livingstone"), "{text}");
}

#[test]
fn export_prints_privacy_summary() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    import_into(&local_fixture("living.ged"), &dir);
    let out = tmp.path().join("o.ged");
    let stdout = run_ok(gt().arg("export").arg(&dir).arg("-o").arg(&out));
    assert!(stdout.contains("placeholder"), "{stdout}");
}

#[test]
fn export_version_follows_source() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    import_into(&repo_fixture("family7.ged"), &dir);
    let text = export_text(&dir, &[]);
    assert!(text.contains("2 VERS 7.0"), "{text}");
    let text = export_text(&dir, &["--gedcom", "5.5.1"]);
    assert!(text.contains("2 VERS 5.5.1"), "{text}");
}

#[test]
fn import_into_existing_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    import_into(&repo_fixture("basic551.ged"), &dir);
    let out = gt()
        .arg("import")
        .arg(repo_fixture("basic551.ged"))
        .arg("--into")
        .arg(&dir)
        .assert()
        .code(1)
        .get_output()
        .clone();
    let err = String::from_utf8(out.stderr).unwrap();
    assert_eq!(err.trim_end().lines().count(), 1, "{err}");
}

#[test]
fn missing_input_is_a_user_error() {
    let tmp = tempfile::tempdir().unwrap();
    gt().arg("check")
        .arg(tmp.path().join("nope.ged"))
        .assert()
        .code(1);
    gt().arg("bogus").assert().code(1);
}

#[test]
fn media_copied() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    let root = local_fixture("");
    run_ok(
        gt().arg("import")
            .arg(local_fixture("media.ged"))
            .arg("--into")
            .arg(&dir)
            .arg("--media-root")
            .arg(&root),
    );
    let files: Vec<_> = walk(&dir.join("media"));
    assert_eq!(files.len(), 1, "{files:?}");
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

#[test]
fn backup_prints_path() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    import_into(&repo_fixture("basic551.ged"), &dir);
    let out = run_ok(gt().arg("backup").arg(&dir));
    assert!(Path::new(out.trim()).exists(), "{out}");
}

#[test]
fn import_writes_report_file() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    let rep = tmp.path().join("r.txt");
    run_ok(
        gt().arg("import")
            .arg(repo_fixture("basic551.ged"))
            .arg("--into")
            .arg(&dir)
            .arg("--report")
            .arg(&rep),
    );
    assert!(std::fs::read_to_string(rep).unwrap().contains("ANCESTRY"));
}

fn stderr_of(cmd: &mut Command, code: i32) -> String {
    let out = cmd.assert().code(code).get_output().clone();
    String::from_utf8(out.stderr).unwrap()
}

#[test]
fn export_refuses_to_overwrite_without_force() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    import_into(&repo_fixture("basic551.ged"), &dir);
    let out = tmp.path().join("o.ged");
    std::fs::write(&out, "precious").unwrap();
    let err = stderr_of(gt().arg("export").arg(&dir).arg("-o").arg(&out), 1);
    assert_eq!(err.trim_end().lines().count(), 1, "{err}");
    assert!(err.contains("pass --force to overwrite"), "{err}");
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "precious");
    run_ok(
        gt().arg("export")
            .arg(&dir)
            .arg("-o")
            .arg(&out)
            .arg("--force"),
    );
    assert!(std::fs::read_to_string(&out).unwrap().contains("HEAD"));
}

#[test]
fn import_report_refuses_to_overwrite_without_force() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    let rep = tmp.path().join("r.txt");
    std::fs::write(&rep, "precious").unwrap();
    let err = stderr_of(
        gt().arg("import")
            .arg(repo_fixture("basic551.ged"))
            .arg("--into")
            .arg(&dir)
            .arg("--report")
            .arg(&rep),
        1,
    );
    assert!(err.contains("pass --force to overwrite"), "{err}");
    assert!(!dir.exists());
    assert_eq!(std::fs::read_to_string(&rep).unwrap(), "precious");
}

#[test]
fn failed_report_write_leaves_no_project() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    let rep = tmp.path().join("no-such-dir").join("r.txt");
    gt().arg("import")
        .arg(repo_fixture("basic551.ged"))
        .arg("--into")
        .arg(&dir)
        .arg("--report")
        .arg(&rep)
        .assert()
        .code(1);
    assert!(!dir.exists());
    let leftovers: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn failed_media_copy_leaves_no_target_and_retry_succeeds() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir(&src).unwrap();
    std::fs::copy(local_fixture("media.ged"), src.join("media.ged")).unwrap();
    let photo = src.join("photo.jpg");
    std::fs::copy(local_fixture("photo.jpg"), &photo).unwrap();
    let dir = tmp.path().join("p");

    // Make the photo unreadable: an exclusive handle on Windows, mode 000 elsewhere.
    #[cfg(windows)]
    let guard = {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&photo)
            .unwrap()
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&photo, std::fs::Permissions::from_mode(0o000)).unwrap();
    }
    let status = gt()
        .arg("import")
        .arg(src.join("media.ged"))
        .arg("--into")
        .arg(&dir)
        .output()
        .unwrap();
    #[cfg(windows)]
    drop(guard);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&photo, std::fs::Permissions::from_mode(0o644)).unwrap();
        if status.status.success() {
            return; // running as root: permissions do not block the copy
        }
    }
    assert_eq!(status.status.code(), Some(1), "{status:?}");
    assert!(!dir.exists(), "target must not exist after a failed import");
    let names: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1, "partial dir left behind: {names:?}");

    run_ok(
        gt().arg("import")
            .arg(src.join("media.ged"))
            .arg("--into")
            .arg(&dir),
    );
    assert_eq!(walk(&dir.join("media")).len(), 1);
}

#[test]
fn round_trip_on_path_with_spaces_and_non_ascii() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().join("Familie Müller 王");
    std::fs::create_dir(&base).unwrap();
    let dir = base.join("mein Projekt");
    import_into(&repo_fixture("basic551.ged"), &dir);
    let out = base.join("Ausgabe Müller.ged");
    run_ok(gt().arg("export").arg(&dir).arg("-o").arg(&out));
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("Mueller"), "{text}");
    let hit = run_ok(gt().arg("search").arg(&dir).arg("muller"));
    assert!(hit.contains("Mueller"), "{hit}");
}

fn current_year() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    1970 + (secs / 31_556_952) as i64
}

#[test]
fn export_uses_the_projects_living_threshold() {
    let tmp = tempfile::tempdir().unwrap();
    let ged = tmp.path().join("old.ged");
    let born = current_year() - 105;
    std::fs::write(
        &ged,
        format!(
            "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n0 @I1@ INDI\n1 NAME Zelda /Centenarian/\n1 BIRT\n2 DATE {born}\n0 TRLR\n"
        ),
    )
    .unwrap();
    let dir = tmp.path().join("p");
    import_into(&ged, &dir);
    // Default threshold of 100: a 105-year-old with no death is presumed dead and exported.
    assert!(export_text(&dir, &[]).contains("Centenarian"));
    {
        let mut p = gt_store::Project::open(&dir).unwrap();
        p.meta_set("living_max_age", "110").unwrap();
    }
    let text = export_text(&dir, &[]);
    assert!(!text.contains("Centenarian"), "{text}");
}

#[test]
fn export_of_an_open_project_is_a_clean_conflict_error() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    import_into(&local_fixture("living.ged"), &dir);
    let _held = gt_store::Project::open(&dir).unwrap();
    let out = tmp.path().join("o.ged");
    let err = stderr_of(gt().arg("export").arg(&dir).arg("-o").arg(&out), 1);
    assert!(
        err.contains("This tree is open in another Gene Tree window."),
        "{err}"
    );
}
