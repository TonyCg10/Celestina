//! What the polkit duty relies on from the build (CEL-2-I; design S-8).
//!
//! The duty's behaviour is tested next to the code, where the seams are:
//! `src/polkit/duty/tests.rs` (the live switch, registration, refusal and the
//! dead bus), `src/polkit/state/tests.rs` (teardown with an answer in flight)
//! and `src/polkit/dumpable.rs` (the non-dumpable window). This file holds the
//! one check that needs the manifest.

use std::path::Path;

/// S-8: every path wipes the secret by ownership ending, so a panicking
/// worker must unwind. `panic = "abort"` in any profile would skip every
/// destructor. The assertion reads the profile the shell is built with: the
/// workspace manifest, which is where `cargo build --release` takes it from.
#[test]
fn the_shell_unwinds_so_ownership_ending_wipes_the_secret() {
    // Checked when the file compiles: a build that aborts fails here.
    const { assert!(cfg!(panic = "unwind")) };
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let text = match std::fs::read_to_string(&manifest) {
        Ok(text) => text,
        Err(err) => panic!("the workspace manifest is readable: {err}"),
    };
    for (number, line) in text.lines().enumerate() {
        let code = line.split('#').next().unwrap_or_default();
        let squeezed: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            !squeezed.starts_with("panic=") || !squeezed.contains("abort"),
            "Cargo.toml:{}: a profile aborts on panic, which skips the secret's destructors",
            number + 1
        );
    }
}
