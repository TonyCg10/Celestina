//! The tray host: the watcher served and claimed, the registered items read
//! and followed, the page's commands sent (design §5.5, §5.6).
//!
//! Two workers on one connection of their own, started by [`spawn`]:
//!
//! - **tray-host** — connects, serves the watcher, claims its name with
//!   `DoNotQueue` (a taken name disables the tray for the session and
//!   publishes who holds it), registers the shell as a host, then loops:
//!   it reads the items the watcher registered and the ones that signalled a
//!   change, at most once per coalescing window and for at most
//!   [`READ_BUDGET`] per pass (the rest waits for the next pass), publishes
//!   the list when it changed, and sends the commands the page queues,
//!   also between two item reads. Every call has the connection's short
//!   timeout and an item's whole read has its own budget, so a slow peer
//!   costs at most that, and no more than once per [`MIN_REREAD`]; the
//!   last item read stays published until the re-read replaces it.
//! - **tray-signals** — drains the connection's message stream without
//!   ever blocking, so a burst from a peer can never stall the connection's
//!   reader (zbus applies backpressure to a full stream), and records what
//!   the host must look at: items and menus that changed, each once, and
//!   names that changed hands, only for the items, menus and names the host
//!   knows (a stranger's signals, and a peer's from paths it did not
//!   register, count for nothing). It wakes the host with one queued poke
//!   at a time, so a burst never takes the page's command slots.
//!
//! Both look at the shell's shutdown flag at least every [`POLL`]; the host
//! owns the signal worker and joins it before it leaves.

use std::{
    collections::HashSet,
    io,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError},
        Arc, Mutex,
    },
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

use celestina_shell::coalesce::Coalescer;
use celestina_wayland::Sender;
use zbus::{
    blocking::{connection::Builder, fdo::DBusProxy, Connection, MessageIterator},
    export::futures_core::Stream,
    fdo::{RequestNameFlags, RequestNameReply},
    message::Type,
    MatchRule, MessageStream,
};

use crate::{
    message::Message,
    providers::Worker,
    tray::{
        dbusmenu::DbusMenu,
        item::{self, BusSignal, ItemAddress, ItemCall, MenuClient, Read},
        model::{bound_items, ItemsCoalescer, TrayCommand, TrayEvent, TrayItem},
        watcher::{self, Registration, SharedState, Watcher, WATCHER_NAME, WATCHER_PATH},
        ItemId,
    },
};

/// How many commands may wait for the host worker; a full queue refuses at
/// once, never building a backlog behind a silent peer.
pub const COMMAND_QUEUE: usize = 64;
/// How long one call to a peer may take.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(1);
/// The host name the shell registers: this prefix, its pid and `-1`.
pub const HOST_NAME_PREFIX: &str = "org.freedesktop.StatusNotifierHost-";
/// How many signals of one kind the signal worker remembers between two
/// looks of the host. Only known items, menus and names are recorded, each
/// item and menu once, so the bound is never reached by the watcher's
/// [`MAX_ITEMS`](crate::tray::model::MAX_ITEMS) items; a name that changes
/// hands over and over is bounded by [`MAX_OWNER_CHANGES_PER_NAME`] first.
pub const MAX_PENDING_SIGNALS: usize = 256;
/// How many changes of one known name's owner are remembered in order
/// between two looks of the host; past this the name counts as churned,
/// the host resolves its owner through the bus, and the rest are dropped.
pub const MAX_OWNER_CHANGES_PER_NAME: usize = 8;
/// How long an item that did not answer (nothing served at its path, or
/// silence past the call timeout) is left alone before it is read again,
/// unless it signals first. Without this a silent registration would cost
/// the timeout at every refresh.
pub const RETRY_AFTER: Duration = Duration::from_secs(30);
/// The least time between two reads of one item: a peer signalling a change
/// every few milliseconds is read once a second, not once per window. The
/// last item read stays published in the meantime.
pub const MIN_REREAD: Duration = Duration::from_secs(1);
/// How long one refresh pass may spend reading items; the entries left
/// unread wait for the next pass, so a queued command or the shutdown flag
/// is looked at within about this long plus one item's read.
pub const READ_BUDGET: Duration = Duration::from_millis(300);
/// How often a blocked worker looks at the shutdown flag.
const POLL: Duration = Duration::from_millis(100);

/// What the host worker receives.
#[derive(Debug)]
enum Inbox {
    Command(TrayCommand),
    /// Something changed: look at the watcher's state and the signals.
    Poke,
}

/// The loop's handle on the tray: the bounded command queue.
#[derive(Debug)]
pub struct TrayProvider {
    inbox: SyncSender<Inbox>,
}

impl TrayProvider {
    /// Queues a command for the host worker.
    ///
    /// # Errors
    ///
    /// The command back, when [`COMMAND_QUEUE`] commands are already waiting
    /// or the tray is disabled (its worker has stopped).
    pub fn command(&self, command: TrayCommand) -> Result<(), TrayCommand> {
        match self.inbox.try_send(Inbox::Command(command)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(refused) | TrySendError::Disconnected(refused)) => {
                match refused {
                    Inbox::Command(command) => Err(command),
                    // Only commands are queued through this handle.
                    Inbox::Poke => Ok(()),
                }
            }
        }
    }
}

/// Where the worker puts an event; `false` once the loop is gone.
pub type Publish = Box<dyn FnMut(TrayEvent) -> bool + Send>;

/// Starts the tray on the session bus, publishing [`Message::Tray`] through
/// `sender`.
///
/// # Errors
///
/// The failure to start the host thread.
pub fn spawn(
    sender: Sender<Message>,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(TrayProvider, Worker)> {
    let publish: Publish = Box::new(move |event| sender.send(Message::Tray(event)).is_ok());
    spawn_on(Builder::session(), publish, production_menu(), shutdown)
}

/// The menu client the production tray serves menus with: the DBusMenu
/// client. A test that starts the host with this function proves the
/// production wiring, not a stand-in.
#[must_use]
pub fn production_menu() -> Box<dyn MenuClient> {
    Box::new(DbusMenu::default())
}

/// Starts the tray on the bus `builder` connects to, with `menu` serving
/// the menu commands.
///
/// # Errors
///
/// The failure to start the host thread.
pub fn spawn_on(
    builder: zbus::Result<Builder<'static>>,
    publish: Publish,
    menu: Box<dyn MenuClient>,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(TrayProvider, Worker)> {
    let (inbox, queue) = mpsc::sync_channel(COMMAND_QUEUE);
    let poker = Poker::new(inbox.clone());
    let flag = Arc::clone(shutdown);
    let worker = Worker::spawn("tray-host", shutdown, move || {
        host_worker(builder, publish, menu, queue, poker, &flag);
    })?;
    Ok((TrayProvider { inbox }, worker))
}

/// Wakes the host: however often it is poked, at most one `Poke` waits in
/// the queue, so a burst of signals or registrations never takes the
/// slots the page's commands need.
#[derive(Clone, Debug)]
struct Poker {
    queued: Arc<AtomicBool>,
    inbox: SyncSender<Inbox>,
}

impl Poker {
    fn new(inbox: SyncSender<Inbox>) -> Poker {
        Poker {
            queued: Arc::new(AtomicBool::new(false)),
            inbox,
        }
    }

    fn poke(&self) {
        if self.queued.swap(true, Ordering::AcqRel) {
            return;
        }
        if self.inbox.try_send(Inbox::Poke).is_err() {
            // The queue is full of commands: the host is awake and looks
            // at its work after each of them.
            self.queued.store(false, Ordering::Release);
        }
    }

    /// The host took the queued poke: the next one is sent again.
    fn taken(&self) {
        self.queued.store(false, Ordering::Release);
    }
}

/// The signals recorded since the host last looked.
#[derive(Debug, Default)]
struct Signals {
    /// Items (sender, path) that signalled a change, each once.
    changed: Vec<(String, String)>,
    /// Known names' owner changes, in order.
    owner_changes: Vec<(String, String)>,
    /// Menus (sender, path) that signalled a change, each once.
    menus: Vec<(String, String)>,
    /// Names whose owner changed more than [`MAX_OWNER_CHANGES_PER_NAME`]
    /// times: the host resolves them through the bus instead.
    churned: Vec<String>,
}

impl Signals {
    fn is_empty(&self) -> bool {
        self.changed.is_empty()
            && self.owner_changes.is_empty()
            && self.menus.is_empty()
            && self.churned.is_empty()
    }
}

/// What the signal worker recorded for the host, and what the host told it
/// to listen for: the (name, path) pairs its items are served at, the
/// (name, path) pairs of their menus and the names (unique and well-known)
/// they involve. A signal from anyone else, or from a known sender about a
/// path it did not register, is dropped unrecorded, so neither a stranger
/// nor a registered peer can fill the bounds with paths it invents.
#[derive(Debug, Default)]
struct Pending {
    signals: Signals,
    known_items: HashSet<(String, String)>,
    known_menus: HashSet<(String, String)>,
    known_names: HashSet<String>,
}

impl Pending {
    fn record(&mut self, signal: BusSignal) -> bool {
        let signals = &mut self.signals;
        match signal {
            // A repeated change needs one read.
            BusSignal::ItemChanged { sender, path } => {
                if !self.known_items.contains(&(sender.clone(), path.clone())) {
                    return false;
                }
                push_once(&mut signals.changed, (sender, path));
            }
            BusSignal::MenuChanged { sender, path } => {
                if !self.known_menus.contains(&(sender.clone(), path.clone())) {
                    return false;
                }
                push_once(&mut signals.menus, (sender, path));
            }
            // An owner's history is replayed in order (a well-known name may
            // go and come back), up to a point: past it the name is churned.
            BusSignal::NameOwnerChanged { name, new_owner } => {
                if !self.known_names.contains(&name) {
                    return false;
                }
                if signals.churned.contains(&name) {
                    return false;
                }
                let changes = signals
                    .owner_changes
                    .iter()
                    .filter(|(n, _)| *n == name)
                    .count();
                if changes >= MAX_OWNER_CHANGES_PER_NAME
                    || signals.owner_changes.len() >= MAX_PENDING_SIGNALS
                {
                    signals.owner_changes.retain(|(n, _)| *n != name);
                    signals.churned.push(name);
                    return true;
                }
                signals.owner_changes.push((name, new_owner));
            }
        }
        true
    }

    fn take_signals(&mut self) -> Signals {
        std::mem::take(&mut self.signals)
    }

    fn is_empty(&self) -> bool {
        self.signals.is_empty()
    }
}

/// Records `value` once; a list is never longer than the known set it is
/// drawn from, and [`MAX_PENDING_SIGNALS`] stands behind that.
fn push_once<T: PartialEq>(into: &mut Vec<T>, value: T) {
    if into.contains(&value) || into.len() >= MAX_PENDING_SIGNALS {
        return;
    }
    into.push(value);
}

type SharedPending = Arc<Mutex<Pending>>;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // The data behind every lock here is plain bookkeeping: a panic on the
    // other side leaves it usable.
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// A waker that pokes a channel, so a worker can wait on its channel with a
/// timeout and still be woken by the stream.
struct ChannelWaker(SyncSender<()>);

impl Wake for ChannelWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        let _ = self.0.try_send(());
    }
}

/// Everything ready on `stream`, without blocking.
fn drain(
    stream: &mut MessageStream,
    cx: &mut Context<'_>,
    mut each: impl FnMut(zbus::message::Message),
) -> bool {
    loop {
        match Pin::new(&mut *stream).poll_next(cx) {
            Poll::Ready(Some(Ok(message))) => each(message),
            Poll::Ready(Some(Err(err))) => tracing::debug!(%err, "tray: the message stream failed"),
            Poll::Ready(None) => return false,
            Poll::Pending => return true,
        }
    }
}

fn signal_worker(
    connection: Connection,
    pending: SharedPending,
    poker: Poker,
    shutdown: &AtomicBool,
) {
    let mut stream = MessageIterator::from(&connection).into_inner();
    let (wake, woken) = mpsc::sync_channel(1);
    let waker = Waker::from(Arc::new(ChannelWaker(wake)));
    let mut cx = Context::from_waker(&waker);
    while !shutdown.load(Ordering::Acquire) {
        let mut any = false;
        let open = drain(&mut stream, &mut cx, |message| {
            if let Some(signal) = item::recognise(&message) {
                any |= lock(&pending).record(signal);
            }
        });
        if any {
            poker.poke();
        }
        if !open {
            return;
        }
        match woken.recv_timeout(POLL) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// One item the host follows.
#[derive(Debug)]
struct Entry {
    registration: Registration,
    /// Set once the item answered.
    address: Option<ItemAddress>,
    /// The last item read: published until the item leaves, even while a
    /// re-read is owed, so a row never vanishes and the rows below it never
    /// move up while the item is only changing.
    item: Option<TrayItem>,
    /// The item signalled a change since it was last read.
    stale: bool,
    /// Not read again before this, after it did not answer or was read
    /// less than [`MIN_REREAD`] ago.
    retry_at: Option<Instant>,
    /// When it was last read.
    read_at: Option<Instant>,
}

impl Entry {
    fn new(registration: Registration) -> Entry {
        Entry {
            registration,
            address: None,
            item: None,
            stale: true,
            retry_at: None,
            read_at: None,
        }
    }

    /// The item must be read again: at once, or once [`MIN_REREAD`] has
    /// passed since its last read. The last item stays published meanwhile.
    fn invalidate(&mut self, now: Instant) {
        self.stale = true;
        self.retry_at = self
            .read_at
            .map(|at| at + MIN_REREAD)
            .filter(|at| *at > now);
    }

    /// Whether the item is owed a read at `now`.
    fn needs_read(&self, now: Instant) -> bool {
        (self.stale || self.item.is_none()) && self.retry_at.is_none_or(|at| at <= now)
    }

    /// The names a signal about this entry comes from or names.
    fn names(&self) -> impl Iterator<Item = &str> {
        [
            Some(self.registration.sender.as_str()),
            Some(self.registration.id.bus_name.as_str()),
            self.address.as_ref().map(|a| a.owner.as_str()),
        ]
        .into_iter()
        .flatten()
    }
}

/// The host worker's state.
struct Host {
    connection: Connection,
    dbus: DBusProxy<'static>,
    state: SharedState,
    pending: SharedPending,
    entries: Vec<Entry>,
    seen_generation: u64,
    reads: Coalescer,
    publishing: ItemsCoalescer,
    last: Option<Vec<TrayItem>>,
    start: Instant,
    publish: Publish,
    menu: Box<dyn MenuClient>,
    poker: Poker,
}

/// Why the host stopped.
#[derive(Debug)]
enum Stopped {
    Shutdown,
    LoopGone,
}

fn host_worker(
    builder: zbus::Result<Builder<'static>>,
    mut publish: Publish,
    menu: Box<dyn MenuClient>,
    queue: Receiver<Inbox>,
    poker: Poker,
    shutdown: &Arc<AtomicBool>,
) {
    let connection = match builder.and_then(|b| b.method_timeout(CALL_TIMEOUT).build()) {
        Ok(connection) => connection,
        Err(err) => {
            tracing::warn!(%err, "tray: no session bus; the tray is disabled");
            return;
        }
    };
    let dbus = match DBusProxy::new(&connection) {
        Ok(dbus) => dbus,
        Err(err) => {
            tracing::warn!(%err, "tray: no bus proxy; the tray is disabled");
            return;
        }
    };
    let state = SharedState::default();
    let host_poke = {
        let poker = poker.clone();
        Box::new(move || poker.poke())
    };
    match claim(&connection, Watcher::new(Arc::clone(&state), host_poke)) {
        Ok(()) => {}
        Err(Claim::Taken) => {
            // The bus would not say who: the page still says someone does.
            let holder = watcher::name_holder(&connection).unwrap_or_default();
            tracing::info!(
                unique = %holder.unique,
                comm = holder.comm.as_deref().unwrap_or(""),
                "tray: {WATCHER_NAME} is held by another program; the tray is disabled for the session"
            );
            publish(TrayEvent::Owner(Some(holder)));
            return;
        }
        Err(Claim::Bus(err)) => {
            tracing::warn!(%err, "tray: cannot serve the watcher; the tray is disabled");
            return;
        }
    }
    if let Err(err) = register_as_host(&connection, &state) {
        tracing::debug!(%err, "tray: the host registration was not announced");
    }
    if let Err(err) = subscribe(&dbus) {
        tracing::warn!(%err, "tray: cannot follow the bus; items will not update");
    }
    let pending = SharedPending::default();
    let signals = {
        let connection = connection.clone();
        let pending = Arc::clone(&pending);
        let flag = Arc::clone(shutdown);
        let poker = poker.clone();
        Worker::spawn("tray-signals", shutdown, move || {
            signal_worker(connection, pending, poker, &flag);
        })
    };
    let signals = match signals {
        Ok(worker) => worker,
        Err(err) => {
            tracing::warn!(%err, "tray: cannot start the signal worker; the tray is disabled");
            let _ = connection.release_name(WATCHER_NAME);
            return;
        }
    };
    if !publish(TrayEvent::Owner(None)) || !publish(TrayEvent::Items(Vec::new())) {
        return;
    }
    let mut host = Host {
        connection: connection.clone(),
        dbus,
        state,
        pending,
        entries: Vec::new(),
        seen_generation: 0,
        reads: Coalescer::default(),
        publishing: ItemsCoalescer::default(),
        last: Some(Vec::new()),
        start: Instant::now(),
        publish,
        menu,
        poker,
    };
    let stopped = host.run(&queue, shutdown);
    tracing::debug!(?stopped, "tray: the host stops");
    if let Err(err) = connection.release_name(WATCHER_NAME) {
        tracing::debug!(%err, "releasing {WATCHER_NAME}");
    }
    signals.join();
}

/// Why the watcher name was not claimed.
#[derive(Debug)]
enum Claim {
    Taken,
    Bus(zbus::Error),
}

/// Serves `watcher` at [`WATCHER_PATH`], then claims [`WATCHER_NAME`] with
/// `DoNotQueue`; a taken name withdraws the object again.
fn claim(connection: &Connection, watcher: Watcher) -> Result<(), Claim> {
    connection
        .object_server()
        .at(WATCHER_PATH, watcher)
        .map_err(Claim::Bus)?;
    match connection.request_name_with_flags(WATCHER_NAME, RequestNameFlags::DoNotQueue.into()) {
        Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => Ok(()),
        Ok(RequestNameReply::Exists | RequestNameReply::InQueue) | Err(zbus::Error::NameTaken) => {
            if let Err(err) = connection
                .object_server()
                .remove::<Watcher, _>(WATCHER_PATH)
            {
                tracing::debug!(%err, "tray: withdrawing the watcher object");
            }
            Err(Claim::Taken)
        }
        Err(err) => Err(Claim::Bus(err)),
    }
}

/// Registers the shell as a host: its own host name, its unique name in the
/// watcher's books, and the protocol's signal.
fn register_as_host(connection: &Connection, state: &SharedState) -> zbus::Result<()> {
    let host_name = format!("{HOST_NAME_PREFIX}{}-1", std::process::id());
    if let Err(err) =
        connection.request_name_with_flags(host_name.as_str(), RequestNameFlags::DoNotQueue.into())
    {
        tracing::debug!(%err, "tray: the host name {host_name} was not claimed");
    }
    if let Some(unique) = connection.unique_name() {
        lock(state).register_host(unique.as_str());
    }
    connection.emit_signal(
        None::<&str>,
        WATCHER_PATH,
        watcher::WATCHER_INTERFACE,
        "StatusNotifierHostRegistered",
        &(),
    )
}

/// The unique name serving `id`, resolving a well-known name through the
/// bus.
fn owner_of(dbus: &DBusProxy<'_>, id: &ItemId) -> Option<String> {
    if id.bus_name.starts_with(':') {
        return Some(id.bus_name.clone());
    }
    owner_of_name(dbus, &id.bus_name)
}

/// The unique name holding `name` now, as the bus says; `None` when nobody
/// does or the bus did not answer.
fn owner_of_name(dbus: &DBusProxy<'_>, name: &str) -> Option<String> {
    let name = zbus::names::BusName::try_from(name).ok()?;
    dbus.get_name_owner(name)
        .ok()
        .map(|unique| unique.as_str().to_owned())
}

/// Asks the bus for the signals the tray acts on.
fn subscribe(dbus: &DBusProxy<'_>) -> zbus::Result<()> {
    let owners = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender("org.freedesktop.DBus")?
        .interface("org.freedesktop.DBus")?
        .member("NameOwnerChanged")?
        .build();
    dbus.add_match_rule(owners)?;
    for interface in item::ITEM_INTERFACES {
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .interface(interface)?
            .build();
        dbus.add_match_rule(rule)?;
    }
    let menus = MatchRule::builder()
        .msg_type(Type::Signal)
        .interface(crate::tray::dbusmenu::MENU_INTERFACE)?
        .build();
    dbus.add_match_rule(menus)?;
    Ok(())
}

impl Host {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn run(&mut self, queue: &Receiver<Inbox>, shutdown: &AtomicBool) -> Stopped {
        loop {
            if shutdown.load(Ordering::Acquire) {
                return Stopped::Shutdown;
            }
            let now = self.now_ms();
            if self.work_owed() {
                self.reads.mark();
                if self.reads.due(now) {
                    self.refresh(queue, shutdown);
                    self.reads.emitted(now);
                }
            }
            if let Some(items) = self.publishing.take_due(self.now_ms()) {
                if self.last.as_ref() != Some(&items) {
                    if !(self.publish)(TrayEvent::Items(items.clone())) {
                        return Stopped::LoopGone;
                    }
                    self.last = Some(items);
                }
            }
            let now = self.now_ms();
            let mut wait = POLL;
            if let Some(ms) = self.reads.wait_ms(now) {
                wait = wait.min(Duration::from_millis(ms.max(1)));
            }
            if let Some(ms) = self.publishing.wait_ms(now) {
                wait = wait.min(Duration::from_millis(ms.max(1)));
            }
            match queue.recv_timeout(wait) {
                Ok(inbox) => self.take(inbox),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Stopped::LoopGone,
            }
        }
    }

    /// Acts on one thing from the queue.
    fn take(&mut self, inbox: Inbox) {
        match inbox {
            Inbox::Command(command) => self.perform(command),
            Inbox::Poke => self.poker.taken(),
        }
    }

    /// Everything waiting in the queue, without blocking: between two item
    /// reads, so a slow peer does not delay the page's commands. `false`
    /// once the loop is gone.
    fn drain_queue(&mut self, queue: &Receiver<Inbox>) -> bool {
        loop {
            match queue.try_recv() {
                Ok(inbox) => self.take(inbox),
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => return false,
            }
        }
    }

    /// Whether the watcher or the signals changed something since the last
    /// refresh, or an entry is owed a read.
    fn work_owed(&self) -> bool {
        let now = Instant::now();
        lock(&self.state).generation() != self.seen_generation
            || !lock(&self.pending).is_empty()
            || self.entries.iter().any(|e| e.needs_read(now))
    }

    /// Brings the entries up to date with the watcher and the signals, and
    /// hands the new list to the publishing coalescer.
    fn refresh(&mut self, queue: &Receiver<Inbox>, shutdown: &AtomicBool) {
        let now = Instant::now();
        let signals = lock(&self.pending).take_signals();
        for (name, new_owner) in &signals.owner_changes {
            self.owner_changed(name, new_owner, now);
        }
        for name in &signals.churned {
            // The name's history was too long to keep: the bus says where
            // it stands now, and only that name's entries are touched.
            let owner = owner_of_name(&self.dbus, name).unwrap_or_default();
            self.owner_changed(name, &owner, now);
        }
        self.sync_registrations();
        for (sender, path) in &signals.changed {
            for entry in &mut self.entries {
                let same_owner = entry.address.as_ref().map_or(
                    entry.registration.sender == *sender
                        || entry.registration.id.bus_name == *sender,
                    |a| a.owner == *sender,
                );
                if same_owner && entry.registration.id.path == *path {
                    entry.invalidate(now);
                }
            }
        }
        for (sender, path) in &signals.menus {
            self.menu
                .menu_changed(&self.connection, sender, path, &mut self.publish);
        }
        self.read_missing(queue, shutdown);
        self.tell_known();
        let items: Vec<TrayItem> = self.entries.iter().filter_map(|e| e.item.clone()).collect();
        self.publishing.push(bound_items(items));
    }

    /// Tells the signal worker which items, menus and names to listen for.
    fn tell_known(&self) {
        let mut known_items = HashSet::new();
        let mut known_menus = HashSet::new();
        let mut known_names = HashSet::new();
        for entry in &self.entries {
            let menu = entry.address.as_ref().and_then(|a| a.menu.as_deref());
            for name in entry.names() {
                known_items.insert((name.to_owned(), entry.registration.id.path.clone()));
                if let Some(menu) = menu {
                    known_menus.insert((name.to_owned(), menu.to_owned()));
                }
                known_names.insert(name.to_owned());
            }
        }
        let mut pending = lock(&self.pending);
        pending.known_items = known_items;
        pending.known_menus = known_menus;
        pending.known_names = known_names;
    }

    /// `name` has `new_owner` (empty when released).
    fn owner_changed(&mut self, name: &str, new_owner: &str, now: Instant) {
        if new_owner.is_empty() {
            let gone = lock(&self.state).name_gone(name);
            for id in &gone {
                if let Err(err) = watcher::announce_unregistered(&self.connection, id) {
                    tracing::debug!(%err, "tray: announcing an item that left");
                }
            }
            self.entries.retain(|e| {
                !(gone.contains(&e.registration.id)
                    || e.address.as_ref().is_some_and(|a| a.owner == name))
            });
        } else {
            for entry in &mut self.entries {
                if entry.registration.id.bus_name == name {
                    if let Some(address) = &mut entry.address {
                        address.owner = new_owner.to_owned();
                    }
                    entry.invalidate(now);
                }
            }
        }
    }

    /// Adds the watcher's new registrations and drops the ones it forgot.
    fn sync_registrations(&mut self) {
        let registrations: Vec<Registration> = {
            let state = lock(&self.state);
            self.seen_generation = state.generation();
            state.items().to_vec()
        };
        self.entries
            .retain(|e| registrations.contains(&e.registration));
        for registration in registrations {
            if !self.entries.iter().any(|e| e.registration == registration) {
                self.entries.push(Entry::new(registration));
            }
        }
    }

    /// Reads the entries owed a read, for at most [`READ_BUDGET`]; the rest
    /// wait for the next pass. The queue is drained and the shutdown flag
    /// looked at before every read.
    fn read_missing(&mut self, queue: &Receiver<Inbox>, shutdown: &AtomicBool) {
        let mut drop_ids = Vec::new();
        let started = Instant::now();
        for index in 0..self.entries.len() {
            if shutdown.load(Ordering::Acquire) || !self.drain_queue(queue) {
                return;
            }
            if started.elapsed() > READ_BUDGET {
                tracing::debug!("tray: the read budget is spent; the rest waits for the next pass");
                break;
            }
            let now = Instant::now();
            let entry = &self.entries[index];
            if !entry.needs_read(now) {
                continue;
            }
            let id = entry.registration.id.clone();
            let owner = match entry.address.as_ref().map(|a| a.owner.clone()) {
                Some(owner) => owner,
                None => match owner_of(&self.dbus, &id) {
                    Some(owner) => owner,
                    None => {
                        drop_ids.push(id);
                        continue;
                    }
                },
            };
            let read = item::read_item(&self.connection, &id, &owner);
            let entry = &mut self.entries[index];
            entry.read_at = Some(Instant::now());
            entry.stale = false;
            match read {
                Read::Item(served) => {
                    entry.address = Some(served.address);
                    entry.item = Some(served.item);
                    entry.retry_at = None;
                }
                Read::NotServed => {
                    // The object is gone from the peer: nothing to show
                    // until it signals or the retry finds it again.
                    tracing::debug!(item = %id.bus_name, path = %id.path, "tray: the item answers on no interface");
                    entry.item = None;
                    entry.retry_at = Some(now + RETRY_AFTER);
                }
                Read::Gone => drop_ids.push(id),
                Read::Unresponsive => {
                    // Silence: the last item, if any, stays listed.
                    tracing::debug!(item = %id.bus_name, path = %id.path, "tray: the item did not answer in time");
                    entry.retry_at = Some(now + RETRY_AFTER);
                }
            }
        }
        for id in drop_ids {
            if lock(&self.state).remove_item(&id) {
                if let Err(err) = watcher::announce_unregistered(&self.connection, &id) {
                    tracing::debug!(%err, "tray: announcing an item that left");
                }
            }
            self.entries.retain(|e| e.registration.id != id);
        }
        // A removal above bumps the watcher's generation, which costs one
        // redundant refresh; syncing it here instead would hide a
        // registration that arrived during the reads.
    }

    fn address_of(&self, id: &ItemId) -> Option<ItemAddress> {
        self.entries
            .iter()
            .find(|e| e.registration.id == *id)
            .and_then(|e| e.address.clone())
    }

    fn perform(&mut self, command: TrayCommand) {
        let id = match &command {
            TrayCommand::Activate(id)
            | TrayCommand::SecondaryActivate(id)
            | TrayCommand::Scroll(id, _, _)
            | TrayCommand::MenuEvent(id, _)
            | TrayCommand::AboutToShow(id, _)
            | TrayCommand::CloseMenu(id)
            | TrayCommand::OpenMenu(id) => id.clone(),
        };
        let Some(address) = self.address_of(&id) else {
            tracing::debug!(item = %id.bus_name, "tray: a command for an item that is not served");
            return;
        };
        let call = match command {
            TrayCommand::Activate(_) => ItemCall::Activate,
            TrayCommand::SecondaryActivate(_) => ItemCall::SecondaryActivate,
            TrayCommand::Scroll(_, delta, orientation) => ItemCall::Scroll(delta, orientation),
            TrayCommand::OpenMenu(_) => {
                self.menu
                    .open_menu(&self.connection, &address, &mut self.publish);
                return;
            }
            TrayCommand::AboutToShow(_, entry) => {
                self.menu
                    .about_to_show(&self.connection, &address, entry, &mut self.publish);
                return;
            }
            TrayCommand::MenuEvent(_, entry) => {
                self.menu.menu_event(&self.connection, &address, entry);
                return;
            }
            TrayCommand::CloseMenu(_) => {
                self.menu.close_menu(&address);
                return;
            }
        };
        if let Err(err) = item::send(&self.connection, &address, &call) {
            tracing::warn!(%err, item = %id.bus_name, "tray: {call:?} was not sent");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::{channel, Receiver};

    use zbus::zvariant::OwnedObjectPath;

    use super::*;
    use crate::tray::{
        item::NoMenu,
        model::{Orientation, Pixmap, Status},
        test_bus::PrivateBus,
        watcher::WATCHER_INTERFACE,
    };

    const ITEM_PATH: &str = "/StatusNotifierItem";
    const WAIT: Duration = Duration::from_secs(5);

    /// A status notifier item served in the test, recording its calls and
    /// answering each property after `delay`.
    struct FakeItem {
        title: String,
        icon_name: String,
        pixmaps: Vec<(i32, i32, Vec<u8>)>,
        menu: String,
        calls: Arc<Mutex<Vec<String>>>,
        delay: Duration,
    }

    #[zbus::interface(name = "org.kde.StatusNotifierItem")]
    impl FakeItem {
        #[zbus(property)]
        fn id(&self) -> String {
            std::thread::sleep(self.delay);
            "fake".to_owned()
        }

        #[zbus(property)]
        fn title(&self) -> String {
            std::thread::sleep(self.delay);
            self.title.clone()
        }

        #[zbus(property)]
        fn status(&self) -> String {
            std::thread::sleep(self.delay);
            "NeedsAttention".to_owned()
        }

        #[zbus(property)]
        fn icon_name(&self) -> String {
            std::thread::sleep(self.delay);
            self.icon_name.clone()
        }

        #[zbus(property)]
        fn icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
            std::thread::sleep(self.delay);
            self.pixmaps.clone()
        }

        #[zbus(property)]
        fn menu(&self) -> OwnedObjectPath {
            std::thread::sleep(self.delay);
            OwnedObjectPath::try_from(self.menu.as_str()).expect("a path")
        }

        fn activate(&self, x: i32, y: i32) {
            lock(&self.calls).push(format!("Activate({x},{y})"));
        }

        fn secondary_activate(&self, x: i32, y: i32) {
            lock(&self.calls).push(format!("SecondaryActivate({x},{y})"));
        }

        fn scroll(&self, delta: i32, orientation: &str) {
            lock(&self.calls).push(format!("Scroll({delta},{orientation})"));
        }
    }

    fn fake(
        title: &str,
        icon_name: &str,
        pixmaps: Vec<(i32, i32, Vec<u8>)>,
        menu: &str,
    ) -> (FakeItem, Arc<Mutex<Vec<String>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let item = FakeItem {
            title: title.to_owned(),
            icon_name: icon_name.to_owned(),
            pixmaps,
            menu: menu.to_owned(),
            calls: Arc::clone(&calls),
            delay: Duration::ZERO,
        };
        (item, calls)
    }

    /// An item whose `Title` is a large byte array, not a string.
    struct HostileTitle;

    #[zbus::interface(name = "org.kde.StatusNotifierItem")]
    impl HostileTitle {
        #[zbus(property)]
        fn id(&self) -> String {
            "hostile".to_owned()
        }

        #[zbus(property)]
        fn title(&self) -> Vec<u8> {
            vec![b'x'; 256 * 1024]
        }

        #[zbus(property)]
        fn icon_name(&self) -> String {
            "dialog-warning".to_owned()
        }
    }

    /// Registers the item served at [`ITEM_PATH`] on `connection`.
    fn register(connection: &Connection) {
        connection
            .call_method(
                Some(WATCHER_NAME),
                WATCHER_PATH,
                Some(WATCHER_INTERFACE),
                "RegisterStatusNotifierItem",
                &(ITEM_PATH,),
            )
            .expect("registered");
    }

    /// Serves `item` on a connection of its own and registers it with the
    /// watcher as `service` (a bare path: the sender serves it).
    fn serve_item(bus: &PrivateBus, item: FakeItem, well_known: Option<&str>) -> Connection {
        let connection = bus
            .builder()
            .serve_at(ITEM_PATH, item)
            .expect("served")
            .build()
            .expect("an item connection");
        let service = match well_known {
            Some(name) => {
                connection.request_name(name).expect("the item's name");
                name.to_owned()
            }
            None => ITEM_PATH.to_owned(),
        };
        connection
            .call_method(
                Some(WATCHER_NAME),
                WATCHER_PATH,
                Some(WATCHER_INTERFACE),
                "RegisterStatusNotifierItem",
                &(service.as_str(),),
            )
            .expect("registered");
        connection
    }

    fn start(
        bus: &PrivateBus,
        shutdown: &Arc<AtomicBool>,
    ) -> (TrayProvider, Worker, Receiver<TrayEvent>) {
        let (tx, rx) = channel();
        let publish: Publish = Box::new(move |event| tx.send(event).is_ok());
        let (provider, worker) = spawn_on(Ok(bus.builder()), publish, Box::new(NoMenu), shutdown)
            .expect("the tray starts");
        (provider, worker, rx)
    }

    /// The next `Items` event for which `accept` holds, within [`WAIT`].
    fn items_until(
        rx: &Receiver<TrayEvent>,
        accept: impl Fn(&[TrayItem]) -> bool,
    ) -> Vec<TrayItem> {
        let deadline = Instant::now() + WAIT;
        let mut last = None;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(TrayEvent::Items(items)) => {
                    if accept(&items) {
                        return items;
                    }
                    last = Some(items);
                }
                Ok(_) => {}
                Err(_) => panic!("no matching item list within {WAIT:?}; last {last:?}"),
            }
        }
    }

    fn registered(bus: &PrivateBus) -> Vec<String> {
        let client = bus.builder().build().expect("a client");
        let proxy =
            zbus::blocking::Proxy::new(&client, WATCHER_NAME, WATCHER_PATH, WATCHER_INTERFACE)
                .expect("a proxy");
        proxy
            .get_property::<Vec<String>>("RegisteredStatusNotifierItems")
            .expect("the property")
    }

    #[test]
    fn owner_taken_disables_the_host() {
        let Some(bus) = PrivateBus::start("taken") else {
            return;
        };
        let holder = bus.builder().build().expect("the holder connects");
        holder
            .request_name(WATCHER_NAME)
            .expect("the holder takes the name");
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker, rx) = start(&bus, &shutdown);

        let event = rx.recv_timeout(WAIT).expect("an event");
        let TrayEvent::Owner(Some(found)) = event else {
            panic!("expected the owner, got {event:?}");
        };
        assert_eq!(
            found.unique,
            holder.unique_name().expect("a unique name").as_str()
        );
        // The holder is this test process: `/proc` names it.
        assert!(
            found.comm.as_ref().is_some_and(|c| !c.is_empty()),
            "{found:?}"
        );
        // The host worker is done: no watcher is served and commands are refused.
        assert!(worker.join_within(WAIT), "the disabled host stops at once");
        assert!(provider
            .command(TrayCommand::Activate(ItemId::default()))
            .is_err());
        assert!(rx.try_recv().is_err(), "nothing else is published");
    }

    #[test]
    fn items_register_change_vanish_and_receive_commands() {
        let Some(bus) = PrivateBus::start("items") else {
            return;
        };
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker, rx) = start(&bus, &shutdown);
        assert_eq!(rx.recv_timeout(WAIT), Ok(TrayEvent::Owner(None)));
        assert_eq!(rx.recv_timeout(WAIT), Ok(TrayEvent::Items(Vec::new())));

        // An item with a theme icon, registered by its path.
        let (net, net_calls) = fake("Red", "network-wired", Vec::new(), "/Menu");
        let net_connection = serve_item(&bus, net, None);
        let net_unique = net_connection
            .unique_name()
            .expect("a unique name")
            .as_str()
            .to_owned();
        let items = items_until(&rx, |items| items.len() == 1);
        assert_eq!(items[0].id.bus_name, net_unique);
        assert_eq!(items[0].id.path, ITEM_PATH);
        assert_eq!(items[0].title, "Red");
        assert_eq!(items[0].status, Status::NeedsAttention);
        assert_eq!(items[0].icon_name, "network-wired");
        assert_eq!(items[0].pixmap, None);
        assert!(items[0].has_menu);

        // A pixmap-only item under a well-known name: the 4096 square is
        // dropped, the 16 square kept.
        let huge = (4096, 4096, vec![0u8; 16]);
        let small = (16, 16, vec![9u8; 16 * 16 * 4]);
        let (pix, _) = fake("Pix", "", vec![huge, small], "/");
        let pix_connection = serve_item(&bus, pix, Some("org.celestina.test.Pix"));
        let items = items_until(&rx, |items| items.len() == 2);
        let pix_item = items
            .iter()
            .find(|i| i.title == "Pix")
            .expect("the pixmap item");
        assert_eq!(pix_item.id.bus_name, "org.celestina.test.Pix");
        assert!(pix_item.icon_name.is_empty());
        assert!(
            matches!(&pix_item.pixmap, Some(Pixmap { w: 16, h: 16, argb }) if argb.len() == 1024)
        );
        assert!(!pix_item.has_menu);
        let mut names = registered(&bus);
        names.sort();
        let mut expected = vec![
            format!("{net_unique}{ITEM_PATH}"),
            format!("org.celestina.test.Pix{ITEM_PATH}"),
        ];
        expected.sort();
        assert_eq!(names, expected);

        // A change is signalled and re-read.
        {
            let iface = net_connection
                .object_server()
                .interface::<_, FakeItem>(ITEM_PATH)
                .expect("the served item");
            iface.get_mut().title = "Red 2".to_owned();
        }
        net_connection
            .emit_signal(
                None::<&str>,
                ITEM_PATH,
                "org.kde.StatusNotifierItem",
                "NewTitle",
                &(),
            )
            .expect("the signal");
        let items = items_until(&rx, |items| items.iter().any(|i| i.title == "Red 2"));
        assert_eq!(items.len(), 2);

        // Commands reach the item.
        let net_id = items
            .iter()
            .find(|i| i.title == "Red 2")
            .expect("the item")
            .id
            .clone();
        provider
            .command(TrayCommand::Activate(net_id.clone()))
            .expect("queued");
        provider
            .command(TrayCommand::Scroll(
                net_id.clone(),
                3,
                Orientation::Horizontal,
            ))
            .expect("queued");
        provider
            .command(TrayCommand::SecondaryActivate(net_id.clone()))
            .expect("queued");
        provider
            .command(TrayCommand::OpenMenu(net_id))
            .expect("queued: served by NoMenu");
        let deadline = Instant::now() + WAIT;
        while lock(&net_calls).len() < 3 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            *lock(&net_calls),
            [
                "Activate(0,0)",
                "Scroll(3,horizontal)",
                "SecondaryActivate(0,0)"
            ]
        );

        // The pixmap item's connection goes away: it leaves the list and the
        // watcher's books.
        drop(pix_connection);
        let items = items_until(&rx, |items| items.len() == 1);
        assert_eq!(items[0].title, "Red 2");
        assert_eq!(registered(&bus), [format!("{net_unique}{ITEM_PATH}")]);

        // Shutdown is deterministic: both workers stop.
        assert!(worker.join_within(WAIT));
        assert!(provider
            .command(TrayCommand::Activate(ItemId::default()))
            .is_err());
        drop(net_connection);
    }

    #[test]
    fn hostile_registrations_are_refused_or_dropped() {
        let Some(bus) = PrivateBus::start("hostile") else {
            return;
        };
        let shutdown = Arc::new(AtomicBool::new(false));
        let (_provider, worker, rx) = start(&bus, &shutdown);
        assert_eq!(rx.recv_timeout(WAIT), Ok(TrayEvent::Owner(None)));
        // The peer serves an object elsewhere, so zbus answers the paths it
        // registers with `UnknownObject` at once instead of silence.
        let (elsewhere, _) = fake("Elsewhere", "", Vec::new(), "/");
        let peer = bus
            .builder()
            .serve_at("/elsewhere", elsewhere)
            .expect("served")
            .build()
            .expect("a peer");
        let register = |service: &str| {
            peer.call_method(
                Some(WATCHER_NAME),
                WATCHER_PATH,
                Some(WATCHER_INTERFACE),
                "RegisterStatusNotifierItem",
                &(service,),
            )
        };
        assert!(
            register("bad name").is_err(),
            "a malformed service is refused"
        );
        assert!(register("").is_err());
        for n in 0..watcher::MAX_ITEMS_PER_SENDER {
            register(&format!("/nothing/served/here{n}")).expect("within the bound");
        }
        assert!(
            register("/one_too_many").is_err(),
            "the per-sender bound holds"
        );
        // A name nobody owns is registered (by another peer), then dropped
        // once the host looks.
        let other = bus.builder().build().expect("another peer");
        other
            .call_method(
                Some(WATCHER_NAME),
                WATCHER_PATH,
                Some(WATCHER_INTERFACE),
                "RegisterStatusNotifierItem",
                &("org.celestina.test.Nobody",),
            )
            .expect("accepted as a string");
        let deadline = Instant::now() + WAIT;
        loop {
            let names = registered(&bus);
            if !names
                .iter()
                .any(|n| n.starts_with("org.celestina.test.Nobody"))
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the unowned name is still registered: {names:?}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        // Paths nobody serves are kept registered but never listed.
        assert_eq!(registered(&bus).len(), watcher::MAX_ITEMS_PER_SENDER);
        assert!(rx
            .try_recv()
            .map_or(true, |e| e == TrayEvent::Items(Vec::new())));
        assert!(worker.join_within(WAIT));
    }

    #[test]
    fn a_title_that_is_a_large_byte_array_reads_as_an_empty_title() {
        let Some(bus) = PrivateBus::start("hostile-title") else {
            return;
        };
        let shutdown = Arc::new(AtomicBool::new(false));
        let (_provider, worker, rx) = start(&bus, &shutdown);
        assert_eq!(rx.recv_timeout(WAIT), Ok(TrayEvent::Owner(None)));
        let connection = bus
            .builder()
            .serve_at(ITEM_PATH, HostileTitle)
            .expect("served")
            .build()
            .expect("an item connection");
        register(&connection);
        let items = items_until(&rx, |items| items.len() == 1);
        assert_eq!(items[0].title, "");
        assert_eq!(items[0].icon_name, "dialog-warning");
        assert!(worker.join_within(WAIT));
    }

    #[test]
    fn a_slow_peer_delays_neither_commands_nor_the_shutdown() {
        let Some(bus) = PrivateBus::start("slow") else {
            return;
        };
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker, rx) = start(&bus, &shutdown);
        assert_eq!(rx.recv_timeout(WAIT), Ok(TrayEvent::Owner(None)));
        let (fast, fast_calls) = fake("Fast", "x", Vec::new(), "/");
        let fast_connection = serve_item(&bus, fast, None);
        let items = items_until(&rx, |items| items.len() == 1);
        let fast_id = items[0].id.clone();
        // A peer answering each property just inside the call timeout.
        let (mut slow, _) = fake("Slow", "y", Vec::new(), "/");
        slow.delay = Duration::from_millis(900);
        let _slow_connection = serve_item(&bus, slow, None);
        std::thread::sleep(Duration::from_millis(200));
        let queued = Instant::now();
        provider
            .command(TrayCommand::Activate(fast_id))
            .expect("queued");
        let deadline = queued + Duration::from_secs(3);
        while lock(&fast_calls).is_empty() {
            assert!(
                Instant::now() < deadline,
                "the command waited behind the slow peer's whole read"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            queued.elapsed() < Duration::from_secs(3),
            "took {:?}",
            queued.elapsed()
        );
        // The shell's exit waits two seconds for each worker: one item's
        // whole read, the call that overran it included, fits under that.
        let asked = Instant::now();
        assert!(
            worker.join_within(Duration::from_secs(2)),
            "the host did not stop within the slow peer's read"
        );
        assert!(asked.elapsed() < Duration::from_secs(2));
        assert!(item::ITEM_READ_BUDGET + CALL_TIMEOUT < Duration::from_secs(2));
        drop(fast_connection);
    }

    #[test]
    fn an_invalidated_entry_keeps_its_item_until_it_is_read_again() {
        let now = Instant::now();
        let mut entry = Entry::new(Registration {
            id: ItemId {
                bus_name: ":1.5".to_owned(),
                path: ITEM_PATH.to_owned(),
            },
            sender: ":1.5".to_owned(),
        });
        assert!(entry.needs_read(now), "never read");
        entry.item = Some(TrayItem::default());
        entry.read_at = Some(now);
        entry.stale = false;
        assert!(!entry.needs_read(now));
        // A signal right after the read: the item stays published while the
        // re-read waits for `MIN_REREAD`.
        let soon = now + Duration::from_millis(10);
        entry.invalidate(soon);
        assert!(entry.item.is_some(), "the last item is kept");
        assert!(!entry.needs_read(soon));
        assert!(entry.needs_read(now + MIN_REREAD));
        // A signal long after the read: owed at once, the item still kept.
        let later = now + 2 * MIN_REREAD;
        entry.invalidate(later);
        assert!(entry.item.is_some() && entry.needs_read(later));
    }

    #[test]
    fn an_item_that_signals_stays_listed_at_its_index() {
        let Some(bus) = PrivateBus::start("signals-stay") else {
            return;
        };
        let shutdown = Arc::new(AtomicBool::new(false));
        let (_provider, worker, rx) = start(&bus, &shutdown);
        assert_eq!(rx.recv_timeout(WAIT), Ok(TrayEvent::Owner(None)));
        let (first, _) = fake("First", "a", Vec::new(), "/");
        let first_connection = serve_item(&bus, first, None);
        let items = items_until(&rx, |items| items.len() == 1);
        let first_id = items[0].id.clone();
        let (second, _) = fake("Second", "b", Vec::new(), "/");
        let _second_connection = serve_item(&bus, second, None);
        let items = items_until(&rx, |items| items.len() == 2);
        assert_eq!(items[0].id, first_id);
        // The first item changes within `MIN_REREAD` of its read and says
        // so: it is re-read later, and until then the last item is listed
        // where it was, so a click meanwhile reaches it and not the second.
        {
            let iface = first_connection
                .object_server()
                .interface::<_, FakeItem>(ITEM_PATH)
                .expect("the served item");
            iface.get_mut().title = "First 2".to_owned();
        }
        first_connection
            .emit_signal(
                None::<&str>,
                ITEM_PATH,
                "org.kde.StatusNotifierItem",
                "NewTitle",
                &(),
            )
            .expect("the signal");
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(TrayEvent::Items(items)) => {
                    assert_eq!(items.len(), 2, "a signalling item left the list: {items:?}");
                    assert_eq!(items[0].id, first_id, "the rows moved: {items:?}");
                    if items[0].title == "First 2" {
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => panic!("the change was never published"),
            }
        }
        assert!(worker.join_within(WAIT));
    }

    #[test]
    fn signals_of_strangers_are_dropped_before_the_bound() {
        let mut pending = Pending::default();
        pending
            .known_items
            .insert((":1.5".to_owned(), ITEM_PATH.to_owned()));
        pending.known_names.insert(":1.5".to_owned());
        pending.known_names.insert("org.kde.foo".to_owned());
        let changed = |sender: &str, path: &str| BusSignal::ItemChanged {
            sender: sender.to_owned(),
            path: path.to_owned(),
        };
        // A stranger's item signals, from many paths, count for nothing.
        for n in 0..(MAX_PENDING_SIGNALS * 2) {
            assert!(!pending.record(changed(":1.9", &format!("/p{n}"))));
        }
        // A known sender signalling from a path it did not register: the
        // same.
        assert!(!pending.record(changed(":1.5", "/elsewhere")));
        assert!(pending.is_empty());
        // The known item's own signal is recorded once.
        assert!(pending.record(changed(":1.5", ITEM_PATH)));
        assert!(pending.record(changed(":1.5", ITEM_PATH)));
        assert_eq!(pending.signals.changed.len(), 1);
        // Name changes: only of known names; a stranger's churn is dropped.
        let owner = |name: &str, new_owner: &str| BusSignal::NameOwnerChanged {
            name: name.to_owned(),
            new_owner: new_owner.to_owned(),
        };
        for n in 0..(MAX_PENDING_SIGNALS * 2) {
            assert!(!pending.record(owner(&format!(":1.{}", 100 + n), "")));
        }
        assert!(pending.record(owner("org.kde.foo", "")));
        assert!(pending.record(owner("org.kde.foo", ":1.7")));
        assert_eq!(
            pending.signals.owner_changes,
            [
                ("org.kde.foo".to_owned(), String::new()),
                ("org.kde.foo".to_owned(), ":1.7".to_owned())
            ],
            "a name's history is kept in order"
        );
        // A known name changing hands over and over: past
        // `MAX_OWNER_CHANGES_PER_NAME` changes the name counts as churned,
        // its history is dropped (the host resolves its owner through the
        // bus instead) and the rest of the burst is dropped too.
        for n in 0..(MAX_PENDING_SIGNALS * 2) {
            let new_owner = if n % 2 == 0 { "" } else { ":1.7" };
            pending.record(owner("org.kde.foo", new_owner));
        }
        assert!(pending.signals.owner_changes.is_empty());
        assert_eq!(pending.signals.churned, ["org.kde.foo"]);
        // A menu signal from a stranger is dropped, and so is one a known
        // sender emits from a path that is not its item's menu, however
        // many paths it tries.
        let menu = |sender: &str, path: &str| BusSignal::MenuChanged {
            sender: sender.to_owned(),
            path: path.to_owned(),
        };
        pending
            .known_menus
            .insert((":1.5".to_owned(), "/Menu".to_owned()));
        assert!(!pending.record(menu(":1.9", "/Menu")));
        for n in 0..(MAX_PENDING_SIGNALS * 2) {
            assert!(!pending.record(menu(":1.5", &format!("/m{n}"))));
        }
        assert!(pending.record(menu(":1.5", "/Menu")));
        assert!(pending.record(menu(":1.5", "/Menu")));
        let signals = pending.take_signals();
        assert!(signals.owner_changes.is_empty());
        assert_eq!(signals.churned, ["org.kde.foo"]);
        assert_eq!(signals.menus.len(), 1);
        assert!(pending.is_empty(), "taken");
        assert!(
            pending.known_names.contains(":1.5"),
            "taking the signals keeps what to listen for"
        );
    }

    #[test]
    fn pokes_take_one_queue_slot_however_many_come() {
        let (inbox, queue) = mpsc::sync_channel::<Inbox>(COMMAND_QUEUE);
        let poker = Poker::new(inbox.clone());
        for _ in 0..1000 {
            poker.poke();
        }
        assert!(matches!(queue.try_recv(), Ok(Inbox::Poke)));
        assert!(queue.try_recv().is_err(), "one poke, not a thousand");
        // Until the host takes it, nothing more is queued; after, one more.
        poker.poke();
        assert!(queue.try_recv().is_err());
        poker.taken();
        poker.poke();
        assert!(matches!(queue.try_recv(), Ok(Inbox::Poke)));
        // A queue full of commands refuses the poke and forgets it, so the
        // next poke after the host drains the queue gets through.
        poker.taken();
        for _ in 0..COMMAND_QUEUE {
            inbox
                .try_send(Inbox::Command(TrayCommand::Activate(ItemId::default())))
                .expect("room");
        }
        poker.poke();
        while queue.try_recv().is_ok() {}
        poker.poke();
        assert!(matches!(queue.try_recv(), Ok(Inbox::Poke)));
    }
}
