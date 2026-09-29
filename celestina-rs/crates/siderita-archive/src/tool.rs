//! Extraction delegated to a tool the machine already has.
//!
//! RAR and 7z are the two containers a desktop keeps meeting that this domain
//! cannot decode itself. RAR is proprietary and its only decoder is published
//! under a licence that a GPL program may not link; 7z has no mature pure-Rust
//! reader. Both, however, are handled by `7z`, `7za`, `7zz` or `unrar`, which
//! desktops already install — so they are *delegated*, exactly as the desktop's
//! own archive managers do, and never linked.
//!
//! What that costs, stated rather than hidden:
//!
//! - The format is offered **only** when the tool is present. Nothing is added
//!   to the package's dependencies, and a machine without it simply does not see
//!   the verb (see [`Tool::for_format`]).
//! - The member-by-member guarantees of a native extraction do not apply during
//!   the run: the tool writes the tree itself. What is kept is the boundary.
//!   Before the tool may write, its own listing of the archive is checked
//!   ([`crate::listing`]): an absolute or `..` name, an escaping link target or
//!   a member under a link refuses the archive with nothing written. The tool
//!   then writes into the empty folder this extraction just created, and its
//!   result is checked again before it is kept (every symlink resolved on the
//!   real tree, every multiply-linked file counted, see
//!   [`crate::contain::Root::verify_tree`]); a failure removes that folder
//!   whole.
//! - Progress arrives as one step per finished archive rather than per member,
//!   because the tools do not report bytes in a machine-readable way.
//!
//! The process is spawned directly with an argument vector — never a shell —
//! so a file name holding quotes, `$(…)` or a newline is data, and `--` closes
//! the option list so a name starting with `-` cannot become a switch. The
//! executable is the canonical path found in an absolute `PATH` folder, so a
//! relative entry cannot make a downloaded `7z` in the current folder the one
//! that runs.
//!
//! A password never appears among the arguments, where every local user can
//! read it in `/proc/<pid>/cmdline` for as long as the tool runs. It is written
//! to the tool's standard input, one line, which is where both tools read the
//! answer to their own password prompt when they have no terminal. Without a
//! password, stdin is `/dev/null` and `-p-` tells the tool not to ask, so a
//! prompt nobody can answer fails the operation instead of hanging it.
//!
//! "When they have no terminal" is made true rather than assumed: when
//! `setsid` is installed the tool runs through it, in a new session with no
//! controlling terminal, so a prompt cannot open `/dev/tty` and wait there even
//! when Siderita itself was started from a terminal. Stopping such a run stops
//! the tool's whole process group (see [`Session`]). Without `setsid` that one
//! case can still prompt on the terminal, and only the tool itself is stopped;
//! every run is still cancellable, and the measurement also has a deadline.
//!
//! What the tool prints is read as it comes and kept only as a bounded tail:
//! enough to classify its complaint, never the whole output of a run that
//! names a million members.

use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use celestina_core::CancellationToken;
use siderita_ops::OpError;

use crate::error::ArchiveError;
use crate::format::Format;
use crate::listing::{Listing, Style};

/// How often the wait loop looks at the cancellation token while the tool runs.
const POLL: Duration = Duration::from_millis(50);

/// How long the measurement may take. It reads headers only and is a nicety,
/// so a tool that has not answered by then is stopped and the extraction goes
/// on without a total.
const MEASURE_DEADLINE: Duration = Duration::from_secs(60);

/// How much of a tool's output is kept: its last bytes, where both tools print
/// their verdict and a listing prints its totals.
const KEPT: usize = 64 * 1024;

/// The longest line read whole. A member name is a path, and no path is longer
/// than `PATH_MAX`; a longer line is cut rather than grown without end.
const LINE_MAX: usize = 8 * 1024;

/// How many lines may wait for the poll loop before the reader stops reading,
/// and so the tool stops writing: the pipe is the backpressure.
const QUEUED_LINES: usize = 1024;

/// The longest password written to a tool. It is written in one piece into an
/// empty pipe before the tool runs, and a pipe always takes this much without
/// blocking; neither tool accepts a password anywhere near as long.
const PASSWORD_MAX: usize = 4095;

/// A decoder found on this machine, and the dialect its arguments follow.
pub(crate) struct Tool {
    /// The canonical path of the executable that runs.
    program: PathBuf,
    /// The name it was found under, passed as `argv[0]` so a tool that is a
    /// link to a multi-call binary still knows which one it is. Through
    /// `setsid`, `argv[0]` is the canonical path instead.
    name: &'static str,
    dialect: Dialect,
    /// How the tool is detached from any controlling terminal, when this
    /// machine can.
    session: Option<Session>,
}

/// `setsid`, and the `kill` that stops the whole session it starts.
///
/// Through `setsid` the tool leads a new session and its own process group,
/// whose id is its process id. Stopping only that process would leave behind
/// anything it started — a tool that forks, a wrapper script that does not
/// `exec`, a background job — still writing into a folder the extraction is
/// removing, or into one it has already checked and returned. The group is
/// stopped instead, with `kill -KILL -- -<pid>`, whenever the tool ends: on
/// cancel or deadline and when it exits by itself (see [`Tool::exited`]). The
/// standard library can only signal one process without `unsafe`. Without
/// `setsid` the group would be Siderita's own, so it is never signalled.
struct Session {
    setsid: PathBuf,
    kill: Option<PathBuf>,
}

impl Session {
    /// Both programs from the folders of `search`, or `None` without `setsid`.
    fn find(search: &OsStr) -> Option<Self> {
        Some(Self {
            setsid: find_in(search, "setsid")?,
            kill: find_in(search, "kill"),
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Dialect {
    /// `7z`, `7za`, `7zz`: `x -y -o<dir> -- <archive>`.
    SevenZip,
    /// `unrar`: `x -y -o+ -c- -- <archive> <dir>/`.
    Unrar,
}

impl Tool {
    /// The tool that can read `format` on this machine, or `None` when none is
    /// installed — which is what makes the verb disappear rather than fail.
    ///
    /// `unrar` comes first for RAR because it is the format's reference decoder
    /// and handles every RAR version; `7z` reads both containers and stands in
    /// when it is the only one present.
    pub(crate) fn for_format(format: Format) -> Option<Self> {
        let candidates: &[(&str, Dialect)] = match format {
            Format::Rar => &[
                ("unrar", Dialect::Unrar),
                ("7z", Dialect::SevenZip),
                ("7zz", Dialect::SevenZip),
                ("7za", Dialect::SevenZip),
            ],
            Format::SevenZip => &[
                ("7z", Dialect::SevenZip),
                ("7zz", Dialect::SevenZip),
                ("7za", Dialect::SevenZip),
            ],
            // Every other container is decoded here, in Rust.
            Format::Zip | Format::Tar | Format::TarGz => &[],
        };
        let search = std::env::var_os("PATH")?;
        let (name, dialect, program) = candidates
            .iter()
            .find_map(|(name, dialect)| Some((*name, *dialect, find_in(&search, name)?)))?;
        Some(Self {
            program,
            name,
            dialect,
            session: Session::find(&search),
        })
    }

    /// The command that runs this tool, with its stdin set up for `password`:
    /// `/dev/null` without one (the caller adds [`no_prompt`]'s `-p-`), a pipe
    /// that [`Tool::spawn`] writes the password to with one.
    fn command(&self, password: Option<&str>) -> Command {
        let mut command = match &self.session {
            // A child spawned here leads no process group, so `setsid` never
            // has to fork: it calls `setsid()` and execs the tool, and the
            // process that is watched and stopped is the tool itself. No
            // `--wait`, which BusyBox's applet does not take; and `argv[0]` is
            // `setsid`, which is how BusyBox picks the applet from its one
            // canonical binary.
            Some(session) => {
                let mut command = Command::new(&session.setsid);
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    command.arg0("setsid");
                }
                command.arg(&self.program);
                command
            }
            None => {
                let mut command = Command::new(&self.program);
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    command.arg0(self.name);
                }
                command
            }
        };
        match password {
            Some(_) => command.stdin(Stdio::piped()),
            None => command.stdin(Stdio::null()),
        };
        command
    }

    /// Spawns `command` and hands it `password` on its standard input.
    ///
    /// The password goes into the empty pipe in one write, which a pipe always
    /// takes whole ([`PASSWORD_MAX`]), and the pipe is then closed, so a tool
    /// that asks twice reads an end instead of waiting. A tool that exits
    /// without asking simply never reads it; that write error is not one.
    fn spawn(&self, mut command: Command, password: Option<&str>) -> std::io::Result<Child> {
        let mut child = command.spawn()?;
        if let (Some(secret), Some(mut stdin)) = (password, child.stdin.take()) {
            let mut line = Vec::with_capacity(secret.len() + 1);
            line.extend_from_slice(secret.as_bytes());
            line.push(b'\n');
            let _ = stdin.write_all(&line);
        }
        Ok(child)
    }

    /// Extracts `archive` into `staging`, the empty folder this extraction just
    /// created (the destination itself, not a hidden copy), reporting each member
    /// as the tool finishes it.
    ///
    /// `password` travels on the tool's stdin, never on its command line;
    /// `None` means "do not ask", which both dialects understand and which
    /// turns an encrypted archive into a clean
    /// [`ArchiveError::PasswordRequired`] instead of a hung prompt. A password
    /// no line can carry — one holding a line break or a NUL, or longer than
    /// [`PASSWORD_MAX`] — cannot be the one that opens the archive and answers
    /// [`ArchiveError::WrongPassword`] without running the tool.
    ///
    /// `observe` is called with the member being written and whether it is
    /// done. Both tools name what they are writing, but only *finish* the line
    /// once the member is complete — so an archive whose first member is 26 GB
    /// says nothing at all for the first half hour. The name is therefore read
    /// from the unfinished line as soon as it appears, and reported again on
    /// every poll while it is still being written, which is what lets a caller
    /// weigh the file on disk and show a byte count that moves.
    pub(crate) fn extract_into(
        &self,
        archive: &Path,
        staging: &Path,
        password: Option<&str>,
        cancellation: &CancellationToken,
        observe: &mut dyn FnMut(&Path, bool),
    ) -> Result<(), ArchiveError> {
        if password.is_some_and(|secret| !deliverable(secret)) {
            return Err(ArchiveError::WrongPassword {
                path: archive.to_path_buf(),
            });
        }
        let mut command = self.command(password);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        match self.dialect {
            Dialect::SevenZip => {
                // `-bb1` names each member on stdout as it is written; `-bd`
                // drops the progress indicator, which is a redrawn line rather
                // than output.
                command.arg("x").arg("-y").arg("-bd").arg("-bb1");
                command.args(no_prompt(password));
                command.arg(destination_argument("-o", staging));
                command.arg("--").arg(archive);
            }
            Dialect::Unrar => {
                // `-c-`: an archive comment is not printed, so its text can
                // never be read as output.
                command.arg("x").arg("-y").arg("-o+").arg("-c-");
                command.args(no_prompt(password));
                command.arg("--").arg(archive);
                // unrar takes the destination as a trailing path, and only
                // treats it as a folder when it ends in a separator.
                let mut into = staging.to_path_buf().into_os_string();
                into.push("/");
                command.arg(into);
            }
        }

        // The member the tool is writing right now, as read from the line it has
        // not finished yet.
        let mut writing: Option<PathBuf> = None;
        let mut on_event = |event: Option<Chunk>| match event {
            Some(Chunk::Line(line) | Chunk::Long(line)) => {
                if let Some(member) = self.dialect.member_of(&line) {
                    observe(&staging.join(member), true);
                    writing = None;
                }
            }
            Some(Chunk::Partial(text)) => {
                writing = self
                    .dialect
                    .member_of(&text)
                    .map(|member| staging.join(member));
            }
            // While one member is being written, its size on disk is the only
            // progress there is.
            None => {
                if let Some(path) = writing.as_deref() {
                    observe(path, false);
                }
            }
        };
        let (status, said) = self
            .supervise(command, password, cancellation, None, &mut on_event)
            .map_err(|halt| self.halted(archive, halt))?;
        if status.success() {
            return Ok(());
        }
        Err(self.failure(archive, password, &said))
    }

    /// Lists `archive` with the tool and refuses it, before anything is
    /// written, when the listing names a member that could leave the root
    /// (see [`crate::listing`]).
    ///
    /// A listing the tool cannot produce refuses the extraction too: without
    /// it nothing is known about what the tool would write. The reasons are
    /// the extraction's own — a password asked for or refused, a damaged
    /// archive — so the caller's answers stay the same.
    pub(crate) fn check_listing(
        &self,
        archive: &Path,
        password: Option<&str>,
        cancellation: &CancellationToken,
    ) -> Result<(), ArchiveError> {
        if password.is_some_and(|secret| !deliverable(secret)) {
            return Err(ArchiveError::WrongPassword {
                path: archive.to_path_buf(),
            });
        }
        let mut command = self.command(password);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let style = match self.dialect {
            Dialect::SevenZip => {
                // `-slt` prints one `Key = value` block per member, which is
                // the only listing that names links and their targets.
                command.arg("l").arg("-slt");
                Style::SevenZip
            }
            Dialect::Unrar => {
                // `lt` is unrar's technical listing: `Name:`, `Type:`,
                // `Target:` per member.
                command.arg("lt").arg("-c-");
                Style::Unrar
            }
        };
        command.args(no_prompt(password));
        command.arg("--").arg(archive);
        let mut listing = Listing::new(style, archive);
        let mut on_event = |event: Option<Chunk>| match event {
            Some(Chunk::Line(line)) => listing.line(&line),
            Some(Chunk::Long(_)) => listing.cut(),
            Some(Chunk::Partial(_)) | None => {}
        };
        let (status, said) = self
            .supervise(command, password, cancellation, None, &mut on_event)
            .map_err(|halt| self.halted(archive, halt))?;
        if !status.success() {
            return Err(self.failure(archive, password, &said));
        }
        listing.finish()
    }

    /// How many bytes the archive holds once extracted, as the tool's own
    /// listing reports it, or `None` when it cannot be read.
    ///
    /// One extra pass, and a cheap one: listing reads headers, never member
    /// data. It is what turns a ring that merely turns into one that fills, and
    /// a byte count into "so much of so much". Failure is not an error — the
    /// extraction simply goes back to reporting progress without a total. So
    /// is a cancellation, and so is a tool that has not answered within
    /// [`MEASURE_DEADLINE`]: both stop the tool and answer `None`.
    pub(crate) fn total_bytes(
        &self,
        archive: &Path,
        password: Option<&str>,
        cancellation: &CancellationToken,
    ) -> Option<u64> {
        self.total_bytes_within(archive, password, cancellation, MEASURE_DEADLINE)
    }

    fn total_bytes_within(
        &self,
        archive: &Path,
        password: Option<&str>,
        cancellation: &CancellationToken,
        deadline: Duration,
    ) -> Option<u64> {
        if password.is_some_and(|secret| !deliverable(secret)) {
            return None;
        }
        let mut command = self.command(password);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        // Both dialects list with the same word; unrar also leaves the
        // archive comment out.
        command.arg("l");
        if self.dialect == Dialect::Unrar {
            command.arg("-c-");
        }
        command.args(no_prompt(password));
        command.arg("--").arg(archive);
        // Only the tail is kept: the totals are the listing's last line, and a
        // listing of a million members is not held in memory to find it.
        let mut listing = Tail::default();
        let mut on_event = |event: Option<Chunk>| {
            if let Some(Chunk::Line(line) | Chunk::Long(line)) = event {
                listing.push_line(&line);
            }
        };
        let (status, _) = self
            .supervise(
                command,
                password,
                cancellation,
                Some(Instant::now() + deadline),
                &mut on_event,
            )
            .ok()?;
        if !status.success() {
            return None;
        }
        summary_total(&listing.text())
    }

    /// Runs `command` to its end, reading its output as it comes, and stops it
    /// on cancellation or at `deadline`.
    ///
    /// `on_event` receives every piece of standard output (`Some`) and a tick
    /// on every poll (`None`). The answer is the exit status and the bounded
    /// tail of everything the tool said, standard error last.
    ///
    /// Both pipes are drained by their own threads: a tool that fills one it
    /// is not being read from stops writing and never exits, and this loop
    /// would then wait forever on a process waiting on us. A stopped tool is
    /// killed and reaped here; the two reader threads end on their own once its
    /// pipes close, and hold nothing but the pipe.
    fn supervise(
        &self,
        command: Command,
        password: Option<&str>,
        cancellation: &CancellationToken,
        deadline: Option<Instant>,
        on_event: &mut dyn FnMut(Option<Chunk>),
    ) -> Result<(ExitStatus, String), Halt> {
        let mut child = self.spawn(command, password).map_err(Halt::Io)?;
        let (lines, reader) = drain_lines(child.stdout.take());
        let complaint = drain_tail(child.stderr.take());
        let mut said = Tail::default();
        let status = loop {
            // Wait for output for at most one poll instead of sleeping, so a
            // busy pipe is read at full speed; then take at most one queue's
            // worth, so a tool that prints faster than it is read cannot keep
            // this loop from its exit and cancellation checks below, which
            // run on every turn.
            match lines.recv_timeout(POLL) {
                Ok(chunk) => {
                    deliver(chunk, &mut said, on_event);
                    for chunk in lines.try_iter().take(QUEUED_LINES) {
                        deliver(chunk, &mut said, on_event);
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                // The tool closed its output but still runs: nothing to wait
                // on, so the poll's pause is taken here.
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    pause();
                }
            }
            on_event(None);
            match self.exited(&mut child) {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if let Err(halt) = halted_by(cancellation, deadline) {
                        self.stop(&mut child);
                        return Err(halt);
                    }
                }
                Err(error) => {
                    self.stop(&mut child);
                    return Err(Halt::Io(error));
                }
            }
        };
        // Whatever the tool wrote between the last poll and its exit, read
        // until both pipes close. Something the tool left behind outside its
        // group can still hold them open, so the wait keeps honouring the
        // cancellation and the deadline; if either ends it, the reader threads
        // are left to end on their own when the pipes close.
        // Checked on every turn, not only when the pipe is quiet: a
        // descendant that floods it would otherwise never let the check run.
        loop {
            halted_by(cancellation, deadline)?;
            match lines.recv_timeout(POLL) {
                Ok(chunk) => deliver(chunk, &mut said, on_event),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
        let _ = reader.join();
        while !complaint.is_finished() {
            halted_by(cancellation, deadline)?;
            pause();
        }
        said.push(&complaint.join().unwrap_or_default());
        Ok((status, said.text()))
    }

    /// The tool's exit status once it has exited, reaping it; `None` while
    /// it runs.
    ///
    /// A tool that exits can leave children behind — a background job, a
    /// daemonised helper — that go on writing into the folder after the
    /// extraction has checked it, or hold its output open. Under `setsid`
    /// they are in the tool's process group, and the group is stopped as soon
    /// as the tool has exited, before it is reaped: an exited, unreaped tool
    /// is a zombie still holding its process id, so the group id names only
    /// what it left. If the tool is found already reaped — it exited between
    /// the look and `try_wait` — the group is stopped right after: Linux keeps
    /// a process id reserved while a group of that id has members, so the id
    /// still reaches only them.
    fn exited(&self, child: &mut Child) -> std::io::Result<Option<ExitStatus>> {
        if self.session.is_some() && is_zombie(child.id()) {
            self.kill_group(child.id());
            return child.wait().map(Some);
        }
        let status = child.try_wait()?;
        if status.is_some() {
            self.kill_group(child.id());
        }
        Ok(status)
    }

    /// Stops the tool and everything it started, then reaps it.
    ///
    /// The group is signalled before the tool is reaped: until then its id
    /// cannot name any other group.
    fn stop(&self, child: &mut Child) {
        self.kill_group(child.id());
        let _ = child.kill();
        let _ = child.wait();
    }

    /// Sends `SIGKILL` to process group `leader` when the tool runs under
    /// `setsid` (where it leads its own group) and a `kill` program exists.
    /// Never otherwise: the group would then be Siderita's own.
    fn kill_group(&self, leader: u32) {
        let Some(kill) = self
            .session
            .as_ref()
            .and_then(|session| session.kill.as_ref())
        else {
            return;
        };
        let mut command = Command::new(kill);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.arg0("kill");
        }
        let _ = command
            .arg("-KILL")
            .arg("--")
            .arg(format!("-{leader}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    /// A run stopped before the tool finished, in this domain's terms.
    fn halted(&self, archive: &Path, halt: Halt) -> ArchiveError {
        match halt {
            Halt::Cancelled => OpError::Cancelled.into(),
            Halt::Deadline => ArchiveError::malformed(archive, "the tool did not answer in time"),
            Halt::Io(error) => OpError::io(&self.program, &error).into(),
        }
    }

    /// Reads the tool's own complaint and answers in this domain's terms.
    ///
    /// Neither tool separates "this needs a password" from "that password is
    /// wrong" — both come out as *Wrong password* — so the distinction is drawn
    /// from what the caller supplied, which is the only thing a person can act
    /// on anyway: no password yet means we must ask, a password that failed
    /// means we must ask again.
    fn failure(&self, archive: &Path, password: Option<&str>, said: &str) -> ArchiveError {
        let said = said.to_lowercase();
        // Each tool words it differently — 7z says *Wrong password*, unrar says
        // *Incorrect password*, both say *encrypted* when the headers are — so
        // the test is the subject, not the sentence.
        let about_the_password = said.contains("password") || said.contains("encrypted");
        if about_the_password {
            return if password.is_some() {
                ArchiveError::WrongPassword {
                    path: archive.to_path_buf(),
                }
            } else {
                ArchiveError::PasswordRequired {
                    path: archive.to_path_buf(),
                }
            };
        }
        ArchiveError::malformed(archive, first_complaint(&said))
    }
}

impl Dialect {
    /// The member a tool's own line says it has just written, if it says so.
    ///
    /// Read from what each tool prints rather than from a listing pass: with
    /// encrypted headers there is no listing to read without asking for the
    /// password twice, and a second pass over a 40 GB archive is not free.
    fn member_of(self, line: &str) -> Option<PathBuf> {
        // unrar draws its percentage with backspaces *inside* the same line and
        // then appends the outcome, so one finished member is one line no
        // matter how long it took: `Extracting  <path>   \b\b 12%\b\b  OK`.
        let cleaned: String = line.chars().filter(|c| !c.is_control()).collect();
        let trimmed = cleaned.trim_end();
        match self {
            // `Creating` announces a folder, which is not a member anybody is
            // waiting for; only `Extracting` writes bytes.
            Dialect::Unrar => {
                let rest = trimmed.strip_prefix("Extracting")?.trim_start();
                // `Extracting from archive.rar` is the banner, not a member.
                if rest.starts_with("from ") {
                    return None;
                }
                // The tool separates the name from its outcome with a run of
                // spaces. A name that itself holds two spaces in a row would be
                // cut short here; that costs the member's byte count, never its
                // extraction.
                let name = rest.split("  ").next()?.trim_end();
                (!name.is_empty()).then(|| PathBuf::from(name))
            }
            // `-bb1` prints `- path/to/file`, relative to the output folder.
            Dialect::SevenZip => {
                let name = trimmed.strip_prefix("- ")?.trim_end();
                (!name.is_empty()).then(|| PathBuf::from(name))
            }
        }
    }
}

/// Takes one poll's pause that no output ends early: while the tool runs with
/// its output closed, or while the last pipe drains after it exited. Counted
/// in tests, where a pause taken while output kept arriving is the sign of a
/// loop that reads a busy pipe slower than the tool writes it.
fn pause() {
    #[cfg(test)]
    PAUSES.with(|pauses| pauses.set(pauses.get() + 1));
    std::thread::sleep(POLL);
}

#[cfg(test)]
thread_local! {
    /// How many [`pause`]s this thread has taken.
    static PAUSES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// `Err` with the reason when `cancellation` fired or `deadline` passed.
fn halted_by(cancellation: &CancellationToken, deadline: Option<Instant>) -> Result<(), Halt> {
    if cancellation.is_cancelled() {
        return Err(Halt::Cancelled);
    }
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return Err(Halt::Deadline);
    }
    Ok(())
}

/// Whether process `pid` has exited and waits, unreaped, as a zombie. Read
/// from `/proc`; anywhere it cannot be read the answer is `false`, and the
/// caller falls back to reaping first.
fn is_zombie(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            let after = stat.get(stat.rfind(')')? + 1..)?;
            after.split_whitespace().next().map(|state| state == "Z")
        })
        .unwrap_or(false)
}

/// Hands one piece of output to the caller, keeping finished lines in `said`.
fn deliver(chunk: Chunk, said: &mut Tail, on_event: &mut dyn FnMut(Option<Chunk>)) {
    if let Chunk::Line(line) | Chunk::Long(line) = &chunk {
        said.push_line(line);
    }
    on_event(Some(chunk));
}

/// Why a supervised run stopped before the tool exited on its own.
enum Halt {
    Cancelled,
    Deadline,
    Io(std::io::Error),
}

/// What the reader thread hands back: a finished line, a finished line that
/// was cut at [`LINE_MAX`], or the line so far.
///
/// The unfinished one matters because unrar writes `Extracting  <name>`, then
/// redraws a percentage over it with backspaces, and only ends the line when the
/// member is complete. Waiting for the end means saying nothing for as long as
/// that member takes.
enum Chunk {
    Line(String),
    Long(String),
    Partial(String),
}

/// Reads a pipe on its own thread, so the caller can keep polling cancellation
/// instead of blocking on a read that may never return.
///
/// Lines are split on both `\n` and `\r`; whatever sits in the buffer between
/// two of them is sent as a partial. A line is kept up to [`LINE_MAX`] bytes
/// and cut there — and then sent as [`Chunk::Long`], so a reader that must see
/// every byte knows it did not — and at most [`QUEUED_LINES`] wait unread:
/// past that the reader waits, and so does the tool.
fn drain_lines<R: Read + Send + 'static>(
    pipe: Option<R>,
) -> (
    std::sync::mpsc::Receiver<Chunk>,
    std::thread::JoinHandle<()>,
) {
    let (sender, receiver) = std::sync::mpsc::sync_channel(QUEUED_LINES);
    let reader = std::thread::spawn(move || {
        let Some(mut pipe) = pipe else { return };
        let mut buffer = [0u8; 4096];
        let mut line = Vec::new();
        let mut cut = false;
        let finished = |line: &[u8], cut: bool| {
            let text = String::from_utf8_lossy(line).into_owned();
            if cut {
                Chunk::Long(text)
            } else {
                Chunk::Line(text)
            }
        };
        loop {
            let read = match pipe.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => read,
            };
            for byte in &buffer[..read] {
                if *byte == b'\n' || *byte == b'\r' {
                    if !line.is_empty() {
                        if sender.send(finished(&line, cut)).is_err() {
                            return;
                        }
                        line.clear();
                        cut = false;
                    }
                } else if line.len() < LINE_MAX {
                    line.push(*byte);
                } else {
                    cut = true;
                }
            }
            if !line.is_empty() {
                let text = String::from_utf8_lossy(&line).into_owned();
                if sender.send(Chunk::Partial(text)).is_err() {
                    return;
                }
            }
        }
        if !line.is_empty() {
            let _ = sender.send(finished(&line, cut));
        }
    });
    (receiver, reader)
}

/// Drains a pipe to its end on its own thread, keeping only its [`KEPT`] last
/// bytes: the complaint a failing tool writes to stderr, or a listing's totals.
fn drain_tail<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut tail = Tail::default();
        if let Some(mut pipe) = pipe {
            let mut buffer = [0u8; 4096];
            loop {
                match pipe.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => tail.push_bytes(&buffer[..read]),
                }
            }
        }
        tail.text()
    })
}

/// The last [`KEPT`] bytes of a stream, however long the stream runs.
#[derive(Default)]
struct Tail {
    bytes: Vec<u8>,
}

impl Tail {
    fn push_bytes(&mut self, more: &[u8]) {
        self.bytes.extend_from_slice(more);
        // Trimmed in halves rather than on every push, so keeping the tail
        // costs one move per `KEPT` bytes read.
        if self.bytes.len() > 2 * KEPT {
            let cut = self.bytes.len() - KEPT;
            self.bytes.drain(..cut);
        }
    }

    fn push(&mut self, text: &str) {
        self.push_bytes(text.as_bytes());
    }

    fn push_line(&mut self, line: &str) {
        self.push(line);
        self.push("\n");
    }

    fn text(&self) -> String {
        let start = self.bytes.len().saturating_sub(KEPT);
        String::from_utf8_lossy(&self.bytes[start..]).into_owned()
    }
}

/// Whether `password` can travel as one line on a pipe: no line break or NUL
/// inside it, and short enough to be written before the tool runs.
fn deliverable(password: &str) -> bool {
    password.len() <= PASSWORD_MAX && !password.contains(['\n', '\r', '\0'])
}

/// The switch that tells both tools not to prompt, when there is no password
/// to answer a prompt with. With one, there is no switch: the password waits
/// on stdin for the prompt.
fn no_prompt(password: Option<&str>) -> Option<&'static str> {
    password.is_none().then_some("-p-")
}

/// The `-o<dir>` argument, kept as bytes so a destination that is not UTF-8
/// still names the same folder.
fn destination_argument(flag: &str, destination: &Path) -> OsString {
    let mut argument = OsString::from(flag);
    argument.push(destination.as_os_str());
    argument
}

/// The uncompressed total from a listing's summary line.
///
/// Both tools close their listing with a rule of dashes and then one line of
/// totals, and in both the first plain number on that line is the uncompressed
/// size — 7z prefixes it with a date, which carries no bare number of its own
/// once its separators are taken into account. Anything unexpected answers
/// `None` rather than a guess.
fn summary_total(listing: &str) -> Option<u64> {
    let mut lines = listing.lines().rev();
    // Walk back from the end: the summary is the last line after the last rule.
    let mut summary = None;
    for line in lines.by_ref() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_rule(trimmed) {
            break;
        }
        summary = Some(trimmed.to_owned());
    }
    let summary = summary?;
    // Drop the date and time 7z puts first: they are the only numbers on the
    // line that are not sizes, and they always carry their own separators.
    summary
        .split_whitespace()
        .filter(|word| !word.contains('-') && !word.contains(':'))
        .find_map(|word| word.parse::<u64>().ok())
}

/// A line of dashes and spaces, which is how both tools rule off a listing.
fn is_rule(line: &str) -> bool {
    !line.is_empty() && line.chars().all(|c| c == '-' || c == ' ')
}

/// The first line that looks like a diagnosis, for a message a person can read.
fn first_complaint(said: &str) -> String {
    said.lines()
        .map(str::trim)
        .find(|line| {
            line.starts_with("error") || line.contains("can't open") || line.contains("cannot")
        })
        .unwrap_or("the tool could not read it")
        .to_string()
}

/// Looks `name` up in the folders of `search` (a `PATH` value), answering the
/// executable's canonical path.
///
/// Resolved here rather than trusting the process's own lookup so that "is this
/// format available at all" is a question with an answer *before* a person picks
/// the verb, and so the executable that will run is the one that was found.
/// Only absolute folders are searched: an empty or relative `PATH` entry names
/// whatever folder the process happens to be in, which for a file manager may
/// be a folder full of downloads. The answer is canonical, so the file that
/// runs is the one that was checked and not whatever a link points at later.
fn find_in(search: &OsStr, name: &str) -> Option<PathBuf> {
    std::env::split_paths(search)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(name))
        .filter(|candidate| is_executable(candidate))
        .find_map(|candidate| std::fs::canonicalize(candidate).ok())
}

#[cfg(unix)]
fn is_executable(candidate: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(candidate)
        .map(|data| data.is_file() && data.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(candidate: &Path) -> bool {
    candidate.is_file()
}

/// Whether any tool on this machine can read `format`.
pub fn can_read(format: Format) -> bool {
    match format {
        Format::Zip | Format::Tar | Format::TarGz => true,
        Format::Rar | Format::SevenZip => Tool::for_format(format).is_some(),
    }
}

/// The name of the tool that would read `format`, for a message that has to
/// name what is missing.
pub fn reader_name(format: Format) -> Option<&'static str> {
    match format {
        Format::Rar => Some("unrar o 7z"),
        Format::SevenZip => Some("7z"),
        Format::Zip | Format::Tar | Format::TarGz => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{deliverable, drain_lines, no_prompt, Chunk, Dialect, Tail, Tool, KEPT, LINE_MAX};
    use crate::format::Format;

    #[test]
    fn only_a_missing_password_becomes_an_argument() {
        // With a password there is no `-p` switch at all: the secret waits on
        // stdin. Without one, `-p-` stops the tool from asking.
        assert_eq!(no_prompt(Some("pass word \"$(rm -rf /)\"")), None);
        assert_eq!(no_prompt(None), Some("-p-"));
        assert!(deliverable("pass word \"$(rm -rf /)\""));
        assert!(!deliverable("two\nlines"));
        assert!(!deliverable(&"x".repeat(super::PASSWORD_MAX + 1)));
    }

    /// SID-17's second half: however much a tool prints, what is kept is
    /// bounded — a line is cut at `LINE_MAX`, the complaint at its last `KEPT`
    /// bytes.
    #[test]
    fn a_tools_output_is_kept_bounded() {
        let mut flood = vec![b'x'; 1024 * 1024];
        flood.extend_from_slice(b"\n- data/one.txt\n");
        let (lines, reader) = drain_lines(Some(std::io::Cursor::new(flood)));
        let mut finished = Vec::new();
        let mut flagged = Vec::new();
        for chunk in lines.iter() {
            match chunk {
                Chunk::Line(line) => {
                    finished.push(line);
                    flagged.push(false);
                }
                Chunk::Long(line) => {
                    finished.push(line);
                    flagged.push(true);
                }
                Chunk::Partial(_) => {}
            }
        }
        reader.join().expect("reader thread");
        assert_eq!(finished.len(), 2);
        assert_eq!(finished[0].len(), LINE_MAX);
        assert_eq!(finished[1], "- data/one.txt");
        // The cut line says it was cut; the whole one does not.
        assert_eq!(flagged, [true, false]);

        let mut tail = Tail::default();
        for _ in 0..100 {
            tail.push(&"y".repeat(10_000));
        }
        tail.push_line("Wrong password");
        let kept = tail.text();
        assert!(kept.len() <= KEPT);
        assert!(kept.ends_with("Wrong password\n"));
        assert!(tail.bytes.len() <= 2 * KEPT);
    }

    /// Real lines from both tools, backspaces and all: what counts as one
    /// finished member and what is banner, folder or noise.
    #[test]
    fn a_finished_member_is_read_out_of_the_tools_own_line() {
        let unrar = |line: &str| Dialect::Unrar.member_of(line);
        assert_eq!(
            unrar(
                "Extracting  /destino/juego/D3D12/D3D12Core.dll     \u{8}\u{8}  0%\u{8}\u{8}  OK "
            ),
            Some(std::path::PathBuf::from(
                "/destino/juego/D3D12/D3D12Core.dll"
            ))
        );
        assert_eq!(unrar("Extracting from SEXOPHOBIA.rar"), None);
        assert_eq!(
            unrar("Creating    /destino/juego/D3D12                    OK"),
            None
        );
        assert_eq!(
            unrar("UNRAR 7.23 freeware      Copyright (c) 1993-2026"),
            None
        );
        assert_eq!(unrar("All OK"), None);

        let seven = |line: &str| Dialect::SevenZip.member_of(line);
        assert_eq!(
            seven("- datos/uno.txt"),
            Some(std::path::PathBuf::from("datos/uno.txt"))
        );
        assert_eq!(seven("Everything is Ok"), None);
    }

    /// Real summary lines from both tools.
    #[test]
    fn the_total_is_read_from_the_listings_summary() {
        let unrar = "\n Attributes  Size   Date   Name\n----------- ---------- ---------- -----\n                     *   ..A.... 822536  2025-11-11 04:45  bin/x.exe\n                     ----------- ---------- ---------- -----\n           48598271394                    37\n";
        assert_eq!(super::summary_total(unrar), Some(48_598_271_394));

        let seven = "   Date      Time    Attr   Size   Compressed  Name\n                     ------------------- ----- ------------ ------------  ----\n                     2026-08-18 15:46:39 ....A            8               datos/uno.txt\n                     ------------------- ----- ------------ ------------  ----\n                     2026-08-18 15:46:39                 12           16  2 files, 1 folders\n";
        assert_eq!(super::summary_total(seven), Some(12));

        // Nothing that looks like a listing: no guess.
        assert_eq!(super::summary_total("no soy un listado\n"), None);
        assert_eq!(super::summary_total(""), None);
    }

    /// A throwaway folder under the system temp dir, removed on drop.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "siderita-tool-{label}-{}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir(&path).expect("create scratch");
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Writing an executable while another test thread forks can make `exec`
    /// fail with "text file busy"; the fake tools are made and run one at a time.
    static FAKE_TOOLS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A `7z` at `program`, run the way [`Tool::for_format`] would run it:
    /// through `setsid` when this machine has it.
    fn fake(program: std::path::PathBuf) -> Tool {
        let session = std::env::var_os("PATH").and_then(|search| super::Session::find(&search));
        Tool {
            program,
            name: "7z",
            dialect: Dialect::SevenZip,
            session,
        }
    }

    /// Writes `script` as an executable `7z` in `dir`.
    #[cfg(unix)]
    fn script_tool(dir: &std::path::Path, script: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let program = dir.join("7z");
        std::fs::write(&program, script).expect("write fake tool");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
            .expect("make fake tool executable");
        program
    }

    /// The session and terminal fields of a `/proc/<pid>/stat` line.
    fn session_and_terminal(stat: &str) -> (String, String) {
        let after = &stat[stat.rfind(')').expect("comm field") + 1..];
        let fields: Vec<&str> = after.split_whitespace().collect();
        (fields[3].to_owned(), fields[4].to_owned())
    }

    /// Important 3 of the review: a tool that would prompt on a terminal runs
    /// in a session of its own, with no controlling terminal to prompt on.
    #[cfg(unix)]
    #[test]
    fn a_tool_runs_without_a_controlling_terminal() {
        let _serial = FAKE_TOOLS
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let scratch = Scratch::new("session");
        let program = script_tool(
            &scratch.0,
            "#!/bin/sh\ncat /proc/$$/stat > \"$(dirname \"$0\")/stat\"\n",
        );
        let tool = fake(program);
        if tool.session.is_none() {
            eprintln!("skipped: setsid is not installed on this machine");
            return;
        }
        let archive = scratch.0.join("a.7z");
        std::fs::write(&archive, b"7z").expect("write archive");
        tool.total_bytes(&archive, None, &celestina_core::CancellationToken::new());

        let theirs = std::fs::read_to_string(scratch.0.join("stat")).expect("stat recorded");
        let ours = std::fs::read_to_string("/proc/self/stat").expect("own stat");
        let (their_session, their_terminal) = session_and_terminal(&theirs);
        let (our_session, _) = session_and_terminal(&ours);
        assert_ne!(their_session, our_session, "the tool shares our session");
        assert_eq!(their_terminal, "0", "the tool has a controlling terminal");
    }

    /// Whether process `pid` is gone, or only a zombie nobody has reaped yet.
    #[cfg(unix)]
    fn gone(pid: &str) -> bool {
        match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Err(_) => true,
            Ok(stat) => {
                let after = &stat[stat.rfind(')').expect("comm field") + 1..];
                after.split_whitespace().next() == Some("Z")
            }
        }
    }

    /// Round 2 of the review (N1): under `setsid` the tool leads its own
    /// process group, and stopping it stops the whole group. A tool whose
    /// shell runs a child without `exec` leaves no child behind still writing.
    #[cfg(unix)]
    #[test]
    fn stopping_a_tool_stops_every_process_it_started() {
        let _serial = FAKE_TOOLS
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let scratch = Scratch::new("group");
        let tool = fake(script_tool(
            &scratch.0,
            "#!/bin/sh\nsleep 30 &\necho $! > \"$(dirname \"$0\")/child\"\nwait\n",
        ));
        if tool.session.is_none() {
            eprintln!("skipped: setsid is not installed on this machine");
            return;
        }
        let archive = scratch.0.join("a.7z");
        std::fs::write(&archive, b"7z").expect("write archive");
        let record = scratch.0.join("child");

        let token = celestina_core::CancellationToken::new();
        let canceller = {
            let token = token.clone();
            let record = record.clone();
            std::thread::spawn(move || {
                let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while !record.exists() && std::time::Instant::now() < until {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
                token.cancel();
            })
        };
        assert_eq!(tool.total_bytes(&archive, None, &token), None);
        canceller.join().expect("canceller");

        let child = std::fs::read_to_string(&record).expect("child recorded");
        let child = child.trim();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !gone(child) && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let survived = !gone(child);
        if survived {
            // Leave nothing running behind a failed assertion.
            let _ = std::process::Command::new("kill")
                .arg("-KILL")
                .arg(child)
                .status();
        }
        assert!(!survived, "the tool's child {child} is still running");
    }

    /// Round 5 of the review: a busy pipe is read at full speed. A listing of
    /// 400 000 lines once took 20 s, read at most 1024 lines per 50 ms poll,
    /// which would have run a million-member measurement past its deadline.
    /// Round 6: measured by the pauses taken, not by the clock.
    #[cfg(unix)]
    #[test]
    fn a_long_listing_is_read_at_full_speed() {
        let _serial = FAKE_TOOLS
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let scratch = Scratch::new("long-listing");
        let tool = fake(script_tool(
            &scratch.0,
            "#!/bin/sh\nseq 400000\necho ----------\necho '12 2 files'\n",
        ));
        let archive = scratch.0.join("a.7z");
        std::fs::write(&archive, b"7z").expect("write archive");

        super::PAUSES.with(|pauses| pauses.set(0));
        // The measurement's own deadline is not under test here; a starved
        // scheduler can take longer than it to run 400 000 lines.
        let total = tool.total_bytes_within(
            &archive,
            None,
            &celestina_core::CancellationToken::new(),
            std::time::Duration::from_secs(3600),
        );
        let paused = super::PAUSES.with(std::cell::Cell::get);

        assert_eq!(total, Some(12));
        // Counted, not timed, so a starved scheduler cannot fail it. A loop
        // that pauses after every batch of at most `QUEUED_LINES` pauses at
        // least 400 000 / 1024, about 390 times, however fast it runs; one that
        // waits only on output pauses only in the moment between the tool
        // closing its output and exiting, and while the last pipe drains.
        assert!(paused < 20, "{paused} pauses while output kept arriving");
    }

    /// Important 3 of the review: the measurement and the pre-flight listing
    /// stop promptly when cancelled, and the measurement at its deadline, even
    /// when the tool never answers.
    #[cfg(unix)]
    #[test]
    fn a_listing_that_never_answers_is_stopped() {
        let _serial = FAKE_TOOLS
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let scratch = Scratch::new("stuck");
        let tool = fake(script_tool(&scratch.0, "#!/bin/sh\nexec sleep 30\n"));
        let archive = scratch.0.join("a.7z");
        std::fs::write(&archive, b"7z").expect("write archive");

        let token = celestina_core::CancellationToken::new();
        let canceller = {
            let token = token.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(200));
                token.cancel();
            })
        };
        let started = std::time::Instant::now();
        assert_eq!(tool.total_bytes(&archive, None, &token), None);
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        canceller.join().expect("canceller");

        let started = std::time::Instant::now();
        let refused = tool.check_listing(&archive, None, &token);
        assert!(refused.is_err_and(|error| error.is_cancelled()));
        assert!(started.elapsed() < std::time::Duration::from_secs(5));

        let started = std::time::Instant::now();
        let answer = tool.total_bytes_within(
            &archive,
            None,
            &celestina_core::CancellationToken::new(),
            std::time::Duration::from_millis(300),
        );
        assert_eq!(answer, None);
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    /// A stand-in `7z` that records its arguments and whatever arrives on its
    /// standard input beside itself, then exits cleanly.
    #[cfg(unix)]
    fn recording_tool(dir: &std::path::Path) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let program = dir.join("7z");
        std::fs::write(
            &program,
            "#!/bin/sh\nhere=$(dirname \"$0\")\nprintf '%s\\n' \"$@\" >> \"$here/argv\"\ncat >> \"$here/stdin\"\n",
        )
        .expect("write fake tool");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
            .expect("make fake tool executable");
        program
    }

    /// SID-17: a password on the command line is readable by every local user
    /// in `/proc/<pid>/cmdline` for as long as the tool runs. It travels on the
    /// tool's standard input instead, for the extraction and for the listing.
    #[cfg(unix)]
    #[test]
    fn a_password_reaches_the_tool_on_stdin_and_never_on_its_command_line() {
        let _serial = FAKE_TOOLS
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let scratch = Scratch::new("password");
        let program = recording_tool(&scratch.0);
        let staging = scratch.0.join("staging");
        std::fs::create_dir(&staging).expect("mk staging");
        let archive = scratch.0.join("secret.7z");
        std::fs::write(&archive, b"7z").expect("write archive");
        let tool = fake(program);

        tool.extract_into(
            &archive,
            &staging,
            Some("pass word"),
            &celestina_core::CancellationToken::new(),
            &mut |_, _| {},
        )
        .expect("the fake tool succeeds");
        let _ = tool.total_bytes(
            &archive,
            Some("pass word"),
            &celestina_core::CancellationToken::new(),
        );

        let argv = std::fs::read_to_string(scratch.0.join("argv")).expect("argv recorded");
        assert!(!argv.contains("pass word"), "password on argv: {argv}");
        let stdin = std::fs::read_to_string(scratch.0.join("stdin")).expect("stdin recorded");
        assert_eq!(stdin, "pass word\npass word\n");
    }

    /// The executable that runs is found only in absolute folders, and is
    /// named by its canonical path.
    #[cfg(unix)]
    #[test]
    fn a_tool_is_found_only_in_absolute_folders_and_by_its_canonical_path() {
        use std::os::unix::fs::PermissionsExt;
        let _serial = FAKE_TOOLS
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let scratch = Scratch::new("lookup");
        let bin = scratch.0.join("bin");
        std::fs::create_dir(&bin).expect("mk bin");
        let real = bin.join("7zz");
        std::fs::write(&real, "#!/bin/sh\n").expect("write tool");
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o755))
            .expect("chmod tool");
        std::os::unix::fs::symlink("7zz", bin.join("7z")).expect("link tool");

        let search = std::env::join_paths([std::path::Path::new("bin"), &bin]).expect("join PATH");
        let found = super::find_in(&search, "7z").expect("found in the absolute folder");
        assert_eq!(found, std::fs::canonicalize(&real).expect("canonical"));
        assert!(found.is_absolute());

        // A relative entry alone finds nothing, even with a `7z` right there.
        let relative = std::env::join_paths([std::path::Path::new(""), std::path::Path::new(".")])
            .expect("join PATH");
        assert_eq!(super::find_in(&relative, "7z"), None);
    }

    #[test]
    fn a_native_format_never_looks_for_a_tool() {
        for format in [Format::Zip, Format::Tar, Format::TarGz] {
            assert!(Tool::for_format(format).is_none());
        }
    }
}
