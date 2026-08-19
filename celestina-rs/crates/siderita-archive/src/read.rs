//! Reading an archive's index: what is inside, without writing anything.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::error::ArchiveError;
use crate::format::{sniff, Format};
use crate::member::{safe_relative, Member};

/// Lists what `archive` holds, in stored order.
///
/// Every member is validated on the way out, so a caller never sees a name that
/// would be written outside the extraction root: a stored `../../etc/passwd`
/// fails the whole listing with [`ArchiveError::UnsafeMember`] rather than being
/// silently renamed into something harmless-looking.
///
/// Only the containers this domain decodes itself answer an index. A RAR or 7z
/// is read by an installed tool, which is asked to *extract*, not to describe:
/// its index would have to be parsed out of human-facing output, and in a 7z
/// with encrypted headers there is no index to read without the password. Such
/// Such an archive answers [`ArchiveError::UnsupportedFormat`] here — this verb
/// cannot describe it — while [`crate::extract`] handles it in full.
pub fn list(archive: &Path) -> Result<Vec<Member>, ArchiveError> {
    match sniff(archive).ok_or_else(|| ArchiveError::UnsupportedFormat {
        path: archive.to_path_buf(),
    })? {
        Format::Zip => list_zip(archive),
        Format::Tar => list_tar(archive, false),
        Format::TarGz => list_tar(archive, true),
        Format::Rar | Format::SevenZip => Err(ArchiveError::UnsupportedFormat {
            path: archive.to_path_buf(),
        }),
    }
}

fn open(archive: &Path) -> Result<BufReader<File>, ArchiveError> {
    let file = File::open(archive).map_err(|error| siderita_ops::OpError::io(archive, &error))?;
    Ok(BufReader::new(file))
}

fn list_zip(archive: &Path) -> Result<Vec<Member>, ArchiveError> {
    let mut zip = zip::ZipArchive::new(open(archive)?)
        .map_err(|error| ArchiveError::malformed(archive, error))?;
    let mut members = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let entry = zip
            .by_index(index)
            .map_err(|error| ArchiveError::malformed(archive, error))?;
        // The *stored* name, byte for byte. Not `mangled_name`, which silently
        // rewrites `../fuera.txt` into `fuera.txt`: a member that tried to
        // escape must be reported as such, never quietly renamed into one that
        // looks honest.
        let raw = crate::tarname::path_from_bytes(entry.name_raw());
        let name = safe_relative(&raw).ok_or_else(|| ArchiveError::UnsafeMember {
            name: raw.display().to_string(),
        })?;
        members.push(Member {
            name,
            size: entry.size(),
            is_directory: entry.is_dir(),
        });
    }
    Ok(members)
}

fn list_tar(archive: &Path, gzip: bool) -> Result<Vec<Member>, ArchiveError> {
    let reader = open(archive)?;
    if gzip {
        collect_tar(archive, flate2::read::GzDecoder::new(reader))
    } else {
        collect_tar(archive, reader)
    }
}

/// Walks a tar stream once, validating every stored name.
fn collect_tar<R: std::io::Read>(archive: &Path, reader: R) -> Result<Vec<Member>, ArchiveError> {
    let mut tar = tar::Archive::new(reader);
    let entries = tar
        .entries()
        .map_err(|error| ArchiveError::malformed(archive, error))?;
    let mut members = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| ArchiveError::malformed(archive, error))?;
        let raw = crate::tarname::path_of(&entry)?;
        let name = safe_relative(&raw).ok_or_else(|| ArchiveError::UnsafeMember {
            name: raw.display().to_string(),
        })?;
        members.push(Member {
            name,
            size: entry.header().size().unwrap_or(0),
            is_directory: entry.header().entry_type().is_dir(),
        });
    }
    Ok(members)
}
