//! What `Mod+Space` finds: the desktop-entry index, and what typing does to it.
//!
//! Reading the `.desktop` files is a bounded, cancellable walk of the XDG
//! application directories — the same truthfulness contract
//! `siderita::search` uses for its own filesystem walk, so an index that had
//! to stop early says so rather than quietly answering as if it saw
//! everything. `celestina_core::desktop_entry` reads one file; this only
//! decides which files to read and what to do with what comes back —
//! `celestina_shell_core::launcher` does the ranking.

use std::collections::HashSet;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock};
use std::thread;

use celestina_core::desktop_entry::{self, DesktopEntry};
use celestina_core::CancellationToken;
use celestina_shell_core::bounded;
use celestina_shell_core::launcher;
use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::snapshot::{Payload, ProviderId, MAX_TEXT_UNITS};
use serde_json::Value;

use super::tools::{launch_argv, lock_runtime};

pub const NAME: &str = "launcher";

/// An index this large is not a session's applications; it is a directory
/// tree gone wrong. A real desktop offers a few hundred to a couple of
/// thousand.
const MAX_INDEXED_ENTRIES: usize = 4096;
/// A launcher shows a screenful, not a database dump.
const MAX_RESULTS: usize = 24;
/// Search opens this engine by default. Settled, not sealed: a `DuckDuckGo`
/// query URL, so nothing about opening a search sends the query anywhere
/// before the person's own browser and its own privacy settings do. Reopen if
/// the author asks for a different one.
const WEB_SEARCH_URL: &str = "https://duckduckgo.com/?q=";

struct Index {
    entries: Vec<DesktopEntry>,
    truncated: bool,
    /// Whether a scan has ever finished. The startup scan takes a few
    /// milliseconds in practice, and the helper runs continuously from well
    /// before anyone could press `Mod+Space` — but a query that lands in that
    /// narrow window must not be told "nothing matches" when the honest answer
    /// is "not indexed yet".
    ready: bool,
}

/// The index and the token of whatever scan is currently filling it, if one
/// is. Module-level because the command dispatch in `main.rs` calls this
/// module's `action` as a free function with no state of its own to carry —
/// the same shape `brightness`'s pending-target table already uses.
static INDEX: OnceLock<RwLock<Index>> = OnceLock::new();
static SCANNING: OnceLock<Mutex<Option<CancellationToken>>> = OnceLock::new();

fn index() -> &'static RwLock<Index> {
    INDEX.get_or_init(|| {
        RwLock::new(Index {
            entries: Vec::new(),
            truncated: false,
            ready: false,
        })
    })
}

fn scanning() -> &'static Mutex<Option<CancellationToken>> {
    SCANNING.get_or_init(|| Mutex::new(None))
}

fn lock_scanning() -> MutexGuard<'static, Option<CancellationToken>> {
    match scanning().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// The desktop name entries' `OnlyShowIn`/`NotShowIn` are compared against —
/// the first name in `$XDG_CURRENT_DESKTOP`, which is colon-separated for a
/// session that identifies as more than one desktop at once. Empty when the
/// session names none, which excludes nothing: an entry with no preference
/// shows everywhere.
fn current_desktop_name() -> String {
    std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .and_then(|value| value.split(':').next().map(str::to_owned))
        .unwrap_or_default()
}

/// Walks the XDG application directories once, bounded and cancellable.
/// Claims each `.desktop` id for the most specific directory it appears in —
/// a user override shadows a system entry of the same id whether or not the
/// override is itself listable, which is what makes `Hidden=true` in the
/// user's own directory actually hide a system application.
fn scan_entries(cancellation: &CancellationToken) -> Index {
    let desktop = current_desktop_name();
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    let mut truncated = false;

    'dirs: for dir in desktop_entry::application_dirs() {
        let Ok(read_dir) = std::fs::read_dir(&dir) else {
            continue;
        };
        for item in read_dir.flatten() {
            if cancellation.is_cancelled() {
                break 'dirs;
            }
            let path = item.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("desktop") {
                continue;
            }
            let Some(id) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if seen.contains(id) {
                continue;
            }
            seen.insert(id.to_owned());

            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Some(parsed) = desktop_entry::parse(id, &content) else {
                continue;
            };
            if !parsed.is_listable() || !parsed.shows_in(&desktop) {
                continue;
            }
            if entries.len() >= MAX_INDEXED_ENTRIES {
                truncated = true;
                break 'dirs;
            }
            entries.push(parsed);
        }
    }

    Index {
        entries,
        truncated,
        ready: true,
    }
}

fn replace_index(cancellation: &CancellationToken) {
    let scanned = scan_entries(cancellation);
    if cancellation.is_cancelled() {
        return;
    }
    if let Ok(mut current) = index().write() {
        *current = scanned;
    }
}

/// The startup scan only: swaps the index in, then republishes an empty query
/// so a launcher opened inside the narrow startup race (see `Index::ready`) is
/// corrected without waiting for a second keystroke that may never come. A
/// `rescan` does not take this path — it must not silently replace whatever a
/// person is already searching for with "show everything".
fn replace_index_and_correct(
    cancellation: &CancellationToken,
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
) {
    replace_index(cancellation);
    publish_hits(runtime, id, "");
}

pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>) -> io::Result<()> {
    let Ok(id) = ProviderId::new(NAME) else {
        eprintln!("celestina-provider-adapter: launcher: unusable provider name");
        return Ok(());
    };
    lock_runtime(runtime).register(id.clone());

    // The first scan runs on its own thread so a large `$HOME` mount or a slow
    // disk never delays the other providers' first frame.
    let cancellation = CancellationToken::new();
    *lock_scanning() = Some(cancellation.clone());
    let scan_runtime = Arc::clone(runtime);
    thread::Builder::new()
        .name("launcher-scan".to_owned())
        .spawn(move || replace_index_and_correct(&cancellation, &scan_runtime, &id))?;
    Ok(())
}

/// One hit as a flat row.
///
/// Every field here comes from a `.desktop` file that any installed package may
/// write, so none of them has a length this shell was promised. They are cut to
/// what a row field carries: one over-long entry would otherwise make the host
/// reject the frame that lists it, taking every other provider's reading with
/// it for as long as the entry keeps matching.
fn hit_payload(entry: &DesktopEntry) -> Value {
    let field = |text: &str| Value::from(bounded(text, MAX_TEXT_UNITS));
    Value::Object(
        [
            ("id".to_owned(), field(&entry.id)),
            ("name".to_owned(), field(&entry.name)),
            ("genericName".to_owned(), field(&entry.generic_name)),
            ("icon".to_owned(), field(&entry.icon)),
            ("comment".to_owned(), field(&entry.comment)),
        ]
        .into_iter()
        .collect(),
    )
}

fn publish_hits(runtime: &Mutex<ProviderRuntime>, id: &ProviderId, query: &str) {
    let Ok(current) = index().read() else {
        return;
    };
    let outcome = launcher::rank_bounded(&current.entries, query, MAX_RESULTS);
    let hits: Vec<Value> = outcome
        .ids
        .iter()
        .filter_map(|hit_id| current.entries.iter().find(|entry| &entry.id == hit_id))
        .map(hit_payload)
        .collect();

    let mut payload = Payload::new();
    payload.insert("query".to_owned(), Value::from(query.to_owned()));
    payload.insert("hits".to_owned(), Value::Array(hits));
    // The overlay must not read "no more results" from a scan that is still
    // running as "the index has only this many applications".
    payload.insert(
        "truncated".to_owned(),
        Value::from(outcome.truncated || current.truncated),
    );
    // False only in the narrow window right after the shell itself starts,
    // before the first scan lands — never once a person could plausibly have
    // reached for the launcher. An empty `hits` here is "not indexed yet", not
    // "nothing matches".
    payload.insert("ready".to_owned(), Value::from(current.ready));

    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: launcher: {error}");
    }
}

/// Percent-encodes a query for a URL's `?q=` — the handful of characters that
/// are not reserved and not already safe unescaped. Written locally: three
/// bytes of table for one query field is not a dependency's worth of surface.
fn percent_encode(text: &str) -> String {
    use std::fmt::Write;

    let mut encoded = String::with_capacity(text.len());
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char);
            }
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

pub fn action(
    verb: &str,
    options: &Payload,
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
) -> Result<(), String> {
    match verb {
        "query" => {
            let query = options
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default();
            publish_hits(runtime, id, query);
            Ok(())
        }
        "rescan" => {
            let cancellation = CancellationToken::new();
            if let Some(previous) = lock_scanning().replace(cancellation.clone()) {
                previous.cancel();
            }
            thread::Builder::new()
                .name("launcher-scan".to_owned())
                .spawn(move || replace_index(&cancellation))
                .map_err(|error| format!("cannot start a rescan: {error}"))?;
            Ok(())
        }
        "launch" => {
            let target = options
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| "launch needs the entry's id".to_owned())?;
            let entry = {
                let current = index()
                    .read()
                    .map_err(|_| "the index is unreadable".to_owned())?;
                current
                    .entries
                    .iter()
                    .find(|entry| entry.id == target)
                    .cloned()
                    .ok_or_else(|| format!("no indexed entry named '{target}'"))?
            };
            let mut argv = desktop_entry::exec_argv(&entry)
                .ok_or_else(|| format!("'{}' names no program to run", entry.name))?;
            if entry.terminal {
                // The session's own terminal, the same one its own binds spawn
                // — not whatever a desktop-neutral heuristic would guess.
                let mut wrapped = vec!["kitty".to_owned(), "-e".to_owned()];
                wrapped.append(&mut argv);
                argv = wrapped;
            }
            let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
            launch_argv(&argv)
        }
        "web-search" => {
            let query = options
                .get("query")
                .and_then(Value::as_str)
                .filter(|query| !query.is_empty())
                .ok_or_else(|| "web-search needs a query".to_owned())?;
            let url = format!("{WEB_SEARCH_URL}{}", percent_encode(query));
            launch_argv(&["xdg-open", &url])
        }
        _ => Err(format!("'{NAME}' does not serve the verb '{verb}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::percent_encode;

    #[test]
    fn safe_characters_pass_through_and_the_rest_is_escaped() {
        assert_eq!(percent_encode("gimp"), "gimp");
        assert_eq!(percent_encode("a b"), "a%20b");
        assert_eq!(percent_encode("c++"), "c%2B%2B");
        assert_eq!(percent_encode("100% seguro"), "100%25%20seguro");
    }

    #[test]
    fn an_empty_query_encodes_to_nothing() {
        assert_eq!(percent_encode(""), "");
    }
}
