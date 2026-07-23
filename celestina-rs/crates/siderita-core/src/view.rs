use std::cmp::Ordering;

use crate::{DirectoryEntry, DirectorySnapshot, EntryKind};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortField {
    #[default]
    Name,
    Size,
    Modified,
    Kind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewOptions {
    pub show_hidden: bool,
    pub query: String,
    pub sort_field: SortField,
    pub sort_direction: SortDirection,
}

/// Projects a snapshot without changing its entries or their identities.
#[must_use]
pub fn project_snapshot<'a>(
    snapshot: &'a DirectorySnapshot,
    options: &ViewOptions,
) -> Vec<&'a DirectoryEntry> {
    let normalized_query = options.query.to_lowercase();
    let mut entries: Vec<_> = snapshot
        .entries()
        .iter()
        .filter(|entry| options.show_hidden || !entry.is_hidden())
        .filter(|entry| {
            normalized_query.is_empty()
                || entry
                    .display_name()
                    .to_lowercase()
                    .contains(&normalized_query)
        })
        .collect();

    entries.sort_by(|left, right| compare_entries(left, right, options));
    entries
}

fn compare_entries(
    left: &DirectoryEntry,
    right: &DirectoryEntry,
    options: &ViewOptions,
) -> Ordering {
    // Hidden entries always sink below visible ones, whatever the sort field or
    // direction — so the dotfiles form one block at the end, not interleaved.
    let hidden_order = left.is_hidden().cmp(&right.is_hidden());
    if hidden_order != Ordering::Equal {
        return hidden_order;
    }

    let group_order = entry_group(left.kind()).cmp(&entry_group(right.kind()));
    if group_order != Ordering::Equal {
        return group_order;
    }

    let field_order = match options.sort_field {
        SortField::Name => left.raw_name().cmp(right.raw_name()),
        SortField::Size => left
            .size()
            .cmp(&right.size())
            .then_with(|| left.raw_name().cmp(right.raw_name())),
        SortField::Modified => left
            .modified()
            .cmp(&right.modified())
            .then_with(|| left.raw_name().cmp(right.raw_name())),
        SortField::Kind => entry_kind(left.kind())
            .cmp(&entry_kind(right.kind()))
            .then_with(|| left.raw_name().cmp(right.raw_name())),
    };

    match options.sort_direction {
        SortDirection::Ascending => field_order,
        SortDirection::Descending => field_order.reverse(),
    }
}

const fn entry_group(kind: EntryKind) -> u8 {
    match kind {
        EntryKind::Directory => 0,
        EntryKind::File | EntryKind::Symlink | EntryKind::Other => 1,
    }
}

const fn entry_kind(kind: EntryKind) -> u8 {
    match kind {
        EntryKind::Directory => 0,
        EntryKind::File => 1,
        EntryKind::Symlink => 2,
        EntryKind::Other => 3,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{scan_directory, ScanCoordinator};

    use super::{project_snapshot, SortDirection, SortField, ViewOptions};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "celestina-snapshot-view-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn scan_fixture(fixture: &TestDirectory) -> crate::DirectorySnapshot {
        let mut coordinator = ScanCoordinator::new();
        let request = coordinator.begin(fixture.path()).expect("scan request");
        scan_directory(&request).expect("scan fixture")
    }

    #[test]
    fn filter_is_case_insensitive_and_does_not_mutate_snapshot() {
        let fixture = TestDirectory::new();
        fs::write(fixture.path().join("Alpha"), b"a").expect("write alpha");
        fs::write(fixture.path().join("beta"), b"b").expect("write beta");
        let snapshot = scan_fixture(&fixture);
        let options = ViewOptions {
            query: "ALP".to_owned(),
            ..ViewOptions::default()
        };

        let projected = project_snapshot(&snapshot, &options);

        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].display_name(), "Alpha");
        assert_eq!(snapshot.entries().len(), 2);
    }

    #[test]
    fn directories_stay_first_when_sort_is_descending() {
        let fixture = TestDirectory::new();
        fs::write(fixture.path().join("large"), [0; 8]).expect("write large");
        fs::write(fixture.path().join("small"), [0; 2]).expect("write small");
        fs::create_dir(fixture.path().join("folder")).expect("create folder");
        let snapshot = scan_fixture(&fixture);
        let options = ViewOptions {
            sort_field: SortField::Size,
            sort_direction: SortDirection::Descending,
            ..ViewOptions::default()
        };

        let projected = project_snapshot(&snapshot, &options);

        assert_eq!(projected[0].display_name(), "folder");
        assert_eq!(projected[1].display_name(), "large");
        assert_eq!(projected[2].display_name(), "small");
    }

    #[test]
    fn hidden_entries_sort_below_visible_ones() {
        let fixture = TestDirectory::new();
        fs::write(fixture.path().join("visible.txt"), b"v").expect("write visible");
        fs::write(fixture.path().join(".hidden.txt"), b"h").expect("write hidden");
        fs::create_dir(fixture.path().join(".hiddendir")).expect("hidden dir");
        fs::create_dir(fixture.path().join("visibledir")).expect("visible dir");
        let snapshot = scan_fixture(&fixture);
        let options = ViewOptions {
            show_hidden: true,
            ..ViewOptions::default()
        };

        let names: Vec<String> = project_snapshot(&snapshot, &options)
            .iter()
            .map(|row| row.display_name().to_string())
            .collect();

        // Visible folder, visible file, then the hidden block (folder then file).
        assert_eq!(names, ["visibledir", "visible.txt", ".hiddendir", ".hidden.txt"]);
    }

    #[test]
    fn hidden_entries_are_only_a_projection_choice() {
        let fixture = TestDirectory::new();
        fs::write(fixture.path().join(".hidden"), b"h").expect("write hidden");
        let snapshot = scan_fixture(&fixture);

        assert!(project_snapshot(&snapshot, &ViewOptions::default()).is_empty());
        assert_eq!(
            project_snapshot(
                &snapshot,
                &ViewOptions {
                    show_hidden: true,
                    ..ViewOptions::default()
                }
            )
            .len(),
            1
        );
        assert_eq!(snapshot.entries().len(), 1);
    }
}
