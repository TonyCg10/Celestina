use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use celestina_core::CancellationToken;
use siderita_ops::{create_directory, create_file, rename, OpError};

/// A throwaway directory in the system temp dir, removed on drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "siderita-ops-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn live() -> CancellationToken {
    CancellationToken::new()
}

#[test]
fn create_directory_makes_a_new_folder() {
    let dir = TestDir::new("mkdir");
    let made = create_directory(dir.path(), OsStr::new("nueva carpeta"), &live())
        .expect("create directory");
    assert_eq!(made, dir.path().join("nueva carpeta"));
    assert!(made.is_dir());
}

#[test]
fn create_directory_refuses_to_overwrite() {
    let dir = TestDir::new("mkdir-conflict");
    fs::create_dir(dir.path().join("dup")).expect("seed dir");
    let error = create_directory(dir.path(), OsStr::new("dup"), &live()).expect_err("must refuse");
    assert!(matches!(error, OpError::AlreadyExists { .. }));
}

#[test]
fn create_rejects_reserved_and_separator_names() {
    let dir = TestDir::new("mkdir-invalid");
    for bad in ["..", ".", "a/b", ""] {
        let error =
            create_directory(dir.path(), OsStr::new(bad), &live()).expect_err("must reject");
        assert!(matches!(error, OpError::InvalidName(_)), "for {bad:?}");
    }
}

#[test]
fn create_file_makes_an_empty_file_and_never_truncates() {
    let dir = TestDir::new("touch");
    let made = create_file(dir.path(), OsStr::new("nota.txt"), &live()).expect("create file");
    assert!(made.is_file());
    assert_eq!(fs::read(&made).expect("read new file").len(), 0);

    // A second create must report the collision and leave the content intact.
    fs::write(&made, b"keep me").expect("write content");
    let error = create_file(dir.path(), OsStr::new("nota.txt"), &live()).expect_err("must refuse");
    assert!(matches!(error, OpError::AlreadyExists { .. }));
    assert_eq!(fs::read(&made).expect("read after refusal"), b"keep me");
}

#[test]
fn rename_moves_within_the_parent() {
    let dir = TestDir::new("rename");
    let from = dir.path().join("old");
    fs::write(&from, b"data").expect("seed");
    let renamed = rename(&from, OsStr::new("new"), &live()).expect("rename");
    assert_eq!(renamed.from, from);
    assert_eq!(renamed.to, dir.path().join("new"));
    assert!(!from.exists());
    assert_eq!(
        fs::read(dir.path().join("new")).expect("read moved"),
        b"data"
    );
}

#[test]
fn rename_refuses_to_clobber_and_leaves_both_intact() {
    let dir = TestDir::new("rename-conflict");
    let from = dir.path().join("a");
    let occupied = dir.path().join("b");
    fs::write(&from, b"a").expect("seed a");
    fs::write(&occupied, b"b").expect("seed b");

    let error = rename(&from, OsStr::new("b"), &live()).expect_err("must refuse");
    assert!(matches!(error, OpError::AlreadyExists { .. }));
    assert_eq!(fs::read(&from).expect("read a"), b"a");
    assert_eq!(fs::read(&occupied).expect("read b"), b"b");
}

#[test]
fn rename_reports_a_missing_source() {
    let dir = TestDir::new("rename-missing");
    let ghost = dir.path().join("ghost");
    let error = rename(&ghost, OsStr::new("whatever"), &live()).expect_err("must fail");
    assert!(matches!(error, OpError::SourceMissing { .. }));
}

#[test]
fn rename_to_the_same_name_is_a_noop() {
    let dir = TestDir::new("rename-noop");
    let path = dir.path().join("same");
    fs::write(&path, b"x").expect("seed");
    let renamed = rename(&path, OsStr::new("same"), &live()).expect("noop rename");
    assert_eq!(renamed.to, path);
    assert!(path.exists());
}

#[test]
fn a_cancelled_token_stops_before_touching_the_filesystem() {
    let dir = TestDir::new("cancelled");
    let token = CancellationToken::new();
    token.cancel();
    let error = create_directory(dir.path(), OsStr::new("nope"), &token).expect_err("cancelled");
    assert!(matches!(error, OpError::Cancelled));
    assert!(!dir.path().join("nope").exists());
}

#[cfg(unix)]
#[test]
fn create_and_rename_preserve_non_utf8_names() {
    use std::os::unix::ffi::OsStrExt;

    let dir = TestDir::new("non-utf8");
    let raw = OsStr::from_bytes(b"na\xffme");
    let made = create_file(dir.path(), raw, &live()).expect("create non-utf8");
    assert_eq!(made.file_name(), Some(raw));

    let raw2 = OsStr::from_bytes(b"re\xfen");
    let renamed = rename(&made, raw2, &live()).expect("rename non-utf8");
    assert_eq!(renamed.to.file_name(), Some(raw2));
    assert!(renamed.to.exists());
}
