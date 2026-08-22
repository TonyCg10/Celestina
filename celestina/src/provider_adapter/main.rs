//! The panel's aggregate provider helper.
//!
//! Every bar provider that needs long-lived, non-Qt IO lives in this one
//! process — never one helper per widget — and publishes into a single
//! coalesced snapshot the Qt host reads on its GUI thread. Later shell-owned,
//! non-Qt services extend this helper rather than starting a third runtime.
//!
//! The shape is the one the Niri adapter proved and `celestina-shell-core` now
//! owns: a dedicated bounded stdin reader, a bounded command queue, one worker,
//! and a single serialized writer every frame leaves through. What is new here
//! is the *aggregate*: providers come and go, so the host is told which
//! provider a value belongs to, which generation produced it and when a
//! provider has nothing to say any more.
//!
//! This file is the plumbing only. Each provider owns its own module — what it
//! reads, how often, and which verbs it serves — so adding one is adding a
//! module and a line, never editing a loop that knows about all of them.

use std::io::{self, BufReader, BufWriter, Stdout};
use std::process::{self, ExitCode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell_core::command::{parse_command, Command, Rejection, ResultFrame};
use celestina_shell_core::diagnostics::{Event, Level, Value};
use celestina_shell_core::journal::{self, Journal};
use celestina_shell_core::lines::{read_bounded_line, HostLine, SharedWriter};
use celestina_shell_core::runtime::ProviderRuntime;

mod audio;
mod brightness;
mod clipboard;
mod generated;
mod held;
mod launcher;
mod media;
mod melibea;
mod nightlight;
mod notifications;
mod portal_settings;
mod recorder;
mod session;
mod sessionholds;
mod settings;
mod sysmon;
mod tools;
mod wallpaper;
mod weather;
mod worker;

use tools::lock_runtime;
use worker::Worker;

/// The host may not outrun the helper: further requests are refused with a
/// visible failure instead of growing an unbounded backlog.
const COMMAND_QUEUE_CAPACITY: usize = 32;
/// The loop wakes often enough to notice a pending frame promptly without
/// polling anything on its own.
const IDLE_TICK: Duration = Duration::from_millis(100);

type HelperWriter = Arc<SharedWriter<BufWriter<Stdout>>>;

/// The clock the runtime's rules are measured against. It is the only piece of
/// the aggregate that belongs to a process rather than to policy, which is why
/// it is the only piece that lives here.
struct Clock {
    started: Instant,
}

impl Clock {
    fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }

    fn now_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Which provider answers a verb. The dispatch is the one place that knows
/// every provider by name; each module owns what its verbs mean.
fn perform(command: &Command, runtime: &Mutex<ProviderRuntime>) -> Result<(), String> {
    match command.provider.as_str() {
        sysmon::NAME => sysmon::action(&command.verb),
        audio::NAME => audio::action(&command.verb, &command.options, runtime, &command.provider),
        media::NAME => media::action(&command.verb),
        melibea::NAME => melibea::action(&command.verb, &command.options, &command.id),
        // The host saw the session's outputs change. It is not a session verb
        // — no key binding produces it and it changes no device — so it never
        // enters the vocabulary a binding writes; it only tells the one DDC
        // worker that looking again is worth the cost.
        brightness::NAME if command.verb == "outputs-changed" => {
            brightness::request_redetect();
            Ok(())
        }
        brightness::NAME => brightness::action(&command.verb, &command.options),
        recorder::NAME => recorder::action(&command.verb, &command.options),
        session::POWER if command.verb == "cycle" => {
            session::cycle_power_profile(runtime, &command.provider)
        }
        // Both connectivity indicators share one action path: the verbs differ
        // but the rule does not — validate against the last confirmed
        // inventory, record what must be observed, then run the tool.
        session::NETWORK | session::BLUETOOTH => session::action(
            &command.provider,
            &command.verb,
            &command.options,
            &command.id,
            runtime,
        ),
        nightlight::NAME => nightlight::action(&command.verb, &command.options),
        sessionholds::CAFFEINE => {
            sessionholds::action(&command.verb, &command.options, runtime, &command.provider)
        }
        launcher::NAME => {
            launcher::action(&command.verb, &command.options, runtime, &command.provider)
        }
        clipboard::NAME => clipboard::action(&command.verb, &command.options),
        settings::NAME => settings::action(&command.verb, &command.options),
        wallpaper::NAME | wallpaper::GALLERY_NAME => {
            wallpaper::action(&command.verb, &command.options, runtime, &command.provider)
        }
        notifications::NAME => {
            notifications::action(&command.verb, &command.options, runtime, &command.provider)
        }
        provider => Err(format!(
            "'{provider}' does not serve the verb '{}'",
            command.verb
        )),
    }
}

fn reject(writer: &HelperWriter, rejection: &Rejection) {
    let Some(id) = rejection.id.as_deref() else {
        eprintln!(
            "celestina-provider-adapter: ignored an unusable command: {}",
            rejection.reason
        );
        return;
    };
    if let Err(error) = writer.emit(&ResultFrame::failed(id, &rejection.reason)) {
        eprintln!("celestina-provider-adapter: {error}");
    }
}

/// Performs one queued command at a time, so a slow provider action cannot
/// stall the snapshot loop or the reader.
fn run_commands(
    receiver: &Receiver<Command>,
    runtime: &Mutex<ProviderRuntime>,
    writer: &HelperWriter,
    shutdown: &AtomicBool,
) {
    while !shutdown.load(Ordering::Acquire) {
        let command = match receiver.recv_timeout(IDLE_TICK) {
            Ok(command) => command,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if let Some(rejection) = lock_runtime(runtime).refuse_unknown(&command) {
            reject(writer, &rejection);
            continue;
        }

        // Acceptance is only that the helper carried it out; what it did to the
        // machine shows up in that provider's next value, if anywhere.
        let (frame, accepted) = match perform(&command, runtime) {
            Ok(()) => (ResultFrame::accepted(&command.id), true),
            Err(reason) => (ResultFrame::failed(&command.id, &reason), false),
        };
        if let Err(error) = writer.emit(&frame) {
            if accepted {
                session::discard(&command.provider, &command.id);
                if command.provider.as_str() == melibea::NAME {
                    melibea::discard(&command.id);
                }
            }
            eprintln!("celestina-provider-adapter: {error}");
            // Only a lost pipe ends this worker. A frame that could not be
            // built is one answer missing, not a reason to stop answering.
            if error.is_fatal() {
                shutdown.store(true, Ordering::Release);
                return;
            }
        } else if accepted {
            // Connectivity reservations cannot be observed until this exact
            // `accepted` frame is already on the pipe.
            session::arm(&command.provider, &command.id);
            if command.provider.as_str() == melibea::NAME {
                melibea::arm(&command.id, runtime);
            }
        }
    }
}

fn queue_command(sender: &SyncSender<Command>, writer: &HelperWriter, command: Command) {
    match sender.try_send(command) {
        Ok(()) => {}
        Err(TrySendError::Full(command)) => reject(
            writer,
            &Rejection {
                id: Some(command.id),
                reason: "the helper's command queue is full".to_owned(),
            },
        ),
        Err(TrySendError::Disconnected(command)) => reject(
            writer,
            &Rejection {
                id: Some(command.id),
                reason: "the helper's command worker is gone".to_owned(),
            },
        ),
    }
}

fn read_host_commands(sender: &SyncSender<Command>, writer: &HelperWriter) {
    let mut reader = BufReader::new(io::stdin());

    loop {
        match read_bounded_line(&mut reader) {
            Ok(HostLine::End) => return,
            Ok(HostLine::Oversized) => {
                eprintln!("celestina-provider-adapter: discarded an oversized command");
            }
            Ok(HostLine::Complete(line)) => {
                if line.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                match parse_command(&line) {
                    Ok(command) => queue_command(sender, writer, command),
                    Err(rejection) => reject(writer, &rejection),
                }
            }
            Err(error) => {
                eprintln!("celestina-provider-adapter: cannot read host commands: {error}");
                return;
            }
        }
    }
}

/// Gives back the process-backed session hold however `run` ends.
///
/// The hold lives in a process-wide static, so nothing drops it on an early
/// return. Night light has a separate owned worker whose join restores identity
/// before its Wayland connection is released.
struct HeldStates;

impl Drop for HeldStates {
    fn drop(&mut self) {
        sessionholds::release_all();
    }
}

/// Waits for a worker that a session may not have started at all — DDC held
/// back by the environment, a provider whose own initialization declined.
fn join_if_started(worker: Option<worker::Worker>) {
    if let Some(worker) = worker {
        worker.join();
    }
}

fn run() -> io::Result<()> {
    let writer: HelperWriter = Arc::new(SharedWriter::new(BufWriter::new(io::stdout())));
    // One process, one generation. A host that sees a new generation clears
    // whatever the previous helper published instead of blending the two.
    let runtime = Arc::new(Mutex::new(ProviderRuntime::new(process::id().into())));
    let clock = Clock::new();
    let (sender, receiver) = sync_channel::<Command>(COMMAND_QUEUE_CAPACITY);
    let shutdown = Arc::new(AtomicBool::new(false));

    // QProcess uses SIGTERM for an orderly helper restart. The held programs
    // affect the whole session, so they must be released before this process
    // leaves even when stdin did not close first.
    let mut shutdown_signals = signal_hook::iterator::Signals::new([
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGHUP,
    ])?;
    let signal_shutdown = Arc::clone(&shutdown);
    thread::Builder::new()
        .name("shutdown-signals".to_owned())
        .spawn(move || {
            if shutdown_signals.forever().next().is_some() {
                signal_shutdown.store(true, Ordering::Release);
            }
        })?;

    // Settings first: they are what the person chose, and the providers below
    // start from them rather than from their own defaults.
    settings::spawn(&runtime);

    // Each provider announces itself before it reads anything, so a command can
    // reach it while it is still starting up.
    sysmon::spawn(&runtime)?;
    media::spawn(&runtime)?;
    let melibea_worker = melibea::spawn(&runtime, &shutdown)?;
    audio::spawn(&runtime)?;
    // A diagnostic safety hold can remove DDC without changing any other
    // provider. It is intentionally process-local and opt-in; normal product
    // behavior still owns brightness.
    let brightness_worker = if std::env::var_os("CELESTINA_DISABLE_DDC").is_some() {
        None
    } else {
        Some(brightness::spawn(&runtime, &shutdown)?)
    };
    session::spawn(&runtime, &shutdown)?;
    // Declared before the holds thread starts and therefore dropped after it
    // has been joined: whatever this helper is holding is given back however
    // `run` ends, including the initialization failures below.
    let _released_on_exit = HeldStates;
    let holds_worker = sessionholds::spawn(&runtime, &shutdown)?;
    let nightlight_worker = nightlight::spawn(&runtime, &shutdown)?;
    launcher::spawn(&runtime)?;
    clipboard::spawn(&runtime)?;
    notifications::spawn(&runtime)?;
    weather::spawn(&runtime)?;
    wallpaper::spawn(&runtime)?;
    portal_settings::spawn(&runtime)?;
    let recorder_worker = recorder::spawn(&runtime, &shutdown)?;
    // Offered, never applied: the author references these or does not.
    generated::write_all();

    let worker_runtime = Arc::clone(&runtime);
    let worker_writer = Arc::clone(&writer);
    let worker_shutdown = Arc::clone(&shutdown);
    let worker = Worker::spawn("provider-commands", &shutdown, move || {
        run_commands(&receiver, &worker_runtime, &worker_writer, &worker_shutdown);
    })?;

    let reader_writer = Arc::clone(&writer);
    let reader_shutdown = Arc::clone(&shutdown);
    thread::Builder::new()
        .name("host-commands".to_owned())
        .spawn(move || {
            read_host_commands(&sender, &reader_writer);
            // Our stdin closed, so the host is gone or shutting down. Dropping
            // the sender closes the queue; the main thread owns every join and
            // the final release order.
            drop(sender);
            reader_shutdown.store(true, Ordering::Release);
        })?;

    // Publish what the runtime says is owed, when it says it is owed. With no
    // provider having spoken yet the host still gets an immediate empty frame,
    // so it knows the helper is alive and carrying nothing.
    while !shutdown.load(Ordering::Acquire) {
        let now = clock.now_ms();
        let mut state = lock_runtime(&runtime);
        // What became of requests already accepted. A provider observes the
        // effect on its own thread; this is the one place that writes.
        let outcomes = state.take_outcomes();
        drop(state);
        for outcome in &outcomes {
            if let Err(error) = writer.emit(&outcome.frame()) {
                eprintln!("celestina-provider-adapter: {error}");
                if error.is_fatal() {
                    shutdown.store(true, Ordering::Release);
                    break;
                }
            }
        }

        let mut state = lock_runtime(&runtime);
        if state.due(now) {
            if let Err(error) = writer.emit(&state.take_frame(now)) {
                eprintln!("celestina-provider-adapter: {error}");
                // A frame the host would have discarded is skipped, not
                // retried: the next change publishes the whole set again. Only
                // a lost pipe means there is nobody left to publish to.
                if error.is_fatal() {
                    shutdown.store(true, Ordering::Release);
                    break;
                }
            }
        }
        drop(state);
        thread::sleep(IDLE_TICK);
    }

    // Stop hardware IO before allowing the process to return. The brightness
    // worker observes this flag inside each bounded DDC child, kills and reaps
    // that child, then becomes joinable. No abrupt process exit bypasses this
    // ownership chain.
    journal::record(Event::new(Level::Critical, "helper.shutdown.start"));
    shutdown.store(true, Ordering::Release);
    join_if_started(brightness_worker);
    worker.join();
    // The holds thread stops before its holds are given back, so it cannot take
    // one after the release. `_released_on_exit` performs that release as this
    // function returns, by whichever path it returns.
    join_if_started(holds_worker);
    join_if_started(nightlight_worker);
    join_if_started(recorder_worker);
    // After its reaper has been joined, so nothing is watching for the exit
    // this asks for: a session that ends mid-recording still gets a file it
    // can play.
    recorder::stop_for_shutdown();
    melibea_worker.join();
    journal::record(Event::new(Level::Critical, "helper.shutdown.end"));
    Ok(())
}

fn main() -> ExitCode {
    // Installed before anything else runs, so a helper that dies during its own
    // startup still leaves a record of having tried. The generation is this
    // process's own id, which is what the host correlates its restarts by.
    journal::install(Journal::for_component(
        "provider-adapter",
        u64::from(process::id()),
    ));
    journal::record(
        Event::new(Level::Critical, "helper.start")
            .with_text("helper", "provider-adapter")
            .with_text("version", env!("CARGO_PKG_VERSION"))
            // A count, not the arguments: this helper takes none today, and a
            // future one must not start leaking a path by growing some.
            .with(
                "argument_count",
                Value::Uint(std::env::args().skip(1).count() as u64),
            ),
    );

    let outcome = run();
    journal::record(
        Event::new(Level::Critical, "helper.stop")
            .with_text("helper", "provider-adapter")
            .with("ok", Value::Bool(outcome.is_ok()))
            .with_text(
                "error",
                &outcome
                    .as_ref()
                    .err()
                    .map_or(String::new(), ToString::to_string),
            ),
    );
    // Deterministic and bounded: the writer is asked to drain here rather than
    // left to process exit, but an unresponsive filesystem cannot hold exit.
    journal::close_process_journal();

    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("celestina-provider-adapter: fatal: {error}");
            ExitCode::FAILURE
        }
    }
}
