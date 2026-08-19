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
//!   the run: the tool writes the tree itself. What is kept is the boundary —
//!   the tool writes into an empty staging folder, its result is checked for
//!   escaping symlinks before anything is promoted, and a failure removes the
//!   staging whole.
//! - Progress arrives as one step per finished archive rather than per member,
//!   because the tools do not report bytes in a machine-readable way.
//!
//! The process is spawned directly with an argument vector — never a shell —
//! so a file name holding quotes, `$(…)` or a newline is data, and `--` closes
//! the option list so a name starting with `-` cannot become a switch. Its
//! stdin is `/dev/null`: these tools prompt for a password on a terminal, and a
//! prompt nobody can answer would hang the operation instead of failing it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use celestina_core::CancellationToken;
use siderita_ops::OpError;

use crate::error::ArchiveError;
use crate::format::Format;

/// How often the wait loop looks at the cancellation token while the tool runs.
const POLL: std::time::Duration = std::time::Duration::from_millis(50);

/// A decoder found on this machine, and the dialect its arguments follow.
pub(crate) struct Tool {
    program: PathBuf,
    dialect: Dialect,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Dialect {
    /// `7z`, `7za`, `7zz`: `x -y -o<dir> -p<password> -- <archive>`.
    SevenZip,
    /// `unrar`: `x -y -o+ -p<password> -- <archive> <dir>/`.
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
        candidates.iter().find_map(|(name, dialect)| {
            Some(Self {
                program: in_path(name)?,
                dialect: *dialect,
            })
        })
    }

    /// Extracts `archive` into the empty `staging` folder, reporting each member
    /// as the tool finishes it.
    ///
    /// `password` is passed as one argument; `None` means "do not ask", which
    /// both dialects understand and which turns an encrypted archive into a
    /// clean [`ArchiveError::PasswordRequired`] instead of a hung prompt.
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
        let mut command = Command::new(&self.program);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match self.dialect {
            Dialect::SevenZip => {
                // `-bb1` names each member on stdout as it is written; `-bd`
                // drops the progress indicator, which is a redrawn line rather
                // than output.
                command.arg("x").arg("-y").arg("-bd").arg("-bb1");
                command.arg(password_argument("-p", password));
                command.arg(destination_argument("-o", staging));
                command.arg("--").arg(archive);
            }
            Dialect::Unrar => {
                command.arg("x").arg("-y").arg("-o+");
                command.arg(password_argument("-p", password));
                command.arg("--").arg(archive);
                // unrar takes the destination as a trailing path, and only
                // treats it as a folder when it ends in a separator.
                let mut into = staging.to_path_buf().into_os_string();
                into.push("/");
                command.arg(into);
            }
        }

        let mut child = command
            .spawn()
            .map_err(|error| OpError::io(&self.program, &error))?;
        // Both pipes are drained by their own threads: a tool that fills one it
        // is not being read from stops writing and never exits, and this loop
        // would then wait forever on a process waiting on us.
        let lines = drain_lines(child.stdout.take());
        let complaint = drain_to_end(child.stderr.take());

        let mut said = String::new();
        // The member the tool is writing right now, as read from the line it has
        // not finished yet.
        let mut writing: Option<PathBuf> = None;
        let status = loop {
            while let Ok(chunk) = lines.try_recv() {
                match chunk {
                    Chunk::Line(line) => {
                        if let Some(member) = self.dialect.member_of(&line) {
                            let path = staging.join(member);
                            observe(&path, true);
                            writing = None;
                        }
                        said.push_str(&line);
                        said.push('\n');
                    }
                    Chunk::Partial(text) => {
                        writing = self
                            .dialect
                            .member_of(&text)
                            .map(|member| staging.join(member));
                    }
                }
            }
            // While one member is being written, its size on disk is the only
            // progress there is.
            if let Some(path) = writing.as_deref() {
                observe(path, false);
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if cancellation.is_cancelled() {
                        // The staging folder is removed by the caller, so a
                        // killed run leaves nothing half-extracted behind.
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(OpError::Cancelled.into());
                    }
                    std::thread::sleep(POLL);
                }
                Err(error) => return Err(OpError::io(&self.program, &error).into()),
            }
        };
        // Whatever the tool wrote between the last poll and its exit.
        while let Ok(chunk) = lines.recv() {
            if let Chunk::Line(line) = chunk {
                if let Some(member) = self.dialect.member_of(&line) {
                    observe(&staging.join(member), true);
                }
                said.push_str(&line);
                said.push('\n');
            }
        }
        if status.success() {
            return Ok(());
        }
        said.push_str(&complaint.join().unwrap_or_default());
        Err(self.failure(archive, password, &said))
    }

    /// How many bytes the archive holds once extracted, as the tool's own
    /// listing reports it, or `None` when it cannot be read.
    ///
    /// One extra pass, and a cheap one: listing reads headers, never member
    /// data. It is what turns a ring that merely turns into one that fills, and
    /// a byte count into "so much of so much". Failure is not an error — the
    /// extraction simply goes back to reporting progress without a total.
    pub(crate) fn total_bytes(&self, archive: &Path, password: Option<&str>) -> Option<u64> {
        let mut command = Command::new(&self.program);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        match self.dialect {
            Dialect::SevenZip => {
                command.arg("l");
                command.arg(password_argument("-p", password));
                command.arg("--").arg(archive);
            }
            Dialect::Unrar => {
                command.arg("l");
                command.arg(password_argument("-p", password));
                command.arg("--").arg(archive);
            }
        }
        let output = command.output().ok()?;
        if !output.status.success() {
            return None;
        }
        summary_total(&String::from_utf8_lossy(&output.stdout))
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

/// What the reader thread hands back: a finished line, or the line so far.
///
/// The unfinished one matters because unrar writes `Extracting  <name>`, then
/// redraws a percentage over it with backspaces, and only ends the line when the
/// member is complete. Waiting for the end means saying nothing for as long as
/// that member takes.
enum Chunk {
    Line(String),
    Partial(String),
}

/// Reads a pipe on its own thread, so the caller can keep polling cancellation
/// instead of blocking on a read that may never return.
///
/// Lines are split on both `\n` and `\r`; whatever sits in the buffer between
/// two of them is sent as a partial.
fn drain_lines(pipe: Option<std::process::ChildStdout>) -> std::sync::mpsc::Receiver<Chunk> {
    let (sender, receiver) = std::sync::mpsc::channel();
    if let Some(mut pipe) = pipe {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut buffer = [0u8; 4096];
            let mut line = Vec::new();
            while let Ok(read) = pipe.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                for byte in &buffer[..read] {
                    if *byte == b'\n' || *byte == b'\r' {
                        if !line.is_empty() {
                            let text = String::from_utf8_lossy(&line).into_owned();
                            if sender.send(Chunk::Line(text)).is_err() {
                                return;
                            }
                            line.clear();
                        }
                    } else {
                        line.push(*byte);
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
                let _ = sender.send(Chunk::Line(String::from_utf8_lossy(&line).into_owned()));
            }
        });
    }
    receiver
}

/// Drains a pipe to its end on its own thread, for the complaint a failing tool
/// writes to stderr.
fn drain_to_end(pipe: Option<std::process::ChildStderr>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut pipe) = pipe {
            use std::io::Read;
            let _ = pipe.read_to_string(&mut text);
        }
        text
    })
}

/// The `-p` argument, built as a single `OsString` so a password holding spaces
/// or quotes never becomes two arguments and never reaches a shell.
///
/// With no password, `-p-` tells both tools not to prompt.
fn password_argument(flag: &str, password: Option<&str>) -> OsString {
    let mut argument = OsString::from(flag);
    match password {
        Some(secret) => argument.push(secret),
        None => argument.push("-"),
    }
    argument
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

/// Looks `name` up in `PATH`, answering the executable's full path.
///
/// Resolved here rather than trusting the process's own lookup so that "is this
/// format available at all" is a question with an answer *before* a person picks
/// the verb, and so the executable that will run is the one that was found.
fn in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| is_executable(candidate))
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

/// Every entry under `root`, checked for a symlink that leaves it.
///
/// A delegated tool wrote this tree, so the member-by-member guard did not run
/// on it. This is that guard applied afterwards: a link pointing out of the
/// extraction is refused and the whole result is discarded, which is the same
/// answer a native extraction gives.
pub(crate) fn no_symlink_escapes(root: &Path) -> Result<(), ArchiveError> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries =
            std::fs::read_dir(&directory).map_err(|error| OpError::io(&directory, &error))?;
        for entry in entries {
            let entry = entry.map_err(|error| OpError::io(&directory, &error))?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .map_err(|error| OpError::io(&path, &error))?;
            if kind.is_symlink() {
                let target =
                    std::fs::read_link(&path).map_err(|error| OpError::io(&path, &error))?;
                // The guard reads the link's place *inside the archive*, which
                // is exactly its path relative to the staging root.
                let inside = path.strip_prefix(root).unwrap_or(&path);
                if !crate::member::target_stays_inside(inside, &target) {
                    return Err(ArchiveError::UnsafeMember {
                        name: display_relative(root, &path),
                    });
                }
            } else if kind.is_dir() {
                pending.push(path);
            }
        }
    }
    Ok(())
}

/// The offending entry's name as a person sees it in the archive.
fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
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
    use super::{password_argument, Dialect, Tool};
    use crate::format::Format;
    use std::ffi::OsStr;

    #[test]
    fn a_password_is_one_argument_however_it_is_spelled() {
        // A space, a quote and a `$(…)` in one password: still one argument,
        // and never text a shell gets to interpret.
        let argument = password_argument("-p", Some("cla ve \"$(rm -rf /)\""));
        assert_eq!(argument, OsStr::new("-pcla ve \"$(rm -rf /)\""));
        assert_eq!(password_argument("-p", None), OsStr::new("-p-"));
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

    #[test]
    fn a_native_format_never_looks_for_a_tool() {
        for format in [Format::Zip, Format::Tar, Format::TarGz] {
            assert!(Tool::for_format(format).is_none());
        }
    }
}
