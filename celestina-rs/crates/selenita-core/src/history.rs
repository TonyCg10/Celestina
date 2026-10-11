//! The last [`CAPACITY`] captures, the latest first, kept in
//! `data_home/selenita/history`.
//!
//! One line per entry, tab-separated: the kind word, the time it was taken
//! in seconds since the epoch, the size in bytes and the path's key
//! (byte-exact, `celestina_core::pathkey`). A line that does not read is
//! skipped, so a damaged file loses lines rather than the whole list. An entry
//! whose file is gone is dropped on load ([`History::prune_missing`]).
//!
//! A file another application wrote into Selenita's folders (Fluorita's
//! edited copy beside a capture) joins the history through [`adopted`], the
//! rule `org.celestina.Selenita1.Adopt` applies (ADR 0012, PRV-1).

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use celestina_core::{atomic_file, pathkey, xdg};

/// How many entries the history keeps.
pub const CAPACITY: usize = 100;

/// 100 lines of a few hundred bytes; anything larger is not Selenita's.
const READ_LIMIT: u64 = 512 * 1024;

/// What an entry is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryKind {
    Screenshot,
    /// Arrives with SEL-1-B; read already so a newer file loads.
    Recording,
}

impl EntryKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Screenshot => "screenshot",
            Self::Recording => "recording",
        }
    }

    fn parse(word: &str) -> Option<Self> {
        match word {
            "screenshot" => Some(Self::Screenshot),
            "recording" => Some(Self::Recording),
            _ => None,
        }
    }
}

/// One capture in the history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    pub kind: EntryKind,
    pub taken_at: SystemTime,
    pub size: u64,
}

impl Entry {
    /// The entry's identity across the Qt seam: its path key.
    #[must_use]
    pub fn id(&self) -> String {
        pathkey::encode(&self.path)
    }

    fn to_line(&self) -> String {
        let seconds = self
            .taken_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        format!(
            "{}\t{}\t{}\t{}\n",
            self.kind.as_str(),
            seconds,
            self.size,
            pathkey::encode(&self.path)
        )
    }

    fn from_line(line: &str) -> Option<Self> {
        let mut fields = line.split('\t');
        let kind = EntryKind::parse(fields.next()?)?;
        let seconds: u64 = fields.next()?.parse().ok()?;
        let size = fields.next()?.parse().ok()?;
        let path = pathkey::decode(fields.next()?).ok()?;
        if fields.next().is_some() {
            return None;
        }
        Some(Self {
            path,
            kind,
            taken_at: SystemTime::UNIX_EPOCH + Duration::from_secs(seconds),
            size,
        })
    }
}

/// Why the history could not be read or written.
#[derive(Debug)]
pub enum HistoryError {
    /// No data home could be named (no `HOME`).
    NoDataHome,
    Read(atomic_file::ReadError),
    Write(atomic_file::WriteError),
}

impl fmt::Display for HistoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDataHome => formatter.write_str("no data home"),
            Self::Read(error) => write!(formatter, "reading the history: {error}"),
            Self::Write(error) => write!(formatter, "writing the history: {error}"),
        }
    }
}

impl std::error::Error for HistoryError {}

/// The captures and the file they are kept in.
#[derive(Clone, Debug, Default)]
pub struct History {
    file: Option<PathBuf>,
    entries: Vec<Entry>,
}

impl History {
    /// The default file, `data_home/selenita/history`.
    #[must_use]
    pub fn default_file() -> Option<PathBuf> {
        xdg::data_home().map(|home| home.join("selenita").join("history"))
    }

    /// An empty history that saves to `file`.
    #[must_use]
    pub fn at(file: PathBuf) -> Self {
        Self {
            file: Some(file),
            entries: Vec::new(),
        }
    }

    /// The history in `file`, without the entries whose file is gone; a
    /// missing file is an empty history.
    ///
    /// # Errors
    ///
    /// [`HistoryError::Read`] when the file exists and cannot be read.
    pub fn load(file: PathBuf) -> Result<Self, HistoryError> {
        let bytes = atomic_file::read_bounded(&file, READ_LIMIT)
            .map_err(HistoryError::Read)?
            .unwrap_or_default();
        // Every field is ASCII, the path included (a `pathkey` key is ASCII
        // whatever bytes the path holds), so a lossy read loses nothing a
        // valid line carries.
        let text = String::from_utf8_lossy(&bytes);
        let mut history = Self::at(file);
        for entry in text.lines().filter_map(Entry::from_line) {
            if history.entries.len() == CAPACITY {
                break;
            }
            if !history.entries.iter().any(|known| known.path == entry.path) {
                history.entries.push(entry);
            }
        }
        history.prune_missing();
        Ok(history)
    }

    /// The entries, the latest first.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The entry with this [`Entry::id`].
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id() == id)
    }

    /// Puts `entry` first, replacing one for the same path, and drops the
    /// oldest past [`CAPACITY`].
    pub fn push(&mut self, entry: Entry) {
        self.entries.retain(|known| known.path != entry.path);
        self.entries.insert(0, entry);
        self.entries.truncate(CAPACITY);
    }

    /// Forgets the entry for `path`; whether there was one.
    pub fn remove(&mut self, path: &Path) -> bool {
        let before = self.entries.len();
        self.entries.retain(|known| known.path != path);
        self.entries.len() != before
    }

    /// Drops the entries whose file no longer exists; whether any was.
    pub fn prune_missing(&mut self) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|entry| std::fs::symlink_metadata(&entry.path).is_ok());
        self.entries.len() != before
    }

    /// Writes the history atomically, private to the person.
    ///
    /// # Errors
    ///
    /// [`HistoryError::NoDataHome`] for a history without a file, or the
    /// write's failure.
    pub fn save(&self) -> Result<(), HistoryError> {
        let file = self.file.as_ref().ok_or(HistoryError::NoDataHome)?;
        let text: String = self.entries.iter().map(Entry::to_line).collect();
        atomic_file::replace_private(file, text.as_bytes())
            .map(|_| ())
            .map_err(HistoryError::Write)
    }
}

/// Selenita's two folders, as [`adopted`] reads them: the captures folder
/// inside the pictures folder and the recordings folder inside the videos
/// folder. The captures folder's name is product copy, so the caller names
/// both.
#[derive(Clone, Copy, Debug)]
pub struct Folders<'a> {
    pub captures: &'a Path,
    pub recordings: &'a Path,
}

/// The entry `path` becomes when another application hands it to Selenita
/// (`Adopt`), taken `now`: a regular file (not a link) directly inside one
/// of `folders`, a screenshot for a `.png` and a recording for a `.mp4`
/// (either case). `None` for anything else, which is ignored. Blocking: it
/// reads the file's metadata.
#[must_use]
pub fn adopted(path: &Path, folders: Folders<'_>, now: SystemTime) -> Option<Entry> {
    let parent = path.parent()?;
    if !path.is_absolute() || (parent != folders.captures && parent != folders.recordings) {
        return None;
    }
    let extension = path.extension()?;
    let kind = if extension.eq_ignore_ascii_case("png") {
        EntryKind::Screenshot
    } else if extension.eq_ignore_ascii_case("mp4") {
        EntryKind::Recording
    } else {
        return None;
    };
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_file() {
        return None;
    }
    Some(Entry {
        path: path.to_path_buf(),
        kind,
        taken_at: now,
        size: meta.len(),
    })
}
