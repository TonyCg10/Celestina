use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use calcita_core::recent::{Recent, RecentStore, CAPACITY};
use calcita_core::zoom::ZoomMode;

fn recent(path: &str, seconds: u64) -> Recent {
    Recent {
        path: PathBuf::from(path),
        page: 1,
        zoom: ZoomMode::FitWidth,
        opened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(seconds),
        dark: false,
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
        dark: false,
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

#[test]
fn the_reading_mode_is_remembered() {
    let file = scratch("dark");
    let mut store = RecentStore::at(file.clone());
    store.touch(Recent {
        dark: true,
        ..recent("/tmp/noche.pdf", 5)
    });
    store.touch(recent("/tmp/dia.pdf", 6));
    store.save().expect("the store saves");
    let loaded = RecentStore::load_from(file.clone()).expect("the store loads");
    let night = loaded
        .reading_for(Path::new("/tmp/noche.pdf"))
        .expect("remembered");
    assert!(night.dark);
    let day = loaded
        .reading_for(Path::new("/tmp/dia.pdf"))
        .expect("remembered");
    assert!(!day.dark);
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_line_without_the_reading_mode_reads_as_light() {
    let file = scratch("four-fields");
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).expect("the scratch folder");
    }
    std::fs::write(
        &file,
        "/tmp/viejo.pdf\t3\twidth\t9\n/tmp/raro.pdf\t3\twidth\t9\tsepia\n",
    )
    .expect("the store is written");
    let loaded = RecentStore::load_from(file.clone()).expect("the store loads");
    assert_eq!(
        loaded.entries().len(),
        1,
        "an unknown mode is a damaged line"
    );
    let old = loaded
        .reading_for(Path::new("/tmp/viejo.pdf"))
        .expect("remembered");
    assert_eq!(old.page, 3);
    assert!(!old.dark);
    let _ = std::fs::remove_file(file);
}

#[test]
fn a_document_can_be_forgotten() {
    let mut store = RecentStore::default();
    store.touch(recent("/tmp/a.pdf", 1));
    store.touch(recent("/tmp/b.pdf", 2));
    assert!(store.forget(Path::new("/tmp/a.pdf")));
    assert!(!store.forget(Path::new("/tmp/a.pdf")));
    assert_eq!(store.entries().len(), 1);
    assert_eq!(store.entries()[0].path, PathBuf::from("/tmp/b.pdf"));
}
