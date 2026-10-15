//! Which file each output should be showing and which exact file revision Qt
//! must request for that output.
//!
//! [`celestina_shell_core::wallpaper`] owns deterministic image selection and
//! this adapter supplies bounded filesystem and image validation on its own
//! worker. It publishes independent output-to-path, file-identity and gallery
//! contracts. The identity is cache invalidation, not a visual classification:
//! foreground colours remain a presentation decision and no image pixels are
//! retained after validation.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, Metadata};
use std::io::{self, BufRead, BufReader, Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use celestina_core::{atomic_file, percent};
use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::settings::wallpaper_directory_is_valid;
use celestina_shell_core::snapshot::{
    Payload, ProviderId, MAX_PAYLOAD_KEYS, MAX_ROW_ITEMS, MAX_TEXT_UNITS,
};
use celestina_shell_core::wallpaper::{self, Choice};
use image::{ImageReader, Limits};
use rustix::fs::{open, Mode, OFlags};
use serde_json::{Map, Value};

use super::{settings, tools::lock_runtime};

pub const NAME: &str = "wallpaper";
pub const IDENTITY_NAME: &str = "wallpaper-identity";
pub const GALLERY_NAME: &str = "wallpaper-gallery";

/// Nothing here changes quickly, and a directory the person edits by hand is
/// noticed within a few seconds rather than watched with an inotify budget.
const INTERVAL: Duration = Duration::from_secs(5);
/// A wallpaper directory with more entries than this is not a wallpaper
/// directory; reading all of them would be spending the session's time on
/// somebody's downloads folder.
const MAX_ENTRIES: usize = 512;
/// Snapshot lists are bounded at the protocol boundary. Refusing the command
/// before storing it keeps the process-local state under that same contract.
const MAX_OUTPUTS: usize = MAX_PAYLOAD_KEYS;
/// The decoder never accepts a decompressed dimension beyond a large desktop
/// texture or more than this approximate working allocation.
const MAX_IMAGE_DIMENSION: u32 = 8_192;
const MAX_IMAGE_ALLOCATION: u64 = 256 * 1024 * 1024;
/// A compressed file is bounded separately because not every decoder can make
/// the allocation limit strict.
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
/// A provider list can carry at most this many flat rows. The gallery keeps a
/// bounded complete catalogue in the worker and publishes one deterministic
/// page at a time rather than exceeding the snapshot protocol.
const MAX_GALLERY_IMAGES: usize = MAX_ROW_ITEMS;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Output {
    name: String,
    geometry: Option<(u32, u32)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct OutputInventory {
    /// Monotonic within this helper process. Every accepted host inventory,
    /// including one equal to its predecessor, starts a distinct publication
    /// generation so an in-flight inspection can never be mistaken for current.
    generation: u64,
    outputs: Vec<Output>,
}

type OutputState = (Mutex<OutputInventory>, Condvar);

/// The outputs the host has told us about, in the order it named them. The
/// condition variable wakes the image worker immediately after hotplug or
/// startup without decoding on the command worker.
static OUTPUTS: OnceLock<OutputState> = OnceLock::new();

fn output_state() -> &'static OutputState {
    OUTPUTS.get_or_init(|| (Mutex::new(OutputInventory::default()), Condvar::new()))
}

fn lock_inventory(state: &OutputState) -> MutexGuard<'_, OutputInventory> {
    match state.0.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn current_inventory() -> OutputInventory {
    lock_inventory(output_state()).clone()
}

fn replace_outputs(state: &OutputState, outputs: Vec<Output>) -> Result<u64, String> {
    let mut inventory = lock_inventory(state);
    let generation = inventory
        .generation
        .checked_add(1)
        .ok_or_else(|| "wallpaper output generation is exhausted".to_owned())?;
    *inventory = OutputInventory {
        generation,
        outputs,
    };
    drop(inventory);
    state.1.notify_one();
    Ok(generation)
}

fn advance_generation(state: &OutputState) -> Result<u64, String> {
    let mut inventory = lock_inventory(state);
    let generation = inventory
        .generation
        .checked_add(1)
        .ok_or_else(|| "wallpaper output generation is exhausted".to_owned())?;
    inventory.generation = generation;
    drop(inventory);
    state.1.notify_one();
    Ok(generation)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum GalleryPublicationState {
    #[default]
    Unconfigured,
    Loading,
    Ready,
    Failed,
}

impl GalleryPublicationState {
    const fn token(self) -> &'static str {
        match self {
            Self::Unconfigured => "unconfigured",
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GalleryEntry {
    id: String,
    name: String,
    path: PathBuf,
    preview_url: String,
    revision: String,
    fingerprint: Fingerprint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GalleryRequest {
    generation: u64,
    folder: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct GalleryInventory {
    request_generation: u64,
    catalogue: u64,
    folder: Option<PathBuf>,
    state: GalleryPublicationState,
    entries: Vec<GalleryEntry>,
    page_index: usize,
    truncated: bool,
    skipped: usize,
}

impl GalleryInventory {
    fn request(&self) -> Option<GalleryRequest> {
        self.folder.as_ref().map(|folder| GalleryRequest {
            generation: self.request_generation,
            folder: folder.clone(),
        })
    }

    fn page_count(&self) -> usize {
        self.entries.len().div_ceil(MAX_GALLERY_IMAGES)
    }

    fn effective_page_index(&self) -> usize {
        self.page_index.min(self.page_count().saturating_sub(1))
    }

    fn page_number(&self) -> usize {
        if self.entries.is_empty() {
            0
        } else {
            self.effective_page_index() + 1
        }
    }

    fn page_entries(&self) -> &[GalleryEntry] {
        let start = self
            .effective_page_index()
            .saturating_mul(MAX_GALLERY_IMAGES);
        let end = start
            .saturating_add(MAX_GALLERY_IMAGES)
            .min(self.entries.len());
        self.entries.get(start..end).unwrap_or_default()
    }
}

type GalleryState = (Mutex<GalleryInventory>, Condvar);

/// The selected folder is durable in settings; this process-local inventory is
/// deliberately rebuilt because directory entries and file bytes may change
/// while the shell is stopped.
static GALLERY: OnceLock<GalleryState> = OnceLock::new();

fn gallery_state() -> &'static GalleryState {
    GALLERY.get_or_init(|| (Mutex::new(GalleryInventory::default()), Condvar::new()))
}

fn lock_gallery(state: &GalleryState) -> MutexGuard<'_, GalleryInventory> {
    match state.0.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn next_gallery_counter(current: u64) -> Result<u64, String> {
    current
        .checked_add(1)
        .ok_or_else(|| "wallpaper gallery generation is exhausted".to_owned())
}

fn file_url(path: &Path) -> Option<String> {
    if !path.is_absolute() {
        return None;
    }
    let encoded = percent::encode_qt_path(&percent::path_bytes(path));
    let url = format!("file://{encoded}");
    (url.encode_utf16().count() <= MAX_TEXT_UNITS).then_some(url)
}

fn preview_url(path: &Path, revision: &str) -> Option<String> {
    let base = file_url(path)?;
    let revision = percent::encode(revision.as_bytes());
    let url = format!("{base}#celestina-revision={revision}");
    (url.encode_utf16().count() <= MAX_TEXT_UNITS).then_some(url)
}

fn gallery_payload(inventory: &GalleryInventory) -> Payload {
    let folder = inventory
        .folder
        .as_deref()
        .and_then(Path::to_str)
        .unwrap_or_default();
    let folder_url = inventory
        .folder
        .as_deref()
        .and_then(file_url)
        .unwrap_or_default();
    let page = inventory.page_number();
    let page_count = inventory.page_count();
    let images = inventory
        .page_entries()
        .iter()
        .map(|entry| {
            Value::Object(
                [
                    ("id".to_owned(), Value::from(entry.id.clone())),
                    ("name".to_owned(), Value::from(entry.name.clone())),
                    (
                        "previewUrl".to_owned(),
                        Value::from(entry.preview_url.clone()),
                    ),
                    ("revision".to_owned(), Value::from(entry.revision.clone())),
                ]
                .into_iter()
                .collect(),
            )
        })
        .collect();

    [
        ("state".to_owned(), Value::from(inventory.state.token())),
        ("folder".to_owned(), Value::from(folder)),
        ("folderUrl".to_owned(), Value::from(folder_url)),
        (
            "catalogue".to_owned(),
            Value::from(inventory.catalogue.to_string()),
        ),
        (
            "page".to_owned(),
            Value::from(u64::try_from(page).unwrap_or(u64::MAX)),
        ),
        (
            "pageCount".to_owned(),
            Value::from(u64::try_from(page_count).unwrap_or(u64::MAX)),
        ),
        (
            "total".to_owned(),
            Value::from(u64::try_from(inventory.entries.len()).unwrap_or(u64::MAX)),
        ),
        ("hasPrevious".to_owned(), Value::from(page > 1)),
        (
            "hasNext".to_owned(),
            Value::from(page > 0 && page < page_count),
        ),
        ("images".to_owned(), Value::Array(images)),
        ("truncated".to_owned(), Value::from(inventory.truncated)),
        (
            "skipped".to_owned(),
            Value::from(u64::try_from(inventory.skipped).unwrap_or(u64::MAX)),
        ),
    ]
    .into_iter()
    .collect()
}

fn publish_gallery(
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
    inventory: &GalleryInventory,
) -> Result<(), String> {
    lock_runtime(runtime)
        .publish(id, gallery_payload(inventory))
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn set_gallery_folder_for(
    state: &GalleryState,
    folder: Option<PathBuf>,
) -> Result<GalleryInventory, String> {
    let mut inventory = lock_gallery(state);
    inventory.request_generation = next_gallery_counter(inventory.request_generation)?;
    inventory.catalogue = next_gallery_counter(inventory.catalogue)?;
    inventory.folder = folder;
    inventory.state = if inventory.folder.is_some() {
        GalleryPublicationState::Loading
    } else {
        GalleryPublicationState::Unconfigured
    };
    inventory.entries.clear();
    inventory.page_index = 0;
    inventory.truncated = false;
    inventory.skipped = 0;
    let snapshot = inventory.clone();
    drop(inventory);
    state.1.notify_one();
    Ok(snapshot)
}

/// Where the images live. One directory, chosen by convention rather than
/// configured: a setting for it would be a path this shell then has to trust,
/// and the XDG data home is already the author's own.
fn directory() -> Option<PathBuf> {
    celestina_core::xdg::data_home().map(|home| home.join("celestina/wallpapers"))
}

/// The file names in the directory, bounded and sorted.
///
/// Sorted so the choice never depends on the order the filesystem happened to
/// return, which is the same reason the core's own selection is order-free.
fn available(directory: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut names: Vec<String> = entries
        .flatten()
        .take(MAX_ENTRIES)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| wallpaper::is_showable(name))
        .collect();
    names.sort();
    names
}

fn bounded_text(value: &str) -> bool {
    !value.is_empty() && value.encode_utf16().count() <= MAX_TEXT_UNITS
}

fn parse_geometry(value: &Value) -> Option<(String, (u32, u32))> {
    let row = value.as_object()?;
    let output = row.get("output")?.as_str()?;
    if !bounded_text(output) {
        return None;
    }
    let width = u32::try_from(row.get("width")?.as_u64()?).ok()?;
    let height = u32::try_from(row.get("height")?.as_u64()?).ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    Some((output.to_owned(), (width, height)))
}

fn parse_outputs(options: &Payload) -> Result<Vec<Output>, String> {
    let listed = options
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("'{NAME}' needs the list of outputs"))?;
    if listed.len() > MAX_OUTPUTS {
        return Err(format!("'{NAME}' received too many outputs"));
    }

    let geometry_rows = options
        .get("output-geometries")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if geometry_rows.len() > MAX_OUTPUTS {
        return Err(format!("'{NAME}' received too many output geometries"));
    }
    let geometries: BTreeMap<String, (u32, u32)> =
        geometry_rows.iter().filter_map(parse_geometry).collect();

    let mut seen = BTreeSet::new();
    let mut outputs = Vec::with_capacity(listed.len());
    for value in listed {
        let Some(name) = value.as_str().filter(|name| bounded_text(name)) else {
            return Err(format!("'{NAME}' received an unusable output name"));
        };
        if !seen.insert(name.to_owned()) {
            continue;
        }
        outputs.push(Output {
            name: name.to_owned(),
            geometry: geometries.get(name).copied(),
        });
    }
    Ok(outputs)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SelectionRequest {
    output: String,
    source: PathBuf,
    target_name: String,
    expected_fingerprint: Option<Fingerprint>,
}

fn parse_selection(
    options: &Payload,
    inventory: &OutputInventory,
) -> Result<SelectionRequest, String> {
    let output = options
        .get("output")
        .and_then(Value::as_str)
        .filter(|output| bounded_text(output))
        .ok_or_else(|| format!("'{NAME}' needs a bounded output name"))?;
    if !inventory.outputs.iter().any(|known| known.name == output) {
        return Err(format!("'{NAME}' does not know the output '{output}'"));
    }

    let source_text = options
        .get("source")
        .and_then(Value::as_str)
        .filter(|source| bounded_text(source))
        .ok_or_else(|| format!("'{NAME}' needs a bounded source path"))?;
    let source = PathBuf::from(source_text);
    if !source.is_absolute() {
        return Err(format!("'{NAME}' needs an absolute source path"));
    }
    let source_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("'{NAME}' cannot use that source file name"))?;
    let target_name = wallpaper::imported_name(output, source_name)
        .ok_or_else(|| format!("'{NAME}' cannot import that image format or output name"))?;

    Ok(SelectionRequest {
        output: output.to_owned(),
        source,
        target_name,
        expected_fingerprint: None,
    })
}

fn parse_gallery_folder(options: &Payload) -> Result<(String, PathBuf), String> {
    let source = options
        .get("source")
        .and_then(Value::as_str)
        .filter(|source| wallpaper_directory_is_valid(source))
        .ok_or_else(|| format!("'{GALLERY_NAME}' needs an absolute bounded folder source"))?;
    let folder = PathBuf::from(source);
    if file_url(&folder).is_none() {
        return Err(format!("'{GALLERY_NAME}' cannot publish that folder URL"));
    }
    let metadata = fs::metadata(&folder)
        .map_err(|error| format!("cannot inspect wallpaper gallery source: {error}"))?;
    if !metadata.is_dir() {
        return Err(format!("'{GALLERY_NAME}' source is not a directory"));
    }
    Ok((source.to_owned(), folder))
}

fn read_validated_source(
    path: &Path,
    expected_fingerprint: Option<Fingerprint>,
) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("cannot inspect source: {error}"))?;
    let expected = Fingerprint::from_metadata(&metadata);
    if expected_fingerprint.is_some_and(|fingerprint| fingerprint != expected) {
        return Err("source changed after the gallery catalogue was published".to_owned());
    }
    if expected.bytes > MAX_FILE_BYTES {
        return Err(format!("source exceeds {MAX_FILE_BYTES} bytes"));
    }

    // Open once and decode the exact bounded bytes that will be imported. A
    // path could otherwise be exchanged between validation and a second open,
    // leaving the destination with bytes that were never decoded.
    let descriptor = open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|error| format!("cannot open source for import: {error}"))?;
    let file = File::from(descriptor);
    let opened = file
        .metadata()
        .map_err(|error| format!("cannot inspect opened source: {error}"))?;
    if !opened.is_file() {
        return Err("source is not a regular file".to_owned());
    }
    if Fingerprint::from_metadata(&opened) != expected {
        return Err("source changed after validation".to_owned());
    }

    let capacity = usize::try_from(expected.bytes)
        .map_err(|_| "source size does not fit this process".to_owned())?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut bounded = file.take(MAX_FILE_BYTES + 1);
    bounded
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read source: {error}"))?;
    let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if actual > MAX_FILE_BYTES {
        return Err(format!("source exceeds {MAX_FILE_BYTES} bytes"));
    }
    if actual != expected.bytes {
        return Err("source changed while being read".to_owned());
    }
    let opened_after = bounded
        .get_ref()
        .metadata()
        .map_err(|error| format!("cannot verify opened source: {error}"))?;
    if Fingerprint::from_metadata(&opened_after) != expected {
        return Err("source changed while being read".to_owned());
    }
    let after = fs::metadata(path).map_err(|error| format!("cannot verify source: {error}"))?;
    if Fingerprint::from_metadata(&after) != expected {
        return Err("source changed while being read".to_owned());
    }
    validate_reader(BufReader::new(Cursor::new(bytes.as_slice())))
        .map_err(|error| format!("cannot validate source: {error}"))?;
    Ok(bytes)
}

fn replacement_candidates(
    directory: &Path,
    request: &SelectionRequest,
    destination: &Path,
) -> Result<Vec<PathBuf>, String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("cannot inspect wallpaper directory: {error}")),
    };

    let mut candidates = Vec::new();
    for (index, entry) in entries.enumerate() {
        if index >= MAX_ENTRIES {
            return Err(format!(
                "wallpaper directory contains more than {MAX_ENTRIES} entries"
            ));
        }
        let entry = entry.map_err(|error| format!("cannot inspect wallpaper entry: {error}"))?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let path = entry.path();
        if wallpaper::belongs_to_output(&request.output, &name)
            && path != destination
            && path != request.source
        {
            candidates.push(path);
        }
    }
    candidates.sort();
    Ok(candidates)
}

fn import_selection(directory: &Path, request: &SelectionRequest) -> Result<PathBuf, String> {
    let bytes = read_validated_source(&request.source, request.expected_fingerprint)?;
    let destination = directory.join(&request.target_name);
    // Enumerate the exact files to retire before mutating anything. They are
    // removed only after the complete destination has been synced and renamed;
    // a failed import therefore leaves every previous choice untouched.
    let alternatives = replacement_candidates(directory, request, &destination)?;
    atomic_file::replace(&destination, &bytes)
        .map_err(|error| format!("cannot import wallpaper atomically: {error}"))?;

    for alternative in alternatives {
        fs::remove_file(&alternative).map_err(|error| {
            format!(
                "wallpaper was imported, but old choice '{}' could not be removed: {error}",
                alternative.display()
            )
        })?;
    }
    Ok(destination)
}

/// Records which outputs exist, persists a gallery folder or imports a selected
/// source for one output. The original `wallpaper/select` source-path verb
/// remains compatible while the gallery uses an exact catalogue token and item
/// id, so stale UI cannot redirect an import.
///
/// # Errors
///
/// Returns the requester's sentence for a verb this provider does not serve,
/// an invalid bounded request or a source that cannot be safely imported.
pub fn action(
    verb: &str,
    options: &Payload,
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
) -> Result<(), String> {
    match (id.as_str(), verb) {
        (NAME, "set-outputs") => {
            replace_outputs(output_state(), parse_outputs(options)?)?;
            publish_choices(runtime, id);
        }
        (NAME, "select") => {
            let request = parse_selection(options, &current_inventory())?;
            let destination = directory()
                .ok_or_else(|| "there is no XDG data directory for wallpapers".to_owned())?;
            import_selection(&destination, &request)?;
            // This is a new image revision even though the selected path may
            // not change. Advancing the shared generation invalidates any
            // in-flight identity result and wakes that worker immediately.
            advance_generation(output_state())?;
            publish_choices(runtime, id);
        }
        (GALLERY_NAME, "set-folder") => {
            let (source, folder) = parse_gallery_folder(options)?;
            settings::remember(move |stored| {
                stored.wallpaper_directory = Some(source);
            })?;
            let inventory = set_gallery_folder_for(gallery_state(), Some(folder))?;
            publish_gallery(runtime, id, &inventory)?;
        }
        (GALLERY_NAME, "select") => {
            let request = gallery_selection(gallery_state(), options, &current_inventory())?;
            let destination = directory()
                .ok_or_else(|| "there is no XDG data directory for wallpapers".to_owned())?;
            import_selection(&destination, &request)?;
            advance_generation(output_state())?;
            let wallpaper_id = ProviderId::new(NAME)
                .map_err(|error| format!("cannot address the wallpaper provider: {error}"))?;
            publish_choices(runtime, &wallpaper_id);
        }
        (GALLERY_NAME, "set-page") => {
            publish_gallery_page(gallery_state(), options, runtime, id)?;
        }
        (provider, _) => {
            return Err(format!("'{provider}' does not serve the verb '{verb}'"));
        }
    }
    Ok(())
}

fn selected(
    outputs: &[Output],
    directory: &Path,
    names: &[String],
) -> Vec<(Output, Option<PathBuf>)> {
    outputs
        .iter()
        .cloned()
        .map(|output| {
            let path = match wallpaper::choose(&output.name, names) {
                Choice::Image(name) => Some(directory.join(name)),
                Choice::Fallback => None,
            };
            (output, path)
        })
        .collect()
}

/// Applies one provider update only while the host inventory is still exactly
/// the one that produced it. Holding the inventory lock through the small
/// in-memory runtime mutation closes the check/publish race: `set-outputs`
/// cannot advance the generation between those two operations.
fn apply_update_if_current(
    state: &OutputState,
    expected: &OutputInventory,
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
    payload: Option<Payload>,
) -> Result<bool, String> {
    let current = lock_inventory(state);
    if &*current != expected {
        return Ok(false);
    }

    let mut runtime = lock_runtime(runtime);
    if let Some(payload) = payload {
        runtime
            .publish(id, payload)
            .map_err(|error| error.to_string())?;
    } else {
        runtime.withdraw(id);
    }
    Ok(true)
}

/// Publishes one entry per output: the absolute path to show, or `null` for an
/// output that has nothing of its own. `null` is the fallback the surface
/// paints deliberately — it is not an error and it is not another screen's
/// picture.
fn publish_choices(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    let inventory = current_inventory();
    publish_choices_for(output_state(), &inventory, runtime, id);
}

fn publish_choices_for(
    state: &OutputState,
    inventory: &OutputInventory,
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
) {
    let payload = if inventory.outputs.is_empty() {
        None
    } else if let Some(directory) = directory() {
        let names = available(&directory);
        let mut payload = Payload::new();
        for (output, path) in selected(&inventory.outputs, &directory, &names) {
            let value = path
                .as_deref()
                .and_then(Path::to_str)
                .map_or(Value::Null, Value::from);
            payload.insert(output.name, value);
        }
        Some(payload)
    } else {
        None
    };

    if let Err(error) = apply_update_if_current(state, inventory, runtime, id, payload) {
        eprintln!("celestina-provider-adapter: wallpaper: {error}");
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Fingerprint {
    bytes: u64,
    modified_ns: Option<u128>,
}

impl Fingerprint {
    fn from_metadata(metadata: &Metadata) -> Self {
        let modified_ns = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos());
        Self {
            bytes: metadata.len(),
            modified_ns,
        }
    }

    fn revision(self) -> String {
        match self.modified_ns {
            Some(modified_ns) => format!("{}:{modified_ns}", self.bytes),
            None => format!("{}:unknown", self.bytes),
        }
    }
}

fn validate_reader<R>(reader: R) -> Result<(), String>
where
    R: BufRead + Seek,
{
    let mut reader = ImageReader::new(reader)
        .with_guessed_format()
        .map_err(|error| format!("cannot identify image: {error}"))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_IMAGE_ALLOCATION);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|error| format!("cannot decode image: {error}"))?;
    Ok(())
}

fn validate_image(path: &Path, expected: Fingerprint) -> Result<(), String> {
    if expected.bytes > MAX_FILE_BYTES {
        return Err(format!("file exceeds {MAX_FILE_BYTES} bytes"));
    }

    // A selected name is still untrusted filesystem input. Opening a FIFO in
    // the usual blocking mode would stop the only wallpaper worker forever,
    // before `metadata().is_file()` had a chance to reject it. Non-blocking
    // open keeps that inspection bounded while continuing to follow ordinary
    // symlinks to regular wallpaper files.
    let descriptor = open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|error| format!("cannot open image: {error}"))?;
    let file = File::from(descriptor);
    let opened = file
        .metadata()
        .map_err(|error| format!("cannot inspect opened image: {error}"))?;
    if !opened.is_file() {
        return Err("image is not a regular file".to_owned());
    }
    if Fingerprint::from_metadata(&opened) != expected {
        return Err("image changed before decoding".to_owned());
    }

    validate_reader(BufReader::new(file))?;

    let after =
        fs::metadata(path).map_err(|error| format!("cannot verify decoded image: {error}"))?;
    if Fingerprint::from_metadata(&after) != expected {
        return Err("image changed while decoding".to_owned());
    }

    Ok(())
}

#[derive(Default)]
struct ImageInspector {
    validated: BTreeMap<PathBuf, Fingerprint>,
    // One last reported fingerprint per selected path. A corrupt file may be
    // replaced repeatedly; retaining every historical fingerprint would make
    // a bounded path into unbounded process memory.
    reported_failures: BTreeMap<PathBuf, Fingerprint>,
}

impl ImageInspector {
    fn should_report_failure(&mut self, path: &Path, fingerprint: Fingerprint) -> bool {
        self.reported_failures.insert(path.to_owned(), fingerprint) != Some(fingerprint)
    }

    fn inspect(&mut self, path: &Path) -> Result<Fingerprint, String> {
        let metadata =
            fs::metadata(path).map_err(|error| format!("cannot inspect image: {error}"))?;
        let fingerprint = Fingerprint::from_metadata(&metadata);
        let cached = self
            .validated
            .get(path)
            .is_some_and(|validated| *validated == fingerprint);
        if !cached {
            self.validated.remove(path);
            match validate_image(path, fingerprint) {
                Ok(()) => {
                    self.reported_failures.remove(path);
                    self.validated.insert(path.to_owned(), fingerprint);
                }
                Err(error) => {
                    if self.should_report_failure(path, fingerprint) {
                        eprintln!(
                            "celestina-provider-adapter: wallpaper validation: {}: {error}",
                            path.display()
                        );
                    }
                    return Err(error);
                }
            }
        }
        self.validated
            .get(path)
            .copied()
            .ok_or_else(|| "validated image identity was not retained".to_owned())
    }

    fn retain(&mut self, paths: &BTreeSet<PathBuf>) {
        self.validated.retain(|path, _| paths.contains(path));
        self.reported_failures
            .retain(|path, _| paths.contains(path));
    }
}

#[derive(Debug, PartialEq, Eq)]
struct GalleryScan {
    entries: Vec<GalleryEntry>,
    /// Compatibility for consumers that predate paging. No accepted entry is
    /// discarded when this is true; `images` is the current bounded page.
    truncated: bool,
    skipped: usize,
}

fn scan_gallery(folder: &Path, inspector: &mut ImageInspector) -> Result<GalleryScan, String> {
    let metadata = fs::metadata(folder)
        .map_err(|error| format!("cannot inspect wallpaper gallery: {error}"))?;
    if !metadata.is_dir() {
        return Err("wallpaper gallery source is not a directory".to_owned());
    }

    let directory =
        fs::read_dir(folder).map_err(|error| format!("cannot read wallpaper gallery: {error}"))?;
    let mut candidates = Vec::new();
    let mut skipped = 0usize;
    for (index, entry) in directory.enumerate() {
        if index >= MAX_ENTRIES {
            return Err(format!(
                "wallpaper gallery contains more than {MAX_ENTRIES} entries"
            ));
        }
        let entry =
            entry.map_err(|error| format!("cannot read wallpaper gallery entry: {error}"))?;
        let Ok(name) = entry.file_name().into_string() else {
            skipped = skipped.saturating_add(1);
            continue;
        };
        if !wallpaper::is_showable(&name) {
            continue;
        }
        let path = entry.path();
        let regular = fs::metadata(&path).is_ok_and(|metadata| metadata.is_file());
        if !regular
            || path
                .to_str()
                .is_none_or(|source| source.encode_utf16().count() > MAX_TEXT_UNITS)
            || file_url(&path).is_none()
        {
            skipped = skipped.saturating_add(1);
            continue;
        }
        candidates.push((name, path));
    }
    candidates.sort_by(|left, right| left.0.cmp(&right.0));

    let mut entries = Vec::with_capacity(candidates.len());
    let mut attempted = BTreeSet::new();
    for (name, path) in candidates {
        attempted.insert(path.clone());
        let Ok(fingerprint) = inspector.inspect(&path) else {
            skipped = skipped.saturating_add(1);
            continue;
        };
        let revision = fingerprint.revision();
        let Some(preview_url) = preview_url(&path, &revision) else {
            skipped = skipped.saturating_add(1);
            continue;
        };
        entries.push(GalleryEntry {
            id: name.clone(),
            name,
            path,
            preview_url,
            revision,
            fingerprint,
        });
    }
    inspector.retain(&attempted);

    Ok(GalleryScan {
        truncated: entries.len() > MAX_GALLERY_IMAGES,
        entries,
        skipped,
    })
}

fn apply_gallery_scan_if_current(
    state: &GalleryState,
    expected: &GalleryRequest,
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
    scan: Result<GalleryScan, String>,
) -> Result<bool, String> {
    let mut inventory = lock_gallery(state);
    if inventory.request().as_ref() != Some(expected) {
        return Ok(false);
    }

    let (next_state, entries, truncated, skipped) = match scan {
        Ok(scan) => (
            GalleryPublicationState::Ready,
            scan.entries,
            scan.truncated,
            scan.skipped,
        ),
        Err(error) => {
            eprintln!("celestina-provider-adapter: wallpaper gallery: {error}");
            (GalleryPublicationState::Failed, Vec::new(), false, 0)
        }
    };
    let changed = inventory.state != next_state
        || inventory.entries != entries
        || inventory.truncated != truncated
        || inventory.skipped != skipped;
    if changed {
        inventory.catalogue = next_gallery_counter(inventory.catalogue)?;
        inventory.state = next_state;
        inventory.entries = entries;
        inventory.page_index = inventory.effective_page_index();
        inventory.truncated = truncated;
        inventory.skipped = skipped;
    }
    publish_gallery(runtime, id, &inventory)?;
    Ok(true)
}

fn gallery_page_request(options: &Payload) -> Result<(u64, usize), String> {
    let catalogue = options
        .get("catalogue")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| format!("'{GALLERY_NAME}' needs a catalogue token"))?;
    let page = options
        .get("page")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|page| *page > 0)
        .ok_or_else(|| format!("'{GALLERY_NAME}' needs a positive page number"))?;
    Ok((catalogue, page))
}

/// Changes only the published slice. Holding the gallery lock through the
/// runtime mutation prevents an older page request from being published after
/// a concurrent rescan has installed a newer catalogue.
fn publish_gallery_page(
    state: &GalleryState,
    options: &Payload,
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
) -> Result<(), String> {
    let (catalogue, page) = gallery_page_request(options)?;
    let mut inventory = lock_gallery(state);
    if inventory.state != GalleryPublicationState::Ready || inventory.catalogue != catalogue {
        return Err(format!("'{GALLERY_NAME}' received a stale catalogue token"));
    }
    let page_count = inventory.page_count();
    if page > page_count {
        return Err(format!("'{GALLERY_NAME}' does not contain page {page}"));
    }
    inventory.page_index = page - 1;
    publish_gallery(runtime, id, &inventory)
}

fn gallery_selection(
    state: &GalleryState,
    options: &Payload,
    outputs: &OutputInventory,
) -> Result<SelectionRequest, String> {
    let output = options
        .get("output")
        .and_then(Value::as_str)
        .filter(|output| bounded_text(output))
        .ok_or_else(|| format!("'{GALLERY_NAME}' needs a bounded output name"))?;
    if !outputs.outputs.iter().any(|known| known.name == output) {
        return Err(format!(
            "'{GALLERY_NAME}' does not know the output '{output}'"
        ));
    }
    let catalogue = options
        .get("catalogue")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| format!("'{GALLERY_NAME}' needs a catalogue token"))?;
    let item_id = options
        .get("id")
        .and_then(Value::as_str)
        .filter(|item_id| wallpaper::is_showable(item_id))
        .ok_or_else(|| format!("'{GALLERY_NAME}' needs a gallery item id"))?;

    let inventory = lock_gallery(state);
    if inventory.state != GalleryPublicationState::Ready || inventory.catalogue != catalogue {
        return Err(format!("'{GALLERY_NAME}' received a stale catalogue token"));
    }
    let entry = inventory
        .entries
        .iter()
        .find(|entry| entry.id == item_id)
        .ok_or_else(|| format!("'{GALLERY_NAME}' does not contain that item"))?;
    let current = fs::metadata(&entry.path)
        .map(|metadata| Fingerprint::from_metadata(&metadata))
        .map_err(|_| format!("'{GALLERY_NAME}' item is no longer available"))?;
    if current != entry.fingerprint {
        return Err(format!("'{GALLERY_NAME}' received a stale gallery item"));
    }
    let target_name = wallpaper::imported_name(output, &entry.name)
        .ok_or_else(|| format!("'{GALLERY_NAME}' cannot import that item"))?;
    Ok(SelectionRequest {
        output: output.to_owned(),
        source: entry.path.clone(),
        target_name,
        expected_fingerprint: Some(entry.fingerprint),
    })
}

fn identity_row(output: &Output, path: &Path, generation: u64, revision: String) -> Option<Value> {
    let source = path.to_str()?;
    if source.encode_utf16().count() > MAX_TEXT_UNITS {
        return None;
    }

    let mut row = Map::new();
    row.insert("output".to_owned(), Value::from(output.name.clone()));
    row.insert("source".to_owned(), Value::from(source));
    row.insert("generation".to_owned(), Value::from(generation));
    row.insert("revision".to_owned(), Value::from(revision));
    if let Some((width, height)) = output.geometry {
        row.insert("width".to_owned(), Value::from(width));
        row.insert("height".to_owned(), Value::from(height));
    }
    Some(Value::Object(row))
}

fn publish_identity_for(
    state: &OutputState,
    inventory: &OutputInventory,
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
    inspector: &mut ImageInspector,
) -> Result<bool, String> {
    if inventory.outputs.is_empty() {
        inspector.retain(&BTreeSet::new());
        return apply_update_if_current(state, inventory, runtime, id, None);
    }

    let Some(directory) = directory() else {
        inspector.retain(&BTreeSet::new());
        return apply_update_if_current(state, inventory, runtime, id, None);
    };
    let names = available(&directory);
    let selected = selected(&inventory.outputs, &directory, &names);
    let current_paths = selected
        .iter()
        .filter_map(|(_, path)| path.clone())
        .collect::<BTreeSet<_>>();
    inspector.retain(&current_paths);

    let mut rows = Vec::with_capacity(selected.len());
    for (output, path) in selected {
        let Some(path) = path else {
            continue;
        };
        if let Ok(fingerprint) = inspector.inspect(&path) {
            if let Some(row) =
                identity_row(&output, &path, inventory.generation, fingerprint.revision())
            {
                rows.push(row);
            }
        }
    }

    let payload = if rows.is_empty() {
        None
    } else {
        let mut payload = Payload::new();
        payload.insert("outputs".to_owned(), Value::Array(rows));
        Some(payload)
    };
    apply_update_if_current(state, inventory, runtime, id, payload)
}

pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>) -> io::Result<()> {
    let (Ok(id), Ok(identity_id), Ok(gallery_id)) = (
        ProviderId::new(NAME),
        ProviderId::new(IDENTITY_NAME),
        ProviderId::new(GALLERY_NAME),
    ) else {
        eprintln!("celestina-provider-adapter: wallpaper: unusable provider name");
        return Ok(());
    };

    {
        let mut state = lock_runtime(runtime);
        state.register(id.clone());
        state.register(identity_id.clone());
        state.register(gallery_id.clone());
    }
    let configured = settings::current().wallpaper_directory.map(PathBuf::from);
    match set_gallery_folder_for(gallery_state(), configured) {
        Ok(inventory) => {
            if let Err(error) = publish_gallery(runtime, &gallery_id, &inventory) {
                eprintln!("celestina-provider-adapter: wallpaper gallery: {error}");
            }
        }
        Err(error) => {
            eprintln!("celestina-provider-adapter: wallpaper gallery: {error}");
        }
    }

    let wallpaper_runtime = Arc::clone(runtime);
    thread::Builder::new()
        .name(NAME.to_owned())
        .spawn(move || run(&wallpaper_runtime, &id, &identity_id))?;
    let gallery_runtime = Arc::clone(runtime);
    thread::Builder::new()
        .name(GALLERY_NAME.to_owned())
        .spawn(move || run_gallery(&gallery_runtime, &gallery_id))?;
    Ok(())
}

fn wait_for_inventory_change(
    state: &OutputState,
    observed_generation: u64,
    timeout: Duration,
) -> bool {
    let guard = lock_inventory(state);
    match state.1.wait_timeout_while(guard, timeout, |inventory| {
        inventory.generation == observed_generation
    }) {
        Ok((inventory, _)) => inventory.generation != observed_generation,
        Err(poisoned) => {
            let (inventory, _) = poisoned.into_inner();
            inventory.generation != observed_generation
        }
    }
}

fn run(runtime: &Mutex<ProviderRuntime>, id: &ProviderId, identity_id: &ProviderId) {
    let mut inspector = ImageInspector::default();
    loop {
        let inventory = current_inventory();
        publish_choices_for(output_state(), &inventory, runtime, id);
        if let Err(error) = publish_identity_for(
            output_state(),
            &inventory,
            runtime,
            identity_id,
            &mut inspector,
        ) {
            eprintln!("celestina-provider-adapter: wallpaper identity: {error}");
        }
        wait_for_inventory_change(output_state(), inventory.generation, INTERVAL);
    }
}

fn current_gallery_request(state: &GalleryState) -> (u64, Option<GalleryRequest>) {
    let inventory = lock_gallery(state);
    (inventory.request_generation, inventory.request())
}

fn wait_for_gallery_change(
    state: &GalleryState,
    observed_generation: u64,
    timeout: Duration,
) -> bool {
    let guard = lock_gallery(state);
    match state.1.wait_timeout_while(guard, timeout, |inventory| {
        inventory.request_generation == observed_generation
    }) {
        Ok((inventory, _)) => inventory.request_generation != observed_generation,
        Err(poisoned) => {
            let (inventory, _) = poisoned.into_inner();
            inventory.request_generation != observed_generation
        }
    }
}

fn run_gallery(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    let mut inspector = ImageInspector::default();
    loop {
        let (observed_generation, request) = current_gallery_request(gallery_state());
        if let Some(request) = request {
            let scan = scan_gallery(&request.folder, &mut inspector);
            if let Err(error) =
                apply_gallery_scan_if_current(gallery_state(), &request, runtime, id, scan)
            {
                eprintln!("celestina-provider-adapter: wallpaper gallery: {error}");
            }
        } else {
            inspector.retain(&BTreeSet::new());
        }
        wait_for_gallery_change(gallery_state(), observed_generation, INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "celestina-wallpaper-provider-{}-{}",
                std::process::id(),
                NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).expect("private test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_png(path: &Path) {
        let pixels = image::RgbaImage::from_pixel(2, 2, image::Rgba([24, 48, 96, 255]));
        pixels
            .save_with_format(path, image::ImageFormat::Png)
            .expect("small valid PNG");
    }

    fn output(name: &str) -> Output {
        Output {
            name: name.to_owned(),
            geometry: None,
        }
    }

    fn output_with_geometry(name: &str, width: u32, height: u32) -> Output {
        Output {
            name: name.to_owned(),
            geometry: Some((width, height)),
        }
    }

    fn state(inventory: OutputInventory) -> OutputState {
        (Mutex::new(inventory), Condvar::new())
    }

    fn gallery(inventory: GalleryInventory) -> GalleryState {
        (Mutex::new(inventory), Condvar::new())
    }

    fn options(outputs: Value) -> Payload {
        [("outputs".to_owned(), outputs)].into_iter().collect()
    }

    fn options_with_geometries(outputs: Value, geometries: Value) -> Payload {
        [
            ("outputs".to_owned(), outputs),
            ("output-geometries".to_owned(), geometries),
        ]
        .into_iter()
        .collect()
    }

    fn selection_options(output: &str, source: &Path) -> Payload {
        [
            ("output".to_owned(), Value::from(output)),
            (
                "source".to_owned(),
                Value::from(source.to_str().expect("UTF-8 test path")),
            ),
        ]
        .into_iter()
        .collect()
    }

    fn gallery_selection_options(output: &str, catalogue: u64, id: &str) -> Payload {
        [
            ("output".to_owned(), Value::from(output)),
            ("catalogue".to_owned(), Value::from(catalogue.to_string())),
            ("id".to_owned(), Value::from(id)),
        ]
        .into_iter()
        .collect()
    }

    fn gallery_page_options(catalogue: u64, page: u64) -> Payload {
        [
            ("catalogue".to_owned(), Value::from(catalogue.to_string())),
            ("page".to_owned(), Value::from(page)),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn output_geometry_is_additive_and_correlated_by_name() {
        let parsed = parse_outputs(&options_with_geometries(
            serde_json::json!(["DP-1", "HDMI-A-1"]),
            serde_json::json!([
                {"output": "HDMI-A-1", "width": 1920, "height": 1080},
                {"output": "DP-1", "width": 3440, "height": 1440}
            ]),
        ))
        .expect("bounded outputs");

        assert_eq!(
            parsed,
            vec![
                output_with_geometry("DP-1", 3440, 1440),
                output_with_geometry("HDMI-A-1", 1920, 1080),
            ]
        );
    }

    #[test]
    fn a_host_without_geometries_remains_valid() {
        assert_eq!(
            parse_outputs(&options(serde_json::json!(["winit"]))).expect("original command shape"),
            vec![output("winit")]
        );
    }

    #[test]
    fn duplicate_outputs_are_coalesced_in_host_order() {
        let parsed = parse_outputs(&options(serde_json::json!(["DP-1", "DP-1", "HDMI-A-1"])))
            .expect("duplicates are coalesced");

        assert_eq!(parsed, vec![output("DP-1"), output("HDMI-A-1")]);
    }

    #[test]
    fn selection_requires_a_known_output_and_absolute_supported_source() {
        let inventory = OutputInventory {
            generation: 7,
            outputs: vec![output("DP-1")],
        };
        let selected = parse_selection(
            &selection_options("DP-1", Path::new("/pictures/Forest.PNG")),
            &inventory,
        )
        .expect("known output and supported absolute source");
        assert_eq!(selected.output, "DP-1");
        assert_eq!(selected.target_name, "DP-1.png");

        assert!(parse_selection(
            &selection_options("HDMI-A-1", Path::new("/pictures/Forest.PNG")),
            &inventory,
        )
        .is_err());
        assert!(parse_selection(
            &selection_options("DP-1", Path::new("pictures/Forest.PNG")),
            &inventory,
        )
        .is_err());
        assert!(parse_selection(
            &selection_options("DP-1", Path::new("/pictures/Forest.svg")),
            &inventory,
        )
        .is_err());
    }

    #[test]
    fn gallery_scan_is_sorted_bounded_and_publishes_encoded_preview_urls() {
        let root = TestDirectory::new();
        let folder = root.path().join("wall papers#1");
        fs::create_dir_all(&folder).expect("gallery directory");
        write_png(&folder.join("zeta.png"));
        write_png(&folder.join("alpha image#.PNG"));
        fs::write(folder.join("broken.jpg"), b"not an image").expect("broken image");
        fs::write(folder.join("notes.txt"), b"not a candidate").expect("text file");
        fs::create_dir(folder.join("folder.webp")).expect("image-shaped directory");

        let mut inspector = ImageInspector::default();
        let scan = scan_gallery(&folder, &mut inspector).expect("bounded gallery scan");

        assert_eq!(
            scan.entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha image#.PNG", "zeta.png"]
        );
        assert!(!scan.truncated);
        assert_eq!(scan.skipped, 2);
        let preview = &scan.entries[0].preview_url;
        assert!(preview.starts_with("file://"));
        assert!(preview.contains("wall%20papers%231"));
        assert!(preview.contains("alpha%20image%23.PNG"));
        assert!(preview.contains("#celestina-revision="));

        let inventory = GalleryInventory {
            request_generation: 4,
            catalogue: 7,
            folder: Some(folder),
            state: GalleryPublicationState::Ready,
            entries: scan.entries,
            page_index: 0,
            truncated: scan.truncated,
            skipped: scan.skipped,
        };
        let payload = gallery_payload(&inventory);
        assert_eq!(payload.get("state"), Some(&Value::from("ready")));
        assert_eq!(payload.get("catalogue"), Some(&Value::from("7")));
        assert_eq!(payload.get("page"), Some(&Value::from(1u64)));
        assert_eq!(payload.get("pageCount"), Some(&Value::from(1u64)));
        assert_eq!(payload.get("total"), Some(&Value::from(2u64)));
        assert_eq!(payload.get("hasPrevious"), Some(&Value::from(false)));
        assert_eq!(payload.get("hasNext"), Some(&Value::from(false)));
        assert_eq!(payload.get("truncated"), Some(&Value::from(false)));
        assert_eq!(payload.get("skipped"), Some(&Value::from(2u64)));
        assert_eq!(
            payload
                .get("images")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn gallery_folder_command_accepts_only_an_absolute_directory() {
        let root = TestDirectory::new();
        let options = |source: &str| {
            [("source".to_owned(), Value::from(source))]
                .into_iter()
                .collect()
        };
        let path = root.path().to_str().expect("UTF-8 test path");

        assert_eq!(
            parse_gallery_folder(&options(path)),
            Ok((path.to_owned(), root.path().to_owned()))
        );
        assert!(parse_gallery_folder(&options("Pictures/Wallpapers")).is_err());
        let file = root.path().join("not-a-folder.png");
        write_png(&file);
        assert!(parse_gallery_folder(&options(file.to_str().expect("UTF-8 test path"))).is_err());
    }

    #[test]
    fn every_bounded_gallery_image_is_reachable_through_protocol_sized_pages() {
        let root = TestDirectory::new();
        for index in 0..=MAX_GALLERY_IMAGES {
            write_png(&root.path().join(format!("wallpaper-{index:03}.png")));
        }

        let mut inspector = ImageInspector::default();
        let scan = scan_gallery(root.path(), &mut inspector).expect("bounded gallery scan");

        assert_eq!(scan.entries.len(), MAX_GALLERY_IMAGES + 1);
        assert!(scan.truncated);
        assert_eq!(scan.entries[0].name, "wallpaper-000.png");
        assert_eq!(
            scan.entries.last().map(|entry| entry.name.as_str()),
            Some("wallpaper-064.png")
        );

        let gallery = gallery(GalleryInventory {
            request_generation: 1,
            catalogue: 7,
            folder: Some(root.path().to_owned()),
            state: GalleryPublicationState::Ready,
            entries: scan.entries,
            page_index: 0,
            truncated: scan.truncated,
            skipped: 0,
        });
        let id = ProviderId::new(GALLERY_NAME).expect("valid provider id");
        let mut provider_runtime = ProviderRuntime::new(1);
        provider_runtime.register(id.clone());
        let runtime = Mutex::new(provider_runtime);

        publish_gallery(&runtime, &id, &lock_gallery(&gallery))
            .expect("the full first page fits the snapshot protocol");
        let first = gallery_payload(&lock_gallery(&gallery));
        assert_eq!(first.get("page"), Some(&Value::from(1u64)));
        assert_eq!(first.get("pageCount"), Some(&Value::from(2u64)));
        assert_eq!(first.get("total"), Some(&Value::from(65u64)));
        assert_eq!(first.get("hasPrevious"), Some(&Value::from(false)));
        assert_eq!(first.get("hasNext"), Some(&Value::from(true)));
        assert_eq!(
            first.get("images").and_then(Value::as_array).map(Vec::len),
            Some(MAX_GALLERY_IMAGES)
        );

        publish_gallery_page(&gallery, &gallery_page_options(7, 2), &runtime, &id)
            .expect("second page");
        let second = gallery_payload(&lock_gallery(&gallery));
        assert_eq!(second.get("catalogue"), Some(&Value::from("7")));
        assert_eq!(second.get("page"), Some(&Value::from(2u64)));
        assert_eq!(second.get("pageCount"), Some(&Value::from(2u64)));
        assert_eq!(second.get("total"), Some(&Value::from(65u64)));
        assert_eq!(second.get("hasPrevious"), Some(&Value::from(true)));
        assert_eq!(second.get("hasNext"), Some(&Value::from(false)));
        let second_images = second
            .get("images")
            .and_then(Value::as_array)
            .expect("second page rows");
        assert_eq!(second_images.len(), 1);
        assert_eq!(
            second_images[0].get("name"),
            Some(&Value::from("wallpaper-064.png"))
        );
        let outputs = OutputInventory {
            generation: 1,
            outputs: vec![output("DP-1")],
        };
        let selected = gallery_selection(
            &gallery,
            &gallery_selection_options("DP-1", 7, "wallpaper-064.png"),
            &outputs,
        )
        .expect("the last accepted image remains selectable");
        assert_eq!(
            selected.source.file_name().and_then(|name| name.to_str()),
            Some("wallpaper-064.png")
        );

        assert!(
            publish_gallery_page(&gallery, &gallery_page_options(6, 1), &runtime, &id,).is_err()
        );
        assert!(
            publish_gallery_page(&gallery, &gallery_page_options(7, 3), &runtime, &id,).is_err()
        );
        assert!(
            publish_gallery_page(&gallery, &gallery_page_options(7, 0), &runtime, &id,).is_err()
        );
    }

    #[test]
    fn gallery_accepts_exactly_512_images_and_refuses_a_513th_entry() {
        let root = TestDirectory::new();
        for index in 0..MAX_ENTRIES {
            write_png(&root.path().join(format!("wallpaper-{index:03}.png")));
        }

        let mut inspector = ImageInspector::default();
        let accepted = scan_gallery(root.path(), &mut inspector)
            .expect("the exact directory budget remains reachable");
        assert_eq!(accepted.entries.len(), MAX_ENTRIES);
        assert_eq!(accepted.entries.len().div_ceil(MAX_GALLERY_IMAGES), 8);
        assert_eq!(accepted.skipped, 0);
        assert!(accepted.truncated);

        fs::write(root.path().join("one-entry-too-many.txt"), b"x").expect("513th directory entry");
        let error = scan_gallery(root.path(), &mut inspector)
            .expect_err("an oversized directory is not partially published");

        assert!(error.contains("more than 512 entries"));
    }

    #[test]
    fn gallery_selection_requires_the_exact_catalogue_item_and_file_revision() {
        let root = TestDirectory::new();
        let source = root.path().join("Forest.png");
        write_png(&source);
        let mut inspector = ImageInspector::default();
        let scan = scan_gallery(root.path(), &mut inspector).expect("gallery scan");
        let gallery = gallery(GalleryInventory {
            request_generation: 2,
            catalogue: 9,
            folder: Some(root.path().to_owned()),
            state: GalleryPublicationState::Ready,
            entries: scan.entries,
            page_index: 0,
            truncated: false,
            skipped: 0,
        });
        let outputs = OutputInventory {
            generation: 3,
            outputs: vec![output("DP-1")],
        };

        let request = gallery_selection(
            &gallery,
            &gallery_selection_options("DP-1", 9, "Forest.png"),
            &outputs,
        )
        .expect("current gallery item");
        assert_eq!(request.source, source);
        assert_eq!(request.target_name, "DP-1.png");
        assert!(gallery_selection(
            &gallery,
            &gallery_selection_options("DP-1", 8, "Forest.png"),
            &outputs,
        )
        .is_err());
        assert!(gallery_selection(
            &gallery,
            &gallery_selection_options("DP-1", 9, "missing.png"),
            &outputs,
        )
        .is_err());
        assert!(gallery_selection(
            &gallery,
            &gallery_selection_options("HDMI-A-1", 9, "Forest.png"),
            &outputs,
        )
        .is_err());

        fs::write(&source, b"changed after the published preview").expect("replace gallery source");
        let managed = root.path().join("managed");
        fs::create_dir_all(&managed).expect("managed directory");
        assert!(import_selection(&managed, &request).is_err());
        assert!(gallery_selection(
            &gallery,
            &gallery_selection_options("DP-1", 9, "Forest.png"),
            &outputs,
        )
        .is_err());
    }

    #[test]
    fn a_stale_gallery_scan_cannot_replace_a_newer_folder() {
        let root = TestDirectory::new();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::create_dir_all(&first).expect("first folder");
        fs::create_dir_all(&second).expect("second folder");
        let gallery = gallery(GalleryInventory::default());
        let first_loading = set_gallery_folder_for(&gallery, Some(first)).expect("first request");
        let first_request = first_loading.request().expect("configured folder");
        let second_loading =
            set_gallery_folder_for(&gallery, Some(second.clone())).expect("second request");

        let id = ProviderId::new(GALLERY_NAME).expect("valid provider id");
        let mut provider_runtime = ProviderRuntime::new(1);
        provider_runtime.register(id.clone());
        let runtime = Mutex::new(provider_runtime);
        let stale = GalleryScan {
            entries: Vec::new(),
            truncated: false,
            skipped: 0,
        };

        assert_eq!(
            apply_gallery_scan_if_current(&gallery, &first_request, &runtime, &id, Ok(stale),),
            Ok(false)
        );
        let current = lock_gallery(&gallery).clone();
        assert_eq!(current.folder.as_deref(), Some(second.as_path()));
        assert_eq!(current.state, GalleryPublicationState::Loading);
        assert_eq!(current.catalogue, second_loading.catalogue);
        assert!(lock_runtime(&runtime).take_frame(0).providers.is_empty());
    }

    #[test]
    fn a_gallery_item_reuses_atomic_per_output_import() {
        let root = TestDirectory::new();
        let source = root.path().join("Forest.png");
        write_png(&source);
        let source_bytes = fs::read(&source).expect("source bytes");
        let mut inspector = ImageInspector::default();
        let scan = scan_gallery(root.path(), &mut inspector).expect("gallery scan");
        let gallery = gallery(GalleryInventory {
            request_generation: 1,
            catalogue: 5,
            folder: Some(root.path().to_owned()),
            state: GalleryPublicationState::Ready,
            entries: scan.entries,
            page_index: 0,
            truncated: false,
            skipped: 0,
        });
        let outputs = OutputInventory {
            generation: 1,
            outputs: vec![output("DP-1"), output("DP-2")],
        };
        let request = gallery_selection(
            &gallery,
            &gallery_selection_options("DP-1", 5, "Forest.png"),
            &outputs,
        )
        .expect("exact gallery item");
        let managed = root.path().join("managed");
        fs::create_dir_all(&managed).expect("managed directory");
        fs::write(managed.join("DP-1.jpg"), b"previous").expect("previous DP-1");
        fs::write(managed.join("DP-2.jpg"), b"other output").expect("DP-2");

        let imported = import_selection(&managed, &request).expect("atomic gallery import");

        assert_eq!(imported, managed.join("DP-1.png"));
        assert_eq!(fs::read(imported).expect("imported bytes"), source_bytes);
        assert_eq!(fs::read(&source).expect("source preserved"), source_bytes);
        assert!(!managed.join("DP-1.jpg").exists());
        assert_eq!(
            fs::read(managed.join("DP-2.jpg")).expect("other output preserved"),
            b"other output"
        );
    }

    #[test]
    fn a_valid_source_is_atomically_imported_without_touching_other_outputs() {
        let root = TestDirectory::new();
        let source = root.path().join("Forest.PNG");
        write_png(&source);
        let original_source = fs::read(&source).expect("source bytes");
        let directory = root.path().join("managed");
        fs::create_dir_all(&directory).expect("managed directory");
        fs::write(directory.join("DP-1.jpg"), b"previous").expect("previous choice");
        fs::write(directory.join("DP-10.jpg"), b"other output").expect("other choice");
        fs::write(directory.join("default.webp"), b"shared choice").expect("shared choice");
        let request = SelectionRequest {
            output: "DP-1".to_owned(),
            source: source.clone(),
            target_name: "DP-1.png".to_owned(),
            expected_fingerprint: None,
        };

        let destination = import_selection(&directory, &request).expect("atomic import");

        assert_eq!(destination, directory.join("DP-1.png"));
        assert_eq!(
            fs::read(&destination).expect("imported bytes"),
            original_source
        );
        assert_eq!(
            fs::read(&source).expect("preserved source"),
            original_source
        );
        assert!(!directory.join("DP-1.jpg").exists());
        assert!(directory.join("DP-10.jpg").exists());
        assert!(directory.join("default.webp").exists());
        assert!(fs::read_dir(&directory)
            .expect("managed entries")
            .all(|entry| !entry
                .expect("managed entry")
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")));
    }

    #[test]
    fn a_managed_source_path_is_not_deleted_during_extension_cleanup() {
        let root = TestDirectory::new();
        let directory = root.path().join("managed");
        fs::create_dir_all(&directory).expect("managed directory");
        let source = directory.join("DP-1.PNG");
        write_png(&source);
        let source_bytes = fs::read(&source).expect("source bytes");
        fs::write(directory.join("DP-1.jpg"), b"previous").expect("previous choice");
        let request = SelectionRequest {
            output: "DP-1".to_owned(),
            source: source.clone(),
            target_name: "DP-1.png".to_owned(),
            expected_fingerprint: None,
        };

        import_selection(&directory, &request).expect("managed-source import");

        assert_eq!(
            fs::read(&source).expect("preserved managed source"),
            source_bytes
        );
        assert_eq!(
            fs::read(directory.join("DP-1.png")).expect("normalized destination"),
            source_bytes
        );
        assert!(!directory.join("DP-1.jpg").exists());
    }

    #[test]
    fn an_undecodable_source_leaves_the_previous_choice_untouched() {
        let root = TestDirectory::new();
        let source = root.path().join("broken.png");
        fs::write(&source, b"not an image").expect("broken source");
        let directory = root.path().join("managed");
        fs::create_dir_all(&directory).expect("managed directory");
        let previous = directory.join("DP-1.jpg");
        fs::write(&previous, b"previous").expect("previous choice");
        let request = SelectionRequest {
            output: "DP-1".to_owned(),
            source,
            target_name: "DP-1.png".to_owned(),
            expected_fingerprint: None,
        };

        assert!(import_selection(&directory, &request).is_err());
        assert_eq!(fs::read(previous).expect("previous choice"), b"previous");
        assert!(!directory.join("DP-1.png").exists());
    }

    #[test]
    fn every_accepted_inventory_advances_its_generation() {
        let state = state(OutputInventory::default());
        let outputs = vec![output("winit")];

        assert_eq!(replace_outputs(&state, outputs.clone()), Ok(1));
        assert_eq!(replace_outputs(&state, outputs.clone()), Ok(2));
        assert_eq!(
            *lock_inventory(&state),
            OutputInventory {
                generation: 2,
                outputs,
            }
        );
    }

    #[test]
    fn an_import_refresh_advances_generation_without_replacing_outputs() {
        let outputs = vec![output("winit")];
        let state = state(OutputInventory {
            generation: 4,
            outputs: outputs.clone(),
        });

        assert_eq!(advance_generation(&state), Ok(5));
        assert_eq!(
            *lock_inventory(&state),
            OutputInventory {
                generation: 5,
                outputs,
            }
        );
    }

    #[test]
    fn exhausted_inventory_generation_is_refused_without_replacement() {
        let original = OutputInventory {
            generation: u64::MAX,
            outputs: vec![output("winit")],
        };
        let state = state(original.clone());

        assert!(replace_outputs(&state, vec![output("DP-1")]).is_err());
        assert_eq!(*lock_inventory(&state), original);
    }

    #[test]
    fn a_change_notified_before_wait_is_not_lost() {
        let state = state(OutputInventory::default());
        let observed_generation = 0;
        assert_eq!(replace_outputs(&state, vec![output("winit")]), Ok(1));

        let started = std::time::Instant::now();
        assert!(wait_for_inventory_change(
            &state,
            observed_generation,
            Duration::from_secs(1),
        ));
        assert!(started.elapsed() < Duration::from_millis(250));
    }

    #[test]
    fn a_stale_identity_payload_is_not_published() {
        let expected = OutputInventory {
            generation: 1,
            outputs: vec![output_with_geometry("winit", 1920, 1080)],
        };
        let state = state(expected.clone());
        assert_eq!(
            replace_outputs(&state, vec![output_with_geometry("winit", 2560, 1440)]),
            Ok(2)
        );

        let id = ProviderId::new(IDENTITY_NAME).expect("valid provider id");
        let mut provider_runtime = ProviderRuntime::new(1);
        provider_runtime.register(id.clone());
        let runtime = Mutex::new(provider_runtime);
        let mut payload = Payload::new();
        payload.insert("outputs".to_owned(), Value::Array(Vec::new()));

        assert_eq!(
            apply_update_if_current(&state, &expected, &runtime, &id, Some(payload)),
            Ok(false)
        );
        let mut runtime = lock_runtime(&runtime);
        assert!(runtime.take_frame(0).providers.is_empty());
    }

    #[test]
    fn identity_rows_carry_only_file_and_geometry_identity() {
        let value = identity_row(
            &output_with_geometry("winit", 1920, 1080),
            Path::new("/wallpaper.png"),
            17,
            "42:99".to_owned(),
        )
        .expect("flat identity row");
        let row = value.as_object().expect("identity object");

        assert_eq!(row.get("output"), Some(&Value::from("winit")));
        assert_eq!(row.get("source"), Some(&Value::from("/wallpaper.png")));
        assert_eq!(row.get("generation"), Some(&Value::from(17)));
        assert_eq!(row.get("revision"), Some(&Value::from("42:99")));
        assert_eq!(row.get("width"), Some(&Value::from(1920)));
        assert_eq!(row.get("height"), Some(&Value::from(1080)));
        assert_eq!(row.len(), 6);
        assert!(!row.contains_key("ink"));
        assert!(!row.contains_key("uncertain"));
    }

    #[test]
    fn unusable_output_inventory_is_rejected() {
        assert!(parse_outputs(&options(serde_json::json!(["winit", 7]))).is_err());
        assert!(parse_outputs(&options(Value::Array(vec![
            Value::from("winit");
            MAX_OUTPUTS + 1
        ])))
        .is_err());
        assert!(parse_outputs(&options_with_geometries(
            serde_json::json!(["winit"]),
            Value::Array(vec![Value::Null; MAX_OUTPUTS + 1]),
        ))
        .is_err());
    }

    #[test]
    fn fingerprint_revision_is_stable_and_bounded() {
        let fingerprint = Fingerprint {
            bytes: 42,
            modified_ns: Some(99),
        };
        assert_eq!(fingerprint.revision(), "42:99");
    }

    #[test]
    fn a_non_regular_image_source_is_refused_after_a_nonblocking_open() {
        let path = Path::new("/dev/null");
        let metadata = fs::metadata(path).expect("Linux null device metadata");
        let error = validate_image(path, Fingerprint::from_metadata(&metadata))
            .expect_err("a character device is not an image file");

        assert_eq!(error, "image is not a regular file");
    }

    #[test]
    fn repeated_failure_fingerprints_keep_one_entry_per_path() {
        let mut inspector = ImageInspector::default();
        let path = Path::new("/wallpapers/broken.png");
        let first = Fingerprint {
            bytes: 10,
            modified_ns: Some(1),
        };
        let second = Fingerprint {
            bytes: 11,
            modified_ns: Some(2),
        };

        assert!(inspector.should_report_failure(path, first));
        assert!(!inspector.should_report_failure(path, first));
        assert!(inspector.should_report_failure(path, second));
        assert_eq!(inspector.reported_failures.len(), 1);
        assert_eq!(inspector.reported_failures.get(path), Some(&second));
    }
}
