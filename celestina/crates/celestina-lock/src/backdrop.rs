//! The backdrop bundle the shell hands over on stdin (design §4.3 (a)) and the
//! dimmed picture drawn from it (design §4.6, §4.6.1, §4.10).
//!
//! **The reading** validates the whole bundle with the shared codec and keeps
//! what the lock acts on: the failures the previous lock counted
//! (`prior_failures`), the UTC offset facts and the validated pixel blocks.
//! The reader never trusts its input and never blocks on it: stdin must be a
//! memory file sealed against shrinking, growing, writing and sealing (a
//! `/dev/null`, a terminal or a pipe is not, and the lock then has no backdrop
//! and waits for nothing); the size is checked against the 16 MiB bound before
//! anything is allocated; the read is positioned (`pread` from offset 0), so
//! the descriptor's file offset is irrelevant, and bounded by a 2 s deadline.
//! Anything that fails a bound is ignored as a whole: black, no failures
//! carried. A bad block keeps the header and the blocks before it (the
//! codec's rule). The read stays at launch, ahead of the machine, because the
//! machine is seeded with `prior_failures`; it is bounded exactly as the
//! worker's would be.
//!
//! **The picture** is the one full-output operation of the lock: a block of at
//! most 640 px a side becomes one [`Base`] per output, upscaled to the output's
//! buffer size, with the scrim already composited over it. That runs on the
//! `backdrop` worker ([`Backdrop`]), never on the loop; the loop only asks for
//! a base by output name and size, gets `None` until the worker answers (the
//! output stays black with the prompt), and caches what comes back so the
//! upscale is computed once per output and size. An output without a block,
//! a bundle that was ignored and a picture that cannot be upscaled all mean
//! black. Nothing here decodes an image or opens a file: the pixels arrive
//! already decoded by the shell, inside the sealed bundle.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::os::fd::BorrowedFd;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use celestina_lock_proto::{decode, FileMeta, Ignored, OutputBlock, Status, MAX_FILE_BYTES};
use celestina_tokens::Rgba;
use rustix::fs::{fcntl_get_seals, fstat, FileType, SealFlags};

/// How long the read of the bundle may take.
pub const READ_DEADLINE: Duration = Duration::from_secs(2);

/// The largest base the worker computes, in pixels (an 8K output is 33 million).
/// A larger buffer is shown black: the bound is on memory, not on taste.
pub const MAX_BASE_PIXELS: u64 = 40_000_000;

/// What the lock keeps of the bundle's header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// Counted failures since the last unlock; seeds the backoff.
    pub prior_failures: u8,
    /// Whether `utc_offset_s` was read for this spawn.
    pub offset_known: bool,
    /// The local UTC offset in seconds (0 when not known).
    pub utc_offset_s: i32,
}

/// How the read ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Loaded {
    /// stdin is not a sealed memory file: no backdrop, nothing waited for.
    NoBundle,
    /// The file could not be read within the bounds.
    Unreadable,
    /// The codec's verdict on the bytes, and what was kept.
    Decoded {
        header: Header,
        status: Status,
        /// The validated pictures, by connector name; empty when the bundle
        /// was ignored.
        blocks: Vec<OutputBlock>,
    },
}

impl Loaded {
    /// The header to use: the default (no failures carried) unless a valid
    /// header was read.
    #[must_use]
    pub fn header(&self) -> Header {
        match self {
            Self::Decoded { header, status, .. } if !matches!(status, Status::Ignored(_)) => {
                *header
            }
            _ => Header::default(),
        }
    }

    /// The pictures to draw: none unless a valid header was read.
    #[must_use]
    pub fn into_blocks(self) -> Vec<OutputBlock> {
        match self {
            Self::Decoded { status, blocks, .. } if !matches!(status, Status::Ignored(_)) => blocks,
            _ => Vec::new(),
        }
    }
}

const ALL_SEALS: SealFlags = SealFlags::from_bits_retain(
    SealFlags::SHRINK.bits()
        | SealFlags::GROW.bits()
        | SealFlags::WRITE.bits()
        | SealFlags::SEAL.bits(),
);

/// Reads and validates the bundle on `fd`.
#[must_use]
pub fn read_bundle(fd: BorrowedFd<'_>, deadline: Duration) -> Loaded {
    // Only a memory file can carry seals: this is also what keeps a pipe, a
    // terminal or `/dev/null` from ever being read.
    let Ok(seals) = fcntl_get_seals(fd) else {
        return Loaded::NoBundle;
    };
    let Ok(stat) = fstat(fd) else {
        return Loaded::Unreadable;
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Loaded::NoBundle;
    }
    let Ok(size) = u64::try_from(stat.st_size) else {
        return Loaded::Unreadable;
    };
    let seals_ok = seals.contains(ALL_SEALS);
    if size > MAX_FILE_BYTES {
        // Never allocate for what the bound refuses.
        return Loaded::Decoded {
            header: Header::default(),
            status: Status::Ignored(Ignored::TooLarge),
            blocks: Vec::new(),
        };
    }
    let Ok(len) = usize::try_from(size) else {
        return Loaded::Unreadable;
    };
    let end = Instant::now() + deadline;
    let mut bytes = vec![0u8; len];
    let mut filled = 0;
    while filled < len {
        if Instant::now() >= end {
            return Loaded::Unreadable;
        }
        let offset = u64::try_from(filled).unwrap_or(u64::MAX);
        match rustix::io::pread(fd, &mut bytes[filled..], offset) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(rustix::io::Errno::INTR) => {}
            Err(_) => return Loaded::Unreadable,
        }
    }
    bytes.truncate(filled);
    let decoded = decode(&bytes, FileMeta { size, seals_ok });
    Loaded::Decoded {
        header: Header {
            prior_failures: decoded.bundle.prior_failures,
            offset_known: decoded.bundle.offset_known,
            utc_offset_s: decoded.bundle.utc_offset_s,
        },
        status: decoded.status,
        blocks: decoded.bundle.outputs,
    }
}

/// An output's backdrop at the size of its buffer: the picture scaled to
/// cover the output (centred, the overflow cropped), smoothed, with the scrim
/// already over it. Opaque `XRGB8888` as `wl_shm` takes it, rows of exactly
/// `width * 4` bytes, so the prompt's contrast depends on the scrim and never
/// on the picture.
#[derive(Debug, PartialEq, Eq)]
pub struct Base {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// Where one destination sample reads from one axis of the picture: the two
/// neighbouring source indices and the weight (0..=255) of the second.
type Tap = (usize, usize, u32);

/// The taps of an axis of `dst` samples over `src` source samples, scaled by
/// `scale` (destination per source) and centred.
fn axis_taps(src: usize, dst: usize, scale: f64) -> Vec<Tap> {
    let visible = dst as f64 / scale;
    let origin = (src as f64 - visible) / 2.0;
    let last = src.saturating_sub(1);
    (0..dst)
        .map(|i| {
            let at = (origin + (i as f64 + 0.5) / scale - 0.5).clamp(0.0, last as f64);
            let low = (at.floor() as usize).min(last);
            let high = (low + 1).min(last);
            let weight = (((at - low as f64) * 256.0) as u32).min(255);
            (low, high, weight)
        })
        .collect()
}

/// For each channel, the value of every input byte once `scrim` (straight
/// alpha) lies over it.
fn scrim_table(scrim: Rgba) -> [[u8; 256]; 3] {
    let alpha = scrim.a.clamp(0.0, 1.0);
    let mut table = [[0u8; 256]; 3];
    for (channel, ink) in [scrim.r, scrim.g, scrim.b].into_iter().enumerate() {
        for (value, slot) in table[channel].iter_mut().enumerate() {
            let mixed = value as f32 * (1.0 - alpha) + f32::from(ink) * alpha;
            *slot = mixed.round().clamp(0.0, 255.0) as u8;
        }
    }
    table
}

impl Base {
    /// The width in pixels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The height in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Bytes per row.
    #[must_use]
    pub fn stride(&self) -> usize {
        self.width as usize * 4
    }

    /// `XRGB8888`, little endian, `stride() * height()` bytes.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Upscales `block` to `size` (buffer pixels) and lays `scrim` over it.
    /// `None` for a size that is empty, past [`MAX_BASE_PIXELS`] or cannot be
    /// allocated: the output then stays black.
    #[must_use]
    pub fn compute(block: &OutputBlock, size: (u32, u32), scrim: Rgba) -> Option<Self> {
        let (dst_w, dst_h) = (size.0 as usize, size.1 as usize);
        let area = u64::from(size.0) * u64::from(size.1);
        if size.0 == 0 || size.1 == 0 || area > MAX_BASE_PIXELS {
            return None;
        }
        let (src_w, src_h) = (usize::from(block.width()), usize::from(block.height()));
        let src = block.pixels();
        if src.len() != src_w * src_h * 4 {
            return None;
        }
        let scale = (dst_w as f64 / src_w as f64).max(dst_h as f64 / src_h as f64);
        let columns = axis_taps(src_w, dst_w, scale);
        let rows = axis_taps(src_h, dst_h, scale);
        let table = scrim_table(scrim);

        let len = dst_w * dst_h * 4;
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(len).ok()?;
        pixels.resize(len, 0);
        let src_row = |y: usize| &src[y * src_w * 4..(y + 1) * src_w * 4];
        for (out, &(y0, y1, fy)) in pixels.chunks_exact_mut(dst_w * 4).zip(&rows) {
            let (upper, lower) = (src_row(y0), src_row(y1));
            for (px, &(x0, x1, fx)) in out.as_chunks_mut::<4>().0.iter_mut().zip(&columns) {
                for (channel, lut) in table.iter().enumerate() {
                    let at = |row: &[u8], x: usize| u32::from(row[x * 4 + channel]);
                    let top = at(upper, x0) * (256 - fx) + at(upper, x1) * fx;
                    let bottom = at(lower, x0) * (256 - fx) + at(lower, x1) * fx;
                    let mixed = (top * (256 - fy) + bottom * fy + 32_768) >> 16;
                    // Blue, green, red, unused: channel 0 is red.
                    px[2 - channel] = lut[mixed.min(255) as usize];
                }
                px[3] = 0xff;
            }
        }
        Some(Self {
            width: size.0,
            height: size.1,
            pixels,
        })
    }
}

/// What the loop asks the worker for.
struct Request {
    name: String,
    size: (u32, u32),
}

/// What the worker answers.
struct Computed {
    name: String,
    size: (u32, u32),
    base: Option<Arc<Base>>,
}

/// The loop's wake-up: called from the worker after every answer, so the loop
/// redraws with the base that arrived.
pub type WakeLoop = Box<dyn Fn() + Send + Sync>;

/// What the loop knows about one output's base.
enum Slot {
    /// Asked for at this size, not answered yet.
    Pending((u32, u32)),
    /// Computed.
    Ready(Arc<Base>),
    /// The worker could not compute it at this size: black, not asked again.
    Failed((u32, u32)),
}

/// The `backdrop` worker and the cache of its answers (design §4.10).
///
/// Owned by the loop thread. The worker only upscales; it reads no file and
/// touches no Wayland object.
pub struct Backdrop {
    /// What the worker needs and has not been given yet: the worker is created
    /// by [`Backdrop::start`], never by `spawn`, because the signal mask of
    /// the loop must be set first (finding C1 of review A: a thread created
    /// before it inherits no mask and the default action of a signal the lock
    /// ignores would kill it).
    unstarted: Option<Unstarted>,
    requests: Option<Sender<Request>>,
    results: Receiver<Computed>,
    /// The connector names that have a picture.
    pictured: BTreeSet<String>,
    slots: BTreeMap<String, Slot>,
    wake: Arc<OnceLock<WakeLoop>>,
    computed: Arc<AtomicUsize>,
    thread: Option<JoinHandle<()>>,
}

/// The worker's inputs, held until the loop has blocked its signals.
struct Unstarted {
    blocks: Vec<OutputBlock>,
    scrim: Rgba,
    requests: Receiver<Request>,
    results: Sender<Computed>,
}

impl Backdrop {
    /// Holds the validated `blocks` with `scrim` to lay over every base. No
    /// thread exists yet: the worker starts at the first [`Backdrop::set_wake`]
    /// or [`Backdrop::base_for`], which the loop makes after it blocked its
    /// signals. `None` when there is nothing to draw.
    #[must_use]
    pub fn spawn(blocks: Vec<OutputBlock>, scrim: Rgba) -> Option<Self> {
        if blocks.is_empty() {
            return None;
        }
        let pictured = blocks.iter().map(|b| b.name().to_owned()).collect();
        let (request_tx, request_rx) = mpsc::channel::<Request>();
        let (result_tx, result_rx) = mpsc::channel::<Computed>();
        let wake: Arc<OnceLock<WakeLoop>> = Arc::new(OnceLock::new());
        Some(Self {
            unstarted: Some(Unstarted {
                blocks,
                scrim,
                requests: request_rx,
                results: result_tx,
            }),
            requests: Some(request_tx),
            results: result_rx,
            pictured,
            slots: BTreeMap::new(),
            wake,
            computed: Arc::new(AtomicUsize::new(0)),
            thread: None,
        })
    }

    /// Creates the worker thread, once. Called by the loop only, after its
    /// signal mask is set. When the thread cannot start the requests are
    /// refused from now on and what was asked is black.
    fn start(&mut self) {
        let Some(Unstarted {
            blocks,
            scrim,
            requests,
            results,
        }) = self.unstarted.take()
        else {
            return;
        };
        let (wake, computed) = (Arc::clone(&self.wake), Arc::clone(&self.computed));
        let started = std::thread::Builder::new()
            .name("backdrop".to_owned())
            .spawn(move || work(&blocks, scrim, &requests, &results, &wake, &computed));
        match started {
            Ok(thread) => self.thread = Some(thread),
            Err(err) => {
                tracing::warn!(%err, "cannot start the backdrop worker: black");
                self.requests = None;
                for slot in self.slots.values_mut() {
                    if let Slot::Pending(size) = *slot {
                        *slot = Slot::Failed(size);
                    }
                }
            }
        }
    }

    /// Whether the worker thread exists.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.thread.is_some()
    }

    /// Sets what the worker calls after every answer, and starts the worker.
    /// Once; a second call is ignored.
    pub fn set_wake(&mut self, wake: WakeLoop) {
        let _ = self.wake.set(wake);
        self.start();
    }

    /// How many bases the worker has computed so far.
    #[must_use]
    pub fn computed(&self) -> usize {
        self.computed.load(Ordering::Relaxed)
    }

    /// Takes in what the worker has answered.
    pub fn collect(&mut self) {
        while let Ok(answer) = self.results.try_recv() {
            self.accept(answer);
        }
    }

    fn accept(&mut self, answer: Computed) {
        let asked = matches!(
            self.slots.get(&answer.name),
            Some(Slot::Pending(size)) if *size == answer.size
        );
        if !asked {
            // Answered for a size the output no longer has.
            return;
        }
        let slot = match answer.base {
            Some(base) => Slot::Ready(base),
            None => Slot::Failed(answer.size),
        };
        self.slots.insert(answer.name, slot);
    }

    /// The base of output `name` at `size` buffer pixels, once computed.
    ///
    /// Never blocks. The first time an output with a picture is asked for at
    /// a size, the worker is asked to compute it and the answer is `None`
    /// (black) until it arrives; the same size is never computed twice.
    /// An output without a picture is never asked for.
    pub fn base_for(&mut self, name: &str, size: (u32, u32)) -> Option<Arc<Base>> {
        self.start();
        self.collect();
        if !self.pictured.contains(name) {
            return None;
        }
        match self.slots.get(name) {
            Some(Slot::Ready(base)) if (base.width(), base.height()) == size => {
                return Some(Arc::clone(base));
            }
            Some(Slot::Pending(asked) | Slot::Failed(asked)) if *asked == size => return None,
            _ => {}
        }
        let sent = self.requests.as_ref().is_some_and(|requests| {
            requests
                .send(Request {
                    name: name.to_owned(),
                    size,
                })
                .is_ok()
        });
        let slot = if sent {
            Slot::Pending(size)
        } else {
            Slot::Failed(size)
        };
        self.slots.insert(name.to_owned(), slot);
        None
    }

    /// Forgets the bases of outputs that are gone.
    pub fn retain_outputs<'a>(&mut self, live: impl IntoIterator<Item = &'a str>) {
        let live: BTreeSet<&str> = live.into_iter().collect();
        self.slots.retain(|name, _| live.contains(name.as_str()));
    }

    /// Blocks until every asked base has been answered or `limit` passes
    /// (tests only: the loop never waits).
    #[cfg(test)]
    pub(crate) fn settle(&mut self, limit: Duration) -> bool {
        self.start();
        let end = Instant::now() + limit;
        loop {
            self.collect();
            if !self.slots.values().any(|s| matches!(s, Slot::Pending(_))) {
                return true;
            }
            let Some(left) = end.checked_duration_since(Instant::now()) else {
                return false;
            };
            if let Ok(answer) = self.results.recv_timeout(left) {
                self.accept(answer);
            }
        }
    }
}

impl Drop for Backdrop {
    fn drop(&mut self) {
        // Closing the request channel ends the worker's loop; the join is
        // bounded by one upscale.
        self.requests = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Drops the requests the loop has already replaced: of everything queued,
/// only the last one per output is worth computing (a resize storm asks for
/// many sizes of the same output and the loop caches only the newest).
fn latest_per_output(first: Request, queued: impl Iterator<Item = Request>) -> Vec<Request> {
    let mut latest: BTreeMap<String, Request> = BTreeMap::new();
    for request in std::iter::once(first).chain(queued) {
        latest.insert(request.name.clone(), request);
    }
    latest.into_values().collect()
}

/// The worker: answers the newest request of each output, until the loop
/// drops the request channel.
fn work(
    blocks: &[OutputBlock],
    scrim: Rgba,
    requests: &Receiver<Request>,
    results: &Sender<Computed>,
    wake: &OnceLock<WakeLoop>,
    computed: &AtomicUsize,
) {
    while let Ok(first) = requests.recv() {
        for request in latest_per_output(first, std::iter::from_fn(|| requests.try_recv().ok())) {
            let base = blocks
                .iter()
                .find(|block| block.name() == request.name)
                .and_then(|block| Base::compute(block, request.size, scrim))
                .map(Arc::new);
            computed.fetch_add(1, Ordering::Relaxed);
            let answer = Computed {
                name: request.name,
                size: request.size,
                base,
            };
            if results.send(answer).is_err() {
                return;
            }
            if let Some(wake) = wake.get() {
                wake();
            }
        }
    }
}

/// Bundles and pictures for the tests of this crate.
#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use celestina_lock_proto::{encode, Bundle};
    use rustix::fs::{fcntl_add_seals, ftruncate, memfd_create, MemfdFlags};
    use std::os::fd::OwnedFd;

    /// A `width x height` block of one colour.
    pub(crate) fn solid(name: &str, width: u16, height: u16, rgb: [u8; 3]) -> OutputBlock {
        let pixels = std::iter::repeat_n(
            [rgb[0], rgb[1], rgb[2], 255],
            usize::from(width) * usize::from(height),
        )
        .flatten()
        .collect();
        OutputBlock::new(name, width, height, pixels).expect("block")
    }

    /// The bytes of a bundle with these blocks and no failures.
    pub(crate) fn bundle_of(blocks: Vec<OutputBlock>) -> Vec<u8> {
        encode(&Bundle {
            prior_failures: 0,
            offset_known: false,
            utc_offset_s: 0,
            outputs: blocks,
        })
    }

    /// A memory file holding `bytes`, sealed or not.
    pub(crate) fn memfd(bytes: &[u8], seal: bool) -> OwnedFd {
        let fd =
            memfd_create("bundle", MemfdFlags::ALLOW_SEALING | MemfdFlags::CLOEXEC).expect("memfd");
        ftruncate(&fd, u64::try_from(bytes.len()).expect("len")).expect("truncate");
        let mut written = 0;
        while written < bytes.len() {
            written +=
                rustix::io::pwrite(&fd, &bytes[written..], u64::try_from(written).expect("o"))
                    .expect("pwrite");
        }
        if seal {
            fcntl_add_seals(&fd, ALL_SEALS).expect("seal");
        }
        fd
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{bundle_of, memfd, solid};
    use super::*;
    use celestina_lock_proto::{encode, Bundle};
    use rustix::fs::{fcntl_add_seals, ftruncate, memfd_create, MemfdFlags};
    use std::os::fd::AsFd;

    fn header_bundle(prior_failures: u8) -> Vec<u8> {
        encode(&Bundle::header_only(prior_failures))
    }

    #[test]
    fn a_sealed_header_only_bundle_yields_its_prior_failures() {
        let fd = memfd(&header_bundle(3), true);
        let loaded = read_bundle(fd.as_fd(), READ_DEADLINE);
        assert_eq!(loaded.header().prior_failures, 3);
        assert!(matches!(
            loaded,
            Loaded::Decoded {
                status: Status::Complete,
                ..
            }
        ));
    }

    #[test]
    fn the_file_offset_does_not_matter() {
        let fd = memfd(&header_bundle(5), true);
        // A reader that had already consumed the file still finds the magic.
        let mut sink = [0u8; 64];
        let _ = rustix::io::read(&fd, &mut sink);
        assert_eq!(
            read_bundle(fd.as_fd(), READ_DEADLINE)
                .header()
                .prior_failures,
            5
        );
    }

    #[test]
    fn an_unsealed_memfd_is_ignored_whole() {
        let fd = memfd(&header_bundle(4), false);
        let loaded = read_bundle(fd.as_fd(), READ_DEADLINE);
        assert_eq!(loaded.header(), Header::default());
        assert!(matches!(
            loaded,
            Loaded::Decoded {
                status: Status::Ignored(Ignored::MissingSeal),
                ..
            }
        ));
    }

    #[test]
    fn stdin_that_is_not_a_memfd_means_no_bundle() {
        let (reader, _writer) = std::io::pipe().expect("pipe");
        assert_eq!(read_bundle(reader.as_fd(), READ_DEADLINE), Loaded::NoBundle);
        let null = std::fs::File::open("/dev/null").expect("null");
        assert_eq!(read_bundle(null.as_fd(), READ_DEADLINE), Loaded::NoBundle);
        let plain = std::fs::File::open("/proc/self/status").expect("status");
        assert_eq!(read_bundle(plain.as_fd(), READ_DEADLINE), Loaded::NoBundle);
        assert_eq!(Loaded::NoBundle.header(), Header::default());
    }

    #[test]
    fn a_bundle_past_16_mib_is_ignored_without_reading_it() {
        let fd = memfd_create("big", MemfdFlags::ALLOW_SEALING).expect("memfd");
        ftruncate(&fd, MAX_FILE_BYTES + 1).expect("truncate");
        fcntl_add_seals(&fd, ALL_SEALS).expect("seal");
        let loaded = read_bundle(fd.as_fd(), READ_DEADLINE);
        assert_eq!(
            loaded,
            Loaded::Decoded {
                header: Header::default(),
                status: Status::Ignored(Ignored::TooLarge),
                blocks: Vec::new(),
            }
        );
    }

    #[test]
    fn bad_magic_a_short_file_and_a_size_mismatch_are_ignored() {
        let mut bad = header_bundle(2);
        bad[0] = b'X';
        for bytes in [bad, vec![1, 2, 3], {
            let mut long = header_bundle(2);
            long.push(0);
            long
        }] {
            let fd = memfd(&bytes, true);
            assert_eq!(
                read_bundle(fd.as_fd(), READ_DEADLINE).header(),
                Header::default()
            );
        }
    }

    #[test]
    fn the_offset_facts_are_kept() {
        let bundle = Bundle {
            prior_failures: 1,
            offset_known: true,
            utc_offset_s: 7200,
            outputs: Vec::new(),
        };
        let fd = memfd(&encode(&bundle), true);
        let header = read_bundle(fd.as_fd(), READ_DEADLINE).header();
        assert_eq!(
            header,
            Header {
                prior_failures: 1,
                offset_known: true,
                utc_offset_s: 7200
            }
        );
    }

    // ---- the pixels (CEL-2-H-b) ----

    const SCRIM_60: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 0.60,
    };

    #[test]
    fn a_sealed_bundle_yields_its_validated_blocks() {
        let blocks = vec![
            solid("DP-1", 4, 3, [10, 20, 30]),
            solid("DP-2", 2, 2, [1, 2, 3]),
        ];
        let fd = memfd(&bundle_of(blocks.clone()), true);
        let loaded = read_bundle(fd.as_fd(), READ_DEADLINE);
        assert_eq!(loaded.into_blocks(), blocks);
    }

    #[test]
    fn a_bad_block_keeps_the_blocks_before_it() {
        let mut bytes = bundle_of(vec![
            solid("DP-1", 2, 2, [9, 9, 9]),
            solid("DP-2", 2, 2, [8, 8, 8]),
        ]);
        // Corrupt the second block's width to zero: name_len(1) + "DP-2"(4).
        let first = 13 + 1 + 4 + 4 + 16;
        bytes[first + 1 + 4] = 0;
        bytes[first + 1 + 5] = 0;
        let fd = memfd(&bytes, true);
        let loaded = read_bundle(fd.as_fd(), READ_DEADLINE);
        assert!(matches!(
            loaded,
            Loaded::Decoded {
                status: Status::BlockRejected { .. },
                ..
            }
        ));
        let blocks = loaded.into_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name(), "DP-1");
    }

    #[test]
    fn an_ignored_bundle_has_no_blocks() {
        let fd = memfd(&bundle_of(vec![solid("DP-1", 2, 2, [9, 9, 9])]), false);
        assert!(read_bundle(fd.as_fd(), READ_DEADLINE)
            .into_blocks()
            .is_empty());
        assert!(Loaded::NoBundle.into_blocks().is_empty());
        assert!(Loaded::Unreadable.into_blocks().is_empty());
    }

    #[test]
    fn the_worker_reads_within_2_s_and_16_mib() {
        // The bounds are the design's (§4.3 (a), §4.10).
        assert_eq!(READ_DEADLINE, Duration::from_secs(2));
        assert_eq!(MAX_FILE_BYTES, 16 * 1024 * 1024);
        // A deadline that has already passed reads nothing: the bundle is
        // slow, so it is black (no blocks, no failures carried).
        let fd = memfd(&bundle_of(vec![solid("DP-1", 2, 2, [9, 9, 9])]), true);
        let slow = read_bundle(fd.as_fd(), Duration::ZERO);
        assert_eq!(slow, Loaded::Unreadable);
        assert!(slow.into_blocks().is_empty());
        // A file one byte past the bound is refused before it is allocated.
        let big = memfd_create("big", MemfdFlags::ALLOW_SEALING).expect("memfd");
        ftruncate(&big, MAX_FILE_BYTES + 1).expect("truncate");
        fcntl_add_seals(&big, ALL_SEALS).expect("seal");
        let loaded = read_bundle(big.as_fd(), READ_DEADLINE);
        assert!(loaded.into_blocks().is_empty());
        // Eight maximal outputs fit under the bound, so a valid bundle is
        // never refused for size.
        let maximal: Vec<OutputBlock> = (1..=8)
            .map(|n| solid(&format!("DP-{n}"), 640, 640, [n, n, n]))
            .collect();
        let bytes = bundle_of(maximal);
        assert!(u64::try_from(bytes.len()).expect("len") <= MAX_FILE_BYTES);
        let fd = memfd(&bytes, true);
        assert_eq!(
            read_bundle(fd.as_fd(), READ_DEADLINE).into_blocks().len(),
            8
        );
        // The production reader is the one the launch calls, with this bound.
        let launch = include_str!("launch.rs");
        assert!(launch
            .contains("backdrop::read_bundle(std::io::stdin().as_fd(), backdrop::READ_DEADLINE)"));
    }

    /// `(blue, green, red)` of pixel `(x, y)` of a base.
    fn at(base: &Base, x: usize, y: usize) -> [u8; 3] {
        let i = y * base.stride() + x * 4;
        [base.pixels()[i], base.pixels()[i + 1], base.pixels()[i + 2]]
    }

    #[test]
    fn a_base_is_the_picture_scaled_to_the_buffer_with_the_scrim_over_it() {
        // Pure white under black at 60 %: 255 * 0.4 = 102 everywhere, in XRGB
        // byte order with an opaque unused byte.
        let block = solid("DP-1", 16, 9, [255, 255, 255]);
        let base = Base::compute(&block, (160, 90), SCRIM_60).expect("base");
        assert_eq!((base.width(), base.height()), (160, 90));
        assert_eq!(base.pixels().len(), 160 * 90 * 4);
        for px in base.pixels().as_chunks::<4>().0.iter() {
            assert_eq!(*px, [102, 102, 102, 0xff]);
        }
        // Pure black stays black; the scrim never brightens.
        let dark = Base::compute(&solid("DP-1", 4, 4, [0, 0, 0]), (40, 40), SCRIM_60).expect("b");
        assert!(dark
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[..3] == [0, 0, 0]));
    }

    #[test]
    fn a_base_keeps_the_channel_order_and_covers_by_cropping() {
        // Left half red, right half blue, 2:1, shown on a square: the sides
        // are cropped evenly, the middle seam stays in the middle.
        let mut pixels = Vec::new();
        for _y in 0..4 {
            for x in 0..8 {
                pixels.extend_from_slice(if x < 4 {
                    &[200, 0, 0, 255]
                } else {
                    &[0, 0, 200, 255]
                });
            }
        }
        let block = OutputBlock::new("DP-1", 8, 4, pixels).expect("block");
        let clear = Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 0.0,
        };
        let base = Base::compute(&block, (40, 40), clear).expect("base");
        assert_eq!(
            at(&base, 2, 20),
            [0, 0, 200],
            "left is red: blue, green, red bytes"
        );
        assert_eq!(at(&base, 37, 20), [200, 0, 0], "right is blue");
        let seam = at(&base, 20, 20);
        assert!(seam[0] > 20 && seam[2] > 20, "the seam blends: {seam:?}");
    }

    #[test]
    fn a_base_refuses_an_empty_or_oversized_buffer() {
        let block = solid("DP-1", 4, 4, [1, 1, 1]);
        assert!(Base::compute(&block, (0, 100), SCRIM_60).is_none());
        assert!(Base::compute(&block, (100, 0), SCRIM_60).is_none());
        assert!(Base::compute(&block, (8192, 8192), SCRIM_60).is_none());
        assert!(Base::compute(&block, (u32::MAX, u32::MAX), SCRIM_60).is_none());
    }

    #[test]
    fn an_upscaled_base_is_computed_once_per_output_and_cached() {
        let mut backdrop = Backdrop::spawn(
            vec![
                solid("DP-1", 8, 8, [255, 255, 255]),
                solid("DP-2", 8, 8, [0, 0, 255]),
            ],
            SCRIM_60,
        )
        .expect("worker");
        // Asked for, not there yet: black while the worker computes.
        let first = backdrop.base_for("DP-1", (320, 180));
        assert!(first.is_none() || backdrop.computed() <= 1);
        // Asking again before the answer sends nothing more.
        for _ in 0..5 {
            let _ = backdrop.base_for("DP-1", (320, 180));
        }
        assert!(backdrop.base_for("DP-2", (160, 90)).is_none() || backdrop.computed() <= 2);
        assert!(backdrop.settle(Duration::from_secs(10)));
        let one = backdrop.base_for("DP-1", (320, 180)).expect("cached");
        let again = backdrop.base_for("DP-1", (320, 180)).expect("cached");
        assert!(Arc::ptr_eq(&one, &again), "the same base, not a copy");
        assert!(backdrop.base_for("DP-2", (160, 90)).is_some());
        assert_eq!(backdrop.computed(), 2, "one per output");
        // A new size is a new base; the old one is replaced, not kept.
        assert!(backdrop.base_for("DP-1", (640, 360)).is_none());
        assert!(backdrop.settle(Duration::from_secs(10)));
        assert_eq!(backdrop.computed(), 3);
        assert!(backdrop.base_for("DP-1", (640, 360)).is_some());
        assert_eq!(backdrop.computed(), 3);
    }

    #[test]
    fn spawn_creates_no_thread_and_the_worker_starts_with_the_loop() {
        let mut backdrop =
            Backdrop::spawn(vec![solid("DP-1", 4, 4, [9, 9, 9])], SCRIM_60).expect("held");
        assert!(!backdrop.is_running(), "no thread before the signal mask");
        backdrop.set_wake(Box::new(|| {}));
        assert!(backdrop.is_running());
        // A second wake does not start a second worker.
        backdrop.set_wake(Box::new(|| {}));
        assert!(backdrop.is_running());
        // And a request without a wake starts it as well.
        let mut other =
            Backdrop::spawn(vec![solid("DP-1", 4, 4, [9, 9, 9])], SCRIM_60).expect("held");
        let _ = other.base_for("DP-1", (32, 32));
        assert!(other.is_running());
    }

    #[test]
    fn only_the_newest_request_of_each_output_is_computed() {
        let ask = |name: &str, width: u32| Request {
            name: name.to_owned(),
            size: (width, 9),
        };
        let kept = latest_per_output(
            ask("DP-1", 1),
            [
                ask("DP-2", 2),
                ask("DP-1", 3),
                ask("DP-1", 4),
                ask("DP-2", 5),
            ]
            .into_iter(),
        );
        let sizes: Vec<_> = kept.iter().map(|r| (r.name.as_str(), r.size.0)).collect();
        assert_eq!(sizes, [("DP-1", 4), ("DP-2", 5)]);
        // And through the real worker: a queue filled before it starts is
        // reduced to one computation per output.
        let (request_tx, request_rx) = mpsc::channel::<Request>();
        let (result_tx, result_rx) = mpsc::channel::<Computed>();
        for width in [20, 30, 40, 50] {
            request_tx.send(ask("DP-1", width)).expect("queued");
        }
        drop(request_tx);
        let blocks = vec![solid("DP-1", 4, 4, [9, 9, 9])];
        let computed = AtomicUsize::new(0);
        work(
            &blocks,
            SCRIM_60,
            &request_rx,
            &result_tx,
            &OnceLock::new(),
            &computed,
        );
        drop(result_tx);
        let answers: Vec<_> = result_rx.iter().map(|a| a.size).collect();
        assert_eq!(answers, [(50, 9)]);
        assert_eq!(computed.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn an_output_without_a_block_is_never_asked_for() {
        let mut backdrop =
            Backdrop::spawn(vec![solid("DP-1", 4, 4, [9, 9, 9])], SCRIM_60).expect("worker");
        assert!(backdrop.base_for("HDMI-A-1", (100, 100)).is_none());
        assert!(backdrop.settle(Duration::from_secs(1)));
        assert_eq!(backdrop.computed(), 0);
        // And no pictures means no worker at all.
        assert!(Backdrop::spawn(Vec::new(), SCRIM_60).is_none());
    }

    #[test]
    fn a_size_the_worker_refuses_stays_black_and_is_not_asked_again() {
        let mut backdrop =
            Backdrop::spawn(vec![solid("DP-1", 4, 4, [9, 9, 9])], SCRIM_60).expect("worker");
        assert!(backdrop.base_for("DP-1", (9000, 9000)).is_none());
        assert!(backdrop.settle(Duration::from_secs(5)));
        assert!(backdrop.base_for("DP-1", (9000, 9000)).is_none());
        assert_eq!(backdrop.computed(), 1);
    }

    #[test]
    fn the_worker_wakes_the_loop_after_each_answer() {
        let mut backdrop =
            Backdrop::spawn(vec![solid("DP-1", 4, 4, [9, 9, 9])], SCRIM_60).expect("worker");
        let woken = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&woken);
        backdrop.set_wake(Box::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        }));
        let _ = backdrop.base_for("DP-1", (64, 64));
        assert!(backdrop.settle(Duration::from_secs(5)));
        // The wake follows the answer by a hair: give it a moment.
        let end = Instant::now() + Duration::from_secs(2);
        while woken.load(Ordering::SeqCst) == 0 && Instant::now() < end {
            std::thread::yield_now();
        }
        assert_eq!(woken.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn outputs_that_go_away_lose_their_base() {
        let mut backdrop = Backdrop::spawn(
            vec![
                solid("DP-1", 4, 4, [9, 9, 9]),
                solid("DP-2", 4, 4, [9, 9, 9]),
            ],
            SCRIM_60,
        )
        .expect("worker");
        let _ = backdrop.base_for("DP-1", (32, 32));
        let _ = backdrop.base_for("DP-2", (32, 32));
        assert!(backdrop.settle(Duration::from_secs(5)));
        backdrop.retain_outputs(["DP-1"]);
        assert!(backdrop.base_for("DP-1", (32, 32)).is_some());
        // DP-2 comes back: it is computed again, not remembered.
        assert!(backdrop.base_for("DP-2", (32, 32)).is_none());
        assert!(backdrop.settle(Duration::from_secs(5)));
        assert_eq!(backdrop.computed(), 3);
    }
}
