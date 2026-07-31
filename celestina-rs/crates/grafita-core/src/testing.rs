//! Scratch directories for the crate's own unit tests.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(1);

/// Creates an empty directory this test owns exclusively.
pub(crate) fn scratch_directory(label: &str) -> PathBuf {
    let sequence = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "grafita-core-{label}-{}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("a scratch directory for this test");
    path
}
