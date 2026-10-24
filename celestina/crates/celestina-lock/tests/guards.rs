//! Structural guards over the lock's sources (design §5, §6 "never"). The
//! properties are "this code does not exist", which a behavioural test cannot
//! show, so each test reads the production part of every source file (what
//! remains once its `#[cfg(test)]` items are removed) without comment lines, and cites the
//! threat or "never" item it enforces. `scripts/` is outside the unit's commit
//! roots, which is why these are tests.

use std::fs;
use std::path::{Path, PathBuf};

// The scanner helper shared by the lock's source guards (round 6, m-d).
#[path = "../../celestina-wayland/tests/support/test_gates.rs"]
mod test_gates;
use test_gates::without_test_blocks;

fn src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// (path relative to `src`, production code lines without comments).
fn production() -> Vec<(String, Vec<String>)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("read src") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let mut paths = Vec::new();
    walk(&src(), &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let text = fs::read_to_string(&path).expect("read");
            let code = without_test_blocks(&text);
            let lines = code
                .lines()
                .map(str::trim)
                .filter(|line| !line.starts_with("//") && !line.is_empty())
                .map(str::to_owned)
                .collect();
            let name = path
                .strip_prefix(src())
                .expect("under src")
                .to_string_lossy()
                .into_owned();
            (name, lines)
        })
        .collect()
}

/// Files in which any line contains `needle`.
fn files_with(needle: &str) -> Vec<String> {
    production()
        .into_iter()
        .filter(|(_, lines)| lines.iter().any(|line| line.contains(needle)))
        .map(|(name, _)| name)
        .collect()
}

/// Occurrences of `needle` in the production code (round 6, m-d: not the
/// number of lines that contain it).
fn count(needle: &str) -> usize {
    production()
        .iter()
        .flat_map(|(_, lines)| lines)
        .map(|line| line.matches(needle).count())
        .sum()
}

#[test]
fn the_sources_were_found() {
    let names: Vec<String> = production().into_iter().map(|(n, _)| n).collect();
    for expected in [
        "app.rs",
        "backdrop.rs",
        "bin/unlock_probe.rs",
        "driver.rs",
        "field.rs",
        "hardening.rs",
        "launch.rs",
        "lib.rs",
        "machine.rs",
        "main.rs",
        "present.rs",
        "report.rs",
        "state_file.rs",
        "unlock.rs",
        "verify.rs",
        "wake.rs",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "{expected} in {names:?}"
        );
    }
}

/// Root invariants and TM-1: no production `unwrap`, `expect`, `panic!`, and
/// no `unsafe` anywhere in the lock (the exceptions are `egl.rs` and the
/// verifier's `pam.rs`, not here).
#[test]
fn no_unwrap_expect_panic_or_unsafe_in_production() {
    let forbidden = [
        ".unwrap(",
        ".expect(",
        "panic!",
        "unreachable!",
        "todo!",
        "unimplemented!",
        "unsafe",
    ];
    for (name, lines) in production() {
        for line in lines {
            if line == "#![forbid(unsafe_code)]" {
                continue;
            }
            for needle in forbidden {
                assert!(!line.contains(needle), "{name}: {needle} in {line}");
            }
        }
        // Every file forbids `unsafe` itself.
    }
    for (name, lines) in production() {
        assert!(
            lines.iter().any(|l| l == "#![forbid(unsafe_code)]"),
            "{name} must forbid unsafe"
        );
    }
}

/// TM-8, §6 item 13: the only process the lock starts is the verifier, from
/// the descriptor it pinned, never by a name `PATH` could resolve.
#[test]
fn command_new_exists_only_in_verify_rs_and_runs_the_pinned_descriptor() {
    for needle in ["Command::new", "process::Command", "Command,", "Command}"] {
        let files = files_with(needle);
        assert!(
            files.iter().all(|f| f == "verify.rs"),
            "`{needle}` outside verify.rs: {files:?}"
        );
    }
    assert_eq!(count("Command::new("), 1);
    assert_eq!(count("Command::new(verifier.binary.exec_path())"), 1);
    assert_eq!(count("exec_path()"), 1, "its one use");
    assert_eq!(count(".process_group(0)"), 1);
    assert_eq!(count(".env_clear()"), 1);
    // The environment handed on is `PATH` and nothing else (T-E1).
    assert_eq!(count(".env(\"PATH\", VERIFIER_PATH)"), 1);
    assert_eq!(count(".env("), 1);
    assert_eq!(count(".envs("), 0);
}

/// §6 items 12 and 25, TM-7b: the lock opens no user file. The few opens are
/// a literal `/proc` path, the descriptor of the pinned verifier, and the
/// author's pid file.
#[test]
fn the_lock_opens_no_file_of_the_users() {
    assert_eq!(files_with("File::open("), ["hardening.rs", "verify.rs"]);
    assert_eq!(
        count("File::open(\"/proc/self/status\")"),
        1,
        "hardening reads one literal path"
    );
    // The one directory listed is the process's own descriptor table, to close
    // what the shell handed over by accident (round 4, I-α).
    assert_eq!(files_with("read_dir("), ["hardening.rs"]);
    // Round 5, m-2: one listing, not merely one file that lists.
    assert_eq!(count("read_dir("), 1, "one directory listing in the lock");
    assert_eq!(
        count("fs::read_dir(\"/proc/self/fd\")"),
        1,
        "hardening lists one literal path"
    );
    assert_eq!(files_with("OpenOptions"), ["state_file.rs"]);
    for needle in [
        "fs::read(",
        "fs::read_to_string(",
        "read_to_end",
        "image::",
        "Decoder",
        "load_system_fonts",
        "load_font_file",
        "load_fonts_dir",
        "load_font_source",
        "FontSystem::new(",
        "fontconfig",
        "freedesktop",
    ] {
        assert!(
            files_with(needle).is_empty(),
            "{needle}: {:?}",
            files_with(needle)
        );
    }
    // The one font database is the painter's, filled from the embedded face
    // alone (T-F1 counts its faces; the calls above never appear).
    assert_eq!(files_with("fontdb"), ["paint.rs"]);
    assert_eq!(count("load_font_data("), 1);
    assert_eq!(count("include_bytes!("), 1);
    // Existence checks (`stat`) of the compiled service paths are the only
    // other file access.
    assert_eq!(files_with("symlink_metadata"), ["launch.rs"]);
}

/// CEL-2-H-b, TM-7, TM-7b, §6 items 12 and 25: the dimmed backdrop opens no file
/// of the user. Its pixels come from the sealed bundle on the one descriptor the
/// shell handed over (stdin), already decoded; `backdrop.rs` has no path, no
/// file API, no environment and no decoder, and nothing but the launch reads a
/// bundle.
#[test]
fn the_lock_opens_no_user_file_for_the_backdrop() {
    let all = production();
    let (_, lines) = all
        .iter()
        .find(|(name, _)| name == "backdrop.rs")
        .expect("backdrop.rs");
    for needle in [
        "std::fs",
        "File::",
        "OpenOptions",
        "read_dir",
        "read_to_end",
        "read_to_string",
        "Path",
        "PathBuf",
        "env::var",
        "std::env",
        "include_bytes",
        "include_str",
        "image::",
        "Decoder",
        "Command",
        "from_raw_fd",
        "openat",
        "open(",
    ] {
        assert!(
            lines.iter().all(|line| !line.contains(needle)),
            "backdrop.rs holds `{needle}`"
        );
    }
    // No string literal in it names a path.
    assert!(
        lines
            .iter()
            .all(|line| !line.contains("\"/") && !line.contains("\"~")),
        "a path literal in backdrop.rs"
    );
    // The one read is a positioned read on the descriptor it was given, and
    // the one place that hands it stdin is the launch.
    assert_eq!(count("pread("), 1, "one positioned read");
    assert_eq!(count("fn read_bundle("), 1);
    assert_eq!(files_with("read_bundle("), ["backdrop.rs", "launch.rs"]);
    assert_eq!(count("stdin()"), 1, "the bundle's descriptor is stdin");
    assert_eq!(files_with("stdin()"), ["launch.rs"]);
    // The worker owns no Wayland object and no file: it takes blocks and
    // sizes in, bases out.
    assert!(
        lines.iter().any(|line| line.contains("name(\"backdrop\"")),
        "the worker is the thread named `backdrop`"
    );
}

/// TM-7b, T-F1: the painter resolves no icon and reads no theme; every glyph is
/// in the one embedded face.
#[test]
fn no_icon_lookup_exists() {
    for needle in [
        "icon_theme",
        "IconTheme",
        "icon_name",
        "lookup_icon",
        "xdg_data",
        "XDG_DATA",
        "/usr/share",
        "index.theme",
        "resvg",
        "usvg",
    ] {
        assert!(
            files_with(needle).is_empty(),
            "{needle}: {:?}",
            files_with(needle)
        );
    }
}

/// §6 item 8, TM-11: no key's text or keysym, and no secret, is formatted,
/// printed or logged.
#[test]
fn no_key_text_keysym_or_secret_is_ever_formatted() {
    let sensitive = ["utf8", "keysym", "secret", "Secret", "field.", "bytes"];
    let formatting = [
        "format!",
        "write!",
        "writeln!",
        "println!",
        "eprintln!",
        "print!",
        "eprint!",
        "dbg!",
        "info!(",
        "warn!(",
        "error!(",
        "debug!(",
        "trace!(",
        "{:?}",
        "{:#?}",
    ];
    for (name, lines) in production() {
        for line in &lines {
            let sensitive_here = sensitive.iter().any(|s| line.contains(s));
            let formats = formatting.iter().any(|f| line.contains(f));
            assert!(!(sensitive_here && formats), "{name}: {line}");
            // Whole events and inputs are never given to a log macro (their
            // `Debug` is redacted, but the rule does not rest on it).
            if formats {
                for debug_of in [
                    "?ev", "?event", "?input", "?key", "{event:", "{input:", "{ev:",
                ] {
                    assert!(!line.contains(debug_of), "{name}: {line}");
                }
            }
        }
    }
    // `Display` is never implemented for the secret types.
    for (name, lines) in production() {
        for line in &lines {
            assert!(
                !(line.contains("impl fmt::Display for")
                    || line.contains("impl std::fmt::Display for"))
                    || name == "launch.rs",
                "{name}: {line}"
            );
        }
    }
}

/// TM-3, TM-14, TM-28, §6 item 10: the lock creates no layer surface, binds no
/// clipboard, no text input, no popup role: it asks the toolkit's lock
/// profile for a session lock and nothing else.
#[test]
fn no_other_wayland_role_is_asked_for() {
    for needle in [
        "layer_shell",
        "LayerShell",
        "LayerSurface",
        "xdg_wm_base",
        "XdgShell",
        "data_device",
        "DataDevice",
        "primary_selection",
        "text_input",
        "TextInput",
        "input_method",
        "background_effect",
        "wayland_client::",
        "smithay_client_toolkit",
    ] {
        assert!(
            files_with(needle).is_empty(),
            "{needle}: {:?}",
            files_with(needle)
        );
    }
}

/// §4.1 ("a D-Bus connection ... never"), §5: the lock has no socket or bus
/// of its own; its only channels are the status pipe and the Wayland
/// connection the toolkit owns.
#[test]
fn no_socket_no_bus_no_network() {
    for needle in [
        "zbus",
        "dbus",
        "UnixStream",
        "UnixListener",
        "UnixDatagram",
        "TcpStream",
        "TcpListener",
        "UdpSocket",
        "std::net",
        "socket(",
    ] {
        assert!(
            files_with(needle).is_empty(),
            "{needle}: {:?}",
            files_with(needle)
        );
    }
}

/// §6 items 17 and 22: nothing is killed by name, and the only signals sent
/// go to the verifier this process spawned (its pidfd or its group).
#[test]
fn nothing_is_killed_by_name_and_only_the_own_child_is_signalled() {
    for needle in ["pkill", "killall", "kill -", "Pid::from_raw", "libc::kill"] {
        assert!(
            files_with(needle).is_empty(),
            "{needle}: {:?}",
            files_with(needle)
        );
    }
    assert_eq!(files_with("pidfd_send_signal("), ["verify.rs"]);
    assert_eq!(files_with("kill_process_group("), ["verify.rs"]);
    assert_eq!(count("Pid::from_child(&child)"), 1);
    // Round B7c, B-2: `nix::sys::signal` is used for the one mask call and
    // nothing else: it sends no signal and never unblocks one.
    assert_eq!(files_with("nix::sys::signal"), ["hardening.rs"]);
    assert_eq!(count("pthread_sigmask("), 1);
    for needle in [
        "SIG_UNBLOCK",
        "SIG_SETMASK",
        "nix::sys::signal::kill",
        "sigaction",
        "signal::raise",
    ] {
        assert!(
            files_with(needle).is_empty(),
            "{needle}: {:?}",
            files_with(needle)
        );
    }
}

/// The first statement of a function (the lines of its body up to the first
/// one that ends a statement), joined, from the production lines of a file.
fn first_statement(lines: &[String], signature: &str) -> String {
    let at = lines
        .iter()
        .position(|l| l.starts_with(signature))
        .unwrap_or_else(|| panic!("{signature} not found"));
    let mut statement = String::new();
    for line in &lines[at + 1..] {
        statement.push_str(line);
        statement.push(' ');
        if line.ends_with(';') {
            break;
        }
    }
    statement
}

/// Round 5, m-1 (amended in round B7c, B-2): the descriptors the lock was not
/// handed are closed by one call, which is the second step of the first
/// statement of `launch::start` (the first is the signal mask), and the only
/// `nix` items the lock uses are the `close` inside that function's callee
/// and the mask call.
#[test]
fn m1_the_inherited_descriptors_are_closed_first_by_exactly_one_call() {
    assert_eq!(count("hardening::close_inherited_descriptors("), 1);
    assert_eq!(count("fn close_inherited_descriptors("), 1);
    let launch = production()
        .into_iter()
        .find(|(name, _)| name == "launch.rs")
        .expect("launch.rs")
        .1;
    let first = first_statement(&launch, "pub fn start(");
    let mask = first.find("hardening::block_ignored_signals(");
    let close = first.find("hardening::close_inherited_descriptors(");
    assert!(
        matches!((mask, close), (Some(m), Some(c)) if m < c),
        "the mask first, the closing of descriptors second, in the first statement of start: {first}"
    );
    assert!(
        first
            .trim_start()
            .starts_with("let hardened = hardening::block_ignored_signals("),
        "nothing runs before the mask: {first}"
    );
    assert_eq!(count("hardening::block_ignored_signals("), 1);
    // `nix::` as a path, not the tail of `unix::`.
    let nix_users: Vec<String> = production()
        .into_iter()
        .filter(|(_, lines)| {
            lines.iter().any(|line| {
                line.match_indices("nix::")
                    .any(|(at, _)| !line[..at].ends_with('u'))
            })
        })
        .map(|(name, _)| name)
        .collect();
    assert_eq!(nix_users, ["hardening.rs"]);
    assert_eq!(count("nix::unistd::close("), 1);
}

/// §6 item 20: `PR_SET_NO_NEW_PRIVS` is never set, here or anywhere.
#[test]
fn no_new_privs_is_never_set() {
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

/// §6 items 13 and 14, TM-8: no environment variable names a binary, a
/// service, a library or a level. The environment is read in `launch.rs` only,
/// and for three allowlisted names (`WAYLAND_SOCKET` is added by round 4: the
/// number of an inherited descriptor to keep open, never a path or a name);
/// `argv` is read once.
#[test]
fn no_override_variable_is_read() {
    let files = files_with("env::var");
    assert_eq!(files, ["launch.rs"]);
    let reads: Vec<String> = production()
        .into_iter()
        .flat_map(|(_, lines)| lines)
        .filter(|line| line.contains("env::var"))
        .collect();
    assert_eq!(
        reads,
        [
            "let socket = std::env::var_os(\"WAYLAND_SOCKET\");",
            "let runtime = std::env::var_os(\"XDG_RUNTIME_DIR\")?;",
            "let display = std::env::var_os(\"WAYLAND_DISPLAY\")?;"
        ]
    );
    for needle in [
        "env::vars",
        "env::set_var",
        "env::remove_var",
        "RUST_LOG",
        "EnvFilter",
        "CELESTINA_LOCK",
        "LD_PRELOAD",
    ] {
        assert!(
            files_with(needle).is_empty(),
            "{needle}: {:?}",
            files_with(needle)
        );
    }
    assert_eq!(files_with("env::args_os()"), ["main.rs"]);
    assert_eq!(files_with("env::current_exe"), ["launch.rs"]);
}

/// TM-1, T-U1: the unlock is one call, in one file, and it needs the
/// `Authenticated` token, which has one constructor in one function.
#[test]
fn t_u1_unlock_rs_is_the_single_call_site_and_authenticated_has_no_public_constructor() {
    // The toolkit's unlock method is called in unlock.rs and nowhere else.
    assert_eq!(files_with(".unlock()"), ["unlock.rs"]);
    assert_eq!(count(".unlock()"), 1);
    // `unlock::unlock` is called from the host adapter, once.
    assert_eq!(files_with("unlock(capability, token)"), ["app.rs"]);
    assert_eq!(count("unlock(capability, token)"), 1);
    // The capability type is named only where it is held and where it is spent.
    assert_eq!(files_with("UnlockCapability"), ["app.rs", "unlock.rs"]);
    // The token: one `mint`, called from `classify`, defined in verify.rs.
    assert_eq!(files_with("Authenticated::mint()"), ["verify.rs"]);
    assert_eq!(
        count("Authenticated::mint()"),
        1,
        "`classify`, the one call"
    );
    assert_eq!(count("fn mint()"), 1);
    assert_eq!(count("Self(())"), 1);
    for (name, lines) in production() {
        if name == "verify.rs" {
            continue;
        }
        for line in lines {
            assert!(
                !line.contains("Authenticated(())") && !line.contains("Authenticated {"),
                "{name} builds a token: {line}"
            );
        }
    }
    // The field is private and the type carries no derive.
    let verify = production()
        .into_iter()
        .find(|(name, _)| name == "verify.rs")
        .expect("verify.rs")
        .1;
    let at = verify
        .iter()
        .position(|l| l == "pub struct Authenticated(());")
        .expect("the token's definition: a private unit field");
    assert!(
        !verify[at.saturating_sub(2)..at]
            .iter()
            .any(|l| l.contains("derive")),
        "no derive on the token"
    );
    // No `Unlock` verb or method anywhere in the lock besides the one site.
    assert_eq!(
        count("fn unlock("),
        3,
        "`unlock::unlock`, the host trait's method and the adapter's impl of it"
    );
}

mod token_traits {
    //! The token is zero-sized and implements none of `Clone`, `Copy`,
    //! `Default`: a compile-time check without a dev-dependency. The two
    //! blanket impls make the call ambiguous (and the test not compile) if
    //! the type ever gains the trait.
    use celestina_lock::verify::Authenticated;

    trait AmbiguousIfClone<A> {
        fn some_item() {}
    }
    impl<T: ?Sized> AmbiguousIfClone<()> for T {}
    impl<T: Clone> AmbiguousIfClone<u8> for T {}

    trait AmbiguousIfCopy<A> {
        fn some_item() {}
    }
    impl<T: ?Sized> AmbiguousIfCopy<()> for T {}
    impl<T: Copy> AmbiguousIfCopy<u8> for T {}

    trait AmbiguousIfDefault<A> {
        fn some_item() {}
    }
    impl<T: ?Sized> AmbiguousIfDefault<()> for T {}
    impl<T: Default> AmbiguousIfDefault<u8> for T {}

    #[test]
    fn authenticated_is_zero_sized_and_neither_clone_nor_copy_nor_default() {
        assert_eq!(std::mem::size_of::<Authenticated>(), 0);
        <Authenticated as AmbiguousIfClone<_>>::some_item();
        <Authenticated as AmbiguousIfCopy<_>>::some_item();
        <Authenticated as AmbiguousIfDefault<_>>::some_item();
    }
}
