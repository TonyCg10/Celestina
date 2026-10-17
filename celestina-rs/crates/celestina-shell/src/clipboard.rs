// Copied from `celestina-rs/crates/celestina-shell-core/src/clipboard.rs` at
// 5924ad457b55d56dd52a7c7d5c69bc255117c5c4, adapted for CEL-2-G
// (`celestina/docs/evidence/2026-10-17-clipboard-privacy-design.md`):
// `MAX_ENTRY_BYTES` became `MAX_TEXT_BYTES`, `MAX_ENTRIES` became
// `MAX_TEXT_ENTRIES`, `SENSITIVE_MIME` became the `EXCLUSION_HINTS` table and
// `is_sensitive` became `is_excluded` (now ASCII case-insensitive), and
// `is_recordable` became `is_recordable_text`. The history holds `Entry`
// values (text or PNG bytes, with a capture time) instead of bare strings,
// with a separate image cap, seven-day expiry and `select`. Two things of the
// halted shell are deliberately not carried over: its persisted plaintext had
// no expiry and no bound on what it wrote, and its echo was a
// skip-the-next-selection flag (the echo is now the worker's, by source
// identity). The persisted form (`encode`, `decode`), the MIME choice and the
// preview are new.

//! What the clipboard history may keep, for how long, and in what shape it
//! may reach a disk.
//!
//! The clipboard is where passwords and tokens pass through, so every rule
//! here errs on the side of keeping less. Watching the selection over
//! `ext-data-control-v1` is the product's business; deciding whether a
//! selection belongs in history, how the list behaves once it does, and what
//! of it is persisted is this file's. It is pure so every rule is testable
//! without a compositor or a filesystem: time is always passed in as plain
//! seconds and never read here.
//!
//! Nothing in this module prints the contents of an entry. `Debug` reports a
//! kind and a length, and no error carries a byte of what it failed to read.

use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::fmt;
use std::hash::BuildHasher;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::bounded::bounded;

/// A selection this large is not something a person copied on purpose; it is
/// a document, or a lossy decode of something that was never text.
pub const MAX_TEXT_BYTES: usize = 256 * 1024;
/// How many text entries the history keeps, most recent first.
pub const MAX_TEXT_ENTRIES: usize = 200;
/// The most `image/png` bytes one image entry may hold.
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
/// How many image entries the history keeps. Images live in memory only.
pub const MAX_IMAGE_ENTRIES: usize = 20;
/// How long an image read from the selection owner may take before the
/// selection is dropped.
pub const IMAGE_READ_DEADLINE: Duration = Duration::from_secs(2);
/// How long an entry lives, in seconds: seven days. A secret that slipped
/// past the exclusion hints is gone after this, from memory and from disk.
pub const MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;
/// The longest preview of one text entry, in the UTF-16 units
/// [`bounded`](crate::bounded::bounded) counts.
pub const PREVIEW_UNITS: usize = 160;
/// The most rows of the history published to a surface.
pub const MAX_PUBLISHED_ROWS: usize = 50;

/// The schema version of the persisted file. A file with any other version,
/// or none, is not loaded.
pub const SCHEMA_VERSION: u32 = 1;
/// The most JSON one persisted entry may take beyond its text's own bytes:
/// the keys, the quotes, the longest `u64` and the separating comma. An entry
/// whose escaped form does not fit is kept in memory and never written.
pub const ENTRY_OVERHEAD_BYTES: usize = 64;
/// The most JSON the file takes outside its entries.
const FILE_OVERHEAD_BYTES: usize = 64;
/// The most a persisted history may be read back from: what the largest
/// history [`encode`] can write occupies, never more. A larger file was
/// written by something else, and reading it whole to find that out is the
/// allocation the bound exists to refuse.
pub const MAX_PERSISTED_BYTES: u64 =
    (FILE_OVERHEAD_BYTES + MAX_TEXT_ENTRIES * (MAX_TEXT_BYTES + ENTRY_OVERHEAD_BYTES)) as u64;

/// MIME types a selection owner adds to say "do not keep this". A selection
/// that offers any of them is skipped without its contents ever being read.
///
/// Only hints with a primary source are listed (design §2):
///
/// - `x-kde-passwordManagerHint`: KeePassXC sets it, with the value
///   `secret`, on what it copies (`src/gui/Clipboard.cpp`), and KDE's Klipper
///   leaves such a selection out of its history (`HistoryModel::checkClipData`
///   in `klipper/historymodel.cpp` of plasma-workspace, which tests
///   `data->data("x-kde-passwordManagerHint") == "secret"`). Klipper reads
///   the value; this shell skips on the type alone, so nothing of the
///   selection is read at all.
pub const EXCLUSION_HINTS: &[&str] = &["x-kde-passwordManagerHint"];

/// The text MIME types read and offered, most preferred first.
pub const TEXT_MIMES: [&str; 2] = ["text/plain;charset=utf-8", "text/plain"];
/// The one image MIME type read and offered.
pub const PNG_MIME: &str = "image/png";
/// The eight bytes every PNG file starts with (PNG specification, §5.2).
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// Whether a selection's offered MIME types ask that it never be stored.
/// Compared without regard to ASCII case, as MIME types are.
#[must_use]
pub fn is_excluded<S: AsRef<str>>(mimes: &[S]) -> bool {
    mimes.iter().any(|mime| {
        EXCLUSION_HINTS
            .iter()
            .any(|hint| mime.as_ref().eq_ignore_ascii_case(hint))
    })
}

/// What to do with a selection, decided from its offered MIME types alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice<'a> {
    /// An exclusion hint is offered: read nothing.
    Excluded,
    /// Read this text MIME, spelled as the owner offered it.
    Text(&'a str),
    /// Read `image/png`, spelled as the owner offered it.
    Png(&'a str),
    /// Neither text nor PNG is offered: read nothing.
    Nothing,
}

/// Decides what to read of a selection. The exclusion hints win over
/// everything; text is preferred over an image when both are offered.
#[must_use]
pub fn choose<S: AsRef<str>>(mimes: &[S]) -> Choice<'_> {
    if is_excluded(mimes) {
        return Choice::Excluded;
    }
    let offered = |wanted: &str| {
        mimes
            .iter()
            .map(AsRef::as_ref)
            .find(|mime| mime.eq_ignore_ascii_case(wanted))
    };
    if let Some(mime) = TEXT_MIMES.into_iter().find_map(offered) {
        return Choice::Text(mime);
    }
    offered(PNG_MIME).map_or(Choice::Nothing, Choice::Png)
}

/// Whether `text` is worth remembering: not empty, not larger than a person
/// could plausibly have selected on purpose, and not carrying a NUL, the mark
/// of a lossy decode of something that was never text.
#[must_use]
pub fn is_recordable_text(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_TEXT_BYTES && !text.contains('\0')
}

/// Whether `bytes` may be kept as an image: within the byte bound and
/// starting with the PNG signature, because an owner may answer something
/// other than what it offered. Whether it decodes is the thumbnailer's test.
#[must_use]
pub fn is_recordable_png(bytes: &[u8]) -> bool {
    bytes.len() <= MAX_IMAGE_BYTES
        && bytes.len() > PNG_SIGNATURE.len()
        && bytes.starts_with(&PNG_SIGNATURE)
}

/// How far ahead of `now` a capture time may lie and still count as now, in
/// seconds: five minutes. The wall clock steps (NTP at early boot, a resume
/// that corrects it), and an entry captured just before a step back must not
/// vanish for it.
pub const FUTURE_GRACE_SECS: u64 = 5 * 60;

/// Whether an entry captured at `captured_at` has expired at `now`, both in
/// seconds of the same clock. An entry dies the moment it is
/// [`MAX_AGE_SECS`] old. A capture time more than [`FUTURE_GRACE_SECS`] after
/// `now` has also expired: a clock that went far back, or a file that claims
/// the future, must not buy an entry a longer life than the grace.
#[must_use]
pub fn is_expired(captured_at: u64, now: u64) -> bool {
    captured_at > now.saturating_add(FUTURE_GRACE_SECS)
        || now.saturating_sub(captured_at) >= MAX_AGE_SECS
}

/// The kind of an entry, for a caller that must not look at its contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Png,
}

#[derive(Clone, PartialEq, Eq)]
enum Payload {
    Text(String),
    Png(Vec<u8>),
}

/// What one entry holds. It can only be built through the recording tests,
/// so a `Content` is always something the history may keep.
#[derive(Clone, PartialEq, Eq)]
pub struct Content(Payload);

impl Content {
    /// Text that passes [`is_recordable_text`], or nothing.
    #[must_use]
    pub fn text(text: String) -> Option<Self> {
        is_recordable_text(&text).then_some(Self(Payload::Text(text)))
    }

    /// Bytes read for a text MIME: valid UTF-8 that passes
    /// [`is_recordable_text`], or nothing.
    #[must_use]
    pub fn text_from_bytes(bytes: Vec<u8>) -> Option<Self> {
        if bytes.len() > MAX_TEXT_BYTES {
            return None;
        }
        String::from_utf8(bytes).ok().and_then(Self::text)
    }

    /// Bytes read for `image/png` that pass [`is_recordable_png`], or nothing.
    #[must_use]
    pub fn png(bytes: Vec<u8>) -> Option<Self> {
        is_recordable_png(&bytes).then_some(Self(Payload::Png(bytes)))
    }

    #[must_use]
    pub fn kind(&self) -> Kind {
        match self.0 {
            Payload::Text(_) => Kind::Text,
            Payload::Png(_) => Kind::Png,
        }
    }

    /// The text, when this is a text entry.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match &self.0 {
            Payload::Text(text) => Some(text),
            Payload::Png(_) => None,
        }
    }

    /// The PNG bytes, when this is an image entry.
    #[must_use]
    pub fn as_png(&self) -> Option<&[u8]> {
        match &self.0 {
            Payload::Text(_) => None,
            Payload::Png(bytes) => Some(bytes),
        }
    }

    /// The size in bytes, which is all a log may say about it.
    #[must_use]
    pub fn len(&self) -> usize {
        match &self.0 {
            Payload::Text(text) => text.len(),
            Payload::Png(bytes) => bytes.len(),
        }
    }

    /// Never true: an empty selection is not recordable. Present because a
    /// `len` without it is a lint, not because a caller needs it.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The MIME types to offer when this entry becomes the selection again.
    #[must_use]
    pub fn mimes(&self) -> &'static [&'static str] {
        match self.0 {
            Payload::Text(_) => &TEXT_MIMES,
            Payload::Png(_) => &[PNG_MIME],
        }
    }
}

impl fmt::Debug for Content {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Content")
            .field("kind", &self.kind())
            .field("bytes", &self.len())
            .finish()
    }
}

/// One remembered selection and when it was captured, in seconds.
#[derive(Clone, PartialEq, Eq)]
pub struct Entry {
    content: Content,
    captured_at: u64,
}

impl Entry {
    #[must_use]
    pub fn content(&self) -> &Content {
        &self.content
    }

    #[must_use]
    pub fn captured_at(&self) -> u64 {
        self.captured_at
    }

    /// The bounded, whitespace-collapsed line a surface may show, or `None`
    /// for an image (whose preview is a thumbnail).
    #[must_use]
    pub fn preview(&self) -> Option<String> {
        self.content.as_text().map(preview)
    }
}

impl fmt::Debug for Entry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Entry")
            .field("kind", &self.content.kind())
            .field("bytes", &self.content.len())
            .field("captured_at", &self.captured_at)
            .finish()
    }
}

/// The text a surface may show for an entry: runs of whitespace collapsed to
/// one space, leading and trailing whitespace dropped, and the result bounded
/// to [`PREVIEW_UNITS`]. Stops reading once the bound is passed, so a large
/// entry costs no more than a short one.
#[must_use]
pub fn preview(text: &str) -> String {
    let mut collapsed = String::new();
    let mut units = 0;
    for word in text.split_whitespace() {
        if !collapsed.is_empty() {
            collapsed.push(' ');
            units += 1;
        }
        for character in word.chars() {
            if units > PREVIEW_UNITS {
                break;
            }
            collapsed.push(character);
            units += character.len_utf16();
        }
        if units > PREVIEW_UNITS {
            break;
        }
    }
    bounded(&collapsed, PREVIEW_UNITS)
}

/// The entry now at the front after [`History::select`], and whether the
/// list's order changed to put it there.
#[derive(Debug)]
pub struct Selected<'a> {
    pub entry: &'a Entry,
    pub changed: bool,
}

/// The remembered selections, most recent first, text and images together.
///
/// It is a list, not a queue: recording something already in it moves it to
/// the front rather than adding a second copy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct History {
    entries: Vec<Entry>,
}

impl History {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Records a new selection captured at `now`. Returns whether the list
    /// changed. Content already at the front changes nothing visible; its
    /// capture time is still refreshed, and a disk copy that keeps the older
    /// time only makes it expire sooner.
    pub fn record(&mut self, content: Content, now: u64) -> bool {
        if let Some(first) = self.entries.first_mut() {
            if first.content == content {
                first.captured_at = now;
                return false;
            }
        }
        if let Some(position) = self
            .entries
            .iter()
            .position(|entry| entry.content == content)
        {
            self.entries.remove(position);
        }
        self.entries.insert(
            0,
            Entry {
                content,
                captured_at: now,
            },
        );
        self.enforce_caps();
        true
    }

    /// Moves the entry at `index` to the front, as a re-copy does, refreshing
    /// its capture time. `None` when there is no such entry.
    pub fn select(&mut self, index: usize, now: u64) -> Option<Selected<'_>> {
        if index >= self.entries.len() {
            return None;
        }
        let mut entry = self.entries.remove(index);
        entry.captured_at = now;
        self.entries.insert(0, entry);
        self.entries.first().map(|entry| Selected {
            entry,
            changed: index != 0,
        })
    }

    /// Whether some entry holds exactly `content`.
    #[must_use]
    pub fn holds(&self, content: &Content) -> bool {
        self.entries.iter().any(|entry| &entry.content == content)
    }

    /// Removes one entry by its current position. Returns whether anything
    /// was there to remove.
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.entries.len() {
            return false;
        }
        self.entries.remove(index);
        true
    }

    /// Returns whether there was anything to clear.
    pub fn clear(&mut self) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        self.entries.clear();
        true
    }

    /// Drops every entry expired at `now`. Returns whether any was dropped.
    pub fn expire(&mut self, now: u64) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|entry| !is_expired(entry.captured_at, now));
        self.entries.len() != before
    }

    /// Keeps the newest [`MAX_TEXT_ENTRIES`] texts and the newest
    /// [`MAX_IMAGE_ENTRIES`] images.
    fn enforce_caps(&mut self) {
        let (mut texts, mut images) = (0, 0);
        self.entries.retain(|entry| {
            let (count, cap) = match entry.content.kind() {
                Kind::Text => (&mut texts, MAX_TEXT_ENTRIES),
                Kind::Png => (&mut images, MAX_IMAGE_ENTRIES),
            };
            *count += 1;
            *count <= cap
        });
    }
}

/// Why the persisted history could not be written. Carries no content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// The JSON writer refused an entry; only the entry's position is kept.
    Entry { index: usize },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Entry { index } => write!(formatter, "clipboard entry {index} did not serialise"),
        }
    }
}

impl std::error::Error for EncodeError {}

/// Why a persisted history was not loaded. The file is then replaced on the
/// next save.
///
/// It deliberately does not keep the JSON parser's error as its source: that
/// error's message quotes the offending value, which in this file is
/// clipboard text. Only the category and position survive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// Larger than [`MAX_PERSISTED_BYTES`].
    TooLarge { bytes: u64 },
    /// Not JSON, or not a JSON object with an `entries` array.
    Malformed { line: usize, column: usize },
    /// A `schema_version` other than [`SCHEMA_VERSION`], or none.
    UnknownSchema { version: Option<u64> },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { bytes } => write!(
                formatter,
                "clipboard history is {bytes} bytes, over the {MAX_PERSISTED_BYTES}-byte bound"
            ),
            Self::Malformed { line, column } => write!(
                formatter,
                "clipboard history is not a history file (line {line}, column {column})"
            ),
            Self::UnknownSchema {
                version: Some(version),
            } => {
                write!(formatter, "clipboard history has unknown schema {version}")
            }
            Self::UnknownSchema { version: None } => {
                write!(formatter, "clipboard history has no schema version")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

impl DecodeError {
    fn malformed(error: &serde_json::Error) -> Self {
        Self::Malformed {
            line: error.line(),
            column: error.column(),
        }
    }
}

#[derive(Serialize)]
struct EntryOut<'a> {
    text: &'a str,
    captured_at: u64,
}

#[derive(Deserialize)]
struct EntryIn {
    text: String,
    captured_at: u64,
}

/// The persisted form of `entries`: text only, newest first, as
/// `{"schema_version":1,"entries":[{"text":..,"captured_at":..},..]}`.
///
/// Images are never written. Entries expired at `now` are not written. An
/// entry whose escaped JSON exceeds `MAX_TEXT_BYTES + ENTRY_OVERHEAD_BYTES`
/// is not written either, so the file can never outgrow
/// [`MAX_PERSISTED_BYTES`] and [`decode`] can hold it to that bound.
///
/// # Errors
///
/// [`EncodeError`] if the JSON writer refuses an entry, which a `&str` and a
/// `u64` cannot cause but is not worth an `unwrap`.
pub fn encode(entries: &[Entry], now: u64) -> Result<Vec<u8>, EncodeError> {
    let mut out = format!(r#"{{"schema_version":{SCHEMA_VERSION},"entries":["#).into_bytes();
    let mut written = 0;
    for (index, entry) in entries.iter().enumerate() {
        if written == MAX_TEXT_ENTRIES {
            break;
        }
        let Some(text) = entry.content.as_text() else {
            continue;
        };
        if is_expired(entry.captured_at, now) {
            continue;
        }
        let encoded = serde_json::to_vec(&EntryOut {
            text,
            captured_at: entry.captured_at,
        })
        .map_err(|_| EncodeError::Entry { index })?;
        // The comma counts against the entry's own overhead.
        if encoded.len() + 1 > MAX_TEXT_BYTES + ENTRY_OVERHEAD_BYTES {
            continue;
        }
        if written > 0 {
            out.push(b',');
        }
        out.extend_from_slice(&encoded);
        written += 1;
    }
    out.extend_from_slice(b"]}\n");
    Ok(out)
}

/// What [`decode`] read back: the history, and how many of the file's
/// entries it left out (expired, unrecordable, duplicated, past the cap or
/// not a text entry), so a caller knows the file on disk still holds them and
/// should be rewritten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub history: History,
    pub dropped: usize,
}

/// A cheap identity of a text for finding duplicates: its length and a hash
/// of its bytes. Equal texts have equal keys; unequal texts compare their
/// bytes only when their keys collide.
fn dedupe_key(text: &str, state: &RandomState) -> (u64, usize) {
    (state.hash_one(text.as_bytes()), text.len())
}

/// Reads a persisted history back, applying every rule a live selection
/// passes: the byte bound on the whole file first, then for each entry the
/// text test, expiry at `now`, duplicates (the first, newest, wins) and the
/// text cap. An entry that fails any rule, or is not a text entry, is
/// dropped and counted; the rest of the file still loads.
///
/// Finding duplicates is linear in the file: each kept text is indexed by
/// [`dedupe_key`], and bytes are compared only on a key hit, so a hostile
/// file of near-identical large texts costs one hash per entry, not a
/// comparison with every entry before it.
///
/// # Errors
///
/// [`DecodeError`] when the file as a whole is not one to load: too large,
/// not a JSON object with an `entries` array, or another schema.
pub fn decode(bytes: &[u8], now: u64) -> Result<Decoded, DecodeError> {
    let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if length > MAX_PERSISTED_BYTES {
        return Err(DecodeError::TooLarge { bytes: length });
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|error| DecodeError::malformed(&error))?;
    let Value::Object(mut file) = value else {
        return Err(DecodeError::Malformed { line: 0, column: 0 });
    };
    let version = file.get("schema_version").and_then(Value::as_u64);
    if version != Some(u64::from(SCHEMA_VERSION)) {
        return Err(DecodeError::UnknownSchema { version });
    }
    let Some(Value::Array(raw)) = file.remove("entries") else {
        return Err(DecodeError::Malformed { line: 0, column: 0 });
    };

    let total = raw.len();
    let state = RandomState::new();
    let mut seen: HashMap<(u64, usize), Vec<usize>> = HashMap::new();
    let mut history = History::new();
    for raw in raw {
        if history.entries.len() == MAX_TEXT_ENTRIES {
            break;
        }
        let Ok(entry) = serde_json::from_value::<EntryIn>(raw) else {
            continue;
        };
        if is_expired(entry.captured_at, now) {
            continue;
        }
        let Some(content) = Content::text(entry.text) else {
            continue;
        };
        let Some(text) = content.as_text() else {
            continue;
        };
        let key = dedupe_key(text, &state);
        let kept = seen.entry(key).or_default();
        if kept
            .iter()
            .any(|&index| history.entries[index].content == content)
        {
            continue;
        }
        kept.push(history.entries.len());
        history.entries.push(Entry {
            content,
            captured_at: entry.captured_at,
        });
    }
    let dropped = total - history.entries.len();
    Ok(Decoded { history, dropped })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An arbitrary fixed "now", well past the epoch, in seconds.
    const NOW: u64 = 1_800_000_000;

    fn text(value: &str) -> Content {
        Content::text(value.to_owned()).expect("recordable test text")
    }

    /// A minimal byte string that passes the PNG test; `tag` keeps two apart.
    fn png(tag: u8) -> Content {
        let mut bytes = PNG_SIGNATURE.to_vec();
        bytes.push(tag);
        Content::png(bytes).expect("recordable test PNG")
    }

    fn texts(history: &History) -> Vec<&str> {
        history
            .entries()
            .iter()
            .filter_map(|entry| entry.content().as_text())
            .collect()
    }

    // --- Exclusion hints: one test per table entry, plus the table itself. ---

    #[test]
    fn the_exclusion_table_is_exactly_the_cited_hints() {
        assert_eq!(EXCLUSION_HINTS, ["x-kde-passwordManagerHint"]);
    }

    #[test]
    fn the_kde_password_manager_hint_excludes_a_selection() {
        let hint = "x-kde-passwordManagerHint";
        assert!(is_excluded(&[hint]));
        assert!(is_excluded(&["text/plain", hint]));
        assert!(is_excluded(&["x-kde-passwordmanagerhint"]));
        assert_eq!(choose(&["text/plain", hint, PNG_MIME]), Choice::Excluded);
    }

    #[test]
    fn a_selection_without_a_hint_is_not_excluded() {
        assert!(!is_excluded(&["text/plain"]));
        assert!(!is_excluded::<&str>(&[]));
        assert!(!is_excluded(&["x-kde-passwordManagerHint-not"]));
    }

    // --- MIME choice. ---

    #[test]
    fn text_is_preferred_and_spelled_as_offered() {
        assert_eq!(
            choose(&["image/png", "text/plain", "TEXT/PLAIN;charset=UTF-8"]),
            Choice::Text("TEXT/PLAIN;charset=UTF-8")
        );
        assert_eq!(choose(&["text/plain"]), Choice::Text("text/plain"));
        assert_eq!(
            choose(&["text/html", "image/png"]),
            Choice::Png("image/png")
        );
    }

    #[test]
    fn a_selection_with_neither_text_nor_png_reads_nothing() {
        assert_eq!(choose(&["text/html", "image/jpeg"]), Choice::Nothing);
        assert_eq!(choose::<&str>(&[]), Choice::Nothing);
    }

    // --- Recording tests and bounds. ---

    #[test]
    fn the_bounds_are_the_designs() {
        assert_eq!(MAX_TEXT_ENTRIES, 200);
        assert_eq!(MAX_TEXT_BYTES, 256 * 1024);
        assert_eq!(MAX_IMAGE_ENTRIES, 20);
        assert_eq!(MAX_IMAGE_BYTES, 8 * 1024 * 1024);
        assert_eq!(IMAGE_READ_DEADLINE, Duration::from_secs(2));
        assert_eq!(MAX_AGE_SECS, 604_800);
        assert_eq!(PREVIEW_UNITS, 160);
        assert_eq!(MAX_PUBLISHED_ROWS, 50);
    }

    #[test]
    fn recordable_text_is_bounded_and_never_carries_a_nul() {
        assert!(is_recordable_text("copied text"));
        assert!(is_recordable_text(&"x".repeat(MAX_TEXT_BYTES)));
        assert!(!is_recordable_text(""));
        assert!(!is_recordable_text(&"x".repeat(MAX_TEXT_BYTES + 1)));
        assert!(!is_recordable_text("bad\0decode"));
    }

    #[test]
    fn text_from_bytes_must_be_utf8_and_recordable() {
        assert!(Content::text_from_bytes(b"plain".to_vec()).is_some());
        assert!(Content::text_from_bytes(vec![0xff, 0xfe]).is_none());
        assert!(Content::text_from_bytes(Vec::new()).is_none());
        assert!(Content::text_from_bytes(b"a\0b".to_vec()).is_none());
        assert!(Content::text_from_bytes(vec![b'x'; MAX_TEXT_BYTES + 1]).is_none());
    }

    #[test]
    fn recordable_png_is_bounded_and_carries_the_signature() {
        let mut largest = PNG_SIGNATURE.to_vec();
        largest.resize(MAX_IMAGE_BYTES, 0);
        assert!(is_recordable_png(&largest));
        largest.push(0);
        assert!(!is_recordable_png(&largest));
        assert!(!is_recordable_png(&PNG_SIGNATURE));
        assert!(!is_recordable_png(b""));
        assert!(!is_recordable_png(b"GIF89a, not what was offered"));
        assert!(Content::png(b"\x89PNG but truncated".to_vec()).is_none());
    }

    // --- Expiry. ---

    #[test]
    fn an_entry_expires_exactly_at_seven_days() {
        assert!(!is_expired(NOW, NOW));
        assert!(!is_expired(NOW - (MAX_AGE_SECS - 1), NOW));
        assert!(is_expired(NOW - MAX_AGE_SECS, NOW));
        assert!(is_expired(0, NOW));
    }

    #[test]
    fn a_capture_time_beyond_the_future_grace_has_expired() {
        assert_eq!(FUTURE_GRACE_SECS, 300);
        assert!(
            !is_expired(NOW + 1, NOW),
            "a small clock step back is forgiven"
        );
        assert!(!is_expired(NOW + FUTURE_GRACE_SECS, NOW));
        assert!(is_expired(NOW + FUTURE_GRACE_SECS + 1, NOW));
        assert!(is_expired(u64::MAX, NOW));
        assert!(!is_expired(u64::MAX, u64::MAX));
        assert!(!is_expired(5, 0), "an early-boot clock at zero");
    }

    #[test]
    fn expire_drops_old_entries_and_reports_it() {
        let mut history = History::new();
        history.record(text("old"), NOW - MAX_AGE_SECS);
        history.record(text("young"), NOW - 1);
        assert!(history.expire(NOW));
        assert_eq!(texts(&history), ["young"]);
        assert!(!history.expire(NOW));
    }

    // --- The list. ---

    #[test]
    fn recording_the_same_content_again_moves_it_to_the_front() {
        let mut history = History::new();
        assert!(history.record(text("one"), NOW));
        assert!(history.record(text("two"), NOW + 1));
        assert!(history.record(text("one"), NOW + 2));
        assert_eq!(texts(&history), ["one", "two"]);
        assert_eq!(history.entries()[0].captured_at(), NOW + 2);
        assert!(history.record(png(1), NOW + 3));
        assert!(history.record(png(2), NOW + 4));
        assert!(history.record(png(1), NOW + 5));
        assert_eq!(history.len(), 4);
        assert_eq!(history.entries()[0].content(), &png(1));
    }

    #[test]
    fn recording_the_front_again_is_not_a_change_but_refreshes_its_time() {
        let mut history = History::new();
        assert!(history.record(text("one"), NOW));
        assert!(!history.record(text("one"), NOW + 9));
        assert_eq!(history.len(), 1);
        assert_eq!(history.entries()[0].captured_at(), NOW + 9);
    }

    #[test]
    fn text_and_image_caps_are_independent() {
        let mut history = History::new();
        for index in 0..MAX_IMAGE_ENTRIES + 3 {
            let tag = u8::try_from(index).expect("small index");
            history.record(png(tag), NOW);
        }
        for index in 0..MAX_TEXT_ENTRIES + 10 {
            history.record(text(&format!("entry-{index}")), NOW);
        }
        let kinds = |kind| {
            history
                .entries()
                .iter()
                .filter(|e| e.content().kind() == kind)
                .count()
        };
        assert_eq!(kinds(Kind::Text), MAX_TEXT_ENTRIES);
        assert_eq!(kinds(Kind::Png), MAX_IMAGE_ENTRIES);
        assert_eq!(
            texts(&history)[0],
            format!("entry-{}", MAX_TEXT_ENTRIES + 9)
        );
        // The three oldest images were evicted, the newest kept.
        assert!(history.entries().iter().any(|e| e.content() == &png(22)));
        assert!(!history.entries().iter().any(|e| e.content() == &png(2)));
    }

    #[test]
    fn select_moves_an_entry_to_the_front_and_reports_whether_it_moved() {
        let mut history = History::new();
        history.record(text("one"), NOW);
        history.record(text("two"), NOW);
        let selected = history.select(1, NOW + 5).expect("entry 1 exists");
        assert!(selected.changed);
        assert_eq!(selected.entry.content().as_text(), Some("one"));
        assert_eq!(selected.entry.captured_at(), NOW + 5);
        assert_eq!(texts(&history), ["one", "two"]);
        let selected = history.select(0, NOW + 6).expect("entry 0 exists");
        assert!(!selected.changed);
        assert!(history.select(2, NOW).is_none());
    }

    #[test]
    fn removing_and_clearing_report_whether_anything_happened() {
        let mut history = History::new();
        assert!(!history.remove(0));
        assert!(!history.clear());
        history.record(text("one"), NOW);
        history.record(text("two"), NOW);
        assert!(history.remove(1));
        assert_eq!(texts(&history), ["two"]);
        assert!(!history.remove(5));
        assert!(history.clear());
        assert!(history.is_empty());
    }

    #[test]
    fn an_image_offers_png_and_text_offers_the_text_mimes() {
        assert_eq!(png(0).mimes(), [PNG_MIME]);
        assert_eq!(text("a").mimes(), TEXT_MIMES);
    }

    // --- Preview. ---

    #[test]
    fn a_preview_collapses_whitespace_and_is_bounded() {
        assert_eq!(preview("  one\n\ttwo   three \r\n"), "one two three");
        let long = "word ".repeat(MAX_TEXT_BYTES / 5);
        let shown = preview(&long);
        assert!(shown.encode_utf16().count() <= PREVIEW_UNITS);
        assert!(shown.encode_utf16().count() >= PREVIEW_UNITS - 1);
        let astral = "\u{1f600}".repeat(200);
        assert_eq!(preview(&astral).chars().count(), PREVIEW_UNITS / 2);
        assert_eq!(png(0).len(), 9);
        let mut history = History::new();
        history.record(png(0), NOW);
        assert_eq!(history.entries()[0].preview(), None);
    }

    // --- No content in any Debug, Display or error output. ---

    #[test]
    fn debug_and_error_output_carry_lengths_never_content() {
        let secret = "hunter2-correct-horse";
        let mut history = History::new();
        history.record(text(secret), NOW);
        history.record(png(7), NOW);
        let selected = format!("{:?}", history.select(1, NOW));
        let printed = format!(
            "{:?} {:?} {:?} {selected}",
            history,
            history.entries()[1],
            history.entries()[1].content(),
        );
        assert!(!printed.contains(secret), "{printed}");
        assert!(!printed.contains("hunter"), "{printed}");
        assert!(
            printed.contains(&format!("bytes: {}", secret.len())),
            "{printed}"
        );
        // Not even the PNG bytes, as numbers.
        assert!(!printed.contains("137"), "{printed}");

        // A value of the wrong type makes serde_json quote it; the decode
        // error must not.
        let file = format!(r#"{{"schema_version":"{secret}","entries":[]}}"#);
        let error = decode(file.as_bytes(), NOW).expect_err("a string schema");
        let broken = format!(r#"{{"schema_version":1,"entries":["{secret}"#);
        let malformed = decode(broken.as_bytes(), NOW).expect_err("truncated");
        for message in [
            format!("{error} {error:?}"),
            format!("{malformed} {malformed:?}"),
            format!("{0} {0:?}", EncodeError::Entry { index: 3 }),
        ] {
            assert!(!message.contains("hunter"), "{message}");
        }
    }

    // --- Persistence. ---

    #[test]
    fn encode_writes_text_only_and_decode_reads_it_back() {
        let mut history = History::new();
        history.record(text("older"), NOW - 10);
        history.record(png(1), NOW - 5);
        history.record(text("newer \"quoted\"\n"), NOW - 1);
        let bytes = encode(history.entries(), NOW).expect("encodes");
        let written = String::from_utf8(bytes.clone()).expect("UTF-8");
        assert!(written.starts_with(r#"{"schema_version":1,"entries":["#));
        assert!(
            !written.contains("PNG") && !written.contains("png"),
            "{written}"
        );
        let read = decode(&bytes, NOW).expect("decodes");
        assert_eq!(read.dropped, 0);
        let read = read.history;
        assert_eq!(texts(&read), ["newer \"quoted\"\n", "older"]);
        assert_eq!(read.entries()[1].captured_at(), NOW - 10);
        assert_eq!(
            decode(&encode(&[], NOW).expect("empty"), NOW),
            Ok(Decoded {
                history: History::new(),
                dropped: 0
            })
        );
    }

    #[test]
    fn encode_drops_expired_entries() {
        let mut history = History::new();
        history.record(text("stale"), NOW - MAX_AGE_SECS);
        history.record(text("fresh"), NOW);
        let read = decode(&encode(history.entries(), NOW).expect("encodes"), NOW)
            .expect("ok")
            .history;
        assert_eq!(texts(&read), ["fresh"]);
    }

    #[test]
    fn the_largest_history_fits_the_read_bound_and_heavy_escaping_is_not_written() {
        let mut history = History::new();
        for index in 0..MAX_TEXT_ENTRIES {
            // The index leads, so telling entries apart is cheap.
            let mut value = format!("{index:04}");
            value.push_str(&"x".repeat(MAX_TEXT_BYTES - 4));
            history.record(text(&value), u64::MAX - 1);
        }
        let bytes = encode(history.entries(), u64::MAX - 1).expect("encodes");
        assert!(u64::try_from(bytes.len()).expect("fits") <= MAX_PERSISTED_BYTES);
        assert_eq!(
            decode(&bytes, u64::MAX - 1).expect("reads").history.len(),
            MAX_TEXT_ENTRIES
        );

        // Control characters escape to six bytes each: such an entry stays in
        // memory and is not written, so the file never outgrows the bound.
        let mut history = History::new();
        history.record(text(&"\u{1}".repeat(MAX_TEXT_BYTES)), NOW);
        history.record(text("small"), NOW);
        let read = decode(&encode(history.entries(), NOW).expect("encodes"), NOW)
            .expect("ok")
            .history;
        assert_eq!(texts(&read), ["small"]);
    }

    #[test]
    fn a_file_over_the_bound_is_refused_before_parsing() {
        let mut huge = br#"{"schema_version":1,"entries":[]}"#.to_vec();
        huge.resize(
            usize::try_from(MAX_PERSISTED_BYTES).expect("fits") + 1,
            b' ',
        );
        assert!(matches!(
            decode(&huge, NOW),
            Err(DecodeError::TooLarge { .. })
        ));
    }

    #[test]
    fn a_file_of_another_schema_or_shape_is_not_loaded() {
        for (bytes, expected) in [
            (
                &br#"{"entries":[]}"#[..],
                DecodeError::UnknownSchema { version: None },
            ),
            (
                br#"{"schema_version":2,"entries":[]}"#,
                DecodeError::UnknownSchema { version: Some(2) },
            ),
            (
                br#"{"schema_version":0,"entries":[]}"#,
                DecodeError::UnknownSchema { version: Some(0) },
            ),
            (
                br#"{"schema_version":1}"#,
                DecodeError::Malformed { line: 0, column: 0 },
            ),
            (
                br#"{"schema_version":1,"entries":{}}"#,
                DecodeError::Malformed { line: 0, column: 0 },
            ),
            (br#"[1,2]"#, DecodeError::Malformed { line: 0, column: 0 }),
        ] {
            assert_eq!(
                decode(bytes, NOW),
                Err(expected),
                "{}",
                String::from_utf8_lossy(bytes)
            );
        }
        assert!(matches!(
            decode(b"not json", NOW),
            Err(DecodeError::Malformed { .. })
        ));
        assert!(matches!(
            decode(b"", NOW),
            Err(DecodeError::Malformed { .. })
        ));
    }

    // The file is ordinary bytes any process of the person could write: every
    // rule a live selection passes is applied again, entry by entry.
    #[test]
    fn a_hostile_file_keeps_only_what_would_have_been_recorded() {
        let oversize = "x".repeat(MAX_TEXT_BYTES + 1);
        let file = serde_json::json!({
            "schema_version": 1,
            "unknown": true,
            "entries": [
                {"text": "keep me", "captured_at": NOW - 1},
                {"text": "", "captured_at": NOW},
                {"text": oversize, "captured_at": NOW},
                {"text": "nul\u{0}inside", "captured_at": NOW},
                {"text": "expired", "captured_at": NOW - MAX_AGE_SECS},
                {"text": "from the future", "captured_at": NOW + FUTURE_GRACE_SECS + 1},
                {"text": "within the grace", "captured_at": NOW + 1},
                {"text": "no time"},
                {"text": "negative", "captured_at": -1},
                {"text": 42, "captured_at": NOW},
                {"png": "iVBORw0KGgo=", "captured_at": NOW},
                "a bare string",
                null,
                {"text": "keep me", "captured_at": NOW - 2},
                {"text": "keep me too", "captured_at": NOW - 3},
            ],
        });
        let decoded = decode(file.to_string().as_bytes(), NOW).expect("the file loads");
        assert_eq!(decoded.dropped, 12, "everything but the three kept");
        let read = decoded.history;
        assert_eq!(texts(&read), ["keep me", "within the grace", "keep me too"]);
        assert!(read
            .entries()
            .iter()
            .all(|entry| entry.content().kind() == Kind::Text));
    }

    #[test]
    fn a_file_with_too_many_entries_is_truncated_to_the_cap() {
        let entries: Vec<Value> = (0..MAX_TEXT_ENTRIES + 50)
            .map(|index| serde_json::json!({"text": format!("e{index}"), "captured_at": NOW}))
            .collect();
        let file = serde_json::json!({"schema_version": 1, "entries": entries});
        let decoded = decode(file.to_string().as_bytes(), NOW).expect("loads");
        assert_eq!(decoded.dropped, 50);
        let read = decoded.history;
        assert_eq!(read.len(), MAX_TEXT_ENTRIES);
        assert_eq!(texts(&read)[0], "e0");
    }

    // A file of the largest texts that differ only at their very end: the
    // duplicate test indexes each kept text by its hash and length, so this
    // costs one hash per entry instead of a comparison with every earlier
    // entry. Exact duplicates are still found among them.
    #[test]
    fn near_identical_large_texts_are_deduplicated_in_linear_time() {
        let body = "x".repeat(MAX_TEXT_BYTES - 4);
        let mut entries: Vec<Value> = (0..MAX_TEXT_ENTRIES - 1)
            .map(|index| serde_json::json!({"text": format!("{body}{index:04}"), "captured_at": NOW}))
            .collect();
        entries.insert(
            1,
            serde_json::json!({"text": format!("{body}0000"), "captured_at": NOW}),
        );
        let file = serde_json::json!({"schema_version": 1, "entries": entries});
        let bytes = file.to_string();
        let started = std::time::Instant::now();
        let decoded = decode(bytes.as_bytes(), NOW).expect("loads");
        let took = started.elapsed();
        assert_eq!(decoded.history.len(), MAX_TEXT_ENTRIES - 1);
        assert_eq!(decoded.dropped, 1, "the exact duplicate");
        // Generous: quadratic comparison of 200 x 256 KiB is ~5 GB of memcmp.
        assert!(took < std::time::Duration::from_secs(5), "{took:?}");
    }

    #[test]
    fn holds_tells_whether_some_entry_has_the_content() {
        let mut history = History::new();
        history.record(text("one"), NOW);
        assert!(history.holds(&text("one")));
        assert!(!history.holds(&text("two")));
        assert!(!history.holds(&png(1)));
    }
}
