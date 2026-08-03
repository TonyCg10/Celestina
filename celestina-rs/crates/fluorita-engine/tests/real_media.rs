//! The engine against real files and the real backend.
//!
//! These are not unit tests with a stub: they start libmpv, decode two tiny
//! synthetic fixtures (`tests/fixtures/`, generated from `lavfi` sources so
//! nothing personal is committed) and check what actually lands on disk. A
//! decode contract that is only tested against a fake is not tested.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use celestina_core::{CancellationToken, Generation, GenerationClock};
use fluorita_core::{ArtworkOrigin, PlaybackRequest, PlaybackState, ReportKind};
use fluorita_engine::backend::{ArtworkJob, MediaEngine, ProbeBudget, SessionRequest};
use fluorita_engine::MpvEngine;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("fluorita-engine-tests/{name}"));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("scratch directory");
    directory
}

fn generation() -> Generation {
    GenerationClock::default().issue().expect("generation")
}

#[test]
fn a_video_probes_as_a_moving_picture_with_its_real_shape() {
    let report = MpvEngine::new()
        .probe(
            &fixture("clip.mp4"),
            ProbeBudget::conservative(),
            &CancellationToken::new(),
        )
        .expect("the fixture decodes");

    assert!(report.has_moving_video());
    assert!(!report.video_is_attached_picture);
    assert_eq!(report.width, Some(64));
    assert_eq!(report.height, Some(64));
    assert!(report.seekable);
    let duration = report.metadata.duration.expect("a duration");
    assert!(
        (duration.as_secs_f64() - 2.0).abs() < 0.5,
        "unexpected duration: {duration:?}"
    );
}

#[test]
fn an_audio_file_reads_its_tags_and_marks_its_cover_as_not_video() {
    let report = MpvEngine::new()
        .probe(
            &fixture("tone.mp3"),
            ProbeBudget::conservative(),
            &CancellationToken::new(),
        )
        .expect("the fixture decodes");

    assert!(report.has_audio);
    assert!(
        !report.has_moving_video(),
        "an embedded cover must not make this look like a video"
    );
    assert!(report.video_is_attached_picture);

    let metadata = &report.metadata;
    assert_eq!(metadata.track_title(), Some("Pista de prueba"));
    assert_eq!(metadata.grouping_artist(), Some("Fluorita"));
    assert_eq!(metadata.album_title(), Some("Motor"));
    assert_eq!(metadata.track_number, Some(3), "`3/12` is track three");
    assert_eq!(metadata.year, Some(2026), "the year comes out of the date");
}

#[test]
fn an_untagged_field_stays_absent_instead_of_being_invented() {
    let report = MpvEngine::new()
        .probe(
            &fixture("clip.mp4"),
            ProbeBudget::conservative(),
            &CancellationToken::new(),
        )
        .expect("the fixture decodes");

    assert_eq!(report.metadata.album, None);
    assert_eq!(report.metadata.artist, None);
    assert_eq!(report.metadata.disc_number, None);
}

#[test]
fn a_video_poster_lands_exactly_where_siderita_looks_for_it() {
    let cache_root = scratch("poster-cache");
    let source = fixture("clip.mp4");
    let mtime = std::fs::metadata(&source)
        .and_then(|data| data.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let published = MpvEngine::new()
        .publish_artwork(&ArtworkJob {
            source: source.clone(),
            cache_root: cache_root.clone(),
            origin: ArtworkOrigin::VideoPoster,
            source_mtime: mtime,
            uniquifier: 1,
            deadline: Duration::from_secs(20),
            cancellation: CancellationToken::new(),
        })
        .expect("the poster is extracted");

    // The path is the core's frozen contract, not something this test recomputes.
    let expected = fluorita_core::large_thumbnail_path(&cache_root, &source).expect("cache path");
    assert_eq!(published, expected);
    assert!(published.exists());

    let bytes = std::fs::read(&published).expect("the published entry");
    assert!(bytes.starts_with(b"\x89PNG"), "the entry must be a PNG");
    assert!(bytes.len() > 100, "an empty file is not a thumbnail");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&published)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "a thumbnail can disclose its source");
    }

    // Nothing temporary survives a successful publication.
    let leftovers: Vec<_> = std::fs::read_dir(cache_root.join("large"))
        .expect("cache directory")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with('.') || name.contains(".tmp-"))
        .collect();
    assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
}

#[test]
fn an_embedded_cover_is_published_from_an_audio_file() {
    let cache_root = scratch("cover-cache");
    let source = fixture("tone.mp3");

    let published = MpvEngine::new()
        .publish_artwork(&ArtworkJob {
            source: source.clone(),
            cache_root: cache_root.clone(),
            origin: ArtworkOrigin::EmbeddedCover,
            source_mtime: SystemTime::UNIX_EPOCH,
            uniquifier: 2,
            deadline: Duration::from_secs(20),
            cancellation: CancellationToken::new(),
        })
        .expect("the cover is extracted");

    assert_eq!(
        published,
        fluorita_core::large_thumbnail_path(&cache_root, &source).expect("cache path")
    );
    assert!(std::fs::read(&published)
        .expect("bytes")
        .starts_with(b"\x89PNG"));
}

#[test]
fn a_session_moves_state_only_when_the_backend_reports_it() {
    let engine = MpvEngine::new();
    let expected = generation();
    let mut session = engine
        .open_session(
            SessionRequest::new(fixture("tone.mp3"), expected)
                .silent()
                .paused(),
        )
        .expect("the session opens");
    session.start().expect("the session starts");

    assert_eq!(session.generation(), expected);

    // Asking to play returns as soon as the backend accepts the request.
    session
        .request(PlaybackRequest::Play)
        .expect("the backend accepts play");

    let mut playing = false;
    let mut duration = None;
    let mut position = None;
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline
        && !(playing && duration.is_some() && position.is_some())
    {
        let Some(report) = session.poll(Duration::from_millis(200)) else {
            continue;
        };
        assert_eq!(
            report.generation, expected,
            "every report carries the session's generation"
        );
        match report.kind {
            ReportKind::State(PlaybackState::Playing) => playing = true,
            ReportKind::Duration(value) => duration = Some(value),
            ReportKind::Position(value) => position = Some(value),
            _ => {}
        }
    }

    assert!(playing, "the backend never confirmed playback");
    assert!(duration.is_some(), "the backend never reported a duration");
    assert!(position.is_some(), "the backend never reported a position");
    session.close();
}

#[test]
fn a_seek_is_reported_as_completed_only_after_the_backend_restarts() {
    let engine = MpvEngine::new();
    let mut session = engine
        .open_session(SessionRequest::new(fixture("clip.mp4"), generation()).silent())
        .expect("the session opens");
    session.start().expect("the session starts");

    // Wait until playback is under way, so the seek has something to seek in.
    let warmup = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < warmup {
        if let Some(report) = session.poll(Duration::from_millis(200)) {
            if matches!(report.kind, ReportKind::Position(_)) {
                break;
            }
        }
    }

    session
        .request(PlaybackRequest::Seek(Duration::from_millis(1_500)))
        .expect("the backend accepts the seek");

    let mut completed = None;
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline && completed.is_none() {
        if let Some(report) = session.poll(Duration::from_millis(200)) {
            if let ReportKind::SeekCompleted(position) = report.kind {
                completed = Some(position);
            }
        }
    }

    let position = completed.expect("the backend never confirmed the seek");
    assert!(
        position >= Duration::from_millis(1_000),
        "confirmed at {position:?}, which is not where the seek asked for"
    );
    session.close();
}

#[test]
fn a_closed_session_refuses_further_requests() {
    let engine = MpvEngine::new();
    let mut session = engine
        .open_session(SessionRequest::new(fixture("tone.mp3"), generation()).silent())
        .expect("the session opens");
    session.start().expect("the session starts");

    session.close();

    assert!(session.request(PlaybackRequest::Play).is_err());
    assert!(session.poll(Duration::from_millis(50)).is_none());
}

#[cfg(unix)]
#[test]
fn a_non_utf8_filename_is_probed_through_a_descriptor_not_a_lossy_name() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let directory = scratch("non-utf8");
    let path = directory.join(OsStr::from_bytes(b"clip-\xFF.mp4"));
    std::fs::copy(fixture("clip.mp4"), &path).expect("the fixture is copied");

    let report = MpvEngine::new()
        .probe(
            &path,
            ProbeBudget::conservative(),
            &CancellationToken::new(),
        )
        .expect("a name that is not UTF-8 is still playable");

    assert!(report.has_moving_video());
    assert_eq!(report.width, Some(64));
}

#[test]
fn the_worker_runs_a_real_probe_off_the_calling_thread() {
    use fluorita_engine::worker::{EngineWorker, Job, JobOutcome};

    let expected = generation();
    let worker = EngineWorker::start().expect("the worker starts");
    worker
        .submit(Job::Probe {
            generation: expected,
            path: fixture("clip.mp4"),
            budget: ProbeBudget::conservative(),
        })
        .expect("queued");

    let outcome = worker.poll(Duration::from_secs(20)).expect("an outcome");
    assert_eq!(outcome.generation(), expected);
    match outcome {
        JobOutcome::Probed { result, .. } => {
            assert!(result.expect("the probe succeeds").has_moving_video());
        }
        _ => panic!("wrong outcome kind"),
    }
}

#[test]
fn a_trailer_is_produced_inside_its_budget_and_outside_the_thumbnail_cache() {
    let cache_root = scratch("trailer-cache");
    let source = fixture("clip.mp4");
    let budget = fluorita_core::TrailerBudget::conservative();

    let outcome = MpvEngine::new()
        .produce_trailer(&fluorita_engine::TrailerJob {
            source: source.clone(),
            cache_root: cache_root.clone(),
            budget,
            uniquifier: 7,
            deadline: Duration::from_secs(30),
            cancellation: CancellationToken::new(),
        })
        .expect("the trailer is produced");

    // The destination is the core's key under Fluorita's own cache — never the
    // freedesktop entry another application scans.
    let request = fluorita_core::TrailerRequest::new(
        generation(),
        fluorita_core::MediaId::from_path(&source),
        source.clone(),
        fluorita_core::MediaKind::Video,
        fluorita_core::SourceIdentity::new(1, SystemTime::UNIX_EPOCH),
        budget,
    )
    .expect("video has a trailer");
    assert_eq!(
        outcome.path,
        request.trailer_cache_path(&cache_root).expect("cache path")
    );
    assert!(outcome.path.exists());
    assert_ne!(
        outcome.path.extension().and_then(|e| e.to_str()),
        Some("png")
    );

    // Every bound is measured from the encode, not assumed from the request.
    assert!(outcome.bytes > 0 && outcome.bytes <= budget.max_bytes);
    assert!(
        outcome.duration <= budget.max_duration + Duration::from_millis(500),
        "trailer runs {:?}",
        outcome.duration
    );
    assert!(u64::from(outcome.width) * u64::from(outcome.height) <= budget.max_pixels);

    // Nothing partial survives a successful publication.
    let leftovers: Vec<_> = std::fs::read_dir(cache_root.join("trailers"))
        .expect("trailer directory")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".tmp-"))
        .collect();
    assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
}

#[test]
fn a_trailer_that_would_break_its_byte_budget_is_discarded_not_published() {
    let cache_root = scratch("trailer-over-budget");
    let source = fixture("clip.mp4");

    let error = MpvEngine::new()
        .produce_trailer(&fluorita_engine::TrailerJob {
            source: source.clone(),
            cache_root: cache_root.clone(),
            // No real encode fits in 32 bytes, so this exercises the check that
            // an over-budget result is thrown away rather than cached.
            budget: fluorita_core::TrailerBudget {
                max_bytes: 32,
                ..fluorita_core::TrailerBudget::conservative()
            },
            uniquifier: 8,
            deadline: Duration::from_secs(30),
            cancellation: CancellationToken::new(),
        })
        .expect_err("the encode cannot fit in 32 bytes");

    assert!(error.to_string().contains("budget"), "{error}");
    let destination =
        fluorita_engine::trailer::destination_for(&cache_root, &source).expect("destination");
    assert!(
        !destination.exists(),
        "an over-budget trailer must not be cached"
    );

    let survivors: Vec<_> = std::fs::read_dir(cache_root.join("trailers"))
        .expect("trailer directory")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(survivors.is_empty(), "left behind: {survivors:?}");
}

#[test]
fn the_worker_produces_a_trailer_off_the_calling_thread() {
    use fluorita_engine::worker::{EngineWorker, Job, JobOutcome};

    let cache_root = scratch("trailer-worker");
    let expected = generation();
    let worker = EngineWorker::start().expect("the worker starts");
    worker
        .submit(Job::Trailer {
            generation: expected,
            job: Box::new(fluorita_engine::TrailerJob {
                source: fixture("clip.mp4"),
                cache_root: cache_root.clone(),
                budget: fluorita_core::TrailerBudget::conservative(),
                uniquifier: 9,
                deadline: Duration::from_secs(30),
                cancellation: CancellationToken::new(),
            }),
        })
        .expect("queued");

    let outcome = worker.poll(Duration::from_secs(60)).expect("an outcome");
    assert_eq!(outcome.generation(), expected);
    match outcome {
        JobOutcome::Trailer { result, .. } => {
            let produced = result.expect("the trailer is produced");
            assert!(produced.path.exists());
        }
        _ => panic!("wrong outcome kind"),
    }
}

#[test]
fn cancelling_a_trailer_mid_flight_leaves_no_partial_file_behind() {
    use fluorita_engine::worker::{EngineWorker, Job, JobOutcome};

    let cache_root = scratch("trailer-cancel");
    // `clip.mp4` re-encodes in tens of milliseconds at the production bounding
    // box — too fast to reliably lose the race against `cancel_current()`
    // called right after submit. `heavy_clip.mp4` is a busier synthetic scene
    // (mandelbrot, so every frame is unique) that keeps the real encoder
    // occupied for ~200ms at that same bounding box, measured with the exact
    // mpv options `trailer::encode` uses. A short deterministic sleep below,
    // well under that, makes landing the cancellation mid-encode reliable
    // instead of a coin flip.
    let source = fixture("heavy_clip.mp4");
    let worker = EngineWorker::start().expect("the worker starts");
    worker
        .submit(Job::Trailer {
            generation: generation(),
            job: Box::new(fluorita_engine::TrailerJob {
                source: source.clone(),
                cache_root: cache_root.clone(),
                budget: fluorita_core::TrailerBudget::conservative(),
                uniquifier: 10,
                deadline: Duration::from_secs(30),
                cancellation: CancellationToken::new(),
            }),
        })
        .expect("queued");
    // Give the worker thread time to start mpv, load the source and begin
    // encoding before asking it to stop — otherwise this would only measure
    // thread-scheduling latency, not a genuine mid-encode cancellation.
    std::thread::sleep(Duration::from_millis(60));
    worker.cancel_current();

    let outcome = worker.poll(Duration::from_secs(60)).expect("an outcome");
    let JobOutcome::Trailer { result, .. } = outcome else {
        panic!("wrong outcome kind");
    };

    let destination =
        fluorita_engine::trailer::destination_for(&cache_root, &source).expect("destination");
    let error = result.expect_err("cancelling mid-encode must not still produce a trailer");
    assert!(error.is_retryable(), "{error}");
    assert!(!destination.exists());

    let partial: Vec<_> = std::fs::read_dir(cache_root.join("trailers"))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.contains(".tmp-"))
                .collect()
        })
        .unwrap_or_default();
    assert!(partial.is_empty(), "left behind: {partial:?}");
}

/// What actually makes a long session degrade is not wall-clock time by
/// itself but repeated open/close/seek — the same operations a person does
/// hundreds of times across hours of use. This drives many cycles back to
/// back and watches two leak signals that must stay flat once the allocator
/// and any connection pooling have warmed up: file descriptors (deterministic
/// — a leaked one never comes back on its own) and resident memory (noisier,
/// so its bound is generous; it only needs to catch growth that compounds
/// every cycle, not chase allocator bookkeeping).
#[test]
fn many_open_close_seek_cycles_leave_no_growing_leak() {
    let engine = MpvEngine::new();
    let sources = [fixture("clip.mp4"), fixture("tone.mp3")];
    const CYCLES: usize = 150;
    const WARMUP: usize = 15;

    let mut baseline_fds = 0usize;
    let mut baseline_rss = 0u64;
    let mut max_fd_delta: i64 = 0;
    let mut final_rss = 0u64;

    for cycle in 0..CYCLES {
        let source = &sources[cycle % sources.len()];
        let mut session = engine
            .open_session(
                SessionRequest::new(source.clone(), generation())
                    .silent()
                    .without_hardware_decoding(),
            )
            .expect("the session opens");
        session.start().expect("the session starts");

        // A `Position` report, not just the `Playing` state, is what the
        // other seek test in this file waits for before seeking — the
        // backend can flip `pause` to false slightly before the demuxer is
        // actually far enough along for a seek to land.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut under_way = false;
        while std::time::Instant::now() < deadline && !under_way {
            if let Some(report) = session.poll(Duration::from_millis(200)) {
                if matches!(report.kind, ReportKind::Position(_)) {
                    under_way = true;
                }
            }
        }
        assert!(
            under_way,
            "cycle {cycle}: backend never reported a position"
        );

        // Real decode work each cycle, not just an idle open: a seek forces
        // the backend to flush and restart decoding at a new position.
        session
            .request(PlaybackRequest::Seek(Duration::from_millis(500)))
            .expect("the backend accepts a seek");
        for _ in 0..5 {
            if session.poll(Duration::from_millis(50)).is_none() {
                break;
            }
        }

        session.close();
        drop(session);

        if cycle == WARMUP {
            baseline_fds = open_fd_count();
            baseline_rss = resident_memory_bytes();
        }
        if cycle >= WARMUP {
            let delta = open_fd_count() as i64 - baseline_fds as i64;
            max_fd_delta = max_fd_delta.max(delta.abs());
        }
        if cycle == CYCLES - 1 {
            final_rss = resident_memory_bytes();
        }
    }

    assert!(
        max_fd_delta <= 8,
        "file descriptor count drifted by {max_fd_delta} across {CYCLES} cycles \
         (baseline {baseline_fds}) — looks like a leak, not noise"
    );

    let growth = final_rss.saturating_sub(baseline_rss);
    assert!(
        growth < 64 * 1024 * 1024,
        "resident memory grew by {growth} bytes over {} cycles after warm-up \
         (baseline {baseline_rss}, final {final_rss}) — looks like a leak, not noise",
        CYCLES - WARMUP,
    );
}

fn open_fd_count() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .map(std::iter::Iterator::count)
        .unwrap_or(0)
}

fn resident_memory_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|kib| kib * 1024)
        .unwrap_or(0)
}
