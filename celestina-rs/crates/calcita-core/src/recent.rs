// language-contract: product-copy
//! The recent documents: at most [`CAPACITY`], the latest first, kept in
//! `data_home/calcita/recent`.
//!
//! One line per document, tab-separated: the path's key (byte-exact,
//! `celestina_core::pathkey`), the page, the zoom word and the opening time
//! in seconds since the epoch. A line that does not read is skipped, so a
//! damaged store loses lines rather than the whole list.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use celestina_core::{atomic_file, pathkey, xdg};

use crate::reading::Reading;
use crate::zoom::ZoomMode;

/// How many documents the store remembers.
pub const CAPACITY: usize = 50;

/// A store larger than this is not Calcita's: 50 lines of a few hundred
/// bytes each.
const READ_LIMIT: u64 = 256 * 1024;

/// One remembered document.
#[derive(Clone, Debug, PartialEq)]
pub struct Recent {
    pub path: PathBuf,
    /// 1-based.
    pub page: u32,
    pub zoom: ZoomMode,
    pub opened_at: SystemTime,
}

impl Recent {
    /// Where this document is reopened.
    #[must_use]
    pub fn reading(&self) -> Reading {
        Reading {
            page: self.page.max(1),
            zoom: self.zoom,
        }
    }

    fn to_line(&self) -> String {
        let seconds = self
            .opened_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        format!(
            "{}\t{}\t{}\t{}\n",
            pathkey::encode(&self.path),
            self.page,
            self.zoom.to_word(),
            seconds
        )
    }

    fn from_line(line: &str) -> Option<Self> {
        let mut fields = line.split('\t');
        let path = pathkey::decode(fields.next()?).ok()?;
        let page = fields.next()?.parse().ok()?;
        let zoom = ZoomMode::parse(fields.next()?)?;
        let seconds: u64 = fields.next()?.parse().ok()?;
        if fields.next().is_some() {
            return None;
        }
        Some(Self {
            path,
            page,
            zoom,
            opened_at: SystemTime::UNIX_EPOCH + Duration::from_secs(seconds),
        })
    }
}

/// Why the store could not be read or written.
#[derive(Debug)]
pub enum RecentError {
    /// No data home could be named (no `HOME`).
    NoDataHome,
    Read(atomic_file::ReadError),
    Write(atomic_file::WriteError),
}

impl RecentError {
    /// What the window says, in Spanish.
    #[must_use]
    pub fn message_es(&self) -> &'static str {
        match self {
            Self::NoDataHome => "No se encuentra la carpeta de datos del usuario.",
            Self::Read(_) => "No se pudieron leer los documentos recientes.",
            Self::Write(_) => "No se pudieron guardar los documentos recientes.",
        }
    }
}

impl fmt::Display for RecentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDataHome => formatter.write_str("no data home"),
            Self::Read(error) => write!(formatter, "reading the recent store: {error}"),
            Self::Write(error) => write!(formatter, "writing the recent store: {error}"),
        }
    }
}

impl std::error::Error for RecentError {}

/// The remembered documents and the file they live in.
#[derive(Clone, Debug, Default)]
pub struct RecentStore {
    file: Option<PathBuf>,
    entries: Vec<Recent>,
}

impl RecentStore {
    /// The default file, `data_home/calcita/recent`.
    #[must_use]
    pub fn default_file() -> Option<PathBuf> {
        xdg::data_home().map(|home| home.join("calcita").join("recent"))
    }

    /// An empty store that saves to `file`.
    #[must_use]
    pub fn at(file: PathBuf) -> Self {
        Self {
            file: Some(file),
            entries: Vec::new(),
        }
    }

    /// The store in the default file; a missing file is an empty store.
    ///
    /// # Errors
    ///
    /// [`RecentError::NoDataHome`] or the read's failure.
    pub fn load() -> Result<Self, RecentError> {
        Self::load_from(Self::default_file().ok_or(RecentError::NoDataHome)?)
    }

    /// The store in `file`; a missing file is an empty store.
    ///
    /// # Errors
    ///
    /// [`RecentError::Read`] when the file exists and cannot be read.
    pub fn load_from(file: PathBuf) -> Result<Self, RecentError> {
        let bytes = atomic_file::read_bounded(&file, READ_LIMIT)
            .map_err(RecentError::Read)?
            .unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        let mut store = Self::at(file);
        for entry in text.lines().filter_map(Recent::from_line) {
            if store.entries.len() == CAPACITY {
                break;
            }
            if !store.entries.iter().any(|known| known.path == entry.path) {
                store.entries.push(entry);
            }
        }
        Ok(store)
    }

    /// The documents, the latest first.
    #[must_use]
    pub fn entries(&self) -> &[Recent] {
        &self.entries
    }

    /// Where `path` was left, if it is remembered.
    #[must_use]
    pub fn reading_for(&self, path: &Path) -> Option<Reading> {
        self.entries
            .iter()
            .find(|entry| entry.path == path)
            .map(Recent::reading)
    }

    /// Puts `recent` first, replacing the entry for the same path, and drops
    /// the oldest past [`CAPACITY`].
    pub fn touch(&mut self, recent: Recent) {
        self.entries.retain(|entry| entry.path != recent.path);
        self.entries.insert(0, recent);
        self.entries.truncate(CAPACITY);
    }

    /// Writes the store atomically, private to the person.
    ///
    /// # Errors
    ///
    /// [`RecentError::NoDataHome`] for a store without a file, or the
    /// write's failure.
    pub fn save(&self) -> Result<(), RecentError> {
        let file = self.file.as_ref().ok_or(RecentError::NoDataHome)?;
        let text: String = self.entries.iter().map(Recent::to_line).collect();
        atomic_file::replace_private(file, text.as_bytes())
            .map(|_| ())
            .map_err(RecentError::Write)
    }
}
