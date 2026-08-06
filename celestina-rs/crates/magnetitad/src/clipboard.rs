//! Wayland clipboard process adaptation for the daemon.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::{Duration, Instant};

use crate::subprocess;

/// How long one clipboard tool may take. [`read`] and [`write`] run on the
/// thread pumping a phone link, so an unresponsive compositor tool must cost
/// that link a bounded pause and nothing more.
const CLIPBOARD_TIMEOUT: Duration = Duration::from_secs(2);

/// Every clipboard read is restricted to text. `wl-paste` otherwise hands back
/// whatever the selection offers — the raw bytes of a copied image, which no
/// later layer can tell apart from text a person copied.
const TEXT_TYPE: &str = "text";

/// Watch desktop clipboard changes without tying this adapter to daemon state.
///
/// The watched command is a *notification*, not a transport: it drains the
/// value `wl-paste` hands it and emits one marker byte, and the value itself
/// always comes from [`read`]. Nothing is framed between the two processes, so
/// no clipboard payload can be mistaken for several changes — a separator
/// scheme has to assume some byte never appears in the content, and clipboard
/// content is exactly where that assumption does not hold. Reading the current
/// value on each marker also coalesces a burst by construction.
///
/// Missing tools are a best-effort degradation.
pub(crate) fn spawn_watch(on_change: impl Fn(String) + Send + 'static) {
    thread::spawn(move || {
        let mut child = match Command::new("wl-paste")
            // `--watch` must come last: wl-paste takes everything after it as
            // the command to run.
            .args([
                "--type",
                TEXT_TYPE,
                "--watch",
                "sh",
                "-c",
                "cat >/dev/null; printf .",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                eprintln!("magnetitad: clipboard watch unavailable: {error}");
                return;
            }
        };
        let Some(mut stdout) = child.stdout.take() else {
            return;
        };
        let mut markers = [0u8; 64];
        loop {
            match stdout.read(&mut markers) {
                Ok(0) => break,
                Ok(_) => {
                    if let Some(text) = read() {
                        on_change(text);
                    }
                }
                Err(_) => break,
            }
        }
    });
}

/// Read the current desktop clipboard text, or `None` when it holds nothing
/// this daemon may sync. Invalid bytes are refused rather than replaced: a
/// lossy decode of a binary selection yields a string of replacement characters
/// that still carries the original NULs and control bytes, which is a worse
/// input to every layer above than having no value at all. What remains still
/// has to satisfy the domain's own rule, so a caller never has to re-check it.
pub(crate) fn read() -> Option<String> {
    let stopping = AtomicBool::new(false);
    let output = subprocess::command_output_from(
        "wl-paste",
        &["--no-newline", "--type", TEXT_TYPE],
        Instant::now() + CLIPBOARD_TIMEOUT,
        &stopping,
    )?;
    decode(output).filter(|text| magnetita_core::clipboard::is_syncable(text))
}

/// The decode half of [`read`], separated so the rule can be tested without a
/// compositor.
fn decode(bytes: Vec<u8>) -> Option<String> {
    String::from_utf8(bytes).ok()
}

/// Put text on the desktop, bounded and with the whole process group reaped.
pub(crate) fn write(text: &str) -> bool {
    let stopping = AtomicBool::new(false);
    subprocess::run_with_input(
        "wl-copy",
        &[],
        text.as_bytes(),
        Instant::now() + CLIPBOARD_TIMEOUT,
        &stopping,
    )
    .succeeded
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn valid_utf8_decodes_to_itself() {
        assert_eq!(
            decode("hola\nmundo ñ €".as_bytes().to_vec()).as_deref(),
            Some("hola\nmundo ñ €")
        );
    }

    #[test]
    fn an_empty_read_decodes_to_empty_text() {
        assert_eq!(decode(Vec::new()).as_deref(), Some(""));
    }

    #[test]
    fn binary_is_refused_instead_of_decoded_lossily() {
        // The head of a PNG: an invalid sequence followed by the NUL bytes a
        // lossy decode would have carried through into a "text" clipboard.
        let png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00@";
        assert!(decode(png.to_vec()).is_none());
    }
}
