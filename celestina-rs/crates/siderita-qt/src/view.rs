use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use celestina_core::Generation;
use siderita_core::{
    project_snapshot, DirectoryEntry, DirectorySnapshot, EntryId, EntryKind, ViewOptions,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntryToken(u64);

impl EntryToken {
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for EntryToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowKind {
    Directory,
    File,
    Symlink,
    Other,
}

impl From<EntryKind> for RowKind {
    fn from(kind: EntryKind) -> Self {
        match kind {
            EntryKind::Directory => Self::Directory,
            EntryKind::File => Self::File,
            EntryKind::Symlink => Self::Symlink,
            EntryKind::Other => Self::Other,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryRow {
    token: EntryToken,
    display_name: String,
    path: PathBuf,
    kind: RowKind,
    size: u64,
    modified: Option<std::time::SystemTime>,
    hidden: bool,
}

impl EntryRow {
    #[must_use]
    pub const fn token(&self) -> EntryToken {
        self.token
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn kind(&self) -> RowKind {
        self.kind
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn modified(&self) -> Option<std::time::SystemTime> {
        self.modified
    }

    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        self.hidden
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewSnapshot {
    generation: Generation,
    location: PathBuf,
    rows: Vec<EntryRow>,
}

impl ViewSnapshot {
    #[must_use]
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    #[must_use]
    pub fn location(&self) -> &Path {
        &self.location
    }

    #[must_use]
    pub fn rows(&self) -> &[EntryRow] {
        &self.rows
    }

    #[must_use]
    pub fn row(&self, index: usize) -> Option<&EntryRow> {
        self.rows.get(index)
    }
}

#[derive(Debug)]
pub struct SnapshotAdapter {
    tokens: HashMap<EntryId, EntryToken>,
    next_token: u64,
}

impl Default for SnapshotAdapter {
    fn default() -> Self {
        Self {
            tokens: HashMap::new(),
            next_token: 1,
        }
    }
}

impl SnapshotAdapter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn adapt(&mut self, snapshot: &DirectorySnapshot) -> Result<ViewSnapshot, TokenExhausted> {
        self.reconcile_tokens(snapshot)?;
        Ok(self.build_view(snapshot, snapshot.entries().iter()))
    }

    /// Adapts a filtered/sorted projection while retaining identities for all
    /// entries in the underlying snapshot.
    ///
    /// Hidden or filtered rows therefore recover the same opaque token when
    /// they become visible again.
    pub fn adapt_projected(
        &mut self,
        snapshot: &DirectorySnapshot,
        options: &ViewOptions,
    ) -> Result<ViewSnapshot, TokenExhausted> {
        self.reconcile_tokens(snapshot)?;
        let entries = project_snapshot(snapshot, options);
        Ok(self.build_view(snapshot, entries))
    }

    fn reconcile_tokens(&mut self, snapshot: &DirectorySnapshot) -> Result<(), TokenExhausted> {
        let previous = std::mem::take(&mut self.tokens);
        let mut current = HashMap::with_capacity(snapshot.entries().len());

        for entry in snapshot.entries() {
            let token = match previous.get(entry.id()) {
                Some(token) => *token,
                None => self.allocate_token()?,
            };
            current.insert(entry.id().clone(), token);
        }

        self.tokens = current;
        Ok(())
    }

    fn build_view<'a>(
        &self,
        snapshot: &DirectorySnapshot,
        entries: impl IntoIterator<Item = &'a DirectoryEntry>,
    ) -> ViewSnapshot {
        let entries = entries.into_iter();
        let (lower_bound, _) = entries.size_hint();
        let mut rows = Vec::with_capacity(lower_bound);

        for entry in entries {
            let token = *self
                .tokens
                .get(entry.id())
                .expect("tokens were reconciled before building the view");
            rows.push(EntryRow {
                token,
                display_name: entry.display_name().into_owned(),
                path: entry.path().to_path_buf(),
                kind: entry.kind().into(),
                size: entry.size(),
                modified: entry.modified(),
                hidden: entry.is_hidden(),
            });
        }

        ViewSnapshot {
            generation: snapshot.generation(),
            location: snapshot.location().to_path_buf(),
            rows,
        }
    }

    fn allocate_token(&mut self) -> Result<EntryToken, TokenExhausted> {
        let token = EntryToken(self.next_token);
        self.next_token = self.next_token.checked_add(1).ok_or(TokenExhausted)?;
        Ok(token)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenExhausted;

impl fmt::Display for TokenExhausted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("entry token counter exhausted")
    }
}

impl Error for TokenExhausted {}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use siderita_core::{scan_directory, ScanCoordinator, ViewOptions};

    use super::SnapshotAdapter;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "celestina-view-adapter-{}-{nonce}",
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

    #[test]
    fn unchanged_entries_keep_tokens_across_refreshes() {
        let fixture = TestDirectory::new();
        fs::write(fixture.path().join("alpha"), b"a").expect("write alpha");
        fs::write(fixture.path().join("beta"), b"b").expect("write beta");
        let mut coordinator = ScanCoordinator::new();
        let first_request = coordinator.begin(fixture.path()).expect("first request");
        let first = scan_directory(&first_request).expect("first scan");
        let mut adapter = SnapshotAdapter::new();
        let first_view = adapter.adapt(&first).expect("adapt first scan");
        let first_tokens: HashMap<_, _> = first_view
            .rows()
            .iter()
            .map(|row| (row.display_name().to_owned(), row.token()))
            .collect();

        fs::write(fixture.path().join("gamma"), b"c").expect("write gamma");
        let second_request = coordinator.begin(fixture.path()).expect("second request");
        let second = scan_directory(&second_request).expect("second scan");
        let second_view = adapter.adapt(&second).expect("adapt second scan");
        let second_tokens: HashMap<_, _> = second_view
            .rows()
            .iter()
            .map(|row| (row.display_name().to_owned(), row.token()))
            .collect();

        assert_eq!(first_tokens["alpha"], second_tokens["alpha"]);
        assert_eq!(first_tokens["beta"], second_tokens["beta"]);
        assert_ne!(second_tokens["gamma"], second_tokens["alpha"]);
    }

    #[test]
    fn display_name_is_not_used_as_the_token() {
        let fixture = TestDirectory::new();
        fs::write(fixture.path().join("42"), b"content").expect("write fixture");
        let mut coordinator = ScanCoordinator::new();
        let request = coordinator.begin(fixture.path()).expect("scan request");
        let snapshot = scan_directory(&request).expect("scan fixture");
        let row = SnapshotAdapter::new()
            .adapt(&snapshot)
            .expect("adapt snapshot")
            .rows()[0]
            .clone();

        assert_eq!(row.display_name(), "42");
        assert_ne!(row.token().to_string(), row.display_name());
    }

    #[test]
    fn filtered_entries_recover_their_stable_tokens() {
        let fixture = TestDirectory::new();
        fs::write(fixture.path().join(".hidden"), b"content").expect("write fixture");
        let mut coordinator = ScanCoordinator::new();
        let request = coordinator.begin(fixture.path()).expect("scan request");
        let snapshot = scan_directory(&request).expect("scan fixture");
        let mut adapter = SnapshotAdapter::new();

        let visible = adapter
            .adapt_projected(
                &snapshot,
                &ViewOptions {
                    show_hidden: true,
                    ..ViewOptions::default()
                },
            )
            .expect("adapt visible hidden entry");
        let token = visible.rows()[0].token();

        let filtered = adapter
            .adapt_projected(&snapshot, &ViewOptions::default())
            .expect("adapt filtered snapshot");
        assert!(filtered.rows().is_empty());

        let visible_again = adapter
            .adapt_projected(
                &snapshot,
                &ViewOptions {
                    show_hidden: true,
                    ..ViewOptions::default()
                },
            )
            .expect("adapt visible snapshot again");
        assert_eq!(visible_again.rows()[0].token(), token);
    }
}
