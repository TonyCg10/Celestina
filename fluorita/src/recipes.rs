//! The recipes behind edited copies, as this application keeps them.
//!
//! [ADR 0009](../../docs/decisions/0009-editing-without-an-encoder.md) promises
//! that a copy stays reopenable: its text, marks and crop come back as
//! objects and steps rather than as flattened pixels. The engine owns the
//! store — its format, its bounds, what a recipe is — and this module owns
//! when the application writes to it, reads from it and forgets from it:
//!
//! - **remembered** when a copy lands, keyed by the copy's identity;
//! - **found again** when that copy is opened, only while its original still
//!   has the bytes the recipe was written for;
//! - **forgotten** when the copy goes to the Trash or is replaced.
//!
//! Every function here stats files and reads or writes the store, so each
//! runs on a worker, never on the GUI thread. A store that cannot be read is
//! reported and left alone, never saved over: losing every copy's layers
//! because one read failed is the silent reset this replaced.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use fluorita_core::{
    Canvas, Composition, EditCapabilities, EditDocument, EditLimits, MediaId, MediaKind,
    SourceIdentity,
};
use fluorita_engine::edit_store::{self, EditStore, StoredEdit};

/// One read-modify-write of the store at a time in this process. The editor
/// remembers from its save worker while the library forgets from its trash
/// worker, and two of them interleaving would each write back a store
/// missing the other's change.
static STORE: Mutex<()> = Mutex::new(());

/// A copy reopened from its recipe.
pub struct Reopened {
    /// The original the recipe renders from. The document is in its frame.
    pub base: PathBuf,
    pub document: EditDocument,
    /// The original's capabilities, which are what the replayed steps were
    /// admitted under.
    pub capabilities: EditCapabilities,
}

/// The identity a recipe is keyed by: the file's device and inode, which is
/// what survives a rename and changes when a file is replaced.
#[must_use]
pub fn identity_at(path: &Path) -> Option<MediaId> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path).ok()?;
    Some(MediaId::filesystem(metadata.dev(), metadata.ino()))
}

/// What a base looks like now, in the terms a recipe remembers it by.
fn measured(path: &Path) -> Option<SourceIdentity> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(SourceIdentity::new(
        metadata.len(),
        metadata.modified().ok()?,
    ))
}

/// Writes down, in the store at `store_path`, the recipe behind a copy that
/// just landed at `written`, computed from `base`. Returns whether it was
/// remembered: a failure is not a failed save — the picture is on disk either
/// way — so the caller says "saved, but it will reopen flat" rather than
/// reporting an error. The editor names the store (`edit_store::default_path`
/// in the application, a scratch file under test).
pub(crate) fn remember_in(
    store_path: &Path,
    base: &Path,
    written: &Path,
    base_canvas: Canvas,
    composition: &Composition,
) -> bool {
    let (Some(base_identity), Some(result)) = (measured(base), identity_at(written)) else {
        return false;
    };
    update(store_path, |store| {
        store.remember(
            result,
            StoredEdit {
                base: base.to_path_buf(),
                base_identity,
                base_canvas,
                transforms: composition.transforms.clone(),
                objects: composition.objects.clone(),
            },
        );
        true
    })
}

/// Forgets the recipe behind the file that had `identity`: it went to the
/// Trash, or a replacement took its name with new bytes.
pub fn forget(identity: &MediaId) {
    if let Some(store_path) = edit_store::default_path() {
        forget_in(&store_path, identity);
    }
}

pub(crate) fn forget_in(store_path: &Path, identity: &MediaId) {
    update(store_path, |store| {
        if store.get(identity).is_none() {
            // Nothing to write: most files never had a recipe.
            return false;
        }
        store.forget(identity);
        true
    });
}

/// Looks in the store at `store_path` for the recipe behind `opened` and
/// rebuilds its document, or `None` when there is none, its original is gone
/// or has changed, or the current rules would not admit it — in every one of
/// which the copy opens as the flat picture it is.
#[must_use]
pub(crate) fn reopen_in(store_path: &Path, opened: &Path, limits: EditLimits) -> Option<Reopened> {
    let identity = identity_at(opened)?;
    let store = read(store_path)?;
    let recipe = store.get(&identity)?;
    let recipe = store.usable(&identity, measured(&recipe.base))?;
    let kind = MediaKind::classify_path(&recipe.base)?;
    let capabilities = EditCapabilities::of(kind, &recipe.base);
    if !capabilities.is_editable() {
        return None;
    }
    match recipe.reopen(limits, &capabilities) {
        Ok(document) => Some(Reopened {
            base: recipe.base.clone(),
            document,
            capabilities,
        }),
        Err(rejected) => {
            eprintln!("fluorita: a stored edit no longer replays ({rejected:?}); opening it flat");
            None
        }
    }
}

/// The store, or `None` with a warning when it cannot be read.
fn read(store_path: &Path) -> Option<EditStore> {
    match edit_store::load(store_path) {
        Ok(loaded) => {
            if loaded.skipped > 0 {
                eprintln!(
                    "fluorita: {} stored edit records could not be read",
                    loaded.skipped
                );
            }
            Some(loaded.store)
        }
        Err(error) => {
            eprintln!("fluorita: the stored edits are left untouched: {error}");
            None
        }
    }
}

/// Reads, changes and writes the store under the process-wide lock. The
/// change returns whether there is anything to write; the result is whether
/// the store on disk now holds it.
fn update(store_path: &Path, change: impl FnOnce(&mut EditStore) -> bool) -> bool {
    // A poisoned lock means a worker panicked mid-update, which in this build
    // aborts the process; carrying on with the guard is what a later thread
    // would do anyway.
    let _held = STORE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(mut store) = read(store_path) else {
        return false;
    };
    if !change(&mut store) {
        return false;
    }
    match edit_store::save(store_path, &store) {
        Ok(()) => true,
        Err(error) => {
            eprintln!("fluorita: the stored edits could not be written: {error}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{forget_in, identity_at, remember_in, reopen_in};
    use fluorita_core::{
        Annotation, Area, Canvas, EditCapabilities, EditDocument, EditLimits, MediaKind, Point,
        Redaction, Transform,
    };
    use std::path::{Path, PathBuf};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("fluorita-recipes-{label}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("scratch");
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn limits() -> EditLimits {
        EditLimits::new(100_000_000)
    }

    /// A crop and a redaction over a 400 × 300 original.
    fn edited(base: &Path) -> EditDocument {
        let capabilities = EditCapabilities::of(MediaKind::Image, base);
        let mut document = EditDocument::new(Canvas::new(400, 300).expect("a canvas"), limits());
        document
            .transform(
                Transform::Crop(Area::new(Point::new(10.0, 10.0), 200.0, 100.0)),
                &capabilities,
            )
            .expect("a crop inside the canvas");
        document
            .annotate(
                Annotation::Redact {
                    area: Area::new(Point::new(5.0, 5.0), 20.0, 10.0),
                    style: Redaction::Solid,
                },
                &capabilities,
            )
            .expect("a mark inside the canvas");
        document
    }

    #[test]
    fn a_copy_reopens_with_its_objects_and_its_crop_still_undoable() {
        let scratch = Scratch::new("reopen");
        let store = scratch.0.join("edits.tsv");
        let base = scratch.0.join("foto.png");
        let copy = scratch.0.join("foto (editado).png");
        std::fs::write(&base, b"the original").expect("original");
        std::fs::write(&copy, b"the flattened copy").expect("copy");
        let document = edited(&base);

        assert!(remember_in(
            &store,
            &base,
            &copy,
            document.base(),
            &document.composition()
        ));
        let mut reopened = reopen_in(&store, &copy, limits()).expect("the copy reopens");

        assert_eq!(reopened.base, base);
        assert_eq!(reopened.document.composition(), document.composition());
        while reopened.document.undo() {}
        assert_eq!(reopened.document.canvas(), document.base());
    }

    #[test]
    fn a_copy_whose_original_changed_opens_flat() {
        let scratch = Scratch::new("stale");
        let store = scratch.0.join("edits.tsv");
        let base = scratch.0.join("foto.png");
        let copy = scratch.0.join("foto (editado).png");
        std::fs::write(&base, b"the original").expect("original");
        std::fs::write(&copy, b"the flattened copy").expect("copy");
        let document = edited(&base);
        assert!(remember_in(
            &store,
            &base,
            &copy,
            document.base(),
            &document.composition()
        ));

        std::fs::write(&base, b"a different original, longer").expect("edited elsewhere");

        assert!(reopen_in(&store, &copy, limits()).is_none());
    }

    #[test]
    fn a_forgotten_copy_opens_flat_and_the_others_keep_theirs() {
        let scratch = Scratch::new("forget");
        let store = scratch.0.join("edits.tsv");
        let base = scratch.0.join("foto.png");
        let first = scratch.0.join("a.png");
        let second = scratch.0.join("b.png");
        std::fs::write(&base, b"the original").expect("original");
        std::fs::write(&first, b"one").expect("copy");
        std::fs::write(&second, b"two").expect("copy");
        let document = edited(&base);
        for copy in [&first, &second] {
            assert!(remember_in(
                &store,
                &base,
                copy,
                document.base(),
                &document.composition()
            ));
        }

        forget_in(&store, &identity_at(&first).expect("an identity"));

        assert!(reopen_in(&store, &first, limits()).is_none());
        assert!(reopen_in(&store, &second, limits()).is_some());
    }

    #[test]
    fn a_store_that_cannot_be_read_is_never_saved_over() {
        let scratch = Scratch::new("unreadable");
        let store = scratch.0.join("edits.tsv");
        let base = scratch.0.join("foto.png");
        let copy = scratch.0.join("copy.png");
        std::fs::write(&base, b"the original").expect("original");
        std::fs::write(&copy, b"the copy").expect("copy");
        // Past the byte budget: the load refuses it.
        let file = std::fs::File::create(&store).expect("store");
        file.set_len(17 * 1024 * 1024).expect("an oversized store");
        let document = edited(&base);

        assert!(!remember_in(
            &store,
            &base,
            &copy,
            document.base(),
            &document.composition()
        ));
        assert_eq!(
            std::fs::metadata(&store).expect("still there").len(),
            17 * 1024 * 1024,
            "the unreadable store was replaced"
        );
    }

    #[test]
    fn a_store_this_build_cannot_read_keeps_every_recipe_in_it() {
        // Another build's format, or bytes that are not text: remembering one
        // copy must not write a store of one recipe over all of them.
        let scratch = Scratch::new("foreign");
        let store = scratch.0.join("edits.tsv");
        let base = scratch.0.join("foto.png");
        let copy = scratch.0.join("copy.png");
        std::fs::write(&base, b"the original").expect("original");
        std::fs::write(&copy, b"the copy").expect("copy");
        let document = edited(&base);

        for foreign in [
            &b"fluorita-edits 9\nE\t1\t2\n"[..],
            &b"fluorita-edits 1\n\xff\n"[..],
        ] {
            std::fs::write(&store, foreign).expect("store");
            assert!(!remember_in(
                &store,
                &base,
                &copy,
                document.base(),
                &document.composition()
            ));
            assert_eq!(std::fs::read(&store).expect("still there"), foreign);
        }
    }
}
