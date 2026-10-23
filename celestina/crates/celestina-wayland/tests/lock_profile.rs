//! The lock profile's structural guarantees (CEL-2-H design §4.13): what it
//! binds, that exactly one place can end a lock and it consumes a
//! non-cloneable capability, that the `gl` feature really separates the GPU
//! code, and that nothing in the profile formats what was typed.
//!
//! Most checks read the profile's own sources: the properties are "this code
//! does not exist", which a behavioural test cannot show. Each cites the
//! threat it enforces (design §5).

use std::{fs, path::PathBuf, process::Command};

use celestina_wayland::lock::{UnlockCapability, BOUND_GLOBALS};

// The one scanner helper of the lock's source guards (round 6, m-d); the two
// other crates include this same file.
#[path = "support/test_gates.rs"]
mod test_gates;
use test_gates::without_test_blocks;

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The lock profile's production sources: `src/lock.rs` and `src/lock/*.rs`
/// with every `#[cfg(test)]` item of each removed, as (name, text).
fn lock_sources() -> Vec<(String, String)> {
    let src = crate_dir().join("src");
    let mut files = vec![src.join("lock.rs")];
    let mut dir: Vec<_> = fs::read_dir(src.join("lock"))
        .expect("src/lock exists")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    dir.sort();
    files.extend(dir);
    files
        .into_iter()
        .map(|path| {
            let text = fs::read_to_string(&path).expect("a readable source");
            (path.display().to_string(), without_test_blocks(&text))
        })
        .collect()
}

/// Source lines without comments, so a doc comment may name what the code
/// must not use.
fn code_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//"))
        .collect()
}

#[test]
fn t_w2_the_lock_profile_binds_exactly_its_list() {
    // Every global the profile obtains, and where: each binding call below is
    // matched to an entry of `BOUND_GLOBALS`; any other binding call, or a
    // forbidden protocol anywhere in the profile, fails.
    let expected_calls: &[(&str, &[&str])] = &[
        // The registry itself: it lists globals, it binds none.
        ("RegistryState::new(", &[]),
        ("CompositorState::bind(", &["wl_compositor"]),
        ("Shm::bind(", &["wl_shm"]),
        (
            "OutputState::new(",
            &["wl_output", "zxdg_output_manager_v1"],
        ),
        ("SeatState::new(", &["wl_seat"]),
        ("SessionLockState::new(", &["ext_session_lock_manager_v1"]),
        // The one optional-global helper: viewporter and fractional scale,
        // for frames at a fractional scale (absent: scale 1).
        (
            "globals.bind::<I,",
            &["wp_viewporter", "wp_fractional_scale_manager_v1"],
        ),
    ];
    let mut covered: Vec<&str> = Vec::new();
    let mut found = Vec::new();
    for (name, text) in lock_sources() {
        for line in code_lines(&text) {
            let binds = line.contains("::bind(")
                || line.contains("globals.bind")
                || line.contains("State::new(&globals");
            if !binds {
                continue;
            }
            let Some((call, globals)) = expected_calls.iter().find(|(call, _)| line.contains(call))
            else {
                panic!("{name}: an unlisted binding call: {line}");
            };
            found.push(*call);
            covered.extend(globals.iter());
        }
    }
    found.sort_unstable();
    let mut wanted: Vec<_> = expected_calls.iter().map(|(call, _)| *call).collect();
    wanted.sort_unstable();
    assert_eq!(
        found, wanted,
        "each listed binding call exists exactly once"
    );
    covered.sort_unstable();
    let mut listed: Vec<_> = BOUND_GLOBALS.to_vec();
    listed.sort_unstable();
    assert_eq!(covered, listed, "BOUND_GLOBALS is the union of the calls");

    // TM-14 / TM-28: no paste, no input method, no popup, no GPU path.
    let forbidden = [
        "LayerShell",
        "wlr_layer",
        "WmBase",
        "xdg_wm_base",
        "XdgWmBase",
        "data_device",
        "DataDevice",
        "primary_selection",
        "PrimarySelection",
        "text_input",
        "TextInput",
        "background_effect",
        "BackgroundEffect",
        "single_pixel",
        "SinglePixel",
        "cursor_shape",
        "CursorShape",
        "crate::egl",
        "crate::popup",
        "crate::runtime",
        "crate::surface",
        "crate::blur",
    ];
    for (name, text) in lock_sources() {
        for line in code_lines(&text) {
            for word in forbidden {
                assert!(!line.contains(word), "{name} mentions {word}: {line}");
            }
        }
    }
}

#[test]
fn the_cover_is_a_zeroed_shm_xrgb8888_buffer_and_no_single_pixel_path_exists() {
    // Spike Q4: niri serves no `wp_single_pixel_buffer_manager_v1`, so the
    // zeroed shm buffer is the only path (checked above: no `single_pixel`).
    let cover = lock_sources()
        .into_iter()
        .find(|(name, _)| name.ends_with("cover.rs"))
        .expect("cover.rs")
        .1;
    let code = code_lines(&cover).join("\n");
    assert!(code.contains("Format::Xrgb8888"));
    // A fresh memory file is zero: nothing may write the pool.
    for write in [".mmap(", "write_all", "copy_from_slice", ".fill("] {
        assert!(
            !code.contains(write),
            "the cover must not write its pool: {write}"
        );
    }
}

#[test]
fn t_u1_unlock_capability_is_created_once_is_neither_clone_nor_copy_and_is_consumed() {
    // Compile-time: `UnlockCapability` is neither `Clone` nor `Copy`. If it
    // were, `<T as AmbiguousIfClone<_>>` would have two candidate impls and
    // the inference below would not compile.
    trait AmbiguousIfClone<A> {
        fn probe() {}
    }
    impl<T: ?Sized> AmbiguousIfClone<()> for T {}
    impl<T: Clone> AmbiguousIfClone<u8> for T {}
    let _ = <UnlockCapability as AmbiguousIfClone<_>>::probe;

    // `unlock` consumes the capability: a function pointer of this exact
    // type exists only for a `self`-taking method.
    let _: fn(UnlockCapability) -> Result<(), celestina_wayland::lock::UnlockError> =
        UnlockCapability::unlock;

    // Source scan: one place sends the unlock, one place mints the
    // capability, and no public `unlock_session` exists (TM-1, TM-25).
    let mut unlock_sites = Vec::new();
    let mut issue_sites = Vec::new();
    let mut leaks = Vec::new();
    for (name, text) in lock_sources() {
        for line in code_lines(&text) {
            if line.contains(".unlock(") || line.contains("unlock_and_destroy") {
                unlock_sites.push(format!("{name}: {line}"));
            }
            if line.contains("UnlockCapability::issue(") {
                issue_sites.push(format!("{name}: {line}"));
            }
            let public = [
                "pub fn",
                "pub struct",
                "pub enum",
                "pub trait",
                "pub type",
                "pub use",
            ]
            .iter()
            .any(|p| line.starts_with(p));
            if public
                && (line.contains("SessionLock,")
                    || line.contains("SessionLock)")
                    || line.contains(": SessionLock")
                    || line.contains("-> SessionLock")
                    || line.contains("unlock_session"))
            {
                leaks.push(format!("{name}: {line}"));
            }
        }
    }
    assert_eq!(unlock_sites.len(), 1, "one unlock site: {unlock_sites:?}");
    assert!(
        unlock_sites[0].contains("capability.rs"),
        "{unlock_sites:?}"
    );
    assert_eq!(issue_sites.len(), 1, "one mint site: {issue_sites:?}");
    assert!(issue_sites[0].contains("handlers.rs"), "{issue_sites:?}");
    assert!(
        leaks.is_empty(),
        "the raw lock must not be public: {leaks:?}"
    );
    // And nowhere else in the crate: the shell profile never touches it.
    let src = crate_dir().join("src");
    for entry in fs::read_dir(&src).expect("src") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            let text = fs::read_to_string(&path).expect("source");
            assert!(
                !text.contains("unlock_and_destroy") && !text.contains("unlock_session"),
                "{} reaches for the unlock",
                path.display()
            );
        }
    }
}

#[test]
fn the_profile_never_formats_a_key_event_or_a_keysym() {
    // TM-11 / design §6 item 8: nothing in the profile logs, formats or
    // prints an input event, a key event or a keysym.
    for (name, text) in lock_sources() {
        for line in code_lines(&text) {
            let logs = [
                "tracing::",
                "println!",
                "eprintln!",
                "format!",
                "write!",
                "dbg!",
            ]
            .iter()
            .any(|m| line.contains(m));
            if logs {
                // Output and seat logging names `output`, `size`, `serial`;
                // an input or key event, a keysym or its text never.
                for word in [
                    "keysym",
                    "utf8",
                    "KeyEvent",
                    "InputEvent",
                    "?ev",
                    "%ev",
                    "{ev",
                    "?event,",
                    "?event)",
                    "?event ",
                    "%event",
                    "{event",
                ] {
                    assert!(!line.contains(word), "{name}: {line}");
                }
            }
        }
    }
}

fn cargo_tree(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO"))
        .arg("tree")
        .args(["-e", "normal", "--prefix", "none", "--locked", "--offline"])
        .args(["-p", "celestina-wayland"])
        .args(args)
        .current_dir(crate_dir())
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf-8 tree")
}

fn crate_names(tree: &str) -> Vec<String> {
    let mut names: Vec<String> = tree
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    names
}

#[test]
fn the_gl_feature_off_does_not_link_egl_blur_or_popup() {
    // TM-7b / design §6 item 25: with `gl` off the toolkit depends on no EGL,
    // GL or femtovg crate, so the lock links none of them. The default build
    // is the control: it must contain them, or this test would pass vacuously.
    let gpu = ["khronos-egl", "wayland-egl", "glow", "femtovg"];
    let with_gl = crate_names(&cargo_tree(&[]));
    for name in gpu {
        assert!(
            with_gl.iter().any(|n| n == name),
            "the shell profile has {name}"
        );
    }
    let without = crate_names(&cargo_tree(&["--no-default-features"]));
    for name in gpu {
        assert!(
            !without.iter().any(|n| n == name),
            "{name} is linked into the lock profile"
        );
    }
    assert!(without.iter().any(|n| n == "smithay-client-toolkit"));
    // The shell-profile modules are compiled only with the feature.
    let lib = fs::read_to_string(crate_dir().join("src/lib.rs")).expect("lib.rs");
    for module in [
        "blur",
        "data_device",
        "egl",
        "handlers",
        "popup",
        "runtime",
        "surface",
        "text_input",
    ] {
        let gated = lib.lines().collect::<Vec<_>>().windows(2).any(|w| {
            w[0].trim() == "#[cfg(feature = \"gl\")]" && w[1].contains(&format!("mod {module};"))
        });
        assert!(gated, "mod {module} must be behind the gl feature");
    }
}

// ── Round 5, ADR 0022 A4: the lock never locks a session by accident ──────

/// Every production `.rs` file of the workspace that can start a lock client:
/// the crates' `src` and `examples` and the shell's own `src`, as (path, text
/// without its `#[cfg(test)]` items).
fn client_sources() -> Vec<(PathBuf, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let workspace = crate_dir().join("../..");
    let mut roots = vec![workspace.join("src")];
    for entry in fs::read_dir(workspace.join("crates"))
        .expect("crates")
        .flatten()
    {
        roots.push(entry.path().join("src"));
        roots.push(entry.path().join("examples"));
    }
    let mut files = Vec::new();
    for root in roots {
        walk(&root, &mut files);
    }
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let text = fs::read_to_string(&path).expect("a readable source");
            (path, without_test_blocks(&text))
        })
        .collect()
}

fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// The one path to `request_lock` runs through the connection guard, first
/// (finding of the 2026-10-22 incident).
#[test]
fn a_the_only_path_to_request_lock_goes_through_the_guard_first() {
    let sources = lock_sources();
    let (_, lock_rs) = sources
        .iter()
        .find(|(name, _)| name.ends_with("src/lock.rs"))
        .expect("lock.rs");
    let body = &lock_rs[lock_rs.find("pub fn run_lock").expect("run_lock")..];
    let at = |needle: &str| {
        body.find(needle)
            .unwrap_or_else(|| panic!("{needle} in run_lock"))
    };
    assert!(
        at("connect::connect(policy)") < at("registry_queue_init"),
        "the guard decides before the first Wayland request"
    );
    assert!(at("registry_queue_init") < at("rt.request_lock("));
    // Exactly one caller of `request_lock`, and it is `run_lock`.
    let mut callers = 0;
    for (path, text) in client_sources() {
        for line in text.lines().map(str::trim) {
            if !line.starts_with("//")
                && line.contains("request_lock(")
                && !line.contains("fn request_lock")
            {
                callers += 1;
                assert_eq!(file_name(&path), "lock.rs", "{}: {line}", path.display());
            }
        }
    }
    assert_eq!(callers, 1, "request_lock has one call site");
    // No code of the profile opens a connection but the guard.
    for (name, text) in &sources {
        if name.ends_with("connect.rs") {
            continue;
        }
        for line in code_lines(text) {
            for banned in ["connect_to_env", "from_socket", "UnixStream", "from_fd"] {
                assert!(!line.contains(banned), "{name} opens a connection: {line}");
            }
        }
    }
}

/// No binary or example calls `run_lock` without a policy, builds the
/// shell-launched policy itself, or lets a probe pick anything but a nest.
#[test]
fn b_every_run_lock_call_passes_a_policy_and_the_probes_are_nest_only() {
    let mut calls = 0;
    for (path, text) in client_sources() {
        let name = file_name(&path);
        for line in text.lines().map(str::trim).filter(|l| !l.starts_with("//")) {
            if line.contains("ConnectPolicy::ShellLaunched") {
                assert_eq!(name, "connect.rs", "{}: {line}", path.display());
            }
            if line.contains("run_lock(") && !line.contains("fn run_lock") {
                calls += 1;
                assert!(
                    line.contains("run_lock(ConnectPolicy::detect(true),")
                        || line.contains("run_lock(run.connect,"),
                    "{}: {line}",
                    path.display()
                );
            }
            if line.contains("ConnectPolicy::detect(") && !line.contains("fn detect") {
                // The production binary alone decides from `--rehearsal`; the
                // probes and examples are nest-only.
                let ok = line.contains("detect(true)")
                    || (name == "main.rs" && line.contains("detect(rehearsal)"));
                assert!(ok, "{}: {line}", path.display());
            }
        }
    }
    // celestina-lock's app, the two examples.
    assert_eq!(calls, 3, "run_lock call sites");
}

/// The policy has no `Default`: a caller states how it was launched.
#[test]
fn c_connect_policy_has_no_default() {
    let (_, text) = lock_sources()
        .into_iter()
        .find(|(name, _)| name.ends_with("connect.rs"))
        .expect("connect.rs");
    let at = text.find("pub enum ConnectPolicy").expect("the enum");
    let attributes = &text[at.saturating_sub(120)..at];
    assert!(!attributes.contains("Default"), "{attributes}");
    assert!(!text.contains("impl Default for ConnectPolicy"));
}
