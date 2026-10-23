//! The lock binaries' dependency closure against the checked-in allowlist
//! (design §4.14, T-D1), and the probe's absence without its feature.
//!
//! The closure is the one of the production build command: the two lock
//! packages in an invocation of their own, so feature unification with the
//! shell cannot switch `gl` on for them.

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

fn closure(packages: &[&str]) -> BTreeSet<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut command = Command::new(cargo);
    command.args([
        "tree",
        "--offline",
        "--locked",
        "-e",
        "normal",
        "--prefix",
        "none",
    ]);
    for package in packages {
        command.args(["-p", package]);
    }
    let output = command
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run cargo tree");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn t_d1_transitive_closure_equals_the_allowlist() {
    let found = closure(&["celestina-lock", "celestina-lock-verify"]);
    let allowed = allowed();
    let unexpected: Vec<&String> = found.difference(&allowed).collect();
    let missing: Vec<&String> = allowed.difference(&found).collect();
    assert!(
        unexpected.is_empty() && missing.is_empty(),
        "new in the closure: {unexpected:?}; listed but gone: {missing:?}"
    );
}

#[test]
fn the_lock_tree_holds_no_shell_gpu_decoder_bus_or_icon_crate() {
    // §4.14 "not": no bus, no decoder, no compositor IPC, no GPU, no system
    // fonts or icon themes in the process that can end the lock.
    let forbidden = [
        "celestina-shell",
        "celestina-paint",
        "celestina-ui",
        "zbus",
        "image",
        "serde_json",
        "niri-ipc",
        "khronos-egl",
        "femtovg",
        "glow",
        "wayland-egl",
        "freedesktop-icons",
        "resvg",
        "fontconfig",
        "yeslogic-fontconfig-sys",
        "regex",
    ];
    let found = closure(&["celestina-lock"]);
    for name in forbidden {
        assert!(!found.contains(name), "{name} is in the lock's closure");
    }
    let allowed = allowed();
    for name in forbidden {
        assert!(!allowed.contains(name), "{name} is in the allowlist");
    }
}

#[test]
fn the_verifiers_own_allowlist_is_inside_the_locks() {
    let verify_only: BTreeSet<String> =
        include_str!("../../celestina-lock-verify/tests/closure.allow")
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_owned)
            .collect();
    let allowed = allowed();
    for name in &verify_only {
        assert!(allowed.contains(name), "{name}");
    }
}

#[test]
fn unlock_probe_is_absent_without_fake_verifier() {
    // The manifest builds the probe only with the feature, and the feature is
    // not a default one.
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest.contains("name = \"unlock_probe\"")
            && manifest.contains("required-features = [\"fake-verifier\"]"),
        "the probe must require the fake-verifier feature"
    );
    assert!(manifest.contains("default = []"));
    // The shipped binary of a normal build holds no trace of the rehearsal
    // paths: the probe's name and the message that types a password.
    let shipped = std::fs::read(env!("CARGO_BIN_EXE_celestina-lock")).expect("the shipped binary");
    for needle in ["unlock_probe", "AutoType"] {
        let present = shipped
            .windows(needle.len())
            .any(|window| window == needle.as_bytes());
        if cfg!(feature = "fake-verifier") {
            continue;
        }
        assert!(!present, "`{needle}` appears in the shipped binary");
    }
}
