//! Guards of the session lock's scripts and PAM text (CEL-2-H design §4.11,
//! §4.14, §9.2, plan Task 7). `scripts/` at the repository root is outside
//! this unit's commit roots, so these scan the project's own `scripts/` and
//! `contrib/` from a test. Each cites the rule it enforces. The last one is
//! the cutover's (CEL-2-M) and runs `dev-session.sh` against a fake niri:
//! every nest starts with `CELESTINA_DDC=0` and `CELESTINA_BACKLIGHT=0` unless
//! the caller chose a value.

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

const SCRIPTS: [&str; 11] = [
    "scripts/build-production.sh",
    "scripts/verify-production.sh",
    "scripts/deploy-production.sh",
    "scripts/status-production.sh",
    "scripts/complete-production.sh",
    "scripts/smoke.sh",
    "scripts/smoke-media.sh",
    "scripts/dev-session.sh",
    "scripts/lock-rehearsal.sh",
    "scripts/cutover-config-probe.sh",
    "scripts/wallpaper-backdrop-probe.sh",
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

/// The lines of the rehearsal's `step_r10` function, comments excluded.
fn r10_body() -> Vec<String> {
    let lines = code("scripts/lock-rehearsal.sh");
    let start = lines
        .iter()
        .position(|l| l.trim_start().starts_with("step_r10() {"))
        .expect("the rehearsal has step_r10");
    let end = lines[start..]
        .iter()
        .position(|l| l == "    }")
        .expect("step_r10 ends");
    lines[start..start + end].to_vec()
}

/// R-10 (H-b, spec §9.2) is a step of the rehearsal like the others: it is
/// named in the list of steps, the usage and the default plan, its nest is
/// 4K at scale 1.5, and it dispatches through the same verified probe.
#[test]
fn r10_is_registered_with_its_options_and_a_scale_1_5_nest() {
    let script = read("scripts/lock-rehearsal.sh");
    assert!(script.contains("R-[1-9]|R-9[abc]|R-10)"), "the step list");
    assert!(script.contains("[--check] [R-1 ... R-10 | R-9a R-9b R-9c]"));
    assert!(
        script.contains("R-9 R-10\""),
        "the default plan ends with R-10"
    );
    assert!(script.contains("        R-10) step_r10 ;;"), "the dispatch");
    assert!(script.contains("--wallpaper-folder"));
    let launch = code("scripts/lock-rehearsal.sh")
        .into_iter()
        .find(|l| l.contains("nest_args=\"--scale 1.5 --monitor $r10_monitor\""))
        .expect("R-10 asks for scale 1.5 on the 4K output");
    assert!(launch.contains("--monitor"), "{launch}");
    // The other steps keep leaving the window where the host puts it.
    assert!(script.contains("nest_args=\"--monitor none\""));
    // The wallpaper duty is switched on in the nest's throwaway configuration,
    // never in the author's; since the cutover (CEL-2-M) every other shipped
    // duty defaults to on, the polkit agent included since
    // CEL-2-M-LOCK-POLKIT, so the file names each one off and the nest runs
    // the lock and the wallpaper alone.
    let r10_file = code("scripts/lock-rehearsal.sh")
        .into_iter()
        .find(|l| l.contains("\"duties\":{\"lock\":\"on\",\"wallpaper\":\"on\","))
        .expect("R-10 writes a file with the lock and the wallpaper on");
    for duty in [
        "bubbles",
        "caffeine",
        "clipboard",
        "idle",
        "media",
        "nightlight",
        "osd",
        "phone",
        "polkit",
    ] {
        assert!(
            r10_file.contains(&format!("\"{duty}\":\"off\"")),
            "R-10 leaves {duty} off: {r10_file}"
        );
    }
}

/// R-10 acts only on the verified nest: the display guard runs first, every
/// capture and every key goes through `nestenv`/`shot`, the lock is started
/// only by the shell's own verb, and no frame time is invented.
#[test]
fn r10_touches_only_the_verified_nest_and_invents_no_frame_time() {
    let body = r10_body();
    assert!(
        body.get(1).is_some_and(|l| l.trim() == "guard || return"),
        "the nest's display is verified before anything else"
    );
    for line in &body {
        // A word that runs `grim` or `wtype` follows `nestenv`; the same names
        // inside the text of a `msg=` assignment are not commands.
        if !line.trim_start().starts_with("msg=") {
            let words: Vec<&str> = line.split_whitespace().collect();
            for (at, word) in words.iter().enumerate() {
                if *word == "grim" || *word == "wtype" {
                    assert!(
                        at > 0 && words[at - 1] == "nestenv",
                        "R-10 runs {word} outside the nest's clean environment: {line}"
                    );
                }
            }
        }
        // Nothing is stopped, started or read outside the nest.
        let text = ["msg=", "result ", "say "]
            .iter()
            .any(|lead| line.trim_start().starts_with(lead));
        for word in ["celestina-lock", "swaylock", "kill", "WAYLAND_DEBUG"] {
            assert!(text || !line.contains(word), "R-10 names {word}: {line}");
        }
    }
    let joined = body.join("\n");
    assert!(
        joined.contains("lock_through_shell"),
        "locks through the shell"
    );
    // Never Enter: no password, not even a made-up one.
    assert!(!joined.contains("Return"), "R-10 presses Enter");
    // Without a frame time from the lock, the limit is recorded as a NOTE.
    assert!(joined.contains("result R-10 NOTE"));
    assert!(joined.contains("NOT the painter's frame time"));
}

/// The two variables a nest is pinned on: the name the script exports and
/// the phrases of its notices (pinned, kept from the caller).
const NEST_SWITCHES: [(&str, &str); 2] = [
    ("CELESTINA_DDC", "never runs ddcutil"),
    ("CELESTINA_BACKLIGHT", "never reads or writes"),
];

/// What `dev-session.sh` hands the niri it starts as `CELESTINA_DDC` and
/// `CELESTINA_BACKLIGHT` (one value per line, in [`NEST_SWITCHES`] order), and
/// what it printed. The niri is a fake that records the variables and exits,
/// the host's display is a name nobody listens on and `--monitor none` keeps
/// the script off the host compositor's IPC, so no nest, no shell, no
/// `ddcutil` and no `brightnessctl` ever starts. `caller` is the caller's own
/// value of `name`, if any, the other variable being left unset; `once`
/// chooses the `--once` mode over the interactive one.
fn switches_a_nest_starts_with(
    scratch: &std::path::Path,
    name: &str,
    caller: Option<&str>,
    once: bool,
) -> (Vec<String>, String) {
    use std::process::Command;

    let fake = scratch.join("fake-niri");
    let seen = scratch.join("seen");
    let _ = fs::remove_file(&seen);
    let mut command = Command::new("sh");
    command
        .arg(manifest().join("scripts/dev-session.sh"))
        .args(["--monitor", "none", "--binary"])
        .arg(&fake);
    if once {
        command.arg("--once").arg(scratch.join("run"));
    }
    command
        .env("WAYLAND_DISPLAY", "celestina-test-no-such-display")
        .env("XDG_RUNTIME_DIR", scratch)
        .env("XDG_CONFIG_HOME", scratch.join("config"))
        .env("HOME", scratch)
        .env("CELESTINA_NEST_NIRI", &fake)
        .env("FAKE_NIRI_SEEN", &seen)
        .env_remove("CELESTINA_NEST_CONFIG")
        .env_remove("NIRI_SOCKET");
    for (variable, _) in NEST_SWITCHES {
        command.env_remove(variable);
    }
    if let Some(value) = caller {
        command.env(name, value);
    }
    let output = command.output().expect("sh runs dev-session.sh");
    let notes = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "dev-session.sh failed: {notes}");
    let seen = fs::read_to_string(&seen).unwrap_or_else(|err| panic!("the fake niri: {err}"));
    (seen.lines().map(str::to_owned).collect(), notes)
}

/// CEL-2-M: a nest never drives the host's hardware. Since the cutover the
/// OSD duty is on and the default layout places the `brightness` piece, so
/// a nested shell without `"osd": "off"` would run a real `ddcutil` at
/// start and open the host's backlight; `dev-session.sh` therefore starts
/// every nest, interactive or `--once`, with `CELESTINA_DDC=0` and
/// `CELESTINA_BACKLIGHT=0` unless the caller chose a value for one. One
/// test on purpose: nothing else in this file starts a process, so nothing
/// forks while the fake below is being written (`ETXTBSY`).
#[test]
fn every_nest_starts_with_ddc_and_backlight_forbidden_unless_the_caller_chose() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = std::env::temp_dir().join(format!("celestina-nest-ddc-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).expect("the scratch directory");
    let fake = scratch.join("fake-niri");
    fs::write(
        &fake,
        "#!/bin/sh\n\
         [ \"${1:-}\" = validate ] && exit 0\n\
         printf '%s\\n' \"${CELESTINA_DDC-unset}\" \"${CELESTINA_BACKLIGHT-unset}\" > \"$FAKE_NIRI_SEEN\"\n",
    )
    .expect("the fake niri");
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).expect("its mode");

    for once in [false, true] {
        for (index, (name, pinned)) in NEST_SWITCHES.into_iter().enumerate() {
            // Nothing chosen, or chosen empty (which the shell reads as
            // allowed and nobody means as a choice): both forbidden.
            for caller in [None, Some("")] {
                let (seen, notes) = switches_a_nest_starts_with(&scratch, name, caller, once);
                assert_eq!(seen, ["0", "0"], "once={once} {name}={caller:?}: {notes}");
                assert_eq!(
                    notes.contains(&format!(">> {name}=0 in the nest: it {pinned}")),
                    !once,
                    "the interactive nest says so, a `--once` run is silent: {notes}"
                );
            }
            // The caller's own value is kept, whichever it is, and the other
            // variable stays pinned.
            for value in ["1", "0", "false"] {
                let (seen, notes) = switches_a_nest_starts_with(&scratch, name, Some(value), once);
                let mut expected = ["0", "0"].map(str::to_owned);
                expected[index] = value.to_owned();
                assert_eq!(seen, expected, "once={once} {name}={value}: {notes}");
                assert_eq!(
                    notes.contains(&format!(">> {name}={value} kept from the caller")),
                    !once,
                    "{notes}"
                );
                let (other, _) = NEST_SWITCHES[1 - index];
                assert_eq!(
                    notes.contains(&format!(">> {other}=0 in the nest")),
                    !once,
                    "the other variable's notice is unchanged: {notes}"
                );
            }
        }
    }
    let _ = fs::remove_dir_all(&scratch);

    // The value the shell reads as "forbidden" is the one the script sets,
    // under the name the shell reads, and both are exported before either
    // launch of the nest's niri.
    assert_eq!(
        celestina::providers::brightness::DDC_ENABLED_VAR,
        NEST_SWITCHES[0].0
    );
    assert_eq!(
        celestina::providers::brightness::BACKLIGHT_ENABLED_VAR,
        NEST_SWITCHES[1].0
    );
    let lines = code("scripts/dev-session.sh");
    let exported = lines
        .iter()
        .position(|l| l.starts_with("export ") && l.contains("CELESTINA_DDC"))
        .expect("dev-session.sh exports the switches");
    for (name, _) in NEST_SWITCHES {
        assert!(
            lines[exported].contains(name),
            "`{name}` is exported with the others: {}",
            lines[exported]
        );
    }
    let launches: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains("-c \"$generated_config\" --"))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(launches.len(), 2, "the two ways niri is started");
    assert!(launches.iter().all(|launch| exported < *launch));
}
