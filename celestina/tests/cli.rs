//! The binary's command line, run as a process: the modes that need no
//! compositor and no running shell.

use std::process::{Command, Output};

fn celestina(args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_celestina"));
    cmd.args(args)
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env_remove("NIRI_SOCKET")
        .env("RUST_LOG", "warn");
    match cmd.output() {
        Ok(out) => out,
        Err(err) => panic!("cannot run celestina: {err}"),
    }
}

#[test]
fn check_exits_zero_without_a_compositor() {
    let out = celestina(&["--check"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn an_unknown_verb_exits_2_with_usage_on_stderr() {
    let out = celestina(&["island", "fly"]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unknown verb \"island fly\""), "{stderr}");
    assert!(stderr.contains("usage: celestina"), "{stderr}");
    assert!(out.stdout.is_empty());
}

#[test]
fn help_prints_the_usage_on_stdout() {
    let out = celestina(&["--help"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("island open"));
}
