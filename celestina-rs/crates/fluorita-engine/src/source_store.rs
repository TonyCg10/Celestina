//! Writing the configured roots down, and reading them back.
//!
//! **Why persist at all.** The roots are what the user chose to map, so they
//! cannot be re-derived from the environment on every launch: a folder added
//! last week is not an XDG media directory, and one the user removed would come
//! straight back. Persisting is also what makes a [`SourceId`] mean anything.
//! The stored catalogue keys every record by one, so a configuration rebuilt in
//! load order would silently reassign records to the wrong root the moment the
//! set stopped matching the seeding order by accident.
//!
//! **The format.** Deliberately the catalogue store's, because the reasons are
//! the same: one header line, then one root per line, tab-separated, with every
//! free-form field percent-encoded through the suite's canonical codec so raw
//! path bytes round-trip and no field can smuggle a tab or a newline into the
//! next record. A whole-file atomic rewrite is correct for a list this small,
//! and the previous file stays intact until the new one is durable.
//!
//! **What a damaged file means.** Not an empty library. An unreadable,
//! oversized or unrecognised file, and every unparsable line inside a readable
//! one, is reported and then answered with the first-run seed, because a user
//! whose configuration was lost is far better served by their media
//! directories than by a blank window with no way back.

use std::path::{Path, PathBuf};

use celestina_core::{atomic_file, percent};
use fluorita_core::{KindSet, MediaKind, MediaSource, SourceId, SourceSet, XdgMediaDirs};

use crate::error::{EngineError, EngineResult};

/// The first line of the file. A future format change bumps this, and an
/// unrecognised version is treated as "nothing configured yet".
const HEADER: &str = "fluorita-sources 1";

/// A ceiling on what will be read into memory. At roughly 100 bytes an entry
/// this is far past any plausible configuration and stops a corrupted or
/// hostile file from being loaded whole.
const MAX_BYTES: u64 = 1024 * 1024;

/// Fields per entry: identity, kinds, root. A line with any other count is
/// skipped rather than guessed at.
const FIELDS: usize = 3;

/// What a load found, including what it had to fall back on.
#[derive(Debug, Default)]
pub struct SourceLoad {
    pub sources: SourceSet,
    /// Lines that could not be read, or that the domain refused. Counted rather
    /// than hidden: a configuration that silently lost half its roots would
    /// look like a scanning problem later.
    pub skipped: usize,
    /// True when the returned set is the first-run seed rather than something
    /// the user configured — either because nothing was stored yet or because
    /// what was stored could not be used.
    pub seeded: bool,
}

/// Reads the configured roots at `path`, falling back to the seed.
///
/// A missing file, an unreadable one, an oversized one and an unrecognised
/// version all mean the same thing to a caller: this user has no usable stored
/// configuration, so start from their media directories. None of them is an
/// error, because none of them should stop the library from opening.
#[must_use]
pub fn load(path: &Path, dirs: &XdgMediaDirs) -> SourceLoad {
    let Some(text) = readable(path) else {
        return seed(dirs);
    };

    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return seed(dirs);
    }

    let mut sources = SourceSet::new();
    let mut skipped = 0;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        // The domain validates a stored entry exactly like a new one, so a
        // relative, kindless, overlapping or duplicated root is refused here
        // rather than trusted because it came from our own file.
        let restored =
            parse(line).is_some_and(|(id, root, kinds)| sources.restore(id, root, kinds).is_ok());
        if !restored {
            skipped += 1;
        }
    }

    // Every line was refused, or the file held only a header. Either way the
    // user has no roots and no way to get any back except the seed.
    if sources.is_empty() {
        let mut seeded = seed(dirs);
        seeded.skipped += skipped;
        return seeded;
    }

    SourceLoad {
        sources,
        skipped,
        seeded: false,
    }
}

/// Writes the whole configuration, atomically.
pub fn save(path: &Path, sources: &SourceSet) -> EngineResult<()> {
    let mut text = String::with_capacity(sources.sources().len() * 96 + HEADER.len() + 1);
    text.push_str(HEADER);
    text.push('\n');
    for source in sources.sources() {
        text.push_str(&format(source));
        text.push('\n');
    }

    atomic_file::replace(path, text.as_bytes()).map_err(|source| EngineError::Io {
        operation: "write the configured sources",
        path: path.to_path_buf(),
        source,
    })
}

/// Where the configuration lives: `$XDG_CONFIG_HOME/fluorita/sources.tsv`.
///
/// Config rather than data, unlike the catalogue: this is the user's choice,
/// not something the application can rebuild by looking at the disk.
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    celestina_core::xdg::config_home().map(|config| config.join("fluorita").join("sources.tsv"))
}

fn readable(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > MAX_BYTES {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

fn seed(dirs: &XdgMediaDirs) -> SourceLoad {
    SourceLoad {
        sources: SourceSet::seeded_from(dirs),
        skipped: 0,
        seeded: true,
    }
}

fn format(source: &MediaSource) -> String {
    [
        source.id().value().to_string(),
        kind_codes(source.kinds()),
        percent::encode(&percent::path_bytes(source.root())),
    ]
    .join("\t")
}

fn parse(line: &str) -> Option<(SourceId, PathBuf, KindSet)> {
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() != FIELDS {
        return None;
    }
    let id = SourceId::from_value(fields[0].parse().ok()?);
    let kinds = parse_kinds(fields[1])?;
    let root = percent::path_from_bytes(&percent::decode_strict(fields[2])?);
    Some((id, root, kinds))
}

/// The kinds as one field of stable single-letter codes, in a fixed order so
/// the same configuration always writes the same bytes.
fn kind_codes(kinds: KindSet) -> String {
    let mut codes = String::with_capacity(3);
    for kind in [MediaKind::Image, MediaKind::Video, MediaKind::Audio] {
        if kinds.contains(kind) {
            codes.push(kind_code(kind));
        }
    }
    codes
}

fn parse_kinds(field: &str) -> Option<KindSet> {
    let mut kinds = KindSet::empty();
    for code in field.chars() {
        kinds = kinds.with(match code {
            'i' => MediaKind::Image,
            'v' => MediaKind::Video,
            'a' => MediaKind::Audio,
            // An unknown code is a file this build cannot read correctly, and
            // guessing which kinds a root contributes would scan the wrong
            // files. The domain then refuses the empty set on its own.
            _ => return None,
        });
    }
    Some(kinds)
}

const fn kind_code(kind: MediaKind) -> char {
    match kind {
        MediaKind::Image => 'i',
        MediaKind::Video => 'v',
        MediaKind::Audio => 'a',
    }
}

#[cfg(test)]
mod tests {
    use super::{load, save, MAX_BYTES};
    use fluorita_core::{KindSet, MediaSource, SourceId, SourceSet, XdgMediaDirs};
    use std::path::{Path, PathBuf};

    fn dirs() -> XdgMediaDirs {
        XdgMediaDirs {
            pictures: Some(PathBuf::from("/home/toni/Pictures")),
            videos: Some(PathBuf::from("/home/toni/Videos")),
            music: Some(PathBuf::from("/home/toni/Music")),
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join("fluorita-source-store-tests");
        std::fs::create_dir_all(&directory).expect("scratch directory");
        let path = directory.join(name);
        std::fs::remove_file(&path).ok();
        path
    }

    fn roots(sources: &SourceSet) -> Vec<PathBuf> {
        sources
            .sources()
            .iter()
            .map(|source| source.root().to_path_buf())
            .collect()
    }

    #[test]
    fn a_first_run_seeds_from_the_media_directories() {
        let outcome = load(Path::new("/nonexistent/fluorita/sources.tsv"), &dirs());

        assert!(outcome.seeded);
        assert_eq!(outcome.sources.sources().len(), 3);
        assert_eq!(outcome.skipped, 0);
    }

    #[test]
    fn a_configuration_round_trips_with_its_identities_intact() {
        let path = scratch("round-trip.tsv");
        let mut written = SourceSet::new();
        let pictures = written
            .add(PathBuf::from("/home/toni/Pictures"), KindSet::gallery())
            .expect("absolute root");
        let music = written
            .add(PathBuf::from("/home/toni/Music"), KindSet::audio())
            .expect("absolute root");
        // A folder the user chose themselves, which no seed would ever produce.
        let chosen = written
            .add(PathBuf::from("/mnt/archive/2026"), KindSet::all())
            .expect("absolute root");
        save(&path, &written).expect("write the configuration");

        let outcome = load(&path, &dirs());

        assert!(!outcome.seeded);
        assert_eq!(outcome.skipped, 0);
        assert_eq!(
            roots(&outcome.sources),
            vec![
                PathBuf::from("/home/toni/Pictures"),
                PathBuf::from("/home/toni/Music"),
                PathBuf::from("/mnt/archive/2026"),
            ]
        );
        // The handles the stored catalogue's records refer to, unchanged.
        assert_eq!(
            outcome
                .sources
                .sources()
                .iter()
                .map(MediaSource::id)
                .collect::<Vec<_>>(),
            vec![pictures, music, chosen]
        );
        assert_eq!(
            outcome.sources.get(music).map(MediaSource::kinds),
            Some(KindSet::audio())
        );
        std::fs::remove_file(&path).ok();
    }

    #[cfg(unix)]
    #[test]
    fn a_root_whose_name_is_not_utf8_survives_a_write_and_a_read() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let path = scratch("non-utf8.tsv");
        let root = PathBuf::from(OsStr::from_bytes(b"/mnt/foto\xFFs"));
        let mut written = SourceSet::new();
        written
            .add(root.clone(), KindSet::gallery())
            .expect("absolute root");
        save(&path, &written).expect("write the configuration");

        let outcome = load(&path, &dirs());

        assert_eq!(roots(&outcome.sources), vec![root]);
        assert!(!outcome.seeded);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_removed_root_stays_removed_across_a_restart() {
        let path = scratch("removal.tsv");
        let mut written = SourceSet::seeded_from(&dirs());
        let dropped = written.sources()[0].id();
        assert!(written.remove(dropped));
        save(&path, &written).expect("write the configuration");

        let outcome = load(&path, &dirs());

        // The seed would have brought it straight back; a stored configuration
        // is the user's answer, not a starting point to be merged with.
        assert!(!outcome.seeded);
        assert_eq!(outcome.sources.sources().len(), 2);
        assert!(outcome.sources.get(dropped).is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn an_unusable_file_seeds_instead_of_emptying_the_library() {
        for (name, bytes) in [
            ("wrong-version.tsv", "fluorita-sources 99\n".to_owned()),
            ("not-ours.tsv", "something else entirely\n".to_owned()),
            (
                "every-line-refused.tsv",
                // Relative, kindless, unknown kind code, wrong field count.
                "fluorita-sources 1\n0\ti\tPictures\n1\t\t%2Fhome\n2\tz\t%2Fhome\n3\t%2Fhome\n"
                    .to_owned(),
            ),
        ] {
            let path = scratch(name);
            std::fs::write(&path, bytes).expect("fixture");

            let outcome = load(&path, &dirs());

            assert!(outcome.seeded, "{name} should have fallen back to the seed");
            assert_eq!(outcome.sources.sources().len(), 3, "{name}");
            std::fs::remove_file(&path).ok();
        }
    }

    #[test]
    fn a_partly_damaged_file_keeps_what_it_can_and_counts_the_rest() {
        let path = scratch("partly-damaged.tsv");
        std::fs::write(
            &path,
            "fluorita-sources 1\n\
             4\tiv\t%2Fhome%2Ftoni%2FPictures\n\
             nonsense\n\
             4\ta\t%2Fmnt%2Fotro\n",
        )
        .expect("fixture");

        let outcome = load(&path, &dirs());

        assert!(!outcome.seeded);
        assert_eq!(
            roots(&outcome.sources),
            vec![PathBuf::from("/home/toni/Pictures")]
        );
        // One unparsable line and one duplicate identity the domain refused.
        assert_eq!(outcome.skipped, 2);
        // The first line won handle 4; the second could not have it too.
        assert!(outcome.sources.get(SourceId::from_value(4)).is_some());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn an_oversized_file_is_never_read_whole() {
        let path = scratch("oversized.tsv");
        let mut text = String::from("fluorita-sources 1\n");
        while text.len() as u64 <= MAX_BYTES {
            text.push_str("0\ti\t%2Fhome%2Ftoni%2FPictures\n");
        }
        std::fs::write(&path, &text).expect("fixture");

        let outcome = load(&path, &dirs());

        assert!(outcome.seeded);
        std::fs::remove_file(&path).ok();
    }
}
