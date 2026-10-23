//! Guards of the session lock's scripts and PAM text (CEL-2-H design §4.11,
//! §4.14, §9.2, plan Task 7). `scripts/` at the repository root is outside
//! this unit's commit roots, so these scan the project's own `scripts/` and
//! `contrib/` from a test. Each cites the rule it enforces.

use std::fs;
use std::path::PathBuf;

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    let path = manifest().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

/// The lines of a script that are not comments.
fn code(relative: &str) -> Vec<String> {
    read(relative)
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .map(str::to_owned)
        .collect()
}

const SCRIPTS: [&str; 8] = [
    "scripts/build-production.sh",
    "scripts/verify-production.sh",
    "scripts/deploy-production.sh",
    "scripts/status-production.sh",
    "scripts/complete-production.sh",
    "scripts/smoke.sh",
    "scripts/dev-session.sh",
    "scripts/lock-rehearsal.sh",
];

/// The spec's text of the PAM service, §4.11, byte for byte.
const PAM_TEXT: &str = "#%PAM-1.0\n\
# Celestina session lock: the password, through the system stack.\n\
auth      include   system-auth\n\
account   include   system-auth\n";

#[test]
fn the_pam_text_is_exactly_the_one_of_the_design() {
    assert_eq!(read("contrib/pam.d/celestina-lock"), PAM_TEXT);
    // `pam_nologin` stays out: `/run/nologin` would refuse every unlock.
    assert!(!PAM_TEXT.contains("nologin"));
}

#[test]
fn no_script_kills_a_process_by_name() {
    // The author's session was once ended by `pkill -x niri`. Every process
    // here is stopped by a pid its own launcher recorded.
    for script in SCRIPTS {
        for line in code(script) {
            for word in ["pkill", "killall", "pgrep", "killproc"] {
                assert!(
                    !line.contains(word),
                    "{script} kills or finds a process by name: {line}"
                );
            }
            assert!(
                !(line.contains("xargs") && line.contains("kill")),
                "{script} pipes into kill: {line}"
            );
        }
    }
    // Where the rehearsal does call `kill`, the target is a variable holding
    // a pid, never a literal, a glob or a command substitution.
    for line in code("scripts/lock-rehearsal.sh") {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed
            .strip_prefix("kill ")
            .or_else(|| trimmed.split("|| kill ").nth(1))
            .or_else(|| trimmed.split("] || kill ").nth(1))
        else {
            continue;
        };
        let target = rest.split_whitespace().nth(1).unwrap_or_default();
        assert!(
            target.starts_with("\"$"),
            "kill with a target that is not a recorded pid: {line}"
        );
    }
}

#[test]
fn the_rehearsal_reads_etc_only_to_refuse_when_its_pam_service_exists() {
    for line in code("scripts/lock-rehearsal.sh") {
        if line.contains("/etc") {
            assert!(
                line.contains("/etc/pam.d/celestina-lock-rehearsal"),
                "the rehearsal names an /etc path other than its own service: {line}"
            );
        }
    }
    // Only tests for existence: no read, no write, no install.
    let rehearsal = read("scripts/lock-rehearsal.sh");
    assert!(rehearsal.contains("[ -e /etc/pam.d/celestina-lock-rehearsal ]"));
}

#[test]
fn no_script_installs_the_pam_service() {
    // The author installs it, with the one command of the README (TM-14,
    // design §4.11, plan P3). No production entry or helper touches `/etc`
    // or names the PAM text; the rehearsal's one existence test is guarded
    // by the previous test.
    for script in SCRIPTS {
        if script == "scripts/lock-rehearsal.sh" {
            continue;
        }
        for line in code(script) {
            assert!(
                !line.contains("pam.d"),
                "{script} names a pam.d path: {line}"
            );
            assert!(!line.contains("/etc/"), "{script} touches /etc: {line}");
        }
    }
}

#[test]
fn the_lock_pair_is_built_in_an_invocation_of_its_own() {
    // `cargo build --release -p celestina-lock -p celestina-lock-verify` apart
    // from the shell, so feature unification cannot switch `gl` on for the
    // process that can end the lock (design §4.14).
    let build = code("scripts/build-production.sh").join("\n");
    assert!(
        build.contains("cargo build --release --locked -p celestina-lock -p celestina-lock-verify")
    );
    let shell_line = build
        .lines()
        .find(|line| line.contains("--bin celestina"))
        .expect("the shell has its own build line");
    assert!(!shell_line.contains("celestina-lock"), "{shell_line}");
}

#[test]
fn verification_restores_the_lock_pair_and_checks_the_three_binaries() {
    // `cargo test --workspace` unifies features and rebuilds the shell and the
    // lock with the feature set of the whole workspace at the artifacts'
    // paths (the landing of CEL-2-H-a stopped on the shell's digest changing);
    // the shell and then the pair are built again with their production
    // commands before anything inspects them. Checked first by this test, so
    // a verification that dropped a step would fail here.
    let verify = code("scripts/verify-production.sh").join("\n");
    let tests = verify
        .find("cargo test --workspace --release --locked")
        .expect("the workspace tests run");
    let shell = verify
        .find("cargo build --release --locked --bin celestina")
        .expect("the shell is built again with the production command");
    let rebuild = verify
        .find("cargo build --release --locked -p celestina-lock -p celestina-lock-verify")
        .expect("the lock pair is built again");
    let closure = verify
        .find("--test closure")
        .expect("T-D1 runs on the production command");
    let checks = verify
        .rfind("check_lock_artifacts \"$project_root/target/release\"")
        .expect("the artifact checks run");
    assert!(tests < shell && shell < rebuild && rebuild < closure && closure < checks);
    for mark in ["unlock_probe", "/usr/bin/true", "auto_type_after", "libpam"] {
        assert!(verify.contains(mark), "the artifact checks lost {mark}");
    }
    let smoke = code("scripts/smoke.sh").join("\n");
    assert!(smoke.contains("celestina-lock-verify"));
}

#[test]
fn deploy_and_status_know_three_binaries_and_deploy_never_compiles() {
    let deploy = code("scripts/deploy-production.sh").join("\n");
    let status = code("scripts/status-production.sh").join("\n");
    for binary in ["celestina", "celestina-lock", "celestina-lock-verify"] {
        assert!(
            deploy.contains(&format!("target/release/{binary}\"")),
            "deploy does not install {binary}"
        );
        assert!(
            status.contains(&format!("target/release/{binary}=")),
            "status does not report {binary}"
        );
    }
    assert!(!deploy.contains("cargo"));
}

/// Round 5, ADR 0022 A4: the nest's niri carries the marker the lock's
/// connection guard reads, both as its configuration's name and as the `$0`
/// of the command it runs, and the rehearsal starts its nests only through
/// `dev-session.sh`.
#[test]
fn the_nest_niri_carries_the_marker_the_connection_guard_reads() {
    let marker = celestina_wayland::lock::NEST_MARKER;
    let lines = code("scripts/dev-session.sh");
    let launches: Vec<&String> = lines
        .iter()
        .filter(|l| l.contains("-c \"$generated_config\" --"))
        .collect();
    assert_eq!(launches.len(), 2, "the two ways niri is started");
    for line in launches {
        assert!(
            line.contains(&format!("\"$inside\" {marker} \"$@\"")),
            "{line}"
        );
    }
    assert!(
        read("scripts/dev-session.sh").contains(&format!("{marker}.env"))
            && read("scripts/dev-session.sh").contains(&format!("$runtime/{marker}}}.kdl")),
        "the long-lived nest's configuration is named after the marker"
    );
    // The rehearsal opens nests through `dev-session.sh` and nowhere else.
    let rehearsal = code("scripts/lock-rehearsal.sh");
    assert!(rehearsal
        .iter()
        .any(|l| l.contains("dev-session.sh\" --own-bus")));
    assert!(!rehearsal
        .iter()
        .any(|l| l.contains("niri -c") || l.contains("niri --config")));
}
