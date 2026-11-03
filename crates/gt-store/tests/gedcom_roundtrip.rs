#![allow(clippy::unwrap_used)]
//! Real importer output must survive save_tree / load_tree unchanged.

use gt_gedcom::{ImportOptions, import};
use gt_store::Project;

const TODAY: i32 = 2_460_000;

#[test]
fn gedcom_fixture_roundtrip_through_store() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/gedcom");
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("ged") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let tree = import(&bytes, &ImportOptions::default()).tree;
        let tmp = tempfile::tempdir().unwrap();
        let mut p = Project::create(&tmp.path().join("t.genetree")).unwrap();
        p.save_tree(&tree, TODAY).unwrap();
        let back = p.load_tree().unwrap();
        assert_eq!(
            back,
            tree,
            "store round trip differs for {}",
            path.display()
        );
        seen += 1;
    }
    assert!(seen >= 4, "expected the fixtures, found {seen}");
}
