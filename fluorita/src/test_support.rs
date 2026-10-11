//! What the unit tests of several modules share: a scratch folder that
//! removes itself, and the fixture picture copied into it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A folder under the system's temporary directory, unique to one test,
/// removed with everything in it when the test ends.
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "fluorita-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("a scratch folder");
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// The fixture picture (32 × 24 PNG) copied to `relative` inside this
    /// folder, its parents made.
    pub fn picture(&self, relative: &str) -> PathBuf {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("the picture's folder");
        }
        std::fs::copy(fixture_picture(), &path).expect("the fixture picture");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `tests/fixtures/picture.png`, a 32 × 24 RGB PNG.
pub fn fixture_picture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/picture.png")
}
