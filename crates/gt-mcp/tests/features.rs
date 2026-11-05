#![allow(clippy::unwrap_used)]
//! `gt-session`'s `test-support` feature opens `commit_command_as`, a way around the gate. This
//! crate's build must never turn it on. Runs in `cargo test --workspace`, so CI has it.

use std::process::Command;

#[test]
fn test_support_stays_off() {
    let output = Command::new(env!("CARGO"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["tree", "-p", "gt-mcp", "-e", "features", "--offline"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "cargo tree failed: {stderr}");
    let tree = String::from_utf8_lossy(&output.stdout);
    assert!(
        tree.contains("gt-session"),
        "not the tree expected:\n{tree}"
    );
    assert!(
        !tree.contains("test-support"),
        "something turns on gt-session's test-support:\n{tree}"
    );
}
