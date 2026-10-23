//! The bindings probe as the shell runs it: `celestina --keybinds-probe PATH`
//! in a child process. Text that crashes the recursive KDL parser must cost
//! only the child; the parent gets an `Unavailable` answer and goes on.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use celestina_shell::keybinds::MAX_FILE_BYTES;

use celestina::providers::keybinds::{
    load_here, load_with, Keybinds, KeybindsUnavailable, PROBE_FLAG,
};

fn never() -> AtomicBool {
    AtomicBool::new(false)
}

fn exe() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_celestina"))
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!(
            "celestina-keybinds-probe-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        Scratch(dir)
    }

    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, text).expect("write file");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Small files that overflow the `kdl` crate's stack through paths the
/// nesting pre-scan does not see (each verified to abort an in-process parse
/// on 2026-10-17). Every one is under the size bound.
fn hostile() -> Vec<(&'static str, String)> {
    vec![
        ("slashdash", "/-".repeat(3000)),
        (
            "multiline-braces",
            format!("a \"\"\"\n{}", "{".repeat(2500)),
        ),
        ("v1-raw-comments", format!("a r\"{}", "/*".repeat(5000))),
        ("escape-comments", format!("a \"\\{}\"", "/*".repeat(20000))),
        ("property-chain", "a=".repeat(80000)),
        ("type-annotations", "a(b)".repeat(50000)),
        ("bad-unicode-escapes", "a \"\\u{zz}\" ".repeat(20000)),
    ]
}

#[test]
fn every_hostile_family_is_unavailable_and_the_parent_survives() {
    let dir = Scratch::new("hostile");
    for (name, text) in hostile() {
        assert!(text.len() <= MAX_FILE_BYTES, "{name} is within the bound");
        let path = dir.write(&format!("{name}.kdl"), &text);
        let started = Instant::now();
        let answer = load_with(exe(), &path, &never());
        assert_eq!(
            answer,
            Keybinds::Unavailable(KeybindsUnavailable::Invalid),
            "{name}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{name} took {:?}",
            started.elapsed()
        );
    }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../celestina-rs/crates/celestina-shell/tests/fixtures/niri-binds.kdl")
}

#[test]
fn the_fixture_loads_through_the_probe_as_it_does_in_process() {
    let through_probe = load_with(exe(), &fixture(), &never());
    let Keybinds::Groups(groups) = &through_probe else {
        panic!("the fixture loads: {through_probe:?}");
    };
    let names: Vec<_> = groups.iter().map(|g| g.name.as_deref()).collect();
    assert_eq!(names, [Some("monitor"), Some("size"), None]);
    assert_eq!(through_probe, load_here(&fixture()));
}

#[test]
fn the_probe_follows_includes_and_reports_bad_text() {
    let dir = Scratch::new("includes");
    dir.write("more.kdl", "binds { Mod+B { spawn \"b\"; } }");
    let root = dir.write(
        "config.kdl",
        "binds { Mod+A { spawn \"a\"; } }\ninclude \"more.kdl\"",
    );
    let Keybinds::Groups(groups) = load_with(exe(), &root, &never()) else {
        panic!("the config loads");
    };
    let chords: Vec<_> = groups[0].binds.iter().map(|b| b.chord.as_str()).collect();
    assert_eq!(chords, ["Mod+A", "Mod+B"]);
    let bad = dir.write("bad.kdl", "binds {");
    assert_eq!(
        load_with(exe(), &bad, &never()),
        Keybinds::Unavailable(KeybindsUnavailable::Invalid)
    );
}

#[test]
fn the_probe_mode_prints_one_answer_and_nothing_on_stderr() {
    let out = std::process::Command::new(exe())
        .arg(PROBE_FLAG)
        .arg(fixture())
        .env_clear()
        .output()
        .expect("the probe runs");
    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with('{') && text.contains("\"v\":1"), "{text}");
}

#[test]
fn the_child_alone_reports_a_missing_unreadable_or_oversize_path() {
    let dir = Scratch::new("paths");
    assert_eq!(
        load_with(exe(), &dir.0.join("nope.kdl"), &never()),
        Keybinds::Unavailable(KeybindsUnavailable::Missing)
    );
    assert_eq!(
        load_with(exe(), &dir.0, &never()),
        Keybinds::Unavailable(KeybindsUnavailable::Unreadable)
    );
    let big = dir.write("big.kdl", &" ".repeat(MAX_FILE_BYTES + 1));
    assert_eq!(
        load_with(exe(), &big, &never()),
        Keybinds::Unavailable(KeybindsUnavailable::TooLarge)
    );
}
