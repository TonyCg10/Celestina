//! The wallpaper worker (CEL-2-E): every blocking step of the duty, off the
//! event loop's thread.
//!
//! One named thread behind the [`Worker`] guard takes [`WallpaperCommand`]s
//! from a bounded queue and publishes [`WallpaperEvent`]s as
//! [`Message::Wallpaper`]. It owns the folder scan (at most
//! [`MAX_FOLDER_ENTRIES`] entries of one directory, regular files with a
//! showable name, never recursive), the choice per output
//! (`celestina_shell::wallpaper::choose`), the bounded decode scaled to the
//! requested device size, and the owner check: one blocking niri request
//! `Layers` (2 s read and write timeouts, one bounded line) that says whether another client already holds an output's
//! `Background` layer (design decision 2: asked when the duty turns on, when an
//! output appears and on `wallpaper retry`; never polled).
//!
//! Every result carries the generation it was asked for, so the loop drops
//! what a newer request has outdated. Bursts are coalesced: of several
//! `Resolve`s for one output only the last runs. The three effects (folder,
//! compositor, decoder) sit behind [`Environment`] so tests use fakes.

use std::collections::{BTreeSet, HashSet};
use std::io::{self, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::Duration;

use celestina_shell::bounded::{bounded, MAX_TEXT_UNITS};
use celestina_shell::settings::WallpaperSettings;
use celestina_shell::wallpaper::{choose, is_showable, Choice, Mode};
use celestina_wayland::Sender;
use niri_ipc::socket::SOCKET_PATH_ENV;
use niri_ipc::{Layer, Reply, Request, Response};

use super::niri::{read_line_bounded, LineRead, SessionError};
use super::Worker;
use crate::message::Message;
use crate::wallpaper::decode::{decode, DecodeError, Decoded, EXTENSIONS};
use crate::wallpaper::thumbnail::{derive, Thumbnail};
use crate::wallpaper::NAMESPACE;

/// Commands that may wait for the worker; the loop gets a refusal beyond it.
pub const COMMAND_QUEUE: usize = 32;
/// The most showable names one scan offers (the smallest in sort order).
pub const MAX_FOLDER_ENTRIES: usize = 512;
/// The most raw directory entries one scan reads, showable or not.
pub const MAX_RAW_ENTRIES: usize = 8192;
/// How long the compositor may take to read or answer one `Layers` request.
pub const LAYERS_TIMEOUT: Duration = Duration::from_secs(2);
/// How often an idle worker looks at the shutdown flag.
const POLL: Duration = Duration::from_millis(100);

/// What the loop asks of the worker.
#[derive(Clone, Debug, PartialEq)]
pub enum WallpaperCommand {
    /// Choose and decode the picture of `output` at `device_px`.
    Resolve {
        output: String,
        device_px: (u32, u32),
        /// Echoed in every event this request produces.
        generation: u64,
        /// The settings as they were when the request was made.
        settings: WallpaperSettings,
    },
    /// Read the folder again and publish [`WallpaperEvent::Folder`].
    Rescan { folder: Option<PathBuf> },
    /// Ask the compositor whether another client holds these outputs'
    /// `Background` layer.
    Check { outputs: Vec<String> },
}

/// What the worker tells the loop.
#[derive(Clone, Debug, PartialEq)]
pub enum WallpaperEvent {
    /// The picture, exactly the requested device size.
    Decoded {
        output: String,
        generation: u64,
        image: Decoded,
        /// The blurred thumbnail the session lock may show (CEL-2-H-b),
        /// derived here on the worker from this decode; `None` when the
        /// picture could not be made small.
        thumbnail: Option<Thumbnail>,
    },
    /// Nothing to show on `output`: no folder, or no showable file for it.
    Fallback { output: String, generation: u64 },
    /// The chosen file or the folder could not be used.
    Failed {
        output: String,
        generation: u64,
        why: String,
    },
    /// Another client (`by`, its layer namespace) holds `output`'s
    /// `Background` layer; the shell takes no surface there.
    Held { output: String, by: String },
    /// `output` has no foreign `Background` surface (answer to `Check`).
    Free { output: String },
    /// The showable file names of the folder, sorted.
    Folder { names: Vec<String> },
}

/// A surface in the `Background` layer, as the compositor reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundSurface {
    pub output: String,
    pub namespace: String,
}

/// Why the compositor could not say which surfaces exist.
#[derive(Debug, thiserror::Error)]
pub enum LayersError {
    /// No socket, or the exchange failed.
    #[error("cannot ask niri for its layer surfaces")]
    Io(#[source] io::Error),
    /// `NIRI_SOCKET` is not set.
    #[error("NIRI_SOCKET is not set")]
    NoSocket,
    /// niri did not answer within [`LAYERS_TIMEOUT`].
    #[error("niri did not answer the layers request in time")]
    Timeout,
    /// niri closed the connection before a whole answer.
    #[error("niri closed the connection before answering the layers request")]
    Closed,
    /// The answer line was too long or could not be read.
    #[error("cannot read niri's layers answer")]
    Read(#[source] SessionError),
    /// The request or the answer is not valid JSON.
    #[error("cannot encode or decode the layers exchange")]
    Json(#[source] serde_json::Error),
    /// niri answered with an error message.
    #[error("niri refused the layers request: {0}")]
    Refused(String),
    /// niri answered with another kind of response.
    #[error("niri answered the layers request with {0}")]
    Unexpected(String),
}

/// The worker's three effects.
pub trait Environment: Send + 'static {
    /// The showable regular files directly inside `folder`, sorted: the
    /// smallest [`MAX_FOLDER_ENTRIES`] of the first [`MAX_RAW_ENTRIES`] raw
    /// entries.
    ///
    /// # Errors
    ///
    /// The directory cannot be read.
    fn list(&mut self, folder: &Path) -> io::Result<Vec<String>>;

    /// Whether `folder` can be read at all.
    ///
    /// # Errors
    ///
    /// It is missing, not a directory or unreadable.
    fn check_folder(&mut self, folder: &Path) -> io::Result<()>;

    /// Whether `folder/name` is a regular file (following a symlink once).
    fn is_file(&mut self, folder: &Path, name: &str) -> bool;

    /// The surfaces in the `Background` layer of every output.
    ///
    /// # Errors
    ///
    /// The compositor cannot be asked.
    fn background_surfaces(&mut self) -> Result<Vec<BackgroundSurface>, LayersError>;

    /// Decodes `path` to exactly `target_px`.
    ///
    /// # Errors
    ///
    /// The file is unusable.
    fn decode(
        &mut self,
        path: &Path,
        target_px: (u32, u32),
        mode: Mode,
    ) -> Result<Decoded, DecodeError>;
}

/// The real folder, niri and decoder.
#[derive(Debug, Default)]
pub struct System;

impl Environment for System {
    fn list(&mut self, folder: &Path) -> io::Result<Vec<String>> {
        let scan = scan_folder(folder, MAX_RAW_ENTRIES, MAX_FOLDER_ENTRIES)?;
        if scan.truncated {
            tracing::warn!(
                folder = %folder.display(),
                "wallpaper: the folder is larger than the scan; some pictures are not offered"
            );
        }
        Ok(scan.names)
    }

    fn check_folder(&mut self, folder: &Path) -> io::Result<()> {
        std::fs::read_dir(folder).map(drop)
    }

    fn is_file(&mut self, folder: &Path, name: &str) -> bool {
        std::fs::metadata(folder.join(name)).is_ok_and(|meta| meta.is_file())
    }

    fn background_surfaces(&mut self) -> Result<Vec<BackgroundSurface>, LayersError> {
        let path = std::env::var_os(SOCKET_PATH_ENV)
            .map(PathBuf::from)
            .ok_or(LayersError::NoSocket)?;
        ask_layers(&path, LAYERS_TIMEOUT)
    }

    fn decode(
        &mut self,
        path: &Path,
        target_px: (u32, u32),
        mode: Mode,
    ) -> Result<Decoded, DecodeError> {
        decode(path, target_px, mode)
    }
}

/// What one folder scan found.
#[derive(Debug, PartialEq, Eq)]
struct Scan {
    names: Vec<String>,
    truncated: bool,
}

/// Reads at most `raw_cap` raw entries of `folder`, skipping any entry that
/// errors, and keeps the `keep` smallest showable regular-file names, sorted.
fn scan_folder(folder: &Path, raw_cap: usize, keep: usize) -> io::Result<Scan> {
    let mut names = BTreeSet::new();
    let mut truncated = false;
    for (read, entry) in std::fs::read_dir(folder)?.enumerate() {
        if read == raw_cap {
            truncated = true;
            break;
        }
        let Ok(entry) = entry else { continue };
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !is_showable(&name) {
            continue;
        }
        // `metadata` follows a symlink once; a directory, FIFO or device is
        // not a picture.
        if std::fs::metadata(entry.path()).is_ok_and(|meta| meta.is_file()) {
            names.insert(name);
        }
    }
    if names.len() > keep {
        truncated = true;
    }
    Ok(Scan {
        names: names.into_iter().take(keep).collect(),
        truncated,
    })
}

/// One `Layers` exchange with niri at `path`: both timeouts set, one request
/// line out, one bounded line in.
fn ask_layers(path: &Path, timeout: Duration) -> Result<Vec<BackgroundSurface>, LayersError> {
    let stream = UnixStream::connect(path).map_err(LayersError::Io)?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(LayersError::Io)?;
    let mut reader = BufReader::new(stream);
    let mut request = serde_json::to_vec(&Request::Layers).map_err(LayersError::Json)?;
    request.push(b'\n');
    reader
        .get_mut()
        .write_all(&request)
        .map_err(|error| match error.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => LayersError::Timeout,
            _ => LayersError::Io(error),
        })?;
    let mut line = Vec::new();
    match read_line_bounded(&mut reader, &mut line).map_err(LayersError::Read)? {
        LineRead::Line => {}
        LineRead::Pending => return Err(LayersError::Timeout),
        LineRead::Closed => return Err(LayersError::Closed),
    }
    let reply: Reply = serde_json::from_slice(&line).map_err(LayersError::Json)?;
    match reply {
        Ok(Response::Layers(surfaces)) => Ok(surfaces
            .into_iter()
            .filter(|surface| surface.layer == Layer::Background)
            .map(|surface| BackgroundSurface {
                output: surface.output,
                namespace: surface.namespace,
            })
            .collect()),
        Ok(other) => Err(LayersError::Unexpected(format!("{other:?}"))),
        Err(message) => Err(LayersError::Refused(bounded(&message, MAX_TEXT_UNITS))),
    }
}

/// The loop's handle on the worker: the bounded command queue.
#[derive(Debug, Clone)]
pub struct WallpaperProvider {
    commands: SyncSender<WallpaperCommand>,
}

impl WallpaperProvider {
    /// Queues a command for the worker.
    ///
    /// # Errors
    ///
    /// The command back, when [`COMMAND_QUEUE`] commands already wait or the
    /// worker has stopped.
    pub fn request(&self, command: WallpaperCommand) -> Result<(), WallpaperCommand> {
        match self.commands.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(refused) | TrySendError::Disconnected(refused)) => Err(refused),
        }
    }
}

/// Starts the worker on the real system, publishing into `sender`.
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn(
    sender: Sender<Message>,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(WallpaperProvider, Worker)> {
    spawn_with(System, shutdown, move |event| {
        sender.send(Message::Wallpaper(event)).is_ok()
    })
}

/// [`spawn`] with its effects and its sink given; `publish` returns whether
/// the event was accepted (false ends the worker).
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn_with(
    environment: impl Environment,
    shutdown: &Arc<AtomicBool>,
    publish: impl FnMut(WallpaperEvent) -> bool + Send + 'static,
) -> io::Result<(WallpaperProvider, Worker)> {
    let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
    let flag = Arc::clone(shutdown);
    let worker = Worker::spawn("wallpaper", shutdown, move || {
        run(environment, &queue, &flag, publish);
    })?;
    Ok((WallpaperProvider { commands }, worker))
}

fn run(
    mut environment: impl Environment,
    queue: &Receiver<WallpaperCommand>,
    shutdown: &AtomicBool,
    mut publish: impl FnMut(WallpaperEvent) -> bool,
) {
    let mut reported: HashSet<(String, String)> = HashSet::new();
    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        let first = match queue.recv_timeout(POLL) {
            Ok(command) => command,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let batch = coalesce(first, queue.try_iter().take(COMMAND_QUEUE));
        for command in batch {
            if shutdown.load(Ordering::Acquire) {
                return;
            }
            let events = handle(&mut environment, command, &mut reported);
            for event in events {
                if !publish(event) {
                    tracing::debug!("wallpaper: the loop is gone; the worker stops");
                    return;
                }
            }
        }
    }
}

/// Folds a burst: of several `Resolve`s for one output the last wins, one
/// `Rescan` (the last) remains, and all `Check`s merge into one (the union)
/// that sits where the FIRST `Check` was, so an output's `Held` always
/// precedes any `Decoded` a later `Resolve` could produce for it.
fn coalesce(
    first: WallpaperCommand,
    rest: impl Iterator<Item = WallpaperCommand>,
) -> Vec<WallpaperCommand> {
    let mut batch: Vec<WallpaperCommand> = Vec::new();
    for command in std::iter::once(first).chain(rest) {
        match &command {
            WallpaperCommand::Resolve { output, .. } => batch.retain(|kept| {
                !matches!(kept, WallpaperCommand::Resolve { output: other, .. } if other == output)
            }),
            WallpaperCommand::Rescan { .. } => {
                batch.retain(|kept| !matches!(kept, WallpaperCommand::Rescan { .. }));
            }
            WallpaperCommand::Check { outputs } => {
                let earlier = batch.iter_mut().find_map(|kept| match kept {
                    WallpaperCommand::Check { outputs } => Some(outputs),
                    _ => None,
                });
                if let Some(earlier) = earlier {
                    for name in outputs {
                        if !earlier.contains(name) {
                            earlier.push(name.clone());
                        }
                    }
                    continue;
                }
            }
        }
        batch.push(command);
    }
    batch
}

fn handle(
    environment: &mut impl Environment,
    command: WallpaperCommand,
    reported: &mut HashSet<(String, String)>,
) -> Vec<WallpaperEvent> {
    match command {
        WallpaperCommand::Resolve {
            output,
            device_px,
            generation,
            settings,
        } => vec![resolve(
            environment,
            output,
            device_px,
            generation,
            &settings,
        )],
        WallpaperCommand::Rescan { folder } => {
            let names = folder.as_deref().map_or_else(Vec::new, |folder| {
                environment.list(folder).unwrap_or_else(|error| {
                    tracing::warn!(folder = %folder.display(), %error, "wallpaper: cannot read the folder");
                    Vec::new()
                })
            });
            vec![WallpaperEvent::Folder { names }]
        }
        WallpaperCommand::Check { outputs } => check(environment, outputs, reported),
    }
}

/// The candidate files of one output that exist in `folder`, probed directly
/// (never searched for in a listing): the explicit entry, then
/// `<output>.<ext>` and `default.<ext>` with each showable extension in lower
/// and upper case. [`choose`] then applies the precedence.
fn candidates(
    environment: &mut impl Environment,
    folder: &Path,
    output: &str,
    explicit: Option<&str>,
) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut probe = |name: String| {
        let safe = !name.contains(['/', '\0']) && name != ".." && is_showable(&name);
        if safe && !found.contains(&name) && environment.is_file(folder, &name) {
            found.push(name);
        }
    };
    if let Some(name) = explicit {
        probe(name.to_owned());
    }
    for stem in [output, "default"] {
        for extension in EXTENSIONS {
            probe(format!("{stem}.{extension}"));
            probe(format!("{stem}.{}", extension.to_ascii_uppercase()));
        }
    }
    found
}

fn resolve(
    environment: &mut impl Environment,
    output: String,
    device_px: (u32, u32),
    generation: u64,
    settings: &WallpaperSettings,
) -> WallpaperEvent {
    let Some(folder) = settings.folder.as_deref() else {
        return WallpaperEvent::Fallback { output, generation };
    };
    if let Err(error) = environment.check_folder(folder) {
        return WallpaperEvent::Failed {
            output,
            generation,
            why: bounded(
                &format!("cannot read {}: {error}", folder.display()),
                MAX_TEXT_UNITS,
            ),
        };
    }
    let explicit = settings.outputs.get(&output).map(String::as_str);
    let names = candidates(environment, folder, &output, explicit);
    match choose(&output, explicit, &names) {
        Choice::Fallback => WallpaperEvent::Fallback { output, generation },
        Choice::Image(name) => {
            let path = folder.join(&name);
            match environment.decode(&path, device_px, settings.mode) {
                Ok(image) => {
                    // Bounded (one pass over the picture, then a fixed blur
                    // over at most 640 x 640 pixels) and on this thread.
                    let thumbnail = derive(&image);
                    WallpaperEvent::Decoded {
                        output,
                        generation,
                        image,
                        thumbnail,
                    }
                }
                Err(error) => WallpaperEvent::Failed {
                    output,
                    generation,
                    why: bounded(&crate::error_chain(&error), MAX_TEXT_UNITS),
                },
            }
        }
    }
}

fn check(
    environment: &mut impl Environment,
    outputs: Vec<String>,
    reported: &mut HashSet<(String, String)>,
) -> Vec<WallpaperEvent> {
    let surfaces = environment.background_surfaces().unwrap_or_else(|error| {
        // Nobody can be observed holding the layer, so nobody is refused.
        tracing::warn!("wallpaper: {}", crate::error_chain(&error));
        Vec::new()
    });
    outputs
        .into_iter()
        .map(|output| {
            let holder = surfaces
                .iter()
                .find(|surface| surface.output == output && surface.namespace != NAMESPACE);
            match holder {
                Some(surface) => {
                    let by = bounded(&surface.namespace, MAX_TEXT_UNITS);
                    if reported.insert((output.clone(), by.clone())) {
                        tracing::warn!(%output, holder = %by, "wallpaper: another client holds the Background layer; no surface here");
                    }
                    WallpaperEvent::Held { output, by }
                }
                None => {
                    reported.retain(|(held, _)| *held != output);
                    WallpaperEvent::Free { output }
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    type Seen = Arc<Mutex<Vec<(PathBuf, (u32, u32), Mode)>>>;

    #[derive(Default)]
    struct Fake {
        folder: Option<Vec<String>>,
        surfaces: Vec<BackgroundSurface>,
        broken: Vec<String>,
        decoded: Seen,
    }

    impl Environment for Fake {
        fn list(&mut self, _folder: &Path) -> io::Result<Vec<String>> {
            self.folder
                .clone()
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        }

        fn check_folder(&mut self, _folder: &Path) -> io::Result<()> {
            self.folder
                .as_ref()
                .map(drop)
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        }

        fn is_file(&mut self, _folder: &Path, name: &str) -> bool {
            self.folder
                .as_ref()
                .is_some_and(|names| names.iter().any(|n| n == name))
        }

        fn background_surfaces(&mut self) -> Result<Vec<BackgroundSurface>, LayersError> {
            Ok(self.surfaces.clone())
        }

        fn decode(
            &mut self,
            path: &Path,
            target_px: (u32, u32),
            mode: Mode,
        ) -> Result<Decoded, DecodeError> {
            if let Ok(mut seen) = self.decoded.lock() {
                seen.push((path.to_path_buf(), target_px, mode));
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if self.broken.iter().any(|b| b == name) {
                return Err(DecodeError::Empty {
                    path: path.to_path_buf(),
                });
            }
            Ok(Decoded {
                rgba: vec![255; (target_px.0 * target_px.1 * 4) as usize],
                width: target_px.0,
                height: target_px.1,
                source_px: (4, 4),
            })
        }
    }

    fn settings(folder: Option<&str>) -> WallpaperSettings {
        WallpaperSettings {
            folder: folder.map(PathBuf::from),
            mode: Mode::Fill,
            outputs: BTreeMap::new(),
        }
    }

    fn resolve_cmd(output: &str, generation: u64, s: WallpaperSettings) -> WallpaperCommand {
        WallpaperCommand::Resolve {
            output: output.into(),
            device_px: (4, 2),
            generation,
            settings: s,
        }
    }

    fn files(names: &[&str]) -> Option<Vec<String>> {
        Some(names.iter().map(|n| (*n).to_owned()).collect())
    }

    struct Rig {
        provider: WallpaperProvider,
        worker: Worker,
        events: Receiver<WallpaperEvent>,
    }

    fn rig(fake: Fake) -> Rig {
        let shutdown = Arc::new(AtomicBool::new(false));
        let (tx, events) = mpsc::channel();
        let (provider, worker) =
            spawn_with(fake, &shutdown, move |event| tx.send(event).is_ok()).expect("the thread");
        Rig {
            provider,
            worker,
            events,
        }
    }

    fn next(rig: &Rig) -> WallpaperEvent {
        rig.events
            .recv_timeout(Duration::from_secs(5))
            .expect("an event")
    }

    #[test]
    fn a_resolve_decodes_the_chosen_file_and_carries_its_generation() {
        let decoded = Arc::new(Mutex::new(Vec::new()));
        let rig = rig(Fake {
            folder: files(&["DP-1.png", "default.jpg"]),
            decoded: Arc::clone(&decoded),
            ..Fake::default()
        });
        let mut s = settings(Some("/pics"));
        s.mode = Mode::Fit;
        assert!(rig.provider.request(resolve_cmd("DP-1", 7, s)).is_ok());
        let WallpaperEvent::Decoded {
            output,
            generation,
            image,
            ..
        } = next(&rig)
        else {
            panic!("expected a decoded picture");
        };
        assert_eq!((output.as_str(), generation, image.width), ("DP-1", 7, 4));
        let seen = decoded.lock().expect("lock").clone();
        assert_eq!(
            seen,
            vec![(PathBuf::from("/pics/DP-1.png"), (4, 2), Mode::Fit)]
        );
        rig.worker.join();
    }

    #[test]
    fn a_decode_carries_its_thumbnail_and_a_failed_one_none() {
        let rig = rig(Fake {
            folder: files(&["DP-1.png", "HDMI-A-1.png"]),
            broken: vec!["HDMI-A-1.png".to_owned()],
            ..Fake::default()
        });
        let s = settings(Some("/pics"));
        assert!(rig
            .provider
            .request(resolve_cmd("DP-1", 1, s.clone()))
            .is_ok());
        let WallpaperEvent::Decoded {
            image, thumbnail, ..
        } = next(&rig)
        else {
            panic!("expected a decoded picture");
        };
        let thumbnail = thumbnail.expect("a thumbnail from the decode");
        assert_eq!(
            (thumbnail.width(), thumbnail.height()),
            (
                u16::try_from(image.width).expect("small"),
                u16::try_from(image.height).expect("small")
            )
        );
        assert!(rig.provider.request(resolve_cmd("HDMI-A-1", 2, s)).is_ok());
        assert!(matches!(next(&rig), WallpaperEvent::Failed { .. }));
        rig.worker.join();
    }

    #[test]
    fn an_explicit_choice_wins_and_a_stale_generation_is_still_echoed() {
        let rig = rig(Fake {
            folder: files(&["a.png", "DP-1.png"]),
            ..Fake::default()
        });
        let mut s = settings(Some("/pics"));
        s.outputs.insert("DP-1".into(), "a.png".into());
        assert!(rig.provider.request(resolve_cmd("DP-1", 1, s)).is_ok());
        // The older generation is reported as such; dropping it is the loop's.
        assert!(matches!(
            next(&rig),
            WallpaperEvent::Decoded { generation: 1, .. }
        ));
        rig.worker.join();
    }

    #[test]
    fn nothing_showable_or_no_folder_is_a_fallback() {
        let rig = rig(Fake {
            folder: files(&["notes.txt", "HDMI-A-1.png"]),
            ..Fake::default()
        });
        assert!(rig
            .provider
            .request(resolve_cmd("DP-1", 3, settings(Some("/pics"))))
            .is_ok());
        assert_eq!(
            next(&rig),
            WallpaperEvent::Fallback {
                output: "DP-1".into(),
                generation: 3
            }
        );
        assert!(rig
            .provider
            .request(resolve_cmd("DP-1", 4, settings(None)))
            .is_ok());
        assert_eq!(
            next(&rig),
            WallpaperEvent::Fallback {
                output: "DP-1".into(),
                generation: 4
            }
        );
        rig.worker.join();
    }

    #[test]
    fn a_failed_decode_is_reported_and_the_worker_keeps_going() {
        let rig = rig(Fake {
            folder: files(&["DP-1.png", "DP-2.png"]),
            broken: vec!["DP-1.png".into()],
            ..Fake::default()
        });
        let s = settings(Some("/pics"));
        assert!(rig
            .provider
            .request(resolve_cmd("DP-1", 1, s.clone()))
            .is_ok());
        assert!(matches!(
            next(&rig),
            WallpaperEvent::Failed { generation: 1, .. }
        ));
        assert!(rig.provider.request(resolve_cmd("DP-2", 2, s)).is_ok());
        assert!(matches!(next(&rig), WallpaperEvent::Decoded { .. }));
        rig.worker.join();
    }

    #[test]
    fn an_unreadable_folder_is_a_failure_not_a_fallback() {
        let rig = rig(Fake::default());
        assert!(rig
            .provider
            .request(resolve_cmd("DP-1", 1, settings(Some("/nope"))))
            .is_ok());
        assert!(matches!(next(&rig), WallpaperEvent::Failed { .. }));
        rig.worker.join();
    }

    #[test]
    fn a_foreign_background_surface_holds_the_output_and_ours_does_not() {
        let surface = |output: &str, namespace: &str| BackgroundSurface {
            output: output.into(),
            namespace: namespace.into(),
        };
        let rig = rig(Fake {
            surfaces: vec![
                surface("DP-1", "noctalia-wallpaper"),
                surface("DP-2", NAMESPACE),
            ],
            ..Fake::default()
        });
        let outputs = vec!["DP-1".to_owned(), "DP-2".to_owned(), "DP-3".to_owned()];
        assert!(rig
            .provider
            .request(WallpaperCommand::Check { outputs })
            .is_ok());
        assert_eq!(
            next(&rig),
            WallpaperEvent::Held {
                output: "DP-1".into(),
                by: "noctalia-wallpaper".into()
            }
        );
        assert_eq!(
            next(&rig),
            WallpaperEvent::Free {
                output: "DP-2".into()
            }
        );
        assert_eq!(
            next(&rig),
            WallpaperEvent::Free {
                output: "DP-3".into()
            }
        );
        rig.worker.join();
    }

    #[test]
    fn a_rescan_publishes_the_names_and_a_missing_folder_publishes_none() {
        let rig = rig(Fake {
            folder: files(&["a.png", "b.webp"]),
            ..Fake::default()
        });
        let folder = Some(PathBuf::from("/pics"));
        assert!(rig
            .provider
            .request(WallpaperCommand::Rescan { folder })
            .is_ok());
        assert_eq!(
            next(&rig),
            WallpaperEvent::Folder {
                names: vec!["a.png".into(), "b.webp".into()]
            }
        );
        assert!(rig
            .provider
            .request(WallpaperCommand::Rescan { folder: None })
            .is_ok());
        assert_eq!(next(&rig), WallpaperEvent::Folder { names: vec![] });
        rig.worker.join();
    }

    #[test]
    fn a_burst_for_one_output_keeps_only_the_last_resolve() {
        let s = settings(Some("/pics"));
        let batch = coalesce(
            resolve_cmd("DP-1", 1, s.clone()),
            vec![
                resolve_cmd("DP-2", 2, s.clone()),
                resolve_cmd("DP-1", 3, s),
                WallpaperCommand::Check {
                    outputs: vec!["DP-1".into()],
                },
                WallpaperCommand::Check {
                    outputs: vec!["DP-2".into(), "DP-1".into()],
                },
            ]
            .into_iter(),
        );
        let generations: Vec<u64> = batch
            .iter()
            .filter_map(|c| match c {
                WallpaperCommand::Resolve { generation, .. } => Some(*generation),
                _ => None,
            })
            .collect();
        assert_eq!(generations, [2, 3]);
        assert!(batch.iter().any(|c| matches!(c,
            WallpaperCommand::Check { outputs } if outputs == &["DP-1".to_owned(), "DP-2".to_owned()])));
        assert_eq!(batch.len(), 3);
    }

    #[test]
    fn coalesce_keeps_the_merged_check_before_any_resolve_that_followed_the_first() {
        let s = settings(Some("/pics"));
        let check = |name: &str| WallpaperCommand::Check {
            outputs: vec![name.to_owned()],
        };
        let batch = coalesce(
            check("A"),
            vec![
                resolve_cmd("A", 1, s.clone()),
                check("B"),
                resolve_cmd("B", 2, s),
            ]
            .into_iter(),
        );
        assert!(matches!(&batch[0], WallpaperCommand::Check { outputs }
            if outputs == &["A".to_owned(), "B".to_owned()]));
        assert_eq!(batch.len(), 3);
        assert!(batch[1..]
            .iter()
            .all(|c| matches!(c, WallpaperCommand::Resolve { .. })));
    }

    #[test]
    fn probing_finds_the_candidates_without_a_listing() {
        let rig = rig(Fake {
            folder: files(&["default.PNG", "DP-1.webp", "DP-10.png", "zzz.png"]),
            ..Fake::default()
        });
        assert!(rig
            .provider
            .request(resolve_cmd("DP-1", 1, settings(Some("/pics"))))
            .is_ok());
        assert!(matches!(next(&rig), WallpaperEvent::Decoded { .. }));
        assert!(rig
            .provider
            .request(resolve_cmd("DP-2", 2, settings(Some("/pics"))))
            .is_ok());
        assert!(matches!(next(&rig), WallpaperEvent::Decoded { .. }));
        rig.worker.join();
    }

    #[test]
    fn a_path_like_explicit_choice_is_never_probed() {
        let mut fake = Fake {
            folder: files(&["a.png"]),
            ..Fake::default()
        };
        let found = candidates(&mut fake, Path::new("/pics"), "DP-1", Some("../a.png"));
        assert!(found.is_empty());
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("celestina-wp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    #[test]
    fn the_scan_keeps_the_smallest_names_and_reports_truncation() {
        let dir = scratch("keep");
        for name in ["e.png", "c.png", "a.png", "d.png", "b.png"] {
            std::fs::write(dir.join(name), b"x").expect("a file");
        }
        let scan = scan_folder(&dir, 100, 3).expect("readable");
        let whole = scan_folder(&dir, 100, 10).expect("readable");
        let capped = scan_folder(&dir, 2, 10).expect("readable");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(scan.names, ["a.png", "b.png", "c.png"]);
        assert!(scan.truncated);
        assert!(!whole.truncated);
        assert!(capped.truncated);
        assert!(capped.names.len() <= 2);
    }

    fn serve_once(name: &str, answer: Option<&str>) -> (PathBuf, std::thread::JoinHandle<()>) {
        let dir = scratch(name);
        let path = dir.join("niri.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).expect("bind");
        let answer = answer.map(str::to_owned);
        let handle = std::thread::spawn(move || {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream);
            let mut request = String::new();
            let _ = std::io::BufRead::read_line(&mut reader, &mut request);
            match answer {
                Some(line) => {
                    let _ = reader.get_mut().write_all(line.as_bytes());
                }
                None => std::thread::sleep(Duration::from_millis(600)),
            }
        });
        (path, handle)
    }

    #[test]
    fn a_silent_niri_times_out_and_check_treats_it_as_free() {
        let (path, server) = serve_once("silent", None);
        let started = Instant::now();
        let result = ask_layers(&path, Duration::from_millis(150));
        assert!(matches!(result, Err(LayersError::Timeout)), "{result:?}");
        assert!(started.elapsed() < Duration::from_millis(550));
        let _ = server.join();
        let _ = std::fs::remove_dir_all(path.parent().expect("parent"));

        // The failure degrades to Free, like any other.
        struct Silent;
        impl Environment for Silent {
            fn list(&mut self, _: &Path) -> io::Result<Vec<String>> {
                Ok(Vec::new())
            }
            fn check_folder(&mut self, _: &Path) -> io::Result<()> {
                Ok(())
            }
            fn is_file(&mut self, _: &Path, _: &str) -> bool {
                false
            }
            fn background_surfaces(&mut self) -> Result<Vec<BackgroundSurface>, LayersError> {
                Err(LayersError::Timeout)
            }
            fn decode(&mut self, _: &Path, _: (u32, u32), _: Mode) -> Result<Decoded, DecodeError> {
                Err(DecodeError::Empty {
                    path: PathBuf::new(),
                })
            }
        }
        let events = check(&mut Silent, vec!["DP-1".into()], &mut HashSet::new());
        assert_eq!(
            events,
            [WallpaperEvent::Free {
                output: "DP-1".into()
            }]
        );
    }

    #[test]
    fn an_answering_niri_yields_its_background_surfaces() {
        let answer = serde_json::to_string(&Reply::Ok(Response::Layers(Vec::new())))
            .expect("encodes")
            + "\n";
        let (path, server) = serve_once("answer", Some(&answer));
        let result = ask_layers(&path, Duration::from_secs(2));
        let _ = server.join();
        let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
        assert!(matches!(result, Ok(surfaces) if surfaces.is_empty()));
    }

    #[test]
    fn the_real_folder_scan_keeps_only_showable_regular_files() {
        let dir = std::env::temp_dir().join(format!("celestina-wp-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("dir.png")).expect("a directory");
        for name in ["b.png", "a.JPG", ".hidden.png", "text.txt"] {
            std::fs::write(dir.join(name), b"x").expect("a file");
        }
        let names = System.list(&dir).expect("readable");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(names, ["a.JPG", "b.png"]);
    }

    #[test]
    fn shutdown_joins_an_idle_worker_and_a_full_queue_refuses() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker) =
            spawn_with(Fake::default(), &shutdown, |_| true).expect("the thread");
        let started = Instant::now();
        assert!(worker.join_within(Duration::from_secs(2)));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(shutdown.load(Ordering::Acquire));
        // The worker is gone: the queue refuses and hands the command back.
        let refused = provider.request(WallpaperCommand::Rescan { folder: None });
        assert_eq!(refused, Err(WallpaperCommand::Rescan { folder: None }));
    }
}
