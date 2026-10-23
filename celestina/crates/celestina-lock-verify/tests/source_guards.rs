//! Structural guards over the verifier's sources (design §6 "never" items 8, 9,
//! 19, 20). They scan the production part of each file (everything but its
//! `#[cfg(test)]` items) and skip comment lines, so a rule can be quoted in a
//! comment without tripping it.

use std::fs;
use std::path::PathBuf;

// The scanner helper shared by the lock's source guards (round 6, m-d).
#[path = "../../celestina-wayland/tests/support/test_gates.rs"]
mod test_gates;
use test_gates::without_test_blocks;

fn src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// (file name, production code lines without comments).
fn production() -> Vec<(String, Vec<String>)> {
    let mut files = Vec::new();
    for entry in fs::read_dir(src()).expect("read src") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read");
        let code = without_test_blocks(&text);
        let lines = code
            .lines()
            .map(str::trim)
            .filter(|line| !line.starts_with("//") && !line.is_empty())
            .map(str::to_owned)
            .collect();
        let name = path
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        files.push((name, lines));
    }
    files.sort();
    files
}

#[test]
fn unsafe_exists_only_in_pam_rs_and_every_other_module_forbids_it() {
    let files = production();
    assert!(files.len() >= 10, "{files:?}");
    for (name, lines) in &files {
        let mentions: Vec<&String> = lines.iter().filter(|l| l.contains("unsafe")).collect();
        if name == "pam.rs" {
            assert!(
                lines.iter().any(|l| l == "#![allow(unsafe_code)]"),
                "pam.rs states its exception"
            );
            assert!(
                lines
                    .iter()
                    .any(|l| l == "#![deny(unsafe_op_in_unsafe_fn)]"),
                "pam.rs denies unsafe_op_in_unsafe_fn"
            );
            continue;
        }
        if name == "main.rs" {
            assert!(mentions.is_empty(), "main.rs: {mentions:?}");
            continue;
        }
        assert_eq!(
            mentions,
            ["#![forbid(unsafe_code)]"],
            "{name} must forbid unsafe and mention it nowhere else"
        );
    }
}

#[test]
fn no_new_privs_is_never_set_anywhere() {
    // The needles are assembled so this file does not contain them.
    let upper = ["NO_NEW", "_PRIVS"].concat();
    let lower = ["no_new", "_privs"].concat();
    for (name, lines) in production() {
        for line in lines {
            assert!(
                !line.contains(&upper) && !line.contains(&lower),
                "{name}: {line}"
            );
        }
    }
}

#[test]
fn the_verifier_writes_nothing_and_calls_no_session_or_token_function() {
    let forbidden = [
        "println!",
        "eprintln!",
        "print!(",
        "eprint!(",
        "dbg!(",
        "io::stdout",
        "io::stderr",
        "tracing",
        "env::var",
        "pam_setcred",
        "pam_open_session",
        "pam_close_session",
        "pam_chauthtok",
        "pam_putenv",
        "pam_set_item",
        "pam_get_item",
        "pam_fail_delay",
    ];
    for (name, lines) in production() {
        for line in lines {
            for needle in forbidden {
                assert!(!line.contains(needle), "{name}: {needle} in {line}");
            }
        }
    }
}

#[test]
fn only_the_four_pam_functions_are_declared() {
    let files = production();
    let pam = files
        .iter()
        .find(|(name, _)| name == "pam.rs")
        .expect("pam.rs");
    let declared: Vec<&str> = pam
        .1
        .iter()
        .filter_map(|line| line.strip_prefix("fn pam_"))
        .map(|rest| rest.split('(').next().unwrap_or_default())
        .collect();
    assert_eq!(declared, ["start", "authenticate", "acct_mgmt", "end"]);
}

#[test]
fn the_secret_is_never_formatted() {
    // No `format!`, `{:?}` of a secret, or Display of its bytes in the
    // production code: the type holding it derives `Debug` only through the
    // conversation state, which this guard keeps out of format strings.
    for (name, lines) in production() {
        for line in lines {
            assert!(!line.contains("format!"), "{name}: {line}");
            assert!(!line.contains("{secret"), "{name}: {line}");
        }
    }
}

/// Round 5, m-1: the descriptors the verifier was not handed are closed by one
/// call, the first statement of `main`; `nix` closes them in one place.
#[test]
fn m1_the_inherited_descriptors_are_closed_first_by_exactly_one_call() {
    let files = production();
    let calls: Vec<&str> = files
        .iter()
        .flat_map(|(name, lines)| {
            lines
                .iter()
                .filter(|l| l.contains("hardening::close_inherited_descriptors("))
                .map(move |_| name.as_str())
        })
        .collect();
    assert_eq!(calls, ["main.rs"], "one call");
    let main = &files
        .iter()
        .find(|(name, _)| name == "main.rs")
        .expect("main.rs")
        .1;
    let at = main
        .iter()
        .position(|l| l.starts_with("fn main()"))
        .expect("fn main");
    assert!(
        main[at + 1].contains("hardening::close_inherited_descriptors("),
        "the first statement of main: {}",
        main[at + 1]
    );
    let closes: Vec<&str> = files
        .iter()
        .flat_map(|(name, lines)| {
            lines
                .iter()
                .filter(|l| l.contains("nix::unistd::close("))
                .map(move |_| name.as_str())
        })
        .collect();
    assert_eq!(closes, ["hardening.rs"], "one close, in hardening");
    // One listing of the process's descriptors.
    let listings: Vec<&str> = files
        .iter()
        .flat_map(|(name, lines)| {
            lines
                .iter()
                .filter(|l| l.contains("read_dir("))
                .map(move |_| name.as_str())
        })
        .collect();
    assert_eq!(listings, ["hardening.rs"]);
}
