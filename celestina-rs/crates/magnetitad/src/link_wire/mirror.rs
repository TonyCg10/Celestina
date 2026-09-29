//! The mirror as a capability of the link: the desktop asks the paired
//! phone to capture, the phone streams an HEVC elementary stream on a
//! unidirectional QUIC stream of a fixed id, and the desktop decodes it in
//! a window it owns — `ffmpeg` remuxing into `mpv`, the pair the
//! `MAG-P0-B` spike measured, started with the session's display
//! variables the same way the `adb` mirror starts scrcpy.
//!
//! Wanting is a standing intent kept here; the session's tick turns it
//! into `MirrorStart` and `MirrorStop`. A player trait keeps the loopback
//! tests from opening a window: they record the bytes.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Mutex;

use magnetita_link::RecvStream;
use magnetita_proto::capability;
use magnetita_proto::mirror::{Codec, MirrorKeyframe, MirrorStart, MirrorStarted, MirrorStop};
use magnetita_proto::Envelope;

use crate::lock::LockOk;
use crate::runtime::log;

/// The transfer id the phone's video stream carries; audio is the next.
pub(crate) const VIDEO_STREAM: u32 = 0xFFFF_0001;
pub(crate) const AUDIO_STREAM: u32 = 0xFFFF_0002;

/// Where the decoded picture goes.
pub(crate) trait MirrorPlayer: Send + Sync {
    /// Opens the window for a stream of this shape; the returned sink takes
    /// the bytes. `None` when no window can be opened (no display, no tool).
    fn open(&self, started: &MirrorStarted) -> Option<Box<dyn VideoSink>>;
}

pub(crate) trait VideoSink: Send {
    fn write(&mut self, bytes: &[u8]);
    fn close(&mut self);
}

/// Where the application reads the picture: a FIFO under the runtime dir,
/// raw HEVC or H.264 as the phone encodes it. The Magnetita window opens
/// it in its own libmpv; `Mirror1`'s `LinkVideo` names it while the
/// mirror streams.
/// A new name per stream: a window that watches the name learns a restart
/// (a rotation, a second start) and reopens, instead of reading a FIFO the
/// daemon has already replaced.
static VIDEO_PATH: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);
static VIDEO_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// The FIFO the current stream writes, if any.
pub(crate) fn video_fifo() -> Option<std::path::PathBuf> {
    VIDEO_PATH.lock_ok().clone()
}

/// The next FIFO name, in the daemon's private runtime directory; without
/// one there is no picture rather than a FIFO under `/tmp`.
fn next_video_fifo() -> Result<std::path::PathBuf, celestina_core::xdg::PrivateDirError> {
    let n = VIDEO_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(crate::runtime::private_runtime_base()?.join(format!("mirror-{n}.video")))
}

/// How many access units wait for the window: two seconds at 60 frames.
const FEED_QUEUE: usize = 120;
/// How many chunks of the phone's sound wait for `pw-cat`: about half a
/// second of 16-bit stereo at 48 kHz (192 KB/s) in the stream's 16 KiB
/// reads. Sound later than that is dropped rather than played out of step
/// with the picture.
const SOUND_QUEUE: usize = 6;

/// The picture as a FIFO the application's window reads. Access units wait
/// in a bounded queue until a reader opens the FIFO and while it decodes.
/// When the reader falls behind, or none has opened the FIFO, the queue does
/// not grow: units are dropped up to the next key frame, and the phone is
/// asked for one at once, so the picture resumes decodable instead of the
/// daemon holding the whole stream in memory. A reader that closes and
/// reopens gets the stream again from its next key frame.
pub(crate) struct FifoPlayer;

/// A unit as the feed queues it: its bytes and whether it is a key frame.
type Unit = (Vec<u8>, bool);

/// What [`Feed::push`] did with a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fed {
    Queued,
    /// Dropped while waiting for the next key frame to fit.
    Dropped,
    /// Dropped, and the phone should be asked for a key frame: the queue
    /// just overflowed, or a key frame found it still full.
    WantKey,
    /// The feed thread is gone.
    Closed,
}

/// The feed's bounded queue and its one policy: past the bound, drop to the
/// next key frame that fits.
struct Feed {
    tx: SyncSender<Unit>,
    dropping: bool,
}

impl Feed {
    fn new(capacity: usize) -> (Self, Receiver<Unit>) {
        let (tx, rx) = sync_channel(capacity);
        (
            Self {
                tx,
                dropping: false,
            },
            rx,
        )
    }

    fn push(&mut self, unit: Unit) -> Fed {
        let key = unit.1;
        if self.dropping && !key {
            return Fed::Dropped;
        }
        match self.tx.try_send(unit) {
            Ok(()) => {
                self.dropping = false;
                Fed::Queued
            }
            Err(TrySendError::Full(_)) => {
                let first = !self.dropping;
                self.dropping = true;
                if first || key {
                    Fed::WantKey
                } else {
                    Fed::Dropped
                }
            }
            Err(TrySendError::Disconnected(_)) => Fed::Closed,
        }
    }
}

/// The request that makes the phone's encoder send a key frame now.
fn keyframe_request() -> Envelope {
    Envelope {
        capability: capability::MIRROR,
        kind: MirrorKeyframe::KIND,
        id: 0,
        body: MirrorKeyframe.encode(),
    }
}

struct FifoSink {
    feed: Option<Feed>,
    stopping: std::sync::Arc<std::sync::atomic::AtomicBool>,
    path: std::path::PathBuf,
    /// The stream cut into access units, and what a new reader needs first.
    units: std::sync::Arc<Mutex<super::mirror_stream::AccessUnits>>,
    units_in: u64,
    keys_in: u64,
    arrived: u64,
    last_report: std::time::Instant,
}

impl MirrorPlayer for FifoPlayer {
    fn open(&self, started: &MirrorStarted) -> Option<Box<dyn VideoSink>> {
        let units = std::sync::Arc::new(Mutex::new(super::mirror_stream::AccessUnits::new(
            started.codec,
        )));
        let feed_units = std::sync::Arc::clone(&units);
        let path = match next_video_fifo() {
            Ok(path) => path,
            Err(e) => {
                log("mirror", &format!("video fifo: {e}"));
                return None;
            }
        };
        let _ = std::fs::remove_file(&path);
        if let Err(e) = rustix::fs::mknodat(
            rustix::fs::CWD,
            &path,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        ) {
            log("mirror", &format!("video fifo: {e}"));
            return None;
        }
        let (feed, rx) = Feed::new(FEED_QUEUE);
        let stopping = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop = std::sync::Arc::clone(&stopping);
        let feed_path = path.clone();
        std::thread::Builder::new()
            .name("magnetita-mirror-feed".into())
            .spawn(move || {
                'readers: while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    // Opening for writing blocks until a reader opens the
                    // other end; the close path opens one to unblock this.
                    let Ok(mut fifo) = std::fs::OpenOptions::new().write(true).open(&feed_path)
                    else {
                        break;
                    };
                    if stop.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    // A reader that opens mid-stream: drop what queued while
                    // nobody read, ask the phone for a key frame, and start
                    // the reader at it, so it decodes from its next frame.
                    while rx.try_recv().is_ok() {
                        QUEUED.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    own().queue_input(keyframe_request());
                    let mut started = false;
                    let (mut units, mut bytes, mut keys, mut skipped) = (0u64, 0u64, 0u64, 0u64);
                    let mut blocked = std::time::Duration::ZERO;
                    let mut last_report = std::time::Instant::now();
                    log("mirror", "feed: reader opened the fifo");
                    loop {
                        let Ok((chunk, key)) = rx.recv() else {
                            break 'readers;
                        };
                        QUEUED.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                        if !started && !key {
                            skipped += 1;
                            continue;
                        }
                        if !started {
                            // The parameter sets go first: the encoder sent them
                            // once, before any key frame this reader will see.
                            let (params, param_types, unit_types) = {
                                let units = feed_units.lock_ok();
                                let params = units.params();
                                let param_types = units.nal_types(&params);
                                (params, param_types, units.nal_types(&chunk))
                            };
                            log(
                                "mirror",
                                &format!(
                                    "feed: first key unit after skipping {skipped}: parameter sets {param_types:?} ({} bytes), unit NAL types {unit_types:?} ({} bytes)",
                                    params.len(),
                                    chunk.len()
                                ),
                            );
                            if !params.is_empty() && fifo.write_all(&params).is_err() {
                                continue 'readers;
                            }
                        }
                        started = true;
                        units += 1;
                        bytes += chunk.len() as u64;
                        keys += u64::from(key);
                        let began = std::time::Instant::now();
                        let outcome = fifo.write_all(&chunk);
                        blocked += began.elapsed();
                        if last_report.elapsed() > std::time::Duration::from_secs(5) {
                            log(
                                "mirror",
                                &format!(
                                    "feed: {units} units, {bytes} bytes, {keys} keys, queued {}, {:.0} ms blocked in writes",
                                    QUEUED.load(std::sync::atomic::Ordering::Relaxed),
                                    blocked.as_secs_f64() * 1000.0
                                ),
                            );
                            (units, bytes, keys, blocked) = (0, 0, 0, std::time::Duration::ZERO);
                            last_report = std::time::Instant::now();
                        }
                        if outcome.is_err() {
                            // The reader went away: wait for the next one.
                            continue 'readers;
                        }
                    }
                }
            })
            .ok()?;
        *VIDEO_PATH.lock_ok() = Some(path.clone());
        Some(Box::new(FifoSink {
            feed: Some(feed),
            stopping,
            path,
            units,
            units_in: 0,
            keys_in: 0,
            arrived: 0,
            last_report: std::time::Instant::now(),
        }))
    }
}

/// Units queued for the feed and not yet written, for the log.
static QUEUED: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

impl VideoSink for FifoSink {
    fn write(&mut self, bytes: &[u8]) {
        let complete = self.units.lock_ok().push(bytes);
        self.arrived += bytes.len() as u64;
        for unit in &complete {
            self.units_in += 1;
            self.keys_in += u64::from(unit.1);
        }
        if self.last_report.elapsed() > std::time::Duration::from_secs(5) {
            log(
                "mirror",
                &format!(
                    "link: {} units in, {} keys, {} bytes",
                    self.units_in, self.keys_in, self.arrived
                ),
            );
            (self.units_in, self.keys_in, self.arrived) = (0, 0, 0);
            self.last_report = std::time::Instant::now();
        }
        if let Some(feed) = &mut self.feed {
            for unit in complete {
                match feed.push(unit) {
                    Fed::Queued => {
                        QUEUED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    Fed::WantKey => {
                        log(
                            "mirror",
                            "feed: the window fell behind; dropping to a key frame",
                        );
                        own().queue_input(keyframe_request());
                    }
                    Fed::Dropped | Fed::Closed => {}
                }
            }
        }
    }

    fn close(&mut self) {
        self.stopping
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.feed.take();
        // A feed thread blocked in its open needs a reader to appear once.
        let _ = rustix::fs::open(
            &self.path,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        );
        let _ = std::fs::remove_file(&self.path);
        let mut current = VIDEO_PATH.lock_ok();
        if current.as_deref() == Some(self.path.as_path()) {
            *current = None;
        }
    }
}

impl Drop for FifoSink {
    fn drop(&mut self) {
        self.close();
    }
}

/// `pw-cat` playing raw PCM from its stdin: the phone's sound on this
/// desktop, with no codec and no buffer beyond PipeWire's own.
struct Speaker {
    child: std::process::Child,
    tx: Option<SyncSender<Vec<u8>>>,
}

impl Speaker {
    fn open() -> Option<Self> {
        let mut child = std::process::Command::new("pw-cat")
            .args([
                "--playback",
                "--raw",
                "--format",
                "s16",
                "--rate",
                "48000",
                "--channels",
                "2",
                "--latency",
                "20ms",
                "-",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| log("mirror", &format!("sound: pw-cat: {e}")))
            .ok()?;
        let mut stdin = child.stdin.take()?;
        let (tx, rx) = sync_channel::<Vec<u8>>(SOUND_QUEUE);
        std::thread::Builder::new()
            .name("magnetita-mirror-sound".into())
            .spawn(move || {
                while let Ok(chunk) = rx.recv() {
                    if stdin.write_all(&chunk).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        log("mirror", "sound: playing the phone through pw-cat");
        Some(Self {
            child,
            tx: Some(tx),
        })
    }

    fn write(&mut self, bytes: &[u8]) {
        // Sound that cannot be played now is worth nothing later.
        if let Some(tx) = &self.tx {
            let _ = tx.try_send(bytes.to_vec());
        }
    }

    fn close(&mut self) {
        self.tx.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Speaker {
    fn drop(&mut self) {
        self.close();
    }
}

/// What the desktop wants and what it has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LinkState {
    Idle,
    Starting,
    Streaming { width: u16, height: u16 },
    Failed(String),
}

pub(crate) fn state_word(state: &LinkState) -> &'static str {
    match state {
        LinkState::Idle => "idle",
        LinkState::Starting => "starting",
        LinkState::Streaming { .. } => "streaming",
        LinkState::Failed(_) => "failed",
    }
}

/// The daemon's one own mirror: the standing intent, the options, the state
/// and the open window.
#[derive(Default)]
pub(crate) struct OwnMirror {
    wanted: Mutex<Option<MirrorStart>>,
    /// The session the mirror runs on: the one named by the request, else
    /// the first session to tick after it. Other sessions leave it alone.
    owner: Mutex<Option<String>>,
    /// The phone's address as the owning session sees it, for adb.
    host: Mutex<Option<std::net::IpAddr>>,
    /// The owning session's outbox: input goes straight to the link, not
    /// through the tick.
    outbox: Mutex<Option<super::writer::Outbox>>,
    state: Mutex<Option<LinkState>>,
    sink: Mutex<Option<Box<dyn VideoSink>>>,
    /// The phone's sound, played here while the phone streams it: a
    /// `pw-cat` child fed raw PCM (16-bit, 48 kHz, stereo) on its stdin.
    speaker: Mutex<Option<Speaker>>,
    /// Input for the phone, queued by `Mirror1` and drained by the session.
    input: Mutex<VecDeque<Envelope>>,
}

static OWN: std::sync::LazyLock<OwnMirror> = std::sync::LazyLock::new(OwnMirror::default);

pub(crate) fn own() -> &'static OwnMirror {
    &OWN
}

impl OwnMirror {
    pub(crate) fn request_start(&self, options: MirrorStart) {
        *self.wanted.lock_ok() = Some(options);
    }

    /// Wants the mirror from one phone in particular.
    #[cfg(test)]
    pub(crate) fn request_start_from(&self, device_id: &str, options: MirrorStart) {
        *self.owner.lock_ok() = Some(device_id.to_owned());
        self.request_start(options);
    }

    pub(crate) fn request_stop(&self) {
        *self.wanted.lock_ok() = None;
    }

    /// The phone's address, from the session that holds the link.
    pub(crate) fn set_host(&self, host: std::net::IpAddr) {
        *self.host.lock_ok() = Some(host);
    }

    /// Whether this session is the one the mirror runs on.
    pub(crate) fn owned_by(&self, device_id: &str) -> bool {
        self.owner.lock_ok().as_deref() == Some(device_id)
    }

    pub(crate) fn state(&self) -> LinkState {
        self.state.lock_ok().clone().unwrap_or(LinkState::Idle)
    }

    /// Input for the phone: down the owning session's link at once, or
    /// queued for the tick to hand over while no session owns the mirror.
    pub(crate) fn queue_input(&self, env: Envelope) {
        if let Some(outbox) = self.outbox.lock_ok().as_ref() {
            // A full queue means the phone stopped reading; input that late
            // is worth nothing, and the session ends at its send deadline.
            let _ = outbox.send(env);
            return;
        }
        let mut input = self.input.lock_ok();
        if input.len() < 1024 {
            input.push_back(env);
        }
    }

    /// The session's tick: what to send, if anything, given the intent.
    /// The owning session leaves its outbox so input skips the tick.
    pub(crate) fn tick(&self, device_id: &str, outbox: &super::writer::Outbox) -> Vec<Envelope> {
        let mut out = Vec::new();
        let wanted = *self.wanted.lock_ok();
        {
            let mut owner = self.owner.lock_ok();
            match owner.as_deref() {
                None if wanted.is_some() => *owner = Some(device_id.to_owned()),
                Some(id) if id == device_id => {}
                _ => return out,
            }
        }
        {
            let mut slot = self.outbox.lock_ok();
            if slot.is_none() {
                *slot = Some(outbox.clone());
            }
        }
        let state = self.state();
        match (wanted, &state) {
            (Some(options), LinkState::Idle) | (Some(options), LinkState::Failed(_)) => {
                *self.state.lock_ok() = Some(LinkState::Starting);
                out.push(Envelope {
                    capability: capability::MIRROR,
                    kind: MirrorStart::KIND,
                    id: 0,
                    body: options.encode(),
                });
            }
            (None, LinkState::Starting) | (None, LinkState::Streaming { .. }) => {
                self.close_window();
                crate::mirror::request_screen_off(false, None);
                *self.state.lock_ok() = Some(LinkState::Idle);
                out.push(Envelope {
                    capability: capability::MIRROR,
                    kind: MirrorStop::KIND,
                    id: 0,
                    body: MirrorStop.encode(),
                });
            }
            _ => {}
        }
        out.extend(self.input.lock_ok().drain(..));
        out
    }

    /// The phone answered: open the window.
    pub(crate) fn started(&self, player: &dyn MirrorPlayer, started: &MirrorStarted) {
        self.close_window();
        // The screen off is the adb worker's: the one path that can power
        // the panel down while the phone stays unlocked.
        let dark = self.wanted.lock_ok().is_some_and(|w| w.screen_off);
        crate::mirror::request_screen_off(dark, *self.host.lock_ok());
        match player.open(started) {
            Some(sink) => {
                *self.sink.lock_ok() = Some(sink);
                *self.state.lock_ok() = Some(LinkState::Streaming {
                    width: started.width,
                    height: started.height,
                });
            }
            None => {
                *self.state.lock_ok() = Some(LinkState::Failed("no window".into()));
                *self.wanted.lock_ok() = None;
            }
        }
    }

    /// The phone stopped, or the session ended.
    pub(crate) fn stopped(&self) {
        self.close_window();
        crate::mirror::request_screen_off(false, None);
        *self.state.lock_ok() = Some(LinkState::Idle);
        *self.wanted.lock_ok() = None;
        *self.outbox.lock_ok() = None;
        *self.owner.lock_ok() = None;
    }

    fn close_window(&self) {
        if let Some(mut sink) = self.sink.lock_ok().take() {
            sink.close();
        }
        if let Some(mut speaker) = self.speaker.lock_ok().take() {
            speaker.close();
        }
    }

    /// The audio stream arrived: play it until it ends.
    pub(crate) fn audio_stream(&'static self, mut stream: RecvStream) {
        tokio::spawn(async move {
            {
                let mut slot = self.speaker.lock_ok();
                if slot.is_none() {
                    *slot = Speaker::open();
                }
                if slot.is_none() {
                    return;
                }
            }
            loop {
                let chunk = match stream.read_chunk(16 * 1024, true).await {
                    Ok(Some(chunk)) => chunk,
                    Ok(None) | Err(_) => break,
                };
                let mut speaker = self.speaker.lock_ok();
                match speaker.as_mut() {
                    Some(speaker) => speaker.write(&chunk.bytes),
                    None => break,
                }
            }
        });
    }

    /// The video stream arrived: pump it into the window until it ends.
    pub(crate) fn video_stream(&'static self, mut stream: RecvStream) {
        tokio::spawn(async move {
            loop {
                let chunk = match stream.read_chunk(64 * 1024, true).await {
                    Ok(Some(chunk)) => chunk,
                    Ok(None) | Err(_) => break,
                };
                let mut sink = self.sink.lock_ok();
                match sink.as_mut() {
                    Some(sink) => sink.write(&chunk.bytes),
                    None => break,
                }
            }
        });
    }
}

/// The scrcpy vocabulary of `Mirror1`'s options as the wire's start.
pub(crate) fn start_from_options(
    options: &std::collections::HashMap<String, String>,
) -> MirrorStart {
    let get = |key: &str| options.get(key).map(String::as_str).unwrap_or("");
    MirrorStart {
        max_size: match get("resolution") {
            "modest" => 1080,
            "sharp" => 1920,
            "native" => 0,
            _ => 1440,
        },
        fps: match get("rate") {
            "calm" => 30,
            "fluid" => 120,
            _ => 60,
        },
        bitrate_kbps: match get("quality") {
            "thrifty" => 4000,
            "generous" => 16000,
            _ => 6000,
        },
        codec: Codec::Hevc,
        audio: get("audio") == "desktop",
        screen_off: get("screenOff") == "true",
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use std::sync::Arc;

    #[derive(Default)]
    pub(crate) struct Recorder {
        pub(crate) bytes: Arc<Mutex<Vec<u8>>>,
        pub(crate) opened: Arc<Mutex<Vec<(u16, u16)>>>,
    }

    struct RecordingSink(Arc<Mutex<Vec<u8>>>);

    impl VideoSink for RecordingSink {
        fn write(&mut self, bytes: &[u8]) {
            self.0.lock_ok().extend_from_slice(bytes);
        }
        fn close(&mut self) {}
    }

    impl MirrorPlayer for Recorder {
        fn open(&self, started: &MirrorStarted) -> Option<Box<dyn VideoSink>> {
            self.opened.lock_ok().push((started.width, started.height));
            Some(Box::new(RecordingSink(Arc::clone(&self.bytes))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MAG-7: the feed never holds more than its bound. Past it, units are
    /// dropped up to the next key frame, which is asked for at once, and the
    /// stream resumes at that key frame.
    #[test]
    fn a_full_feed_drops_to_the_next_key_frame() {
        let unit = |n: u8, key: bool| (vec![n], key);
        let (mut feed, rx) = Feed::new(3);
        assert_eq!(feed.push(unit(0, true)), Fed::Queued);
        assert_eq!(feed.push(unit(1, false)), Fed::Queued);
        assert_eq!(feed.push(unit(2, false)), Fed::Queued);
        // Nobody reads: the queue is full.
        assert_eq!(feed.push(unit(3, false)), Fed::WantKey);
        assert_eq!(feed.push(unit(4, false)), Fed::Dropped);
        // A key frame that finds it still full asks again.
        assert_eq!(feed.push(unit(5, true)), Fed::WantKey);
        assert_eq!(rx.try_recv().unwrap(), unit(0, true));
        // Room again, but only a key frame restarts the stream.
        assert_eq!(feed.push(unit(6, false)), Fed::Dropped);
        assert_eq!(feed.push(unit(7, true)), Fed::Queued);
        assert_eq!(feed.push(unit(8, false)), Fed::WantKey);
        let rest: Vec<_> = rx.try_iter().collect();
        assert_eq!(rest, [unit(1, false), unit(2, false), unit(7, true)]);
        drop(rx);
        assert_eq!(feed.push(unit(9, true)), Fed::Closed);
    }

    #[test]
    fn the_options_map_onto_the_wire_and_the_intent_drives_start_and_stop() {
        let mut options = std::collections::HashMap::new();
        options.insert("resolution".to_owned(), "sharp".to_owned());
        options.insert("rate".to_owned(), "fluid".to_owned());
        options.insert("quality".to_owned(), "thrifty".to_owned());
        let start = start_from_options(&options);
        assert_eq!(
            (start.max_size, start.fps, start.bitrate_kbps),
            (1920, 120, 4000)
        );

        let mirror = OwnMirror::default();
        assert!(mirror
            .tick("phone", &super::super::writer::Outbox::detached(8).0)
            .is_empty());
        mirror.request_start(start);
        let sent = mirror.tick("phone", &super::super::writer::Outbox::detached(8).0);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].kind, MirrorStart::KIND);
        assert_eq!(mirror.state(), LinkState::Starting);
        assert!(
            mirror
                .tick("phone", &super::super::writer::Outbox::detached(8).0)
                .is_empty(),
            "starting is asked once"
        );
        let recorder = testing::Recorder::default();
        mirror.started(
            &recorder,
            &MirrorStarted {
                width: 1080,
                height: 2340,
                codec: Codec::Hevc,
                audio: false,
            },
        );
        assert_eq!(
            mirror.state(),
            LinkState::Streaming {
                width: 1080,
                height: 2340
            }
        );
        mirror.request_stop();
        let sent = mirror.tick("phone", &super::super::writer::Outbox::detached(8).0);
        assert_eq!(sent[0].kind, MirrorStop::KIND);
        assert_eq!(mirror.state(), LinkState::Idle);
    }
}
