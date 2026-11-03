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
    run_ok(gt().arg("export").arg(dir).arg("-o").arg(&out).args(extra));
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
