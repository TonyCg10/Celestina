//! The real `celestina-lock-verify` binary (design §9.1 T-V1, T-V8, T-H).
//!
//! Needs no root and writes nothing under `/etc`. It never starts the
//! production service `celestina-lock`: the only service that reaches
//! `libpam` here is `celestina-lock-rehearsal`, which has no file, so PAM falls
//! to `other` (`pam_deny`) and denies without `pam_faillock` and without a
//! password check. The secret sent is a made-up test value, never a real one.
//! Each such run first checks that precondition read-only and skips otherwise.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_celestina-lock-verify");

/// A value no account has; never a real password.
const TEST_VALUE: &[u8] = b"celestina-test-value-not-a-password";

fn run(args: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(BIN)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the verifier");
    let mut pipe = child.stdin.take().expect("stdin");
    // The verifier may answer without reading (a bad request): a broken pipe
    // is not a failure of the test.
    let _ = pipe.write_all(stdin);
    drop(pipe);
    child.wait_with_output().expect("wait")
}

fn code(output: &Output) -> Option<i32> {
    output.status.code()
}

fn assert_silent(output: &Output) {
    assert!(output.stdout.is_empty(), "stdout: {:?}", output.stdout);
    assert!(output.stderr.is_empty(), "stderr: {:?}", output.stderr);
}

const REHEARSAL: [&str; 4] = ["--protocol", "1", "--service", "celestina-lock-rehearsal"];

#[test]
fn a_bad_protocol_answers_6_and_writes_nothing() {
    for args in [
        &["--protocol", "2", "--service", "celestina-lock-rehearsal"][..],
        &["--protocol", "1", "--service", "login"],
        &["--protocol", "1", "--service", "other"],
        &["--protocol", "1"],
        &[],
    ] {
        let output = run(args, TEST_VALUE);
        assert_eq!(code(&output), Some(6), "{args:?}");
        assert_silent(&output);
    }
}

#[test]
fn secret_bounds_answer_6_before_pam() {
    let oversized = vec![b'x'; 513];
    for input in [&b""[..], b"ab\0cd", &oversized] {
        let output = run(&REHEARSAL, input);
        assert_eq!(code(&output), Some(6), "{} bytes", input.len());
        assert_silent(&output);
    }
}

fn exists(path: &str) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// `other` is deny-only on this machine: every active line uses `pam_deny.so`
/// or `pam_warn.so`. Read-only.
fn other_is_deny_only() -> bool {
    ["/etc/pam.d/other", "/usr/lib/pam.d/other"]
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
        .is_some_and(|text| {
            let mut lines = text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .peekable();
            lines.peek().is_some()
                && lines.all(|l| l.contains("pam_deny.so") || l.contains("pam_warn.so"))
        })
}

/// The faillock tally of the user, read-only: (length, modified time), if any.
fn tally() -> Option<(u64, std::time::SystemTime)> {
    let user = std::env::var("USER").ok()?;
    let meta = std::fs::metadata(Path::new("/run/faillock").join(user)).ok()?;
    Some((meta.len(), meta.modified().ok()?))
}

#[test]
fn t_v8_rehearsal_service_with_no_such_file_answers_1_via_other() {
    if exists("/etc/pam.d/celestina-lock-rehearsal")
        || exists("/usr/lib/pam.d/celestina-lock-rehearsal")
    {
        // With the file present the verifier must answer 6, and that is the
        // other half of this test.
        let output = run(&REHEARSAL, TEST_VALUE);
        assert_eq!(code(&output), Some(6));
        assert_silent(&output);
        return;
    }
    if !other_is_deny_only() {
        eprintln!("skipped: the `other` PAM service is not deny-only on this machine");
        return;
    }
    let before = tally();
    let output = run(&REHEARSAL, TEST_VALUE);
    assert_eq!(code(&output), Some(1), "stack denied through `other`");
    assert_silent(&output);
    assert_eq!(tally(), before, "the faillock tally was touched");
}

#[test]
fn the_binary_links_only_libpam_libc_and_their_dependencies() {
    let Ok(ldd) = Command::new("ldd").arg(BIN).output() else {
        eprintln!("skipped: ldd is not available");
        return;
    };
    assert!(ldd.status.success(), "ldd failed");
    let listing = String::from_utf8_lossy(&ldd.stdout);
    let linked = names(&listing);
    assert!(linked.contains("libpam.so.0"), "{linked:?}");
    // What libpam itself needs, plus the C runtime Rust's std uses. libpam's
    // path is the one the loader resolved for the binary, because the
    // directory differs between distributions.
    let pam_path = resolved_path(&listing, "libpam.so.0")
        .unwrap_or_else(|| panic!("no resolved libpam.so.0 path in ldd output:\n{listing}"));
    let pam = Command::new("ldd")
        .arg(pam_path)
        .output()
        .expect("ldd libpam");
    assert!(pam.status.success(), "ldd {pam_path} failed");
    let mut allowed = names(&String::from_utf8_lossy(&pam.stdout));
    for extra in ["libpam.so.0", "libgcc_s.so.1", "libm.so.6", "libc.so.6"] {
        allowed.insert(extra.to_owned());
    }
    let unexpected: Vec<&String> = linked.difference(&allowed).collect();
    assert!(
        unexpected.is_empty(),
        "unexpected libraries: {unexpected:?}"
    );
}

/// The path `ldd` resolved for `library`, from a line of the form
/// `libpam.so.0 => /path/to/libpam.so.0 (0x...)`.
fn resolved_path<'a>(text: &'a str, library: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let (name, rest) = line.split_once("=>")?;
        if name.trim() != library {
            return None;
        }
        rest.split_whitespace()
            .next()
            .filter(|path| path.starts_with('/'))
    })
}

/// The library names of `ldd` output, without paths and addresses; the
/// dynamic loader and the vdso are not libraries to compare.
fn names(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(|word| word.rsplit('/').next().unwrap_or(word).to_owned())
        .filter(|name| !name.starts_with("linux-vdso") && !name.starts_with("ld-linux"))
        .collect()
}
