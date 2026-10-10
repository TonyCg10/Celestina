//! Runs a tool as a child with a deadline: its output is drained on threads
//! of its own so a full pipe never stalls it, its input (when given) is
//! written on another, and a child still running at the deadline is killed.
//!
//! A tool that forks a server inheriting its output (`wl-copy` keeps the
//! clipboard that way) runs through [`run_quiet`], whose child has no output
//! pipes at all. For any other child that forks anyway, the runner waits for
//! the drains only briefly once the child has exited.

use std::ffi::OsString;
use std::fmt;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long the drains may take after the child exited.
const DRAIN_GRACE: Duration = Duration::from_millis(250);

/// What a successful run printed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Why a run failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunError {
    /// The argv was empty.
    NoProgram,
    /// The program is not installed (or not in the stub folder).
    Missing(String),
    /// The program could not be started for another reason.
    Spawn(String),
    /// Still running at the deadline; killed.
    Deadline(String),
    /// It ended unsuccessfully: its exit code (`None` for a signal) and the
    /// last line it wrote on stderr.
    Failed {
        program: String,
        code: Option<i32>,
        stderr: String,
    },
}

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoProgram => formatter.write_str("empty argv"),
            Self::Missing(program) => write!(formatter, "{program}: not installed"),
            Self::Spawn(detail) => write!(formatter, "cannot start: {detail}"),
            Self::Deadline(program) => write!(formatter, "{program}: killed at the deadline"),
            Self::Failed {
                program,
                code,
                stderr,
            } => match code {
                Some(code) => write!(formatter, "{program}: exit {code}: {stderr}"),
                None => write!(formatter, "{program}: killed by a signal: {stderr}"),
            },
        }
    }
}

impl std::error::Error for RunError {}

/// Runs `argv` (the program first), feeding `stdin` when given, and kills it
/// at `deadline`.
///
/// # Errors
///
/// [`RunError`] for a missing program, a failed start, the deadline or an
/// unsuccessful exit.
pub fn run(
    argv: &[OsString],
    stdin: Option<&[u8]>,
    deadline: Duration,
) -> Result<Output, RunError> {
    run_with(argv, stdin, deadline, true)
}

/// [`run`] for a child whose standard output and error go nowhere: a forked
/// server it leaves behind inherits no pipe of this process. A failure
/// carries no stderr line.
///
/// # Errors
///
/// As [`run`].
pub fn run_quiet(
    argv: &[OsString],
    stdin: Option<&[u8]>,
    deadline: Duration,
) -> Result<(), RunError> {
    run_with(argv, stdin, deadline, false).map(|_| ())
}

fn run_with(
    argv: &[OsString],
    stdin: Option<&[u8]>,
    deadline: Duration,
    piped: bool,
) -> Result<Output, RunError> {
    let (program, args) = argv.split_first().ok_or(RunError::NoProgram)?;
    let name = program.to_string_lossy().into_owned();
    let output = || if piped { Stdio::piped() } else { Stdio::null() };
    let mut child = Command::new(program)
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(output())
        .stderr(output())
        .spawn()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => RunError::Missing(name.clone()),
            _ => RunError::Spawn(format!("{name}: {error}")),
        })?;

    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        let bytes = bytes.to_vec();
        // A child that never reads must not hold this thread past the
        // deadline: the write ends when the child is killed.
        std::thread::spawn(move || {
            let _ = pipe.write_all(&bytes);
        });
    }
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RunError::Deadline(name));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RunError::Spawn(format!("{name}: {error}")));
            }
        }
    };
    let stdout = stdout.recv_timeout(DRAIN_GRACE).unwrap_or_default();
    let stderr = stderr.recv_timeout(DRAIN_GRACE).unwrap_or_default();
    if status.success() {
        return Ok(Output { stdout, stderr });
    }
    let last = String::from_utf8_lossy(&stderr)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .to_owned();
    Err(RunError::Failed {
        program: name,
        code: status.code(),
        stderr: last,
    })
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> mpsc::Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        let _ = sender.send(bytes);
    });
    receiver
}
