use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use calcita_core::recent::{Recent, RecentStore, CAPACITY};
use calcita_core::zoom::ZoomMode;

fn recent(path: &str, seconds: u64) -> Recent {
    Recent {
        path: PathBuf::from(path),
        page: 1,
        zoom: ZoomMode::FitWidth,
        opened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(seconds),
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("calcita-core-recent-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("recent")
}

#[test]
fn touch_puts_the_latest_first_and_keeps_one_entry_per_path() {
    let mut store = RecentStore::default();
    store.touch(recent("/a.pdf", 1));
    store.touch(recent("/b.pdf", 2));
    store.touch(recent("/a.pdf", 3));
    let paths: Vec<_> = store.entries().iter().map(|r| r.path.clone()).collect();
    assert_eq!(
        paths,
        vec![PathBuf::from("/a.pdf"), PathBuf::from("/b.pdf")]
    );
}

#[test]
fn the_store_keeps_fifty() {
    let mut store = RecentStore::default();
    for index in 0..60 {
        store.touch(recent(&format!("/{index}.pdf"), index));
    }
    assert_eq!(CAPACITY, 50);
    assert_eq!(store.entries().len(), 50);
    assert_eq!(store.entries()[0].path, PathBuf::from("/59.pdf"));
}

#[test]
fn a_non_utf8_path_round_trips_through_the_file() {
    let file = scratch("non-utf8");
    let path = PathBuf::from(OsStr::from_bytes(b"/tmp/informe-\xff.pdf"));
    let mut store = RecentStore::at(file.clone());
    store.touch(Recent {
        path: path.clone(),
        page: 7,
        zoom: ZoomMode::Free(1.5),
        opened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(42),
    });
    store.save().expect("the store saves");
    let loaded = RecentStore::load_from(file.clone()).expect("the store loads");
    assert_eq!(loaded.entries().len(), 1);
    let entry = &loaded.entries()[0];
    assert_eq!(entry.path, path);
    assert_eq!(entry.page, 7);
    assert_eq!(entry.zoom, ZoomMode::Free(1.5));
    assert_eq!(
        entry.opened_at,
        SystemTime::UNIX_EPOCH + Duration::from_secs(42)
    );
    let _ = std::fs::remove_dir_all(file.parent().expect("a parent"));
}

#[test]
fn a_missing_file_is_an_empty_store_and_bad_lines_are_skipped() {
    let file = scratch("bad-lines");
    assert!(RecentStore::load_from(file.clone())
        .expect("loads")
        .entries()
        .is_empty());
    std::fs::create_dir_all(file.parent().expect("a parent")).expect("dir");
    std::fs::write(&file, "garbage\n%2Fok.pdf\t2\twidth\t5\n").expect("write");
    let loaded = RecentStore::load_from(file.clone()).expect("loads");
    assert_eq!(loaded.entries().len(), 1);
    assert_eq!(
        loaded
            .reading_for(&PathBuf::from("/ok.pdf"))
            .map(|r| r.page),
        Some(2)
    );
    let _ = std::fs::remove_dir_all(file.parent().expect("a parent"));
}
