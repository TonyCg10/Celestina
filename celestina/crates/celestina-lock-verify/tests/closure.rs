//! The verifier's dependency closure against the checked-in allowlist
//! (design §4.14, the verifier half of T-D1).

use std::collections::BTreeSet;
use std::process::Command;

fn allowed() -> BTreeSet<String> {
    include_str!("closure.allow")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_transitive_closure_equals_the_allowlist() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let output = Command::new(cargo)
        .args([
            "tree",
            "--offline",
            "--locked",
            "-e",
            "normal",
            "--prefix",
            "none",
            "-p",
            "celestina-lock-verify",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run cargo tree");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("utf-8");
    let found: BTreeSet<String> = text
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect();
    assert_eq!(found, allowed());
}
