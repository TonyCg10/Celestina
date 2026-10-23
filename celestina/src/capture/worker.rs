// Written for the overview (CEL-2-L, T5) in the shape of the energy workers
// (`energy/idle.rs`): a thread behind the `Worker` guard, a seam so the
// request rules are tested without a compositor, and the shared bounded pump
// of `energy/wire.rs` for the connection.

//! The capture worker: one thread that waits for requests and, for each,
//! opens its own Wayland connection, takes one `wlr-screencopy` frame per
//! requested output, shrinks each whole frame to the preview of the
//! workspace active on that output, closes the connection and replies.
//!
//! - [`CaptureHandle::request`] takes one request at a time, process-wide:
//!   while one is in flight every other is refused with [`Busy::InFlight`]
//!   and dropped, never queued, whichever worker would take it. The mark is
//!   [`IN_FLIGHT`] for the live workers, so a worker that was given up and is
//!   still draining its capture keeps the next opening's fresh worker
//!   refused until that capture has really ended (the app then opens with
//!   geometry only). The mark travels with the request as a [`Ticket`] and is
//!   cleared when the ticket drops: after the capture, while a panicking
//!   capturer unwinds, or with a request the queue never delivered. A
//!   request accepted in the instant its worker stops is dropped with its
//!   ticket and gets no reply; the app's own timer bounds that wait.
//! - The frame's storage is an anonymous memory file the worker creates and
//!   sizes itself, after [`super::check_buffer`] has accepted the
//!   compositor's `buffer` event. It is read back with positioned reads, not
//!   mapped, so no `unsafe` is needed and a compositor that truncates the file
//!   produces a short read (an error) rather than a fault.
//! - Every frame, buffer, pool and memory file is released before the reply
//!   is published; the connection closes with the request.
//! - A missing global, a `failed` frame or a deadline is
//!   [`CaptureResult::Unavailable`] (no output captured) with one log line,
//!   or a partial [`CaptureResult::Done`]: a preview already made is kept
//!   even when the connection fails afterwards.
//! - Memory: every frame's storage plus the copy read back from one of them
//!   (frames are read back one at a time) stays within
//!   [`super::MAX_FRAME_BYTES_IN_FLIGHT`], checked before each allocation.
//! - Shutdown is looked at before connecting, during every wait (the sync of
//!   the globals included), before each `copy` request, and before and after
//!   each read-back, so a given-up capture stops at the next step.
//! - Each reply carries its request's opening number ([`CaptureReply`]).
//! - Every wait is bounded by [`POLL`] and the request by
//!   [`super::CAPTURE_TIMEOUT`], so [`Worker::join`] is bounded too.

use std::fs::File;
use std::io;
use std::os::fd::AsFd;
use std::os::unix::fs::FileExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::Instant;

use celestina_shell::thumbnail::{PixelFormat, Thumb};
use rustix::fs::{memfd_create, MemfdFlags};
use wayland_client::protocol::{wl_buffer, wl_output, wl_registry, wl_shm, wl_shm_pool};
use wayland_client::{delegate_noop, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1 as frame_proto, zwlr_screencopy_manager_v1 as manager_proto,
};

use super::{
    check_buffer, fits_budget, frame_matches, half_lumas, mean_luma, preview_output, BufferSpec,
    CaptureReply, CaptureRequest, CaptureResult, OutputRequest, CAPTURE_TIMEOUT,
    MAX_FRAME_BYTES_IN_FLIGHT, MAX_OUTPUTS, MAX_PREVIEWS,
};
use crate::energy::wire::{self, Link, Wire, POLL, SYNC_TIMEOUT};
use crate::providers::Worker;

/// How many protocol events one dispatch may carry before the compositor is
/// treated as hostile.
const MAX_EVENTS: usize = 256;
/// The longest output name kept; a longer one never matches a request.
const MAX_NAME: usize = 128;
/// `zwlr_screencopy_frame_v1.flags`: the frame is upside down.
const FLAG_Y_INVERT: u32 = 1;

/// The process-wide in-flight mark of the live capture workers: at most one
/// capture runs in the shell at any time.
pub static IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Why [`CaptureHandle::request`] refused a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Busy {
    /// A request is already in flight; this one is dropped, not queued.
    #[error("a capture is already in flight")]
    InFlight,
    /// The worker has stopped.
    #[error("the capture worker has stopped")]
    Gone,
}

/// The loop's side of the capture worker.
#[derive(Clone, Debug)]
pub struct CaptureHandle {
    requests: SyncSender<(CaptureRequest, Ticket)>,
    in_flight: &'static AtomicBool,
}

impl CaptureHandle {
    /// Hands `request` to the worker. Its reply arrives later through the
    /// `publish` the worker was started with.
    ///
    /// # Errors
    ///
    /// [`Busy::InFlight`] while a capture sharing this handle's mark has not
    /// ended (on any worker), [`Busy::Gone`] when this worker has stopped.
    pub fn request(&self, request: CaptureRequest) -> Result<(), Busy> {
        if self
            .in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(Busy::InFlight);
        }
        let ticket = Ticket(self.in_flight);
        // A refused request comes back with its ticket, which clears the mark
        // as it drops.
        self.requests
            .try_send((request, ticket))
            .map_err(|error| match error {
                TrySendError::Full(_) => Busy::InFlight,
                TrySendError::Disconnected(_) => Busy::Gone,
            })
    }
}

/// The in-flight mark, owned by the request that set it: clears the mark
/// when dropped, after the capture, while a panicking capturer unwinds, or
/// with a request the queue discards undelivered (a closed queue drops what
/// it holds).
#[derive(Debug)]
pub struct Ticket(&'static AtomicBool);

impl Drop for Ticket {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// What turns a request into a reply: the compositor, or a test's stand-in.
pub trait Capturer: Send + 'static {
    /// Captures `request`, giving up at `deadline` or when `shutdown` is
    /// raised. Logs its own failure (one line) before returning it.
    fn capture(
        &mut self,
        request: &CaptureRequest,
        deadline: Instant,
        shutdown: &AtomicBool,
    ) -> CaptureResult;
}

/// Starts the capture worker over `capturer`, with `in_flight` as its
/// one-at-a-time mark: [`IN_FLIGHT`] for the live workers, so they share it,
/// or a test's own.
///
/// `publish` receives every reply, tagged with its request's
/// [`CaptureRequest::generation`], and returns whether the loop still listens;
/// once it returns `false` the worker stops. The worker also stops when
/// `shutdown` is raised (which joining the returned [`Worker`] does) or every
/// handle is dropped. A reply to a request interrupted by `shutdown` is not
/// published.
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn(
    capturer: impl Capturer,
    publish: impl FnMut(CaptureReply) -> bool + Send + 'static,
    shutdown: &Arc<AtomicBool>,
    in_flight: &'static AtomicBool,
) -> io::Result<(CaptureHandle, Worker)> {
    let (requests, queue) = mpsc::sync_channel(1);
    let worker_shutdown = Arc::clone(shutdown);
    let worker = Worker::spawn("capture", shutdown, move || {
        run(capturer, &queue, publish, &worker_shutdown);
        // The queue closes here, whatever ended the loop: a later request is
        // told `Gone`, and one that slipped in before is discarded with its
        // ticket, which clears the mark. The worker never clears a mark it
        // does not hold.
        drop(queue);
    })?;
    Ok((
        CaptureHandle {
            requests,
            in_flight,
        },
        worker,
    ))
}

fn run(
    mut capturer: impl Capturer,
    requests: &Receiver<(CaptureRequest, Ticket)>,
    mut publish: impl FnMut(CaptureReply) -> bool,
    shutdown: &AtomicBool,
) {
    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        match requests.recv_timeout(POLL) {
            Ok((request, ticket)) => {
                let deadline = Instant::now() + CAPTURE_TIMEOUT;
                let result = capturer.capture(&request, deadline, shutdown);
                let generation = request.generation;
                drop(request);
                // The capture has ended and freed its frames: the mark goes
                // now, before the reply, so the loop may ask again as soon
                // as it has read it, and also when the reply is not sent.
                drop(ticket);
                if shutdown.load(Ordering::Acquire) {
                    return;
                }
                if !publish(CaptureReply { generation, result }) {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

// The Wayland capturer.

/// The session's compositor, over a connection opened for each request.
#[derive(Clone, Copy, Debug, Default)]
pub struct WaylandCapturer;

impl Capturer for WaylandCapturer {
    fn capture(
        &mut self,
        request: &CaptureRequest,
        deadline: Instant,
        shutdown: &AtomicBool,
    ) -> CaptureResult {
        if request.outputs.is_empty() {
            return CaptureResult::Done {
                previews: Vec::new(),
            };
        }
        match capture_session(request, deadline, shutdown) {
            Ok(Outcome { previews, captured }) if captured > 0 => CaptureResult::Done { previews },
            Ok(_) => {
                let reason = Failure::NothingCaptured.to_string();
                tracing::warn!("capture: unavailable: {reason}");
                CaptureResult::Unavailable(reason)
            }
            Err(failure) => {
                let reason = crate::error_chain(&failure);
                tracing::warn!("capture: unavailable: {reason}");
                CaptureResult::Unavailable(reason)
            }
        }
    }
}

/// Why a request captured nothing.
#[derive(Debug, thiserror::Error)]
enum Failure {
    #[error("the compositor offers no wlr-screencopy")]
    NoScreencopy,
    #[error("the compositor offers no wl_shm")]
    NoShm,
    #[error("the compositor withdrew wlr-screencopy or wl_shm")]
    GlobalGone,
    #[error("the shell is stopping")]
    Stopped,
    #[error("no requested output could be captured in time")]
    NothingCaptured,
    #[error(transparent)]
    Wire(#[from] wire::Lost),
}

struct OutputSlot {
    global: u32,
    proxy: wl_output::WlOutput,
    name: Option<String>,
}

struct CaptureState {
    shm: Option<(u32, wl_shm::WlShm)>,
    manager: Option<(u32, manager_proto::ZwlrScreencopyManagerV1)>,
    outputs: Vec<OutputSlot>,
    gone: bool,
}

/// One frame event, addressed by the frame's index in the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Note {
    Buffer {
        frame: usize,
        format: u32,
        width: u32,
        height: u32,
        stride: u32,
    },
    Flags {
        frame: usize,
        flags: u32,
    },
    BufferDone(usize),
    Ready(usize),
    Failed(usize),
}

type CaptureWire = Wire<CaptureState, Note>;

fn intact(state: &CaptureState) -> Result<(), Failure> {
    if state.gone {
        return Err(Failure::GlobalGone);
    }
    Ok(())
}

/// The check every wait of a capture makes: the shell is not stopping and
/// the globals it uses are still there.
fn live(shutdown: &AtomicBool) -> impl Fn(&CaptureState) -> Result<(), Failure> + '_ {
    move |state| {
        if shutdown.load(Ordering::Acquire) {
            return Err(Failure::Stopped);
        }
        intact(state)
    }
}

/// The frame's own storage: a memory file, the pool over it, the buffer.
struct Storage {
    file: File,
    pool: wl_shm_pool::WlShmPool,
    buffer: wl_buffer::WlBuffer,
}

impl Storage {
    fn release(self) {
        self.buffer.destroy();
        self.pool.destroy();
        drop(self.file);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Waiting for the `buffer` (and, from version 3, `buffer_done`) events.
    Announced,
    /// `copy` was sent; waiting for `ready` or `failed`.
    Copying,
    Settled,
}

/// One output's frame in flight.
struct Pending {
    /// Index into the request's outputs.
    output: usize,
    frame: frame_proto::ZwlrScreencopyFrameV1,
    spec: Option<BufferSpec>,
    y_invert: bool,
    storage: Option<Storage>,
    phase: Phase,
}

impl Pending {
    /// Destroys every protocol object and the memory file of this frame.
    fn settle(&mut self) {
        if self.phase == Phase::Settled {
            return;
        }
        // The frame first: the compositor stops copying before the buffer it
        // copies into goes away.
        self.frame.destroy();
        if let Some(storage) = self.storage.take() {
            storage.release();
        }
        self.phase = Phase::Settled;
    }
}

struct Outcome {
    /// Keyed by workspace id.
    previews: Vec<(u64, Thumb)>,
    /// How many outputs produced a frame.
    captured: usize,
}

impl Outcome {
    /// Whether [`MAX_PREVIEWS`] previews are already kept.
    fn is_full(&self) -> bool {
        self.previews.len() >= MAX_PREVIEWS
    }

    /// Keeps `preview` of workspace `id`, unless the reply is full.
    fn keep(&mut self, id: u64, preview: Thumb) {
        if !self.is_full() {
            self.captured += 1;
            self.previews.push((id, preview));
        }
    }
}

fn capture_session(
    request: &CaptureRequest,
    deadline: Instant,
    shutdown: &AtomicBool,
) -> Result<Outcome, Failure> {
    if shutdown.load(Ordering::Acquire) {
        return Err(Failure::Stopped);
    }
    let state = CaptureState {
        shm: None,
        manager: None,
        outputs: Vec::new(),
        gone: false,
    };
    let mut link: Link<CaptureState, Note> = Link::connect(state, MAX_EVENTS)?;
    // The globals, then the names of the outputs bound while dispatching them.
    for _ in 0..2 {
        link.sync(deadline.min(Instant::now() + SYNC_TIMEOUT), live(shutdown))?;
    }
    let (Some((_, manager)), Some((_, shm))) =
        (link.wire.state.manager.clone(), link.wire.state.shm.clone())
    else {
        return Err(if link.wire.state.manager.is_none() {
            Failure::NoScreencopy
        } else {
            Failure::NoShm
        });
    };

    let mut pending = Vec::new();
    for (index, output) in request.outputs.iter().enumerate().take(MAX_OUTPUTS) {
        let bound = link
            .wire
            .state
            .outputs
            .iter()
            .find(|slot| slot.name.as_deref() == Some(output.name.as_str()));
        let Some(slot) = bound else {
            tracing::warn!(
                "capture: output {}: not offered by the compositor",
                output.name
            );
            continue;
        };
        let frame = pending.len();
        // `overlay_cursor = 0`: the pointer is never in the picture.
        let proxy = manager.capture_output(0, &slot.proxy, &link.qh, frame);
        pending.push(Pending {
            output: index,
            frame: proxy,
            spec: None,
            y_invert: false,
            storage: None,
            phase: Phase::Announced,
        });
    }

    let mut outcome = Outcome {
        previews: Vec::new(),
        captured: 0,
    };
    let served = serve(
        &mut link,
        &shm,
        request,
        &mut pending,
        &mut outcome,
        deadline,
        shutdown,
    );
    // Every frame is freed before the reply, whatever ended the wait.
    for frame in &mut pending {
        frame.settle();
    }
    manager.destroy();
    let _ = link.flush();
    drop(link);
    settle(served, outcome, shutdown.load(Ordering::Acquire))
}

/// What a request comes to once its frames are freed: a stop wins over
/// everything; a failure after a preview was already made keeps that preview
/// (a partial [`CaptureResult::Done`]); any other failure is the failure.
fn settle(
    served: Result<(), Failure>,
    outcome: Outcome,
    stopped: bool,
) -> Result<Outcome, Failure> {
    if stopped {
        return Err(Failure::Stopped);
    }
    match served {
        Ok(()) => Ok(outcome),
        Err(failure) if outcome.captured > 0 => {
            tracing::warn!(
                "capture: ended early ({}); keeping {} preview(s)",
                crate::error_chain(&failure),
                outcome.captured
            );
            Ok(outcome)
        }
        Err(failure) => Err(failure),
    }
}

/// Pumps the connection and answers frame events until every frame settled,
/// the deadline passed or the shell stops.
fn serve(
    link: &mut Link<CaptureState, Note>,
    shm: &wl_shm::WlShm,
    request: &CaptureRequest,
    pending: &mut [Pending],
    outcome: &mut Outcome,
    deadline: Instant,
    shutdown: &AtomicBool,
) -> Result<(), Failure> {
    while pending.iter().any(|frame| frame.phase != Phase::Settled) {
        if shutdown.load(Ordering::Acquire) {
            return Err(Failure::Stopped);
        }
        let Some(remaining) = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
        else {
            let late = pending
                .iter()
                .filter(|frame| frame.phase != Phase::Settled)
                .count();
            tracing::warn!(
                "capture: {late} output(s) did not answer within {} ms",
                CAPTURE_TIMEOUT.as_millis()
            );
            return Ok(());
        };
        link.pump(remaining.min(POLL), live(shutdown))?;
        for note in link.wire.take_events() {
            handle(note, link, shm, request, pending, outcome, shutdown);
        }
    }
    Ok(())
}

fn handle(
    note: Note,
    link: &mut Link<CaptureState, Note>,
    shm: &wl_shm::WlShm,
    request: &CaptureRequest,
    pending: &mut [Pending],
    outcome: &mut Outcome,
    shutdown: &AtomicBool,
) {
    let index = match note {
        Note::Buffer { frame, .. } | Note::Flags { frame, .. } => frame,
        Note::BufferDone(frame) | Note::Ready(frame) | Note::Failed(frame) => frame,
    };
    // The sizes of the other frames' storage held right now.
    let held: Vec<u64> = pending
        .iter()
        .enumerate()
        .filter(|(other, frame)| *other != index && frame.storage.is_some())
        .filter_map(|(_, frame)| frame.spec.map(|spec| spec.bytes()))
        .collect();
    let Some(frame) = pending.get_mut(index) else {
        return;
    };
    if frame.phase == Phase::Settled {
        return;
    }
    let Some(output) = request.outputs.get(frame.output) else {
        frame.settle();
        return;
    };
    let name = output.name.as_str();
    match note {
        Note::Buffer {
            format,
            width,
            height,
            stride,
            ..
        } => {
            if frame.phase != Phase::Announced || frame.spec.is_some() {
                tracing::warn!("capture: output {name}: a second shm buffer was announced");
                frame.settle();
                return;
            }
            match check_buffer(format, width, height, stride) {
                Ok(spec) => {
                    frame.spec = Some(spec);
                    // Before version 3 there is no `buffer_done`: the one
                    // shm buffer is the whole announcement.
                    if frame.frame.version() < 3 {
                        start_copy(frame, &held, shm, &link.qh, output, shutdown);
                    }
                }
                Err(refusal) => {
                    tracing::warn!("capture: output {name}: buffer refused: {refusal}");
                    frame.settle();
                }
            }
        }
        Note::Flags { flags, .. } => frame.y_invert = flags & FLAG_Y_INVERT != 0,
        Note::BufferDone(_) => {
            if frame.phase != Phase::Announced {
                return;
            }
            if frame.spec.is_none() {
                tracing::warn!("capture: output {name}: no shm buffer was offered");
                frame.settle();
                return;
            }
            start_copy(frame, &held, shm, &link.qh, output, shutdown);
        }
        Note::Ready(_) => {
            if frame.phase != Phase::Copying {
                frame.settle();
                return;
            }
            if outcome.is_full() || shutdown.load(Ordering::Acquire) {
                frame.settle();
                return;
            }
            let stopping = || io::Error::other("the shell is stopping");
            let cropped = match (&frame.spec, &frame.storage) {
                (Some(spec), Some(storage)) => {
                    preview_output(spec, frame.y_invert, output, |offset, len| {
                        let mut bytes = vec![0_u8; len];
                        storage.file.read_exact_at(&mut bytes, offset)?;
                        // Between the read and the scale: a capture given up
                        // meanwhile drops the copy instead of scaling it.
                        if shutdown.load(Ordering::Acquire) {
                            return Err(stopping());
                        }
                        Ok(bytes)
                    })
                }
                _ => {
                    frame.settle();
                    return;
                }
            };
            // The memory file goes before anything is reported.
            frame.settle();
            match cropped {
                Ok(preview) => {
                    let id = output.workspace_id;
                    tracing::info!("capture: output {name}: 1 preview(s)");
                    tracing::debug!(
                        "capture: workspace {id}: {}x{} mean luma {}",
                        preview.width,
                        preview.height,
                        mean_luma(&preview)
                    );
                    let (left, right) = half_lumas(&preview);
                    tracing::debug!("capture: workspace {id}: left luma {left} right luma {right}");
                    outcome.keep(id, preview);
                }
                Err(error) => {
                    tracing::warn!("capture: output {name}: {}", crate::error_chain(&error));
                }
            }
        }
        Note::Failed(_) => {
            tracing::warn!("capture: output {name}: the compositor failed the frame");
            frame.settle();
        }
    }
}

/// Creates the frame's storage at the size its accepted spec says and asks
/// the compositor to copy into it. A frame whose size is not the one the
/// output's geometry predicts, or that would take the request's peak memory
/// (the storage of every frame, `held` for the others plus its own, and the
/// copy of the largest while it is read back) past
/// [`MAX_FRAME_BYTES_IN_FLIGHT`], is refused here, before anything is
/// allocated, so the memory a capture takes is bounded by the outputs it
/// asked for and by that budget, not by what the compositor announces.
/// Nothing is allocated or asked once the shell is stopping.
fn start_copy(
    frame: &mut Pending,
    held: &[u64],
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<CaptureWire>,
    output: &OutputRequest,
    shutdown: &AtomicBool,
) {
    if shutdown.load(Ordering::Acquire) {
        frame.settle();
        return;
    }
    let name = output.name.as_str();
    let Some(spec) = frame.spec else {
        frame.settle();
        return;
    };
    if !frame_matches(output, spec.width, spec.height) {
        tracing::warn!(
            "capture: output {name}: a {}x{} frame does not match the output's size and scale",
            spec.width,
            spec.height
        );
        frame.settle();
        return;
    }
    if !fits_budget(held, spec.bytes()) {
        tracing::warn!(
            "capture: output {name}: a {}x{} frame would pass the {} MiB budget of a capture",
            spec.width,
            spec.height,
            MAX_FRAME_BYTES_IN_FLIGHT / (1024 * 1024)
        );
        frame.settle();
        return;
    }
    match allocate(&spec, shm, qh) {
        Ok(storage) if shutdown.load(Ordering::Acquire) => {
            storage.release();
            frame.settle();
        }
        Ok(storage) => {
            frame.frame.copy(&storage.buffer);
            frame.storage = Some(storage);
            frame.phase = Phase::Copying;
        }
        Err(error) => {
            tracing::warn!("capture: output {name}: cannot allocate the frame: {error}");
            frame.settle();
        }
    }
}

fn allocate(
    spec: &BufferSpec,
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<CaptureWire>,
) -> io::Result<Storage> {
    // `check_buffer` bounds every one of these to the protocol's `int`.
    let too_large = || io::Error::other("the frame does not fit the protocol");
    let size = i32::try_from(spec.bytes()).map_err(|_| too_large())?;
    let width = i32::try_from(spec.width).map_err(|_| too_large())?;
    let height = i32::try_from(spec.height).map_err(|_| too_large())?;
    let stride = i32::try_from(spec.stride).map_err(|_| too_large())?;
    let format = match spec.format {
        PixelFormat::Argb8888 => wl_shm::Format::Argb8888,
        PixelFormat::Xrgb8888 => wl_shm::Format::Xrgb8888,
    };
    let fd = memfd_create("celestina-capture", MemfdFlags::CLOEXEC).map_err(io::Error::from)?;
    let file = File::from(fd);
    file.set_len(spec.bytes())?;
    let pool = shm.create_pool(file.as_fd(), size, qh, ());
    let buffer = pool.create_buffer(0, width, height, stride, format, qh, ());
    Ok(Storage { file, pool, buffer })
}

impl Dispatch<wl_registry::WlRegistry, ()> for CaptureWire {
    fn event(
        wire: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let state = &mut wire.state;
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                if interface == wl_shm::WlShm::interface().name && state.shm.is_none() {
                    let shm = registry.bind::<wl_shm::WlShm, _, _>(name, 1, qh, ());
                    state.shm = Some((name, shm));
                } else if interface == manager_proto::ZwlrScreencopyManagerV1::interface().name
                    && state.manager.is_none()
                {
                    let manager = registry.bind::<manager_proto::ZwlrScreencopyManagerV1, _, _>(
                        name,
                        version.min(3),
                        qh,
                        (),
                    );
                    state.manager = Some((name, manager));
                } else if interface == wl_output::WlOutput::interface().name
                    && version >= 4
                    && state.outputs.len() < MAX_OUTPUTS
                {
                    // Version 4 is the first with `name`, the only way to
                    // match an output to a request.
                    let index = state.outputs.len();
                    let proxy = registry.bind::<wl_output::WlOutput, _, _>(name, 4, qh, index);
                    state.outputs.push(OutputSlot {
                        global: name,
                        proxy,
                        name: None,
                    });
                }
            }
            wl_registry::Event::GlobalRemove { name } => {
                let ours = |global: Option<u32>| global == Some(name);
                if ours(state.shm.as_ref().map(|(global, _)| *global))
                    || ours(state.manager.as_ref().map(|(global, _)| *global))
                {
                    state.gone = true;
                }
                // A removed output stops matching; its frame, if any, is
                // failed by the compositor.
                if let Some(slot) = state.outputs.iter_mut().find(|slot| slot.global == name) {
                    slot.name = None;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, usize> for CaptureWire {
    fn event(
        wire: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        index: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            if let Some(slot) = wire.state.outputs.get_mut(*index) {
                slot.name = (name.len() <= MAX_NAME).then_some(name);
            }
        }
    }
}

impl Dispatch<frame_proto::ZwlrScreencopyFrameV1, usize> for CaptureWire {
    fn event(
        wire: &mut Self,
        _: &frame_proto::ZwlrScreencopyFrameV1,
        event: frame_proto::Event,
        frame: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let frame = *frame;
        let note = match event {
            frame_proto::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => Note::Buffer {
                frame,
                format: u32::from(format),
                width,
                height,
                stride,
            },
            frame_proto::Event::Flags { flags } => Note::Flags {
                frame,
                flags: u32::from(flags),
            },
            frame_proto::Event::BufferDone => Note::BufferDone(frame),
            frame_proto::Event::Ready { .. } => Note::Ready(frame),
            frame_proto::Event::Failed => Note::Failed(frame),
            // Damage and dmabuf offers are not used: one shm copy, no refresh.
            _ => return,
        };
        wire.push(note);
    }
}

delegate_noop!(CaptureWire: ignore wl_shm::WlShm);
delegate_noop!(CaptureWire: wl_shm_pool::WlShmPool);
delegate_noop!(CaptureWire: ignore wl_buffer::WlBuffer);
delegate_noop!(CaptureWire: manager_proto::ZwlrScreencopyManagerV1);

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_shell::thumbnail::Thumb;
    use std::time::Duration;

    fn outcome(captured: usize) -> Outcome {
        Outcome {
            previews: (0..captured)
                .map(|id| {
                    (
                        id as u64,
                        Thumb {
                            width: 1,
                            height: 1,
                            rgba: vec![0, 0, 0, 255],
                        },
                    )
                })
                .collect(),
            captured,
        }
    }

    #[test]
    fn a_failure_after_a_preview_keeps_the_preview() {
        let kept = settle(Err(Failure::GlobalGone), outcome(1), false);
        assert!(kept.is_ok_and(|o| o.previews.len() == 1 && o.captured == 1));
        assert!(matches!(
            settle(Err(Failure::GlobalGone), outcome(0), false),
            Err(Failure::GlobalGone)
        ));
        assert!(matches!(
            settle(Ok(()), outcome(1), true),
            Err(Failure::Stopped)
        ));
        assert!(settle(Ok(()), outcome(0), false).is_ok());
    }

    /// Panics on its first request.
    struct Panicking;

    impl Capturer for Panicking {
        fn capture(&mut self, _: &CaptureRequest, _: Instant, _: &AtomicBool) -> CaptureResult {
            panic!("a capturer that panics, on purpose");
        }
    }

    #[test]
    fn a_panicking_capturer_never_leaves_the_request_stuck_in_flight() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let mark = crate::capture::tests::own_mark();
        let (handle, worker) =
            spawn(Panicking, |_| true, &shutdown, mark).expect("the worker starts");
        assert_eq!(handle.request(CaptureRequest::default()), Ok(()));
        let started = Instant::now();
        let refusal = loop {
            match handle.request(CaptureRequest::default()) {
                Err(Busy::InFlight) if started.elapsed() < Duration::from_secs(2) => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                other => break other,
            }
        };
        assert_eq!(refusal, Err(Busy::Gone));
        assert!(!mark.load(Ordering::Acquire), "the mark is not left set");
        worker.join();
    }

    #[test]
    fn a_closed_queue_clears_the_mark_of_a_request_it_never_delivered() {
        let mark = crate::capture::tests::own_mark();
        let (requests, queue) = mpsc::sync_channel(1);
        let handle = CaptureHandle {
            requests,
            in_flight: mark,
        };
        assert_eq!(handle.request(CaptureRequest::default()), Ok(()));
        assert!(mark.load(Ordering::Acquire));
        assert_eq!(
            handle.request(CaptureRequest::default()),
            Err(Busy::InFlight)
        );
        // The worker stops without taking it: the queue drops its ticket.
        drop(queue);
        assert!(!mark.load(Ordering::Acquire));
        assert_eq!(handle.request(CaptureRequest::default()), Err(Busy::Gone));
        assert!(!mark.load(Ordering::Acquire), "a refused request frees it");
    }

    #[test]
    fn a_reply_holds_at_most_the_preview_bound() {
        let thumb = || Thumb {
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
        };
        let mut outcome = outcome(0);
        for id in 0..(MAX_PREVIEWS as u64 + 3) {
            outcome.keep(id, thumb());
        }
        assert_eq!(outcome.previews.len(), MAX_PREVIEWS);
        assert_eq!(outcome.captured, MAX_PREVIEWS);
        assert!(outcome.is_full());
    }
}
