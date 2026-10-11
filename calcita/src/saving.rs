// language-contract: product-copy
//! Landing an edited document beside, or instead of, the original.
//!
//! `calcita_core` makes the bytes (the original's, then one incremental
//! update); this module puts them on disk with the suite's two outcomes and
//! its shared owners, in the order that never leaves a person with neither
//! file:
//!
//! - **«Guardar ambas»**: the bytes land under the next free «(editado)»
//!   name beside the original ([`siderita_ops::next_available`]) through
//!   [`atomic_file::land_media`], which takes the original's permission
//!   bits and refuses a name that appeared meanwhile; the original is not
//!   touched.
//! - **«Guardar solo la editada»**: the bytes first land as an ordinary
//!   file under a hidden name of their own beside the original; only then
//!   does the original go to the Trash (the suite's one implementation,
//!   `siderita_ops::trash`, behind [`Bin`]); then the hidden file takes the
//!   original's name without replacing anything. If that last step fails,
//!   the edited file stays at its hidden name and the error names it.
//!
//! Fluorita's engine lands an edited picture in the same order
//! (`fluorita_engine::land_file`); it is not reused because it lives in
//! Fluorita's media engine with Fluorita's types, and both are built on
//! the same two owners (`atomic_file`, `siderita_ops`).
//!
//! Before anything, the file must still be the version the edit began on
//! ([`Stamp`]): edits name annotations by their place in that version.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use calcita_core::annot::edit::EditSet;
use calcita_core::annot::write::apply_to_bytes;
use calcita_core::annot::AnnotError;
use celestina_core::atomic_file::{self, WriteError};

/// The marker a copy beside the original carries in its name.
pub const COPY_MARKER: &str = "editado";

/// How many hidden names a replacement tries before giving up; each
/// collision means another process holds that exact name.
const HIDDEN_ATTEMPTS: u32 = 16;

/// How many free copy names are tried when one is taken between finding
/// it and landing there.
const COPY_ATTEMPTS: u32 = 16;

/// The most bytes of the original's name a hidden name keeps, so the hidden
/// name and `atomic_file`'s own temporary name on it stay under 255 bytes.
const HIDDEN_STEM_BYTES: usize = 180;

/// A version of a file: its size and modification time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stamp {
    bytes: u64,
    modified: Option<SystemTime>,
}

impl Stamp {
    /// The version of the file at `path` now.
    ///
    /// # Errors
    ///
    /// The `stat` failure.
    pub fn of(path: &Path) -> std::io::Result<Self> {
        let metadata = std::fs::metadata(path)?;
        Ok(Self {
            bytes: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
}

/// The two outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveChoice {
    /// «Guardar ambas»: a copy beside the original.
    Both,
    /// «Guardar solo la editada»: the original to the Trash, the edited file
    /// under its name.
    EditedOnly,
}

/// Where a replaced original goes. The application's is the desktop Trash;
/// tests record what they are asked.
pub trait Bin {
    /// Moves `path` somewhere it can be restored from.
    ///
    /// # Errors
    ///
    /// Why it could not: the file is then where it was.
    fn send(&self, path: &Path) -> Result<(), String>;
}

/// The desktop Trash, through the suite's one implementation of the
/// freedesktop specification.
pub struct DesktopTrash;

impl Bin for DesktopTrash {
    fn send(&self, path: &Path) -> Result<(), String> {
        siderita_ops::trash(path, &celestina_core::CancellationToken::new(), &mut |_| {})
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

/// What a save did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Saved {
    /// The file now holding the edited document.
    pub written: PathBuf,
    /// Whether the original went to the Trash.
    pub replaced: bool,
}

/// Why a save did not land. Each says what is on disk.
#[derive(Debug)]
pub enum SaveError {
    /// The file is not the version the edit began on (or is gone); nothing
    /// was written.
    Changed,
    /// The edited bytes could not be made; nothing was written.
    Write(AnnotError),
    /// The edited file could not be written beside the original; the
    /// original is untouched.
    Landing(WriteError),
    /// The original could not go to the Trash; nothing was replaced and the
    /// edited file was removed.
    Trash(String),
    /// The original is in the Trash, and the edited file could not take its
    /// name: it is kept at `kept`.
    NotPublished {
        kept: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for SaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Changed => formatter.write_str("the document changed since the edit began"),
            Self::Write(error) => write!(formatter, "{error}"),
            Self::Landing(error) => write!(formatter, "the edited file did not land: {error}"),
            Self::Trash(error) => {
                write!(formatter, "the original did not reach the Trash: {error}")
            }
            Self::NotPublished { kept, source } => write!(
                formatter,
                "the edited file stays at {}: {source}",
                kept.display()
            ),
        }
    }
}

impl std::error::Error for SaveError {}

/// The name a person reads for `path`.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl SaveError {
    /// What the window says, in Spanish.
    #[must_use]
    pub fn message_es(&self) -> String {
        match self {
            Self::Changed => {
                "El documento cambió fuera de Calcita mientras lo editabas; no se guardó."
                    .to_owned()
            }
            Self::Write(error) => error.message_es(),
            Self::Landing(_) => "No se pudo guardar el documento editado.".to_owned(),
            Self::Trash(_) => {
                "No se pudo mover el original a la papelera; no se reemplazó.".to_owned()
            }
            Self::NotPublished { kept, .. } => format!(
                "El original está en la papelera y el editado quedó como «{}».",
                file_name(kept)
            ),
        }
    }
}

impl Saved {
    /// What the window says, in Spanish.
    #[must_use]
    pub fn message_es(&self) -> String {
        if self.replaced {
            "Guardado. El original está en la papelera.".to_owned()
        } else {
            format!("Guardado como «{}».", file_name(&self.written))
        }
    }
}

/// `name` cut to at most `limit` bytes, on a character boundary.
fn truncated(name: &str, limit: usize) -> &str {
    let mut end = name.len().min(limit);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    &name[..end]
}

/// Saves `edits` to the document at `source`, which must still be at
/// `stamp`, as `choice` says; `bin` takes a replaced original.
///
/// # Errors
///
/// A [`SaveError`], which says what is on disk.
pub fn save(
    source: &Path,
    stamp: Stamp,
    edits: &EditSet,
    choice: SaveChoice,
    bin: &dyn Bin,
) -> Result<Saved, SaveError> {
    if Stamp::of(source).ok() != Some(stamp) {
        return Err(SaveError::Changed);
    }
    let bytes = apply_to_bytes(source, edits).map_err(SaveError::Write)?;
    match choice {
        SaveChoice::Both => land_copy(source, &bytes),
        SaveChoice::EditedOnly => replace(source, &bytes, bin),
    }
}

fn land_copy(source: &Path, bytes: &[u8]) -> Result<Saved, SaveError> {
    let unusable = || {
        SaveError::Landing(WriteError::TargetExists {
            path: source.to_path_buf(),
        })
    };
    let directory = source.parent().ok_or_else(unusable)?;
    let name = source.file_name().ok_or_else(unusable)?;
    let mut last = None;
    for _ in 0..COPY_ATTEMPTS {
        let destination = siderita_ops::next_available(
            directory,
            name,
            COPY_MARKER,
            siderita_ops::NameShape::File,
        );
        match atomic_file::land_media(&destination, bytes, source) {
            Ok(_published) => {
                return Ok(Saved {
                    written: destination,
                    replaced: false,
                })
            }
            // Taken between the look and the landing: look again.
            Err(error @ WriteError::TargetExists { .. }) => last = Some(error),
            Err(error) => return Err(SaveError::Landing(error)),
        }
    }
    Err(last.map_or_else(unusable, SaveError::Landing))
}

/// Lands `bytes` beside `source` under a fresh hidden name.
fn land_hidden(source: &Path, bytes: &[u8]) -> Result<PathBuf, SaveError> {
    let name = file_name(source);
    let stem = truncated(&name, HIDDEN_STEM_BYTES);
    let mut last = None;
    for attempt in 0..HIDDEN_ATTEMPTS {
        let hidden =
            source.with_file_name(format!(".{stem}.calcita-{}-{attempt}", std::process::id()));
        match atomic_file::land_media(&hidden, bytes, source) {
            Ok(_published) => return Ok(hidden),
            Err(error @ WriteError::TargetExists { .. }) => last = Some(error),
            Err(error) => return Err(SaveError::Landing(error)),
        }
    }
    Err(SaveError::Landing(last.unwrap_or(
        WriteError::TargetExists {
            path: source.to_path_buf(),
        },
    )))
}

fn replace(source: &Path, bytes: &[u8], bin: &dyn Bin) -> Result<Saved, SaveError> {
    // The edited file is an ordinary, synced file before the original moves.
    let hidden = land_hidden(source, bytes)?;
    if let Err(error) = bin.send(source) {
        let _ = std::fs::remove_file(&hidden);
        return Err(SaveError::Trash(error));
    }
    match atomic_file::publish_without_replacing(&hidden, source) {
        Ok(()) => Ok(Saved {
            written: source.to_path_buf(),
            replaced: true,
        }),
        Err(error) => Err(SaveError::NotPublished {
            kept: hidden,
            source: error,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{save, Bin, SaveChoice, SaveError, Stamp};
    use calcita_core::annot::edit::{self, Base, Edit, EditSet};
    use calcita_core::annot::{read_annotations, Kind, Rect, Rgb};
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("calcita-saving-{}-{label}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("scratch");
            Self(dir)
        }

        /// A copy of the three-page fixture named `name`.
        fn document(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/three-pages.pdf"),
                &path,
            )
            .expect("copied");
            path
        }

        fn names(&self) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(&self.0)
                .expect("listed")
                .map(|entry| {
                    entry
                        .expect("entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A bin that records what it was sent, moving it aside, or refuses.
    struct FakeBin {
        refuse: bool,
        sent: RefCell<Vec<PathBuf>>,
    }

    impl FakeBin {
        fn new(refuse: bool) -> Self {
            Self {
                refuse,
                sent: RefCell::new(Vec::new()),
            }
        }
    }

    impl Bin for FakeBin {
        fn send(&self, path: &Path) -> Result<(), String> {
            if self.refuse {
                return Err("no trash here".to_owned());
            }
            let aside = path.with_file_name("trashed.bin");
            std::fs::rename(path, &aside).map_err(|error| error.to_string())?;
            self.sent.borrow_mut().push(path.to_path_buf());
            Ok(())
        }
    }

    fn one_text_box(path: &Path) -> EditSet {
        let base = Base::read(path).expect("readable");
        let mut edits = EditSet::new();
        let added = edit::text_box(
            base.boxes[0],
            0,
            Rect::new(72.0, 72.0, 272.0, 80.0),
            "guardado",
            12.0,
            Rgb { r: 0, g: 0, b: 255 },
        );
        edits.push(&base, Edit::Add(added)).expect("added");
        edits
    }

    fn texts(path: &Path) -> Vec<String> {
        read_annotations(path)
            .expect("rereads")
            .into_iter()
            .filter_map(|annotation| match annotation.kind {
                Kind::Text { contents, .. } => Some(contents),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn both_lands_a_copy_beside_and_leaves_the_original() {
        let scratch = Scratch::new("both");
        let source = scratch.document("informe.pdf");
        let original = std::fs::read(&source).expect("read");
        let edits = one_text_box(&source);
        let stamp = Stamp::of(&source).expect("stat");
        let bin = FakeBin::new(false);
        let saved = save(&source, stamp, &edits, SaveChoice::Both, &bin).expect("saved");
        assert_eq!(
            saved.written,
            source.with_file_name("informe (editado).pdf")
        );
        assert!(!saved.replaced);
        assert_eq!(saved.message_es(), "Guardado como «informe (editado).pdf».");
        assert_eq!(std::fs::read(&source).expect("read"), original, "untouched");
        assert!(bin.sent.borrow().is_empty());
        assert_eq!(texts(&saved.written), vec!["guardado"]);
        let copy = std::fs::read(&saved.written).expect("read");
        assert!(copy.starts_with(&original), "an incremental update");

        let again = save(&source, stamp, &edits, SaveChoice::Both, &bin).expect("saved");
        assert_eq!(
            again.written,
            source.with_file_name("informe (editado 2).pdf")
        );
        assert_eq!(
            scratch.names(),
            vec![
                "informe (editado 2).pdf",
                "informe (editado).pdf",
                "informe.pdf"
            ]
        );
    }

    #[test]
    fn edited_only_trashes_the_original_after_the_result_is_written() {
        let scratch = Scratch::new("only");
        let source = scratch.document("carta.pdf");
        let original = std::fs::read(&source).expect("read");
        let edits = one_text_box(&source);
        let stamp = Stamp::of(&source).expect("stat");
        let bin = FakeBin::new(false);
        let saved = save(&source, stamp, &edits, SaveChoice::EditedOnly, &bin).expect("saved");
        assert_eq!(saved.written, source);
        assert!(saved.replaced);
        assert_eq!(
            saved.message_es(),
            "Guardado. El original está en la papelera."
        );
        assert_eq!(*bin.sent.borrow(), vec![source.clone()]);
        assert_eq!(texts(&source), vec!["guardado"]);
        assert_eq!(
            std::fs::read(scratch.0.join("trashed.bin")).expect("the original, aside"),
            original
        );
        assert_eq!(
            scratch.names(),
            vec!["carta.pdf", "trashed.bin"],
            "no hidden file left"
        );
    }

    #[test]
    fn a_refused_trash_replaces_nothing_and_leaves_nothing() {
        let scratch = Scratch::new("refused");
        let source = scratch.document("carta.pdf");
        let original = std::fs::read(&source).expect("read");
        let edits = one_text_box(&source);
        let stamp = Stamp::of(&source).expect("stat");
        let error = save(
            &source,
            stamp,
            &edits,
            SaveChoice::EditedOnly,
            &FakeBin::new(true),
        )
        .expect_err("refused");
        assert!(matches!(error, SaveError::Trash(_)), "{error:?}");
        assert_eq!(
            error.message_es(),
            "No se pudo mover el original a la papelera; no se reemplazó."
        );
        assert_eq!(std::fs::read(&source).expect("read"), original);
        assert_eq!(scratch.names(), vec!["carta.pdf"]);
    }

    #[test]
    fn a_file_changed_since_the_edit_began_is_not_saved() {
        let scratch = Scratch::new("changed");
        let source = scratch.document("carta.pdf");
        let edits = one_text_box(&source);
        let stamp = Stamp::of(&source).expect("stat");
        let mut bytes = std::fs::read(&source).expect("read");
        bytes.extend_from_slice(b"\n% changed elsewhere\n");
        std::fs::write(&source, &bytes).expect("rewritten");
        for choice in [SaveChoice::Both, SaveChoice::EditedOnly] {
            let error =
                save(&source, stamp, &edits, choice, &FakeBin::new(false)).expect_err("refused");
            assert!(matches!(error, SaveError::Changed), "{error:?}");
        }
        assert_eq!(scratch.names(), vec!["carta.pdf"]);
        std::fs::remove_file(&source).expect("removed");
        assert!(matches!(
            save(
                &source,
                stamp,
                &edits,
                SaveChoice::Both,
                &FakeBin::new(false)
            ),
            Err(SaveError::Changed)
        ));
    }
}
