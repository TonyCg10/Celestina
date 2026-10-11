//! The trim's worker: one `ffmpeg` child, its progress, its cancellation, and
//! the landing of what it wrote.
//!
//! Everything here runs on the trim's own thread, never Qt's. The child gets
//! a null standard input (and `-nostdin`), its `-progress pipe:1` lines are
//! read on a thread of their own and folded by
//! [`fluorita_core::trim::Progress`], and its standard error is read on a
//! third, keeping only the last line — what a failure is reported with. A
//! cancellation kills the child and removes its file.
//!
//! The child writes a hidden sibling of the original,
//! `.<name>.trim-<pid>-<n>.mp4`. A VA-API run that fails before anything is
//! published is tried once more with `libx264`.
//! Only after it exits 0, having written at least one frame, does that file
//! take a name, through [`fluorita_engine::land_file`]: the order every edit
//! lands by, so a copy goes beside the original under the keep-both name and
//! a replacement sends the original to the Trash only once the result exists.
//! The original is never written to.

use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use celestina_core::CancellationToken;
use fluorita_core::trim::{
    probe_argv, trim_argv, Encoders, Progress, SourceFacts, Span, TrimError, VideoEncoder,
    PROBE_PROGRAM, PROGRAM,
};
use fluorita_core::SaveChoice;
use fluorita_engine::{land_file, Bin, EngineError, Landed};

/// How often the worker looks at the child and the cancellation.
const POLL: Duration = Duration::from_millis(25);
/// The longest line read from the child; the rest of a longer one is
/// skipped. Its output is treated as hostile like any other input.
const MAX_LINE_BYTES: u64 = 4096;
/// The most of the child's last error line kept for the log.
pub(super) const MAX_DETAIL_CHARS: usize = 300;
/// The most read of what a short child writes: `-encoders` is a few tens of
/// kilobytes, a film's description far less.
const MAX_LISTING_BYTES: u64 = 1024 * 1024;
/// How long a short child (the encoder list, a film's description) may take
/// before it is killed: a film on a stalled mount must not hold a worker.
const SHORT_DEADLINE: Duration = Duration::from_secs(10);
/// Where render nodes live.
const DRI: &str = "/dev/dri";
/// The longest part of the original's name the hidden name keeps, so the
/// hidden name stays within a file name's 255 bytes.
const HIDDEN_NAME_BYTES: usize = 200;

/// How the child is found: `ffmpeg` on `PATH`, unless a test names another
/// program or another `PATH`.
#[derive(Clone, Debug)]
pub(crate) struct Tool {
    pub(crate) program: OsString,
    /// The `PATH` the child is looked up on and runs with; `None` inherits.
    pub(crate) search_path: Option<OsString>,
}

impl Tool {
    /// The system's `ffmpeg`.
    pub(crate) fn system() -> Self {
        Self {
            program: PROGRAM.into(),
            search_path: None,
        }
    }

    /// The system's `ffprobe`, from the same package.
    pub(crate) fn probe() -> Self {
        Self {
            program: PROBE_PROGRAM.into(),
            search_path: None,
        }
    }

    fn command(&self, arguments: &[OsString]) -> Command {
        let mut command = Command::new(&self.program);
        if let Some(path) = &self.search_path {
            command.env("PATH", path);
        }
        command.args(arguments).stdin(Stdio::null());
        command
    }

    fn spawn(&self, arguments: &[OsString]) -> Result<Child, TrimError> {
        self.command(arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => TrimError::ToolMissing,
                _ => TrimError::Unstarted {
                    detail: error.to_string(),
                },
            })
    }
}

/// The encoder the suite's rule picks on this host: VA-API when a render node
/// exists and `ffmpeg` lists `h264_vaapi`, else `libx264`.
///
/// # Errors
///
/// [`TrimError::ToolMissing`] without `ffmpeg`, [`TrimError::EncoderMissing`]
/// when neither encoder can be used, [`TrimError::Failed`] when the listing
/// itself fails.
pub(crate) fn choose_encoder(tool: &Tool) -> Result<VideoEncoder, TrimError> {
    let arguments = [OsString::from("-hide_banner"), OsString::from("-encoders")];
    let listing = capture(tool, &arguments, &CancellationToken::new())?;
    let encoders = Encoders::parse(&listing);
    match VideoEncoder::choose(render_node(), encoders.h264_vaapi) {
        VideoEncoder::X264 if !encoders.libx264 => Err(TrimError::EncoderMissing),
        encoder => Ok(encoder),
    }
}

/// What `ffprobe` says of `film`: its frame, and the streams a trim leaves
/// out.
///
/// # Errors
///
/// [`TrimError::ToolMissing`] without `ffprobe`, [`TrimError::Cancelled`],
/// and [`TrimError::Failed`] when it cannot read the film or takes longer
/// than its deadline.
pub(crate) fn probe_source(
    tool: &Tool,
    film: &Path,
    cancellation: &CancellationToken,
) -> Result<SourceFacts, TrimError> {
    let argv = probe_argv(film);
    capture(tool, &argv[1..], cancellation).map(|output| SourceFacts::parse(&output))
}

/// Runs a short child to its end and returns what it wrote, bounded; kills
/// it on a cancellation or past [`SHORT_DEADLINE`].
fn capture(
    tool: &Tool,
    arguments: &[OsString],
    cancellation: &CancellationToken,
) -> Result<String, TrimError> {
    let mut child = tool.spawn(arguments)?;
    let stdout = child.stdout.take();
    let reader = std::thread::Builder::new()
        .name("fluorita-trim-read".to_owned())
        .spawn(move || {
            let mut bytes = Vec::new();
            if let Some(stdout) = stdout {
                let mut bounded = stdout.take(MAX_LISTING_BYTES);
                let _ = bounded.read_to_end(&mut bytes);
                // Whatever is past the bound is drained, so the child never
                // blocks on a full pipe.
                let _ = std::io::copy(&mut bounded.into_inner(), &mut std::io::sink());
            }
            bytes
        });
    let stderr = child.stderr.take();
    let errors = std::thread::Builder::new()
        .name("fluorita-trim-errors".to_owned())
        .spawn(move || stderr.map(last_line_of).unwrap_or_default());
    let (Ok(reader), Ok(errors)) = (reader, errors) else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(TrimError::Unstarted {
            detail: "no thread to read the child".to_owned(),
        });
    };
    let started = Instant::now();
    let status = loop {
        if cancellation.is_cancelled() || started.elapsed() > SHORT_DEADLINE {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            let _ = errors.join();
            return Err(if cancellation.is_cancelled() {
                TrimError::Cancelled
            } else {
                TrimError::Failed {
                    code: None,
                    detail: format!("no answer within {SHORT_DEADLINE:?}"),
                }
            });
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(POLL),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                let _ = errors.join();
                return Err(TrimError::Unstarted {
                    detail: error.to_string(),
                });
            }
        }
    };
    let output = reader.join().unwrap_or_default();
    let last_line = errors.join().unwrap_or_default();
    if !status.success() {
        return Err(failed(status, last_line));
    }
    Ok(String::from_utf8_lossy(&output).into_owned())
}

/// The first `/dev/dri/renderD*` node, in name order.
fn render_node() -> Option<PathBuf> {
    let mut nodes: Vec<PathBuf> = std::fs::read_dir(DRI)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| {
            use std::os::unix::ffi::OsStrExt;
            entry.file_name().as_bytes().starts_with(b"renderD")
        })
        .map(|entry| entry.path())
        .collect();
    nodes.sort();
    nodes.into_iter().next()
}

/// One trim, as the worker runs it.
pub(crate) struct TrimJob {
    /// The film, byte-exact.
    pub(crate) source: PathBuf,
    pub(crate) span: Span,
    pub(crate) encoder: VideoEncoder,
    pub(crate) choice: SaveChoice,
}

/// What a trim touches besides the film: where a replaced original goes, and
/// who hears of a copy that landed. The desktop's in the application; scratch
/// ones under test.
pub(crate) struct Seams<'a> {
    pub(crate) bin: &'a dyn Bin,
    pub(crate) adopt: &'a dyn Fn(&Path),
}

/// Why a trim did not land: refused or failed before anything took a name,
/// or failed while landing.
#[derive(Debug)]
pub(crate) enum Failure {
    Trim(TrimError),
    Land(EngineError),
}

impl From<TrimError> for Failure {
    fn from(error: TrimError) -> Self {
        Self::Trim(error)
    }
}

/// Runs one trim on the calling thread, which is the trim's worker: the
/// child, then the landing, then — for a copy — the adoption.
///
/// `on_progress` hears the share of the span written, `0.0..=1.0`, on this
/// thread, as the child reports it.
///
/// # Errors
///
/// Every [`Failure`]: the original is untouched by all of them except a
/// replacement's landing errors, which [`fluorita_engine::land_file`]
/// describes (the original is then in the Trash and the result kept, or
/// the result written and the original where it was).
pub(crate) fn run(
    job: &TrimJob,
    tool: &Tool,
    seams: &Seams<'_>,
    cancellation: &CancellationToken,
    on_progress: &dyn Fn(f64),
) -> Result<Landed, Failure> {
    if cancellation.is_cancelled() {
        return Err(TrimError::Cancelled.into());
    }
    let hidden = hidden_output(&job.source).ok_or(TrimError::NotAVideo)?;
    let attempt = |encoder: &VideoEncoder| {
        let argv = trim_argv(&job.source, &hidden, job.span, encoder);
        let outcome = encode(tool, &argv[1..], job.span, cancellation, on_progress);
        if outcome.is_err() {
            let _ = std::fs::remove_file(&hidden);
        }
        outcome
    };
    let encoded = match attempt(&job.encoder) {
        // A driver can list `h264_vaapi` and still fail to encode with it.
        // Nothing was published, so the same span is tried once in
        // software before anything is reported.
        Err(error @ (TrimError::Failed { .. } | TrimError::NoFrames))
            if matches!(job.encoder, VideoEncoder::Vaapi { .. }) =>
        {
            eprintln!("fluorita: the VA-API trim failed ({error}); trying libx264");
            attempt(&VideoEncoder::X264)
        }
        outcome => outcome,
    };
    encoded?;
    let Some(destination) = destination(&job.source, job.choice) else {
        let _ = std::fs::remove_file(&hidden);
        return Err(TrimError::NotAVideo.into());
    };
    let landed = land_file(
        &job.source,
        &hidden,
        &destination,
        job.choice,
        seams.bin,
        cancellation,
        "writing the trimmed film",
    )
    .map_err(|error| match error {
        EngineError::Cancelled => Failure::Trim(TrimError::Cancelled),
        other => Failure::Land(other),
    })?;
    if job.choice == SaveChoice::Copy {
        (seams.adopt)(&landed.written);
    }
    Ok(landed)
}

/// Runs the child to its end, or kills it on a cancellation.
fn encode(
    tool: &Tool,
    arguments: &[OsString],
    span: Span,
    cancellation: &CancellationToken,
    on_progress: &dyn Fn(f64),
) -> Result<(), TrimError> {
    let mut child = tool.spawn(arguments)?;
    let (shares, received) = mpsc::channel::<f64>();
    let stdout = child.stdout.take();
    let reader = std::thread::Builder::new()
        .name("fluorita-trim-progress".to_owned())
        .spawn(move || {
            let mut progress = Progress::new(span);
            if let Some(stdout) = stdout {
                each_line(stdout, |line| {
                    if let Some(share) = progress.read(line) {
                        let _ = shares.send(share);
                    }
                });
            }
            progress
        });
    let stderr = child.stderr.take();
    let errors = std::thread::Builder::new()
        .name("fluorita-trim-errors".to_owned())
        .spawn(move || stderr.map(last_line_of).unwrap_or_default());
    let (reader, errors) = match (reader, errors) {
        (Ok(reader), Ok(errors)) => (reader, errors),
        (reader, errors) => {
            // A thread the system refused: the child must not outlive it.
            let _ = child.kill();
            let _ = child.wait();
            if let Ok(reader) = reader {
                let _ = reader.join();
            }
            if let Ok(errors) = errors {
                let _ = errors.join();
            }
            return Err(TrimError::Unstarted {
                detail: "no thread to read the child".to_owned(),
            });
        }
    };

    let status = loop {
        for share in received.try_iter() {
            on_progress(share);
        }
        if cancellation.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            let _ = errors.join();
            return Err(TrimError::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => std::thread::sleep(POLL),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(error);
            }
        }
    };
    // The pipes close with the child, so both readers end.
    let progress = reader.join().ok();
    let last_line = errors.join().unwrap_or_default();
    for share in received.try_iter() {
        on_progress(share);
    }
    let status = status.map_err(|error| TrimError::Unstarted {
        detail: error.to_string(),
    })?;
    if !status.success() {
        return Err(failed(status, last_line));
    }
    if cancellation.is_cancelled() {
        return Err(TrimError::Cancelled);
    }
    if progress.is_none_or(|progress| progress.frames() == 0) {
        return Err(TrimError::NoFrames);
    }
    Ok(())
}

fn failed(status: ExitStatus, detail: String) -> TrimError {
    TrimError::Failed {
        code: status.code(),
        detail,
    }
}

/// Calls `each` with every line `reader` yields, each at most
/// [`MAX_LINE_BYTES`]; the rest of a longer line is skipped.
fn each_line(reader: impl Read, mut each: impl FnMut(&str)) {
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    loop {
        line.clear();
        match (&mut reader)
            .take(MAX_LINE_BYTES)
            .read_until(b'\n', &mut line)
        {
            Ok(0) | Err(_) => return,
            Ok(_) => {
                let complete = line.ends_with(b"\n");
                each(&String::from_utf8_lossy(&line));
                if !complete && !skip_line(&mut reader) {
                    return;
                }
            }
        }
    }
}

/// Reads past the end of an overlong line. `false` at the end of input.
fn skip_line(reader: &mut impl BufRead) -> bool {
    let mut rest = Vec::new();
    loop {
        rest.clear();
        match reader.take(MAX_LINE_BYTES).read_until(b'\n', &mut rest) {
            Ok(0) | Err(_) => return false,
            Ok(_) if rest.ends_with(b"\n") => return true,
            Ok(_) => {}
        }
    }
}

/// The last non-empty line of the child's standard error, bounded. Its
/// statistics end in carriage returns, so those end a line too.
fn last_line_of(reader: impl Read) -> String {
    let mut last = String::new();
    each_line(reader, |line| {
        if let Some(segment) = line
            .split(['\r', '\n'])
            .map(str::trim)
            .rfind(|segment| !segment.is_empty())
        {
            last = segment.chars().take(MAX_DETAIL_CHARS).collect();
        }
    });
    last
}

/// `.<name>.trim-<pid>-<n>.mp4` beside `source`, `n` counting this
/// process's trims; `None` for a path with no name.
pub(crate) fn hidden_output(source: &Path) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = source.file_name()?.as_bytes();
    // Cut on a byte, not a character: a name is bytes, and this one is
    // private. Two long names can share the bytes kept; the process id and
    // the counter are what keep two hidden names apart.
    let kept = &name[..name.len().min(HIDDEN_NAME_BYTES)];
    let mut hidden = OsString::from(".");
    hidden.push(OsStr::from_bytes(kept));
    hidden.push(format!(
        ".trim-{}-{}.mp4",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    Some(source.with_file_name(hidden))
}

/// Where the result lands: a copy takes the next free `name (editado).mp4`
/// beside the original; a replacement takes the original's own name when it
/// is an MP4 already, else its name as `.mp4` (or the next free one), and the
/// original goes to the Trash either way. The extension is put on before the
/// free-name search, for the reason the picture editor learned: searching
/// under one extension and writing another overwrites a file.
fn destination(source: &Path, choice: SaveChoice) -> Option<PathBuf> {
    let directory = source.parent()?;
    let target = Path::new(source.file_name()?).with_extension("mp4");
    let free = || {
        siderita_ops::next_available(
            directory,
            target.as_os_str(),
            crate::editor::copy::COPY_MARKER,
            siderita_ops::NameShape::File,
        )
    };
    match choice {
        SaveChoice::Copy => Some(free()),
        SaveChoice::Replace => {
            let is_mp4 = source
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"));
            if is_mp4 {
                return Some(source.to_path_buf());
            }
            let renamed = directory.join(&target);
            Some(if renamed.exists() { free() } else { renamed })
        }
    }
}

#[cfg(test)]
mod tests {
    //! The trim worker against the real `ffmpeg` and the committed fixture: a
    //! hidden file written and published, both outcomes, a cancel, a missing
    //! tool, a failing child.

    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use celestina_core::CancellationToken;
    use fluorita_core::trim::{Span, TrimError, VideoEncoder};
    use fluorita_core::SaveChoice;
    use fluorita_engine::Bin;

    use super::{choose_encoder, hidden_output, probe_source, run, Failure, Seams, Tool, TrimJob};
    use crate::test_support::Scratch;

    /// One frame at 30 fps: what the measured duration may differ by.
    const ONE_FRAME: f64 = 0.034;

    /// A Trash that is a folder in the test's scratch space.
    struct ScratchBin(PathBuf);

    impl Bin for ScratchBin {
        fn send(
            &self,
            path: &Path,
            _cancellation: &CancellationToken,
        ) -> Result<PathBuf, siderita_ops::OpError> {
            std::fs::create_dir_all(&self.0)
                .map_err(|error| siderita_ops::OpError::io(path, &error))?;
            let trashed = self.0.join(path.file_name().unwrap_or_default());
            std::fs::rename(path, &trashed)
                .map_err(|error| siderita_ops::OpError::io(path, &error))?;
            Ok(trashed)
        }
    }

    fn span(start: f64, end: f64) -> Span {
        Span::new(
            Duration::from_secs_f64(start),
            Duration::from_secs_f64(end),
            Duration::from_secs(3),
        )
        .expect("a span of the fixture")
    }

    /// The container's duration as `ffprobe` reads it.
    fn probed_duration(path: &Path) -> f64 {
        let output = std::process::Command::new("ffprobe")
            .args(["-v", "error", "-show_entries", "format=duration"])
            .args(["-of", "default=noprint_wrappers=1:nokey=1"])
            .arg(path)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("ffprobe runs");
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .expect("a duration")
    }

    /// The names in `folder`, sorted, the Trash folder left out.
    fn names(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(folder)
            .expect("the folder")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|name| name != "trash")
            .collect();
        names.sort();
        names
    }

    struct Recorded {
        adopted: Mutex<Vec<PathBuf>>,
        progress: RefCell<Vec<f64>>,
        hidden_seen: RefCell<bool>,
    }

    impl Recorded {
        fn new() -> Self {
            Self {
                adopted: Mutex::new(Vec::new()),
                progress: RefCell::new(Vec::new()),
                hidden_seen: RefCell::new(false),
            }
        }
    }

    fn trim(
        film: &Path,
        span: Span,
        encoder: VideoEncoder,
        choice: SaveChoice,
        trash: &Path,
        recorded: &Recorded,
    ) -> Result<fluorita_engine::Landed, Failure> {
        let bin = ScratchBin(trash.to_path_buf());
        let adopt = |copy: &Path| {
            recorded
                .adopted
                .lock()
                .expect("adopted")
                .push(copy.to_path_buf())
        };
        let folder = film.parent().expect("a folder").to_path_buf();
        let prefix = format!(
            ".{}.trim-",
            film.file_name().expect("a name").to_string_lossy()
        );
        let on_progress = |share: f64| {
            recorded.progress.borrow_mut().push(share);
            let hidden = std::fs::read_dir(&folder)
                .expect("the folder")
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().starts_with(&prefix));
            if hidden {
                *recorded.hidden_seen.borrow_mut() = true;
            }
        };
        let job = TrimJob {
            source: film.to_path_buf(),
            span,
            encoder,
            choice,
        };
        run(
            &job,
            &Tool::system(),
            &Seams {
                bin: &bin,
                adopt: &adopt,
            },
            &CancellationToken::new(),
            &on_progress,
        )
    }

    #[test]
    fn the_fixture_trimmed_to_one_second_lands_beside_it_and_is_adopted() {
        let scratch = Scratch::new("trim-copy");
        let film = scratch.video("folder/clip.mp4");
        let original = std::fs::read(&film).expect("the original");
        let recorded = Recorded::new();

        let landed = trim(
            &film,
            span(1.0, 2.0),
            VideoEncoder::X264,
            SaveChoice::Copy,
            &scratch.path().join("trash"),
            &recorded,
        )
        .expect("the trim lands");

        assert_eq!(landed.written, film.with_file_name("clip (editado).mp4"));
        assert!(landed.trashed_original.is_none());
        let duration = probed_duration(&landed.written);
        eprintln!("x264 trim of [1.000, 2.000): {duration:.6} s");
        assert!((duration - 1.0).abs() <= ONE_FRAME, "{duration}");
        assert_eq!(std::fs::read(&film).expect("the original"), original);
        assert!(
            *recorded.hidden_seen.borrow(),
            "the child wrote a hidden file first"
        );
        assert_eq!(
            names(film.parent().expect("a folder")),
            vec!["clip (editado).mp4".to_owned(), "clip.mp4".to_owned()],
            "the hidden file took its name; nothing else is left"
        );
        let progress = recorded.progress.borrow();
        assert!(progress.iter().all(|share| (0.0..=1.0).contains(share)));
        assert_eq!(progress.last().copied(), Some(1.0));
        assert_eq!(
            *recorded.adopted.lock().expect("adopted"),
            std::slice::from_ref(&landed.written)
        );
    }

    #[test]
    fn the_edited_only_outcome_replaces_the_film_and_trashes_the_original() {
        let scratch = Scratch::new("trim-replace");
        let film = scratch.video("clip.mp4");
        let original = std::fs::read(&film).expect("the original");
        let recorded = Recorded::new();

        let landed = trim(
            &film,
            span(0.5, 2.5),
            VideoEncoder::X264,
            SaveChoice::Replace,
            &scratch.path().join("trash"),
            &recorded,
        )
        .expect("the trim lands");

        assert_eq!(landed.written, film);
        let duration = probed_duration(&film);
        assert!((duration - 2.0).abs() <= ONE_FRAME, "{duration}");
        let trashed = landed
            .trashed_original
            .expect("the original is in the Trash");
        assert_eq!(std::fs::read(trashed).expect("the original"), original);
        assert_eq!(names(scratch.path()), vec!["clip.mp4".to_owned()]);
        assert!(recorded.adopted.lock().expect("adopted").is_empty());
    }

    #[test]
    fn the_suite_rule_trims_the_fixture_on_this_host() {
        let scratch = Scratch::new("trim-rule");
        let film = scratch.video("clip.mp4");
        let encoder = choose_encoder(&Tool::system()).expect("an H.264 encoder on this host");
        let landed = trim(
            &film,
            span(1.0, 2.0),
            encoder.clone(),
            SaveChoice::Copy,
            &scratch.path().join("trash"),
            &Recorded::new(),
        )
        .expect("the trim lands");
        let duration = probed_duration(&landed.written);
        eprintln!("{encoder:?} trim of [1.000, 2.000): {duration:.6} s");
        assert!((duration - 1.0).abs() <= ONE_FRAME, "{duration}");
    }

    #[test]
    fn a_cancel_mid_run_kills_the_child_and_leaves_no_file() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = Scratch::new("trim-cancel");
        let film = scratch.video("clip.mp4");
        // A child that writes its output, then never finishes.
        let slow = scratch.path().join("slow-ffmpeg");
        std::fs::write(
            &slow,
            "#!/bin/sh\nfor last; do :; done\nprintf partial > \"$last\"\nexec sleep 30\n",
        )
        .expect("the slow child");
        std::fs::set_permissions(&slow, std::fs::Permissions::from_mode(0o755))
            .expect("executable");
        let tool = Tool {
            program: slow.into_os_string(),
            search_path: None,
        };
        let cancellation = CancellationToken::new();
        let cancel_later = {
            let cancellation = cancellation.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(300));
                cancellation.cancel();
            })
        };
        let bin = ScratchBin(scratch.path().join("trash"));
        let started = Instant::now();
        let outcome = run(
            &TrimJob {
                source: film.clone(),
                span: span(1.0, 2.0),
                encoder: VideoEncoder::X264,
                choice: SaveChoice::Replace,
            },
            &tool,
            &Seams {
                bin: &bin,
                adopt: &|_: &Path| panic!("nothing is adopted"),
            },
            &cancellation,
            &|_| {},
        );
        cancel_later.join().expect("the canceller");

        assert!(matches!(outcome, Err(Failure::Trim(TrimError::Cancelled))));
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the child was killed"
        );
        assert_eq!(
            names(scratch.path()),
            vec!["clip.mp4".to_owned(), "slow-ffmpeg".to_owned()]
        );
    }

    #[test]
    fn a_cancel_before_the_result_takes_its_name_publishes_nothing() {
        let scratch = Scratch::new("trim-cancel-late");
        let film = scratch.video("clip.mp4");
        let original = std::fs::read(&film).expect("the original");
        let cancellation = CancellationToken::new();
        let bin = ScratchBin(scratch.path().join("trash"));
        let outcome = run(
            &TrimJob {
                source: film.clone(),
                span: span(1.0, 2.0),
                encoder: VideoEncoder::X264,
                choice: SaveChoice::Replace,
            },
            &Tool::system(),
            &Seams {
                bin: &bin,
                adopt: &|_: &Path| {},
            },
            &cancellation,
            // Cancelled on the first report, wherever the child is by then.
            &|_| cancellation.cancel(),
        );

        assert!(matches!(outcome, Err(Failure::Trim(TrimError::Cancelled))));
        assert_eq!(names(scratch.path()), vec!["clip.mp4".to_owned()]);
        assert_eq!(std::fs::read(&film).expect("the original"), original);
    }

    #[test]
    fn no_ffmpeg_on_the_path_is_a_typed_error_and_the_original_is_untouched() {
        let scratch = Scratch::new("trim-missing");
        let film = scratch.video("clip.mp4");
        let original = std::fs::read(&film).expect("the original");
        let emptied = Tool {
            program: fluorita_core::trim::PROGRAM.into(),
            search_path: Some("".into()),
        };

        assert_eq!(choose_encoder(&emptied), Err(TrimError::ToolMissing));
        let bin = ScratchBin(scratch.path().join("trash"));
        let outcome = run(
            &TrimJob {
                source: film.clone(),
                span: span(1.0, 2.0),
                encoder: VideoEncoder::X264,
                choice: SaveChoice::Replace,
            },
            &emptied,
            &Seams {
                bin: &bin,
                adopt: &|_: &Path| {},
            },
            &CancellationToken::new(),
            &|_| {},
        );

        let Err(Failure::Trim(error)) = outcome else {
            panic!("not a typed trim error: {outcome:?}");
        };
        assert_eq!(error, TrimError::ToolMissing);
        assert!(!error.message_es().is_empty());
        assert_eq!(std::fs::read(&film).expect("the original"), original);
        assert_eq!(names(scratch.path()), vec!["clip.mp4".to_owned()]);
    }

    #[test]
    fn a_child_that_fails_reports_its_last_line_and_leaves_no_file() {
        let scratch = Scratch::new("trim-failed");
        let film = scratch.path().join("not-a-film.mp4");
        std::fs::write(&film, b"plain words, no film").expect("a broken film");
        let bin = ScratchBin(scratch.path().join("trash"));
        let outcome = run(
            &TrimJob {
                source: film.clone(),
                span: span(1.0, 2.0),
                encoder: VideoEncoder::X264,
                choice: SaveChoice::Replace,
            },
            &Tool::system(),
            &Seams {
                bin: &bin,
                adopt: &|_: &Path| {},
            },
            &CancellationToken::new(),
            &|_| {},
        );

        let Err(Failure::Trim(TrimError::Failed { code, detail })) = outcome else {
            panic!("not a failed child: {outcome:?}");
        };
        assert_ne!(code, Some(0));
        assert!(!detail.is_empty());
        assert!(detail.chars().count() <= super::MAX_DETAIL_CHARS);
        assert_eq!(names(scratch.path()), vec!["not-a-film.mp4".to_owned()]);
    }

    #[test]
    fn the_hidden_output_sits_beside_the_original_and_stays_a_legal_name() {
        use std::os::unix::ffi::OsStrExt;

        let pid = std::process::id();
        let first = hidden_output(Path::new("/v/clip.mkv")).expect("a name");
        let name = first
            .file_name()
            .expect("a name")
            .to_string_lossy()
            .into_owned();
        assert!(first.starts_with("/v"));
        assert!(
            name.starts_with(&format!(".clip.mkv.trim-{pid}-")),
            "{name}"
        );
        assert!(name.ends_with(".mp4"), "{name}");
        assert_ne!(
            hidden_output(Path::new("/v/clip.mkv")),
            Some(first),
            "every job has a name of its own"
        );
        // 250 bytes of two-byte characters: a name as long as one can be. Two
        // names that share the 200 bytes kept still get two hidden names.
        let long = format!("/v/{}a.mp4", "\u{e9}".repeat(124));
        let other = format!("/v/{}b.mp4", "\u{e9}".repeat(124));
        let hidden = hidden_output(Path::new(&long)).expect("a name");
        assert!(hidden.file_name().expect("a name").as_bytes().len() <= 255);
        assert_ne!(hidden_output(Path::new(&other)), Some(hidden));
        assert!(hidden_output(Path::new("/")).is_none());
    }

    /// A child that stands for an `ffmpeg` whose VA-API encode fails: it
    /// records each call and runs the real `ffmpeg` for anything else.
    fn failing_vaapi(scratch: &Scratch) -> (Tool, PathBuf) {
        use std::os::unix::fs::PermissionsExt;

        let calls = scratch.path().join("calls");
        let fake = scratch.path().join("va-broken-ffmpeg");
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\necho \"$*\" >> '{}'\ncase \"$*\" in *h264_vaapi*) echo 'Failed to initialise VAAPI connection' >&2; exit 1;; esac\nexec ffmpeg \"$@\"\n",
                calls.display()
            ),
        )
        .expect("the fake child");
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755))
            .expect("executable");
        (
            Tool {
                program: fake.into_os_string(),
                search_path: None,
            },
            calls,
        )
    }

    #[test]
    fn a_vaapi_run_that_fails_is_retried_once_with_x264() {
        let scratch = Scratch::new("trim-va-fallback");
        let film = scratch.video("films/clip.mp4");
        let (tool, calls) = failing_vaapi(&scratch);
        let bin = ScratchBin(scratch.path().join("trash"));
        let landed = run(
            &TrimJob {
                source: film.clone(),
                span: span(1.0, 2.0),
                encoder: VideoEncoder::Vaapi {
                    device: PathBuf::from("/dev/dri/renderD128"),
                },
                choice: SaveChoice::Copy,
            },
            &tool,
            &Seams {
                bin: &bin,
                adopt: &|_: &Path| {},
            },
            &CancellationToken::new(),
            &|_| {},
        )
        .expect("x264 lands what VA-API could not");

        let calls = std::fs::read_to_string(calls).expect("the calls");
        let calls: Vec<&str> = calls.lines().collect();
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert!(calls[0].contains("h264_vaapi"));
        assert!(calls[1].contains("libx264"));
        let duration = probed_duration(&landed.written);
        assert!((duration - 1.0).abs() <= ONE_FRAME, "{duration}");
        assert_eq!(
            names(film.parent().expect("a folder")),
            vec!["clip (editado).mp4".to_owned(), "clip.mp4".to_owned()]
        );
    }

    #[test]
    fn an_x264_run_that_fails_is_not_retried() {
        let scratch = Scratch::new("trim-x264-failed");
        let film = scratch.path().join("not-a-film.mp4");
        std::fs::write(&film, b"plain words").expect("a broken film");
        let (tool, calls) = failing_vaapi(&scratch);
        let bin = ScratchBin(scratch.path().join("trash"));
        let outcome = run(
            &TrimJob {
                source: film,
                span: span(1.0, 2.0),
                encoder: VideoEncoder::X264,
                choice: SaveChoice::Copy,
            },
            &tool,
            &Seams {
                bin: &bin,
                adopt: &|_: &Path| {},
            },
            &CancellationToken::new(),
            &|_| {},
        );
        assert!(matches!(
            outcome,
            Err(Failure::Trim(TrimError::Failed { .. }))
        ));
        assert_eq!(
            std::fs::read_to_string(calls)
                .expect("the calls")
                .lines()
                .count(),
            1
        );
    }

    #[test]
    fn a_film_without_sound_trims_to_a_film_without_sound() {
        let scratch = Scratch::new("trim-silent");
        let source = scratch.video("source.mp4");
        let silent = scratch.path().join("silent.mp4");
        let made = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-nostdin", "-loglevel", "error", "-i"])
            .arg(&source)
            .args(["-an", "-c", "copy"])
            .arg(&silent)
            .stdin(std::process::Stdio::null())
            .status()
            .expect("ffmpeg runs");
        assert!(made.success());
        let landed = trim(
            &silent,
            span(1.0, 2.0),
            VideoEncoder::X264,
            SaveChoice::Copy,
            &scratch.path().join("trash"),
            &Recorded::new(),
        )
        .expect("a silent film trims");
        let duration = probed_duration(&landed.written);
        assert!((duration - 1.0).abs() <= ONE_FRAME, "{duration}");
        let streams = std::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "stream=codec_type",
                "-of",
                "csv=p=0",
            ])
            .arg(&landed.written)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("ffprobe runs");
        assert_eq!(String::from_utf8_lossy(&streams.stdout).trim(), "video");
    }

    #[test]
    fn the_probe_gives_the_film_s_frame_and_what_a_trim_leaves_out() {
        let scratch = Scratch::new("trim-probe");
        let film = scratch.video("clip.mp4");
        let facts = probe_source(&Tool::probe(), &film, &CancellationToken::new())
            .expect("the fixture is described");
        assert_eq!(facts.frame(), Some(Duration::from_nanos(33_333_333)));
        assert_eq!(facts.left_out(), 0);

        // Two audio tracks: the second is left out.
        let doubled = scratch.path().join("doubled.mkv");
        let made = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-nostdin", "-loglevel", "error", "-i"])
            .arg(&film)
            .args(["-map", "0:v", "-map", "0:a", "-map", "0:a", "-c", "copy"])
            .arg(&doubled)
            .stdin(std::process::Stdio::null())
            .status()
            .expect("ffmpeg runs");
        assert!(made.success());
        let facts = probe_source(&Tool::probe(), &doubled, &CancellationToken::new())
            .expect("the copy is described");
        assert_eq!(facts.left_out(), 1);

        let missing = Tool {
            program: fluorita_core::trim::PROBE_PROGRAM.into(),
            search_path: Some("".into()),
        };
        assert_eq!(
            probe_source(&missing, &film, &CancellationToken::new()),
            Err(TrimError::ToolMissing)
        );
    }
}
