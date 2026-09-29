//! Where each object of a PDF lives, and how to get at it.
//!
//! A PDF says where its objects are in a cross-reference, which is either a
//! table of offsets or, since version 1.5, a stream of them — and objects
//! themselves may be packed inside other streams. All three shapes are read
//! here, because refusing the modern ones would mean refusing most files
//! anybody actually has.
//!
//! Nothing is rewritten. A file is read as it is, and an edit is appended as an
//! incremental update, which is the format's own way of changing a document
//! without touching a byte of what came before.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use celestina_core::CancellationToken;
use flate2::read::ZlibDecoder;

use super::object::{Dictionary, Lexer, Object, PdfError};
use crate::inflate::{Budget, InflateError};

/// How many object streams may be in the middle of being read at once.
///
/// Reading one can need an entry of its dictionary that lives in another
/// (a `/Length` is allowed to be a reference), which can need a third. Real
/// files never go past one or two; a crafted chain would otherwise recurse
/// until the stack ran out, which aborts the process.
const MAX_OBJECT_STREAM_CHAIN: usize = 8;

/// What one object lookup costs on the work counter before anything is read,
/// so that looking up the same small object a million times is not free.
const LOOKUP_COST: u64 = 16;

/// The least room one classic cross-reference entry takes in a file.
const XREF_ENTRY: usize = 18;

/// How many times the document ceiling one walk over a document may spend on
/// the work counter: bytes lexed, decoded and copied, lookups, and what the
/// walk builds on top (font maps). A document reads each of its objects and
/// streams a small number of times, so real files stay far below this.
pub const WORK_FACTOR: u64 = 8;

/// How many times its own length reading a file's cross-references may lex.
/// Sections of a real file do not overlap, so they lex the file at most once.
const SECTION_FACTOR: u64 = 4;

/// The walk in progress over a document: where its budget ends on the work
/// counter, and the hosts' tokens that stop it — its own and those of the
/// walks it runs inside.
#[derive(Clone, Debug)]
struct Operation {
    ceiling: u64,
    cancellation: Vec<CancellationToken>,
}

/// How many header entries of an object stream are read between two checks
/// of the walk's budget and token.
const HEADER_CHECK: usize = 4096;

/// Where one object is: at an offset in the file, or inside another object.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Location {
    Offset(usize),
    InStream { container: u32, index: usize },
}

/// Which bytes an object number reads: two numbers with the same key are the
/// same object, whatever the cross-reference calls them.
///
/// A real file gives each object one number. A crafted one can point
/// thousands of numbers at one large object — at one offset in the file, or
/// at one offset inside an object stream through a header that repeats it —
/// and a walk that tells objects apart by number would read, and keep, that
/// object thousands of times.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ObjectKey(Bytes);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Bytes {
    /// An object at this offset in the file.
    File(usize),
    /// The `slot`-th distinct object of object stream `container`.
    Packed { container: u32, slot: usize },
}

/// An object stream, unpacked: each distinct object once, with the bytes it
/// was read from, and which of them each entry of the header names.
///
/// A header may name one offset any number of times; the object there is read
/// and kept once, and every entry that names it shares it.
#[derive(Debug)]
struct Unpack {
    objects: Vec<(Object, usize)>,
    slots: Vec<usize>,
}

/// What is known about one object stream.
#[derive(Clone, Debug)]
enum Packed {
    /// Being read right now; meeting it again is a cycle.
    Reading,
    /// Read, with every object in it in its own order and the bytes it was
    /// read from — or the reason it could not be, which is not worth finding
    /// out twice.
    Read(Result<Arc<Unpack>, PdfError>),
}

/// The object streams a file has already had unpacked and read.
///
/// Most modern files keep nearly every object in object streams, and text
/// extraction resolves pages, fonts and maps one object at a time; without
/// this, every lookup inflated and re-read its whole stream. What is kept
/// draws on its own budget of the document ceiling, so the cache is bounded
/// by the same number as everything else.
#[derive(Clone, Debug)]
struct Unpacked {
    streams: BTreeMap<u32, Packed>,
    /// How many of `streams` are `Reading`, kept alongside so the chain check
    /// costs nothing however many streams have been read.
    reading: usize,
    budget: Budget,
}

/// A PDF file, with its objects located but not yet read.
pub struct Pdf {
    bytes: Vec<u8>,
    locations: BTreeMap<u32, Location>,
    trailer: Dictionary,
    /// Where the newest cross-reference starts, which an update points back at.
    start_xref: usize,
    /// Whether that newest one is a stream. An update writes the same kind: a
    /// table appended to a stream-based file is a shape some readers reject.
    xref_streams: bool,
    /// The ceiling on what one stream may unpack, filters and all.
    limit: u64,
    /// Behind a lock rather than a cell so a document stays `Sync`, as it was
    /// before it had anything to remember.
    unpacked: Mutex<Unpacked>,
    /// How many bytes reading this file has lexed, decoded and copied so far;
    /// see [`Pdf::work`].
    work: AtomicU64,
    /// The walk in progress, if any; see [`Pdf::within`].
    operation: Mutex<Option<Operation>>,
}

impl Clone for Pdf {
    fn clone(&self) -> Self {
        Self {
            bytes: self.bytes.clone(),
            locations: self.locations.clone(),
            trailer: self.trailer.clone(),
            start_xref: self.start_xref,
            xref_streams: self.xref_streams,
            limit: self.limit,
            unpacked: Mutex::new(self.unpacked().clone()),
            work: AtomicU64::new(self.work()),
            operation: Mutex::new(None),
        }
    }
}

impl std::fmt::Debug for Pdf {
    /// Named without its bytes: a debug line for a document should not be a
    /// megabyte of compressed streams.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Pdf")
            .field("bytes", &self.bytes.len())
            .field("objects", &self.locations.len())
            .finish_non_exhaustive()
    }
}

impl Pdf {
    /// Reads a file's structure. No stream in it may unpack past `limit`
    /// bytes, and neither may its cross-reference streams together; reading
    /// the sections lexes at most a few times the file's length, and stops
    /// when `cancellation` says so.
    pub fn parse(
        bytes: Vec<u8>,
        limit: u64,
        cancellation: &CancellationToken,
    ) -> Result<Self, PdfError> {
        if !bytes.starts_with(b"%PDF-") {
            return Err(PdfError::NotPdf);
        }
        let start = find_start_xref(&bytes)?;
        // The sections are read through the file itself with nothing located
        // yet, so a reference inside a cross-reference stream's dictionary
        // reads as null, as it always has: the table that would resolve it is
        // the one being read.
        let mut pdf = Self {
            bytes,
            locations: BTreeMap::new(),
            trailer: Dictionary::new(),
            start_xref: start,
            xref_streams: false,
            limit,
            unpacked: Mutex::new(Unpacked {
                streams: BTreeMap::new(),
                reading: 0,
                budget: Budget::new(limit),
            }),
            work: AtomicU64::new(0),
            operation: Mutex::new(None),
        };
        let lexing = u64::try_from(pdf.bytes.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(SECTION_FACTOR);
        let (locations, trailer, xref_streams) =
            pdf.within(lexing.saturating_add(limit), cancellation, || {
                pdf.read_sections(start)
            })?;
        if trailer.contains_key("Encrypt") {
            return Err(PdfError::Encrypted);
        }
        pdf.locations = locations;
        pdf.trailer = trailer;
        pdf.xref_streams = xref_streams;
        Ok(pdf)
    }

    /// Every cross-reference section, newest first: where each object is, the
    /// merged trailer, and whether the newest section is a stream.
    fn read_sections(
        &self,
        start: usize,
    ) -> Result<(BTreeMap<u32, Location>, Dictionary, bool), PdfError> {
        let mut xref_streams = false;
        let mut locations = BTreeMap::new();
        let mut trailer = Dictionary::new();
        let mut next = Some(start);
        let mut seen = BTreeSet::new();
        let mut budget = Budget::new(self.limit);
        // Every classic entry of every section draws on one count, which the
        // file's length bounds: sections that overlap one another's entries
        // would otherwise read the same bytes once per section.
        let mut entries = self.bytes.len() / XREF_ENTRY + 1;

        // Each cross-reference points back at the one it updates, so a file
        // that has been edited before is read newest first and older entries
        // only fill what the newer ones left out.
        while let Some(offset) = next {
            if !seen.insert(offset) {
                break;
            }
            let section = read_section(self, offset, &mut budget, &mut entries)?;
            if seen.len() == 1 {
                xref_streams = section.is_stream;
            }
            for (number, location) in section.locations {
                locations.entry(number).or_insert(location);
            }
            for (key, value) in section.trailer {
                trailer.entry(key).or_insert(value);
            }
            next = section.previous;
        }
        Ok((locations, trailer, xref_streams))
    }

    /// Where the newest cross-reference begins.
    #[must_use]
    pub const fn start_xref(&self) -> usize {
        self.start_xref
    }

    /// Whether this file's cross-reference is a stream rather than a table.
    #[must_use]
    pub const fn uses_xref_streams(&self) -> bool {
        self.xref_streams
    }

    /// The file's own bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The trailer, which names the catalogue and the rest of the roots.
    #[must_use]
    pub const fn trailer(&self) -> &Dictionary {
        &self.trailer
    }

    /// The ceiling on what one stream may unpack.
    #[must_use]
    pub const fn limit(&self) -> u64 {
        self.limit
    }

    /// Which bytes object `number` reads: `Ok(None)` when the file does not
    /// locate it (it reads as null), and an error exactly when reading the
    /// object would fail.
    ///
    /// An object inside an object stream is keyed by the distinct object its
    /// header entry names, which unpacks that stream (once; it is remembered).
    /// Like a lookup, asking costs [`LOOKUP_COST`] and checks the walk, so a
    /// file that names a million objects in streams it does not have is
    /// charged for each, and stops when the host cancels.
    pub fn key(&self, number: u32) -> Result<Option<ObjectKey>, PdfError> {
        self.spend(LOOKUP_COST as usize)?;
        let Some(location) = self.locations.get(&number).copied() else {
            return Ok(None);
        };
        match location {
            Location::Offset(offset) => Ok(Some(ObjectKey(Bytes::File(offset)))),
            Location::InStream { container, index } => {
                let unpack = self.object_stream(container)?;
                let slot = *unpack.slots.get(index).ok_or_else(|| PdfError::Malformed {
                    detail: format!("object {number} is not in the stream that claims it"),
                })?;
                Ok(Some(ObjectKey(Bytes::Packed { container, slot })))
            }
        }
    }

    /// The highest object number the file uses.
    #[must_use]
    pub fn last_object(&self) -> u32 {
        self.locations.keys().copied().max().unwrap_or(0)
    }

    /// How many bytes reading this file has lexed, decoded and copied since
    /// it was parsed, plus a small cost per object lookup. It only grows.
    #[must_use]
    pub fn work(&self) -> u64 {
        self.work.load(Ordering::Relaxed)
    }

    /// Runs one walk over the document under a budget of `budget` on the
    /// work counter, stopping when `cancellation` says so.
    ///
    /// Each lookup and each decode is bounded on its own; what is not is how
    /// many a walk makes. While `run` runs, every lookup, decode and
    /// [`Pdf::spend`] checks the budget and the token, so a file that sends
    /// every page back to the same large object is refused as
    /// [`PdfError::TooLarge`] wherever the walk happens to be, not when it
    /// finishes. Walks nest: an inner walk ends at the nearer of its own
    /// ceiling and the outer one, answers to both tokens, and restores the
    /// outer walk on return.
    pub fn within<T>(
        &self,
        budget: u64,
        cancellation: &CancellationToken,
        run: impl FnOnce() -> Result<T, PdfError>,
    ) -> Result<T, PdfError> {
        let own = self.work().saturating_add(budget);
        let outer = self.operation().clone();
        let operation = match &outer {
            Some(outer) => Operation {
                ceiling: own.min(outer.ceiling),
                cancellation: outer
                    .cancellation
                    .iter()
                    .cloned()
                    .chain([cancellation.clone()])
                    .collect(),
            },
            None => Operation {
                ceiling: own,
                cancellation: vec![cancellation.clone()],
            },
        };
        *self.operation() = Some(operation);
        let result = run();
        *self.operation() = outer;
        result
    }

    /// The budget one walk over this document gets: [`WORK_FACTOR`] times
    /// the ceiling.
    #[must_use]
    pub const fn walk_budget(&self) -> u64 {
        self.limit.saturating_mul(WORK_FACTOR)
    }

    fn operation(&self) -> MutexGuard<'_, Option<Operation>> {
        // Only this module writes it, whole, under the lock.
        self.operation
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether the walk in progress may go on: within its budget and not
    /// cancelled. Outside a walk there is nothing to check.
    pub fn checkpoint(&self) -> Result<(), PdfError> {
        let Some(operation) = self.operation().clone() else {
            return Ok(());
        };
        if self.work() > operation.ceiling {
            return Err(PdfError::TooLarge { limit: self.limit });
        }
        if operation
            .cancellation
            .iter()
            .any(CancellationToken::is_cancelled)
        {
            return Err(PdfError::Cancelled);
        }
        Ok(())
    }

    /// Records `bytes` of work done outside this file's own reads — a font
    /// map a caller built from it, say — and checks the walk.
    pub fn spend(&self, bytes: usize) -> Result<(), PdfError> {
        self.charge(bytes);
        self.checkpoint()
    }

    fn charge(&self, bytes: usize) {
        let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
        let _ = self
            .work
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |spent| {
                Some(spent.saturating_add(bytes))
            });
    }

    /// Reads one object by number.
    pub fn object(&self, number: u32) -> Result<Object, PdfError> {
        self.spend(LOOKUP_COST as usize)?;
        match self.locations.get(&number).copied() {
            None => Ok(Object::Null),
            Some(Location::Offset(offset)) => self.object_at(offset),
            Some(Location::InStream { container, index }) => {
                let unpack = self.object_stream(container)?;
                let (object, size) = unpack
                    .slots
                    .get(index)
                    .and_then(|slot| unpack.objects.get(*slot))
                    .ok_or_else(|| PdfError::Malformed {
                        detail: format!("object {number} is not in the stream that claims it"),
                    })?;
                // The copy handed out costs what the object took to read.
                self.charge(*size);
                Ok(object.clone())
            }
        }
    }

    /// Follows a reference until it is not one.
    pub fn resolve(&self, object: &Object) -> Result<Object, PdfError> {
        match object {
            Object::Reference { number, .. } => self.object(*number),
            other => Ok(other.clone()),
        }
    }

    /// One entry of a dictionary, resolved.
    pub fn entry(&self, dictionary: &Dictionary, key: &str) -> Result<Object, PdfError> {
        match dictionary.get(key) {
            None => Ok(Object::Null),
            Some(object) => self.resolve(object),
        }
    }

    /// A stream's content, decompressed when this crate knows the filter.
    ///
    /// Every filter in the stream's chain draws on one budget of the
    /// document ceiling, so repeating a filter cannot multiply what it
    /// unpacks.
    pub fn stream_data(&self, object: &Object) -> Result<Vec<u8>, PdfError> {
        self.stream_data_within(object, &mut Budget::new(self.limit))
    }

    fn stream_data_within(
        &self,
        object: &Object,
        budget: &mut Budget,
    ) -> Result<Vec<u8>, PdfError> {
        let Object::Stream { dictionary, data } = object else {
            return Err(PdfError::Malformed {
                detail: "this object is not a stream".to_owned(),
            });
        };
        let start = data.0;
        let length = self.length_of(dictionary, start);
        let end = start.saturating_add(length).min(self.bytes.len());
        let raw = self
            .bytes
            .get(start..end)
            .ok_or_else(|| PdfError::Malformed {
                detail: "a stream starts past the end of the file".to_owned(),
            })?;

        let filters = match self.entry(dictionary, "Filter")? {
            Object::Name(name) => vec![name],
            Object::Array(items) => items
                .iter()
                .filter_map(|item| item.as_name().map(str::to_owned))
                .collect(),
            _ => Vec::new(),
        };
        let mut content = raw.to_vec();
        self.charge(content.len());
        for filter in filters {
            content = match filter.as_str() {
                "FlateDecode" => {
                    let room = budget.remaining();
                    budget
                        .inflate(ZlibDecoder::new(content.as_slice()))
                        .map_err(|error| match error {
                            InflateError::TooLarge { limit } => {
                                // The decode produced one byte past what was
                                // left before it stopped; that is work done.
                                self.charge(
                                    usize::try_from(room.saturating_add(1)).unwrap_or(usize::MAX),
                                );
                                PdfError::TooLarge { limit }
                            }
                            InflateError::Corrupt => PdfError::Malformed {
                                detail: "a compressed stream could not be read".to_owned(),
                            },
                        })?
                }
                other => {
                    return Err(PdfError::Unsupported {
                        detail: format!("the {other} stream filter"),
                    })
                }
            };
            self.charge(content.len());
        }
        self.checkpoint()?;
        Ok(content)
    }

    /// How long a stream's data is.
    ///
    /// The declared length is trusted when it is there and plausible; when it
    /// is a reference this file cannot resolve yet — which happens while the
    /// cross-reference itself is being read — the `endstream` keyword decides.
    fn length_of(&self, dictionary: &Dictionary, start: usize) -> usize {
        if let Ok(Object::Number(value)) = self.entry(dictionary, "Length") {
            let length = value.max(0.0) as usize;
            if start
                .checked_add(length)
                .is_some_and(|end| end <= self.bytes.len())
            {
                return length;
            }
        }
        let rest = self.bytes.get(start..).unwrap_or(&[]);
        rest.windows(9)
            .position(|window| window == b"endstream")
            .unwrap_or(rest.len())
    }

    fn object_at(&self, offset: usize) -> Result<Object, PdfError> {
        let mut lexer = Lexer::new(&self.bytes, offset);
        // `12 0 obj`
        lexer.skip_space();
        while lexer
            .bytes
            .get(lexer.cursor)
            .is_some_and(u8::is_ascii_digit)
        {
            lexer.cursor += 1;
        }
        lexer.skip_space();
        while lexer
            .bytes
            .get(lexer.cursor)
            .is_some_and(u8::is_ascii_digit)
        {
            lexer.cursor += 1;
        }
        if !lexer.eat(b"obj") {
            self.charge(lexer.cursor.saturating_sub(offset));
            return Err(PdfError::Malformed {
                detail: format!("no object begins at byte {offset}"),
            });
        }
        let object = lexer.object();
        self.charge(lexer.cursor.saturating_sub(offset));
        self.checkpoint()?;
        object
    }

    fn unpacked(&self) -> MutexGuard<'_, Unpacked> {
        // The lock guards a cache, and every writer leaves it whole; a panic
        // elsewhere cannot leave half an entry behind.
        self.unpacked.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The objects packed inside an object stream, in its own order, read
    /// once and remembered.
    fn object_stream(&self, number: u32) -> Result<Arc<Unpack>, PdfError> {
        {
            let mut unpacked = self.unpacked();
            match unpacked.streams.get(&number) {
                Some(Packed::Read(result)) => return result.clone(),
                Some(Packed::Reading) => {
                    return Err(PdfError::Malformed {
                        detail: format!("object stream {number} needs itself to be read"),
                    })
                }
                None => {}
            }
            if unpacked.reading >= MAX_OBJECT_STREAM_CHAIN {
                return Err(PdfError::Malformed {
                    detail: "object streams refer to one another too deeply".to_owned(),
                });
            }
            unpacked.streams.insert(number, Packed::Reading);
            unpacked.reading += 1;
        }
        // The lock is not held while reading: the stream's own dictionary may
        // send the read into another object stream.
        let result = self.read_object_stream(number);
        let mut unpacked = self.unpacked();
        let result = result.and_then(|(unpack, unpacked_bytes)| {
            unpacked
                .budget
                .charge(unpacked_bytes)
                .map_err(|_| PdfError::TooLarge { limit: self.limit })?;
            Ok(Arc::new(unpack))
        });
        unpacked.reading = unpacked.reading.saturating_sub(1);
        // A failure is the stream's own and is remembered — its inflate past
        // the ceiling, the retained total, a malformed header — unless the
        // walk around it has stopped (over its budget, or cancelled), which
        // says nothing about the stream: then the next walk may read it.
        drop(unpacked);
        let walk_stopped = result.is_err() && self.checkpoint().is_err();
        let mut unpacked = self.unpacked();
        match &result {
            Err(PdfError::TooLarge { .. } | PdfError::Cancelled) if walk_stopped => {
                unpacked.streams.remove(&number);
            }
            _ => {
                unpacked
                    .streams
                    .insert(number, Packed::Read(result.clone()));
            }
        }
        result
    }

    /// Unpacks and reads one object stream: its objects, and how many bytes
    /// it unpacked to.
    ///
    /// Inside a walk, the unpacking itself is checked: the header every
    /// [`HEADER_CHECK`] entries, and each distinct object as it is lexed.
    fn read_object_stream(&self, number: u32) -> Result<(Unpack, usize), PdfError> {
        let container = match self.locations.get(&number).copied() {
            Some(Location::Offset(offset)) => self.object_at(offset)?,
            _ => {
                return Err(PdfError::Malformed {
                    detail: format!("object stream {number} is not in the file"),
                })
            }
        };
        let Object::Stream { ref dictionary, .. } = container else {
            return Err(PdfError::Malformed {
                detail: format!("object {number} is not a stream"),
            });
        };
        let declared = self.entry(dictionary, "N")?.as_number().unwrap_or(0.0);
        let first = self.entry(dictionary, "First")?.as_number().unwrap_or(0.0) as usize;
        let content = self.stream_data(&container)?;

        // `/N` is the file's claim. Each object takes at least a number, a
        // space, an offset and a space in the header, so the content bounds
        // the count however large the claim.
        if !(declared >= 0.0 && declared <= (content.len() / 4) as f64) {
            return Err(PdfError::Malformed {
                detail: format!("object stream {number} claims more objects than it holds"),
            });
        }
        let count = declared as usize;
        let mut header = Lexer::new(&content, 0);
        let mut offsets = Vec::with_capacity(count);
        self.spend(content.len())?;
        for entry in 0..count {
            if entry % HEADER_CHECK == 0 {
                self.checkpoint()?;
            }
            let _number = header.object()?;
            let offset = header.object()?.as_number().unwrap_or(0.0) as usize;
            let start = first
                .checked_add(offset)
                .ok_or_else(|| PdfError::Malformed {
                    detail: format!("object stream {number} points past itself"),
                })?;
            offsets.push(start);
        }

        // Each object is read only up to where the next one starts, and each
        // start only once and kept once, so the stream is lexed in one pass
        // and held as one copy of each object however its header orders or
        // repeats its offsets.
        let starts: BTreeSet<usize> = offsets.iter().copied().collect();
        let mut read: BTreeMap<usize, usize> = BTreeMap::new();
        let mut objects = Vec::with_capacity(starts.len());
        let mut slots = Vec::with_capacity(count);
        for offset in offsets {
            if let Some(slot) = read.get(&offset) {
                slots.push(*slot);
                continue;
            }
            let end = starts
                .range(offset.saturating_add(1)..)
                .next()
                .copied()
                .unwrap_or(content.len())
                .min(content.len());
            let slice = content.get(offset..end).unwrap_or(&[]);
            let mut lexer = Lexer::new(slice, 0);
            let object = lexer.object()?;
            self.spend(lexer.cursor)?;
            read.insert(offset, objects.len());
            slots.push(objects.len());
            objects.push((object, lexer.cursor));
        }
        Ok((Unpack { objects, slots }, content.len()))
    }
}

struct Section {
    locations: Vec<(u32, Location)>,
    trailer: Dictionary,
    previous: Option<usize>,
    is_stream: bool,
}

fn read_section(
    pdf: &Pdf,
    offset: usize,
    budget: &mut Budget,
    entries: &mut usize,
) -> Result<Section, PdfError> {
    let mut lexer = Lexer::new(&pdf.bytes, offset);
    if lexer.eat(b"xref") {
        let section = read_table(&mut lexer, entries);
        // A section is charged for everything it lexed, trailer included: a
        // section nested inside the previous one's trailer lexes the rest of
        // the file again, and that is what the parse's budget counts.
        pdf.spend(lexer.cursor.saturating_sub(offset))?;
        return section;
    }
    read_stream_section(pdf, offset, budget)
}

/// The classic cross-reference: `xref`, then runs of twenty-byte entries.
///
/// `entries` is what the file's sections may still read between them.
fn read_table(lexer: &mut Lexer<'_>, entries: &mut usize) -> Result<Section, PdfError> {
    let mut locations = Vec::new();
    loop {
        lexer.skip_space();
        if lexer
            .bytes
            .get(lexer.cursor..)
            .is_some_and(|rest| rest.starts_with(b"trailer"))
        {
            lexer.cursor += b"trailer".len();
            break;
        }
        let start = lexer.object()?.as_number().unwrap_or(0.0) as u32;
        let count = lexer.object()?.as_number().unwrap_or(0.0) as usize;
        for index in 0..count {
            *entries = entries.checked_sub(1).ok_or_else(|| PdfError::Malformed {
                detail: "the cross-references list more entries than the file holds".to_owned(),
            })?;
            lexer.skip_space();
            let entry = lexer
                .cursor
                .checked_add(18)
                .and_then(|end| lexer.bytes.get(lexer.cursor..end))
                .ok_or_else(|| PdfError::Malformed {
                    detail: "the cross-reference table is cut short".to_owned(),
                })?;
            // Ten digits of offset, a space, five of generation, a space and
            // the kind. Only an entry in use says where anything is, and its
            // offset has to be digits; the rest are skipped as they always
            // were.
            if entry.get(17) == Some(&b'n') {
                let offset = entry
                    .get(..10)
                    .and_then(|digits| std::str::from_utf8(digits).ok())
                    .and_then(|digits| digits.trim().parse::<usize>().ok())
                    .ok_or_else(|| PdfError::Malformed {
                        detail: "a cross-reference entry is not an offset".to_owned(),
                    })?;
                let number = u32::try_from(index)
                    .ok()
                    .and_then(|index| start.checked_add(index))
                    .ok_or_else(|| PdfError::Malformed {
                        detail: "the cross-reference numbers objects past any file".to_owned(),
                    })?;
                locations.push((number, Location::Offset(offset)));
            }
            lexer.cursor += 18;
        }
    }
    let trailer = match lexer.object()? {
        Object::Dictionary(dictionary) => dictionary,
        _ => Dictionary::new(),
    };
    let previous = trailer
        .get("Prev")
        .and_then(Object::as_number)
        .map(|value| value as usize);
    Ok(Section {
        locations,
        trailer,
        previous,
        is_stream: false,
    })
}

/// The widest cross-reference field this reader assembles: eight bytes is a
/// 64-bit offset, already past any file it will be handed.
const MAX_FIELD_WIDTH: usize = 8;

/// The cross-reference stream a modern file carries instead of a table.
///
/// Read through `pdf` while it locates nothing, and inflated against the
/// budget every cross-reference of the file shares.
fn read_stream_section(pdf: &Pdf, offset: usize, budget: &mut Budget) -> Result<Section, PdfError> {
    let object = pdf.object_at(offset)?;
    let Object::Stream { ref dictionary, .. } = object else {
        return Err(PdfError::Malformed {
            detail: format!("byte {offset} holds no cross-reference"),
        });
    };
    let content = pdf.stream_data_within(&object, budget)?;
    let content = apply_predictor(pdf, dictionary, content)?;

    let widths: Vec<usize> = pdf
        .entry(dictionary, "W")?
        .as_array()
        .unwrap_or(&[])
        .iter()
        .map(|item| {
            item.as_number()
                .filter(|width| (0.0..=MAX_FIELD_WIDTH as f64).contains(width))
                .map(|width| width as usize)
                .ok_or_else(|| PdfError::Malformed {
                    detail: "a cross-reference field is wider than any offset".to_owned(),
                })
        })
        .collect::<Result<_, _>>()?;
    if widths.len() < 3 {
        return Err(PdfError::Malformed {
            detail: "a cross-reference stream has no field widths".to_owned(),
        });
    }
    // Each width is at most eight and there are no more of them than the
    // file has bytes, so the sum cannot overflow.
    let row: usize = widths.iter().sum();
    if row == 0 {
        return Err(PdfError::Malformed {
            detail: "a cross-reference stream has rows of no width".to_owned(),
        });
    }
    let size = pdf.entry(dictionary, "Size")?.as_number().unwrap_or(0.0) as u32;
    let index: Vec<u32> = match pdf.entry(dictionary, "Index")? {
        Object::Array(items) => items
            .iter()
            .map(|item| item.as_number().unwrap_or(0.0) as u32)
            .collect(),
        _ => vec![0, size],
    };

    let mut locations = Vec::new();
    let mut rows = content.chunks_exact(row);
    'ranges: for pair in index.chunks(2) {
        let (first, count) = (pair[0], pair.get(1).copied().unwrap_or(0));
        // A range is numbered from its first object; past `u32::MAX` there
        // are no object numbers left to give, whatever the count says.
        for number in (first..=u32::MAX).take(count as usize) {
            // The content, not the claimed count, decides how many rows
            // there are.
            let Some(fields_bytes) = rows.next() else {
                break 'ranges;
            };
            let mut fields = [1u64, 0, 0];
            let mut at = 0;
            for (slot, width) in widths.iter().enumerate().take(3) {
                if *width > 0 {
                    let value = fields_bytes
                        .get(at..at + width)
                        .unwrap_or(&[])
                        .iter()
                        .fold(0u64, |value, byte| (value << 8) | u64::from(*byte));
                    fields[slot] = value;
                    at += width;
                }
            }
            match fields[0] {
                1 => locations.push((number, Location::Offset(fields[1] as usize))),
                2 => locations.push((
                    number,
                    Location::InStream {
                        container: fields[1] as u32,
                        index: fields[2] as usize,
                    },
                )),
                _ => {}
            }
        }
    }
    let previous = dictionary
        .get("Prev")
        .and_then(Object::as_number)
        .map(|value| value as usize);
    Ok(Section {
        locations,
        trailer: dictionary.clone(),
        previous,
        is_stream: true,
    })
}

/// Undoes the PNG predictor a cross-reference stream is usually written with.
fn apply_predictor(
    pdf: &Pdf,
    dictionary: &Dictionary,
    content: Vec<u8>,
) -> Result<Vec<u8>, PdfError> {
    let parameters = match pdf.entry(dictionary, "DecodeParms")? {
        Object::Dictionary(parameters) => parameters,
        _ => return Ok(content),
    };
    let predictor = pdf
        .entry(&parameters, "Predictor")?
        .as_number()
        .unwrap_or(1.0) as usize;
    if predictor < 10 {
        return Ok(content);
    }
    if content.is_empty() {
        return Ok(content);
    }
    let columns = pdf
        .entry(&parameters, "Columns")?
        .as_number()
        .unwrap_or(1.0);
    // A row is its tag byte and its columns, and it has to fit in the data;
    // the file's number sized an allocation before it was checked.
    if !(columns >= 1.0 && columns < content.len() as f64) {
        return Err(PdfError::Malformed {
            detail: "a predictor's rows are wider than its data".to_owned(),
        });
    }
    let columns = columns as usize;
    let row = columns + 1;
    let mut out = Vec::with_capacity(content.len());
    let mut previous = vec![0u8; columns];
    for chunk in content.chunks(row) {
        let Some((&tag, rest)) = chunk.split_first() else {
            break;
        };
        if rest.is_empty() {
            break;
        }
        let mut line = rest.to_vec();
        line.resize(columns, 0);
        // "Up" is what every writer uses for this table; the others would need
        // a pixel width these rows do not have.
        if tag == 2 {
            for (byte, above) in line.iter_mut().zip(&previous) {
                *byte = byte.wrapping_add(*above);
            }
        }
        out.extend_from_slice(&line);
        previous = line;
    }
    Ok(out)
}

fn find_start_xref(bytes: &[u8]) -> Result<usize, PdfError> {
    let tail_start = bytes.len().saturating_sub(2048);
    let tail = bytes.get(tail_start..).unwrap_or(&[]);
    let at = tail
        .windows(9)
        .rposition(|window| window == b"startxref")
        .ok_or(PdfError::Malformed {
            detail: "the file names no cross-reference".to_owned(),
        })?;
    let mut lexer = Lexer::new(bytes, tail_start + at + 9);
    let offset = lexer.object()?.as_number().ok_or(PdfError::Malformed {
        detail: "the cross-reference offset is not a number".to_owned(),
    })?;
    // Where the cross-reference starts is the file's claim, and a claim
    // past its own end is not one to follow.
    if !(offset >= 0.0 && offset < bytes.len() as f64) {
        return Err(PdfError::Malformed {
            detail: "the cross-reference starts past the end of the file".to_owned(),
        });
    }
    Ok(offset as usize)
}

#[cfg(test)]
mod tests {
    use celestina_core::CancellationToken;

    use super::{Pdf, PdfError};

    /// A PDF 1.5 file with a catalogue at object 1, object 9 packed in object
    /// stream 50 (which is `container` when it exists), and an uncompressed
    /// cross-reference stream.
    fn packed_nine(with_container: bool) -> Vec<u8> {
        let mut out = b"%PDF-1.5\n".to_vec();
        let catalogue = out.len();
        out.extend_from_slice(b"1 0 obj\n<< /Type /Catalog >>\nendobj\n");
        let container = out.len();
        if with_container {
            let content = b"9 0 << /Name (packed) >>";
            out.extend_from_slice(
                format!(
                    "50 0 obj\n<< /Type /ObjStm /N 1 /First 4 /Length {} >>\nstream\n",
                    content.len()
                )
                .as_bytes(),
            );
            out.extend_from_slice(content);
            out.extend_from_slice(b"\nendstream\nendobj\n");
        }
        let xref = out.len();
        let mut rows = Vec::new();
        for number in 0..=51u32 {
            let (kind, field, index): (u8, u32, u16) = match number {
                1 => (1, u32::try_from(catalogue).unwrap_or(0), 0),
                9 => (2, 50, 0),
                50 if with_container => (1, u32::try_from(container).unwrap_or(0), 0),
                51 => (1, u32::try_from(xref).unwrap_or(0), 0),
                _ => (0, 0, 0),
            };
            rows.push(kind);
            rows.extend_from_slice(&field.to_be_bytes());
            rows.extend_from_slice(&index.to_be_bytes());
        }
        out.extend_from_slice(
            format!(
                "51 0 obj\n<< /Type /XRef /Size 52 /W [1 4 2] /Root 1 0 R /Length {} >>\nstream\n",
                rows.len()
            )
            .as_bytes(),
        );
        out.extend_from_slice(&rows);
        out.extend_from_slice(
            format!("\nendstream\nendobj\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
        );
        out
    }

    fn parse(bytes: Vec<u8>) -> Pdf {
        Pdf::parse(bytes, 1 << 20, &CancellationToken::new()).expect("a small PDF")
    }

    #[test]
    fn a_key_in_a_missing_object_stream_is_an_error_not_an_absent_object() {
        let pdf = parse(packed_nine(false));
        assert!(matches!(pdf.key(9), Err(PdfError::Malformed { .. })));
        assert!(pdf.object(9).is_err());
        // An object the file does not locate is absent, which is not an error.
        assert_eq!(pdf.key(8), Ok(None));
    }

    #[test]
    fn asking_for_a_key_checks_the_hosts_token() {
        let pdf = parse(packed_nine(true));
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert_eq!(
            pdf.within(1 << 20, &cancelled, || pdf.key(9)),
            Err(PdfError::Cancelled)
        );
        assert_eq!(
            pdf.within(1 << 20, &cancelled, || pdf.key(8)),
            Err(PdfError::Cancelled)
        );
    }

    #[test]
    fn a_walk_that_runs_out_of_budget_is_not_remembered_as_the_streams_fault() {
        let pdf = parse(packed_nine(true));
        // A budget that pays for the lookup but not for unpacking the
        // stream: the walk is refused inside the unpack ...
        assert_eq!(
            pdf.within(40, &CancellationToken::new(), || pdf.key(9)),
            Err(PdfError::TooLarge { limit: 1 << 20 })
        );
        // ... and the next walk, with room, reads the stream.
        let key = pdf
            .within(1 << 20, &CancellationToken::new(), || pdf.key(9))
            .expect("the stream unpacks");
        assert!(key.is_some());
        assert!(pdf.object(9).is_ok());
    }
}
