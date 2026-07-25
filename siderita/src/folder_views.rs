use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// How one folder was left: its view mode and its sort. A folder the user never
/// arranged has no record at all and simply follows the global defaults.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderView {
    pub path: String,
    pub view_mode: String,
    pub sort_field: i32,
    pub sort_ascending: bool,
}

/// How many folders are remembered. The file is a convenience, not an archive:
/// past this, the least recently arranged records fall off the front rather
/// than growing without bound on a machine that browses thousands of folders.
const MAX_RECORDS: usize = 250;

fn config_file() -> Option<PathBuf> {
    Some(celestina_core::xdg::config_home()?.join("siderita").join("folder-views.conf"))
}

/// Loads the per-folder records, oldest first. Any error yields an empty list —
/// forgetting how a folder was arranged costs the user a click, never data.
pub fn load() -> Vec<FolderView> {
    match config_file() {
        Some(path) => load_from(&path),
        None => Vec::new(),
    }
}

pub fn save(records: &[FolderView]) -> io::Result<()> {
    match config_file() {
        Some(path) => save_to(&path, records),
        None => Ok(()),
    }
}

/// Records how `path` is arranged, replacing any earlier record for it and
/// moving it to the most-recent end.
pub fn remember(records: &mut Vec<FolderView>, view: FolderView) {
    records.retain(|record| record.path != view.path);
    records.push(view);
    if records.len() > MAX_RECORDS {
        let excess = records.len() - MAX_RECORDS;
        records.drain(0..excess);
    }
}

/// Drops the record for `path`, if there is one. Returns whether there was.
pub fn forget(records: &mut Vec<FolderView>, path: &str) -> bool {
    let before = records.len();
    records.retain(|record| record.path != path);
    records.len() != before
}

#[must_use]
pub fn find<'a>(records: &'a [FolderView], path: &str) -> Option<&'a FolderView> {
    records.iter().find(|record| record.path == path)
}

fn load_from(path: &Path) -> Vec<FolderView> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let path = parts.next()?.trim();
            let view_mode = parts.next()?.trim();
            let sort_field = parts.next()?.trim().parse::<i32>().ok()?;
            let sort_ascending = parts.next()?.trim() != "false";
            if path.is_empty() || !matches!(view_mode, "list" | "grid" | "details") {
                return None;
            }
            Some(FolderView {
                path: path.to_owned(),
                view_mode: view_mode.to_owned(),
                sort_field: sort_field.clamp(0, 3),
                sort_ascending,
            })
        })
        .collect()
}

fn save_to(path: &Path, records: &[FolderView]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = String::new();
    for record in records {
        // A path cannot hold a tab or a newline in any sane tree, but a corrupt
        // record could; skipping it beats writing a line that reads back wrong.
        if record.path.contains(['\t', '\n', '\r']) {
            continue;
        }
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            record.path, record.view_mode, record.sort_field, record.sort_ascending
        ));
    }
    fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn view(path: &str, mode: &str) -> FolderView {
        FolderView {
            path: path.to_owned(),
            view_mode: mode.to_owned(),
            sort_field: 2,
            sort_ascending: false,
        }
    }

    fn temp_file(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "siderita-fv-{label}-{}-{nonce}/folder-views.conf",
            std::process::id()
        ))
    }

    #[test]
    fn save_then_load_round_trips() {
        let file = temp_file("roundtrip");
        let records = vec![view("/home/u/Pictures", "grid"), view("/etc", "details")];
        save_to(&file, &records).expect("save");
        assert_eq!(load_from(&file), records);
        let _ = fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn remembering_a_folder_twice_keeps_one_record() {
        let mut records = vec![view("/a", "grid"), view("/b", "list")];
        remember(&mut records, view("/a", "details"));
        assert_eq!(records.len(), 2);
        assert_eq!(find(&records, "/a").map(|r| r.view_mode.as_str()), Some("details"));
        // …and it is now the most recent, so it outlives older records.
        assert_eq!(records.last().map(|r| r.path.as_str()), Some("/a"));
    }

    #[test]
    fn the_oldest_records_fall_off_at_the_cap() {
        let mut records = Vec::new();
        for i in 0..MAX_RECORDS + 10 {
            remember(&mut records, view(&format!("/f{i}"), "grid"));
        }
        assert_eq!(records.len(), MAX_RECORDS);
        assert!(find(&records, "/f0").is_none());
        assert!(find(&records, "/f9").is_none());
        assert!(find(&records, "/f10").is_some());
    }

    #[test]
    fn forget_reports_whether_it_removed_anything() {
        let mut records = vec![view("/a", "grid")];
        assert!(forget(&mut records, "/a"));
        assert!(!forget(&mut records, "/a"));
        assert!(records.is_empty());
    }

    #[test]
    fn a_corrupt_line_is_skipped_not_fatal() {
        let file = temp_file("corrupt");
        fs::create_dir_all(file.parent().unwrap()).expect("dir");
        fs::write(&file, "/a\tgrid\t0\ttrue\nnonsense\n/b\tweird\t0\ttrue\n/c\tlist\tx\ttrue\n")
            .expect("write");
        let loaded = load_from(&file);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].path, "/a");
        let _ = fs::remove_dir_all(file.parent().unwrap());
    }
}
