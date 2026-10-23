//! Source guards of the shell's side of the session lock (CEL-2-H design §5,
//! §6, §9.1). Each is a scan of the production part of the sources (the text
//! left once the `#[cfg(test)]` items of a file are removed), because the guards of `scripts/`
//! are outside this unit's commit roots. Each cites the threat it enforces.

use std::fs;
use std::path::{Path, PathBuf};

// The scanner helper shared by the lock's source guards (round 6, m-d).
#[path = "../crates/celestina-wayland/tests/support/test_gates.rs"]
mod test_gates;
use test_gates::without_test_blocks;

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// The production code of a file: comments and every
/// `#[cfg(test)]` item removed.
fn production(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    without_test_blocks(&text)
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn lock_sources() -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    rust_files(&manifest().join("src/lock"), &mut files);
    assert!(!files.is_empty(), "src/lock exists");
    files
        .into_iter()
        .map(|path| {
            let text = production(&path);
            (path, text)
        })
        .collect()
}

fn all_sources() -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    rust_files(&manifest().join("src"), &mut files);
    files
        .into_iter()
        .map(|path| {
            let text = production(&path);
            (path, text)
        })
        .collect()
}

fn name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[test]
fn tm_8_a_process_is_spawned_only_by_the_supervisor_and_never_through_path() {
    // TM-8, TM-9: which program may stand for the lock is chosen by the
    // descriptor that was checked, never by `PATH` or a name.
    for (path, text) in lock_sources() {
        let spawns = text.matches("Command::new(").count();
        if name(&path) == "supervisor.rs" {
            assert_eq!(spawns, 1, "the supervisor spawns exactly one program");
            assert!(
                text.contains("Command::new(lock.exec_path())"),
                "and it is the pinned descriptor"
            );
        } else {
            assert_eq!(spawns, 0, "{} spawns a process", path.display());
        }
        assert!(!text.contains("std::process::Command") || name(&path) == "supervisor.rs");
        assert!(
            !text.contains("\"PATH\""),
            "{} looks at PATH",
            path.display()
        );
        assert!(!text.contains("env::var(\"PATH"), "{}", path.display());
    }
}

#[test]
fn tm_1_no_unwrap_expect_panic_or_unsafe_in_the_lock_provider() {
    // The root contract (production code of the privileged path).
    for (path, text) in lock_sources() {
        for banned in [
            ".unwrap()",
            ".expect(",
            "panic!(",
            "unreachable!(",
            "todo!(",
            "unimplemented!(",
            "unsafe ",
            "#[allow(",
        ] {
            assert!(
                !text.contains(banned),
                "{} contains `{banned}`",
                path.display()
            );
        }
        assert!(
            text.contains("#![forbid(unsafe_code)]"),
            "{} does not forbid unsafe",
            path.display()
        );
    }
}

#[test]
fn tm_25_no_unlock_verb_method_or_signal_handler_exists_in_the_shell() {
    // TM-25, §6 items 4 and 5: nothing in the shell can end a lock.
    let mut checked = 0;
    for (path, text) in all_sources() {
        checked += 1;
        for (index, line) in text.lines().enumerate() {
            let word = |needle: &str| {
                line.match_indices(needle).any(|(at, _)| {
                    let before = line[..at].chars().last();
                    let after = line[at + needle.len()..].chars().next();
                    !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
                        && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
                })
            };
            assert!(
                !word("Unlock") && !word("unlock") && !word("unlock_session"),
                "{}:{}: `{line}`",
                path.display(),
                index + 1
            );
            assert!(
                !line.contains("fn unlock"),
                "{}:{}: `{line}`",
                path.display(),
                index + 1
            );
            // The strings a bus verb or a logind member would be spelled with.
            for banned in [
                "\"unlock\"",
                "\"Unlock\"",
                "\"UnlockSession\"",
                "\"lock-session\"",
            ] {
                assert!(
                    !line.contains(banned),
                    "{}:{}: `{line}`",
                    path.display(),
                    index + 1
                );
            }
        }
    }
    assert!(checked > 20, "the scan saw the shell's sources");
}

#[test]
fn t_g1_no_subscription_to_logind_lock_or_unlock_signals() {
    // TM-25, §6 item 5: logind's `Lock` and `Unlock` signals are not served
    // (any process of the session may emit `Unlock` without a password).
    let mut files = Vec::new();
    rust_files(&manifest().join("src/lock"), &mut files);
    rust_files(&manifest().join("src/energy"), &mut files);
    assert!(files.len() > 5);
    for path in files {
        let text = production(&path);
        for banned in ["\"Lock\"", "\"Unlock\"", "LockedHint", "loginctl"] {
            assert!(
                !text.contains(banned),
                "{} mentions {banned}",
                path.display()
            );
        }
        // The only logind signal the lock listens to is `PrepareForSleep`.
        if name(&path) == "sleep.rs" {
            assert!(text.contains(".member(PREPARE_FOR_SLEEP)"));
            // Rules named by `.member(NAME)`; reading a header's `.member()`
            // is not a subscription.
            assert_eq!(
                text.matches(".member(").count() - text.matches(".member()").count(),
                1
            );
        }
    }
}

#[test]
fn tm_1_the_proof_cannot_be_forged_outside_the_policy_module() {
    // The proof is minted only by the policy: a private field, no derive that
    // would copy it, no constructor.
    let policy = production(&manifest().join("../celestina-rs/crates/celestina-shell/src/lock.rs"));
    let at = policy
        .find("pub struct LockedProof")
        .expect("the proof type");
    let head = &policy[at.saturating_sub(120)..at];
    assert!(
        !head.contains("Clone") && !head.contains("Copy") && !head.contains("Default"),
        "the proof is not copyable: {head}"
    );
    let body = &policy[at..];
    let end = body.find("}\n").expect("the end of the struct");
    assert!(
        !body[..end].contains("pub generation"),
        "the generation is private"
    );
    assert!(!policy.contains("impl Clone for LockedProof"));
    assert!(!policy.contains("impl Default for LockedProof"));
    // The type, its impl block and the one literal in `mint`.
    assert_eq!(policy.matches("LockedProof {").count(), 3);
    assert_eq!(policy.matches("Some(LockedProof {").count(), 1);
    // And nothing in the shell builds one by struct literal.
    for (path, text) in all_sources() {
        assert!(
            !text.contains("LockedProof {"),
            "{} builds a proof",
            path.display()
        );
    }
}

#[test]
fn tm_11_no_secret_or_password_type_travels_through_the_command_surface() {
    // TM-11, TM-14: the shell never holds the password.
    for file in ["message.rs", "bus.rs", "command.rs"] {
        let text = production(&manifest().join("src").join(file)).to_lowercase();
        for banned in ["password", "secret", "passwd"] {
            assert!(!text.contains(banned), "{file} mentions {banned}");
        }
    }
    for (path, text) in lock_sources() {
        let lower = text.to_lowercase();
        for banned in ["password", "passwd"] {
            assert!(
                !lower.contains(banned),
                "{} mentions {banned}",
                path.display()
            );
        }
    }
}

#[test]
fn tm_8_the_lock_environment_is_built_from_a_fixed_allowlist() {
    // TM-8, T-E1: the names the lock is given are the three of the design,
    // spelled once, in the supervisor.
    let supervisor = lock_sources()
        .into_iter()
        .find(|(path, _)| name(path) == "supervisor.rs")
        .map(|(_, text)| text)
        .expect("the supervisor");
    assert!(supervisor.contains(".env_clear()"));
    for name in [
        "WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
        "MESA_SHADER_CACHE_DISABLE",
    ] {
        assert_eq!(
            supervisor.matches(&format!("\"{name}\"")).count(),
            1,
            "{name}"
        );
    }
    // Round 5 (ADR 0022 A4): the fourth entry is the launcher marker the
    // toolkit's connection guard looks for, named through the toolkit's own
    // constants and set to the constant value, never read from the caller.
    let production = without_test_blocks(&supervisor);
    assert!(production.contains("OsString::from(LAUNCHER_VARIABLE)"));
    assert!(production.contains("OsString::from(LAUNCHER_SHELL)"));
    assert!(!production.contains("read(LAUNCHER_VARIABLE)"));
    assert!(supervisor.contains(".process_group(0)"));
    // No other module of the provider reads the environment.
    for (path, text) in lock_sources() {
        if name(&path) != "supervisor.rs" {
            assert!(
                !text.contains("env::var"),
                "{} reads the environment",
                path.display()
            );
        }
    }
}
