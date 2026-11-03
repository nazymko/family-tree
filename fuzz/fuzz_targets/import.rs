#![no_main]
use gt_gedcom::{ExportOptions, GedcomVersion, ImportOptions, export, import};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let out = import(data, &ImportOptions::default());
    let _ = out.report.to_text();
    for version in [GedcomVersion::V551, GedcomVersion::V70] {
        let _ = export(&out.tree, &ExportOptions { version });
    }
});
