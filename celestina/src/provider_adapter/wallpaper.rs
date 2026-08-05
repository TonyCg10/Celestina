//! Which file each output should be showing, decided once and published.
//!
//! [`celestina_shell_core::wallpaper`] owns the rules — named image first, the
//! shared one next, and a deliberate fallback rather than another screen's
//! picture. This module supplies the two things those rules need from the
//! world: which files are in the directory, and which outputs exist.
//!
//! The outputs come from the host, because the host is what talks to the
//! compositor; it sends them through the command channel that already exists
//! rather than through a second path. Until it has, this provider publishes
//! nothing: a wallpaper choice for a screen nobody has is not information.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::snapshot::{Payload, ProviderId};
use celestina_shell_core::wallpaper::{self, Choice};
use serde_json::Value;

use super::tools::lock_runtime;

pub const NAME: &str = "wallpaper";

/// Nothing here changes quickly, and a directory the person edits by hand is
/// noticed within a few seconds rather than watched with an inotify budget.
const INTERVAL: Duration = Duration::from_secs(5);
/// A wallpaper directory with more entries than this is not a wallpaper
/// directory; reading all of them would be spending the session's time on
/// somebody's downloads folder.
const MAX_ENTRIES: usize = 512;

/// The outputs the host has told us about, in the order it named them.
static OUTPUTS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

fn outputs() -> &'static Mutex<Vec<String>> {
    OUTPUTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn lock_outputs() -> std::sync::MutexGuard<'static, Vec<String>> {
    match outputs().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
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
fn available(directory: &PathBuf) -> Vec<String> {
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

/// Records which outputs exist. The host sends this whenever its screens
/// change, including at start.
///
/// # Errors
///
/// Returns the requester's sentence for a verb this provider does not serve,
/// or for a list that is not one of output names.
pub fn action(
    verb: &str,
    options: &Payload,
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
) -> Result<(), String> {
    if verb != "set-outputs" {
        return Err(format!("'{NAME}' does not serve the verb '{verb}'"));
    }

    let listed = options
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("'{NAME}' needs the list of outputs"))?;
    let named: Vec<String> = listed
        .iter()
        .filter_map(Value::as_str)
        .filter(|output| !output.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    *lock_outputs() = named;
    publish(runtime, id);
    Ok(())
}

/// Publishes one entry per output: the absolute path to show, or `null` for an
/// output that has nothing of its own. `null` is the fallback the surface
/// paints deliberately — it is not an error and it is not another screen's
/// picture.
fn publish(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    let known = lock_outputs().clone();
    if known.is_empty() {
        // Nothing is known about this session's screens yet, so there is
        // nothing truthful to say about their wallpapers.
        lock_runtime(runtime).withdraw(id);
        return;
    }

    let Some(directory) = directory() else {
        lock_runtime(runtime).withdraw(id);
        return;
    };
    let names = available(&directory);

    let mut chosen: BTreeMap<String, Value> = BTreeMap::new();
    for output in known {
        let value = match wallpaper::choose(&output, &names) {
            Choice::Image(name) => directory
                .join(name)
                .to_str()
                .map_or(Value::Null, Value::from),
            Choice::Fallback => Value::Null,
        };
        chosen.insert(output, value);
    }

    let mut payload = Payload::new();
    for (output, value) in chosen {
        payload.insert(output, value);
    }
    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: wallpaper: {error}");
    }
}

pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>) -> io::Result<()> {
    let Ok(id) = ProviderId::new(NAME) else {
        eprintln!("celestina-provider-adapter: wallpaper: unusable provider name");
        return Ok(());
    };

    lock_runtime(runtime).register(id.clone());
    let runtime = Arc::clone(runtime);
    thread::Builder::new()
        .name(NAME.to_owned())
        .spawn(move || run(&runtime, &id))?;
    Ok(())
}

fn run(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    loop {
        publish(runtime, id);
        thread::sleep(INTERVAL);
    }
}
