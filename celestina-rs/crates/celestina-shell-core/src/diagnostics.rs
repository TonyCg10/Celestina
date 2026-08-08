//! A journal that can be read after the machine stopped.
//!
//! This shell has been running on a machine whose GPU has more than once been
//! lost from the PCIe bus — `amdgpu 0000:03:00.0: device lost from bus!` — most
//! recently with Celestina inside a *nested* Niri session, which separated the
//! surfaces and shared the GPU, the VCN block, the DDC/I²C buses and the session
//! bus with everything else. Nothing here claims this shell caused that. It
//! answers a smaller and answerable question: after the freeze and the physical
//! reset, **nobody could say what Celestina had been doing**.
//!
//! That was a defect of this shell's own. The record was scattered Qt messages
//! in the host, `eprintln!` in the helpers, discarded `stderr` in several
//! external processes, an optional trace variable that is normally off, and no
//! identity tying any of it together. A run started from a terminal reaches
//! journald only if something happened to be capturing it, and a freeze plus a
//! reset loses whatever was only ever in a buffer.
//!
//! # What this module owns, and what it refuses to own
//!
//! This module is pure. It owns the vocabulary — levels, components, event
//! names, the correlation fields, the bounds, the redaction rules, the exact
//! text of one line and the rotation arithmetic — and it touches no filesystem,
//! no clock and no thread. The sink that performs the writing lives with the
//! runtime that can block; the host has its own writer for the same contract,
//! because it is a different process in a different language that must still be
//! able to record a helper's death after that helper is gone.
//!
//! # Privacy is structural, not a convention
//!
//! A journal that could record a clipboard would be worse than no journal. So
//! the shapes here cannot carry content. Anything derived from something a
//! person wrote, received, played or opened enters as [`Redaction`], which is a
//! size and nothing else — there is no constructor that keeps the text.
//!
//! Content is **not** hashed. A hash of a short string is not irreversible in
//! practice: a window title, a track name or an SSID is guessable, and a digest
//! would invite exactly the brute force it appears to prevent. What a diagnosis
//! actually needs is whether something was present, how big it was and how often
//! it happened, and those are recorded exactly.
//!
//! Technical identities that a diagnosis cannot do without — an output name, a
//! DDC bus, a provider key, a process name, a bus name — are recorded, because
//! they are the identities the failure is described in.

use std::collections::VecDeque;

use crate::bounded;

/// The schema of one line. A reader that sees a higher number is looking at a
/// journal written by a newer shell and should say so rather than guess.
pub const SCHEMA_VERSION: u32 = 1;
/// The longest line that may be written. A line past this is truncated to a
/// bounded refusal rather than dropped, so the fact that something oversized
/// happened is itself never lost.
pub const MAX_LINE_BYTES: usize = 4 * 1024;
/// How large one journal file grows before it is rotated.
pub const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
/// How many journal files one component keeps, the live one included. Four
/// megabytes each across eight files bounds one component at 32 MiB.
pub const MAX_FILES: usize = 8;
/// How many events may wait to be written. Past this the drop policy applies.
pub const MAX_QUEUE: usize = 4096;
/// The longest technical string any field may carry: an output name, a process
/// name, a bus name, an error reason.
pub const MAX_TEXT_CHARS: usize = 160;
/// How many fields one event may carry beyond its identity.
pub const MAX_FIELDS: usize = 24;

/// How much an event matters, and what the sink owes it.
///
/// [`Level::Critical`] is not "very important". It is the class of event whose
/// loss would defeat the purpose of the journal — anything about a process, a
/// DDC operation, a helper's death, shutdown, or activity adjacent to the GPU —
/// and the sink flushes it to the disk rather than leaving it in a buffer that a
/// power cycle would take.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Critical,
}

impl Level {
    /// The token written to the file. Stable: a reader parses these.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }

    /// Whether the sink must reach the disk before returning.
    ///
    /// The whole failure this journal exists for takes the machine down without
    /// warning, so an event that describes a process, a bus or a shutdown is
    /// worth an `fsync` the ordinary flow of events is not.
    #[must_use]
    pub fn must_flush(self) -> bool {
        matches!(self, Self::Critical)
    }

    /// Whether the compact mirror should carry it.
    ///
    /// The mirror exists so journald catches what it can when the launch shape
    /// allows. It is deliberately quieter than the file: the file is the
    /// evidence, and a terminal full of trace lines is how people learn to
    /// ignore diagnostics.
    #[must_use]
    pub fn mirrored(self) -> bool {
        self >= Self::Warn
    }
}

/// Which process, and which part of it, produced an event.
///
/// A free string rather than an enum because the host is a different language
/// and must be able to name its own regions without this crate enumerating them.
/// Bounded like every other identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component(String);

impl Component {
    /// Names a component, bounded.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self(bounded(name, MAX_TEXT_CHARS))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The size of something this journal must never contain.
///
/// Clipboard text, a notification body, a track name, a window title, an SSID, a
/// launched command line, an external payload — all of them enter here and
/// nothing else. There is deliberately no accessor for the original, no
/// `Display`, and no digest: see the module documentation for why hashing was
/// rejected rather than merely omitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Redaction {
    chars: usize,
    bytes: usize,
}

impl Redaction {
    /// Measures a value and forgets it.
    #[must_use]
    pub fn of(value: &str) -> Self {
        Self {
            chars: value.chars().count(),
            bytes: value.len(),
        }
    }

    /// Measures raw bytes — an image, a frame, an external payload.
    #[must_use]
    pub fn of_bytes(value: &[u8]) -> Self {
        Self {
            chars: 0,
            bytes: value.len(),
        }
    }

    #[must_use]
    pub fn chars(self) -> usize {
        self.chars
    }

    #[must_use]
    pub fn bytes(self) -> usize {
        self.bytes
    }
}

/// One field's value. Every variant is either a number, a flag, or a bounded
/// technical identity; none of them can carry a person's content.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A bounded technical identity: an output, a bus, a provider key, a process
    /// name, an error reason. Never a title, a body or a command line.
    Text(String),
    Int(i64),
    Uint(u64),
    Bool(bool),
    /// Milliseconds. Written as a number so a reader can sort and subtract.
    Millis(u64),
    /// A size that stands in for something deliberately not recorded.
    Redacted(Redaction),
}

impl Value {
    /// A bounded technical identity.
    #[must_use]
    pub fn text(value: &str) -> Self {
        Self::Text(bounded(value, MAX_TEXT_CHARS))
    }
}

/// One journal event, before it is given an identity and a time.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    level: Level,
    name: String,
    fields: Vec<(String, Value)>,
}

impl Event {
    /// Names an event. The name is the class of thing that happened, in
    /// `area.thing` form — `process.exit`, `ddc.write.start`, `helper.death`.
    #[must_use]
    pub fn new(level: Level, name: &str) -> Self {
        Self {
            level,
            name: bounded(name, MAX_TEXT_CHARS),
            fields: Vec::new(),
        }
    }

    /// Adds one field. Past [`MAX_FIELDS`] the event keeps the fields it has:
    /// an event that grew a field too many is still worth its first twenty-four.
    #[must_use]
    pub fn with(mut self, key: &str, value: Value) -> Self {
        if self.fields.len() < MAX_FIELDS {
            self.fields.push((bounded(key, MAX_TEXT_CHARS), value));
        }
        self
    }

    /// Adds a bounded technical identity.
    #[must_use]
    pub fn with_text(self, key: &str, value: &str) -> Self {
        self.with(key, Value::text(value))
    }

    /// Records that something was present and how big it was, without recording
    /// it. This is how every private value reaches the journal.
    #[must_use]
    pub fn with_redacted(self, key: &str, value: &str) -> Self {
        self.with(key, Value::Redacted(Redaction::of(value)))
    }

    #[must_use]
    pub fn level(&self) -> Level {
        self.level
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn fields(&self) -> &[(String, Value)] {
        &self.fields
    }
}

/// Who is writing, and what ties their lines to everyone else's.
///
/// The `run_id` is generated once by the host and handed to both helpers in
/// their environment before they start, so three processes' files interleave
/// into one ordering. A helper started outside a host — by a test, or by hand —
/// generates its own, which is why a reader must key on `run_id` and never
/// assume there is only one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    run_id: String,
    component: Component,
    pid: u32,
    /// Which incarnation of this component the line belongs to. The host counts
    /// helper generations; a helper carries the one it was told.
    generation: u64,
}

impl Identity {
    #[must_use]
    pub fn new(run_id: &str, component: Component, pid: u32, generation: u64) -> Self {
        Self {
            run_id: bounded(run_id, MAX_TEXT_CHARS),
            component,
            pid,
            generation,
        }
    }

    #[must_use]
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    #[must_use]
    pub fn component(&self) -> &Component {
        &self.component
    }
}

/// When something happened, from two clocks that fail differently.
///
/// The wall clock names a moment a person and `journalctl` can both find. The
/// monotonic reading orders events even when the wall clock steps — and it is
/// the only one that keeps meaning across a resume, a time-zone change or an
/// NTP correction landing in the middle of the seconds being reconstructed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    /// Nanoseconds since the Unix epoch, UTC.
    pub wall_nanos: u128,
    /// Milliseconds since this process started.
    pub monotonic_millis: u64,
}

/// One rendered line, and whether the sink must reach the disk for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub flush: bool,
}

/// Renders one event as a single line of JSON.
///
/// The field names are the contract: the host's own writer emits exactly these,
/// so a reader merges three processes' files without knowing which wrote which.
///
/// A line that would exceed [`MAX_LINE_BYTES`] is replaced by a bounded record
/// that keeps the identity, the name and the fact of the overflow. Dropping it
/// would hide the one event that was unusual enough to be too big.
#[must_use]
pub fn render(identity: &Identity, event: &Event, stamp: Stamp, worker: &str) -> Line {
    let line = render_full(identity, event, stamp, worker);
    if line.len() <= MAX_LINE_BYTES {
        return Line {
            text: line,
            flush: event.level.must_flush(),
        };
    }

    let overflow = Event::new(event.level, event.name())
        .with("journal_overflow_bytes", Value::Uint(line.len() as u64))
        .with(
            "journal_overflow_fields",
            Value::Uint(event.fields.len() as u64),
        );
    Line {
        text: render_full(identity, &overflow, stamp, worker),
        flush: event.level.must_flush(),
    }
}

fn render_full(identity: &Identity, event: &Event, stamp: Stamp, worker: &str) -> String {
    let mut out = String::with_capacity(256);
    out.push('{');
    push_number(&mut out, "v", &SCHEMA_VERSION.to_string(), true);
    push_number(&mut out, "t", &stamp.wall_nanos.to_string(), false);
    push_number(
        &mut out,
        "mono_ms",
        &stamp.monotonic_millis.to_string(),
        false,
    );
    push_string(&mut out, "level", event.level.as_str(), false);
    push_string(&mut out, "component", identity.component.as_str(), false);
    push_string(&mut out, "event", &event.name, false);
    push_string(&mut out, "run_id", &identity.run_id, false);
    push_number(&mut out, "pid", &identity.pid.to_string(), false);
    push_number(
        &mut out,
        "generation",
        &identity.generation.to_string(),
        false,
    );
    if !worker.is_empty() {
        push_string(&mut out, "worker", &bounded(worker, MAX_TEXT_CHARS), false);
    }

    for (key, value) in &event.fields {
        match value {
            Value::Text(text) => push_string(&mut out, key, text, false),
            Value::Int(number) => push_number(&mut out, key, &number.to_string(), false),
            Value::Uint(number) => push_number(&mut out, key, &number.to_string(), false),
            Value::Millis(number) => push_number(&mut out, key, &number.to_string(), false),
            Value::Bool(flag) => {
                push_number(&mut out, key, if *flag { "true" } else { "false" }, false)
            }
            Value::Redacted(size) => {
                push_number(
                    &mut out,
                    &format!("{key}_chars"),
                    &size.chars.to_string(),
                    false,
                );
                push_number(
                    &mut out,
                    &format!("{key}_bytes"),
                    &size.bytes.to_string(),
                    false,
                );
            }
        }
    }

    out.push('}');
    out
}

fn push_string(out: &mut String, key: &str, value: &str, first: bool) {
    if !first {
        out.push(',');
    }
    push_json_string(out, key);
    out.push(':');
    push_json_string(out, value);
}

fn push_number(out: &mut String, key: &str, value: &str, first: bool) {
    if !first {
        out.push(',');
    }
    push_json_string(out, key);
    out.push(':');
    out.push_str(value);
}

/// Escapes one JSON string.
///
/// Written here rather than delegated because this crate's serializer works on
/// typed values and the line is assembled field by field, and because a journal
/// that could be broken by an unusual character in an output name would be
/// worthless exactly when something unusual is happening. Every control
/// character is escaped, so no producer's text can inject a second line.
fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

/// What the sink must do with the file before appending `incoming` bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    Append,
    Rotate,
}

/// Whether the next line fits in the current file.
///
/// Deterministic and arithmetic: no clocks, no directory listing, no size the
/// caller has to guess. A file that is already at or past the bound rotates even
/// for an empty line, so a file that grew for any other reason still gets closed.
#[must_use]
pub fn rotation(current_bytes: u64, incoming: usize) -> Rotation {
    if current_bytes == 0 {
        return Rotation::Append;
    }
    if current_bytes.saturating_add(incoming as u64) > MAX_FILE_BYTES {
        Rotation::Rotate
    } else {
        Rotation::Append
    }
}

/// Which of a component's files should be removed, newest first in the input.
///
/// Returns the surplus beyond [`MAX_FILES`]. The caller supplies the names in
/// the order it considers newest-to-oldest, because ordering by name is the
/// caller's business and this decides only how many survive.
#[must_use]
pub fn retire<'a>(newest_first: &[&'a str]) -> Vec<&'a str> {
    newest_first.iter().skip(MAX_FILES).copied().collect()
}

/// What happened to an event that was offered to a full queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
    /// Queued.
    Queued,
    /// Refused, and counted. The journal will say so.
    Dropped,
    /// Queued after evicting one ordinary event to make room for a critical one.
    Evicted,
}

/// The bounded queue and its explicit drop policy.
///
/// The policy in one sentence: **the journal never blocks and never grows, and
/// what it sacrifices first is the ordinary event.** A queue at capacity refuses
/// a new ordinary event and counts it; a critical event — a process, a DDC
/// operation, a helper's death, a shutdown — displaces the oldest ordinary one
/// instead, because those are the events the whole file exists for. A queue of
/// nothing but critical events refuses even a critical one rather than growing,
/// and counts that too.
///
/// Nothing is ever lost silently: [`Self::take_dropped`] hands the count to the
/// writer, which publishes it as its own event.
#[derive(Debug)]
pub struct Queue {
    waiting: VecDeque<(Event, Stamp, String)>,
    dropped: u64,
    capacity: usize,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new(MAX_QUEUE)
    }
}

impl Queue {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            waiting: VecDeque::new(),
            dropped: 0,
            capacity: capacity.max(1),
        }
    }

    /// Offers an event. Never blocks and never grows past the capacity.
    pub fn offer(&mut self, event: Event, stamp: Stamp, worker: &str) -> Admission {
        if self.waiting.len() < self.capacity {
            self.waiting
                .push_back((event, stamp, bounded(worker, MAX_TEXT_CHARS)));
            return Admission::Queued;
        }

        if !event.level.must_flush() {
            self.dropped += 1;
            return Admission::Dropped;
        }

        let ordinary = self
            .waiting
            .iter()
            .position(|(waiting, _, _)| !waiting.level.must_flush());
        match ordinary {
            Some(index) => {
                self.waiting.remove(index);
                self.dropped += 1;
                self.waiting
                    .push_back((event, stamp, bounded(worker, MAX_TEXT_CHARS)));
                Admission::Evicted
            }
            None => {
                self.dropped += 1;
                Admission::Dropped
            }
        }
    }

    /// Takes the next event to write.
    pub fn take(&mut self) -> Option<(Event, Stamp, String)> {
        self.waiting.pop_front()
    }

    /// Takes the loss count and resets it, so it is reported once.
    pub fn take_dropped(&mut self) -> u64 {
        std::mem::replace(&mut self.dropped, 0)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.waiting.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }
}

/// The event a sink publishes after it has had to drop something.
///
/// A gap in a journal that does not say it is a gap is worse than no journal:
/// it reads as a period when nothing happened.
#[must_use]
pub fn loss_event(dropped: u64) -> Event {
    Event::new(Level::Warn, "journal.dropped").with("events", Value::Uint(dropped))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> Identity {
        Identity::new("run-abc", Component::new("host"), 4242, 3)
    }

    fn stamp() -> Stamp {
        Stamp {
            wall_nanos: 1_754_000_000_000_000_000,
            monotonic_millis: 1234,
        }
    }

    fn parsed(line: &str) -> serde_json::Value {
        serde_json::from_str(line).expect("every journal line is one valid JSON object")
    }

    #[test]
    fn a_line_is_one_valid_json_object_carrying_the_whole_identity() {
        let event = Event::new(Level::Info, "host.start").with_text("mode", "panel");

        let line = render(&identity(), &event, stamp(), "gui");

        assert!(!line.text.contains('\n'));
        let value = parsed(&line.text);
        assert_eq!(value["v"], SCHEMA_VERSION);
        assert_eq!(value["t"], 1_754_000_000_000_000_000_u64);
        assert_eq!(value["mono_ms"], 1234);
        assert_eq!(value["level"], "info");
        assert_eq!(value["component"], "host");
        assert_eq!(value["event"], "host.start");
        assert_eq!(value["run_id"], "run-abc");
        assert_eq!(value["pid"], 4242);
        assert_eq!(value["generation"], 3);
        assert_eq!(value["worker"], "gui");
        assert_eq!(value["mode"], "panel");
    }

    #[test]
    fn three_processes_of_one_run_correlate_by_run_id_and_generation() {
        let host = Identity::new("run-abc", Component::new("host"), 10, 0);
        let niri = Identity::new("run-abc", Component::new("niri-adapter"), 11, 1);
        let providers = Identity::new("run-abc", Component::new("provider-adapter"), 12, 2);
        let event = Event::new(Level::Info, "helper.ready");

        let lines: Vec<serde_json::Value> = [&host, &niri, &providers]
            .iter()
            .map(|who| parsed(&render(who, &event, stamp(), "").text))
            .collect();

        assert!(lines.iter().all(|line| line["run_id"] == "run-abc"));
        assert_eq!(
            lines
                .iter()
                .map(|line| line["generation"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            [0, 1, 2]
        );
        // Absent rather than empty: a line with no worker says nothing about
        // one instead of claiming an unnamed one.
        assert!(lines[0].get("worker").is_none());
    }

    #[test]
    fn ordering_within_a_process_is_the_monotonic_reading() {
        let first = Stamp {
            wall_nanos: 2_000,
            monotonic_millis: 10,
        };
        // The wall clock stepped backwards — NTP landing mid-incident — while
        // the monotonic reading kept going. The second event is still second.
        let second = Stamp {
            wall_nanos: 1_000,
            monotonic_millis: 20,
        };
        let event = Event::new(Level::Info, "tick");

        let a = parsed(&render(&identity(), &event, first, "").text);
        let b = parsed(&render(&identity(), &event, second, "").text);

        assert!(b["mono_ms"].as_u64() > a["mono_ms"].as_u64());
        assert!(b["t"].as_u64() < a["t"].as_u64());
    }

    #[test]
    fn a_redacted_field_carries_only_a_size() {
        let clipboard = "the password is hunter2";
        let event =
            Event::new(Level::Debug, "clipboard.captured").with_redacted("entry", clipboard);

        let line = render(&identity(), &event, stamp(), "clipboard");

        assert!(!line.text.contains("hunter2"));
        assert!(!line.text.contains("password"));
        let value = parsed(&line.text);
        assert_eq!(value["entry_chars"], clipboard.chars().count());
        assert_eq!(value["entry_bytes"], clipboard.len());
        assert!(value.get("entry").is_none());
    }

    #[test]
    fn no_hostile_fixture_reaches_the_line() {
        // One of each thing this journal exists never to contain.
        let hostile = [
            ("clipboard", "BEGIN OPENSSH PRIVATE KEY"),
            ("notification_body", "Your verification code is 819322"),
            ("media_title", "Nothing Else Matters — Metallica"),
            ("window_title", "Nextcloud — invoice-2026.pdf"),
            (
                "desktop_exec",
                "/usr/bin/firefox --profile /home/toni/.mozilla",
            ),
            ("ssid", "MiFibra-A4C1"),
            ("token", "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"),
        ];

        for (key, secret) in hostile {
            let event = Event::new(Level::Info, "provider.published").with_redacted(key, secret);
            let line = render(&identity(), &event, stamp(), "").text;

            // Distinctive words only: a three-letter one collides with the
            // journal's own field names and would prove nothing either way.
            for word in secret.split_whitespace().filter(|word| word.len() > 3) {
                assert!(
                    !line.contains(word),
                    "the journal leaked `{word}` through `{key}`: {line}"
                );
            }
        }
    }

    #[test]
    fn a_technical_identity_is_bounded_but_kept() {
        let long = "DP-".to_owned() + &"1".repeat(MAX_TEXT_CHARS * 2);
        let event = Event::new(Level::Info, "ddc.detect").with_text("output", &long);

        let value = parsed(&render(&identity(), &event, stamp(), "").text);

        // Kept, because a diagnosis is written in output names — and bounded,
        // because the name arrived from a monitor's own EDID.
        assert_eq!(
            value["output"].as_str().unwrap().chars().count(),
            MAX_TEXT_CHARS
        );
        assert!(value["output"].as_str().unwrap().starts_with("DP-1"));
    }

    #[test]
    fn a_producer_cannot_inject_a_second_line() {
        let event = Event::new(Level::Warn, "process.failed")
            .with_text("reason", "no\n{\"event\":\"forged\"}\nmore");

        let line = render(&identity(), &event, stamp(), "");

        assert_eq!(line.text.lines().count(), 1);
        assert_eq!(parsed(&line.text)["event"], "process.failed");
    }

    #[test]
    fn an_oversized_line_becomes_a_bounded_record_of_the_overflow() {
        let mut event = Event::new(Level::Error, "provider.frame");
        for index in 0..MAX_FIELDS {
            event = event.with_text(&format!("field{index}"), &"x".repeat(MAX_TEXT_CHARS));
        }

        let line = render(&identity(), &event, stamp(), "");

        // The event that was unusual enough to overflow is exactly the one worth
        // knowing about, so it is recorded as an overflow rather than dropped.
        assert!(line.text.len() <= MAX_LINE_BYTES);
        let value = parsed(&line.text);
        assert_eq!(value["event"], "provider.frame");
        assert!(value["journal_overflow_bytes"].as_u64().unwrap() > MAX_LINE_BYTES as u64);
    }

    #[test]
    fn an_event_stops_taking_fields_at_the_bound() {
        let mut event = Event::new(Level::Info, "wide");
        for index in 0..MAX_FIELDS + 20 {
            event = event.with(&format!("f{index}"), Value::Uint(index as u64));
        }

        assert_eq!(event.fields().len(), MAX_FIELDS);
    }

    #[test]
    fn critical_events_are_the_ones_flushed_and_mirrored() {
        assert!(Level::Critical.must_flush());
        assert!(!Level::Error.must_flush());
        assert!(Level::Warn.mirrored());
        assert!(!Level::Info.mirrored());
    }

    #[test]
    fn rotation_is_decided_by_arithmetic_alone() {
        assert_eq!(rotation(0, MAX_LINE_BYTES), Rotation::Append);
        assert_eq!(rotation(MAX_FILE_BYTES - 10, 9), Rotation::Append);
        assert_eq!(rotation(MAX_FILE_BYTES - 10, 11), Rotation::Rotate);
        // A file that grew past the bound for any other reason still closes.
        assert_eq!(rotation(MAX_FILE_BYTES * 4, 1), Rotation::Rotate);
        // And no arithmetic here can overflow into an Append.
        assert_eq!(rotation(u64::MAX, usize::MAX), Rotation::Rotate);
    }

    #[test]
    fn only_the_surplus_beyond_the_file_bound_is_retired() {
        let names: Vec<String> = (0..MAX_FILES + 3)
            .map(|index| format!("f{index}"))
            .collect();
        let borrowed: Vec<&str> = names.iter().map(String::as_str).collect();

        assert_eq!(retire(&borrowed), ["f8", "f9", "f10"]);
        assert!(retire(&borrowed[..MAX_FILES]).is_empty());
    }

    #[test]
    fn a_full_queue_refuses_an_ordinary_event_and_counts_it() {
        let mut queue = Queue::new(2);
        for _ in 0..2 {
            queue.offer(Event::new(Level::Info, "tick"), stamp(), "");
        }

        assert_eq!(
            queue.offer(Event::new(Level::Info, "tick"), stamp(), ""),
            Admission::Dropped
        );
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.take_dropped(), 1);
        // Reported once, not for ever.
        assert_eq!(queue.take_dropped(), 0);
    }

    #[test]
    fn a_full_queue_makes_room_for_a_critical_event() {
        let mut queue = Queue::new(2);
        queue.offer(Event::new(Level::Info, "tick"), stamp(), "");
        queue.offer(Event::new(Level::Critical, "ddc.write.start"), stamp(), "");

        assert_eq!(
            queue.offer(Event::new(Level::Critical, "process.kill"), stamp(), ""),
            Admission::Evicted
        );

        // The ordinary event was what gave way; both critical ones survived.
        let names: Vec<String> = std::iter::from_fn(|| queue.take())
            .map(|(event, _, _)| event.name().to_owned())
            .collect();
        assert_eq!(names, ["ddc.write.start", "process.kill"]);
    }

    #[test]
    fn a_queue_of_only_critical_events_refuses_rather_than_grows() {
        let mut queue = Queue::new(2);
        for _ in 0..2 {
            queue.offer(Event::new(Level::Critical, "ddc.read"), stamp(), "");
        }

        assert_eq!(
            queue.offer(Event::new(Level::Critical, "ddc.read"), stamp(), ""),
            Admission::Dropped
        );
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.take_dropped(), 1);
    }

    #[test]
    fn a_loss_is_published_as_its_own_event() {
        let event = loss_event(17);

        let value = parsed(&render(&identity(), &event, stamp(), "").text);
        assert_eq!(value["event"], "journal.dropped");
        assert_eq!(value["events"], 17);
        assert_eq!(value["level"], "warn");
    }
}
