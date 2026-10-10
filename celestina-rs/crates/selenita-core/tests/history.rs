use selenita_core::history::CAPACITY;
use selenita_core::{Entry, EntryKind, History};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("selenita-history-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn entry(path: PathBuf, seconds: u64) -> Entry {
    Entry {
        path,
        kind: EntryKind::Screenshot,
        taken_at: SystemTime::UNIX_EPOCH + Duration::from_secs(seconds),
        size: 67,
    }
}

#[test]
fn a_saved_history_loads_back_latest_first() {
    let dir = scratch("roundtrip");
    let file = dir.join("history");
    let shots: Vec<PathBuf> = (0..3)
        .map(|i| dir.join(format!("Captura {i}.png")))
        .collect();
    let mut history = History::at(file.clone());
    for (i, shot) in shots.iter().enumerate() {
        std::fs::write(shot, b"png").expect("shot");
        history.push(entry(shot.clone(), 1_000 + i as u64));
    }
    history.save().expect("save");

    let loaded = History::load(file).expect("load");
    let paths: Vec<_> = loaded.entries().iter().map(|e| e.path.clone()).collect();
    assert_eq!(
        paths,
        vec![shots[2].clone(), shots[1].clone(), shots[0].clone()]
    );
    assert_eq!(loaded.entries()[0], entry(shots[2].clone(), 1_002));
    assert_eq!(
        loaded.find(&loaded.entries()[1].id()).map(|e| &e.path),
        Some(&shots[1])
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_entry_whose_file_vanished_is_pruned() {
    let dir = scratch("prune");
    let kept = dir.join("kept.png");
    let gone = dir.join("gone.png");
    std::fs::write(&kept, b"png").expect("kept");
    std::fs::write(&gone, b"png").expect("gone");
    let mut history = History::at(dir.join("history"));
    history.push(entry(kept.clone(), 1));
    history.push(entry(gone.clone(), 2));
    history.save().expect("save");

    std::fs::remove_file(&gone).expect("vanish");
    let loaded = History::load(dir.join("history")).expect("load");
    assert_eq!(loaded.entries().len(), 1);
    assert_eq!(loaded.entries()[0].path, kept);

    let mut history = loaded;
    std::fs::remove_file(&kept).expect("vanish too");
    assert!(history.prune_missing());
    assert!(history.entries().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_history_keeps_the_latest_hundred() {
    let mut history = History::default();
    for i in 0..(CAPACITY as u64 + 5) {
        history.push(entry(PathBuf::from(format!("/nowhere/{i}.png")), i));
    }
    assert_eq!(history.entries().len(), CAPACITY);
    assert_eq!(history.entries()[0].path, PathBuf::from("/nowhere/104.png"));
    assert_eq!(
        history.entries()[CAPACITY - 1].path,
        PathBuf::from("/nowhere/5.png")
    );
}

#[test]
fn pushing_a_known_path_moves_it_first_and_remove_forgets_it() {
    let mut history = History::default();
    history.push(entry(PathBuf::from("/a.png"), 1));
    history.push(entry(PathBuf::from("/b.png"), 2));
    history.push(entry(PathBuf::from("/a.png"), 3));
    assert_eq!(history.entries().len(), 2);
    assert_eq!(
        history.entries()[0].taken_at,
        SystemTime::UNIX_EPOCH + Duration::from_secs(3)
    );
    assert!(history.remove(&PathBuf::from("/a.png")));
    assert!(!history.remove(&PathBuf::from("/a.png")));
    assert_eq!(history.entries().len(), 1);
}

#[test]
fn damaged_lines_are_skipped_and_a_missing_file_is_empty() {
    let dir = scratch("damaged");
    let shot = dir.join("ok.png");
    std::fs::write(&shot, b"png").expect("shot");
    let mut good = History::at(dir.join("history"));
    good.push(entry(shot.clone(), 9));
    good.save().expect("save");
    let mut text = std::fs::read_to_string(dir.join("history")).expect("read");
    text.insert_str(0, "garbage\nscreenshot\tnot-a-number\t1\t/x\n");
    std::fs::write(dir.join("history"), text).expect("damage");

    let loaded = History::load(dir.join("history")).expect("load");
    assert_eq!(loaded.entries().len(), 1);
    assert_eq!(loaded.entries()[0].path, shot);
    assert!(History::load(dir.join("absent"))
        .expect("empty")
        .entries()
        .is_empty());
    let _ = std::fs::remove_dir_all(dir);
}
