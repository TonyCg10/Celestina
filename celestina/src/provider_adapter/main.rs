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
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell_core::command::{parse_command, Command, Rejection, ResultFrame};
use celestina_shell_core::lines::{read_bounded_line, HostLine, SharedWriter};
use celestina_shell_core::runtime::ProviderRuntime;

mod audio;
mod brightness;
mod media;
mod session;
mod sysmon;
mod tools;

use tools::lock_runtime;

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
        audio::NAME => audio::action(&command.verb, runtime, &command.provider),
        media::NAME => media::action(&command.verb),
        brightness::NAME => brightness::action(&command.verb, &command.options),
        session::POWER if command.verb == "cycle" => session::cycle_power_profile(),
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
) {
    while let Ok(command) = receiver.recv() {
        if let Some(rejection) = lock_runtime(runtime).refuse_unknown(&command) {
            reject(writer, &rejection);
            continue;
        }

        // Acceptance is only that the helper carried it out; what it did to the
        // machine shows up in that provider's next value, if anywhere.
        let frame = match perform(&command, runtime) {
            Ok(()) => ResultFrame::accepted(&command.id),
            Err(reason) => ResultFrame::failed(&command.id, &reason),
        };
        if let Err(error) = writer.emit(&frame) {
            eprintln!("celestina-provider-adapter: {error}");
            return;
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

fn run() -> io::Result<()> {
    let writer: HelperWriter = Arc::new(SharedWriter::new(BufWriter::new(io::stdout())));
    // One process, one generation. A host that sees a new generation clears
    // whatever the previous helper published instead of blending the two.
    let runtime = Arc::new(Mutex::new(ProviderRuntime::new(process::id().into())));
    let clock = Clock::new();
    let (sender, receiver) = sync_channel::<Command>(COMMAND_QUEUE_CAPACITY);

    // Each provider announces itself before it reads anything, so a command can
    // reach it while it is still starting up.
    sysmon::spawn(&runtime)?;
    media::spawn(&runtime)?;
    audio::spawn(&runtime)?;
    brightness::spawn(&runtime)?;
    session::spawn(&runtime)?;

    let worker_runtime = Arc::clone(&runtime);
    let worker_writer = Arc::clone(&writer);
    let worker = thread::Builder::new()
        .name("provider-commands".to_owned())
        .spawn(move || run_commands(&receiver, &worker_runtime, &worker_writer))?;

    let reader_writer = Arc::clone(&writer);
    thread::Builder::new()
        .name("host-commands".to_owned())
        .spawn(move || {
            read_host_commands(&sender, &reader_writer);
            // Our stdin closed, so the host is gone or shutting down. Close the
            // queue, let the worker finish what it holds, and leave together.
            drop(sender);
            if worker.join().is_err() {
                eprintln!("celestina-provider-adapter: the command worker panicked");
            }
            process::exit(0);
        })?;

    // Publish what the runtime says is owed, when it says it is owed. With no
    // provider having spoken yet the host still gets an immediate empty frame,
    // so it knows the helper is alive and carrying nothing.
    loop {
        let now = clock.now_ms();
        let mut state = lock_runtime(&runtime);
        if state.due(now) {
            if let Err(error) = writer.emit(&state.take_frame(now)) {
                eprintln!("celestina-provider-adapter: {error}");
                return Ok(());
            }
        }
        drop(state);
        thread::sleep(IDLE_TICK);
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("celestina-provider-adapter: fatal: {error}");
            ExitCode::FAILURE
        }
    }
}
