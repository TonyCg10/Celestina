//! Creating an archive from entries on disk.
//!
//! Same loss-free rule as every other write verb: the destination must not
//! exist, and a run that fails or is cancelled removes the partial archive
//! instead of leaving a truncated container that opens to half a folder.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;
use siderita_ops::{OpError, Progress};

use crate::error::ArchiveError;
use crate::format::Format;
use crate::stamp::Zone;

/// Bytes read per step, and so the cancellation granularity inside one file.
const CHUNK: usize = 64 * 1024;

/// Compresses `sources` into the exact, non-existent path `destination`.
///
/// Every source keeps the name it has on disk, and a folder keeps its tree, so
/// `~/notas` becomes `notas/…` inside the archive and never an absolute path.
/// Sources may come from different folders; only their names are stored.
///
/// `zone` dates a zip, whose date field carries no zone of its own and is read
/// as local time by every other tool. A tar stores a Unix timestamp and ignores
/// it. Pass [`Utc`](crate::Utc) when there is no zone information to give.
pub fn create(
    sources: &[PathBuf],
    destination: &Path,
    format: Format,
    zone: &dyn Zone,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<(), ArchiveError> {
    if sources.is_empty() {
        return Err(ArchiveError::NothingToCompress);
    }
    if !format.is_writable() {
        return Err(ArchiveError::NotWritable {
            format: format.extension(),
        });
    }
    if cancellation.is_cancelled() {
        return Err(OpError::Cancelled.into());
    }
    if fs::symlink_metadata(destination).is_ok() {
        return Err(OpError::AlreadyExists {
            path: destination.to_path_buf(),
        }
        .into());
    }
    let plan = plan(sources, destination)?;

    let mut state = Packing {
        cancellation,
        progress,
        total: Progress::default(),
    };
    let outcome = match format {
        Format::Zip => write_zip(&plan, destination, zone, &mut state),
        Format::TarGz => write_tar_gz(&plan, destination, &mut state),
        Format::Tar | Format::Rar | Format::SevenZip => Err(ArchiveError::NotWritable {
            format: format.extension(),
        }),
    };
    if outcome.is_err() {
        let _ = fs::remove_file(destination);
    }
    outcome
}

/// One entry that will be stored: where it lives and the name it takes inside.
struct Planned {
    path: PathBuf,
    stored: PathBuf,
    is_directory: bool,
    is_symlink: bool,
}

/// The running state of one compression.
struct Packing<'a> {
    cancellation: &'a CancellationToken,
    progress: &'a mut dyn FnMut(Progress),
    total: Progress,
}

impl Packing<'_> {
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

    fn read(&mut self, bytes: u64) {
        self.total.bytes += bytes;
        (self.progress)(self.total);
    }
}

/// Expands every source into the entries that will be stored, depth-first, with
/// each directory listed before its content so an extractor meets it first.
fn plan(sources: &[PathBuf], destination: &Path) -> Result<Vec<Planned>, ArchiveError> {
    let mut planned = Vec::new();
    for source in sources {
        let Some(name) = source.file_name() else {
            return Err(OpError::SourceMissing {
                path: source.clone(),
            }
            .into());
        };
        walk(source, &PathBuf::from(name), destination, &mut planned)?;
    }
    if planned.is_empty() {
        return Err(ArchiveError::NothingToCompress);
    }
    Ok(planned)
}

fn walk(
    path: &Path,
    stored: &Path,
    destination: &Path,
    planned: &mut Vec<Planned>,
) -> Result<(), ArchiveError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            OpError::SourceMissing {
                path: path.to_path_buf(),
            }
        } else {
            OpError::io(path, &error)
        }
    })?;

    if metadata.is_symlink() {
        planned.push(Planned {
            path: path.to_path_buf(),
            stored: stored.to_path_buf(),
            is_directory: false,
            is_symlink: true,
        });
        return Ok(());
    }
    if metadata.is_file() {
        // The archive being written is not stored inside itself, which is what
        // "compress this whole folder into this folder" would otherwise mean.
        if path == destination {
            return Ok(());
        }
        planned.push(Planned {
            path: path.to_path_buf(),
            stored: stored.to_path_buf(),
            is_directory: false,
            is_symlink: false,
        });
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(OpError::UnsupportedFileType {
            path: path.to_path_buf(),
        }
        .into());
    }

    planned.push(Planned {
        path: path.to_path_buf(),
        stored: stored.to_path_buf(),
        is_directory: true,
        is_symlink: false,
    });
    let mut children: Vec<PathBuf> = fs::read_dir(path)
        .map_err(|error| OpError::io(path, &error))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()
        .map_err(|error| OpError::io(path, &error))?;
    // Stable order, byte-wise: the same folder always produces the same archive.
    children.sort();
    for child in children {
        let Some(name) = child.file_name() else {
            continue;
        };
        walk(&child, &stored.join(name), destination, planned)?;
    }
    Ok(())
}

fn write_zip(
    planned: &[Planned],
    destination: &Path,
    zone: &dyn Zone,
    state: &mut Packing<'_>,
) -> Result<(), ArchiveError> {
    let file = File::create(destination).map_err(|error| OpError::io(destination, &error))?;
    let mut zip = zip::ZipWriter::new(io::BufWriter::new(file));

    for entry in planned {
        state.check()?;
        // A zip stores names as text. A byte name that is not UTF-8 has no
        // faithful spelling here, and mangling it would be a silent rename, so
        // the whole archive is refused and the caller can offer `.tar.gz`.
        let stored = entry
            .stored
            .to_str()
            .ok_or_else(|| ArchiveError::NonUtf8Name {
                name: entry.stored.clone(),
            })?
            .to_string();
        let options = options_for(entry, zone);

        if entry.is_directory {
            zip.add_directory(stored, options)
                .map_err(|error| ArchiveError::malformed(destination, error))?;
            state.finished_item();
            continue;
        }
        if entry.is_symlink {
            let target =
                fs::read_link(&entry.path).map_err(|error| OpError::io(&entry.path, &error))?;
            zip.add_symlink(stored, target.to_string_lossy().into_owned(), options)
                .map_err(|error| ArchiveError::malformed(destination, error))?;
            state.finished_item();
            continue;
        }
        zip.start_file(stored, options)
            .map_err(|error| ArchiveError::malformed(destination, error))?;
        stream(&entry.path, &mut zip, state)?;
        state.finished_item();
    }

    let mut writer = zip
        .finish()
        .map_err(|error| ArchiveError::malformed(destination, error))?;
    writer
        .flush()
        .map_err(|error| OpError::io(destination, &error))?;
    Ok(())
}

/// The stored options for one entry: deflate, the unix mode it already has on
/// disk so an extraction restores an executable as executable, and its
/// modification date so the extracted tree keeps the dates it went in with.
///
/// The date goes in twice, because a zip has two places for it. The MS-DOS
/// field every reader understands has no zone, so it carries local time (see
/// [`crate::stamp`]); the `0x5455` extended-timestamp field carries the exact
/// Unix instant, which is what a modern reader prefers and what makes the date
/// survive being extracted in another zone.
fn options_for(entry: &Planned, zone: &dyn Zone) -> zip::write::FullFileOptions<'static> {
    let mut options =
        zip::write::FullFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let Ok(metadata) = fs::symlink_metadata(&entry.path) else {
        return options;
    };
    let modified = metadata.modified().ok();
    if let Some(stamp) = modified
        .and_then(|time| crate::stamp::local_parts(zone, time))
        .and_then(|(year, month, day, hour, minute, second)| {
            zip::DateTime::from_date_and_time(year, month, day, hour, minute, second).ok()
        })
    {
        options = options.last_modified_time(stamp);
    }
    if let Some(seconds) = modified.and_then(crate::stamp::epoch_seconds) {
        // Flags `0b001`: the modification time is present and is the only one
        // stored. It belongs in the local header, where readers look for it.
        let mut payload = Vec::with_capacity(5);
        payload.push(0b0000_0001u8);
        payload.extend_from_slice(&seconds.to_le_bytes());
        // A rejected extra field only costs the precise timestamp; the MS-DOS
        // date above still dates the entry, so there is nothing to fail over.
        let _ = options.add_extra_data(0x5455, payload.into_boxed_slice(), false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        options = options.unix_permissions(metadata.permissions().mode() & 0o7777);
    }
    options
}

fn write_tar_gz(
    planned: &[Planned],
    destination: &Path,
    state: &mut Packing<'_>,
) -> Result<(), ArchiveError> {
    let file = File::create(destination).map_err(|error| OpError::io(destination, &error))?;
    let encoder =
        flate2::write::GzEncoder::new(io::BufWriter::new(file), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    builder.follow_symlinks(false);

    for entry in planned {
        state.check()?;
        let metadata =
            fs::symlink_metadata(&entry.path).map_err(|error| OpError::io(&entry.path, &error))?;
        let mut header = tar::Header::new_gnu();
        header.set_metadata_in_mode(&metadata, tar::HeaderMode::Complete);
        header.set_size(if entry.is_directory || entry.is_symlink {
            0
        } else {
            metadata.len()
        });

        if entry.is_symlink {
            let target =
                fs::read_link(&entry.path).map_err(|error| OpError::io(&entry.path, &error))?;
            header.set_entry_type(tar::EntryType::Symlink);
            builder
                .append_link(&mut header, &entry.stored, &target)
                .map_err(|error| ArchiveError::malformed(destination, error))?;
            state.finished_item();
            continue;
        }
        if entry.is_directory {
            header.set_entry_type(tar::EntryType::Directory);
            builder
                .append_data(&mut header, &entry.stored, io::empty())
                .map_err(|error| ArchiveError::malformed(destination, error))?;
            state.finished_item();
            continue;
        }
        let file = File::open(&entry.path).map_err(|error| OpError::io(&entry.path, &error))?;
        let mut counted = Counting {
            inner: file,
            state,
            destination,
        };
        builder
            .append_data(&mut header, &entry.stored, &mut counted)
            .map_err(|error| ArchiveError::malformed(destination, error))?;
        state.finished_item();
    }

    let encoder = builder
        .into_inner()
        .map_err(|error| ArchiveError::malformed(destination, error))?;
    let mut writer = encoder
        .finish()
        .map_err(|error| OpError::io(destination, &error))?;
    writer
        .flush()
        .map_err(|error| OpError::io(destination, &error))?;
    Ok(())
}

/// A reader that reports what it has read and honours cancellation, so a long
/// tar member is as interruptible and as visible as a long copy.
struct Counting<'a, 'b> {
    inner: File,
    state: &'a mut Packing<'b>,
    destination: &'a Path,
}

impl Read for Counting<'_, '_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.state.cancellation.is_cancelled() {
            let _ = self.destination;
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "cancelled by the caller",
            ));
        }
        let read = self.inner.read(buffer)?;
        self.state.read(read as u64);
        Ok(read)
    }
}

/// Streams one file into an open zip member, cancellable every chunk.
fn stream<W: Write + io::Seek>(
    path: &Path,
    zip: &mut zip::ZipWriter<W>,
    state: &mut Packing<'_>,
) -> Result<(), ArchiveError> {
    let mut file = File::open(path).map_err(|error| OpError::io(path, &error))?;
    let mut buffer = vec![0u8; CHUNK];
    loop {
        state.check()?;
        let read = match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(OpError::io(path, &error).into()),
        };
        zip.write_all(&buffer[..read])
            .map_err(|error| OpError::io(path, &error))?;
        state.read(read as u64);
    }
    Ok(())
}
