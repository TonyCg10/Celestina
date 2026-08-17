//! One owned `wlr-gamma-control` client for the session's night light.
//!
//! `wlsunset` applied and released its table in one compositor commit, which
//! made the complete output jump between neutral and 2700 K. Celestina now
//! owns the narrow protocol conversation directly: one worker owns every
//! Wayland proxy and file descriptor, one controller exists per output only
//! while the effect is active, and commands cross into that thread through a
//! bounded request/reply channel. Pure whitepoint, timing and LUT policy lives
//! in `celestina-shell-core`; this module only transports it.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::os::fd::AsFd;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell_core::nightlight::{self, TransitionFrame, Whitepoint};
use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::session::{self, SessionRequest, Switch};
use celestina_shell_core::snapshot::{Payload, ProviderId};
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use rustix::fs::{memfd_create, MemfdFlags};
use serde_json::Value;
use wayland_client::protocol::{wl_callback, wl_output, wl_registry};
use wayland_client::{delegate_noop, Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols_wlr::gamma_control::v1::client::{
    zwlr_gamma_control_manager_v1 as manager_proto, zwlr_gamma_control_v1 as gamma_proto,
};

use super::tools::lock_runtime;
use super::worker::Worker;

pub const NAME: &str = "night-light";

const REQUEST_CAPACITY: usize = 4;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const RECOVERY_TIMEOUT: Duration = Duration::from_secs(1);
const WAYLAND_POLL_INTERVAL: Duration = Duration::from_millis(25);

const REQUEST_PENDING: u8 = 0;
const REQUEST_COMMITTED: u8 = 1;
const REQUEST_CANCELLED: u8 = 2;

type Reply = SyncSender<Result<(), String>>;

struct Request {
    switch: Switch,
    permit: Arc<RequestPermit>,
    reply: Reply,
}

struct RequestPermit {
    deadline: Instant,
    state: AtomicU8,
}

impl RequestPermit {
    fn new(deadline: Instant) -> Self {
        Self {
            deadline,
            state: AtomicU8::new(REQUEST_PENDING),
        }
    }

    fn ensure_pending(&self) -> Result<(), String> {
        if self.state.load(Ordering::Acquire) == REQUEST_CANCELLED {
            return Err("night-light request was cancelled".to_owned());
        }
        if Instant::now() >= self.deadline {
            let _ = self.state.compare_exchange(
                REQUEST_PENDING,
                REQUEST_CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            return Err("night-light request expired".to_owned());
        }
        if self.state.load(Ordering::Acquire) != REQUEST_PENDING {
            return Err("night-light request is no longer pending".to_owned());
        }
        Ok(())
    }

    fn try_commit(&self) -> bool {
        if Instant::now() >= self.deadline {
            let _ = self.cancel();
            return false;
        }
        self.state
            .compare_exchange(
                REQUEST_PENDING,
                REQUEST_COMMITTED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    fn cancel(&self) -> bool {
        !matches!(
            self.state.compare_exchange(
                REQUEST_PENDING,
                REQUEST_CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            ),
            Err(REQUEST_COMMITTED)
        )
    }

    fn committed(&self) -> bool {
        self.state.load(Ordering::Acquire) == REQUEST_COMMITTED
    }
}

#[derive(Clone, Copy)]
struct OperationBudget<'a> {
    deadline: Instant,
    permit: Option<&'a RequestPermit>,
    shutdown: Option<&'a AtomicBool>,
}

impl<'a> OperationBudget<'a> {
    fn internal(timeout: Duration, shutdown: Option<&'a AtomicBool>) -> Self {
        Self {
            deadline: Instant::now() + timeout,
            permit: None,
            shutdown,
        }
    }

    fn request(permit: &'a RequestPermit, shutdown: &'a AtomicBool) -> Self {
        Self {
            deadline: permit.deadline,
            permit: Some(permit),
            shutdown: Some(shutdown),
        }
    }

    fn check(&self) -> Result<(), String> {
        if self
            .shutdown
            .is_some_and(|shutdown| shutdown.load(Ordering::Acquire))
        {
            return Err("night-light worker is shutting down".to_owned());
        }
        if let Some(permit) = self.permit {
            permit.ensure_pending()?;
        }
        if Instant::now() >= self.deadline {
            return Err("night-light operation exceeded its deadline".to_owned());
        }
        Ok(())
    }

    fn remaining(&self) -> Result<Duration, String> {
        self.check()?;
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| "night-light operation exceeded its deadline".to_owned())
    }
}

static REQUESTS: OnceLock<SyncSender<Request>> = OnceLock::new();

/// Registers the provider and starts the only thread allowed to touch its
/// Wayland connection.
pub fn spawn(
    runtime: &Arc<Mutex<ProviderRuntime>>,
    shutdown: &Arc<AtomicBool>,
) -> std::io::Result<Option<Worker>> {
    let Ok(id) = ProviderId::new(NAME) else {
        eprintln!("celestina-provider-adapter: night-light: unusable provider name");
        return Ok(None);
    };
    lock_runtime(runtime).register(id.clone());

    let (sender, receiver) = sync_channel(REQUEST_CAPACITY);
    if REQUESTS.set(sender).is_err() {
        eprintln!("celestina-provider-adapter: night-light: worker already started");
        lock_runtime(runtime).unregister(&id);
        return Ok(None);
    }

    let runtime = Arc::clone(runtime);
    let worker_shutdown = Arc::clone(shutdown);
    Worker::spawn(NAME, shutdown, move || {
        run(&runtime, &id, &receiver, &worker_shutdown);
    })
    .map(Some)
}

/// Sends a typed switch request to the Wayland owner and waits for its final
/// gamma state. Acceptance is returned only after the last transition frame.
pub fn action(verb: &str, options: &Payload) -> Result<(), String> {
    let switch = match session::parse_for(NAME, verb, options)? {
        SessionRequest::NightLight(switch) => switch,
        // Choosing the warmth is not switching the light. The preference is
        // remembered here and the worker reads it on its next transition, so a
        // change while the light is already on lands without a toggle; while it
        // is off, it sets what the light will be.
        SessionRequest::NightLightTemperature(kelvin) => {
            super::settings::remember(|settings| settings.night_light_kelvin = kelvin)
                .map_err(|error| format!("cannot remember the night-light warmth: {error}"))?;
            if super::settings::current().night_light {
                if let Some(sender) = REQUESTS.get() {
                    let (reply, _answer) = sync_channel(1);
                    let _ = sender.try_send(Request {
                        switch: Switch::On,
                        permit: Arc::new(RequestPermit::new(Instant::now() + REQUEST_TIMEOUT)),
                        reply,
                    });
                }
            }
            return Ok(());
        }
        _ => return Err(session::unserved_verb(NAME, verb)),
    };
    let Some(sender) = REQUESTS.get() else {
        return Err("the night-light worker has not started".to_owned());
    };

    let (reply, answer) = sync_channel(1);
    let permit = Arc::new(RequestPermit::new(Instant::now() + REQUEST_TIMEOUT));
    sender
        .try_send(Request {
            switch,
            permit: Arc::clone(&permit),
            reply,
        })
        .map_err(|error| match error {
            TrySendError::Full(_) => "night light is finishing an earlier request".to_owned(),
            TrySendError::Disconnected(_) => "the night-light worker is gone".to_owned(),
        })?;
    let remaining = permit
        .deadline
        .checked_duration_since(Instant::now())
        .unwrap_or(Duration::ZERO);
    match answer.recv_timeout(remaining) {
        Ok(outcome) => outcome,
        Err(RecvTimeoutError::Disconnected) if permit.committed() => Ok(()),
        Err(RecvTimeoutError::Disconnected) => {
            let _ = permit.cancel();
            Err("the night-light worker ended without an answer".to_owned())
        }
        Err(RecvTimeoutError::Timeout) if permit.cancel() => {
            Err("night light did not finish its transition in time".to_owned())
        }
        Err(RecvTimeoutError::Timeout) if permit.committed() => Ok(()),
        Err(RecvTimeoutError::Timeout) => {
            Err("night light did not finish its transition in time".to_owned())
        }
    }
}

fn publish(runtime: &Mutex<ProviderRuntime>, id: &ProviderId, active: bool) {
    let mut payload = Payload::new();
    payload.insert("active".to_owned(), Value::from(active));
    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: night-light: {error}");
    }
}

fn remember(active: bool) {
    if let Err(error) = super::settings::remember(|settings| settings.night_light = active) {
        eprintln!("celestina-provider-adapter: night-light: {error}");
    }
}

fn requested(switch: Switch, active: bool) -> bool {
    match switch {
        Switch::On => true,
        Switch::Off => false,
        Switch::Toggle => !active,
    }
}

fn answer_unavailable(receiver: &Receiver<Request>, shutdown: &AtomicBool, reason: &str) {
    while !shutdown.load(Ordering::Acquire) {
        match receiver.recv_timeout(WAYLAND_POLL_INTERVAL) {
            Ok(request) => {
                let _ = request.reply.send(Err(reason.to_owned()));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn run(
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
    receiver: &Receiver<Request>,
    shutdown: &AtomicBool,
) {
    let mut session = match GammaSession::connect(shutdown) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("celestina-provider-adapter: night-light: {error}");
            publish(runtime, id, false);
            answer_unavailable(receiver, shutdown, &error);
            return;
        }
    };

    let mut active = false;
    if super::settings::current().night_light && !shutdown.load(Ordering::Acquire) {
        let budget = OperationBudget::internal(REQUEST_TIMEOUT, Some(shutdown));
        match session.set_active(true, &budget) {
            Ok(()) => active = true,
            Err(error) => {
                eprintln!("celestina-provider-adapter: night-light: {error}");
                let recovery = OperationBudget::internal(RECOVERY_TIMEOUT, Some(shutdown));
                session.fail_closed(&recovery);
            }
        }
    }
    publish(runtime, id, active);

    let lost_reason = loop {
        if shutdown.load(Ordering::Acquire) {
            break None;
        }
        if let Err(error) = session.pump(WAYLAND_POLL_INTERVAL, None) {
            break Some(error);
        }

        if active && session.needs_reconciliation() {
            let budget = OperationBudget::internal(REQUEST_TIMEOUT, Some(shutdown));
            if let Err(error) = session.reconcile_active_outputs(&budget) {
                eprintln!("celestina-provider-adapter: night-light: {error}");
                let recovery = OperationBudget::internal(RECOVERY_TIMEOUT, Some(shutdown));
                session.fail_closed(&recovery);
                active = false;
                publish(runtime, id, false);
            }
        }

        while let Ok(request) = receiver.try_recv() {
            let budget = OperationBudget::request(&request.permit, shutdown);
            let target = requested(request.switch, active);
            let outcome = if target == active {
                budget.check()
            } else {
                session.set_active(target, &budget)
            };

            match outcome {
                Ok(()) if request.permit.try_commit() => {
                    // Claim the request before publishing or persisting. The
                    // caller's timeout uses the inverse CAS, so exactly one
                    // side wins and a timed-out request can never commit late.
                    let _ = request.reply.send(Ok(()));
                    if target != active {
                        active = target;
                        publish(runtime, id, active);
                        remember(active);
                    }
                }
                Ok(()) => {
                    let restored = session.restore_confirmed(active, shutdown);
                    if restored != active {
                        active = restored;
                        publish(runtime, id, active);
                    }
                    let _ = request
                        .reply
                        .send(Err("night-light request expired before commit".to_owned()));
                }
                Err(error) => {
                    let _ = request.permit.cancel();
                    let restored = session.restore_confirmed(active, shutdown);
                    if restored != active {
                        active = restored;
                        publish(runtime, id, active);
                    }
                    let _ = request.reply.send(Err(error));
                }
            }
        }
    };

    // The worker owns the connection, so it is also the only place that can
    // restore identity in order. A lost connection already makes the
    // compositor restore the original tables when it destroys the client.
    if lost_reason.is_none() {
        // Cleanup deliberately ignores the already-raised shutdown flag, but
        // retains its own hard deadline so termination cannot hang.
        let budget = OperationBudget::internal(RECOVERY_TIMEOUT, None);
        if active {
            if let Err(error) = session.set_active(false, &budget) {
                eprintln!("celestina-provider-adapter: night-light: {error}");
                let emergency = OperationBudget::internal(RECOVERY_TIMEOUT, None);
                session.fail_closed(&emergency);
            }
        } else {
            session.release_inactive_controls(&budget);
        }
    }
    publish(runtime, id, false);

    if let Some(error) = lost_reason {
        eprintln!("celestina-provider-adapter: night-light: {error}");
        answer_unavailable(receiver, shutdown, &error);
    }
}

struct Controller {
    proxy: gamma_proto::ZwlrGammaControlV1,
    gamma_size: Option<u32>,
    pending_tables: Vec<File>,
    applied: Whitepoint,
    failure: Option<String>,
    valid: bool,
}

impl Controller {
    fn new(proxy: gamma_proto::ZwlrGammaControlV1) -> Self {
        Self {
            proxy,
            gamma_size: None,
            pending_tables: Vec::new(),
            applied: Whitepoint::NEUTRAL,
            failure: None,
            valid: true,
        }
    }

    fn write(&mut self, ramp: &[u16]) -> Result<(), String> {
        let byte_capacity = ramp
            .len()
            .checked_mul(std::mem::size_of::<u16>())
            .ok_or_else(|| "a gamma table is too large".to_owned())?;
        let mut bytes = Vec::with_capacity(byte_capacity);
        for sample in ramp {
            bytes.extend_from_slice(&sample.to_ne_bytes());
        }

        // Each request gets immutable storage of its own. SCM_RIGHTS shares a
        // file description, offset and contents; reusing one memfd for the next
        // frame could let the compositor read that later frame under an older
        // request. Files stay alive until the confirming display sync.
        let fd = memfd_create("celestina-night-light", MemfdFlags::CLOEXEC)
            .map_err(|error| format!("cannot create a gamma table: {error}"))?;
        let mut table = File::from(fd);
        table
            .seek(SeekFrom::Start(0))
            .and_then(|_| table.write_all(&bytes))
            // The compositor reads from the descriptor's current offset. The
            // rewind after writing is therefore part of the protocol, not an
            // implementation detail.
            .and_then(|()| table.seek(SeekFrom::Start(0)).map(|_| ()))
            .map_err(|error| format!("cannot write a gamma table: {error}"))?;
        self.proxy.set_gamma(table.as_fd());
        self.pending_tables.push(table);
        Ok(())
    }
}

struct Output {
    proxy: wl_output::WlOutput,
    control: Option<Controller>,
}

struct WaylandState {
    manager: Option<(u32, manager_proto::ZwlrGammaControlManagerV1)>,
    outputs: BTreeMap<u32, Output>,
    completed_sync: u64,
}

impl WaylandState {
    fn new() -> Self {
        Self {
            manager: None,
            outputs: BTreeMap::new(),
            completed_sync: 0,
        }
    }

    fn create_missing_controls(&mut self, qh: &QueueHandle<Self>) -> Result<(), String> {
        let Some((_, manager)) = self.manager.as_ref() else {
            return Err("this compositor does not offer wlr gamma control".to_owned());
        };
        if self.outputs.is_empty() {
            return Err("the compositor reports no output for night light".to_owned());
        }
        let manager = manager.clone();
        for (global, output) in &mut self.outputs {
            if output
                .control
                .as_ref()
                .is_some_and(|control| !control.valid || control.failure.is_some())
            {
                output.control = None;
            }
            if output.control.is_none() {
                let proxy = manager.get_gamma_control(&output.proxy, qh, *global);
                output.control = Some(Controller::new(proxy));
            }
        }
        Ok(())
    }

    fn validate_controls(&self) -> Result<(), String> {
        if self.outputs.is_empty() {
            return Err("the compositor reports no output for night light".to_owned());
        }
        for (global, output) in &self.outputs {
            let Some(control) = output.control.as_ref() else {
                return Err(format!("output {global} has no gamma controller"));
            };
            if let Some(failure) = &control.failure {
                return Err(format!(
                    "gamma control for output {global} failed: {failure}"
                ));
            }
            if !control.valid {
                return Err(format!(
                    "gamma control for output {global} is no longer valid"
                ));
            }
            let Some(size) = control.gamma_size else {
                return Err(format!("output {global} did not publish its gamma size"));
            };
            if nightlight::gamma_ramp(size, Whitepoint::NEUTRAL).is_none() {
                return Err(format!(
                    "output {global} published an unusable gamma size {size}"
                ));
            }
        }
        Ok(())
    }

    fn needs_reconciliation(&self) -> bool {
        self.outputs.is_empty()
            || self.outputs.values().any(|output| {
                output.control.as_ref().is_none_or(|control| {
                    !control.valid || control.failure.is_some() || control.gamma_size.is_none()
                })
            })
    }

    fn transition_plans(
        &self,
        target: Whitepoint,
    ) -> Result<BTreeMap<u32, Vec<TransitionFrame>>, String> {
        self.validate_controls()?;
        Ok(self
            .outputs
            .iter()
            .filter_map(|(global, output)| {
                output
                    .control
                    .as_ref()
                    .map(|control| (*global, nightlight::transition(control.applied, target)))
            })
            .collect())
    }

    fn apply_frame(
        &mut self,
        plans: &BTreeMap<u32, Vec<TransitionFrame>>,
        frame_index: usize,
    ) -> Result<(), String> {
        for (global, frames) in plans {
            let Some(frame) = frames.get(frame_index) else {
                return Err("night-light transition plans have different lengths".to_owned());
            };
            let Some(output) = self.outputs.get_mut(global) else {
                continue;
            };
            let Some(control) = output.control.as_mut() else {
                return Err(format!("output {global} lost its gamma controller"));
            };
            let Some(size) = control.gamma_size else {
                return Err(format!("output {global} lost its gamma size"));
            };
            let ramp = nightlight::gamma_ramp(size, frame.whitepoint)
                .ok_or_else(|| format!("output {global} has an unusable gamma size"))?;
            control.write(&ramp)?;
            control.applied = frame.whitepoint;
        }
        Ok(())
    }

    fn destroy_controls(&mut self) {
        for output in self.outputs.values_mut() {
            if let Some(control) = output.control.take() {
                if control.valid {
                    control.proxy.destroy();
                }
            }
        }
    }

    fn clear_pending_tables(&mut self) {
        for output in self.outputs.values_mut() {
            if let Some(control) = output.control.as_mut() {
                control.pending_tables.clear();
            }
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == wl_output::WlOutput::interface().name => {
                state.outputs.entry(name).or_insert_with(|| Output {
                    proxy: registry.bind::<wl_output::WlOutput, _, _>(
                        name,
                        version.min(1),
                        qh,
                        name,
                    ),
                    control: None,
                });
            }
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == manager_proto::ZwlrGammaControlManagerV1::interface().name
                && state.manager.is_none() =>
            {
                state.manager = Some((
                    name,
                    registry.bind::<manager_proto::ZwlrGammaControlManagerV1, _, _>(
                        name,
                        version.min(1),
                        qh,
                        (),
                    ),
                ));
            }
            wl_registry::Event::GlobalRemove { name } => {
                if state
                    .manager
                    .as_ref()
                    .is_some_and(|(global, _)| *global == name)
                {
                    if let Some((_, manager)) = state.manager.take() {
                        manager.destroy();
                    }
                }
                if let Some(mut output) = state.outputs.remove(&name) {
                    if let Some(control) = output.control.take() {
                        if control.valid {
                            control.proxy.destroy();
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, u32> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_output::WlOutput,
        _: wl_output::Event,
        _: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_callback::WlCallback, u64> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        token: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            state.completed_sync = state.completed_sync.max(*token);
        }
    }
}

impl Dispatch<gamma_proto::ZwlrGammaControlV1, u32> for WaylandState {
    fn event(
        state: &mut Self,
        control: &gamma_proto::ZwlrGammaControlV1,
        event: gamma_proto::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(current) = state
            .outputs
            .get_mut(global)
            .and_then(|output| output.control.as_mut())
        else {
            return;
        };
        if current.proxy.id() != control.id() {
            return;
        }

        match event {
            gamma_proto::Event::GammaSize { size } => current.gamma_size = Some(size),
            gamma_proto::Event::Failed => {
                current.failure = Some("the compositor rejected or revoked it".to_owned());
                current.valid = false;
                control.destroy();
            }
            _ => {}
        }
    }
}

delegate_noop!(WaylandState: ignore manager_proto::ZwlrGammaControlManagerV1);

struct GammaSession {
    connection: Connection,
    _registry: wl_registry::WlRegistry,
    queue: EventQueue<WaylandState>,
    qh: QueueHandle<WaylandState>,
    state: WaylandState,
    next_sync: u64,
}

impl GammaSession {
    fn connect(shutdown: &AtomicBool) -> Result<Self, String> {
        let connection = Connection::connect_to_env()
            .map_err(|error| format!("cannot connect to Wayland: {error}"))?;
        let queue = connection.new_event_queue();
        let qh = queue.handle();
        let registry = connection.display().get_registry(&qh, ());
        let mut session = Self {
            connection,
            _registry: registry,
            queue,
            qh,
            state: WaylandState::new(),
            next_sync: 0,
        };
        let budget = OperationBudget::internal(REQUEST_TIMEOUT, Some(shutdown));
        session.sync(&budget, "cannot discover gamma-control globals")?;
        Ok(session)
    }

    fn sync(&mut self, budget: &OperationBudget<'_>, what: &str) -> Result<(), String> {
        budget.check()?;
        self.next_sync = self.next_sync.wrapping_add(1).max(1);
        let token = self.next_sync;
        self.connection.display().sync(&self.qh, token);
        self.queue
            .flush()
            .map_err(|error| format!("{what}: cannot flush sync request: {error}"))?;

        while self.state.completed_sync < token {
            let timeout = budget.remaining()?.min(WAYLAND_POLL_INTERVAL);
            self.pump(timeout, Some(budget))
                .map_err(|error| format!("{what}: {error}"))?;
        }
        budget.check()
    }

    fn pump(
        &mut self,
        timeout: Duration,
        budget: Option<&OperationBudget<'_>>,
    ) -> Result<(), String> {
        if let Some(budget) = budget {
            budget.check()?;
        }
        self.queue
            .dispatch_pending(&mut self.state)
            .map_err(|error| format!("lost the Wayland gamma connection: {error}"))?;
        if let Some(budget) = budget {
            budget.check()?;
        }
        self.queue
            .flush()
            .map_err(|error| format!("cannot flush the Wayland gamma connection: {error}"))?;

        let Some(guard) = self.queue.prepare_read() else {
            return Ok(());
        };
        let fd = guard.connection_fd();
        let mut fds = [PollFd::new(&fd, PollFlags::IN)];
        let timeout = budget
            .map(OperationBudget::remaining)
            .transpose()?
            .map_or(timeout, |remaining| remaining.min(timeout));
        let timeout = Timespec {
            tv_sec: i64::try_from(timeout.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(timeout.subsec_nanos()),
        };
        match poll(&mut fds, Some(&timeout)) {
            Ok(count) if count > 0 && fds[0].revents().contains(PollFlags::IN) => {
                guard
                    .read()
                    .map_err(|error| format!("cannot read Wayland gamma events: {error}"))?;
            }
            Ok(_) => drop(guard),
            Err(error) => {
                drop(guard);
                return Err(format!("cannot wait for Wayland gamma events: {error}"));
            }
        }
        self.queue
            .dispatch_pending(&mut self.state)
            .map_err(|error| format!("cannot dispatch Wayland gamma events: {error}"))?;
        if let Some(budget) = budget {
            budget.check()?;
        }
        Ok(())
    }

    fn needs_reconciliation(&self) -> bool {
        self.state.needs_reconciliation()
    }

    fn ensure_controls(&mut self, budget: &OperationBudget<'_>) -> Result<(), String> {
        budget.check()?;
        self.state.create_missing_controls(&self.qh)?;
        self.sync(budget, "cannot initialize output gamma controls")?;
        self.state.validate_controls()
    }

    fn transition_to(
        &mut self,
        target: Whitepoint,
        budget: &OperationBudget<'_>,
    ) -> Result<(), String> {
        budget.check()?;
        let plans = self.state.transition_plans(target)?;
        let Some(schedule) = plans.values().next() else {
            return Err("there is no output to transition".to_owned());
        };
        let started = Instant::now();
        for (index, frame) in schedule.iter().enumerate() {
            let frame_at = started + frame.offset;
            while let Some(remaining) = frame_at.checked_duration_since(Instant::now()) {
                budget.check()?;
                thread::sleep(remaining.min(WAYLAND_POLL_INTERVAL));
            }
            budget.check()?;
            self.state.apply_frame(&plans, index)?;
            self.queue
                .flush()
                .map_err(|error| format!("cannot send a gamma transition frame: {error}"))?;
        }
        self.sync(budget, "cannot confirm the final gamma transition frame")?;
        self.state.clear_pending_tables();
        self.state.validate_controls()
    }

    /// The warmth the person chose, read at the moment it is applied.
    ///
    /// Read here rather than cached so that changing the temperature while the
    /// light is already on takes effect on the next transition, without the
    /// session having to toggle it off and on again.
    fn chosen_whitepoint() -> Whitepoint {
        Whitepoint::for_temperature(super::settings::current().night_light_kelvin)
    }

    fn set_active(&mut self, active: bool, budget: &OperationBudget<'_>) -> Result<(), String> {
        if active {
            self.ensure_controls(budget)?;
            self.transition_to(Self::chosen_whitepoint(), budget)
        } else {
            self.transition_to(Whitepoint::NEUTRAL, budget)?;
            self.destroy_controls(budget)
        }
    }

    fn reconcile_active_outputs(&mut self, budget: &OperationBudget<'_>) -> Result<(), String> {
        self.ensure_controls(budget)?;
        // Existing outputs start and end warm; a newly added output is the
        // only one whose per-output plan travels from neutral to warm.
        self.transition_to(Self::chosen_whitepoint(), budget)
    }

    fn destroy_controls(&mut self, budget: &OperationBudget<'_>) -> Result<(), String> {
        budget.check()?;
        self.state.destroy_controls();
        self.queue
            .flush()
            .map_err(|error| format!("cannot release output gamma controls: {error}"))?;
        self.sync(budget, "cannot confirm output gamma-control release")
    }

    fn fail_closed(&mut self, budget: &OperationBudget<'_>) {
        // Best effort only: on the ordinary path, `set_active(false)` reached
        // identity gradually and confirmed it before this destroy. On an error
        // path, one final identity table minimizes any surviving partial tint;
        // destroying the objects (or losing the connection) then restores the
        // compositor's original tables by protocol law.
        if budget.check().is_ok() {
            for output in self.state.outputs.values_mut() {
                let Some(control) = output.control.as_mut() else {
                    continue;
                };
                let Some(size) = control.gamma_size else {
                    continue;
                };
                if control.valid {
                    if let Some(ramp) = nightlight::gamma_ramp(size, Whitepoint::NEUTRAL) {
                        let _ = control.write(&ramp);
                    }
                }
            }
            let _ = self.queue.flush();
            let _ = self.sync(budget, "cannot confirm emergency identity gamma");
        }
        self.state.clear_pending_tables();
        self.state.destroy_controls();
        let _ = self.queue.flush();
        let _ = self.sync(budget, "cannot confirm emergency gamma-control release");
    }

    fn release_inactive_controls(&mut self, budget: &OperationBudget<'_>) {
        if self
            .state
            .outputs
            .values()
            .any(|output| output.control.is_some())
        {
            let _ = self.destroy_controls(budget);
        }
    }

    fn restore_confirmed(&mut self, confirmed: bool, shutdown: &AtomicBool) -> bool {
        let budget = OperationBudget::internal(RECOVERY_TIMEOUT, Some(shutdown));
        let restored = if confirmed {
            self.set_active(true, &budget)
        } else if self
            .state
            .outputs
            .values()
            .any(|output| output.control.is_some())
        {
            self.set_active(false, &budget)
        } else {
            Ok(())
        };
        if restored.is_ok() {
            return confirmed;
        }

        let emergency = OperationBudget::internal(RECOVERY_TIMEOUT, Some(shutdown));
        self.fail_closed(&emergency);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_wins_before_the_worker_can_commit() {
        let permit = RequestPermit::new(Instant::now() + Duration::from_secs(1));

        assert!(permit.cancel());
        assert!(!permit.try_commit());
        assert!(!permit.committed());
    }

    #[test]
    fn a_committed_request_cannot_be_relabelled_as_timed_out() {
        let permit = RequestPermit::new(Instant::now() + Duration::from_secs(1));

        assert!(permit.try_commit());
        assert!(!permit.cancel());
        assert!(permit.committed());
    }

    #[test]
    fn an_expired_request_never_commits() {
        let permit = RequestPermit::new(Instant::now());

        assert!(permit.ensure_pending().is_err());
        assert!(!permit.try_commit());
        assert!(!permit.committed());
    }

    #[test]
    fn an_internal_operation_observes_worker_shutdown() {
        let shutdown = AtomicBool::new(true);
        let budget = OperationBudget::internal(Duration::from_secs(1), Some(&shutdown));

        assert!(budget.check().is_err());
    }

    #[test]
    fn an_internal_operation_observes_its_deadline() {
        let budget = OperationBudget {
            deadline: Instant::now(),
            permit: None,
            shutdown: None,
        };

        assert!(budget.remaining().is_err());
    }
}
