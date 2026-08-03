//! The desktop clipboard's history, watched over `ext-data-control-v1`.
//!
//! This is the one provider with no subprocess tool to shell out to: watching
//! and setting the desktop selection is a Wayland conversation of its own, so
//! this holds a `wayland-client` connection directly, on its own thread,
//! independent of every other provider here. `celestina_shell_core::clipboard`
//! decides what a selection means — bounded, not a password, worth keeping;
//! this only speaks the protocol and persists what that decided.
//!
//! `ext-data-control-v1` is the standardized successor to wlroots' own
//! protocol; niri advertises both, and the standardized one is the one to
//! prefer going forward. Only the regular selection is watched — the primary
//! selection (mouse-drag, middle-click paste) is a different, more ephemeral
//! convention, and archiving it is not what "clipboard history" means to a
//! person who just pressed Ctrl+C.
//!
//! **A deliberate simplification, named rather than hidden:** re-selecting a
//! history entry sets our own source as the selection, which the compositor
//! then echoes back to us as an ordinary `selection` event — reading our own
//! text back would require this thread to answer its own `send` request while
//! it is blocked waiting to read that very answer. Rather than build the
//! nested event loop that would take, one flag skips the read for exactly the
//! next `selection` event after we set one ourselves — the text is not new
//! information, we already have it. The risk this accepts: a real, unrelated
//! copy that lands in the narrow window between our request and its echo is
//! missed. No clipboard manager in the wlroots ecosystem does better than this
//! without a full second event-loop integration.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use celestina_core::atomic_file;
use celestina_shell_core::clipboard::{self, ClipboardHistory};
use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::snapshot::{Payload, ProviderId};
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use serde_json::Value;
use wayland_client::backend::ObjectId;
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{delegate_noop, Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1 as device_proto, ext_data_control_manager_v1 as manager_proto,
    ext_data_control_offer_v1 as offer_proto, ext_data_control_source_v1 as source_proto,
};

use super::tools::lock_runtime;

pub const NAME: &str = "clipboard";

/// Every text mime this suite offers or accepts, most preferred first.
const TEXT_MIMES: [&str; 2] = ["text/plain;charset=utf-8", "text/plain"];
/// How long a poll waits before checking the command queue again. A history
/// entry chosen by click answers within a fraction of this — imperceptible —
/// and it is what lets this thread do without a wakeup pipe of its own.
const POLL_INTERVAL: Duration = Duration::from_millis(150);

fn history_path() -> Option<std::path::PathBuf> {
    celestina_core::xdg::state_home().map(|dir| dir.join("celestina").join("clipboard.json"))
}

fn load_history() -> ClipboardHistory {
    let Some(path) = history_path() else {
        return ClipboardHistory::new();
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return ClipboardHistory::new();
    };
    let Ok(entries) = serde_json::from_slice::<Vec<String>>(&bytes) else {
        return ClipboardHistory::new();
    };
    ClipboardHistory::from_entries(entries)
}

fn save_history(history: &ClipboardHistory) {
    let Some(path) = history_path() else {
        return;
    };
    let Ok(bytes) = serde_json::to_vec(history.entries()) else {
        return;
    };
    if let Err(error) = atomic_file::replace(&path, &bytes) {
        eprintln!("celestina-provider-adapter: clipboard: could not persist history: {error}");
    }
}

/// What the command worker asks of the clipboard thread. Requests, not
/// direct calls: the connection and every proxy it owns live on one thread,
/// and every other provider here already keeps its state to itself.
enum Request {
    Select(usize),
    Remove(usize),
    Clear,
}

static REQUESTS: OnceLock<SyncSender<Request>> = OnceLock::new();

pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>) -> std::io::Result<()> {
    let Ok(id) = ProviderId::new(NAME) else {
        eprintln!("celestina-provider-adapter: clipboard: unusable provider name");
        return Ok(());
    };
    lock_runtime(runtime).register(id.clone());

    let (sender, receiver) = sync_channel(32);
    REQUESTS
        .set(sender)
        .expect("clipboard::spawn is called exactly once");

    let runtime = Arc::clone(runtime);
    thread::Builder::new()
        .name(NAME.to_owned())
        .spawn(move || run(&runtime, &id, &receiver))?;
    Ok(())
}

pub fn action(verb: &str, options: &Payload) -> Result<(), String> {
    let Some(sender) = REQUESTS.get() else {
        return Err("the clipboard thread has not started".to_owned());
    };

    let request = match verb {
        "select" => Request::Select(index_option(options)?),
        "remove" => Request::Remove(index_option(options)?),
        "clear" => Request::Clear,
        _ => return Err(format!("'{NAME}' does not serve the verb '{verb}'")),
    };

    sender.try_send(request).map_err(|error| match error {
        TrySendError::Full(_) => "the clipboard is busy with an earlier request".to_owned(),
        TrySendError::Disconnected(_) => "the clipboard thread is gone".to_owned(),
    })
}

fn index_option(options: &Payload) -> Result<usize, String> {
    options
        .get("index")
        .and_then(Value::as_u64)
        .and_then(|index| usize::try_from(index).ok())
        .ok_or_else(|| "needs a whole-number 'index'".to_owned())
}

/// A history entry is shown as a preview, never handed back whole — the host's
/// payload bound (`providerstates.cpp`'s `maxTextChars`) is far smaller than
/// what an entry is allowed to hold, and a list row only needs enough text to
/// recognize the entry by. Selecting or removing one addresses it by index
/// back through this same list, so the full text never has to round-trip.
const MAX_PUBLISHED_ENTRIES: usize = 50;
const MAX_PREVIEW_CHARS: usize = 160;

/// Collapses an entry to one line and bounds it — a preview reads by shape,
/// not by reproducing whatever whitespace was on the clipboard.
fn preview(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let truncated = collapsed.chars().count() > MAX_PREVIEW_CHARS;
    let mut shown: String = collapsed.chars().take(MAX_PREVIEW_CHARS).collect();
    if truncated {
        shown.push('…');
    }
    shown
}

fn publish(runtime: &Mutex<ProviderRuntime>, id: &ProviderId, history: &ClipboardHistory) {
    let entries = history.entries();
    let rows: Vec<Value> = entries
        .iter()
        .take(MAX_PUBLISHED_ENTRIES)
        .enumerate()
        .map(|(index, text)| {
            Value::Object(
                [
                    ("index".to_owned(), Value::from(index)),
                    ("preview".to_owned(), Value::from(preview(text))),
                ]
                .into_iter()
                .collect(),
            )
        })
        .collect();

    let mut payload = Payload::new();
    // The overlay must not read "the history has only this many entries" from
    // a list that was cut off for the payload's own sake.
    payload.insert(
        "truncated".to_owned(),
        Value::from(entries.len() > MAX_PUBLISHED_ENTRIES),
    );
    payload.insert("entries".to_owned(), Value::Array(rows));
    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: clipboard: {error}");
    }
}

struct State {
    seat: Option<wl_seat::WlSeat>,
    manager: Option<manager_proto::ExtDataControlManagerV1>,
    device: Option<device_proto::ExtDataControlDeviceV1>,
    /// Mimes accumulated for each offer as its `offer` events arrive, keyed by
    /// the offer's own id — the same id the later `selection` event names.
    offer_mimes: HashMap<ObjectId, Vec<String>>,
    /// Set right after we set our own source as the selection; cleared on the
    /// very next `selection` event, which is that change echoed back. See the
    /// module doc for why this exists instead of a nested event loop.
    expect_self_echo: bool,
    outgoing: Option<(source_proto::ExtDataControlSourceV1, String)>,
    history: ClipboardHistory,
    runtime: Arc<Mutex<ProviderRuntime>>,
    provider_id: ProviderId,
}

impl State {
    fn record_if_new(&mut self, text: String) {
        if self.history.record(text) {
            save_history(&self.history);
            publish(&self.runtime, &self.provider_id, &self.history);
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };

        match interface.as_str() {
            "wl_seat" if state.seat.is_none() => {
                state.seat =
                    Some(registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(1), qh, ()));
            }
            "ext_data_control_manager_v1" if state.manager.is_none() => {
                state.manager = Some(
                    registry.bind::<manager_proto::ExtDataControlManagerV1, _, _>(
                        name,
                        version.min(1),
                        qh,
                        (),
                    ),
                );
            }
            _ => return,
        }

        if state.device.is_none() {
            if let (Some(manager), Some(seat)) = (&state.manager, &state.seat) {
                state.device = Some(manager.get_data_device(seat, qh, ()));
            }
        }
    }
}

impl Dispatch<device_proto::ExtDataControlDeviceV1, ()> for State {
    fn event(
        state: &mut Self,
        _device: &device_proto::ExtDataControlDeviceV1,
        event: device_proto::Event,
        (): &(),
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            device_proto::Event::Selection { id: Some(offer) } => {
                let mimes = state.offer_mimes.remove(&offer.id()).unwrap_or_default();

                if std::mem::take(&mut state.expect_self_echo) {
                    // Our own re-selection, echoed back. We already know the
                    // text; see the module doc.
                    offer.destroy();
                    return;
                }

                if clipboard::is_sensitive(&mimes) {
                    // The safest thing this list can do with a password is
                    // never even ask for it.
                    offer.destroy();
                    return;
                }

                let Some(mime) = TEXT_MIMES
                    .iter()
                    .find(|mime| mimes.iter().any(|offered| offered == *mime))
                else {
                    // Not text — an image, a file list. Not this list's
                    // concern, the same rule the phone bridge already lives by.
                    offer.destroy();
                    return;
                };

                let text = receive_text(&offer, mime, conn);
                offer.destroy();

                if let Some(text) = text.filter(|text| clipboard::is_recordable(text)) {
                    state.record_if_new(text);
                }
            }
            device_proto::Event::PrimarySelection { id: Some(offer) } => {
                // Not archived — see the module doc — but still ours to
                // release, or the compositor never reclaims it.
                state.offer_mimes.remove(&offer.id());
                offer.destroy();
            }
            // A `Selection { id: None }` is the clipboard cleared, not
            // replaced — nothing to keep. The rest carry nothing this history
            // acts on.
            _ => {}
        }
        let _ = qh;
    }

    wayland_client::event_created_child!(State, device_proto::ExtDataControlDeviceV1, [
        // `data_offer` is the interface's first declared event.
        0 => (offer_proto::ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<offer_proto::ExtDataControlOfferV1, ()> for State {
    fn event(
        state: &mut Self,
        offer: &offer_proto::ExtDataControlOfferV1,
        event: offer_proto::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let offer_proto::Event::Offer { mime_type } = event {
            state
                .offer_mimes
                .entry(offer.id())
                .or_default()
                .push(mime_type);
        }
    }
}

impl Dispatch<source_proto::ExtDataControlSourceV1, ()> for State {
    fn event(
        state: &mut Self,
        source: &source_proto::ExtDataControlSourceV1,
        event: source_proto::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            source_proto::Event::Send { fd, .. } => {
                if let Some((current, text)) = &state.outgoing {
                    if current.id() == source.id() {
                        let mut file = File::from(fd);
                        let _ = file.write_all(text.as_bytes());
                        // Dropping `file` closes our end; the requester reads
                        // EOF once we do, which is how it knows we are done.
                    }
                }
            }
            source_proto::Event::Cancelled => {
                if let Some((current, _)) = &state.outgoing {
                    if current.id() == source.id() {
                        current.destroy();
                        state.outgoing = None;
                    }
                }
            }
            _ => {}
        }
    }
}

delegate_noop!(State: ignore wl_seat::WlSeat);
delegate_noop!(State: ignore manager_proto::ExtDataControlManagerV1);

/// Asks the offer for one mime type and reads it back, bounded to what
/// [`clipboard::is_recordable`] would accept anyway — a source that keeps
/// writing past that is not a clipboard entry, and this stops reading before
/// it becomes one.
///
/// Blocks this thread until the source finishes writing or closes; nothing
/// else here depends on this thread's responsiveness except the clipboard
/// provider itself, so a slow or stuck source only stalls its own history
/// entry, the same bound every other provider's subprocess timeout accepts.
fn receive_text(
    offer: &offer_proto::ExtDataControlOfferV1,
    mime: &str,
    conn: &Connection,
) -> Option<String> {
    let (read_fd, write_fd) = rustix::pipe::pipe().ok()?;
    offer.receive(mime.to_owned(), write_fd.as_fd());
    conn.flush().ok()?;
    // Our copy must close so the writer's EOF is reachable; the source's own
    // copy, relayed by the compositor, is what stays open while it writes.
    drop(write_fd);

    let file = File::from(read_fd);
    let mut buffer = Vec::new();
    file.take(u64::try_from(clipboard::MAX_ENTRY_BYTES).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut buffer)
        .ok()?;
    String::from_utf8(buffer).ok()
}

fn set_selection(state: &mut State, qh: &QueueHandle<State>, text: String) {
    let (Some(manager), Some(device)) = (&state.manager, &state.device) else {
        return;
    };
    if let Some((previous, _)) = state.outgoing.take() {
        previous.destroy();
    }

    let source = manager.create_data_source(qh, ());
    for mime in TEXT_MIMES {
        source.offer(mime.to_owned());
    }
    device.set_selection(Some(&source));
    state.expect_self_echo = true;
    state.outgoing = Some((source, text));
}

fn drain_requests(state: &mut State, qh: &QueueHandle<State>, receiver: &Receiver<Request>) {
    while let Ok(request) = receiver.try_recv() {
        match request {
            Request::Select(index) => {
                if let Some(text) = state.history.entries().get(index).cloned() {
                    // Selecting is felt immediately; the round-trip through
                    // the compositor that follows changes nothing this list
                    // has not already shown.
                    state.record_if_new(text.clone());
                    set_selection(state, qh, text);
                }
            }
            Request::Remove(index) => {
                if state.history.remove(index) {
                    save_history(&state.history);
                    publish(&state.runtime, &state.provider_id, &state.history);
                }
            }
            Request::Clear => {
                if state.history.clear() {
                    save_history(&state.history);
                    publish(&state.runtime, &state.provider_id, &state.history);
                }
            }
        }
    }
}

/// Polls the Wayland socket with a bounded timeout so this thread also comes
/// up for air often enough to notice a queued request — no wakeup pipe of its
/// own, since a fraction of a second of latency on a history click is not
/// something anyone perceives.
fn pump(event_queue: &mut EventQueue<State>, state: &mut State) -> bool {
    if event_queue.dispatch_pending(state).is_err() {
        return false;
    }
    if event_queue.flush().is_err() {
        return false;
    }

    let Some(guard) = event_queue.prepare_read() else {
        // Events were already queued by the dispatch above; nothing to wait on.
        return true;
    };

    let fd = guard.connection_fd();
    let mut fds = [PollFd::new(&fd, PollFlags::IN)];
    let timeout = Timespec {
        tv_sec: 0,
        tv_nsec: i64::try_from(POLL_INTERVAL.as_nanos()).unwrap_or(0),
    };
    match poll(&mut fds, Some(&timeout)) {
        Ok(count) if count > 0 && fds[0].revents().contains(PollFlags::IN) => {
            let _ = guard.read();
        }
        // Nothing readable within the window, or the poll itself failed:
        // either way, cancel the read registration and come back around.
        Ok(_) | Err(_) => drop(guard),
    }
    true
}

fn run(runtime: &Arc<Mutex<ProviderRuntime>>, id: &ProviderId, receiver: &Receiver<Request>) {
    let Ok(connection) = Connection::connect_to_env() else {
        eprintln!("celestina-provider-adapter: clipboard: no Wayland connection");
        lock_runtime(runtime).unregister(id);
        return;
    };
    let mut event_queue = connection.new_event_queue();
    let qh = event_queue.handle();
    let display = connection.display();
    display.get_registry(&qh, ());

    let mut state = State {
        seat: None,
        manager: None,
        device: None,
        offer_mimes: HashMap::new(),
        expect_self_echo: false,
        outgoing: None,
        history: load_history(),
        runtime: Arc::clone(runtime),
        provider_id: id.clone(),
    };

    // The globals arrive on the first round-trip; without both, this session's
    // compositor does not offer clipboard history at all.
    if event_queue.roundtrip(&mut state).is_err() || state.device.is_none() {
        eprintln!(
            "celestina-provider-adapter: clipboard: this compositor offers no data-control device"
        );
        lock_runtime(runtime).unregister(id);
        return;
    }

    if !state.history.entries().is_empty() {
        publish(runtime, id, &state.history);
    }

    loop {
        if !pump(&mut event_queue, &mut state) {
            eprintln!("celestina-provider-adapter: clipboard: lost the Wayland connection");
            lock_runtime(runtime).unregister(id);
            return;
        }
        drain_requests(&mut state, &qh, receiver);
    }
}

#[cfg(test)]
mod tests {
    use super::{preview, MAX_PREVIEW_CHARS};

    #[test]
    fn a_short_entry_previews_unchanged_but_on_one_line() {
        assert_eq!(preview("hello"), "hello");
        assert_eq!(preview("two\nlines"), "two lines");
        assert_eq!(preview("  padded  words  "), "padded words");
    }

    #[test]
    fn a_long_entry_is_cut_and_marked() {
        let long = "x".repeat(MAX_PREVIEW_CHARS + 20);
        let shown = preview(&long);
        assert_eq!(shown.chars().count(), MAX_PREVIEW_CHARS + 1);
        assert!(shown.ends_with('…'));
    }
}
