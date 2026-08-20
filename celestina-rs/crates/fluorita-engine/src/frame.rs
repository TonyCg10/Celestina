//! Keeping one frame of a film as a picture.
//!
//! The rest of cutting time — trimming between two points, dropping a track —
//! needs a muxer, and this suite has none. Extracting a frame does not: the
//! backend already renders one to produce a poster, and the only difference
//! here is *which* frame and *where it lands*. So this is the part of the arc
//! that could be built honestly today, and it is built on the path that already
//! exists rather than on a second one.
//!
//! What comes out is a picture in the library's own terms: a PNG beside the
//! film, under a name the keep-both policy chose, at the film's own resolution.
//! It is then an ordinary image — which means F7's editor can crop it, annotate
//! it and redact it, and none of that had to be written twice.

use std::path::{Path, PathBuf};
use std::time::Duration;

use celestina_core::{atomic_file, CancellationToken};
use siderita_ops::{next_available, NameShape};

use crate::error::{EngineError, EngineResult};
use crate::instance::{wait_for_load, Instance, LoadOutcome};
use crate::source::SourceHandle;

/// How long one extraction may take before it is abandoned. A seek into a large
/// file and a single decode; a backend still working after this is one that is
/// not going to answer.
pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(30);

/// The largest frame this will write, in pixels. A film is bounded by what a
/// decoder will produce, but the result lands in the person's own folder and a
/// ceiling belongs where the writing happens.
pub const MAX_FRAME_BYTES: u64 = 256 * 1024 * 1024;

/// One frame to keep.
pub struct FrameRequest<'a> {
    /// The film it comes from.
    pub source: &'a Path,
    /// Where in it. Clamped to the film by the backend itself: a position past
    /// the end simply produces the last frame rather than an error nobody can
    /// act on.
    pub at: Duration,
    /// The word the new picture's name is marked with — product copy, so the
    /// host owns it and the engine only places it.
    pub marker: &'a str,
    pub deadline: Duration,
}

/// What an extraction produced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameExtracted {
    pub written: PathBuf,
    pub at: Duration,
}

/// Renders the frame at `at` and lands it beside the film.
///
/// # Errors
///
/// Refuses a relative source, an item the backend cannot decode, a frame past
/// [`MAX_FRAME_BYTES`], a cancellation, a deadline and any filesystem failure.
/// Nothing is written unless a frame really arrived.
pub fn extract(
    request: &FrameRequest<'_>,
    cancellation: &CancellationToken,
) -> EngineResult<FrameExtracted> {
    if cancellation.is_cancelled() {
        return Err(EngineError::Cancelled);
    }
    if !request.source.is_absolute() {
        return Err(EngineError::UnusableSource {
            path: request.source.to_path_buf(),
            reason: "a frame is taken from an absolute path",
        });
    }

    let staging = staging_directory(request.source)?;
    let rendered = render(request, &staging, cancellation);
    let outcome = rendered.and_then(|path| {
        let bytes = std::fs::read(&path).map_err(|source| EngineError::Io {
            operation: "reading the extracted frame",
            path: path.clone(),
            source,
        })?;
        if bytes.len() as u64 > MAX_FRAME_BYTES {
            return Err(EngineError::OverBudget {
                what: "the extracted frame",
                limit: MAX_FRAME_BYTES,
                actual: bytes.len() as u64,
            });
        }
        Ok(bytes)
    });
    // The staging directory goes whatever happened: a failed extraction must
    // not leave a temporary tree behind in the cache.
    let _ = std::fs::remove_dir_all(&staging);
    let bytes = outcome?;

    let destination = destination_for(request)?;
    atomic_file::replace(&destination, &bytes).map_err(|source| EngineError::Io {
        operation: "writing the extracted frame",
        path: destination.clone(),
        source,
    })?;

    Ok(FrameExtracted {
        written: destination,
        at: request.at,
    })
}

/// The name the frame takes: the film's own, marked, as a PNG.
///
/// The extension is put on the candidate *before* the free-name search, for the
/// reason the edit path had to learn: searching under one extension and writing
/// another is how a keep-both policy overwrites the file it exists to protect.
fn destination_for(request: &FrameRequest<'_>) -> EngineResult<PathBuf> {
    let directory = request
        .source
        .parent()
        .ok_or_else(|| EngineError::UnusableSource {
            path: request.source.to_path_buf(),
            reason: "a film has a parent directory",
        })?;
    let name = request
        .source
        .file_name()
        .ok_or_else(|| EngineError::UnusableSource {
            path: request.source.to_path_buf(),
            reason: "a film has a name",
        })?;
    let target = Path::new(name).with_extension("png");
    Ok(next_available(
        directory,
        target.as_os_str(),
        request.marker,
        NameShape::File,
    ))
}

fn staging_directory(source: &Path) -> EngineResult<PathBuf> {
    let root = celestina_core::xdg::cache_home()
        .ok_or_else(|| EngineError::UnusableSource {
            path: source.to_path_buf(),
            reason: "no cache directory to render into",
        })?
        .join("fluorita")
        .join("frames");
    let unique = root.join(format!("{}", std::process::id()));
    std::fs::create_dir_all(&unique).map_err(|source| EngineError::Io {
        operation: "preparing the frame staging directory",
        path: unique.clone(),
        source,
    })?;
    Ok(unique)
}

fn render(
    request: &FrameRequest<'_>,
    staging: &Path,
    cancellation: &CancellationToken,
) -> EngineResult<PathBuf> {
    let handle = SourceHandle::open(request.source)?;
    let outdir = staging
        .to_str()
        .ok_or_else(|| EngineError::UnusableSource {
            path: staging.to_path_buf(),
            reason: "the cache path is not valid UTF-8",
        })?;
    let start = format!("{:.3}", request.at.as_secs_f64());

    let instance = Instance::new(&[
        ("vo", "image"),
        ("vo-image-format", "png"),
        ("vo-image-outdir", outdir),
        ("ao", "null"),
        ("audio", "no"),
        ("frames", "1"),
        ("start", &start),
        // No scale filter, unlike a poster: this is the picture a person keeps,
        // so it comes out at the size the film really is.
        ("hwdec", "no"),
    ])?;
    let client = instance.client()?;
    instance.load(&handle)?;

    match wait_for_load(&client, request.deadline, cancellation, request.source)? {
        LoadOutcome::Loaded => {}
        LoadOutcome::Ended(reason) => {
            return Err(EngineError::Undecodable {
                path: request.source.to_path_buf(),
                detail: format!("nothing to extract ({reason:?})"),
            })
        }
    }
    wait_for_frame(&instance, &client, request, cancellation)?;

    first_png(staging).ok_or_else(|| EngineError::Undecodable {
        path: request.source.to_path_buf(),
        detail: "the backend produced no frame".to_owned(),
    })
}

/// Waits for the single frame to be written. The backend reports end-of-file
/// once it has rendered it, and goes idle when it has nothing queued — with
/// `frames=1` either of those means the picture is on disk.
fn wait_for_frame(
    instance: &Instance,
    client: &libmpv2::Mpv,
    request: &FrameRequest<'_>,
    cancellation: &CancellationToken,
) -> EngineResult<()> {
    let started = std::time::Instant::now();
    while started.elapsed() < request.deadline {
        if cancellation.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        match client.wait_event(0.1) {
            Some(Ok(libmpv2::events::Event::EndFile(_) | libmpv2::events::Event::Shutdown)) => {
                return Ok(())
            }
            Some(Err(source)) => {
                return Err(EngineError::Backend {
                    operation: "wait for the extracted frame",
                    source,
                })
            }
            _ => {}
        }
        if instance.optional_bool("idle-active").unwrap_or(false) {
            return Ok(());
        }
    }
    Err(EngineError::TimedOut {
        operation: "extracting a frame",
        after: request.deadline,
    })
}

fn first_png(directory: &Path) -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "png"))
        .collect();
    entries.sort();
    entries.into_iter().next()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;

    use super::{destination_for, extract, FrameRequest};
    use crate::error::EngineError;

    fn request<'a>(source: &'a Path, at: Duration) -> FrameRequest<'a> {
        FrameRequest {
            source,
            at,
            marker: "fotograma",
            deadline: Duration::from_secs(5),
        }
    }

    fn directory(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fluorita-frame-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("test directory");
        path
    }

    #[test]
    fn the_frame_takes_the_films_name_as_a_png_and_never_overwrites_one() {
        let directory = directory("naming");
        let source = directory.join("clip.mkv");
        std::fs::write(&source, b"not really a film").expect("test file");

        assert_eq!(
            destination_for(&request(&source, Duration::ZERO)).expect("a destination"),
            directory.join("clip (fotograma).png")
        );

        std::fs::write(directory.join("clip (fotograma).png"), b"an earlier frame")
            .expect("test file");
        assert_eq!(
            destination_for(&request(&source, Duration::ZERO)).expect("a destination"),
            directory.join("clip (fotograma 2).png"),
            "a second frame must not overwrite the first"
        );

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_cancelled_or_relative_request_never_starts_the_backend() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let failure = extract(
            &request(Path::new("/m/clip.mkv"), Duration::ZERO),
            &cancellation,
        )
        .expect_err("cancelled");
        assert!(matches!(failure, EngineError::Cancelled));

        let relative = extract(
            &request(Path::new("clip.mkv"), Duration::ZERO),
            &CancellationToken::new(),
        )
        .expect_err("refused");
        assert!(matches!(relative, EngineError::UnusableSource { .. }));
    }

    #[test]
    fn a_file_that_is_not_a_film_leaves_nothing_behind() {
        let directory = directory("undecodable");
        let source = directory.join("clip.mkv");
        std::fs::write(&source, b"not really a film").expect("test file");

        let failure = extract(&request(&source, Duration::ZERO), &CancellationToken::new())
            .expect_err("the backend cannot decode this");
        // Which refusal it is depends on where the backend gives up — it
        // reports a rejected load as a backend error and a file it opened but
        // could not use as undecodable. What matters is that it refused rather
        // than writing something, and that nothing survived the attempt.
        assert!(
            !matches!(failure, EngineError::Cancelled),
            "unexpected: {failure:?}"
        );
        assert!(
            !directory.join("clip (fotograma).png").exists(),
            "a failed extraction must not leave a picture"
        );

        std::fs::remove_dir_all(&directory).ok();
    }
}
