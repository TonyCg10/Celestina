//! `scripts/cutover-config-probe.sh` on the two cutover fixtures (CEL-2-M,
//! design §8): the config as the session ran it before the cutover must fail
//! the probe, the one with the design's §5.3 applied must pass it, and the
//! script refuses a path under `$HOME/.config` (or `$XDG_CONFIG_HOME`),
//! however it is spelled, and a missing argument. The script runs
//! `niri validate` only when a niri is installed, so the test depends on no
//! compositor. The edited copies here stay valid KDL, so a niri that is
//! installed accepts them and the probe's own checks are what decides.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn project() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn script() -> PathBuf {
    project().join("scripts/cutover-config-probe.sh")
}

fn fixture(name: &str) -> PathBuf {
    project().join("tests/fixtures/cutover").join(name)
}

/// Runs the probe with `args`, `HOME` pointed at `home`, and returns its
/// output. The host's `HOME` and `XDG_CONFIG_HOME` never reach the script,
/// so the refusal of a `~/.config` path is tested against a directory the
/// test owns.
fn probe(home: &Path, args: &[&Path]) -> Output {
    Command::new("sh")
        .arg(script())
        .args(args)
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("sh runs the probe")
}

/// `probe` with no `HOME` in the environment at all.
fn probe_without_home(args: &[&Path]) -> Output {
    Command::new("sh")
        .arg(script())
        .args(args)
        .env_remove("HOME")
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("sh runs the probe")
}

/// `probe` with `XDG_CONFIG_HOME` set to `config_home` and `HOME` elsewhere.
fn probe_with_xdg(home: &Path, config_home: &Path, args: &[&Path]) -> Output {
    Command::new("sh")
        .arg(script())
        .args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", config_home)
        .output()
        .expect("sh runs the probe")
}

/// The after fixture with `from` replaced by `to`, written as `name` under
/// `dir`; the fixture must contain `from`.
fn edited(dir: &Path, name: &str, from: &str, to: &str) -> PathBuf {
    let after = fs::read_to_string(fixture("config-after.kdl")).expect("read the fixture");
    let changed = after.replace(from, to);
    assert_ne!(after, changed, "the fixture holds {from:?}");
    let copy = dir.join(name);
    fs::write(&copy, changed).expect("write the copy");
    copy
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!(
            "celestina-cutover-probe-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn the_config_before_the_cutover_fails_the_probe() {
    let home = Scratch::new("before");
    let out = probe(&home.0, &[&fixture("config-before.kdl")]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("still names noctalia"), "{err}");
    assert!(err.contains("forces caffeine or the night light"), "{err}");
    assert!(
        err.contains("no spawn-at-startup line starts celestina"),
        "{err}"
    );
    assert!(
        !err.contains("Mod+Shift+A"),
        "the swaylock bind is intact: {err}"
    );
}

#[test]
fn the_config_after_the_cutover_passes_the_probe() {
    let home = Scratch::new("after");
    let out = probe(&home.0, &[&fixture("config-after.kdl")]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", text(&out.stderr));
    assert!(text(&out.stdout).contains("cutover-config-probe: OK"));
}

#[test]
fn a_rebound_swaylock_key_is_a_finding() {
    let home = Scratch::new("swaylock");
    let after = fs::read_to_string(fixture("config-after.kdl")).expect("read the fixture");
    let rebound = after.replace(
        r#"Mod+Shift+A repeat=false { spawn "swaylock" "-f" "-c" "000000" "-u"; }"#,
        r#"Mod+Shift+A repeat=false { spawn "celestina" "lock"; }"#,
    );
    assert_ne!(after, rebound, "the fixture holds the swaylock bind");
    let copy = home.0.join("rebound.kdl");
    fs::write(&copy, rebound).expect("write the copy");
    let out = probe(&home.0, &[&copy]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(
        err.contains("Mod+Shift+A bind does not spawn swaylock"),
        "{err}"
    );

    let removed = after.replace(
        r#"    Mod+Shift+A repeat=false { spawn "swaylock" "-f" "-c" "000000" "-u"; }
"#,
        "",
    );
    assert_ne!(after, removed, "the fixture holds the swaylock bind");
    let copy = home.0.join("removed.kdl");
    fs::write(&copy, removed).expect("write the copy");
    let out = probe(&home.0, &[&copy]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("Mod+Shift+A bind is missing"), "{err}");
}

#[test]
fn a_path_under_the_config_home_is_refused_before_it_is_read() {
    let home = Scratch::new("home");
    let live = home.0.join(".config/niri/config.kdl");
    fs::create_dir_all(live.parent().expect("a parent")).expect("create the directory");
    fs::copy(fixture("config-after.kdl"), &live).expect("copy the fixture");
    let out = probe(&home.0, &[&live]);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", text(&out.stderr));
    assert!(text(&out.stderr).contains("pass a copy"));
}

#[test]
fn a_symlink_a_dot_dot_and_a_doubled_slash_into_the_config_home_are_refused() {
    // M1 of the cutover review: the refusal compares canonical paths, so no
    // spelling of the live file's path gets it read.
    let home = Scratch::new("spelled");
    let live = home.0.join(".config/niri/config.kdl");
    fs::create_dir_all(live.parent().expect("a parent")).expect("create the directory");
    fs::copy(fixture("config-after.kdl"), &live).expect("copy the fixture");
    let elsewhere = home.0.join("elsewhere");
    fs::create_dir_all(&elsewhere).expect("the other directory");
    let link = elsewhere.join("link.kdl");
    std::os::unix::fs::symlink(&live, &link).expect("the symlink");
    let dot_dot = elsewhere.join("../.config/niri/config.kdl");
    let doubled = PathBuf::from(format!("{}//.config/niri/config.kdl", home.0.display()));
    for (name, path) in [
        ("a symlink", &link),
        ("a .. path", &dot_dot),
        ("a doubled slash", &doubled),
    ] {
        let out = probe(&home.0, &[path]);
        let err = text(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{name}: stderr: {err}");
        assert!(err.contains("pass a copy"), "{name}: {err}");
        assert!(
            !text(&out.stdout).contains("OK"),
            "{name}: the file was read"
        );
    }
    // A real copy elsewhere, reached through a symlinked directory, is read.
    let copy = elsewhere.join("copy.kdl");
    fs::copy(fixture("config-after.kdl"), &copy).expect("the copy");
    let via = home.0.join("via");
    std::os::unix::fs::symlink(&elsewhere, &via).expect("the directory symlink");
    let out = probe(&home.0, &[&via.join("copy.kdl")]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", text(&out.stderr));
}

#[test]
fn a_path_under_xdg_config_home_is_refused_too() {
    let home = Scratch::new("xdg");
    let config_home = home.0.join("elsewhere-config");
    let live = config_home.join("niri/config.kdl");
    fs::create_dir_all(live.parent().expect("a parent")).expect("create the directory");
    fs::copy(fixture("config-after.kdl"), &live).expect("copy the fixture");
    // `HOME/.config` does not contain it; `XDG_CONFIG_HOME` does.
    let out = probe(&home.0, &[&live]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", text(&out.stderr));
    let out = probe_with_xdg(&home.0, &config_home, &[&live]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "stderr: {err}");
    assert!(err.contains("pass a copy"), "{err}");
}

#[test]
fn a_bind_written_over_several_lines_is_read_with_its_spawn() {
    // M2 of the cutover review: the probe joins a node with the lines that
    // hold its children, so the swaylock bind passes and a noctalia bind is
    // flagged whichever way they are laid out.
    let home = Scratch::new("split");
    let split = edited(
        &home.0,
        "split.kdl",
        r#"    Mod+Shift+A repeat=false { spawn "swaylock" "-f" "-c" "000000" "-u"; }
"#,
        r#"    Mod+Shift+A repeat=false {
        spawn "swaylock" "-f" "-c" "000000" "-u";
    }
"#,
    );
    let out = probe(&home.0, &[&split]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", text(&out.stderr));
    assert!(text(&out.stdout).contains("cutover-config-probe: OK"));

    let noctalia = edited(
        &home.0,
        "split-noctalia.kdl",
        r#"    Mod+N { spawn "kitty" "--directory" "/home/person"; }
"#,
        r#"    Mod+N {
        spawn "noctalia" "msg" "panel-toggle" "launcher";
    }
"#,
    );
    let out = probe(&home.0, &[&noctalia]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("still names noctalia: 1 line(s)"), "{err}");
    assert!(
        !err.contains("Mod+Shift+A"),
        "the one-line swaylock bind is intact: {err}"
    );
}

#[test]
fn spawn_sh_at_startup_counts_as_a_startup_line() {
    // M2 of the cutover review: niri 25.08's `spawn-sh-at-startup` starts
    // celestina as well as `spawn-at-startup` does, and names noctalia just
    // as badly.
    let home = Scratch::new("spawn-sh");
    let celestina = edited(
        &home.0,
        "sh-startup.kdl",
        r#"spawn-at-startup "sh" "-c" "exec systemd-cat -t celestina celestina"
"#,
        r#"spawn-sh-at-startup "exec systemd-cat -t celestina celestina"
"#,
    );
    let out = probe(&home.0, &[&celestina]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", text(&out.stderr));

    let noctalia = edited(
        &home.0,
        "sh-noctalia.kdl",
        r#"spawn-at-startup "gnome-keyring-daemon" "--start" "--components=secrets"
"#,
        r#"spawn-sh-at-startup "noctalia"
spawn-sh-at-startup "sleep 1 && noctalia msg caffeine-enable"
"#,
    );
    let out = probe(&home.0, &[&noctalia]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("still names noctalia: 2 line(s)"), "{err}");
    assert!(err.contains("forces caffeine or the night light"), "{err}");
}

#[test]
fn a_startup_line_inside_a_block_comment_or_slashdashed_does_not_start_celestina() {
    // m-2 of the cutover re-review: niri reads neither a `/* ... */` block
    // nor a `/-` node, so a celestina start written inside either is no
    // start, and the probe must say so instead of passing the file.
    let home = Scratch::new("block-comment");
    let start = "spawn-at-startup \"sh\" \"-c\" \"exec systemd-cat -t celestina celestina\"\n";
    let blocked = edited(&home.0, "blocked.kdl", start, &format!("/*\n{start}*/\n"));
    let out = probe(&home.0, &[&blocked]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(
        err.contains("no spawn-at-startup line starts celestina"),
        "{err}"
    );

    let slashdashed = edited(&home.0, "slashdashed.kdl", start, &format!("/-{start}"));
    let out = probe(&home.0, &[&slashdashed]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(
        err.contains("no spawn-at-startup line starts celestina"),
        "{err}"
    );

    // A block comment that closes, nested as KDL allows, leaves the line
    // after it in force; a `//` inside a string is text.
    let nested = edited(
        &home.0,
        "nested.kdl",
        start,
        &format!("/* a /* nested */ comment */\nspawn-at-startup \"echo\" \"http://x\"\n{start}"),
    );
    let out = probe(&home.0, &[&nested]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", text(&out.stderr));
    assert!(text(&out.stdout).contains("cutover-config-probe: OK"));
}

#[test]
fn a_slashdashed_binds_block_disables_the_swaylock_bind() {
    // m-2 of the cutover re-review: `/-binds {` disables every bind inside,
    // the swaylock recovery bind included, however intact its line looks.
    let home = Scratch::new("slashdash-binds");
    let disabled = edited(&home.0, "disabled.kdl", "binds {", "/-binds {");
    let out = probe(&home.0, &[&disabled]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("Mod+Shift+A bind is missing"), "{err}");

    // A disabled bind that named noctalia is not a finding: niri will not
    // run it. The intact file with one such line passes.
    let disabled_noctalia = edited(
        &home.0,
        "disabled-noctalia.kdl",
        r#"    Mod+N { spawn "kitty" "--directory" "/home/person"; }
"#,
        r#"    /-Mod+N { spawn "noctalia" "msg" "panel-toggle" "launcher"; }
    Mod+N { spawn "kitty" "--directory" "/home/person"; }
"#,
    );
    let out = probe(&home.0, &[&disabled_noctalia]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", text(&out.stderr));
}

#[test]
fn an_unset_home_is_a_usage_error() {
    // m-3 of the cutover re-review: without `HOME` the probe has no
    // `~/.config` to refuse, so it refuses to run rather than aborting on an
    // unset parameter.
    let out = probe_without_home(&[&fixture("config-after.kdl")]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "stderr: {err}");
    assert!(err.contains("HOME is not set"), "{err}");
    assert!(
        !text(&out.stdout).contains("OK"),
        "the file was read: {err}"
    );
}

#[test]
fn no_argument_is_a_usage_error() {
    let home = Scratch::new("usage");
    let out = probe(&home.0, &[]);
    assert_eq!(out.status.code(), Some(2));
    let out = probe(
        &home.0,
        &[&fixture("config-after.kdl"), &fixture("config-before.kdl")],
    );
    assert_eq!(out.status.code(), Some(2));
}
