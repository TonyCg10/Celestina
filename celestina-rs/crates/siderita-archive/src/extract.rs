//! Extracting an archive into a folder, loss-free.
//!
//! Three guarantees, the same ones the copy verb holds. Nothing existing is ever
//! overwritten: the extraction lands on a name freed by
//! [`next_available`](siderita_ops::next_available) when the obvious one is
//! taken, and that name is taken atomically before a byte is written. Nothing
//! half-written is left claiming to be complete: a failed or cancelled run
//! removes the folder it created, whole. And nothing is written outside the
//! destination: a member whose stored name or symlink target escapes the root
//! fails the extraction.
//!
//! The folder it writes into is the **visible destination**, not a hidden
//! staging directory. A 40 GB archive takes an hour, and for that hour a person
//! is entitled to watch their files appear under the name they will keep,
//! instead of watching a dotted folder they did not ask for. The only thing
//! deferred to the end is the case where the archive turns out to carry its own
//! single top-level folder: that one is lifted out of the wrapper with two
//! renames, which cost nothing, and the empty wrapper is removed.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;
use siderita_ops::{next_available, NameShape, OpError, Progress};

use crate::error::ArchiveError;
use crate::format::{sniff, Format};
use crate::member::{safe_relative, target_stays_inside};
use crate::stamp::Zone;
use crate::tool::Tool;

/// Bytes written per step, and so the cancellation granularity inside one large
/// member. Matches the copy verb's chunk.
const CHUNK: usize = 64 * 1024;

/// A member that was not written, and why.
///
/// The name and the reason travel apart because the sentence a person reads is
/// the host's to write, in the language the product speaks — this domain states
/// the fact, not the wording.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Skipped {
    /// The member's name as the archive stored it.
    pub name: PathBuf,
    /// Why it stayed out.
    pub reason: SkipReason,
}

/// Why a member was not written.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkipReason {
    /// A hard link, device node, fifo or socket: an archive may carry them, a
    /// loss-free extraction does not invent them.
    UnsupportedKind,
    /// A symlink member whose stored target is missing, so there is nothing to
    /// point at.
    SymlinkWithoutTarget,
}

/// What an extraction produced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Extracted {
    /// The folder (or single promoted entry) now present in the destination.
    pub root: PathBuf,
    /// Members written.
    pub written: u64,
    /// Members the domain refused to write, each with its reason. Reported
    /// rather than hidden: an extraction that skipped something is not the same
    /// as one that did not.
    pub skipped: Vec<Skipped>,
}

/// How an extraction should be carried out: everything a caller may vary, in one
/// place, so adding an answer never changes the shape of every call again.
#[derive(Clone, Copy)]
pub struct ExtractOptions<'a> {
    zone: &'a dyn Zone,
    password: Option<&'a str>,
    marker: &'a str,
}

impl<'a> ExtractOptions<'a> {
    /// The plain case: the caller's time zone and the word that frees a taken
    /// name, no password.
    ///
    /// `marker` is the localized word a person reads in a freed name such as
    /// `folder (extracted)`,
    /// and it belongs to the caller for the same reason the copy verb's does:
    /// the domain owns the collision search, the application owns the wording.
    pub fn new(zone: &'a dyn Zone, marker: &'a str) -> Self {
        Self {
            zone,
            password: None,
            marker,
        }
    }

    /// The same, for an encrypted archive.
    ///
    /// A password is never stored, logged or written into an error by this
    /// domain; it lives exactly as long as the call.
    pub fn with_password(self, password: &'a str) -> Self {
        Self {
            password: Some(password),
            ..self
        }
    }
}

/// How many bytes `archive` holds once extracted, or `None` when that cannot be
/// known without extracting it.
///
/// One cheap pass over the index — headers only, never member data — so a host
/// can show "so much of so much" and fill a ring instead of merely turning one.
/// A container this domain reads answers from its own index; a delegated one
/// asks its tool, which needs the password when the headers are encrypted.
///
/// `None` is a normal answer, not a failure: the extraction then reports
/// progress without a total, exactly as before.
pub fn measure(archive: &Path, options: &ExtractOptions<'_>) -> Option<u64> {
    match sniff(archive)? {
        Format::Zip | Format::Tar | Format::TarGz => Some(
            crate::read::list(archive)
                .ok()?
                .iter()
                .filter(|member| !member.is_directory)
                .map(|member| member.size)
                .sum(),
        ),
        format @ (Format::Rar | Format::SevenZip) => {
            Tool::for_format(format)?.total_bytes(archive, options.password)
        }
    }
}

/// Extracts `archive` into `into_dir` and returns what was created.
///
/// An archive that already carries a single top-level folder is extracted *as*
/// that folder; one that would spill several entries into `into_dir` is given a
/// folder named after the archive, so extracting never scatters files over the
/// folder the person was looking at.
///
/// A zip, tar or tar.gz is decoded here. A RAR or 7z is delegated to an
/// installed tool, and answers [`ArchiveError::ToolMissing`] when there is none
/// — see [`crate::tool`]. An encrypted archive of any of them answers
/// [`ArchiveError::PasswordRequired`] until
/// [`ExtractOptions::with_password`] carries one.
pub fn extract(
    archive: &Path,
    into_dir: &Path,
    options: &ExtractOptions<'_>,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<Extracted, ArchiveError> {
    if cancellation.is_cancelled() {
        return Err(OpError::Cancelled.into());
    }
    let format = sniff(archive).ok_or_else(|| ArchiveError::UnsupportedFormat {
        path: archive.to_path_buf(),
    })?;
    if !into_dir.is_dir() {
        return Err(OpError::SourceMissing {
            path: into_dir.to_path_buf(),
        }
        .into());
    }

    let destination = make_destination(into_dir, archive, options.marker)?;
    let outcome = write_all(
        archive,
        format,
        &destination,
        options,
        cancellation,
        progress,
    );
    match outcome {
        Ok(Written { written, skipped }) => match unwrap_own_folder(&destination, options.marker) {
            Ok(root) => Ok(Extracted {
                root,
                written,
                skipped,
            }),
            Err(error) => {
                let _ = fs::remove_dir_all(&destination);
                Err(error)
            }
        },
        Err(error) => {
            let _ = fs::remove_dir_all(&destination);
            Err(error)
        }
    }
}

/// Creates the folder the extraction writes into: named after the archive, in
/// the folder the person is looking at, and free.
///
/// `create_dir` is what takes the name — atomically, so two extractions started
/// at once cannot both believe they own it, and so nothing that already exists
/// is written into.
fn make_destination(
    into_dir: &Path,
    archive: &Path,
    marker: &str,
) -> Result<PathBuf, ArchiveError> {
    let wanted = archive_stem(archive);
    let mut candidate = into_dir.join(&wanted);
    for attempt in 0u32.. {
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                // Freed by the same rule the copy verb uses, and tried again:
                // between the two calls another writer may have taken it too.
                candidate = next_available(into_dir, &wanted, marker, NameShape::Directory);
                if attempt > 64 {
                    return Err(OpError::AlreadyExists { path: candidate }.into());
                }
            }
            Err(error) => return Err(OpError::io(&candidate, &error).into()),
        }
    }
    unreachable!("the destination-name search always terminates before u32 wraps")
}

/// Lifts the archive's own single top-level folder out of the wrapper.
///
/// An archive that carries `folder/…` inside should extract *as* that folder,
/// not as `folder/folder`. Since the wrapper was created before anything could be
/// read — a 7z with encrypted headers has no index to consult first — the
/// question is answered afterwards, and answered with renames: the wrapper
/// steps aside, its one folder takes the free name, and the empty wrapper goes.
/// A failed step leaves the wrapper exactly as it was, which is still the whole
/// extraction under a correct name.
fn unwrap_own_folder(destination: &Path, marker: &str) -> Result<PathBuf, ArchiveError> {
    let Some(inner) = own_folder(destination)? else {
        return Ok(destination.to_path_buf());
    };
    let Some(into_dir) = destination.parent() else {
        return Ok(destination.to_path_buf());
    };

    // The wrapper is holding the name the inner folder may want, so it steps
    // aside first, under a name nothing else is using.
    let aside = next_available(
        into_dir,
        &wrapper_name(destination),
        marker,
        NameShape::Directory,
    );
    if fs::rename(destination, &aside).is_err() {
        return Ok(destination.to_path_buf());
    }
    let mut wanted = into_dir.join(&inner);
    if fs::symlink_metadata(&wanted).is_ok() {
        wanted = next_available(into_dir, &inner, marker, NameShape::Directory);
    }
    if fs::rename(aside.join(&inner), &wanted).is_err() {
        // Put the wrapper back rather than leave the extraction under a name
        // nobody chose.
        let _ = fs::rename(&aside, destination);
        return Ok(destination.to_path_buf());
    }
    let _ = fs::remove_dir(&aside);
    Ok(wanted)
}

/// The folder the archive brought with it: the single directory the extraction
/// produced, when that is all it produced. A lone file, or several entries, keep
/// the folder made for them.
fn own_folder(destination: &Path) -> Result<Option<std::ffi::OsString>, ArchiveError> {
    let mut entries =
        fs::read_dir(destination).map_err(|error| OpError::io(destination, &error))?;
    let Some(first) = entries.next() else {
        return Ok(None);
    };
    let first = first.map_err(|error| OpError::io(destination, &error))?;
    if entries.next().is_some() {
        return Ok(None);
    }
    let kind = first
        .file_type()
        .map_err(|error| OpError::io(&first.path(), &error))?;
    Ok(kind.is_dir().then(|| first.file_name()))
}

/// A name for the wrapper to step aside to, derived from its own so the
/// intermediate state is still recognisable if anything interrupts it.
fn wrapper_name(destination: &Path) -> std::ffi::OsString {
    destination
        .file_name()
        .map(std::ffi::OsStr::to_os_string)
        .unwrap_or_else(|| std::ffi::OsString::from("archivo"))
}

/// The archive's name with its archive extension removed — `notas.tar.gz` gives
/// `notas`, not `notas.tar`. Byte-wise, so a non-UTF-8 name still names a folder.
fn archive_stem(archive: &Path) -> std::ffi::OsString {
    use std::ffi::OsString;
    let name = archive
        .file_name()
        .map(|name| name.as_encoded_bytes().to_vec())
        .unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    for suffix in [
        b".tar.gz".as_slice(),
        b".tar.gzip".as_slice(),
        b".tgz".as_slice(),
        b".zip".as_slice(),
        b".tar".as_slice(),
        b".rar".as_slice(),
        b".7z".as_slice(),
    ] {
        if lower.ends_with(suffix) && lower.len() > suffix.len() {
            let kept = &name[..name.len() - suffix.len()];
            // Safe: the bytes come from an OsStr and only whole trailing ASCII
            // suffixes were removed, so no multi-byte sequence is split.
            return unsafe_free_os_string(kept);
        }
    }
    if name.is_empty() {
        OsString::from("archivo")
    } else {
        unsafe_free_os_string(&name)
    }
}

/// Rebuilds an `OsString` from bytes without `unsafe`: on unix the conversion is
/// total, elsewhere it is the platform's own lossy rule.
#[cfg(unix)]
fn unsafe_free_os_string(bytes: &[u8]) -> std::ffi::OsString {
    use std::os::unix::ffi::OsStringExt;
    std::ffi::OsString::from_vec(bytes.to_vec())
}

#[cfg(not(unix))]
fn unsafe_free_os_string(bytes: &[u8]) -> std::ffi::OsString {
    std::ffi::OsString::from(String::from_utf8_lossy(bytes).into_owned())
}

/// What the write pass produced: how many members reached disk and which the
/// domain refused.
struct Written {
    written: u64,
    skipped: Vec<Skipped>,
}

/// Writes every member of `archive` under `root`.
fn write_all(
    archive: &Path,
    format: Format,
    root: &Path,
    options: &ExtractOptions<'_>,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<Written, ArchiveError> {
    let mut state = Writing {
        root: root.to_path_buf(),
        cancellation,
        progress,
        total: Progress::default(),
        skipped: Vec::new(),
    };
    match format {
        Format::Zip => write_zip(archive, options, &mut state)?,
        Format::Tar => {
            let reader = io::BufReader::new(
                File::open(archive).map_err(|error| OpError::io(archive, &error))?,
            );
            write_tar(archive, reader, &mut state)?;
        }
        Format::TarGz => {
            let reader = io::BufReader::new(
                File::open(archive).map_err(|error| OpError::io(archive, &error))?,
            );
            write_tar(archive, flate2::read::GzDecoder::new(reader), &mut state)?;
        }
        Format::Rar | Format::SevenZip => {
            let tool = Tool::for_format(format).ok_or(ArchiveError::ToolMissing {
                format: format.extension(),
                tool: crate::tool::reader_name(format).unwrap_or("una herramienta externa"),
            })?;
            // The tool names each member as it writes it, so the shared
            // progress surface counts real entries and real bytes here too,
            // rather than jumping from nothing to everything at the end.
            //
            // A member is weighed while it is still being written, not only when
            // it is finished: an archive whose first member is 26 GB would
            // otherwise report nothing for half an hour. Only the growth since
            // the last look is added, so the total stays the truth.
            {
                let state = &mut state;
                let mut counted: u64 = 0;
                let mut observe = |member: &Path, done: bool| {
                    let size = fs::symlink_metadata(member)
                        .map(|data| data.len())
                        .unwrap_or(0);
                    state.wrote(size.saturating_sub(counted));
                    if done {
                        state.finished_item();
                        counted = 0;
                    } else {
                        counted = size;
                    }
                };
                tool.extract_into(archive, root, options.password, cancellation, &mut observe)?;
            }
            crate::tool::no_symlink_escapes(root)?;
        }
    }
    Ok(Written {
        written: state.total.items,
        skipped: state.skipped,
    })
}

/// The running state of one extraction: where it writes, how it reports and what
/// it refused.
struct Writing<'a> {
    root: PathBuf,
    cancellation: &'a CancellationToken,
    progress: &'a mut dyn FnMut(Progress),
    total: Progress,
    skipped: Vec<Skipped>,
}

impl Writing<'_> {
    fn check(&self) -> Result<(), ArchiveError> {
        if self.cancellation.is_cancelled() {
            return Err(OpError::Cancelled.into());
        }
        Ok(())
    }

    fn finished_item(&mut self) {
        self.total.items += 1;
        (self.progress)(self.total);
    }

    fn wrote(&mut self, bytes: u64) {
        self.total.bytes += bytes;
        (self.progress)(self.total);
    }

    /// The absolute path a validated member name takes, creating its parents.
    fn place(&self, name: &Path) -> Result<PathBuf, ArchiveError> {
        let target = self.root.join(name);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| OpError::io(parent, &error))?;
        }
        Ok(target)
    }
}

fn write_zip(
    archive: &Path,
    options: &ExtractOptions<'_>,
    state: &mut Writing<'_>,
) -> Result<(), ArchiveError> {
    let file = File::open(archive).map_err(|error| OpError::io(archive, &error))?;
    let mut zip = zip::ZipArchive::new(io::BufReader::new(file))
        .map_err(|error| ArchiveError::malformed(archive, error))?;
    for index in 0..zip.len() {
        state.check()?;
        let opened = match options.password {
            Some(password) => zip.by_index_decrypt(index, password.as_bytes()),
            None => zip.by_index(index),
        };
        let mut entry = opened.map_err(|error| encrypted_or_malformed(archive, options, error))?;
        // The stored bytes, never `mangled_name` — see `read::list_zip`.
        let raw = crate::tarname::path_from_bytes(entry.name_raw());
        let name = safe_relative(&raw).ok_or_else(|| ArchiveError::UnsafeMember {
            name: raw.display().to_string(),
        })?;
        let mode = entry.unix_mode();

        if entry.is_dir() {
            let target = state.place(&name)?;
            fs::create_dir_all(&target).map_err(|error| OpError::io(&target, &error))?;
            state.finished_item();
            continue;
        }
        if is_symlink_mode(mode) {
            let mut target = Vec::new();
            entry
                .read_to_end(&mut target)
                .map_err(|error| ArchiveError::malformed(archive, error))?;
            let link_target = crate::tarname::path_from_bytes(&target);
            write_symlink(state, &name, &link_target)?;
            continue;
        }
        let stamp = zip_stamp(options.zone, &entry);
        let target = state.place(&name)?;
        copy_member(state, &mut entry, &target)?;
        apply_mode(&target, mode);
        apply_stamp(&target, stamp);
        state.finished_item();
    }
    Ok(())
}

/// Tells an encrypted member apart from a damaged one.
///
/// The zip reader answers "this needs a password" and "that password is wrong"
/// as two distinct errors; both mean the same thing to a person — we have to
/// ask — so they become this domain's two password variants and everything else
/// stays a malformed container.
fn encrypted_or_malformed(
    archive: &Path,
    options: &ExtractOptions<'_>,
    error: zip::result::ZipError,
) -> ArchiveError {
    let needs_one = matches!(
        &error,
        zip::result::ZipError::UnsupportedArchive(zip::result::ZipError::PASSWORD_REQUIRED)
    );
    let wrong = matches!(&error, zip::result::ZipError::InvalidPassword);
    if wrong || (needs_one && options.password.is_some()) {
        return ArchiveError::WrongPassword {
            path: archive.to_path_buf(),
        };
    }
    if needs_one {
        return ArchiveError::PasswordRequired {
            path: archive.to_path_buf(),
        };
    }
    ArchiveError::malformed(archive, error)
}

/// The member's stored modification date as an instant.
///
/// The `0x5455` extended timestamp is preferred when the writer stored one: it
/// is an exact Unix instant, so it needs no zone and cannot drift. Only when it
/// is absent does the zone-less MS-DOS field apply, read as `zone`'s local time
/// — which is what every writer put there (see [`crate::stamp`]).
fn zip_stamp(zone: &dyn Zone, entry: &zip::read::ZipFile<'_>) -> Option<std::time::SystemTime> {
    let precise = entry.extra_data_fields().find_map(|field| match field {
        zip::extra_fields::ExtraField::ExtendedTimestamp(stamp) => stamp.mod_time(),
        _ => None,
    });
    if let Some(seconds) = precise {
        return crate::stamp::from_epoch_seconds(i64::from(seconds));
    }
    let stored = entry.last_modified()?;
    crate::stamp::instant_from_local(
        zone,
        stored.year(),
        stored.month(),
        stored.day(),
        stored.hour(),
        stored.minute(),
        stored.second(),
    )
}

fn write_tar<R: Read>(
    archive: &Path,
    reader: R,
    state: &mut Writing<'_>,
) -> Result<(), ArchiveError> {
    let mut tar = tar::Archive::new(reader);
    let entries = tar
        .entries()
        .map_err(|error| ArchiveError::malformed(archive, error))?;
    for entry in entries {
        state.check()?;
        let mut entry = entry.map_err(|error| ArchiveError::malformed(archive, error))?;
        let raw = crate::tarname::path_of(&entry)?;
        let name = safe_relative(&raw).ok_or_else(|| ArchiveError::UnsafeMember {
            name: raw.display().to_string(),
        })?;
        let kind = entry.header().entry_type();
        let mode = entry.header().mode().ok().map(|mode| mode & 0o7777);

        if kind.is_dir() {
            let target = state.place(&name)?;
            fs::create_dir_all(&target).map_err(|error| OpError::io(&target, &error))?;
            state.finished_item();
        } else if kind.is_symlink() {
            let Some(link_target) = crate::tarname::link_target_of(&entry) else {
                state.skipped.push(Skipped {
                    name: name.clone(),
                    reason: SkipReason::SymlinkWithoutTarget,
                });
                continue;
            };
            write_symlink(state, &name, &link_target)?;
        } else if kind.is_file() {
            let stamp = entry
                .header()
                .mtime()
                .ok()
                .and_then(|seconds| crate::stamp::from_epoch_seconds(seconds as i64));
            let target = state.place(&name)?;
            copy_member(state, &mut entry, &target)?;
            apply_mode(&target, mode);
            apply_stamp(&target, stamp);
            state.finished_item();
        } else {
            state.skipped.push(Skipped {
                name: name.clone(),
                reason: SkipReason::UnsupportedKind,
            });
        }
    }
    Ok(())
}

/// Creates one symlink, after proving its target cannot leave the root.
fn write_symlink(
    state: &mut Writing<'_>,
    name: &Path,
    link_target: &Path,
) -> Result<(), ArchiveError> {
    if !target_stays_inside(name, link_target) {
        return Err(ArchiveError::UnsafeMember {
            name: name.display().to_string(),
        });
    }
    let target = state.place(name)?;
    symlink(link_target, &target)?;
    state.finished_item();
    Ok(())
}

#[cfg(unix)]
fn symlink(link_target: &Path, at: &Path) -> Result<(), ArchiveError> {
    std::os::unix::fs::symlink(link_target, at).map_err(|error| OpError::io(at, &error).into())
}

#[cfg(not(unix))]
fn symlink(_link_target: &Path, at: &Path) -> Result<(), ArchiveError> {
    Err(OpError::UnsupportedFileType {
        path: at.to_path_buf(),
    }
    .into())
}

/// Streams one member's bytes onto `target`, cancellable every chunk.
fn copy_member<R: Read>(
    state: &mut Writing<'_>,
    source: &mut R,
    target: &Path,
) -> Result<(), ArchiveError> {
    let mut file = File::create(target).map_err(|error| OpError::io(target, &error))?;
    let mut buffer = vec![0u8; CHUNK];
    loop {
        state.check()?;
        let read = match source.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(OpError::io(target, &error).into()),
        };
        file.write_all(&buffer[..read])
            .map_err(|error| OpError::io(target, &error))?;
        state.wrote(read as u64);
    }
    file.flush().map_err(|error| OpError::io(target, &error))?;
    Ok(())
}

/// Applies a stored permission bit set, keeping the owner able to read and write
/// what was just extracted. A missing or nonsensical mode leaves the umask's.
fn apply_mode(target: &Path, mode: Option<u32>) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let Some(mode) = mode else { return };
        let mode = (mode & 0o777) | 0o600;
        let _ = fs::set_permissions(target, fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    {
        let _ = (target, mode);
    }
}

/// Restores a member's modification date onto what was just written.
///
/// Best effort by design: a filesystem that will not take the date is not a
/// reason to fail an extraction whose bytes are already correct. Only files are
/// stamped — a directory's date changes again as its own members are written.
fn apply_stamp(target: &Path, stamp: Option<std::time::SystemTime>) {
    let Some(stamp) = stamp else { return };
    if let Ok(file) = fs::OpenOptions::new().write(true).open(target) {
        let _ = file.set_times(fs::FileTimes::new().set_modified(stamp));
    }
}

/// Whether a zip member's stored unix mode says "symlink".
fn is_symlink_mode(mode: Option<u32>) -> bool {
    mode.is_some_and(|mode| mode & 0o170000 == 0o120000)
}
