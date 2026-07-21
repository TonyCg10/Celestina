use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use celestina_core::CancellationToken;
use siderita_ops::{copy, create_directory, create_file, move_entry, rename, OpError, Progress};

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

// ── copy ─────────────────────────────────────────────────────────────────

#[test]
fn copy_duplicates_a_file_without_touching_the_source() {
    let dir = TestDir::new("copy-file");
    let source = dir.path().join("orig.txt");
    let into = dir.path().join("dest");
    fs::create_dir(&into).expect("mk dest");
    fs::write(&source, b"hello").expect("seed");

    let made = copy(&source, &into, &live(), &mut |_| {}).expect("copy");
    assert_eq!(made, into.join("orig.txt"));
    assert_eq!(fs::read(&made).expect("read copy"), b"hello");
    assert_eq!(fs::read(&source).expect("source intact"), b"hello");
}

#[test]
fn copy_recurses_a_directory_tree() {
    let dir = TestDir::new("copy-tree");
    let source = dir.path().join("tree");
    fs::create_dir(&source).expect("mk tree");
    fs::create_dir(source.join("sub")).expect("mk sub");
    fs::write(source.join("top.txt"), b"top").expect("seed top");
    fs::write(source.join("sub/leaf.txt"), b"leaf").expect("seed leaf");
    let into = dir.path().join("into");
    fs::create_dir(&into).expect("mk into");

    let made = copy(&source, &into, &live(), &mut |_| {}).expect("copy tree");
    assert_eq!(made, into.join("tree"));
    assert_eq!(fs::read(made.join("top.txt")).expect("read top"), b"top");
    assert_eq!(
        fs::read(made.join("sub/leaf.txt")).expect("read leaf"),
        b"leaf"
    );
    assert!(source.join("sub/leaf.txt").exists(), "source tree intact");
}

#[test]
fn copy_refuses_to_overwrite_an_existing_destination() {
    let dir = TestDir::new("copy-conflict");
    let source = dir.path().join("f");
    let into = dir.path().join("into");
    fs::create_dir(&into).expect("mk into");
    fs::write(&source, b"new").expect("seed source");
    fs::write(into.join("f"), b"existing").expect("seed existing");

    let error = copy(&source, &into, &live(), &mut |_| {}).expect_err("must refuse");
    assert!(matches!(error, OpError::AlreadyExists { .. }));
    assert_eq!(
        fs::read(into.join("f")).expect("existing intact"),
        b"existing"
    );
}

#[test]
fn copy_refuses_a_destination_inside_the_source() {
    let dir = TestDir::new("copy-inside");
    let source = dir.path().join("box");
    fs::create_dir(&source).expect("mk box");
    let inside = source.join("inner");
    fs::create_dir(&inside).expect("mk inner");

    let error = copy(&source, &inside, &live(), &mut |_| {}).expect_err("must refuse");
    assert!(matches!(error, OpError::DestinationInsideSource { .. }));
}

#[test]
fn copy_reports_cumulative_progress() {
    let dir = TestDir::new("copy-progress");
    let source = dir.path().join("data");
    let into = dir.path().join("into");
    fs::create_dir(&into).expect("mk into");
    fs::write(&source, vec![0u8; 5000]).expect("seed");

    let mut last = Progress::default();
    copy(&source, &into, &live(), &mut |report| last = report).expect("copy");
    assert_eq!(last.bytes, 5000);
    assert_eq!(last.items, 1);
}

#[test]
fn a_cancelled_copy_creates_no_destination() {
    let dir = TestDir::new("copy-cancel");
    let source = dir.path().join("src.txt");
    let into = dir.path().join("into");
    fs::create_dir(&into).expect("mk into");
    fs::write(&source, b"data").expect("seed");

    let token = CancellationToken::new();
    token.cancel();
    let error = copy(&source, &into, &token, &mut |_| {}).expect_err("cancelled");
    assert!(matches!(error, OpError::Cancelled));
    assert!(!into.join("src.txt").exists());
}

#[cfg(unix)]
#[test]
fn copy_preserves_a_symlink_as_a_link() {
    let dir = TestDir::new("copy-symlink");
    let target = dir.path().join("target.txt");
    fs::write(&target, b"t").expect("seed target");
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link).expect("mk symlink");
    let into = dir.path().join("into");
    fs::create_dir(&into).expect("mk into");

    let made = copy(&link, &into, &live(), &mut |_| {}).expect("copy symlink");
    let meta = fs::symlink_metadata(&made).expect("stat copy");
    assert!(meta.file_type().is_symlink(), "copy must remain a symlink");
    assert_eq!(fs::read_link(&made).expect("read link"), target);
}

// ── move ─────────────────────────────────────────────────────────────────

#[test]
fn move_entry_relocates_a_file_on_the_same_filesystem() {
    let dir = TestDir::new("move-file");
    let source = dir.path().join("m.txt");
    let into = dir.path().join("into");
    fs::create_dir(&into).expect("mk into");
    fs::write(&source, b"move me").expect("seed");

    let moved = move_entry(&source, &into, &live(), &mut |_| {}).expect("move");
    assert_eq!(moved.to, into.join("m.txt"));
    assert!(!source.exists());
    assert_eq!(
        fs::read(into.join("m.txt")).expect("read moved"),
        b"move me"
    );
}

#[test]
fn move_entry_relocates_a_directory_tree() {
    let dir = TestDir::new("move-tree");
    let source = dir.path().join("d");
    fs::create_dir(&source).expect("mk d");
    fs::write(source.join("f.txt"), b"x").expect("seed");
    let into = dir.path().join("into");
    fs::create_dir(&into).expect("mk into");

    move_entry(&source, &into, &live(), &mut |_| {}).expect("move dir");
    assert!(!source.exists());
    assert_eq!(fs::read(into.join("d/f.txt")).expect("read"), b"x");
}

#[test]
fn move_entry_refuses_to_overwrite_and_keeps_the_source() {
    let dir = TestDir::new("move-conflict");
    let source = dir.path().join("s");
    let into = dir.path().join("into");
    fs::create_dir(&into).expect("mk into");
    fs::write(&source, b"src").expect("seed src");
    fs::write(into.join("s"), b"dst").expect("seed dst");

    let error = move_entry(&source, &into, &live(), &mut |_| {}).expect_err("must refuse");
    assert!(matches!(error, OpError::AlreadyExists { .. }));
    assert_eq!(fs::read(&source).expect("source kept"), b"src");
    assert_eq!(fs::read(into.join("s")).expect("dst intact"), b"dst");
}

#[test]
fn move_entry_reports_a_missing_source() {
    let dir = TestDir::new("move-missing");
    let into = dir.path().join("into");
    fs::create_dir(&into).expect("mk into");

    let error =
        move_entry(&dir.path().join("ghost"), &into, &live(), &mut |_| {}).expect_err("must fail");
    assert!(matches!(error, OpError::SourceMissing { .. }));
}

#[test]
fn a_cancelled_move_does_nothing() {
    let dir = TestDir::new("move-cancel");
    let source = dir.path().join("s");
    let into = dir.path().join("into");
    fs::create_dir(&into).expect("mk into");
    fs::write(&source, b"keep").expect("seed");

    let token = CancellationToken::new();
    token.cancel();
    let error = move_entry(&source, &into, &token, &mut |_| {}).expect_err("cancelled");
    assert!(matches!(error, OpError::Cancelled));
    assert!(source.exists(), "source untouched");
    assert!(!into.join("s").exists());
}
