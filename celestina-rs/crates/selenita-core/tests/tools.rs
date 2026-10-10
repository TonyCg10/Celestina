use selenita_core::runner::{self, RunError};
use selenita_core::tools::{
    grim_argv, niri_window_argv, resolve, slurp_argv, slurp_colour, wl_copy_argv,
};
use selenita_core::{Geometry, Target};
use std::ffi::OsString;
use std::path::Path;
use std::time::{Duration, Instant};

fn words(argv: &[OsString]) -> Vec<String> {
    argv.iter()
        .map(|word| word.to_string_lossy().into_owned())
        .collect()
}

const GEOMETRY: Geometry = Geometry {
    x: 10,
    y: -20,
    w: 300,
    h: 400,
};

#[test]
fn grim_takes_an_output_a_window_or_a_region() {
    let out = Path::new("/tmp/out.png");
    let all = Target::Screen {
        output: String::new(),
    };
    assert_eq!(
        words(&grim_argv(&all, out, None)),
        ["grim", "-t", "png", "/tmp/out.png"]
    );
    assert_eq!(
        words(&grim_argv(&all, out, Some("DP-1"))),
        ["grim", "-t", "png", "-o", "DP-1", "/tmp/out.png"]
    );
    let one = Target::Screen {
        output: "HDMI-A-1".to_owned(),
    };
    assert_eq!(
        words(&grim_argv(&one, out, None)),
        ["grim", "-t", "png", "-o", "HDMI-A-1", "/tmp/out.png"]
    );
    let region = Target::Region { geometry: GEOMETRY };
    assert_eq!(
        words(&grim_argv(&region, out, None)),
        ["grim", "-t", "png", "-g", "10,-20 300x400", "/tmp/out.png"]
    );
    let window = Target::Window {
        id: 7,
        geometry: GEOMETRY,
        app_id: "x".to_owned(),
        title: "y".to_owned(),
    };
    assert_eq!(
        words(&grim_argv(&window, out, None))[3..5],
        ["-g", "10,-20 300x400"]
    );
}

#[test]
fn slurp_wl_copy_and_niri_argv() {
    assert_eq!(
        words(&slurp_argv("#3e91ffff", "#00000040")),
        [
            "slurp",
            "-c",
            "#3e91ffff",
            "-b",
            "#00000040",
            "-f",
            "%x,%y %wx%h"
        ]
    );
    assert_eq!(words(&wl_copy_argv()), ["wl-copy", "--type", "image/png"]);
    assert_eq!(
        words(&niri_window_argv(Path::new("/tmp/w.png"), Some(45)))[..6],
        ["niri", "msg", "action", "screenshot-window", "--id", "45"]
    );
    assert_eq!(
        words(&niri_window_argv(Path::new("/tmp/w.png"), None)),
        [
            "niri",
            "msg",
            "action",
            "screenshot-window",
            "--write-to-disk",
            "true",
            "--show-pointer",
            "false",
            "--path",
            "/tmp/w.png"
        ]
    );
}

#[test]
fn qt_colours_become_slurps() {
    assert_eq!(slurp_colour("#3e91ff").as_deref(), Some("#3e91ffff"));
    assert_eq!(slurp_colour("#663E91FF").as_deref(), Some("#3e91ff66"));
    assert_eq!(slurp_colour("blue"), None);
    assert_eq!(slurp_colour("#12345"), None);
    assert_eq!(slurp_colour("#zzzzzz"), None);
}

#[test]
fn a_stub_folder_replaces_the_program_only() {
    let argv = resolve(wl_copy_argv(), Some(Path::new("/stubs")));
    assert_eq!(words(&argv), ["/stubs/wl-copy", "--type", "image/png"]);
    assert_eq!(words(&resolve(wl_copy_argv(), None))[0], "wl-copy");
}

fn sh(script: &str) -> Vec<OsString> {
    vec!["sh".into(), "-c".into(), script.into()]
}

#[test]
fn the_runner_returns_output_and_feeds_input() {
    let output = runner::run(
        &sh("cat; echo done"),
        Some(b"png bytes "),
        Duration::from_secs(5),
    )
    .expect("runs");
    assert_eq!(output.stdout, b"png bytes done\n");
}

#[test]
fn the_runner_kills_a_child_at_the_deadline() {
    let started = Instant::now();
    let result = runner::run(&sh("sleep 5"), None, Duration::from_millis(200));
    assert_eq!(result, Err(RunError::Deadline("sh".to_owned())));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn the_runner_types_failures_and_missing_programs() {
    assert_eq!(
        runner::run(
            &sh("echo warning >&2; echo selection cancelled >&2; exit 1"),
            None,
            Duration::from_secs(5)
        ),
        Err(RunError::Failed {
            program: "sh".to_owned(),
            code: Some(1),
            stderr: "selection cancelled".to_owned()
        })
    );
    assert_eq!(
        runner::run(&["/nonexistent/grim".into()], None, Duration::from_secs(1)),
        Err(RunError::Missing("/nonexistent/grim".to_owned()))
    );
    assert_eq!(
        runner::run(&[], None, Duration::from_secs(1)),
        Err(RunError::NoProgram)
    );
}

#[test]
fn a_quiet_child_leaves_no_pipe_to_a_forked_server() {
    // The child forks a sleeper that would hold an inherited pipe for 5 s.
    let started = Instant::now();
    runner::run_quiet(
        &sh("(sleep 5 &) ; cat >/dev/null"),
        Some(b"png"),
        Duration::from_secs(3),
    )
    .expect("runs");
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn a_running_child_stops_on_sigint_and_is_killed_at_the_deadline() {
    use selenita_core::runner::{start, Exit};
    use std::time::{Duration, Instant};
    // `sh` ends on SIGINT with status 130 while it sleeps.
    let argv: Vec<OsString> = ["sh", "-c", "echo starting >&2; sleep 30"]
        .into_iter()
        .map(OsString::from)
        .collect();
    let mut running = start(&argv, None).expect("started");
    assert_eq!(running.name(), "sh");
    assert!(running.poll().is_none());
    running.interrupt().expect("interrupted");
    let began = Instant::now();
    let (exited, exit) = running.wait_until(Duration::from_secs(5));
    assert!(exited);
    assert!(began.elapsed() < Duration::from_secs(5));
    assert_ne!(exit.code, Some(0));

    let argv: Vec<OsString> = ["sh", "-c", "trap '' INT; sleep 30"]
        .into_iter()
        .map(OsString::from)
        .collect();
    let mut stubborn = start(&argv, None).expect("started");
    std::thread::sleep(Duration::from_millis(100));
    stubborn.interrupt().expect("interrupted");
    let began = Instant::now();
    let (exited, exit) = stubborn.wait_until(Duration::from_millis(300));
    assert!(!exited, "the deadline kills it");
    assert!(began.elapsed() < Duration::from_secs(2));
    assert_eq!(
        exit,
        Exit {
            code: None,
            stderr: String::new()
        }
    );
    assert!(start(&[OsString::from("/nonexistent/launcher")], None).is_err());
}
