//! `org.kde.StatusNotifierWatcher`: the registry items and hosts announce
//! themselves to (design §5.6).
//!
//! The shell serves [`Watcher`] at [`WATCHER_PATH`] and only then requests
//! [`WATCHER_NAME`] with `DoNotQueue`. When another program holds the name
//! (Noctalia in the author's session) the object is withdrawn, the holder is
//! looked up for the Tray page ([`name_holder`]) and the tray is disabled
//! for the session: no queue, no retry.
//!
//! The served methods run on zbus's executor thread. They only parse,
//! bound and record what a peer (hostile input) says, emit the protocol's
//! signals and poke the host worker; they never call back into the bus, which
//! would block the thread that reads the replies. The host worker reads the
//! bookkeeping ([`WatcherState`]) through the shared handle and does the
//! calls.

use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use zbus::{
    blocking::{fdo::DBusProxy, Connection},
    message::Header,
    object_server::SignalEmitter,
};

use crate::tray::model::{ItemId, ItemIdError, NameHolder, MAX_ITEMS};

/// The watcher's well-known name.
pub const WATCHER_NAME: &str = "org.kde.StatusNotifierWatcher";
/// Where the watcher is served.
pub const WATCHER_PATH: &str = "/StatusNotifierWatcher";
/// The watcher interface.
pub const WATCHER_INTERFACE: &str = "org.kde.StatusNotifierWatcher";
/// The protocol version the watcher reports.
pub const PROTOCOL_VERSION: i32 = 0;
/// The most items one bus connection may register: an application has one,
/// a multi-account client a few; past this a peer is flooding.
pub const MAX_ITEMS_PER_SENDER: usize = 4;
/// The most foreign hosts remembered.
pub const MAX_HOSTS: usize = 8;
/// The most bytes read of a process's `comm` (the kernel keeps 16).
pub const MAX_COMM_BYTES: u64 = 64;

/// Why a registration was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RegisterError {
    /// The service string does not name an item.
    #[error("the service string does not name an item")]
    Malformed(#[source] ItemIdError),
    /// The tray holds [`MAX_ITEMS`] items already.
    #[error("the tray is full")]
    Full,
    /// This connection registered [`MAX_ITEMS_PER_SENDER`] items already.
    #[error("this connection registered too many items")]
    SenderFull,
}

/// One registered item: who serves it and which connection registered it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registration {
    /// The item.
    pub id: ItemId,
    /// The unique name of the connection that registered it.
    pub sender: String,
}

/// The item a peer registers: `service` as `RegisterStatusNotifierItem`
/// receives it, from `sender`. A bare object path (`/org/ayatana/...`, as
/// libappindicator sends) is served by the sender itself.
///
/// # Errors
///
/// [`ItemIdError`] for a malformed or oversized string.
pub fn registered_item(sender: &str, service: &str) -> Result<ItemId, ItemIdError> {
    if service.starts_with('/') {
        ItemId::parse(&format!("{sender}{service}"))
    } else {
        ItemId::parse(service)
    }
}

/// The registration string of an item, as the `RegisteredStatusNotifierItems`
/// property and the signals carry it: name and path.
#[must_use]
pub fn service_of(id: &ItemId) -> String {
    format!("{}{}", id.bus_name, id.path)
}

/// The watcher's bookkeeping, shared between the served object and the host
/// worker.
#[derive(Debug, Default)]
pub struct WatcherState {
    items: Vec<Registration>,
    hosts: Vec<String>,
    generation: u64,
}

impl WatcherState {
    /// Records the item `service` from `sender`; a repeat is not an error.
    ///
    /// # Errors
    ///
    /// [`RegisterError`] when the string is malformed or a bound is reached.
    pub fn register_item(&mut self, sender: &str, service: &str) -> Result<ItemId, RegisterError> {
        let id = registered_item(sender, service).map_err(RegisterError::Malformed)?;
        if self.items.iter().any(|r| r.id == id) {
            return Ok(id);
        }
        if self.items.len() >= MAX_ITEMS {
            return Err(RegisterError::Full);
        }
        if self.items.iter().filter(|r| r.sender == sender).count() >= MAX_ITEMS_PER_SENDER {
            return Err(RegisterError::SenderFull);
        }
        self.items.push(Registration {
            id: id.clone(),
            sender: sender.to_owned(),
        });
        self.generation += 1;
        Ok(id)
    }

    /// Records a host connection; past [`MAX_HOSTS`] it is accepted but not
    /// remembered.
    pub fn register_host(&mut self, sender: &str) {
        if self.hosts.iter().any(|h| h == sender) || self.hosts.len() >= MAX_HOSTS {
            return;
        }
        self.hosts.push(sender.to_owned());
    }

    /// Forgets the item `id`; whether it was known.
    pub fn remove_item(&mut self, id: &ItemId) -> bool {
        let before = self.items.len();
        self.items.retain(|r| &r.id != id);
        let removed = self.items.len() != before;
        if removed {
            self.generation += 1;
        }
        removed
    }

    /// Forgets every item and host that `name` (a unique or well-known name
    /// that just lost its owner) registered or is served by. Returns the
    /// items forgotten.
    pub fn name_gone(&mut self, name: &str) -> Vec<ItemId> {
        let (gone, kept): (Vec<_>, Vec<_>) = self
            .items
            .drain(..)
            .partition(|r| r.sender == name || r.id.bus_name == name);
        self.items = kept;
        self.hosts.retain(|h| h != name);
        if !gone.is_empty() {
            self.generation += 1;
        }
        gone.into_iter().map(|r| r.id).collect()
    }

    /// The registered items, in registration order.
    #[must_use]
    pub fn items(&self) -> &[Registration] {
        &self.items
    }

    /// The remembered foreign hosts.
    #[must_use]
    pub fn hosts(&self) -> &[String] {
        &self.hosts
    }

    /// A counter that changes with every change of the item list, so the
    /// host worker can tell whether to look again.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// The shared handle on the bookkeeping.
pub type SharedState = Arc<Mutex<WatcherState>>;

/// What the served object calls after a change, to wake the host worker.
pub type Poke = Box<dyn Fn() + Send + Sync>;

/// The served `org.kde.StatusNotifierWatcher` object.
pub struct Watcher {
    state: SharedState,
    poke: Poke,
}

impl Watcher {
    /// A watcher over `state` that calls `poke` after every change.
    #[must_use]
    pub fn new(state: SharedState, poke: Poke) -> Self {
        Self { state, poke }
    }

    fn with_state<T>(&self, f: impl FnOnce(&mut WatcherState) -> T) -> T {
        // A poisoned lock means a worker panicked while holding it; the
        // bookkeeping is plain data and stays usable.
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        f(&mut guard)
    }
}

impl std::fmt::Debug for Watcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Watcher").finish_non_exhaustive()
    }
}

fn sender_of(header: &Header<'_>) -> zbus::fdo::Result<String> {
    header
        .sender()
        .map(|name| name.as_str().to_owned())
        .ok_or_else(|| zbus::fdo::Error::Failed("the call has no sender".to_owned()))
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    /// Registers the item `service` (a bus name, a name and path, or a bare
    /// path served by the caller).
    async fn register_status_notifier_item(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        service: &str,
    ) -> zbus::fdo::Result<()> {
        let sender = sender_of(&header)?;
        let id = self
            .with_state(|state| state.register_item(&sender, service))
            .map_err(|err| zbus::fdo::Error::InvalidArgs(err.to_string()))?;
        (self.poke)();
        Self::status_notifier_item_registered(&emitter, &service_of(&id)).await?;
        self.registered_status_notifier_items_changed(&emitter)
            .await?;
        Ok(())
    }

    /// Registers the caller as a host; the shell is one itself, so the
    /// property is true regardless.
    async fn register_status_notifier_host(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        _service: &str,
    ) -> zbus::fdo::Result<()> {
        let sender = sender_of(&header)?;
        self.with_state(|state| state.register_host(&sender));
        Self::status_notifier_host_registered(&emitter).await?;
        Ok(())
    }

    /// The registered items as `name/path` strings.
    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.with_state(|state| state.items().iter().map(|r| service_of(&r.id)).collect())
    }

    /// Always true while the shell runs: the shell is a host.
    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    /// [`PROTOCOL_VERSION`].
    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        PROTOCOL_VERSION
    }

    #[zbus(signal)]
    async fn status_notifier_item_registered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_item_unregistered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_host_registered(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_host_unregistered(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

/// Announces on `connection` that the item `id` left the watcher, from the
/// host worker (a peer that vanished without unregistering).
///
/// # Errors
///
/// The bus's failure to send.
pub fn announce_unregistered(connection: &Connection, id: &ItemId) -> zbus::Result<()> {
    connection.emit_signal(
        None::<&str>,
        WATCHER_PATH,
        WATCHER_INTERFACE,
        "StatusNotifierItemUnregistered",
        &(service_of(id),),
    )
}

/// The `comm` of process `pid` from `/proc`, bounded and cleaned, or `None`.
#[must_use]
pub fn comm_of(pid: u32) -> Option<String> {
    comm_in(Path::new("/proc"), pid)
}

fn comm_in(proc_root: &Path, pid: u32) -> Option<String> {
    let bytes = celestina_core::atomic_file::read_bounded(
        &proc_root.join(pid.to_string()).join("comm"),
        MAX_COMM_BYTES,
    )
    .ok()
    .flatten()?;
    let text = String::from_utf8_lossy(&bytes);
    let clean: String = text
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(32)
        .collect();
    (!clean.is_empty()).then_some(clean)
}

/// Who holds [`WATCHER_NAME`] on `connection`'s bus: the unique name and,
/// when the bus gives the pid and `/proc` names it, the process. `None`
/// when nobody holds it or the bus will not say. The page's copy composes
/// the line a person reads.
#[must_use]
pub fn name_holder(connection: &Connection) -> Option<NameHolder> {
    let dbus = DBusProxy::new(connection).ok()?;
    let name = zbus::names::BusName::try_from(WATCHER_NAME).ok()?;
    let unique = dbus.get_name_owner(name).ok()?;
    let comm = dbus
        .get_connection_unix_process_id(zbus::names::BusName::from(unique.inner().clone()))
        .ok()
        .and_then(comm_of)
        .filter(|comm| !comm.is_empty());
    Some(NameHolder {
        unique: unique.as_str().to_owned(),
        comm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tray::model::DEFAULT_ITEM_PATH;

    #[test]
    fn a_bare_path_is_served_by_the_sender_and_a_name_keeps_its_own() {
        assert_eq!(
            registered_item(":1.5", "/org/ayatana/NotificationItem/x"),
            Ok(ItemId {
                bus_name: ":1.5".into(),
                path: "/org/ayatana/NotificationItem/x".into()
            })
        );
        assert_eq!(
            registered_item(":1.5", "org.kde.foo"),
            Ok(ItemId {
                bus_name: "org.kde.foo".into(),
                path: DEFAULT_ITEM_PATH.into()
            })
        );
        assert_eq!(
            registered_item(":1.5", ":1.9/StatusNotifierItem").map(|i| i.bus_name),
            Ok(":1.9".to_owned())
        );
        assert!(registered_item(":1.5", "").is_err());
        assert!(registered_item(":1.5", "bad name").is_err());
        assert_eq!(
            service_of(&ItemId {
                bus_name: ":1.5".into(),
                path: "/x".into()
            }),
            ":1.5/x"
        );
    }

    #[test]
    fn registrations_are_bounded_per_sender_and_in_total() {
        let mut state = WatcherState::default();
        for n in 0..MAX_ITEMS_PER_SENDER {
            assert!(state.register_item(":1.1", &format!("/item{n}")).is_ok());
        }
        assert_eq!(
            state.register_item(":1.1", "/one_more"),
            Err(RegisterError::SenderFull)
        );
        // A repeat of a known item is not a new registration.
        assert!(state.register_item(":1.1", "/item0").is_ok());
        assert_eq!(state.items().len(), MAX_ITEMS_PER_SENDER);

        let mut n = 0;
        while state.items().len() < MAX_ITEMS {
            n += 1;
            assert!(state
                .register_item(&format!(":1.{}", 100 + n), "/StatusNotifierItem")
                .is_ok());
        }
        assert_eq!(
            state.register_item(":1.999", "/StatusNotifierItem"),
            Err(RegisterError::Full)
        );
        assert!(matches!(
            state.register_item(":1.999", "x y"),
            Err(RegisterError::Malformed(_))
        ));
    }

    #[test]
    fn a_vanished_name_takes_its_registrations_with_it() {
        let mut state = WatcherState::default();
        state.register_item(":1.1", "/a").expect("registered");
        state
            .register_item(":1.2", "org.kde.foo")
            .expect("registered");
        state.register_item(":1.3", "/c").expect("registered");
        state.register_host(":1.1");
        state.register_host(":1.7");
        let before = state.generation();

        let gone = state.name_gone("org.kde.foo");
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].bus_name, "org.kde.foo");
        let gone = state.name_gone(":1.1");
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].path, "/a");
        assert_eq!(state.items().len(), 1);
        assert_eq!(state.hosts(), [":1.7".to_owned()]);
        assert!(state.generation() > before);

        assert!(state.name_gone(":1.unknown").is_empty());
        assert!(!state.remove_item(&gone[0]), "already forgotten");
        let last = state.items()[0].id.clone();
        assert!(state.remove_item(&last));
        assert!(state.items().is_empty());
    }

    #[test]
    fn hosts_are_bounded() {
        let mut state = WatcherState::default();
        for n in 0..(MAX_HOSTS + 3) {
            state.register_host(&format!(":1.{n}"));
        }
        assert_eq!(state.hosts().len(), MAX_HOSTS);
    }

    #[test]
    fn comm_is_read_bounded_and_cleaned() {
        let dir = std::env::temp_dir().join(format!("celestina-comm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("42")).expect("a proc dir");
        std::fs::write(dir.join("42").join("comm"), "noc\u{7}talia\n").expect("a comm");
        assert_eq!(comm_in(&dir, 42).as_deref(), Some("noctalia"));
        std::fs::write(dir.join("42").join("comm"), "x".repeat(200)).expect("a comm");
        assert_eq!(comm_in(&dir, 42), None, "a comm past the bound is refused");
        assert_eq!(comm_in(&dir, 43), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
