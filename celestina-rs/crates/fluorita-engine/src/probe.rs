//! Reading what a file actually is, without rendering it.
//!
//! A catalogue scan calls this once per file, so it stays paused, renders
//! nothing, decodes no hardware surface and gives up on a fixed budget. Every
//! field it cannot establish stays `None`: an untagged file is a normal file,
//! not a failure, and inventing a title here would poison the whole library.

use std::path::Path;
use std::time::Duration;

use celestina_core::CancellationToken;
use fluorita_core::{claimed_tag, duration_from_seconds, MediaMetadata};

use crate::backend::{ProbeBudget, ProbeReport};
use crate::error::{EngineError, EngineResult};
use crate::instance::{wait_for_load, Instance, LoadOutcome};
use crate::source::SourceHandle;

pub fn probe(
    path: &Path,
    budget: ProbeBudget,
    cancellation: &CancellationToken,
) -> EngineResult<ProbeReport> {
    if cancellation.is_cancelled() {
        return Err(EngineError::Cancelled);
    }
    let handle = SourceHandle::open(path)?;
    let instance = Instance::new(&[
        ("vo", "null"),
        ("ao", "null"),
        ("pause", "yes"),
        // Metadata needs no decoder acceleration, and a VA-API device per
        // catalogued file would be a cost with nothing to show for it.
        ("hwdec", "no"),
    ])?;
    let client = instance.client()?;
    instance.load(&handle)?;

    match wait_for_load(&client, budget.deadline, cancellation, path)? {
        LoadOutcome::Loaded => {}
        LoadOutcome::Ended(reason) => {
            return Err(EngineError::Undecodable {
                path: path.to_path_buf(),
                detail: format!("the backend stopped before loading ({reason:?})"),
            })
        }
    }

    Ok(collect(&instance))
}

fn collect(instance: &Instance) -> ProbeReport {
    let tracks = track_summary(instance);
    ProbeReport {
        metadata: metadata(instance),
        width: instance
            .optional_i64("width")
            .and_then(|value| u32::try_from(value).ok()),
        height: instance
            .optional_i64("height")
            .and_then(|value| u32::try_from(value).ok()),
        has_video: tracks.has_video,
        has_audio: tracks.has_audio,
        seekable: instance.optional_bool("seekable").unwrap_or(false),
        video_is_attached_picture: tracks.attached_picture,
    }
}

fn metadata(instance: &Instance) -> MediaMetadata {
    MediaMetadata {
        title: tag(instance, "title"),
        artist: tag(instance, "artist"),
        album: tag(instance, "album"),
        album_artist: tag(instance, "album_artist"),
        track_number: tag(instance, "track").as_deref().and_then(parse_index),
        disc_number: tag(instance, "disc").as_deref().and_then(parse_index),
        year: tag(instance, "date").as_deref().and_then(parse_year),
        duration: instance.optional_f64("duration").and_then(known_duration),
    }
}

/// A container's claimed length, or `None` when it claims none that is real.
/// Zero, like a missing value, means "not known".
fn known_duration(seconds: f64) -> Option<Duration> {
    duration_from_seconds(seconds)
        .ok()
        .filter(|duration| !duration.is_zero())
}

/// A tag as the file spells it, sanitised and cut to what the catalogue holds.
/// The raw value is attacker-sized: a crafted ID3 or Vorbis title can be
/// megabytes, and it would flow into every projection and the stored catalogue.
///
/// The cap applies after `libmpv2` copies the string out of the backend: its
/// safe property API has no bounded read, and the backend already holds the
/// whole value in memory, so the one copy is the cost of staying without
/// `unsafe`. Nothing past the cap outlives this function.
fn tag(instance: &Instance, key: &str) -> Option<String> {
    instance
        .optional_string(&format!("metadata/by-key/{key}"))
        .as_deref()
        .and_then(claimed_tag)
}

/// Track numbers are often `3/12`, and a date is often a full timestamp.
fn parse_index(value: &str) -> Option<u32> {
    value
        .split(['/', '-'])
        .next()?
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|number| *number > 0)
}

fn parse_year(value: &str) -> Option<i32> {
    let head: String = value
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    head.parse::<i32>()
        .ok()
        .filter(|year| (1000..=9999).contains(year))
}

#[derive(Default)]
struct TrackSummary {
    has_video: bool,
    has_audio: bool,
    attached_picture: bool,
}

/// Walks the backend's track list. An attached picture is reported as a video
/// track by every container, so the distinction has to be asked for explicitly
/// — otherwise a tagged MP3 would look like a video to the whole library.
fn track_summary(instance: &Instance) -> TrackSummary {
    let mut summary = TrackSummary::default();
    let count = instance.optional_i64("track-list/count").unwrap_or(0);
    for index in 0..count.max(0) {
        let kind = instance.optional_string(&format!("track-list/{index}/type"));
        match kind.as_deref() {
            Some("video") => {
                summary.has_video = true;
                if instance
                    .optional_bool(&format!("track-list/{index}/albumart"))
                    .unwrap_or(false)
                {
                    summary.attached_picture = true;
                }
            }
            Some("audio") => summary.has_audio = true,
            _ => {}
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::{known_duration, parse_index, parse_year};
    use std::time::Duration;

    #[test]
    fn a_crafted_duration_is_unknown_rather_than_fatal() {
        assert_eq!(known_duration(2.5), Some(Duration::from_millis(2500)));
        assert_eq!(known_duration(0.0), None);
        assert_eq!(known_duration(f64::MAX), None);
        assert_eq!(known_duration(f64::NAN), None);
        assert_eq!(known_duration(-1.0), None);
    }

    #[test]
    fn a_track_number_survives_the_common_spellings() {
        assert_eq!(parse_index("3"), Some(3));
        assert_eq!(parse_index("3/12"), Some(3));
        assert_eq!(parse_index(" 07 "), Some(7));
        assert_eq!(parse_index("0"), None, "cero no es una pista");
        assert_eq!(parse_index("A"), None);
    }

    #[test]
    fn a_year_is_extracted_but_never_invented() {
        assert_eq!(parse_year("1999"), Some(1999));
        assert_eq!(parse_year("2014-08-01"), Some(2014));
        assert_eq!(parse_year("14"), None, "dos dígitos son ambiguos");
        assert_eq!(parse_year("sin fecha"), None);
    }
}
