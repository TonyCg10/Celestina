//! The process's startup, in the order the design fixes (§4.2): harden, check
//! for a tracer, pin the verifier, write the pid file, read the bundle, then
//! lock. Everything before `requested` that fails ends the process with exit
//! 15 (setup) or 1, and nothing has been asked of the compositor yet.
//!
//! The environment is read in exactly two places, both here and both from the
//! allowlist the shell spawns the lock with: `XDG_RUNTIME_DIR` and
//! `WAYLAND_DISPLAY`, for the author's pid file. (The toolkit reads
//! `WAYLAND_DISPLAY` and the launcher marker itself, in its connection guard:
//! `celestina_wayland::lock::ConnectPolicy`, round 5.) No variable names a
//! binary, a service or a library.
#![forbid(unsafe_code)]

use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use celestina_lock_proto::{service_check, LockExit, Service, Status, Verdict};
use celestina_wayland::lock::ConnectPolicy;

use crate::app::{run, Run};
use crate::backdrop::{self, Loaded};
use crate::hardening;
use crate::machine::Config;
use crate::report::Reporter;
use crate::state_file::StateFile;
use crate::verify::{Attempts, PinnedBinary, Timings, Verifier};
use crate::view::CpuPresenter;

#[cfg(feature = "fake-verifier")]
use std::ffi::OsString;

/// Which verifier to run.
#[derive(Debug)]
pub enum VerifierChoice {
    /// `celestina-lock-verify` beside the running executable, found only
    /// after the process hardened itself (design §4.2).
    BesideSelf,
    /// `celestina-lock-verify` at this path, with the production contract.
    Production(PathBuf),
    /// Rehearsal only: any ELF at `path`, with the arguments given, and a
    /// password typed by the probe itself `auto_type_after` after `locked`.
    #[cfg(feature = "fake-verifier")]
    Fake {
        path: PathBuf,
        args: Vec<OsString>,
        auto_type_after: Duration,
    },
}

/// How the process was asked to run.
#[derive(Debug)]
pub struct Launch {
    /// `--rehearsal`: attempts go to the rehearsal service; the lock never
    /// unlocks.
    pub rehearsal: bool,
    pub verifier: VerifierChoice,
    /// How the process was launched, chosen by the binary with
    /// `ConnectPolicy::detect` (no default: the toolkit's connection guard
    /// refuses a launch the environment does not support, round 5).
    pub connect: ConnectPolicy,
}

/// The verifier beside the running executable (design §4.2): never found
/// through `PATH` or an environment variable. After a deploy replaced the
/// running binary the kernel reports `... (deleted)`; only the directory is
/// used, so the name suffix does no harm.
///
/// # Errors
///
/// When the executable's own path cannot be read.
pub fn verifier_beside_self() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    Ok(dir.join("celestina-lock-verify"))
}

/// The subscriber the lock uses: stderr, the level fixed at `info`, no
/// environment filter (so `RUST_LOG` changes nothing and no filter crate is
/// linked).
#[must_use]
pub fn logger() -> impl tracing::Subscriber + Send + Sync {
    logger_to(std::io::stderr)
}

/// [`logger`] over any writer: the tests capture what it lets through.
#[must_use]
pub fn logger_to<W>(writer: W) -> impl tracing::Subscriber + Send + Sync
where
    W: for<'a> tracing_subscriber::fmt::MakeWriter<'a> + Send + Sync + 'static,
{
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(writer)
        .with_ansi(false)
        .finish()
}

/// Installs [`logger`] as the process's subscriber.
pub fn init_logging() {
    // A second call in one process (the tests) finds one already installed.
    let _ = tracing::subscriber::set_global_default(logger());
}

/// The descriptor `WAYLAND_SOCKET` names, the one the lock is handed on
/// purpose (read once, here, and only as a number to keep open).
fn handed_in_descriptor() -> Option<i32> {
    let socket = std::env::var_os("WAYLAND_SOCKET");
    celestina_lock_proto::wayland_fd(socket.as_deref().and_then(|v| v.to_str()))
}

/// Runs the whole process and returns its exit status.
pub fn start(launch: Launch) -> ExitCode {
    // Hardening is the first thing the process does (design §4.2, M4): before
    // the subscriber, before argv-derived paths, before any file is looked at.
    // Even before it, whatever the shell handed over by accident is closed
    // (round 4, I-α): a copy of the logind delay inhibitor must not live as
    // long as the lock does.
    let hardened = hardening::close_inherited_descriptors(handed_in_descriptor())
        .and_then(|()| hardening::apply());
    init_logging();
    let exit = match prepare_and_run_with(launch, hardened) {
        Ok(exit) | Err(exit) => exit,
    };
    // `ExitCode::from` takes a `u8`; every code of the table fits.
    ExitCode::from(u8::try_from(exit.code()).unwrap_or(1))
}

fn pin(path: &Path) -> Result<PinnedBinary, LockExit> {
    PinnedBinary::open(path).map_err(|err| {
        tracing::error!(%err, "cannot pin the verifier: not locking");
        LockExit::SetupFailed
    })
}

fn pin_verifier(
    choice: VerifierChoice,
    service: Service,
) -> Result<(Verifier, Option<Duration>), LockExit> {
    match choice {
        VerifierChoice::BesideSelf => {
            let path = verifier_beside_self().map_err(|err| {
                tracing::error!(%err, "cannot find the verifier beside the lock");
                LockExit::SetupFailed
            })?;
            Ok((Verifier::for_service(pin(&path)?, service), None))
        }
        VerifierChoice::Production(path) => Ok((Verifier::for_service(pin(&path)?, service), None)),
        #[cfg(feature = "fake-verifier")]
        VerifierChoice::Fake {
            path,
            args,
            auto_type_after,
        } => Ok((
            Verifier::with_args(pin(&path)?, args),
            Some(auto_type_after),
        )),
    }
}

/// Which candidate files of `service` exist (`stat`, never opened).
fn existing(service: Service) -> Vec<PathBuf> {
    service
        .paths()
        .into_iter()
        .filter(|path| std::fs::symlink_metadata(path).is_ok())
        .collect()
}

/// The pid file for the author, when the allowlisted variables are there.
fn state_file() -> Option<StateFile> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
    let display = std::env::var_os("WAYLAND_DISPLAY")?;
    match StateFile::create(
        Path::new(&runtime),
        &display.to_string_lossy(),
        std::process::id(),
    ) {
        Ok(state) => Some(state),
        Err(err) => {
            // Only the author's pid lookup is lost (F31).
            tracing::warn!(%err, "cannot write the pid file");
            None
        }
    }
}

fn log_bundle(loaded: &Loaded) {
    match loaded {
        Loaded::NoBundle => tracing::info!("no backdrop bundle on stdin"),
        Loaded::Unreadable => tracing::warn!("the backdrop bundle could not be read"),
        Loaded::Decoded { status, .. } => match status {
            Status::Complete => tracing::info!("backdrop bundle read"),
            other => tracing::warn!(status = ?other, "backdrop bundle not fully usable"),
        },
    }
}

/// Everything after the hardening, whose outcome is a parameter so a refusal
/// of it (a tracer, a failed syscall) is tested without a traced process.
fn prepare_and_run_with(
    launch: Launch,
    hardened: Result<(), hardening::HardeningError>,
) -> Result<LockExit, LockExit> {
    if let Err(err) = hardened {
        tracing::error!(%err, "hardening failed: not locking");
        return Err(LockExit::SetupFailed);
    }
    let service = if launch.rehearsal {
        Service::Rehearsal
    } else {
        Service::Production
    };
    let (verifier, _auto_type_after) = pin_verifier(launch.verifier, service)?;

    // The author's pid file: best effort, never read, never a decision. Held
    // until the process is done.
    let _state = state_file();

    let loaded = backdrop::read_header(std::io::stdin().as_fd(), backdrop::READ_DEADLINE);
    log_bundle(&loaded);
    let header = loaded.header();

    let service_missing =
        service_check(service, &existing(service)) == Err(Verdict::ServiceMissing);
    if service_missing {
        tracing::warn!("the PAM service file is missing: every attempt will be denied");
    }

    // Not started here: the writer thread must be created after the loop has
    // blocked the signals the lock ignores (see `Reporter`).
    let reporter = Reporter::new(std::io::stdout());
    Ok(run(Run {
        config: Config::new(launch.rehearsal, service_missing, header.prior_failures),
        attempts: Attempts::new(verifier, Timings::default()),
        presenter: CpuPresenter::new(),
        reporter,
        connect: launch.connect,
        #[cfg(feature = "fake-verifier")]
        auto_type_after: _auto_type_after,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Capture {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn t_h2_a_refused_hardening_exits_15_before_anything_is_asked_of_the_compositor() {
        let launch = Launch {
            rehearsal: true,
            verifier: VerifierChoice::Production(PathBuf::from("/usr/bin/true")),
            connect: ConnectPolicy::VerifiedNest,
        };
        let result = prepare_and_run_with(launch, Err(hardening::HardeningError::Traced(4242)));
        assert_eq!(result, Err(LockExit::SetupFailed));
        assert_eq!(LockExit::SetupFailed.code(), 15);
    }

    #[test]
    fn a_verifier_that_cannot_be_pinned_exits_15_too() {
        for path in [
            "/nonexistent/celestina-lock-verify",
            "/etc/hostname",
            "/usr",
        ] {
            let launch = Launch {
                rehearsal: true,
                verifier: VerifierChoice::Production(PathBuf::from(path)),
                connect: ConnectPolicy::VerifiedNest,
            };
            assert_eq!(
                prepare_and_run_with(launch, Ok(())),
                Err(LockExit::SetupFailed),
                "{path}"
            );
        }
    }

    #[test]
    fn the_verifier_is_looked_for_beside_the_running_executable() {
        let path = verifier_beside_self().expect("current exe");
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("celestina-lock-verify")
        );
        assert_eq!(
            path.parent(),
            std::env::current_exe().expect("exe").parent()
        );
    }

    #[test]
    fn the_subscriber_filter_is_fixed_at_info() {
        // The environment cannot raise it: `RUST_LOG` is not consulted.
        std::env::set_var("RUST_LOG", "trace");
        let capture = Capture::default();
        let writer = capture.clone();
        let subscriber = logger_to(move || writer.clone());
        tracing::subscriber::with_default(subscriber, || {
            tracing::trace!("a-trace-line");
            tracing::debug!("a-debug-line");
            tracing::info!("an-info-line");
            tracing::warn!("a-warn-line");
        });
        std::env::remove_var("RUST_LOG");
        let text = String::from_utf8(capture.0.lock().unwrap_or_else(|e| e.into_inner()).clone())
            .expect("utf-8");
        assert!(!text.contains("a-trace-line"), "{text}");
        assert!(!text.contains("a-debug-line"), "{text}");
        assert!(text.contains("an-info-line"), "{text}");
        assert!(text.contains("a-warn-line"), "{text}");
    }
}
