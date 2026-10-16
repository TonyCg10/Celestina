//! Clipboard paste through `wl_data_device`/`wl_data_offer`: the current
//! selection's `text/plain;charset=utf-8` is read on a worker thread (never
//! on the loop), bounded to [`MAX_PASTE_BYTES`] and a deadline, checked as
//! UTF-8, and delivered as `InputEvent::Paste` to the surface that asked.
//! Without `wl_data_device_manager` paste is off. Drag and drop offers are
//! never accepted.
#![forbid(unsafe_code)]

use std::{
    fs::File,
    io::{self, Read},
    os::fd::{AsFd, OwnedFd},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use calloop::channel::Sender as ChannelSender;
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use smithay_client_toolkit::{
    data_device_manager::{
        data_device::{DataDevice, DataDeviceHandler},
        data_offer::{DataOfferHandler, DragOffer},
        DataDeviceManagerState,
    },
    globals::GlobalData,
};
use wayland_client::{
    globals::GlobalList,
    protocol::{
        wl_data_device::WlDataDevice, wl_data_device_manager::DndAction, wl_seat::WlSeat,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, QueueHandle,
};

use crate::{
    input::InputEvent,
    runtime::{Runtime, Shell, SurfaceId},
};

/// The only clipboard type read.
pub(crate) const TEXT_MIME: &str = "text/plain;charset=utf-8";
/// The most clipboard bytes read; the rest is left unread.
pub(crate) const MAX_PASTE_BYTES: usize = 64 * 1024;
/// How long a clipboard owner gets to write its text.
const PASTE_DEADLINE: Duration = Duration::from_secs(2);
/// How often the reader looks at its stop flag while waiting.
const POLL_SLICE: Duration = Duration::from_millis(50);

/// What the paste worker hands back to the loop.
#[derive(Debug)]
pub(crate) struct PasteDone {
    surface: SurfaceId,
    text: Option<String>,
}

/// Why a clipboard read produced no text.
#[derive(Debug, thiserror::Error)]
pub(crate) enum PasteError {
    #[error("the clipboard owner did not finish writing in time")]
    TimedOut,
    #[error("the read was cancelled")]
    Cancelled,
    #[error("cannot read the clipboard pipe")]
    Io(#[source] io::Error),
}

/// Reads `pipe` to its end, at most `max` bytes, until `deadline`, looking
/// at `stop` at least every [`POLL_SLICE`]. Returns the bytes and whether
/// more remained unread.
pub(crate) fn read_bounded<R: Read + AsFd>(
    pipe: &mut R,
    max: usize,
    deadline: Instant,
    stop: &AtomicBool,
) -> Result<(Vec<u8>, bool), PasteError> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        if stop.load(Ordering::Relaxed) {
            return Err(PasteError::Cancelled);
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(PasteError::TimedOut);
        }
        let wait = Timespec::try_from(left.min(POLL_SLICE))
            .map_err(|_| PasteError::Io(io::Error::from(io::ErrorKind::InvalidInput)))?;
        let mut fds = [PollFd::new(pipe, PollFlags::IN)];
        match poll(&mut fds, Some(&wait)) {
            Ok(0) => continue,
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(err) => return Err(PasteError::Io(err.into())),
        }
        let room = (max + 1 - bytes.len()).min(chunk.len());
        let n = match pipe.read(&mut chunk[..room]) {
            Ok(n) => n,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(PasteError::Io(err)),
        };
        if n == 0 {
            return Ok((bytes, false));
        }
        bytes.extend_from_slice(&chunk[..n]);
        if bytes.len() > max {
            bytes.truncate(max);
            return Ok((bytes, true));
        }
    }
}

/// The pasted text: valid UTF-8 (a character cut by the size bound is
/// dropped), not empty. Anything else is refused.
pub(crate) fn decode_paste(mut bytes: Vec<u8>, truncated: bool) -> Option<String> {
    if let Err(err) = std::str::from_utf8(&bytes) {
        // Only a character the bound cut in half may be dropped.
        if !(truncated && err.error_len().is_none()) {
            return None;
        }
        bytes.truncate(err.valid_up_to());
    }
    String::from_utf8(bytes).ok().filter(|s| !s.is_empty())
}

/// The one paste read in flight.
struct PasteWorker {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

impl PasteWorker {
    fn join(self) {
        self.stop.store(true, Ordering::Relaxed);
        if self.handle.join().is_err() {
            tracing::error!("the paste worker panicked");
        }
    }
}

/// The seat's clipboard: the manager (if the compositor has one), the data
/// device of the seat and the paste in flight.
pub(crate) struct Clipboard {
    manager: Option<DataDeviceManagerState>,
    device: Option<DataDevice>,
    worker: Option<PasteWorker>,
    done: ChannelSender<PasteDone>,
}

impl Clipboard {
    /// Binds the manager; without it paste is off. Results come back
    /// through `done`.
    pub(crate) fn bind<D>(
        globals: &GlobalList,
        qh: &QueueHandle<D>,
        done: ChannelSender<PasteDone>,
    ) -> Self
    where
        D: Dispatch<
                wayland_client::protocol::wl_data_device_manager::WlDataDeviceManager,
                GlobalData,
            > + 'static,
    {
        let manager = DataDeviceManagerState::bind(globals, qh)
            .inspect_err(|err| tracing::info!(%err, "no wl_data_device_manager: paste is off"))
            .ok();
        Self {
            manager,
            device: None,
            worker: None,
            done,
        }
    }

    /// The seat gained a keyboard (the selection follows keyboard focus).
    pub(crate) fn seat_added<S: Shell>(&mut self, seat: &WlSeat, qh: &QueueHandle<Runtime<S>>) {
        if let (Some(manager), None) = (&self.manager, &self.device) {
            self.device = Some(manager.get_data_device(qh, seat));
        }
    }

    /// The keyboard went away; the device is released.
    pub(crate) fn seat_removed(&mut self) {
        self.device = None;
    }

    /// Starts reading the selection's text for `surface`. Nothing happens
    /// without a device, a text selection, or while a read is in flight.
    pub(crate) fn request(&mut self, surface: SurfaceId, conn: &Connection) {
        if self.worker.is_some() {
            tracing::debug!(
                surface = surface.0,
                "paste: a read is in flight, request dropped"
            );
            return;
        }
        let Some(offer) = self
            .device
            .as_ref()
            .and_then(|d| d.data().selection_offer())
        else {
            tracing::debug!(surface = surface.0, "paste: the clipboard is empty");
            return;
        };
        if !offer.with_mime_types(|types| types.iter().any(|t| t == TEXT_MIME)) {
            tracing::debug!(surface = surface.0, "paste: no UTF-8 text on the clipboard");
            return;
        }
        let pipe = match offer.receive(TEXT_MIME.to_owned()) {
            Ok(pipe) => pipe,
            Err(err) => {
                tracing::warn!(%err, "paste: cannot receive the selection");
                return;
            }
        };
        // The owner only starts writing once the compositor forwards the
        // receive request.
        if let Err(err) = conn.flush() {
            tracing::warn!(%err, "paste: cannot flush the receive request");
        }
        let mut file = File::from(OwnedFd::from(pipe));
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let done = self.done.clone();
        let spawned = thread::Builder::new()
            .name("celestina-paste".into())
            .spawn(move || {
                let deadline = Instant::now() + PASTE_DEADLINE;
                let text = match read_bounded(&mut file, MAX_PASTE_BYTES, deadline, &flag) {
                    Ok((bytes, truncated)) => {
                        let text = decode_paste(bytes, truncated);
                        if text.is_none() {
                            tracing::info!("paste: the clipboard text is empty or not UTF-8");
                        }
                        text
                    }
                    Err(err) => {
                        tracing::info!(%err, "paste: no text read");
                        None
                    }
                };
                if done.send(PasteDone { surface, text }).is_err() {
                    tracing::debug!("paste: the loop is gone, text dropped");
                }
            });
        match spawned {
            Ok(handle) => self.worker = Some(PasteWorker { stop, handle }),
            Err(err) => tracing::warn!(%err, "paste: cannot start the reader"),
        }
    }

    /// The worker reported: it is joined (it returns right after sending).
    fn finished(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.join();
        }
    }

    /// Stops and joins a read in flight (bounded by the poll slice) and
    /// releases the device at exit.
    pub(crate) fn shutdown(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.join();
        }
        self.device = None;
    }
}

impl<S: Shell> Runtime<S> {
    /// A paste read finished: its text goes to the surface that asked (if
    /// it is still there).
    pub(crate) fn paste_done(&mut self, done: PasteDone) {
        self.core.clipboard.finished();
        if let Some(text) = done.text {
            self.core
                .pending_events
                .push((done.surface, InputEvent::Paste(text)));
        }
        self.flush();
    }
}

/// Only the selection is used; drags are ignored and so refused.
impl<S: Shell> DataDeviceHandler for Runtime<S> {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataDevice,
        _: f64,
        _: f64,
        _: &WlSurface,
    ) {
    }

    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}

    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _: f64, _: f64) {}

    /// The offer is kept by the device's data and read on request.
    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}

    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
}

impl<S: Shell> DataOfferHandler for Runtime<S> {
    fn source_actions(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: DndAction,
    ) {
    }

    fn selected_action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: DndAction,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn later(ms: u64) -> Instant {
        Instant::now() + Duration::from_millis(ms)
    }

    #[test]
    fn a_closed_pipe_reads_to_its_end() {
        let (mut reader, mut writer) = io::pipe().expect("pipe");
        writer.write_all("hola \u{e9}".as_bytes()).expect("write");
        drop(writer);
        let stop = AtomicBool::new(false);
        let (bytes, truncated) =
            read_bounded(&mut reader, MAX_PASTE_BYTES, later(1000), &stop).expect("read");
        assert!(!truncated);
        assert_eq!(
            decode_paste(bytes, truncated).as_deref(),
            Some("hola \u{e9}")
        );
    }

    #[test]
    fn the_read_stops_at_the_bound() {
        let (mut reader, mut writer) = io::pipe().expect("pipe");
        // More than the bound, from a writer that never closes.
        let feeder = thread::spawn(move || {
            let block = [b'a'; 4096];
            for _ in 0..20 {
                if writer.write_all(&block).is_err() {
                    return;
                }
            }
        });
        let stop = AtomicBool::new(false);
        let (bytes, truncated) =
            read_bounded(&mut reader, 10_000, later(1000), &stop).expect("read");
        assert!(truncated);
        assert_eq!(bytes.len(), 10_000);
        drop(reader);
        feeder.join().expect("feeder");
    }

    #[test]
    fn a_silent_owner_times_out_and_a_stop_cancels() {
        let (mut reader, writer) = io::pipe().expect("pipe");
        let stop = AtomicBool::new(false);
        let started = Instant::now();
        assert!(matches!(
            read_bounded(&mut reader, MAX_PASTE_BYTES, later(120), &stop),
            Err(PasteError::TimedOut)
        ));
        assert!(started.elapsed() < Duration::from_millis(1000));
        stop.store(true, Ordering::Relaxed);
        assert!(matches!(
            read_bounded(&mut reader, MAX_PASTE_BYTES, later(5000), &stop),
            Err(PasteError::Cancelled)
        ));
        drop(writer);
    }

    #[test]
    fn invalid_utf8_is_refused_and_a_cut_character_dropped() {
        assert_eq!(decode_paste(b"ab\xffcd".to_vec(), false), None);
        assert_eq!(decode_paste(b"ab\xffcd".to_vec(), true), None);
        // "a" + the first two bytes of a three-byte character.
        let cut = vec![b'a', 0xe4, 0xbd];
        assert_eq!(decode_paste(cut.clone(), true).as_deref(), Some("a"));
        // Without the bound, a dangling half is a malformed text.
        assert_eq!(decode_paste(cut, false), None);
        assert_eq!(decode_paste(Vec::new(), false), None);
    }
}
