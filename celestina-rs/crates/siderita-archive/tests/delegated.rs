//! A delegated extraction, end to end through [`extract`], with a stand-in `7z`
//! found on `PATH`.
//!
//! Kept apart from `archives.rs` because these tests replace the process's
//! `PATH`, which the other binary's tests read through `can_read`; a test
//! binary is its own process, and within this one the tests take turns.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use celestina_core::CancellationToken;
use siderita_archive::{extract, ArchiveError, ExtractOptions, Utc};
use siderita_ops::Progress;

/// `PATH` is process-wide: one test at a time owns it.
static PATH_OWNER: Mutex<()> = Mutex::new(());

/// A throwaway directory in the system temp dir, removed on drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "siderita-delegated-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A stand-in `7z` in `dir/bin`. `l` prints `dir/bin/listing`; `x` marks that
/// it ran and runs `dir/bin/payload` with the output folder, which is where
/// the "tool" does whatever the archive would make it do.
fn install_tool(dir: &Path, listing: &str, payload: &str) -> PathBuf {
    let bin = dir.join("bin");
    fs::create_dir(&bin).expect("mk bin");
    let program = bin.join("7z");
    fs::write(
        &program,
        "#!/bin/sh\n\
         here=$(dirname \"$0\")\n\
         case \"$1\" in\n\
         l) cat \"$here/listing\" ;;\n\
         x) : > \"$here/extracted\"\n\
            for a in \"$@\"; do case \"$a\" in -o*) out=${a#-o} ;; esac; done\n\
            sh \"$here/payload\" \"$out\" ;;\n\
         esac\n",
    )
    .expect("write tool");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("chmod tool");
    fs::write(bin.join("listing"), listing).expect("write listing");
    fs::write(bin.join("payload"), payload).expect("write payload");
    bin
}

/// A file that `sniff` reads as a 7z.
fn seven_zip(dir: &Path) -> PathBuf {
    let archive = dir.join("evil.7z");
    let mut bytes = b"7z\xbc\xaf\x27\x1c".to_vec();
    bytes.resize(64, 0);
    fs::write(&archive, bytes).expect("write archive");
    archive
}

/// A `7z l -slt` listing of `members`, each a block of `Key = value` lines.
fn listing(members: &[&[&str]]) -> String {
    let mut text = String::from("--\nPath = evil.7z\nType = 7z\n\n----------\n");
    for member in members {
        for line in *member {
            text.push_str(line);
            text.push('\n');
        }
        text.push('\n');
    }
    text
}

/// Runs `extract` with `bin` first on `PATH`, the system folders after it for
/// the shell the stand-in needs.
fn extract_with(
    bin: &Path,
    archive: &Path,
    into: &Path,
) -> Result<siderita_archive::Extracted, ArchiveError> {
    extract_cancellable(bin, archive, into, &CancellationToken::new())
}

/// [`extract_with`], cancelled through `token`.
fn extract_cancellable(
    bin: &Path,
    archive: &Path,
    into: &Path,
    token: &CancellationToken,
) -> Result<siderita_archive::Extracted, ArchiveError> {
    let search =
        std::env::join_paths([bin, Path::new("/usr/bin"), Path::new("/bin")]).expect("join PATH");
    std::env::set_var("PATH", search);
    extract(
        archive,
        into,
        &ExtractOptions::new(&Utc, "extracted"),
        token,
        &mut |_: Progress| {},
    )
}

/// Whether process `pid` is gone, or only a zombie nobody has reaped yet.
fn gone(pid: &str) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(_) => true,
        Ok(stat) => {
            let after = &stat[stat.rfind(')').expect("comm field") + 1..];
            after.split_whitespace().next() == Some("Z")
        }
    }
}

/// Waits up to `limit` for process `pid` to be gone.
fn gone_within(pid: &str, limit: Duration) -> bool {
    let until = Instant::now() + limit;
    while !gone(pid) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    gone(pid)
}

/// Kills `pid` so a failed assertion leaves nothing running behind.
fn reap_leftover(pid: &str) {
    let _ = std::process::Command::new("kill")
        .arg("-KILL")
        .arg(pid)
        .status();
}

/// Whether `setsid` is on the test's `PATH`, which the group rules need.
fn has_setsid() -> bool {
    ["/usr/bin/setsid", "/bin/setsid"]
        .iter()
        .any(|path| Path::new(path).exists())
}

/// Round 3 of the review (M2, case A): a tool that exits normally but leaves
/// a child holding its output does not keep the extraction waiting, and a
/// cancel is honoured promptly; the child is stopped with the tool's group.
#[test]
fn a_child_left_holding_the_output_neither_blocks_nor_survives() {
    let _owner = PATH_OWNER
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if !has_setsid() {
        eprintln!("skipped: setsid is not installed on this machine");
        return;
    }
    let dir = TestDir::new("orphan-output");
    let bin = install_tool(
        &dir.0,
        &listing(&[&["Path = notes.txt", "Size = 3"]]),
        "cd \"$1\" && printf one > notes.txt\nsleep 8 &\necho $! > \"$(dirname \"$0\")/orphan\"\nexit 0\n",
    );
    let archive = seven_zip(&dir.0);
    let into = dir.0.join("destination");
    fs::create_dir(&into).expect("mk destination");

    let token = CancellationToken::new();
    let canceller = {
        let token = token.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(1));
            token.cancel();
        })
    };
    let started = Instant::now();
    let _ = extract_cancellable(&bin, &archive, &into, &token);
    let took = started.elapsed();
    canceller.join().expect("canceller");

    let orphan = fs::read_to_string(bin.join("orphan")).expect("orphan recorded");
    let orphan = orphan.trim().to_owned();
    let stopped = gone_within(&orphan, Duration::from_secs(3));
    if !stopped {
        reap_leftover(&orphan);
    }
    assert!(took < Duration::from_secs(3), "extract took {took:?}");
    assert!(stopped, "the tool's child {orphan} is still running");
}

/// Round 3 of the review (M2, case A'): a child that left the tool's group
/// still holds the output; the group rule cannot reach it, and a cancel must
/// still end the extraction promptly.
#[test]
fn a_cancel_is_honoured_while_an_escaped_child_holds_the_output() {
    let _owner = PATH_OWNER
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if !has_setsid() {
        eprintln!("skipped: setsid is not installed on this machine");
        return;
    }
    let dir = TestDir::new("escaped-output");
    let bin = install_tool(
        &dir.0,
        &listing(&[&["Path = notes.txt", "Size = 3"]]),
        "cd \"$1\" && printf one > notes.txt\nsetsid sleep 8 &\necho $! > \"$(dirname \"$0\")/orphan\"\nexit 0\n",
    );
    let archive = seven_zip(&dir.0);
    let into = dir.0.join("destination");
    fs::create_dir(&into).expect("mk destination");

    let token = CancellationToken::new();
    let canceller = {
        let token = token.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(1));
            token.cancel();
        })
    };
    let started = Instant::now();
    let outcome = extract_cancellable(&bin, &archive, &into, &token);
    let took = started.elapsed();
    canceller.join().expect("canceller");
    let orphan = fs::read_to_string(bin.join("orphan")).expect("orphan recorded");
    reap_leftover(orphan.trim());

    assert!(took < Duration::from_secs(3), "extract took {took:?}");
    assert!(
        outcome.as_ref().is_err_and(ArchiveError::is_cancelled),
        "{outcome:?}"
    );
    assert_eq!(fs::read_dir(&into).expect("read destination").count(), 0);
}

/// Round 3 of the review (M2, case B): a child the tool left behind, with no
/// hold on its output, that writes later: it is stopped with the tool's
/// group, so nothing lands in the extraction after `extract` has checked and
/// returned it.
#[test]
fn a_child_left_behind_writes_nothing_after_extract_returns() {
    let _owner = PATH_OWNER
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if !has_setsid() {
        eprintln!("skipped: setsid is not installed on this machine");
        return;
    }
    let dir = TestDir::new("orphan-late");
    let bin = install_tool(
        &dir.0,
        &listing(&[&["Path = notes.txt", "Size = 3"]]),
        "cd \"$1\" && printf one > notes.txt\n(sleep 2; ln -s /etc late) </dev/null >/dev/null 2>&1 &\nexit 0\n",
    );
    let archive = seven_zip(&dir.0);
    let into = dir.0.join("destination");
    fs::create_dir(&into).expect("mk destination");

    let extracted = extract_with(&bin, &archive, &into).expect("honest archive");
    std::thread::sleep(Duration::from_secs(3));

    assert!(
        fs::symlink_metadata(extracted.root.join("late")).is_err(),
        "a child of the tool wrote after extract returned"
    );
}

/// Important 2 of the review: the auditor's chain, handed to a tool, is
/// refused from the tool's own listing before the tool writes anything.
#[test]
fn the_chain_is_refused_from_the_listing_before_the_tool_writes() {
    let _owner = PATH_OWNER
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let dir = TestDir::new("listing");
    let bin = install_tool(
        &dir.0,
        &listing(&[
            &["Path = d", "Folder = +"],
            &["Path = d/up", "Symbolic Link = .."],
            &["Path = esc", "Symbolic Link = d/up/.."],
            &["Path = esc/pwned.txt", "Size = 5"],
        ]),
        "cd \"$1\" && mkdir d && ln -s .. d/up && ln -s d/up/.. esc && echo pwned > esc/pwned.txt\n",
    );
    let archive = seven_zip(&dir.0);
    let into = dir.0.join("destination");
    fs::create_dir(&into).expect("mk destination");

    let outcome = extract_with(&bin, &archive, &into);

    assert!(
        matches!(outcome, Err(ArchiveError::UnsafeMember { .. })),
        "{outcome:?}"
    );
    assert!(
        !bin.join("extracted").exists(),
        "the tool was allowed to write"
    );
    assert!(!into.join("pwned.txt").exists());
    assert_eq!(fs::read_dir(&into).expect("read destination").count(), 0);
}

/// The tool-path escape: a tool whose listing looked honest but which lays the
/// chain down anyway is caught by the check of its finished tree, and the
/// whole result is removed.
#[test]
fn a_tool_that_lays_an_escaping_chain_anyway_is_refused_afterwards() {
    let _owner = PATH_OWNER
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let dir = TestDir::new("afterwards");
    let bin = install_tool(
        &dir.0,
        &listing(&[&["Path = notes.txt", "Size = 3"]]),
        "cd \"$1\" && mkdir d && ln -s .. d/up && ln -s d/up/.. esc\n",
    );
    let archive = seven_zip(&dir.0);
    let into = dir.0.join("destination");
    fs::create_dir(&into).expect("mk destination");

    let outcome = extract_with(&bin, &archive, &into);

    assert!(
        matches!(outcome, Err(ArchiveError::UnsafeMember { .. })),
        "{outcome:?}"
    );
    assert!(bin.join("extracted").exists(), "the tool should have run");
    assert_eq!(fs::read_dir(&into).expect("read destination").count(), 0);
}

/// The honest counterpart: a clean listing lets the tool run and its result
/// is kept.
#[test]
fn an_honest_delegated_archive_is_extracted() {
    let _owner = PATH_OWNER
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let dir = TestDir::new("honest");
    let bin = install_tool(
        &dir.0,
        &listing(&[&["Path = notes.txt", "Size = 3"]]),
        "cd \"$1\" && printf one > notes.txt\n",
    );
    let archive = seven_zip(&dir.0);
    let into = dir.0.join("destination");
    fs::create_dir(&into).expect("mk destination");

    let extracted = extract_with(&bin, &archive, &into).expect("honest archive");

    assert_eq!(
        fs::read(extracted.root.join("notes.txt")).expect("extracted file"),
        b"one"
    );
}

/// Round 4 of the review: an escaped child that floods the tool's output must
/// not starve the cancellation check.
#[test]
fn a_cancel_is_honoured_while_an_escaped_child_floods_the_output() {
    let _owner = PATH_OWNER
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if !has_setsid() {
        eprintln!("skipped: setsid is not installed on this machine");
        return;
    }
    let dir = TestDir::new("flood");
    let bin = install_tool(
        &dir.0,
        &listing(&[&["Path = a.txt", "Size = 2"]]),
        "cd \"$1\" && echo a > a.txt\nsetsid timeout 8 yes &\nexit 0\n",
    );
    let archive = seven_zip(&dir.0);
    let into = dir.0.join("destination");
    fs::create_dir(&into).expect("mk destination");

    let token = CancellationToken::new();
    let canceller = {
        let token = token.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(1));
            token.cancel();
        })
    };
    let started = Instant::now();
    let outcome = extract_cancellable(&bin, &archive, &into, &token);
    let took = started.elapsed();
    canceller.join().expect("canceller");

    assert!(took < Duration::from_secs(3), "extract took {took:?}");
    assert!(
        outcome.as_ref().is_err_and(ArchiveError::is_cancelled),
        "{outcome:?}"
    );
}
