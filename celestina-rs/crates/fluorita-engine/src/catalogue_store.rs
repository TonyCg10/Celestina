//! Writing the catalogue down, and reading it back.
//!
//! **Why persist at all.** Not to save the walk: scanning the author's real
//! library takes 251 µs, and re-walking is free next to what a launch already
//! costs. What is expensive is *metadata* — a tag read costs a backend probe of
//! tens of milliseconds, so a music library of two thousand tracks would spend
//! minutes re-learning what it already knew. Persisting also keeps an honest
//! memory of items last seen missing, so a disconnected drive does not silently
//! come back as "available" before anything looked.
//!
//! **Why a file and not a database.** The shape was measured before it was
//! chosen: flat records, no relations, no queries, keyed by device+inode. A
//! whole-file rewrite guarded by the suite's existing atomic replacement is the
//! smallest thing that is correct for that shape, and it adds no dependency.
//! The threshold for revisiting is written down rather than left to taste: once
//! a library is large enough that rewriting the file on every scan is felt, or
//! once something needs to update one record without loading the rest, a real
//! database earns its place. Everything here is behind `load`/`save`, so that
//! change costs this file.
//!
//! **The format.** One header line, then one record per line, tab-separated.
//! Every free-form field — paths above all — is percent-encoded with the
//! suite's canonical codec, which round-trips raw bytes; a filename that is not
//! valid UTF-8 therefore survives a write and a read unchanged, and no field
//! can smuggle a tab or a newline into the next record.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use celestina_core::{atomic_file, percent};
use fluorita_core::{
    Availability, Catalogue, MediaId, MediaKind, MediaMetadata, MediaRecord, SourceId,
    SourceIdentity,
};

use crate::error::{EngineError, EngineResult};

/// The first line of the file. A future format change bumps this, and an
/// unrecognised version is treated as "no catalogue yet" rather than as
/// something to guess at.
const HEADER: &str = "fluorita-catalogue 1";

/// A ceiling on what will be read into memory. At roughly 200 bytes a record
/// this is far past the scanner's own 50 000-file limit, and it stops a
/// corrupted or hostile file from being loaded whole.
const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Fields per record. A line with any other count is skipped.
const FIELDS: usize = 16;

/// What a load found, including what it had to skip.
#[derive(Debug, Default)]
pub struct LoadOutcome {
    pub catalogue: Catalogue,
    /// Lines that could not be read as records. Counted rather than hidden: a
    /// catalogue that silently lost half its entries would look like a scan
    /// problem later.
    pub skipped: usize,
    /// True when there was simply nothing stored yet — a first run.
    pub absent: bool,
}

/// Reads the catalogue at `path`.
///
/// A missing file, an unreadable one and an unrecognised version all mean the
/// same thing to a caller: start empty and let the scan fill it in. None of
/// them is an error, because none of them should stop the app from opening.
pub fn load(path: &Path) -> EngineResult<LoadOutcome> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => {
            return Ok(LoadOutcome {
                absent: true,
                ..LoadOutcome::default()
            })
        }
    };
    if metadata.len() > MAX_BYTES {
        return Err(EngineError::UnusableSource {
            path: path.to_path_buf(),
            reason: "the stored catalogue is larger than the read budget",
        });
    }

    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => {
            return Ok(LoadOutcome {
                absent: true,
                ..LoadOutcome::default()
            })
        }
    };

    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return Ok(LoadOutcome {
            absent: true,
            ..LoadOutcome::default()
        });
    }

    let mut outcome = LoadOutcome::default();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        match parse(line) {
            Some(record) => {
                outcome.catalogue.upsert(record);
            }
            None => outcome.skipped += 1,
        }
    }
    Ok(outcome)
}

/// Writes the whole catalogue, atomically.
///
/// The previous file stays intact until the complete new one is durable, so a
/// crash mid-write costs the newest scan and never the catalogue.
pub fn save(path: &Path, catalogue: &Catalogue) -> EngineResult<()> {
    let mut text = String::with_capacity(catalogue.len() * 160 + HEADER.len() + 1);
    text.push_str(HEADER);
    text.push('\n');
    for record in catalogue.records() {
        text.push_str(&format(record));
        text.push('\n');
    }

    atomic_file::replace(path, text.as_bytes()).map_err(|source| EngineError::Io {
        operation: "write the catalogue",
        path: path.to_path_buf(),
        source,
    })
}

/// Where the catalogue lives: `$XDG_DATA_HOME/fluorita/catalogue.tsv`.
///
/// Data rather than cache, deliberately. It *is* rebuildable, but rebuilding
/// the metadata means decoding every track again, and a cache directory is
/// something a system may clear at will.
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    celestina_core::xdg::data_home().map(|data| data.join("fluorita").join("catalogue.tsv"))
}

fn format(record: &MediaRecord) -> String {
    let (device, inode) = record.id().filesystem_parts().unwrap_or((0, 0));
    let identity_path = record
        .id()
        .path_part()
        .map(|path| percent::encode(&percent::path_bytes(path)))
        .unwrap_or_default();
    let metadata = record.metadata();

    [
        device.to_string(),
        inode.to_string(),
        identity_path,
        record.source().value().to_string(),
        kind_code(record.kind()).to_owned(),
        record.identity().size.to_string(),
        timestamp(record.identity().modified),
        availability_code(record.availability()).to_owned(),
        percent::encode(&percent::path_bytes(record.path())),
        optional_text(metadata.title.as_deref()),
        optional_text(metadata.artist.as_deref()),
        optional_text(metadata.album.as_deref()),
        optional_text(metadata.album_artist.as_deref()),
        optional_number(metadata.track_number),
        optional_number(metadata.disc_number),
        optional_duration(metadata.duration, metadata.year),
    ]
    .join("\t")
}

fn parse(line: &str) -> Option<MediaRecord> {
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() != FIELDS {
        return None;
    }

    let device: u64 = fields[0].parse().ok()?;
    let inode: u64 = fields[1].parse().ok()?;
    let path = decode_path(fields[8])?;
    // A record written from a stat has a real identity; one written from a
    // path keeps the path it was keyed on, so neither turns into the other.
    let id = if fields[2].is_empty() {
        MediaId::filesystem(device, inode)
    } else {
        MediaId::from_path(&decode_path(fields[2])?)
    };

    let source = SourceId::from_value(fields[3].parse().ok()?);
    let kind = kind_of(fields[4])?;
    let size: u64 = fields[5].parse().ok()?;
    let modified = timestamp_of(fields[6])?;
    let availability = availability_of(fields[7])?;

    let (duration, year) = duration_and_year(fields[15]);
    let metadata = MediaMetadata {
        title: text_of(fields[9]),
        artist: text_of(fields[10]),
        album: text_of(fields[11]),
        album_artist: text_of(fields[12]),
        track_number: number_of(fields[13]),
        disc_number: number_of(fields[14]),
        year,
        duration,
    };

    Some(
        MediaRecord::new(id, source, path, kind, SourceIdentity::new(size, modified))
            .with_metadata(metadata)
            .with_availability(availability),
    )
}

const fn kind_code(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Image => "i",
        MediaKind::Video => "v",
        MediaKind::Audio => "a",
    }
}

fn kind_of(code: &str) -> Option<MediaKind> {
    match code {
        "i" => Some(MediaKind::Image),
        "v" => Some(MediaKind::Video),
        "a" => Some(MediaKind::Audio),
        _ => None,
    }
}

const fn availability_code(availability: Availability) -> &'static str {
    match availability {
        Availability::Available => "1",
        Availability::Missing => "0",
    }
}

fn availability_of(code: &str) -> Option<Availability> {
    match code {
        "1" => Some(Availability::Available),
        "0" => Some(Availability::Missing),
        _ => None,
    }
}

/// `<seconds>.<nanoseconds>` since the epoch.
///
/// The sub-second part is not decoration: a filesystem reports mtime with
/// nanosecond precision, and the catalogue decides "did this file change?" by
/// comparing that timestamp exactly. Storing whole seconds made every record
/// look changed on the next launch, which threw away every tag the previous run
/// had extracted — the one thing this file exists to keep.
fn timestamp(time: SystemTime) -> String {
    let elapsed = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    format!("{}.{:09}", elapsed.as_secs(), elapsed.subsec_nanos())
}

/// Reads a timestamp, accepting a bare second count so a file written before
/// the sub-second part existed still loads.
fn timestamp_of(field: &str) -> Option<SystemTime> {
    let (seconds, nanoseconds) = match field.split_once('.') {
        Some((seconds, nanoseconds)) => (seconds, nanoseconds.parse::<u32>().ok()?),
        None => (field, 0),
    };
    SystemTime::UNIX_EPOCH.checked_add(Duration::new(seconds.parse().ok()?, nanoseconds))
}

fn decode_path(field: &str) -> Option<PathBuf> {
    if field.is_empty() {
        return None;
    }
    Some(percent::path_from_bytes(&percent::decode_strict(field)?))
}

/// An absent tag and an empty tag are the same thing to the library, and `-`
/// keeps the column from ever being empty — which is what would let a stray
/// tab silently shift every field after it.
fn optional_text(value: Option<&str>) -> String {
    match value.map(str::trim).filter(|text| !text.is_empty()) {
        Some(text) => percent::encode(text.as_bytes()),
        None => "-".to_owned(),
    }
}

fn text_of(field: &str) -> Option<String> {
    if field == "-" {
        return None;
    }
    let bytes = percent::decode_strict(field)?;
    let text = String::from_utf8(bytes).ok()?;
    Some(text).filter(|text| !text.is_empty())
}

fn optional_number(value: Option<u32>) -> String {
    value.map_or_else(|| "-".to_owned(), |number| number.to_string())
}

fn number_of(field: &str) -> Option<u32> {
    field.parse().ok()
}

/// Duration and year share a column as `<millis>/<year>`; both are optional and
/// `-` on either side means "not known".
fn optional_duration(duration: Option<Duration>, year: Option<i32>) -> String {
    let duration = duration.map_or_else(|| "-".to_owned(), |value| value.as_millis().to_string());
    let year = year.map_or_else(|| "-".to_owned(), |value| value.to_string());
    format!("{duration}/{year}")
}

fn duration_and_year(field: &str) -> (Option<Duration>, Option<i32>) {
    let mut parts = field.splitn(2, '/');
    let duration = parts
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_millis);
    let year = parts.next().and_then(|value| value.parse::<i32>().ok());
    (duration, year)
}

#[cfg(test)]
mod tests {
    use super::{default_path, load, save, HEADER};
    use fluorita_core::{
        Availability, Catalogue, MediaId, MediaKind, MediaMetadata, MediaRecord, SourceId,
        SourceIdentity,
    };
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("fluorita-store-tests/{name}"));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("scratch");
        directory.join("catalogue.tsv")
    }

    fn record(inode: u64, path: &str, kind: MediaKind) -> MediaRecord {
        MediaRecord::new(
            MediaId::filesystem(66, inode),
            SourceId::from_value(2),
            PathBuf::from(path),
            kind,
            SourceIdentity::new(
                4_096,
                SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            ),
        )
    }

    fn tagged() -> MediaMetadata {
        MediaMetadata {
            title: Some("Canción con acentos".to_owned()),
            artist: Some("Intérprete".to_owned()),
            album: Some("Álbum".to_owned()),
            album_artist: Some("Grupo".to_owned()),
            track_number: Some(3),
            disc_number: Some(2),
            year: Some(1999),
            duration: Some(Duration::from_millis(213_400)),
        }
    }

    #[test]
    fn a_catalogue_survives_a_round_trip_with_its_metadata() {
        let path = scratch("round-trip");
        let mut catalogue = Catalogue::new();
        catalogue.upsert(
            record(1, "/home/toni/Música/a.flac", MediaKind::Audio).with_metadata(tagged()),
        );
        catalogue.upsert(record(2, "/home/toni/Imágenes/foto.png", MediaKind::Image));

        save(&path, &catalogue).expect("the catalogue is written");
        let outcome = load(&path).expect("the catalogue is read");

        assert_eq!(outcome.skipped, 0);
        assert!(!outcome.absent);
        assert_eq!(outcome.catalogue.len(), 2);

        let restored = outcome
            .catalogue
            .get(&MediaId::filesystem(66, 1))
            .expect("the tagged record");
        assert_eq!(restored.path(), Path::new("/home/toni/Música/a.flac"));
        assert_eq!(restored.kind(), MediaKind::Audio);
        assert_eq!(restored.identity().size, 4_096);
        assert_eq!(restored.metadata(), &tagged());
        assert_eq!(restored.source().value(), 2);
    }

    #[test]
    fn an_untagged_record_comes_back_untagged_rather_than_empty_stringed() {
        let path = scratch("untagged");
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/home/toni/Vídeos/clip.mkv", MediaKind::Video));

        save(&path, &catalogue).expect("written");
        let outcome = load(&path).expect("read");
        let restored = outcome
            .catalogue
            .get(&MediaId::filesystem(66, 1))
            .expect("the record");

        assert_eq!(restored.metadata(), &MediaMetadata::default());
        assert_eq!(restored.metadata().track_title(), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_path_round_trips_byte_for_byte() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let path = scratch("non-utf8");
        let source = PathBuf::from(OsStr::from_bytes(b"/home/toni/V\xC3\xADdeos/mal-\xFF.mkv"));
        let mut catalogue = Catalogue::new();
        catalogue.upsert(MediaRecord::new(
            MediaId::filesystem(66, 7),
            SourceId::from_value(0),
            source.clone(),
            MediaKind::Video,
            SourceIdentity::new(1, SystemTime::UNIX_EPOCH),
        ));

        save(&path, &catalogue).expect("written");
        let outcome = load(&path).expect("read");

        let restored = outcome
            .catalogue
            .get(&MediaId::filesystem(66, 7))
            .expect("the record");
        assert_eq!(restored.path(), source.as_path());
        assert_eq!(
            restored.path().as_os_str().as_bytes(),
            b"/home/toni/V\xC3\xADdeos/mal-\xFF.mkv"
        );
    }

    #[test]
    fn an_item_last_seen_missing_does_not_come_back_available() {
        let path = scratch("missing");
        let mut catalogue = Catalogue::new();
        catalogue.upsert(
            record(1, "/mnt/externo/clip.mkv", MediaKind::Video)
                .with_availability(Availability::Missing),
        );

        save(&path, &catalogue).expect("written");
        let outcome = load(&path).expect("read");

        assert_eq!(
            outcome
                .catalogue
                .get(&MediaId::filesystem(66, 1))
                .map(MediaRecord::availability),
            Some(Availability::Missing)
        );
    }

    #[test]
    fn a_first_run_finds_nothing_and_says_so_without_failing() {
        let outcome = load(Path::new("/nonexistent/fluorita/catalogue.tsv"))
            .expect("a missing catalogue is not an error");

        assert!(outcome.absent);
        assert!(outcome.catalogue.is_empty());
    }

    #[test]
    fn a_corrupt_line_is_skipped_and_counted_while_the_rest_survives() {
        let path = scratch("corrupt");
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/home/toni/a.png", MediaKind::Image));
        catalogue.upsert(record(2, "/home/toni/b.png", MediaKind::Image));
        save(&path, &catalogue).expect("written");

        // Two damaged lines: one truncated, one with a field that is not a number.
        let mut text = std::fs::read_to_string(&path).expect("read back");
        text.push_str("sólo\tunos\tcampos\n");
        text.push_str("no-un-numero\t1\t\t2\ti\t1\t1\t1\t/a\t-\t-\t-\t-\t-\t-\t-/-\n");
        std::fs::write(&path, text).expect("damaged");

        let outcome = load(&path).expect("read");

        assert_eq!(outcome.skipped, 2);
        assert_eq!(outcome.catalogue.len(), 2, "the good records survived");
    }

    #[test]
    fn an_unknown_version_is_treated_as_no_catalogue_rather_than_guessed_at() {
        let path = scratch("version");
        std::fs::write(&path, "fluorita-catalogue 99\nlo que sea\n").expect("written");

        let outcome = load(&path).expect("read");

        assert!(outcome.absent);
        assert!(outcome.catalogue.is_empty());
        assert!(
            HEADER.ends_with('1'),
            "el encabezado actual es la versión 1"
        );
    }

    #[test]
    fn a_field_cannot_smuggle_a_tab_or_a_newline_into_the_next_record() {
        let path = scratch("injection");
        let mut catalogue = Catalogue::new();
        catalogue.upsert(
            record(1, "/home/toni/a.flac", MediaKind::Audio).with_metadata(MediaMetadata {
                title: Some("titulo\tcon\ttabuladores\ny salto".to_owned()),
                ..MediaMetadata::default()
            }),
        );

        save(&path, &catalogue).expect("written");
        let text = std::fs::read_to_string(&path).expect("read back");
        let outcome = load(&path).expect("read");

        // One header plus exactly one record line.
        assert_eq!(text.lines().count(), 2);
        assert_eq!(outcome.skipped, 0);
        assert_eq!(
            outcome
                .catalogue
                .get(&MediaId::filesystem(66, 1))
                .and_then(|record| record.metadata().title.clone()),
            Some("titulo\tcon\ttabuladores\ny salto".to_owned())
        );
    }

    #[test]
    fn saving_replaces_the_previous_catalogue_without_leaving_temporaries() {
        let path = scratch("replace");
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/home/toni/a.png", MediaKind::Image));
        save(&path, &catalogue).expect("first write");

        catalogue.upsert(record(2, "/home/toni/b.png", MediaKind::Image));
        save(&path, &catalogue).expect("second write");

        let outcome = load(&path).expect("read");
        assert_eq!(outcome.catalogue.len(), 2);

        let leftovers: Vec<String> = std::fs::read_dir(path.parent().expect("directory"))
            .expect("listing")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name != "catalogue.tsv")
            .collect();
        assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
    }

    #[test]
    fn the_default_location_is_under_the_data_directory() {
        let path = default_path().expect("HOME is set in the test environment");
        assert!(path.ends_with("fluorita/catalogue.tsv"), "{path:?}");
    }
}

#[cfg(test)]
mod fidelity_tests {
    use super::{load, save};
    use fluorita_core::{
        Catalogue, MediaId, MediaKind, MediaMetadata, MediaRecord, SourceId, SourceIdentity,
    };
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    /// The regression this pins, found by measuring a real launch rather than
    /// by reading the code: storing the mtime as whole seconds made a reloaded
    /// record differ from the same file on disk, so `absorb` treated every
    /// track as changed and re-read every tag on every launch — silently
    /// undoing the only thing persistence is for.
    #[test]
    fn a_stored_record_still_matches_the_file_it_describes() {
        let directory = std::env::temp_dir().join("fluorita-store-tests/fidelity");
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("scratch");
        let path = directory.join("catalogue.tsv");

        // A real filesystem timestamp: seconds *and* nanoseconds.
        let modified = SystemTime::UNIX_EPOCH + Duration::new(1_748_316_078, 123_456_789);
        let identity = SourceIdentity::new(16_951, modified);
        let record = MediaRecord::new(
            MediaId::filesystem(52, 251_071),
            SourceId::from_value(1),
            PathBuf::from("/home/toni/Música/pista.mp3"),
            MediaKind::Audio,
            identity,
        )
        .with_metadata(MediaMetadata {
            title: Some("Pista de prueba".to_owned()),
            duration: Some(Duration::from_millis(2_000)),
            ..MediaMetadata::default()
        });

        let mut catalogue = Catalogue::new();
        catalogue.upsert(record.clone());
        save(&path, &catalogue).expect("written");
        let mut reloaded = load(&path).expect("read").catalogue;

        // The same file, scanned again with no metadata: absorbing it must keep
        // the tags, which only happens if the stored identity is exact.
        let rescanned = MediaRecord::new(
            MediaId::filesystem(52, 251_071),
            SourceId::from_value(1),
            PathBuf::from("/home/toni/Música/pista.mp3"),
            MediaKind::Audio,
            identity,
        );
        let summary = reloaded.absorb([rescanned], true);

        assert_eq!(summary.unchanged, 1, "el archivo no ha cambiado");
        assert_eq!(summary.replaced, 0);
        assert_eq!(
            reloaded
                .get(&MediaId::filesystem(52, 251_071))
                .and_then(|found| found.metadata().title.clone()),
            Some("Pista de prueba".to_owned()),
            "una etiqueta ya leída no vuelve a leerse"
        );
    }
}
