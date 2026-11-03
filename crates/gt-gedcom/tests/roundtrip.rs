#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use gt_core::DayOrder;
use gt_core::date::GedcomVersion;
use gt_core::tree::Tree;
use gt_gedcom::{ExportOptions, ImportOptions, export, import};
use std::path::{Path, PathBuf};

/// Files that cannot round-trip, with the reason. Never a blanket skip: each entry is a file name.
const KNOWN_UNREPRESENTABLE: &[(&str, &str)] = &[];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests")
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.path());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != ".git") {
                collect(&p, out);
            }
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ged")) {
            out.push(p);
        }
    }
}

fn files() -> Vec<PathBuf> {
    let mut v = Vec::new();
    collect(&root().join("fixtures/gedcom"), &mut v);
    collect(&root().join("corpus"), &mut v);
    v
}

fn opts() -> ImportOptions {
    ImportOptions {
        day_order: DayOrder::Dmy,
        media_roots: vec![],
        ..Default::default()
    }
}

fn diff(a: &Tree, b: &Tree) -> String {
    macro_rules! cmp {
        ($f:ident) => {
            if a.$f != b.$f {
                let (la, lb) = (a.$f.len(), b.$f.len());
                if la != lb {
                    return format!("{}: {la} vs {lb} entries", stringify!($f));
                }
                for (k, va) in &a.$f {
                    if b.$f.get(k) != Some(va) {
                        let (sa, sb) = (
                            format!("{va:?}"),
                            b.$f.get(k).map_or("None".to_string(), |x| format!("{x:?}")),
                        );
                        let at = sa
                            .bytes()
                            .zip(sb.bytes())
                            .position(|(x, y)| x != y)
                            .unwrap_or(0);
                        let lo = sa.floor_char_boundary(at.saturating_sub(120));
                        let ha = sa.ceil_char_boundary((at + 160).min(sa.len()));
                        let hb = sb.ceil_char_boundary((at + 160).min(sb.len()));
                        return format!(
                            "{} {k} differs at byte {at}:
  a = ...{}
  b = ...{}",
                            stringify!($f),
                            &sa[lo..ha],
                            &sb[sb.floor_char_boundary(lo)..hb]
                        );
                    }
                }
            }
        };
    }
    if a.header != b.header {
        return format!("header:\n  a = {:?}\n  b = {:?}", a.header, b.header);
    }
    cmp!(persons);
    cmp!(families);
    cmp!(events);
    cmp!(places);
    cmp!(sources);
    cmp!(repositories);
    cmp!(citations);
    cmp!(media);
    cmp!(notes);
    "no difference found".into()
}

#[test]
fn corpus_round_trips_in_both_versions() {
    let all = files();
    assert!(!all.is_empty(), "no fixtures found");
    if std::env::var("GT_REQUIRE_CORPUS").is_ok_and(|v| v == "1") {
        let mut corpus = Vec::new();
        collect(&root().join("corpus"), &mut corpus);
        assert!(
            corpus.len() >= 50,
            "GT_REQUIRE_CORPUS=1 but tests/corpus has {} .ged files (need >= 50): run scripts/fetch-corpus.sh",
            corpus.len()
        );
    }
    let mut failures = Vec::new();
    let mut checked = 0;
    for f in &all {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        if KNOWN_UNREPRESENTABLE.iter().any(|(n, _)| *n == name) {
            continue;
        }
        let bytes = std::fs::read(f).unwrap();
        let a = import(&bytes, &opts());
        for v in [GedcomVersion::V551, GedcomVersion::V70] {
            let out = export(&a.tree, &ExportOptions { version: v });
            let b = import(&out, &opts());
            checked += 1;
            if a.tree.normalized_for_compare() != b.tree.normalized_for_compare() {
                failures.push(format!(
                    "{} [{v:?}]: {}",
                    f.display(),
                    diff(
                        &a.tree.normalized_for_compare(),
                        &b.tree.normalized_for_compare()
                    )
                ));
            }
            // Exporting what we just imported must be stable.
            let again = export(&b.tree, &ExportOptions { version: v });
            if again.len() != out.len() {
                failures.push(format!(
                    "{} [{v:?}]: second export differs in size",
                    f.display()
                ));
            }
        }
    }
    eprintln!("round-tripped {} files, {checked} runs", all.len());
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn person_counts_match_ged_io() {
    let mut compared = 0;
    let mut mismatches = Vec::new();
    for f in files() {
        let Ok(bytes) = std::fs::read(&f) else {
            continue;
        };
        let Ok(text) = String::from_utf8(bytes.clone()) else {
            continue;
        };
        let Ok(parsed) =
            ged_io::GedcomBuilder::new().build_from_str(text.trim_start_matches('\u{feff}'))
        else {
            continue;
        };
        let ours = import(&bytes, &opts()).tree.persons.len();
        compared += 1;
        if ours != parsed.individuals.len() {
            mismatches.push(format!(
                "{}: ours {ours}, ged_io {}",
                f.display(),
                parsed.individuals.len()
            ));
        }
    }
    eprintln!("compared {compared} files with ged_io");
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
