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
use std::process::{Child, Stdio};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::Mutex;

use magnetita_link::RecvStream;
use magnetita_proto::capability;
use magnetita_proto::mirror::{Codec, MirrorStart, MirrorStarted, MirrorStop};
use magnetita_proto::Envelope;

use crate::lock::LockOk;
use crate::runtime::log;

/// The transfer id the phone's video stream carries; audio is the next.
pub(crate) const VIDEO_STREAM: u32 = 0xFFFF_0001;
pub(crate) const AUDIO_STREAM: u32 = 0xFFFF_0002;

/// How many chunks may wait for the decoder before the newest are dropped:
/// a slow window must never stall the QUIC reader.
const QUEUE_CHUNKS: usize = 256;

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

/// `mpv` on stdin, reading the raw HEVC or H.264 with the least delay its
/// demuxer allows; its window is the mirror and its log carries the
/// pointer, wheel and keys the window's script reports, which a thread of
/// this side turns into the wire's touches and keys.
pub(crate) struct DesktopPlayer {
    pub(crate) display_env: Vec<(String, String)>,
}

struct Children {
    mpv: Child,
    tx: SyncSender<Vec<u8>>,
    script: std::path::PathBuf,
}

impl MirrorPlayer for DesktopPlayer {
    fn open(&self, started: &MirrorStarted) -> Option<Box<dyn VideoSink>> {
        if self.display_env.is_empty() {
            log(
                "mirror",
                "no display variables: the link mirror has no window",
            );
            return None;
        }
        let format = match started.codec {
            Codec::Hevc => "hevc",
            Codec::H264 => "h264",
        };
        let script = std::env::var_os("XDG_RUNTIME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("magnetita")
            .join("touch.lua");
        if let Some(dir) = script.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&script, super::mirror_window::script()) {
            log("mirror", &format!("window script: {e}"));
            return None;
        }
        let mut mpv = std::process::Command::new("mpv");
        mpv.args([
            "--profile=low-latency",
            "--untimed",
            "--no-cache",
            "--demuxer=lavf",
            &format!("--demuxer-lavf-format={format}"),
            "--demuxer-lavf-o=fflags=+nobuffer,flags=+low_delay",
            "--demuxer-lavf-analyzeduration=1",
            "--demuxer-readahead-secs=0",
            "--demuxer-thread=no",
            "--vd-lavc-threads=1",
            "--vd-lavc-o=flags=+low_delay",
            "--video-latency-hacks=yes",
            "--framedrop=vo",
            "--wayland-disable-vsync=yes",
            "--hwdec=auto-safe",
            "--container-fps-override=60",
            "--input-default-bindings=no",
            // The left button is the phone's finger, not the window's handle.
            "--window-dragging=no",
            "--osc=no",
            "--osd-level=0",
            "--cursor-autohide=no",
            "--terminal=yes",
            "--no-input-terminal",
            "--msg-level=all=no,touch=info",
            "--title=Magnetita",
            "--wayland-app-id=org.celestina.Magnetita",
        ])
        .arg(format!("--script={}", script.display()))
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
        for (key, value) in &self.display_env {
            mpv.env(key, value);
        }
        let mut mpv = match mpv.spawn() {
            Ok(child) => child,
            Err(e) => {
                log("mirror", &format!("mpv: {e}"));
                return None;
            }
        };
        let mut stdin = mpv.stdin.take()?;
        let reports = mpv.stdout.take()?;
        let translator = super::mirror_window::Translator::new(started.width, started.height);
        std::thread::Builder::new()
            .name("magnetita-mirror-input".into())
            .spawn(move || {
                super::mirror_window::pump(reports, translator, |env| own().queue_input(env));
            })
            .ok()?;
        let (tx, rx) = sync_channel::<Vec<u8>>(QUEUE_CHUNKS);
        std::thread::Builder::new()
            .name("magnetita-mirror-feed".into())
            .spawn(move || {
                while let Ok(chunk) = rx.recv() {
                    if stdin.write_all(&chunk).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Box::new(Children { mpv, tx, script }))
    }
}

impl VideoSink for Children {
    fn write(&mut self, bytes: &[u8]) {
        match self.tx.try_send(bytes.to_vec()) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => {}
        }
    }

    fn close(&mut self) {
        let _ = self.mpv.kill();
        let _ = self.mpv.wait();
        let _ = std::fs::remove_file(&self.script);
    }
}

impl Drop for Children {
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
    /// The owning session's outbox: input goes straight to the link, not
    /// through the tick.
    outbox: Mutex<Option<tokio::sync::mpsc::UnboundedSender<Envelope>>>,
    state: Mutex<Option<LinkState>>,
    sink: Mutex<Option<Box<dyn VideoSink>>>,
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
        let env = match self.outbox.lock_ok().as_ref() {
            Some(outbox) => match outbox.send(env) {
                Ok(()) => return,
                Err(tokio::sync::mpsc::error::SendError(env)) => env,
            },
            None => env,
        };
        let mut input = self.input.lock_ok();
        if input.len() < 1024 {
            input.push_back(env);
        }
    }

    /// The session's tick: what to send, if anything, given the intent.
    /// The owning session leaves its outbox so input skips the tick.
    pub(crate) fn tick(
        &self,
        device_id: &str,
        outbox: &tokio::sync::mpsc::UnboundedSender<Envelope>,
    ) -> Vec<Envelope> {
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
        *self.state.lock_ok() = Some(LinkState::Idle);
        *self.wanted.lock_ok() = None;
        *self.outbox.lock_ok() = None;
        *self.owner.lock_ok() = None;
    }

    fn close_window(&self) {
        if let Some(mut sink) = self.sink.lock_ok().take() {
            sink.close();
        }
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
            .tick("phone", &tokio::sync::mpsc::unbounded_channel().0)
            .is_empty());
        mirror.request_start(start);
        let sent = mirror.tick("phone", &tokio::sync::mpsc::unbounded_channel().0);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].kind, MirrorStart::KIND);
        assert_eq!(mirror.state(), LinkState::Starting);
        assert!(
            mirror
                .tick("phone", &tokio::sync::mpsc::unbounded_channel().0)
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
        let sent = mirror.tick("phone", &tokio::sync::mpsc::unbounded_channel().0);
        assert_eq!(sent[0].kind, MirrorStop::KIND);
        assert_eq!(mirror.state(), LinkState::Idle);
    }
}
