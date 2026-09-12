//! The own protocol inside the daemon: one runtime thread that listens,
//! dials, pairs and runs sessions, publishing each phone into the same
//! registry the KDE Connect links publish into.
//!
//! The daemon stays thread-based; QUIC needs an async runtime, so this module
//! owns exactly one, on one thread, and everything the link does happens
//! there. What crosses to the rest of the daemon is what already crosses for
//! a KDE Connect link: a [`DeviceEntry`] in the registry, a command channel,
//! a [`SessionRegistration`] whose drop cleans up, and the revocation
//! barrier — `Forget` on `org.celestina.Devices1` forgets the pin in the
//! shared trust store, and this thread closes the session and acknowledges
//! the generation, exactly as the KDE Connect thread does.
//!
//! Pairing is armed from the app: [`PairingArm::arm`] draws a one-time
//! secret and returns the QR text; for two minutes an unpinned phone may
//! connect, and it is admitted only under the certificate it presents and
//! only until it proves the secret. Everything else unpinned is refused
//! before its first envelope.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use magnetita_link::endpoint::Expect;
use magnetita_link::trust::fingerprint_text;
use magnetita_link::{
    Backoff, DeviceCert, Endpoint, EndpointConfig, LinkError, Session, TrustStore, TrustedPeer,
};
use magnetita_proto::control::commands::{CommandResult, CommandRun};
use magnetita_proto::daily::battery::BatteryStatus;
use magnetita_proto::daily::clipboard::{ClipboardRequest, ClipboardText};
use magnetita_proto::daily::find::FindRing;
use magnetita_proto::daily::media::{MediaCommand, MediaRequest, MediaState};
use magnetita_proto::daily::notifications::Action;
use magnetita_proto::daily::notifications::{
    NotificationAction, NotificationDismissed, NotificationPosted, NotificationReply,
};
use magnetita_proto::mirror::{MirrorStarted, MirrorStop};
use magnetita_proto::pair::{kind as pair_kind, Fingerprint, QrPairing, QrPayload};
use magnetita_proto::phone::contacts::ContactsSync;
use magnetita_proto::phone::sms::{SmsConversations, SmsReceived, SmsThread};
use magnetita_proto::phone::telephony::{CallEvent, CallState};
use magnetita_proto::storage::StorageState;
use magnetita_proto::{capability, CapabilityVersion, DeviceKind, Hello};
use rand_core::{OsRng, RngCore};

use crate::devices::{command_channel, Command, DeviceEntry};
pub(crate) mod commands;
pub(crate) mod discovery;
pub(crate) mod input;
pub(crate) mod media;
pub(crate) mod mirror;
pub(crate) mod mirror_stream;
pub(crate) mod notifications;
pub(crate) mod phone;
pub(crate) mod share;
pub(crate) mod storage;

use crate::lock::LockOk;
use crate::runtime::log;
use crate::session_registration::SessionRegistration;
use crate::{ui_log, Daemon};
use discovery::Advertisement;

/// How long an armed pairing stays open.
const PAIRING_WINDOW: Duration = Duration::from_secs(120);
/// How often the dialer asks Avahi who is around.
const BROWSE_INTERVAL: Duration = Duration::from_secs(5);
/// How often a session checks the revocation barrier and its command queue.
const TICK: Duration = Duration::from_secs(1);
/// How long a newer session of the same phone waits for the older one to
/// leave: a tick to notice the order, and the cleanup after it.
const SUPERSEDE_WAIT: Duration = Duration::from_secs(5);

struct Armed {
    secret: [u8; 32],
    until: Instant,
}

/// The pairing window, shared between the served interface (which arms it)
/// and the link thread (which consumes it).
#[derive(Clone, Default)]
pub(crate) struct PairingArm(Arc<Mutex<Option<Armed>>>);

impl PairingArm {
    /// Draws a fresh secret and returns the text the QR shows.
    pub(crate) fn arm(
        &self,
        device_id: &str,
        fingerprint: Fingerprint,
        addresses: Vec<String>,
    ) -> String {
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        *self.0.lock_ok() = Some(Armed {
            secret,
            until: Instant::now() + PAIRING_WINDOW,
        });
        QrPayload {
            device_id: device_id.to_owned(),
            fingerprint,
            secret,
            addresses,
        }
        .to_uri()
    }

    /// The secret, if a window is open; taking it closes the window, so one
    /// QR admits one phone.
    fn take_live(&self) -> Option<[u8; 32]> {
        let mut g = self.0.lock_ok();
        match g.take() {
            Some(a) if a.until > Instant::now() => Some(a.secret),
            _ => None,
        }
    }

    #[cfg(test)]
    fn is_armed(&self) -> bool {
        self.0
            .lock_ok()
            .as_ref()
            .is_some_and(|a| a.until > Instant::now())
    }
}

/// The running wire: stop it and join it.
pub(crate) struct LinkWire {
    stopping: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl Drop for LinkWire {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// What this daemon offers on the own wire today.
fn hello(device_id: &str) -> Hello {
    Hello {
        device_id: device_id.to_owned(),
        device_name: "Celestina".into(),
        device_kind: DeviceKind::Desktop,
        capabilities: vec![
            CapabilityVersion {
                capability: capability::BATTERY,
                version: 1,
            },
            CapabilityVersion {
                capability: capability::FIND,
                version: 1,
            },
            CapabilityVersion {
                capability: capability::CLIPBOARD,
                version: 1,
            },
            CapabilityVersion {
                capability: capability::NOTIFICATIONS,
                version: 1,
            },
            CapabilityVersion {
                capability: capability::SHARE,
                version: 1,
            },
            CapabilityVersion {
                capability: capability::MEDIA,
                version: 1,
            },
            CapabilityVersion {
                capability: capability::CONTACTS,
                version: 1,
            },
            CapabilityVersion {
                capability: capability::SMS,
                version: 1,
            },
            CapabilityVersion {
                capability: capability::TELEPHONY,
                version: 1,
            },
            CapabilityVersion {
                capability: capability::COMMANDS,
                version: 1,
            },
            CapabilityVersion {
                capability: capability::INPUT,
                version: 1,
            },
            CapabilityVersion {
                capability: capability::MIRROR,
                version: 1,
            },
            CapabilityVersion {
                capability: capability::STORAGE,
                version: 1,
            },
        ],
    }
}

/// Where a phone's clipboard text lands: the Wayland adapter in the daemon,
/// a recorder in tests, so a test never writes the author's clipboard.
pub(crate) type ClipboardSink = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// Where the phone's notifications are shown; see `notifications`.
pub(crate) type NotificationServer = Arc<dyn notifications::NotificationServer>;

/// The desktop adapters a wire drives: injectable so the loopback tests
/// record instead of touching the session.
pub(crate) struct Adapters {
    pub(crate) clipboard_sink: ClipboardSink,
    pub(crate) notification_server: NotificationServer,
    pub(crate) notifications: Arc<notifications::Bridge>,
    /// Where received files land; partials of broken transfers stay here.
    pub(crate) download_dir: std::path::PathBuf,
    pub(crate) shares: Arc<share::ShareStore>,
    /// Where the phone's trackpad and keyboard events go.
    pub(crate) input: Arc<dyn input::InputSink>,
    /// Where the mirrored picture goes.
    pub(crate) mirror_player: Arc<dyn mirror::MirrorPlayer>,
}

/// The address a phone on this LAN would reach us at, for the QR. Best
/// effort: the mDNS advertisement carries the rest.
pub(crate) fn lan_address() -> Option<IpAddr> {
    let probe = UdpSocket::bind("0.0.0.0:0").ok()?;
    probe.connect("10.255.255.255:1").ok()?;
    let ip = probe.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

/// A copy of the pins for one gate check, so no std lock is held across an
/// await and a `Forget` in between is seen by the next check.
fn trust_snapshot(trust: &Mutex<TrustStore>) -> TrustStore {
    let mut snapshot = TrustStore::in_memory();
    for peer in trust.lock_ok().peers() {
        let _ = snapshot.pin(peer);
    }
    snapshot
}

/// The `StartPairing` implementation for the served interface: arms the
/// window with this daemon's identity and the address a phone would dial.
pub(crate) fn own_pairing(
    pairing: &PairingArm,
    device_id: &str,
    cert: &DeviceCert,
) -> Result<crate::devices::OwnPairing, LinkError> {
    let pairing = pairing.clone();
    let device_id = device_id.to_owned();
    let fingerprint = magnetita_link::fingerprint_of(&cert.chain()?[0]);
    Ok(Arc::new(move || {
        let addresses = lan_address()
            .map(|ip| vec![SocketAddr::new(ip, magnetita_link::PORT).to_string()])
            .unwrap_or_default();
        pairing.arm(&device_id, fingerprint, addresses)
    }))
}

/// The daemon's own wire on its port, advertised; `None` with a log line
/// when it cannot start, because the KDE Connect wire must not die with it.
pub(crate) fn install(
    daemon: Arc<Daemon>,
    cert: DeviceCert,
    device_id: String,
    pairing: PairingArm,
) -> Option<LinkWire> {
    let server: NotificationServer = match &daemon.dbus {
        Some(connection) => Arc::new(notifications::DbusServer(connection.clone())),
        None => Arc::new(notifications::NoServer),
    };
    let bridge = Arc::new(notifications::Bridge::default());
    notifications::spawn_signal_watch(Arc::clone(&bridge), Arc::clone(&daemon));
    match spawn(
        daemon,
        cert,
        device_id,
        pairing,
        SocketAddr::from(([0, 0, 0, 0], magnetita_link::PORT)),
        true,
        Adapters {
            clipboard_sink: Arc::new(crate::clipboard::write),
            notification_server: server,
            notifications: bridge,
            download_dir: crate::incoming_file::download_dir(),
            shares: Arc::new(share::ShareStore::default()),
            input: Arc::new(input::LazyUinput::default()),
            mirror_player: Arc::new(mirror::FifoPlayer),
        },
    ) {
        Ok((wire, addr)) => {
            log("link", &format!("own wire listening on {addr}"));
            Some(wire)
        }
        Err(e) => {
            log("link", &format!("own wire unavailable: {e}"));
            None
        }
    }
}

/// Starts the wire on its own thread. `bind` is `0.0.0.0:1760` in the
/// daemon and a loopback port in tests; `advertise` publishes on Avahi.
pub(crate) fn spawn(
    daemon: Arc<Daemon>,
    cert: DeviceCert,
    device_id: String,
    pairing: PairingArm,
    bind: SocketAddr,
    advertise: bool,
    adapters: Adapters,
) -> Result<(LinkWire, SocketAddr), LinkError> {
    let stopping = Arc::new(AtomicBool::new(false));
    let stop = Arc::clone(&stopping);
    let my_fingerprint = magnetita_link::fingerprint_of(&cert.chain()?[0]);
    // quinn binds only inside a runtime, so the thread binds and reports back.
    let (bound_tx, bound_rx) = std::sync::mpsc::channel::<Result<SocketAddr, LinkError>>();
    let join = thread::Builder::new()
        .name("magnetita-link".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = bound_tx.send(Err(LinkError::Io(e)));
                    return;
                }
            };
            let endpoint = match runtime.block_on(async {
                Endpoint::bind(
                    EndpointConfig {
                        cert,
                        hello: hello(&device_id),
                    },
                    bind,
                )
            }) {
                Ok(ep) => ep,
                Err(e) => {
                    let _ = bound_tx.send(Err(e));
                    return;
                }
            };
            let local = match endpoint.local_addr() {
                Ok(a) => a,
                Err(e) => {
                    let _ = bound_tx.send(Err(e));
                    return;
                }
            };
            let _ = bound_tx.send(Ok(local));
            let _advertisement = advertise
                .then(|| Advertisement::publish(&device_id, "Celestina", local.port()).ok())
                .flatten();
            let wire = Arc::new(Wire {
                daemon,
                endpoint,
                my_fingerprint,
                pairing,
                adapters,
                stop,
            });
            runtime.block_on(wire.run());
            wire.endpoint.close();
            runtime.block_on(wire.endpoint.wait_idle());
        })
        .map_err(LinkError::Io)?;
    let local = bound_rx.recv().unwrap_or_else(|_| {
        Err(LinkError::Connection(
            "the link thread ended before binding".into(),
        ))
    })?;
    Ok((
        LinkWire {
            stopping,
            join: Some(join),
        },
        local,
    ))
}

struct Wire {
    daemon: Arc<Daemon>,
    endpoint: Endpoint,
    my_fingerprint: Fingerprint,
    pairing: PairingArm,
    adapters: Adapters,
    stop: Arc<AtomicBool>,
}

impl Wire {
    async fn run(self: &Arc<Self>) {
        let accepter = Arc::clone(self);
        let dialer = Arc::clone(self);
        tokio::join!(accepter.accept_loop(), dialer.dial_loop());
    }

    async fn accept_loop(self: Arc<Self>) {
        loop {
            let next = tokio::select! {
                incoming = self.endpoint.accept() => incoming,
                _ = self.stopped() => return,
            };
            let Some(incoming) = next else { return };
            let incoming = match incoming {
                Ok(i) => i,
                Err(e) => {
                    log("link", &format!("accept: {e}"));
                    continue;
                }
            };
            let wire = Arc::clone(&self);
            tokio::spawn(async move { wire.admit(incoming).await });
        }
    }

    async fn admit(&self, incoming: magnetita_link::Incoming) {
        let fp = incoming.peer_fingerprint();
        let address = incoming.remote_address();
        let snapshot = trust_snapshot(&self.daemon.trust);
        let pinned = magnetita_link::Trust(&snapshot)
            .peer_by_fingerprint(&fp)
            .is_some();
        if pinned {
            match self
                .endpoint
                .admit(incoming, Expect::Trusted(&snapshot))
                .await
            {
                Ok((session, hello)) => self.run_session(session, hello, "accepted").await,
                Err(e) => log("link", &format!("{address}: {e}")),
            }
            return;
        }
        let Some(secret) = self.pairing.take_live() else {
            log(
                "link",
                &format!("{address}: unpinned and no pairing armed; refused"),
            );
            incoming.refuse();
            return;
        };
        match self.endpoint.admit(incoming, Expect::Fingerprint(fp)).await {
            Ok((session, hello)) => self.pair_then_run(session, hello, secret, fp).await,
            Err(e) => log("link", &format!("{address}: pairing admit: {e}")),
        }
    }

    /// The desktop half of the QR path over a fresh session; on success the
    /// phone is pinned and the session continues as a trusted one.
    async fn pair_then_run(
        &self,
        session: Session,
        hello: Hello,
        secret: [u8; 32],
        fp: Fingerprint,
    ) {
        let mut pairing = QrPairing::desktop(secret, self.my_fingerprint, fp);
        let proof =
            match tokio::time::timeout(magnetita_link::HANDSHAKE_BUDGET, session.recv()).await {
                Ok(Ok(env))
                    if env.capability == capability::PAIRING && env.kind == pair_kind::QR_PROOF =>
                {
                    env
                }
                other => {
                    log(
                        "link",
                        &format!("{}: pairing: no proof ({other:?})", hello.device_name),
                    );
                    session.close("no proof");
                    return;
                }
            };
        let (reply, pinned) = match pairing.accept_proof(&proof.body) {
            Ok(r) => r,
            Err(e) => {
                log(
                    "link",
                    &format!("{}: pairing refused: {e}", hello.device_name),
                );
                ui_log(
                    &self.daemon,
                    &hello.device_name,
                    "emparejamiento rechazado",
                    true,
                );
                session.close("wrong proof");
                return;
            }
        };
        let peer = TrustedPeer {
            device_id: hello.device_id.clone(),
            device_name: hello.device_name.clone(),
            fingerprint: fingerprint_text(&pinned.peer_fingerprint),
        };
        if let Err(e) = self.daemon.trust.lock_ok().pin(peer) {
            log(
                "link",
                &format!("{}: cannot persist the pin: {e}", hello.device_name),
            );
            session.close("cannot pin");
            return;
        }
        if let Err(e) = session
            .send_message(capability::PAIRING, pair_kind::QR_REPLY, reply)
            .await
        {
            log(
                "link",
                &format!("{}: pairing reply: {e}", hello.device_name),
            );
            return;
        }
        ui_log(&self.daemon, &hello.device_name, "emparejado", false);
        self.run_session(session, hello, "paired").await;
    }

    async fn dial_loop(self: Arc<Self>) {
        let mut backoff = Backoff::new();
        loop {
            tokio::select! {
                _ = tokio::time::sleep(BROWSE_INTERVAL) => {}
                _ = self.stopped() => return,
            }
            let stop = Arc::clone(&self.stop);
            let peers = match tokio::task::spawn_blocking(move || discovery::browse(&stop)).await {
                Ok(p) => p,
                Err(_) => continue,
            };
            let mut dialled = false;
            for peer in peers {
                let known = self.daemon.trust.lock_ok().is_trusted(&peer.device_id);
                let connected = self.daemon.devices.lock_ok().contains_key(&peer.device_id);
                if !known || connected {
                    continue;
                }
                dialled = true;
                let snapshot = trust_snapshot(&self.daemon.trust);
                match self
                    .endpoint
                    .connect(peer.address, Expect::Trusted(&snapshot))
                    .await
                {
                    Ok((session, hello)) => {
                        backoff.reset();
                        let wire = Arc::clone(&self);
                        tokio::spawn(
                            async move { wire.run_session(session, hello, "dialled").await },
                        );
                    }
                    Err(e) => {
                        log(
                            "link",
                            &format!("{} at {}: {e}", peer.device_id, peer.address),
                        );
                        let delay = backoff.next_delay();
                        tokio::time::sleep(delay).await;
                    }
                }
            }
            if !dialled {
                backoff.reset();
            }
        }
    }

    async fn stopped(&self) {
        while !self.stop.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    /// One pinned session, from publication to cleanup.
    async fn run_session(&self, session: Session, hello: Hello, how: &str) {
        // Shared with the reader tasks: `select!` drops a losing branch's
        // future, and a control-stream read dropped mid-frame desynchronises
        // the stream, so the reads live in tasks of their own and the loop
        // receives from channels, which is cancel-safe.
        let session = Arc::new(session);
        let daemon = &self.daemon;
        let device_id = hello.device_id.clone();
        let name = hello.device_name.clone();
        let device_type = match hello.device_kind {
            DeviceKind::Phone => "phone",
            DeviceKind::Desktop => "desktop",
        };
        let (sender, commands) = command_channel();
        // The same phone again, its application restarted or reinstalled,
        // while its old session waits out the idle timeout: the newer one
        // is the live one. The old session is told to leave and this one
        // waits for its slot; a phone that is truly connected twice loses
        // the older connection, which the phone itself has dropped.
        let superseded = daemon
            .commands
            .lock_ok()
            .get(&device_id)
            .map(|old| old.try_send(Command::Superseded).is_ok());
        if superseded.is_some() {
            log(
                "link",
                &format!("{name}: connected again; the earlier session yields"),
            );
            let deadline = Instant::now() + SUPERSEDE_WAIT;
            while daemon.devices.lock_ok().contains_key(&device_id) {
                if Instant::now() >= deadline {
                    log(
                        "link",
                        &format!("{name}: the earlier session did not leave; dropping this one"),
                    );
                    session.close("duplicate");
                    return;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
        {
            let mut devices = daemon.devices.lock_ok();
            if devices.contains_key(&device_id) {
                log(
                    "link",
                    &format!("{name}: already connected; dropping the second session"),
                );
                session.close("duplicate");
                return;
            }
            daemon.commands.lock_ok().insert(device_id.clone(), sender);
            let mut entry = DeviceEntry::connected(
                device_id.clone(),
                name.clone(),
                device_type.into(),
                fingerprint_text(&session.peer_fingerprint()),
            );
            entry.paired = true;
            devices.insert(device_id.clone(), entry);
        }
        let _registration =
            SessionRegistration::new(Arc::clone(daemon), device_id.clone(), name.clone());
        daemon.notify_change();
        log(
            how,
            &format!("{name} at {} on the own wire", session.remote_address()),
        );
        ui_log(daemon, &name, "conectado y cifrado", false);
        if daemon.settings.lock_ok().clipboard {
            // The phone answers only while its application is in front.
            if let Err(e) = session
                .send_message(
                    capability::CLIPBOARD,
                    ClipboardRequest::KIND,
                    ClipboardRequest.encode(),
                )
                .await
            {
                log("link", &format!("{name}: clipboard request: {e}"));
            }
        }

        let (outbox, mut outbox_rx) =
            tokio::sync::mpsc::unbounded_channel::<magnetita_proto::Envelope>();
        let shares_outbox = outbox.clone();
        let shares = share::SessionShare::new(
            Arc::clone(daemon),
            &device_id,
            &name,
            session.transfers(),
            Arc::clone(&self.adapters.shares),
            self.adapters.download_dir.clone(),
            outbox,
        );
        let storage_client = storage::StorageClient::new(shares_outbox.clone());
        storage::clients()
            .lock_ok()
            .insert(device_id.clone(), Arc::clone(&storage_client));
        // The phone's files, mounted while it shares a root; dropping the
        // session unmounts, so a lost link never strands the directory.
        let mut phone_mount: Option<fuser::BackgroundSession> = None;
        let governor = input::Governor::default();
        let media = Arc::new(media::SessionMedia::default());
        if daemon.settings.lock_ok().media {
            let req = media::SessionMedia::request();
            if let Err(e) = session
                .send_message(req.capability, req.kind, req.body)
                .await
            {
                log("link", &format!("{name}: media request: {e}"));
            }
        }
        {
            let settings = *daemon.settings.lock_ok();
            let mut greetings = Vec::new();
            if settings.contacts {
                let since = phone::store().with(&device_id, |book| book.contacts_version);
                greetings.push(phone::contacts_request(since));
            }
            if settings.sms {
                greetings.push(phone::conversations_request());
            }
            if settings.commands {
                greetings.push(commands::store().published());
            }
            for env in greetings {
                if let Err(e) = session
                    .send_message(env.capability, env.kind, env.body)
                    .await
                {
                    log("link", &format!("{name}: phone request: {e}"));
                }
            }
        }
        // Bulk streams are accepted by their own task: `select!` drops the
        // future of a branch that loses the race, and a dropped half-accepted
        // stream is a lost transfer. A channel receive is cancel-safe.
        let (streams_tx, mut streams_rx) = tokio::sync::mpsc::unbounded_channel();
        let acceptor = {
            let transfers = session.transfers();
            let name = name.clone();
            tokio::spawn(async move {
                loop {
                    match transfers.accept().await {
                        Ok(stream) => {
                            if streams_tx.send(stream).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            log("link", &format!("{name}: bulk stream: {e}"));
                            break;
                        }
                    }
                }
            })
        };
        let (control_tx, mut control_rx) = tokio::sync::mpsc::channel(64);
        let control_reader = {
            let session = Arc::clone(&session);
            tokio::spawn(async move {
                loop {
                    let next = session.recv().await;
                    let failed = next.is_err();
                    if control_tx.send(next).await.is_err() || failed {
                        break;
                    }
                }
            })
        };
        let (datagram_tx, mut datagram_rx) = tokio::sync::mpsc::channel(256);
        let datagram_reader = {
            let session = Arc::clone(&session);
            tokio::spawn(async move {
                loop {
                    let next = session.recv_datagram().await;
                    let failed = next.is_err();
                    if datagram_tx.send(next).await.is_err() || failed {
                        break;
                    }
                }
            })
        };
        let mut tick = tokio::time::interval(TICK);
        loop {
            tokio::select! {
                Some(envelope) = control_rx.recv() => match envelope {
                    Ok(env) if env.capability == capability::PAIRING && env.kind == pair_kind::QR_PROOF => {
                        self.prove_again(&session, &name, &env.body).await;
                    }
                    Ok(env) if env.capability == capability::MEDIA => {
                        self.handle_media(&device_id, &name, &media, &env);
                    }
                    Ok(env) if env.capability == capability::MIRROR && mirror::own().owned_by(&device_id) => match env.kind {
                        MirrorStarted::KIND => match MirrorStarted::decode(&env.body) {
                            Ok(started) => {
                                mirror::own().started(self.adapters.mirror_player.as_ref(), &started);
                                log("mirror", &format!("{name}: streaming {}x{}", started.width, started.height));
                            }
                            Err(e) => log("link", &format!("{name}: mirror: {e}")),
                        },
                        MirrorStop::KIND => mirror::own().stopped(),
                        _ => {}
                    },

                    Ok(env) if matches!(env.capability, capability::CONTACTS | capability::SMS | capability::TELEPHONY) => {
                        self.handle_phone(&device_id, &name, &env);
                    }
                    Ok(env) if env.capability == capability::STORAGE && env.kind == StorageState::KIND => {
                        let available = StorageState::decode(&env.body).map(|s| s.available).unwrap_or(false);
                        if available && phone_mount.is_none() {
                            match crate::mount::mountpoint_for(&device_id) {
                                Ok(mountpoint) => {
                                    let client = Arc::clone(&storage_client);
                                    let handle = tokio::runtime::Handle::current();
                                    let point = mountpoint.clone();
                                    match tokio::task::spawn_blocking(move || storage::mount(client, handle, &point)).await {
                                        Ok(Ok(mounted)) => {
                                            phone_mount = Some(mounted);
                                            daemon.set_mount(&device_id, Some(&mountpoint));
                                            daemon.notify_change();
                                            log("storage", &format!("{name}: mounted at {}", mountpoint.display()));
                                        }
                                        Ok(Err(e)) => log("storage", &format!("{name}: mount: {e}")),
                                        Err(e) => log("storage", &format!("{name}: mount: {e}")),
                                    }
                                }
                                Err(e) => log("storage", &format!("{name}: {e}")),
                            }
                        } else if !available {
                            if let Some(mounted) = phone_mount.take() {
                                let _ = tokio::task::spawn_blocking(move || drop(mounted)).await;
                                daemon.set_mount(&device_id, None);
                                daemon.notify_change();
                            }
                        }
                    }
                    Ok(env) if env.capability == capability::STORAGE => storage_client.reply(env),
                    Ok(env) if env.capability == capability::INPUT => {
                        self.handle_input(&name, &governor, env.kind, &env.body);
                    }
                    Ok(env) if env.capability == capability::COMMANDS && env.kind == CommandRun::KIND => {
                        if self.daemon.settings.lock_ok().commands {
                            if let Ok(run) = CommandRun::decode(&env.body) {
                                let outbox = shares_outbox.clone();
                                let stop = Arc::clone(&self.stop);
                                let who = name.clone();
                                tokio::task::spawn_blocking(move || {
                                    let result = commands::store().run(run.id, &stop);
                                    log("link", &format!("{who}: command {} {}", run.id, if result.ok { "ok" } else { "failed" }));
                                    let _ = outbox.send(magnetita_proto::Envelope {
                                        capability: capability::COMMANDS,
                                        kind: CommandResult::KIND,
                                        id: 0,
                                        body: result.encode(),
                                    });
                                });
                            }
                        }
                    }
                    Ok(env) if env.capability == capability::SHARE => {
                        if let Some(reply) = shares.handle(&env) {
                            if let Err(e) = session.send_message(reply.capability, reply.kind, reply.body).await {
                                log("link", &format!("{name}: share: {e}"));
                            }
                        }
                    }
                    Ok(env) => self.handle(&device_id, &name, env),
                    Err(e) => {
                        log("link", &format!("{name}: {e}"));
                        break;
                    }
                },
                Some((transfer, stream)) = streams_rx.recv() => match transfer {
                    mirror::VIDEO_STREAM if mirror::own().owned_by(&device_id) => mirror::own().video_stream(stream),
                    mirror::VIDEO_STREAM | mirror::AUDIO_STREAM => {}
                    _ => shares.stream_arrived(transfer, stream),
                },
                // Motion may arrive as datagrams: same body, no reliability.
                Some(datagram) = datagram_rx.recv() => match datagram {
                    Ok(env) if env.capability == capability::INPUT => {
                        self.handle_input(&name, &governor, env.kind, &env.body);
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log("link", &format!("{name}: {e}"));
                        break;
                    }
                },
                Some(reply) = outbox_rx.recv() => {
                    if let Err(e) = session.send_message(reply.capability, reply.kind, reply.body).await {
                        log("link", &format!("{name}: share: {e}"));
                    }
                }
                _ = tick.tick() => {
                    for env in mirror::own().tick(&device_id, &shares_outbox) {
                        if let Err(e) = session.send_message(env.capability, env.kind, env.body).await {
                            log("link", &format!("{name}: mirror: {e}"));
                        }
                    }
                    media.set_active(daemon.settings.lock_ok().media);
                    for state in media.tick() {
                        if let Err(e) = session.send_message(state.capability, state.kind, state.body).await {
                            log("link", &format!("{name}: media: {e}"));
                        }
                    }
                    // The desktop's clipboard changes land in the shared slot
                    // for every device; this wire drains its own entry here.
                    if let Some(text) = daemon.pending_clipboards.take(&device_id) {
                        if let Err(e) = session
                            .send_message(
                                capability::CLIPBOARD,
                                ClipboardText::KIND,
                                ClipboardText { text }.encode(),
                            )
                            .await
                        {
                            log("link", &format!("{name}: clipboard: {e}"));
                        }
                    }
                    if let Some(generation) = daemon.revocations.current(&device_id) {
                        session.close("forgotten");
                        daemon.revocations.acknowledge(&device_id, generation);
                        log("link", &format!("{name}: forgotten; session closed"));
                        break;
                    }
                    let mut superseded = false;
                    while let Ok(command) = commands.try_recv() {
                        if matches!(command, Command::Superseded) {
                            superseded = true;
                            continue;
                        }
                        if let Err(e) = self.command(&session, &shares, &media, &device_id, command).await {
                            log("link", &format!("{name}: command: {e}"));
                        }
                    }
                    if superseded {
                        session.close("superseded");
                        log("link", &format!("{name}: superseded by a newer session"));
                        break;
                    }
                    if self.stop.load(Ordering::Relaxed) {
                        session.close("daemon stopping");
                        break;
                    }
                }
            }
        }
        acceptor.abort();
        control_reader.abort();
        datagram_reader.abort();
        if mirror::own().owned_by(&device_id) {
            mirror::own().stopped();
        }
        phone::store().forget_device(&device_id);
        daemon.pending_clipboards.clear(&device_id);
        self.adapters.notifications.forget_device(&device_id);
        storage::clients().lock_ok().remove(&device_id);
        if let Some(mounted) = phone_mount.take() {
            let _ = tokio::task::spawn_blocking(move || drop(mounted)).await;
            daemon.set_mount(&device_id, None);
            daemon.notify_change();
        }
    }

    /// Media envelopes from the phone: its player onto the registry's card,
    /// its buttons onto the desktop's players, its request onto the tick.
    fn handle_media(
        &self,
        device_id: &str,
        name: &str,
        media: &media::SessionMedia,
        env: &magnetita_proto::Envelope,
    ) {
        if !self.daemon.settings.lock_ok().media {
            return;
        }
        match env.kind {
            MediaState::KIND => match MediaState::decode(&env.body) {
                Ok(state) => {
                    media.note_phone_state(&state);
                    let mapped = media::player_state(&state);
                    let stale =
                        crate::devices::set_media(&self.daemon.devices, device_id, mapped.as_ref());
                    if let Some(path) = stale {
                        crate::artwork::discard(&path);
                    }
                    self.daemon.notify_change();
                }
                Err(e) => log("link", &format!("{name}: media: {e}")),
            },
            MediaCommand::KIND => match MediaCommand::decode(&env.body) {
                Ok(command) => media.drive(&command),
                Err(e) => log("link", &format!("{name}: media: {e}")),
            },
            MediaRequest::KIND => media.wanted(),
            _ => {}
        }
    }

    /// Trackpad and keyboard events: gated by the setting, bounded by the
    /// governor, decoded at this boundary, and handed to the sink.
    fn handle_input(&self, name: &str, governor: &input::Governor, kind: u16, body: &[u8]) {
        if !self.daemon.settings.lock_ok().input || !governor.admit() {
            return;
        }
        if let Err(e) = input::apply(self.adapters.input.as_ref(), kind, body) {
            log("link", &format!("{name}: input: {e}"));
        }
    }

    /// Contacts, SMS and calls from the phone: into the store, onto the
    /// registry, and the ones a person must see onto the notification server.
    fn handle_phone(&self, device_id: &str, name: &str, env: &magnetita_proto::Envelope) {
        let settings = *self.daemon.settings.lock_ok();
        let store = phone::store();
        match (env.capability, env.kind) {
            (capability::CONTACTS, ContactsSync::KIND) if settings.contacts => {
                match ContactsSync::decode(&env.body) {
                    Ok(page) => {
                        let (people, version) = store.with(device_id, |book| {
                            book.sync(&page);
                            (book.people.len(), book.contacts_version)
                        });
                        if page.complete {
                            log(
                                "link",
                                &format!("{name}: {people} contacts at version {version}"),
                            );
                        }
                    }
                    Err(e) => log("link", &format!("{name}: contacts: {e}")),
                }
            }
            (capability::SMS, SmsConversations::KIND) if settings.sms => {
                match SmsConversations::decode(&env.body) {
                    Ok(list) => {
                        store.with(device_id, |book| book.conversations = list.conversations);
                        self.daemon.notify_change();
                    }
                    Err(e) => log("link", &format!("{name}: sms: {e}")),
                }
            }
            (capability::SMS, SmsThread::KIND) if settings.sms => {
                match SmsThread::decode(&env.body) {
                    Ok(page) => {
                        store.with(device_id, |book| phone::merge_thread(book, &page));
                        self.daemon.notify_change();
                    }
                    Err(e) => log("link", &format!("{name}: sms: {e}")),
                }
            }
            (capability::SMS, SmsReceived::KIND) if settings.sms => {
                match SmsReceived::decode(&env.body) {
                    Ok(received) => {
                        let label = store.with(device_id, |book| {
                            book.received(received.thread, &received.message);
                            book.label(std::slice::from_ref(&received.message.address))
                        });
                        self.daemon.notify_change();
                        if !received.message.from_me {
                            let note = NotificationPosted {
                                key: format!("{}{}", phone::SMS_KEY_PREFIX, received.thread),
                                app_name: "SMS".into(),
                                title: label,
                                body: received.message.body.clone(),
                                timestamp_ms: received.message.timestamp_ms,
                                replyable: true,
                                actions: Vec::new(),
                                icon: None,
                                media: false,
                            };
                            if let Some(line) = self.adapters.notifications.posted(
                                self.adapters.notification_server.as_ref(),
                                device_id,
                                name,
                                &note,
                            ) {
                                ui_log(&self.daemon, name, &line, false);
                            }
                        }
                    }
                    Err(e) => log("link", &format!("{name}: sms: {e}")),
                }
            }
            (capability::TELEPHONY, CallEvent::KIND) if settings.telephony => {
                match CallEvent::decode(&env.body) {
                    Ok(event) => self.call_changed(device_id, name, event),
                    Err(e) => log("link", &format!("{name}: call: {e}")),
                }
            }
            _ => {}
        }
    }

    fn call_changed(&self, device_id: &str, name: &str, event: CallEvent) {
        let store = phone::store();
        let resolved = store.with(device_id, |book| {
            let resolved = book.resolve(&event.number);
            book.call = Some(event.clone());
            resolved
        });
        let (what, who) = phone::call_line(&event, resolved.as_deref());
        {
            let mut devices = self.daemon.devices.lock_ok();
            if let Some(entry) = devices.get_mut(device_id) {
                entry.call_state = if event.state == CallState::Ended {
                    String::new()
                } else {
                    phone::call_state_word(event.state).to_owned()
                };
                entry.call_number = event.number.clone();
                entry.call_name = who.clone();
            }
        }
        self.daemon.notify_change();
        let server = self.adapters.notification_server.as_ref();
        match event.state {
            CallState::Ended => {
                self.adapters
                    .notifications
                    .dismissed(server, device_id, phone::CALL_KEY);
            }
            state => {
                let note = NotificationPosted {
                    key: phone::CALL_KEY.into(),
                    app_name: "Tel\u{e9}fono".into(),
                    title: format!("{what}: {who}"),
                    body: if who == event.number {
                        String::new()
                    } else {
                        event.number.clone()
                    },
                    timestamp_ms: event.timestamp_ms,
                    replyable: false,
                    actions: phone::call_buttons(state)
                        .into_iter()
                        .map(|(label, _)| Action {
                            label: label.into(),
                        })
                        .collect(),
                    icon: None,
                    media: false,
                };
                if let Some(line) = self
                    .adapters
                    .notifications
                    .posted(server, device_id, name, &note)
                {
                    ui_log(&self.daemon, name, &line, false);
                }
            }
        }
    }

    /// A phone that forgot this desktop while the desktop still pins it is
    /// admitted as trusted, yet it sends a QR proof: answer it from the armed
    /// window so the phone can pin again, and keep the session.
    async fn prove_again(&self, session: &Session, name: &str, proof: &[u8]) {
        let Some(secret) = self.pairing.take_live() else {
            log(
                "link",
                &format!("{name}: proof without an armed pairing; ignored"),
            );
            return;
        };
        let mut pairing =
            QrPairing::desktop(secret, self.my_fingerprint, session.peer_fingerprint());
        let reply = match pairing.accept_proof(proof) {
            Ok((reply, _)) => reply,
            Err(e) => {
                log("link", &format!("{name}: pairing again refused: {e}"));
                return;
            }
        };
        match session
            .send_message(capability::PAIRING, pair_kind::QR_REPLY, reply)
            .await
        {
            Ok(_) => ui_log(&self.daemon, name, "emparejado de nuevo", false),
            Err(e) => log("link", &format!("{name}: pairing reply: {e}")),
        }
    }

    fn handle(&self, device_id: &str, name: &str, env: magnetita_proto::Envelope) {
        match (env.capability, env.kind) {
            (capability::BATTERY, BatteryStatus::KIND) => match BatteryStatus::decode(&env.body) {
                Ok(status) => {
                    self.daemon
                        .set_battery(device_id, i32::from(status.level), status.charging);
                    self.daemon.notify_change();
                }
                Err(e) => log("link", &format!("{name}: battery: {e}")),
            },
            (capability::CLIPBOARD, ClipboardText::KIND) => {
                if !self.daemon.settings.lock_ok().clipboard {
                    return;
                }
                match ClipboardText::decode(&env.body) {
                    Ok(clip) if magnetita_core::clipboard::is_syncable(&clip.text) => {
                        // Record before writing so the watcher does not echo it back.
                        *self.daemon.last_clipboard.lock_ok() = clip.text.clone();
                        if (self.adapters.clipboard_sink)(&clip.text) {
                            ui_log(&self.daemon, name, "portapapeles recibido", false);
                        }
                    }
                    Ok(_) => log("link", &format!("{name}: clipboard: not syncable")),
                    Err(e) => log("link", &format!("{name}: clipboard: {e}")),
                }
            }
            (capability::NOTIFICATIONS, NotificationPosted::KIND) => {
                if !self.daemon.settings.lock_ok().notifications {
                    return;
                }
                match NotificationPosted::decode(&env.body) {
                    Ok(note)
                        if note.media && !self.daemon.settings.lock_ok().media_notifications => {}
                    Ok(note) => {
                        if let Some(line) = self.adapters.notifications.posted(
                            self.adapters.notification_server.as_ref(),
                            device_id,
                            name,
                            &note,
                        ) {
                            ui_log(&self.daemon, name, &line, false);
                        }
                    }
                    Err(e) => log("link", &format!("{name}: notification: {e}")),
                }
            }
            (capability::NOTIFICATIONS, NotificationDismissed::KIND) => {
                match NotificationDismissed::decode(&env.body) {
                    Ok(gone) => self.adapters.notifications.dismissed(
                        self.adapters.notification_server.as_ref(),
                        device_id,
                        &gone.key,
                    ),
                    Err(e) => log("link", &format!("{name}: notification: {e}")),
                }
            }
            (cap, kind) => log(
                "link",
                &format!("{name}: unhandled capability {cap} kind {kind}"),
            ),
        }
    }

    async fn command(
        &self,
        session: &Session,
        shares: &Arc<share::SessionShare>,
        media: &Arc<media::SessionMedia>,
        device_id: &str,
        command: Command,
    ) -> Result<(), LinkError> {
        match command {
            Command::Superseded => {}
            Command::CommandsChanged => {
                let env = commands::store().published();
                session
                    .send_message(env.capability, env.kind, env.body)
                    .await?;
            }
            Command::SmsList => {
                let env = phone::conversations_request();
                session
                    .send_message(env.capability, env.kind, env.body)
                    .await?;
            }
            Command::SmsThread { thread, before_ms } => {
                let env = phone::thread_request(thread, before_ms);
                session
                    .send_message(env.capability, env.kind, env.body)
                    .await?;
            }
            Command::SmsSend { thread, body } => {
                let env = phone::sms_send(thread, &body);
                session
                    .send_message(env.capability, env.kind, env.body)
                    .await?;
            }
            Command::CallAction(action) => {
                let env = phone::call_command(action);
                session
                    .send_message(env.capability, env.kind, env.body)
                    .await?;
            }
            // A reply on an SMS notification is a send in that thread; a
            // button on the call notification is a call action.
            Command::NotificationReply { key, text } if key.starts_with(phone::SMS_KEY_PREFIX) => {
                if let Ok(thread) = key[phone::SMS_KEY_PREFIX.len()..].parse::<u64>() {
                    let env = phone::sms_send(thread, &text);
                    session
                        .send_message(env.capability, env.kind, env.body)
                        .await?;
                }
            }
            Command::NotificationAction { key, action } if key == phone::CALL_KEY => {
                let state =
                    phone::store().with(device_id, |book| book.call.as_ref().map(|c| c.state));
                if let Some(state) = state {
                    if let Some((_, call_action)) =
                        phone::call_buttons(state).get(usize::from(action))
                    {
                        let env = phone::call_command(*call_action);
                        session
                            .send_message(env.capability, env.kind, env.body)
                            .await?;
                    }
                }
            }
            Command::NotificationDismiss { key }
                if key == phone::CALL_KEY || key.starts_with(phone::SMS_KEY_PREFIX) => {}
            Command::Media(action) => {
                if let Some(env) = media.command_for_phone(action) {
                    session
                        .send_message(env.capability, env.kind, env.body)
                        .await?;
                }
            }
            Command::SendFile(path) => match shares.offer(path) {
                Ok(offer) => {
                    session
                        .send_message(offer.capability, offer.kind, offer.body)
                        .await?;
                }
                Err(e) => log("link", &format!("share: {e}")),
            },
            Command::Ring => {
                session
                    .send_message(capability::FIND, FindRing::KIND, FindRing.encode())
                    .await?;
            }
            Command::NotificationAction { key, action } => {
                session
                    .send_message(
                        capability::NOTIFICATIONS,
                        NotificationAction::KIND,
                        NotificationAction { key, action }.encode(),
                    )
                    .await?;
            }
            Command::NotificationReply { key, text } => {
                session
                    .send_message(
                        capability::NOTIFICATIONS,
                        NotificationReply::KIND,
                        NotificationReply { key, text }.encode(),
                    )
                    .await?;
            }
            Command::NotificationDismiss { key } => {
                session
                    .send_message(
                        capability::NOTIFICATIONS,
                        NotificationDismissed::KIND,
                        NotificationDismissed { key }.encode(),
                    )
                    .await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{Commands, Log, Registry};
    use crate::revocation::Revocations;
    use crate::settings::Settings;
    use magnetita_link::endpoint::Expect as PeerExpect;
    use std::collections::{BTreeMap, HashMap, VecDeque};

    fn test_daemon(_cert: &DeviceCert) -> Arc<Daemon> {
        Arc::new(Daemon {
            trust: Arc::new(Mutex::new(TrustStore::in_memory())),
            settings: Arc::new(Mutex::new(Settings::default())),
            devices: Registry::new(Mutex::new(BTreeMap::new())),
            log: Log::new(Mutex::new(VecDeque::new())),
            commands: Commands::new(Mutex::new(HashMap::new())),
            pending_clipboards: Default::default(),
            revocations: Arc::new(Revocations::new()),
            payloads: magnetita_net::PayloadLimiter::new(),
            dbus: None,
            notifications: Default::default(),
            last_clipboard: Mutex::new(String::new()),
        })
    }

    fn phone(name: &str) -> (Endpoint, Fingerprint) {
        let cert = DeviceCert::generate(name);
        let fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let hello = Hello {
            device_id: name.into(),
            device_name: name.into(),
            device_kind: DeviceKind::Phone,
            capabilities: vec![CapabilityVersion {
                capability: capability::BATTERY,
                version: 1,
            }],
        };
        (
            Endpoint::bind(
                EndpointConfig { cert, hello },
                "127.0.0.1:0".parse().unwrap(),
            )
            .unwrap(),
            fp,
        )
    }

    #[test]
    fn a_pairing_window_admits_one_phone_and_expires() {
        let arm = PairingArm::default();
        assert!(arm.take_live().is_none());
        let uri = arm.arm("desk", [1u8; 32], vec!["10.0.0.1:1760".into()]);
        assert!(uri.starts_with("magnetita://pair?v=1&id=desk&fp=0101"));
        assert!(arm.is_armed());
        assert!(arm.take_live().is_some());
        assert!(arm.take_live().is_none(), "one QR admits one phone");
        *arm.0.lock_ok() = Some(Armed {
            secret: [0; 32],
            until: Instant::now() - Duration::from_secs(1),
        });
        assert!(arm.take_live().is_none(), "an expired window admits nobody");
    }

    /// The phone half of the QR path: connect to the QR's address, prove, accept the reply.
    async fn prove(phone: &Endpoint, phone_fp: Fingerprint, uri: &str) -> (Session, Fingerprint) {
        let scanned = QrPayload::parse_uri(uri).unwrap();
        let target: SocketAddr = scanned.addresses[0].parse().unwrap();
        let (session, _hello) = phone
            .connect(target, PeerExpect::Fingerprint(scanned.fingerprint))
            .await
            .unwrap();
        let mut pairing = QrPairing::phone(&scanned, phone_fp, session.peer_fingerprint()).unwrap();
        session
            .send_message(
                capability::PAIRING,
                pair_kind::QR_PROOF,
                pairing.proof_to_send().unwrap(),
            )
            .await
            .unwrap();
        // A trusted session may greet with a clipboard request before the
        // pairing reply; only the pairing envelope is the reply.
        let reply = loop {
            let env = tokio::time::timeout(Duration::from_secs(5), session.recv())
                .await
                .expect("a reply within the budget")
                .unwrap();
            if env.capability == capability::PAIRING {
                break env;
            }
        };
        let pinned = pairing.accept_reply(&reply.body).unwrap();
        (session, pinned.peer_fingerprint)
    }

    #[test]
    fn the_clipboard_flows_both_ways_on_the_own_wire() {
        let cert = DeviceCert::generate("desktop");
        let daemon = test_daemon(&cert);
        let arm = PairingArm::default();
        let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&received);
        let (wire, addr) = spawn(
            Arc::clone(&daemon),
            cert.clone(),
            "desktop".into(),
            arm.clone(),
            "127.0.0.1:0".parse().unwrap(),
            false,
            Adapters {
                clipboard_sink: Arc::new(move |text: &str| {
                    sink.lock_ok().push(text.to_owned());
                    true
                }),
                notification_server: Arc::new(notifications::NoServer),
                notifications: Arc::new(notifications::Bridge::default()),
                download_dir: std::env::temp_dir().join("magnetita-test-downloads"),
                shares: Arc::new(share::ShareStore::default()),
                input: Arc::new(input::testing::Recorder::default()),
                mirror_player: Arc::new(mirror::testing::Recorder::default()),
            },
        )
        .unwrap();
        let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (phone, phone_fp) = rt.block_on(async { phone("phone3") });
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));

        // The desktop asks for the phone's clipboard as the session opens.
        let first = rt.block_on(async {
            loop {
                let env = tokio::time::timeout(Duration::from_secs(5), session.recv())
                    .await
                    .unwrap()
                    .unwrap();
                if env.capability == capability::CLIPBOARD {
                    break env;
                }
            }
        });
        assert_eq!(first.kind, ClipboardRequest::KIND);

        // Phone to desktop: the text lands in the sink, recorded as last synced.
        rt.block_on(async {
            session
                .send_message(
                    capability::CLIPBOARD,
                    ClipboardText::KIND,
                    ClipboardText {
                        text: "copied on the phone".into(),
                    }
                    .encode(),
                )
                .await
                .unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while received.lock_ok().is_empty() {
            assert!(
                Instant::now() < deadline,
                "the phone's clipboard never landed"
            );
            thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(received.lock_ok().as_slice(), ["copied on the phone"]);
        assert_eq!(*daemon.last_clipboard.lock_ok(), "copied on the phone");

        // Desktop to phone: the shared slot drains into this session.
        daemon
            .pending_clipboards
            .replace_for(["phone3".to_owned()], "copied on the desk".into());
        let env = rt.block_on(async {
            loop {
                let env = tokio::time::timeout(Duration::from_secs(5), session.recv())
                    .await
                    .unwrap()
                    .unwrap();
                if env.capability == capability::CLIPBOARD {
                    break env;
                }
            }
        });
        assert_eq!(env.kind, ClipboardText::KIND);
        assert_eq!(
            ClipboardText::decode(&env.body).unwrap().text,
            "copied on the desk"
        );
        session.close("done");
        drop(wire);
    }

    #[test]
    fn a_notification_is_shown_replaced_answered_and_closed_over_the_own_wire() {
        use magnetita_proto::daily::notifications::Action;
        let cert = DeviceCert::generate("desktop");
        let daemon = test_daemon(&cert);
        let arm = PairingArm::default();
        let recorder = Arc::new(notifications::testing::Recorder::default());
        let bridge = Arc::new(notifications::Bridge::default());
        let (wire, addr) = spawn(
            Arc::clone(&daemon),
            cert.clone(),
            "desktop".into(),
            arm.clone(),
            "127.0.0.1:0".parse().unwrap(),
            false,
            Adapters {
                clipboard_sink: Arc::new(|_: &str| true),
                notification_server: recorder.clone(),
                notifications: Arc::clone(&bridge),
                download_dir: std::env::temp_dir().join("magnetita-test-downloads"),
                shares: Arc::new(share::ShareStore::default()),
                input: Arc::new(input::testing::Recorder::default()),
                mirror_player: Arc::new(mirror::testing::Recorder::default()),
            },
        )
        .unwrap();
        let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (phone, phone_fp) = rt.block_on(async { phone("phone4") });
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));

        let note = NotificationPosted {
            key: "0|com.example|7".into(),
            app_name: "Messages".into(),
            title: "Ana".into(),
            body: "are you there".into(),
            timestamp_ms: 1,
            replyable: true,
            actions: vec![Action {
                label: "Mark read".into(),
            }],
            icon: None,
            media: false,
        };
        rt.block_on(async {
            session
                .send_message(
                    capability::NOTIFICATIONS,
                    NotificationPosted::KIND,
                    note.encode(),
                )
                .await
                .unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while recorder.posted.lock_ok().is_empty() {
            assert!(Instant::now() < deadline, "the notification never showed");
            thread::sleep(Duration::from_millis(50));
        }
        let shown = recorder.posted.lock_ok()[0].clone();
        assert_eq!(
            (
                shown.app.as_str(),
                shown.summary.as_str(),
                shown.body.as_str()
            ),
            ("Messages", "Ana", "are you there")
        );
        assert!(daemon
            .log
            .lock_ok()
            .iter()
            .any(|e| e.message.contains("Messages: Ana")));

        // The server's signals become messages to the phone.
        for signal in [
            notifications::ServerSignal::Action(shown.id, "0".into()),
            notifications::ServerSignal::Replied(shown.id, "on my way".into()),
        ] {
            let (device, command) = bridge.command_for(signal).unwrap();
            daemon
                .commands
                .lock_ok()
                .get(&device)
                .unwrap()
                .try_send(command)
                .unwrap();
        }
        let mut got = Vec::new();
        rt.block_on(async {
            while got.len() < 2 {
                let env = tokio::time::timeout(Duration::from_secs(5), session.recv())
                    .await
                    .unwrap()
                    .unwrap();
                if env.capability == capability::NOTIFICATIONS {
                    got.push((env.kind, env.body));
                }
            }
        });
        assert_eq!(got[0].0, NotificationAction::KIND);
        assert_eq!(NotificationAction::decode(&got[0].1).unwrap().action, 0);
        assert_eq!(got[1].0, NotificationReply::KIND);
        assert_eq!(
            NotificationReply::decode(&got[1].1).unwrap().text,
            "on my way"
        );

        // The phone withdraws it: the desktop closes the same id.
        rt.block_on(async {
            session
                .send_message(
                    capability::NOTIFICATIONS,
                    NotificationDismissed::KIND,
                    NotificationDismissed {
                        key: note.key.clone(),
                    }
                    .encode(),
                )
                .await
                .unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while recorder.closed.lock_ok().is_empty() {
            assert!(Instant::now() < deadline, "the desktop never closed it");
            thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(recorder.closed.lock_ok().as_slice(), [shown.id]);
        session.close("done");
        drop(wire);
    }

    #[test]
    fn a_file_resumes_after_a_broken_stream_and_flows_both_ways() {
        use magnetita_proto::daily::share::{ShareAccept, ShareDone, ShareOffer};
        use std::io::Write;
        let cert = DeviceCert::generate("desktop");
        let daemon = test_daemon(&cert);
        let arm = PairingArm::default();
        let downloads =
            std::env::temp_dir().join(format!("magnetita-share-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&downloads);
        let (wire, addr) = spawn(
            Arc::clone(&daemon),
            cert.clone(),
            "desktop".into(),
            arm.clone(),
            "127.0.0.1:0".parse().unwrap(),
            false,
            Adapters {
                clipboard_sink: Arc::new(|_: &str| true),
                notification_server: Arc::new(notifications::NoServer),
                notifications: Arc::new(notifications::Bridge::default()),
                download_dir: downloads.clone(),
                shares: Arc::new(share::ShareStore::default()),
                input: Arc::new(input::testing::Recorder::default()),
                mirror_player: Arc::new(mirror::testing::Recorder::default()),
            },
        )
        .unwrap();
        let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (phone, phone_fp) = rt.block_on(async { phone("phone5") });
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));
        let payload: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let next_share = |session: &Session| {
            rt.block_on(async {
                loop {
                    let env = tokio::time::timeout(Duration::from_secs(5), session.recv())
                        .await
                        .expect("a share envelope in time")
                        .unwrap();
                    if env.capability == capability::SHARE {
                        break env;
                    }
                }
            })
        };

        // Phone to desktop, first attempt: half the bytes, then the stream breaks.
        let offer = ShareOffer {
            transfer: 7,
            name: "photo.bin".into(),
            size: payload.len() as u64,
            mime: "application/octet-stream".into(),
        };
        rt.block_on(session.send_message(capability::SHARE, ShareOffer::KIND, offer.encode()))
            .unwrap();
        let accept = ShareAccept::decode(&next_share(&session).body).unwrap();
        assert_eq!((accept.transfer, accept.offset), (7, 0));
        // The stream stays open (dropping it would reset it and discard the
        // bytes); the link itself dies, as it does when Wi-Fi goes.
        let half = rt.block_on(async {
            let mut stream = session.open_transfer(7).await.unwrap();
            stream.write_all(&payload[..150_000]).await.unwrap();
            tokio::time::sleep(Duration::from_millis(300)).await;
            stream
        });
        session.close("wifi dropped");
        drop(half);
        let deadline = Instant::now() + Duration::from_secs(5);
        while daemon.devices.lock_ok().contains_key("phone5") {
            assert!(Instant::now() < deadline, "the broken session never left");
            thread::sleep(Duration::from_millis(50));
        }
        assert!(daemon
            .log
            .lock_ok()
            .iter()
            .any(|e| e.message.contains("transferencia interrumpida")));

        // A new session, the same offer: the desktop asks for the rest only.
        let (session, _hello) = rt
            .block_on(phone.connect(addr, PeerExpect::Fingerprint(desktop_fp)))
            .unwrap();
        rt.block_on(session.send_message(capability::SHARE, ShareOffer::KIND, offer.encode()))
            .unwrap();
        let accept = ShareAccept::decode(&next_share(&session).body).unwrap();
        assert!(
            accept.offset > 0 && accept.offset < payload.len() as u64,
            "resumes from the bytes it holds: {}",
            accept.offset
        );
        rt.block_on(async {
            let mut stream = session.open_transfer(7).await.unwrap();
            stream
                .write_all(&payload[accept.offset as usize..])
                .await
                .unwrap();
            stream.finish().unwrap();
            let _ = stream.stopped().await;
        });
        let done = ShareDone::decode(&next_share(&session).body).unwrap();
        assert!(done.complete);
        let received = std::fs::read(downloads.join("photo.bin")).unwrap();
        assert_eq!(received, payload);
        assert!(daemon
            .log
            .lock_ok()
            .iter()
            .any(|e| e.message.contains("archivo recibido")));

        // Desktop to phone: SendFile offers, the phone accepts and drains the stream.
        let outgoing = downloads.join("from-desk.bin");
        std::fs::File::create(&outgoing)
            .unwrap()
            .write_all(&payload[..100_000])
            .unwrap();
        daemon
            .commands
            .lock_ok()
            .get("phone5")
            .unwrap()
            .try_send(Command::SendFile(outgoing.clone()))
            .unwrap();
        let offer = ShareOffer::decode(&next_share(&session).body).unwrap();
        assert_eq!(
            (offer.name.as_str(), offer.size),
            ("from-desk.bin", 100_000)
        );
        rt.block_on(
            session.send_message(
                capability::SHARE,
                ShareAccept::KIND,
                ShareAccept {
                    transfer: offer.transfer,
                    offset: 0,
                }
                .encode(),
            ),
        )
        .unwrap();
        let got = rt.block_on(async {
            let (id, mut stream) =
                tokio::time::timeout(Duration::from_secs(5), session.accept_transfer())
                    .await
                    .unwrap()
                    .unwrap();
            assert_eq!(id, offer.transfer);
            stream.read_to_end(1 << 20).await.unwrap()
        });
        assert_eq!(got, &payload[..100_000]);
        let done = ShareDone::decode(&next_share(&session).body).unwrap();
        assert!(done.complete);
        session.close("done");
        drop(wire);
        let _ = std::fs::remove_dir_all(&downloads);
    }

    #[test]
    fn the_phone_player_reaches_the_card_and_the_desktop_buttons_reach_the_phone() {
        use magnetita_proto::daily::media::MediaButton;
        let cert = DeviceCert::generate("desktop");
        let daemon = test_daemon(&cert);
        let arm = PairingArm::default();
        let (wire, addr) = spawn(
            Arc::clone(&daemon),
            cert.clone(),
            "desktop".into(),
            arm.clone(),
            "127.0.0.1:0".parse().unwrap(),
            false,
            Adapters {
                clipboard_sink: Arc::new(|_: &str| true),
                notification_server: Arc::new(notifications::NoServer),
                notifications: Arc::new(notifications::Bridge::default()),
                download_dir: std::env::temp_dir().join("magnetita-test-downloads"),
                shares: Arc::new(share::ShareStore::default()),
                input: Arc::new(input::testing::Recorder::default()),
                mirror_player: Arc::new(mirror::testing::Recorder::default()),
            },
        )
        .unwrap();
        let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (phone, phone_fp) = rt.block_on(async { phone("phone6") });
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));

        // The desktop asks for the phone's media as the session opens.
        let first = rt.block_on(async {
            loop {
                let env = tokio::time::timeout(Duration::from_secs(5), session.recv())
                    .await
                    .unwrap()
                    .unwrap();
                if env.capability == capability::MEDIA {
                    break env;
                }
            }
        });
        assert_eq!(first.kind, MediaRequest::KIND);

        rt.block_on(
            session.send_message(
                capability::MEDIA,
                MediaState::KIND,
                MediaState {
                    player: "YT Music".into(),
                    title: "Song".into(),
                    artist: "Band".into(),
                    album: String::new(),
                    playing: true,
                    position_ms: 10_000,
                    length_ms: 200_000,
                    can_seek: true,
                    can_next: true,
                    can_previous: true,
                    volume: 60,
                }
                .encode(),
            ),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let entry = daemon.devices.lock_ok().get("phone6").cloned();
            if let Some(e) = entry.filter(|e| e.media_player == "YT Music") {
                assert_eq!(
                    (e.media_title.as_str(), e.media_now_playing.as_str()),
                    ("Song", "Band - Song")
                );
                assert!(e.media_playing && e.media_can_next);
                assert_eq!(e.media_length, 200_000);
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the phone's player never reached the card"
            );
            thread::sleep(Duration::from_millis(50));
        }

        daemon
            .commands
            .lock_ok()
            .get("phone6")
            .unwrap()
            .try_send(Command::Media(magnetita_core::MediaAction::Next))
            .unwrap();
        let env = rt.block_on(async {
            loop {
                let env = tokio::time::timeout(Duration::from_secs(5), session.recv())
                    .await
                    .unwrap()
                    .unwrap();
                if env.capability == capability::MEDIA && env.kind == MediaCommand::KIND {
                    break env;
                }
            }
        });
        let command = MediaCommand::decode(&env.body).unwrap();
        assert_eq!(
            (command.player.as_str(), command.button),
            ("YT Music", Some(MediaButton::Next))
        );
        session.close("done");
        drop(wire);
    }

    #[test]
    fn contacts_name_the_sms_and_the_call_and_the_desktop_answers_both() {
        use magnetita_proto::phone::contacts::{Contact, ContactsRequest};
        use magnetita_proto::phone::sms::{SmsMessage, SmsSend};
        use magnetita_proto::phone::telephony::{CallAction, CallCommand};
        let cert = DeviceCert::generate("desktop");
        let daemon = test_daemon(&cert);
        let arm = PairingArm::default();
        let recorder = Arc::new(notifications::testing::Recorder::default());
        let bridge = Arc::new(notifications::Bridge::default());
        let (wire, addr) = spawn(
            Arc::clone(&daemon),
            cert.clone(),
            "desktop".into(),
            arm.clone(),
            "127.0.0.1:0".parse().unwrap(),
            false,
            Adapters {
                clipboard_sink: Arc::new(|_: &str| true),
                notification_server: recorder.clone(),
                notifications: Arc::clone(&bridge),
                download_dir: std::env::temp_dir().join("magnetita-test-downloads"),
                shares: Arc::new(share::ShareStore::default()),
                input: Arc::new(input::testing::Recorder::default()),
                mirror_player: Arc::new(mirror::testing::Recorder::default()),
            },
        )
        .unwrap();
        let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (phone, phone_fp) = rt.block_on(async { phone("phone7") });
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));
        let next_of = |cap: u16, kind: u16| {
            rt.block_on(async {
                loop {
                    let env = tokio::time::timeout(Duration::from_secs(5), session.recv())
                        .await
                        .expect("an envelope in time")
                        .unwrap();
                    if env.capability == cap && env.kind == kind {
                        break env;
                    }
                }
            })
        };

        // The desktop asks for contacts from version 0 and for the conversations.
        let ask = next_of(capability::CONTACTS, ContactsRequest::KIND);
        assert_eq!(ContactsRequest::decode(&ask.body).unwrap().since_version, 0);
        next_of(capability::SMS, SmsConversations::KIND);

        rt.block_on(session.send_message(
            capability::CONTACTS,
            ContactsSync::KIND,
            ContactsSync {
                version: 3,
                contacts: vec![Contact {
                    id: 1,
                    version: 3,
                    vcard: "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Ana\r\nTEL:+34600111222\r\nEND:VCARD\r\n".into(),
                }],
                removed: vec![],
                complete: true,
            }
            .encode(),
        ))
        .unwrap();
        let message = SmsMessage {
            id: 5,
            from_me: false,
            address: "600111222".into(),
            body: "are you coming".into(),
            timestamp_ms: 1,
            attachments: vec![],
        };
        rt.block_on(
            session.send_message(
                capability::SMS,
                SmsReceived::KIND,
                SmsReceived {
                    thread: 42,
                    message,
                }
                .encode(),
            ),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while recorder.posted.lock_ok().is_empty() {
            assert!(Instant::now() < deadline, "the SMS never showed");
            thread::sleep(Duration::from_millis(50));
        }
        let shown = recorder.posted.lock_ok()[0].clone();
        assert_eq!((shown.app.as_str(), shown.summary.as_str()), ("SMS", "Ana"));
        assert!(shown.replyable);
        assert_eq!(phone::store().conversations("phone7").len(), 1);

        // Replying on that notification sends in the thread.
        let (device, command) = bridge
            .command_for(notifications::ServerSignal::Replied(shown.id, "yes".into()))
            .unwrap();
        daemon
            .commands
            .lock_ok()
            .get(&device)
            .unwrap()
            .try_send(command)
            .unwrap();
        let sent = next_of(capability::SMS, SmsSend::KIND);
        let sent = SmsSend::decode(&sent.body).unwrap();
        assert_eq!((sent.thread, sent.body.as_str()), (42, "yes"));

        // A ringing call: the registry names Ana, the notification has three buttons.
        rt.block_on(
            session.send_message(
                capability::TELEPHONY,
                CallEvent::KIND,
                CallEvent {
                    state: CallState::Ringing,
                    number: "+34600111222".into(),
                    name: None,
                    timestamp_ms: 2,
                }
                .encode(),
            ),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let entry = daemon.devices.lock_ok().get("phone7").cloned();
            if let Some(e) = entry.filter(|e| e.call_state == "ringing") {
                assert_eq!(e.call_name, "Ana");
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the call never reached the registry"
            );
            thread::sleep(Duration::from_millis(50));
        }
        let call = recorder
            .posted
            .lock_ok()
            .iter()
            .find(|p| p.app == "Tel\u{e9}fono")
            .cloned()
            .expect("the call notification");
        assert_eq!(call.buttons.len(), 3);
        let (device, command) = bridge
            .command_for(notifications::ServerSignal::Action(call.id, "2".into()))
            .unwrap();
        daemon
            .commands
            .lock_ok()
            .get(&device)
            .unwrap()
            .try_send(command)
            .unwrap();
        let hang = next_of(capability::TELEPHONY, CallCommand::KIND);
        assert_eq!(
            CallCommand::decode(&hang.body).unwrap().action,
            CallAction::HangUp
        );

        // The call ends: the registry clears and the notification closes.
        rt.block_on(
            session.send_message(
                capability::TELEPHONY,
                CallEvent::KIND,
                CallEvent {
                    state: CallState::Ended,
                    number: "+34600111222".into(),
                    name: None,
                    timestamp_ms: 3,
                }
                .encode(),
            ),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !recorder.closed.lock_ok().contains(&call.id) {
            assert!(
                Instant::now() < deadline,
                "the call notification never closed"
            );
            thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(
            daemon.devices.lock_ok().get("phone7").unwrap().call_state,
            ""
        );
        session.close("done");
        drop(wire);
    }

    #[test]
    fn a_registered_command_runs_by_id_and_input_reaches_the_sink_by_stream_and_datagram() {
        use magnetita_proto::control::commands::CommandList;
        use magnetita_proto::control::input::{Key, PointerMove, Text};
        let cert = DeviceCert::generate("desktop");
        let daemon = test_daemon(&cert);
        let arm = PairingArm::default();
        let recorder = Arc::new(input::testing::Recorder::default());
        let (wire, addr) = spawn(
            Arc::clone(&daemon),
            cert.clone(),
            "desktop".into(),
            arm.clone(),
            "127.0.0.1:0".parse().unwrap(),
            false,
            Adapters {
                clipboard_sink: Arc::new(|_: &str| true),
                notification_server: Arc::new(notifications::NoServer),
                notifications: Arc::new(notifications::Bridge::default()),
                download_dir: std::env::temp_dir().join("magnetita-test-downloads"),
                shares: Arc::new(share::ShareStore::default()),
                input: recorder.clone(),
                mirror_player: Arc::new(mirror::testing::Recorder::default()),
            },
        )
        .unwrap();
        let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (phone, phone_fp) = rt.block_on(async { phone("phone8") });
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));
        let next_of = |cap: u16, kind: u16| {
            rt.block_on(async {
                loop {
                    let env = tokio::time::timeout(Duration::from_secs(5), session.recv())
                        .await
                        .expect("an envelope in time")
                        .unwrap();
                    if env.capability == cap && env.kind == kind {
                        break env;
                    }
                }
            })
        };

        // The session greets with the registered commands; the registry is
        // the daemon's one store, so the test registers and cleans up its own.
        let published = next_of(capability::COMMANDS, CommandList::KIND);
        let before = CommandList::decode(&published.body).unwrap().commands.len();
        let id = commands::store()
            .set(0, "Loopback true", "true", vec![])
            .unwrap();
        daemon
            .commands
            .lock_ok()
            .get("phone8")
            .unwrap()
            .try_send(Command::CommandsChanged)
            .unwrap();
        let published = next_of(capability::COMMANDS, CommandList::KIND);
        let list = CommandList::decode(&published.body).unwrap();
        assert_eq!(list.commands.len(), before + 1);
        assert!(list
            .commands
            .iter()
            .any(|c| c.id == id && c.name == "Loopback true"));

        rt.block_on(session.send_message(
            capability::COMMANDS,
            CommandRun::KIND,
            CommandRun { id }.encode(),
        ))
        .unwrap();
        let result =
            CommandResult::decode(&next_of(capability::COMMANDS, CommandResult::KIND).body)
                .unwrap();
        assert!(result.ok && result.id == id);
        rt.block_on(session.send_message(
            capability::COMMANDS,
            CommandRun::KIND,
            CommandRun { id: 999_999 }.encode(),
        ))
        .unwrap();
        let result =
            CommandResult::decode(&next_of(capability::COMMANDS, CommandResult::KIND).body)
                .unwrap();
        assert!(!result.ok, "an unknown id runs nothing");
        commands::store().remove(id).unwrap();

        // Input: a key and a text on the stream, motion as a datagram.
        rt.block_on(async {
            session
                .send_message(
                    capability::INPUT,
                    Key::KIND,
                    Key {
                        code: 30,
                        pressed: true,
                    }
                    .encode(),
                )
                .await
                .unwrap();
            session
                .send_message(
                    capability::INPUT,
                    Text::KIND,
                    Text { text: "hi".into() }.encode(),
                )
                .await
                .unwrap();
            session
                .send_datagram(
                    capability::INPUT,
                    PointerMove::KIND,
                    PointerMove { dx: 5, dy: -3 }.encode(),
                )
                .unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let seen = recorder.0.lock_ok().clone();
            if seen.len() >= 3 {
                assert!(seen.contains(&"key 30 true".to_string()));
                assert!(seen.contains(&"text hi".to_string()));
                assert!(seen.contains(&"move 5 -3".to_string()));
                break;
            }
            assert!(
                Instant::now() < deadline,
                "input never reached the sink: {seen:?}"
            );
            thread::sleep(Duration::from_millis(50));
        }
        session.close("done");
        drop(wire);
    }

    #[test]
    fn the_link_mirror_starts_streams_into_the_window_and_stops() {
        use magnetita_proto::mirror::{Codec, MirrorStart, MirrorTouch, TouchAction};
        let cert = DeviceCert::generate("desktop");
        let daemon = test_daemon(&cert);
        let arm = PairingArm::default();
        let player = Arc::new(mirror::testing::Recorder::default());
        let (wire, addr) = spawn(
            Arc::clone(&daemon),
            cert.clone(),
            "desktop".into(),
            arm.clone(),
            "127.0.0.1:0".parse().unwrap(),
            false,
            Adapters {
                clipboard_sink: Arc::new(|_: &str| true),
                notification_server: Arc::new(notifications::NoServer),
                notifications: Arc::new(notifications::Bridge::default()),
                download_dir: std::env::temp_dir().join("magnetita-test-downloads"),
                shares: Arc::new(share::ShareStore::default()),
                input: Arc::new(input::testing::Recorder::default()),
                mirror_player: player.clone(),
            },
        )
        .unwrap();
        let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (phone, phone_fp) = rt.block_on(async { phone("phone9") });
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));
        let next_of = |cap: u16, kind: u16| {
            rt.block_on(async {
                loop {
                    let env = tokio::time::timeout(Duration::from_secs(5), session.recv())
                        .await
                        .expect("an envelope in time")
                        .unwrap();
                    if env.capability == cap && env.kind == kind {
                        break env;
                    }
                }
            })
        };

        // The desktop wants the mirror: the tick sends the start.
        mirror::own().request_start_from(
            "phone9",
            MirrorStart {
                max_size: 1440,
                fps: 60,
                bitrate_kbps: 6000,
                codec: Codec::Hevc,
                audio: false,
            },
        );
        let start =
            MirrorStart::decode(&next_of(capability::MIRROR, MirrorStart::KIND).body).unwrap();
        assert_eq!(start.fps, 60);

        // The phone answers and streams; the window receives the bytes.
        rt.block_on(
            session.send_message(
                capability::MIRROR,
                MirrorStarted::KIND,
                MirrorStarted {
                    width: 1080,
                    height: 2340,
                    codec: Codec::Hevc,
                    audio: false,
                }
                .encode(),
            ),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while player.opened.lock_ok().is_empty() {
            assert!(Instant::now() < deadline, "the window never opened");
            thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(player.opened.lock_ok()[0], (1080, 2340));
        let frame: Vec<u8> = (0..100_000u32).map(|i| (i % 7) as u8).collect();
        rt.block_on(async {
            let mut stream = session.open_transfer(mirror::VIDEO_STREAM).await.unwrap();
            stream.write_all(&frame).await.unwrap();
            stream.finish().unwrap();
            let _ = stream.stopped().await;
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while player.bytes.lock_ok().len() < frame.len() {
            assert!(
                Instant::now() < deadline,
                "the picture never reached the window"
            );
            thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(*player.bytes.lock_ok(), frame);

        // A touch queued by Mirror1 reaches the phone; a stop closes the window.
        mirror::own().queue_input(magnetita_proto::Envelope {
            capability: capability::MIRROR,
            kind: MirrorTouch::KIND,
            id: 0,
            body: MirrorTouch {
                action: TouchAction::Down,
                x: 10,
                y: 20,
                pointer: 0,
            }
            .encode(),
        });
        let touch =
            MirrorTouch::decode(&next_of(capability::MIRROR, MirrorTouch::KIND).body).unwrap();
        assert_eq!((touch.x, touch.y), (10, 20));
        mirror::own().request_stop();
        next_of(capability::MIRROR, MirrorStop::KIND);
        assert_eq!(mirror::own().state(), mirror::LinkState::Idle);
        session.close("done");
        drop(wire);
    }

    /// A phone endpoint that serves `root` as its storage until the session
    /// ends; returns when the desktop closes.
    fn serve_storage(
        rt: &tokio::runtime::Runtime,
        session: Arc<magnetita_link::Session>,
        root: std::path::PathBuf,
        announce: bool,
    ) -> tokio::task::JoinHandle<()> {
        rt.spawn(async move {
            if announce {
                let state = magnetita_mobile::storage::state(true);
                session
                    .send_message(state.capability, state.kind, state.body)
                    .await
                    .unwrap();
            }
            while let Ok(env) = session.recv().await {
                if let Some(request) = magnetita_mobile::storage::StorageRequest::decode(&env) {
                    let reply = magnetita_mobile::storage::serve(&root, &request);
                    if session
                        .send_message(reply.capability, reply.kind, reply.body)
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        })
    }

    fn test_adapters() -> Adapters {
        Adapters {
            clipboard_sink: Arc::new(|_: &str| true),
            notification_server: Arc::new(notifications::NoServer),
            notifications: Arc::new(notifications::Bridge::default()),
            download_dir: std::env::temp_dir().join("magnetita-test-downloads"),
            shares: Arc::new(share::ShareStore::default()),
            input: Arc::new(input::testing::Recorder::default()),
            mirror_player: Arc::new(mirror::testing::Recorder::default()),
        }
    }

    fn storage_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("magnetita-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("DCIM")).unwrap();
        std::fs::write(root.join("DCIM/one.jpg"), vec![7u8; 300_000]).unwrap();
        std::fs::write(root.join("notes.txt"), b"phone notes").unwrap();
        root
    }

    #[test]
    fn the_storage_client_browses_the_phone_tree() {
        let cert = DeviceCert::generate("desktop");
        let daemon = test_daemon(&cert);
        let arm = PairingArm::default();
        let (wire, addr) = spawn(
            Arc::clone(&daemon),
            cert.clone(),
            "desktop".into(),
            arm.clone(),
            "127.0.0.1:0".parse().unwrap(),
            false,
            test_adapters(),
        )
        .unwrap();
        let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (phone, phone_fp) = rt.block_on(async { phone("phone10") });
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));
        let session = Arc::new(session);
        let root = storage_root("storage");
        let server = serve_storage(&rt, Arc::clone(&session), root.clone(), false);

        let deadline = Instant::now() + Duration::from_secs(5);
        let client = loop {
            if let Some(c) = storage::clients().lock_ok().get("phone10").cloned() {
                break c;
            }
            assert!(Instant::now() < deadline, "the client never registered");
            thread::sleep(Duration::from_millis(20));
        };
        rt.block_on(async {
            let mut names: Vec<String> = client
                .list("")
                .await
                .unwrap()
                .into_iter()
                .map(|e| e.name)
                .collect();
            names.sort();
            assert_eq!(names, ["DCIM", "notes.txt"]);
            let entry = client.stat("DCIM/one.jpg").await.unwrap().unwrap();
            assert_eq!((entry.dir, entry.size), (false, 300_000));
            assert!(client.stat("nope").await.unwrap().is_none());
            let bytes = client.read("DCIM/one.jpg", 299_990, 1000).await.unwrap();
            assert_eq!(bytes.len(), 10);
            client
                .write("new.txt", 0, b"from the desktop", true)
                .await
                .unwrap();
            client.mkdir("Music").await.unwrap();
            client.rename("new.txt", "Music/new.txt").await.unwrap();
            assert_eq!(
                std::fs::read(root.join("Music/new.txt")).unwrap(),
                b"from the desktop"
            );
            client.delete("Music/new.txt").await.unwrap();
            client.delete("Music").await.unwrap();
            assert!(client.delete("Music").await.is_err());
            assert!(!root.join("Music").exists());
        });
        session.close("done");
        let _ = rt.block_on(server);
        drop(wire);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Mounts through the kernel: needs `/dev/fuse` and `fusermount3`.
    #[test]
    #[ignore]
    fn the_phone_is_a_directory_while_it_shares_a_root() {
        let runtime_dir =
            std::env::temp_dir().join(format!("magnetita-xdg-{}", std::process::id()));
        std::fs::create_dir_all(&runtime_dir).unwrap();
        std::env::set_var("XDG_RUNTIME_DIR", &runtime_dir);
        let cert = DeviceCert::generate("desktop");
        let daemon = test_daemon(&cert);
        let arm = PairingArm::default();
        let (wire, addr) = spawn(
            Arc::clone(&daemon),
            cert.clone(),
            "desktop".into(),
            arm.clone(),
            "127.0.0.1:0".parse().unwrap(),
            false,
            test_adapters(),
        )
        .unwrap();
        let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (phone, phone_fp) = rt.block_on(async { phone("phone11") });
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));
        let session = Arc::new(session);
        let root = storage_root("fuse");
        let server = serve_storage(&rt, Arc::clone(&session), root.clone(), true);

        let deadline = Instant::now() + Duration::from_secs(10);
        let mountpoint = loop {
            let path = daemon
                .devices
                .lock_ok()
                .get("phone11")
                .filter(|d| d.mounted)
                .map(|d| d.mount_path.clone());
            if let Some(path) = path {
                break std::path::PathBuf::from(path);
            }
            assert!(Instant::now() < deadline, "the phone never mounted");
            thread::sleep(Duration::from_millis(50));
        };
        // Siderita's view: plain std::fs on the mount.
        let mut names: Vec<String> = std::fs::read_dir(&mountpoint)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["DCIM", "notes.txt"]);
        assert_eq!(
            std::fs::read(mountpoint.join("notes.txt")).unwrap(),
            b"phone notes"
        );
        assert_eq!(
            std::fs::metadata(mountpoint.join("DCIM/one.jpg"))
                .unwrap()
                .len(),
            300_000
        );
        assert_eq!(
            std::fs::read(mountpoint.join("DCIM/one.jpg"))
                .unwrap()
                .len(),
            300_000
        );
        std::fs::write(mountpoint.join("DCIM/copy.txt"), b"written through fuse").unwrap();
        assert_eq!(
            std::fs::read(root.join("DCIM/copy.txt")).unwrap(),
            b"written through fuse"
        );
        std::fs::create_dir(mountpoint.join("Music")).unwrap();
        std::fs::rename(
            mountpoint.join("DCIM/copy.txt"),
            mountpoint.join("Music/copy.txt"),
        )
        .unwrap();
        assert!(root.join("Music/copy.txt").exists());
        std::fs::remove_file(mountpoint.join("Music/copy.txt")).unwrap();
        std::fs::remove_dir(mountpoint.join("Music")).unwrap();
        assert!(!root.join("Music").exists());

        session.close("done");
        let _ = rt.block_on(server);
        let deadline = Instant::now() + Duration::from_secs(10);
        while daemon
            .devices
            .lock_ok()
            .get("phone11")
            .is_some_and(|d| d.mounted)
        {
            assert!(Instant::now() < deadline, "the mount never released");
            thread::sleep(Duration::from_millis(50));
        }
        assert!(
            std::fs::read_dir(&mountpoint)
                .map(|d| d.count())
                .unwrap_or(0)
                == 0
        );
        drop(wire);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_phone_that_connects_again_replaces_its_earlier_session() {
        let cert = DeviceCert::generate("desktop");
        let daemon = test_daemon(&cert);
        let arm = PairingArm::default();
        let (wire, addr) = spawn(
            Arc::clone(&daemon),
            cert.clone(),
            "desktop".into(),
            arm.clone(),
            "127.0.0.1:0".parse().unwrap(),
            false,
            test_adapters(),
        )
        .unwrap();
        let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (phone, phone_fp) = rt.block_on(async { phone("phone12") });
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (first, _) = rt.block_on(prove(&phone, phone_fp, &uri));
        let deadline = Instant::now() + Duration::from_secs(5);
        while !daemon.devices.lock_ok().contains_key("phone12") {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(20));
        }
        // The application restarts: the same phone dials again while the
        // first session is still open on this side.
        let second = rt.block_on(async {
            let (session, _) = phone
                .connect(addr, PeerExpect::Fingerprint(desktop_fp))
                .await
                .unwrap();
            session
        });
        // The first session is closed by the desktop; the second lives.
        let closed = rt.block_on(async {
            let until = tokio::time::Instant::now() + Duration::from_secs(8);
            loop {
                match tokio::time::timeout_at(until, first.recv()).await {
                    Ok(Ok(_greeting)) => continue,
                    Ok(Err(_)) => break true,
                    Err(_) => break false,
                }
            }
        });
        assert!(closed, "the earlier session was told to leave");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let alive = rt.block_on(async {
                tokio::time::timeout(Duration::from_millis(200), second.recv())
                    .await
                    .map(|r| r.is_ok())
                    .unwrap_or(true)
            });
            assert!(alive, "the newer session must stay open");
            if daemon
                .devices
                .lock_ok()
                .get("phone12")
                .is_some_and(|d| d.connected)
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the newer session never published"
            );
        }
        second.close("done");
        drop(wire);
    }

    #[test]
    fn a_phone_that_forgot_the_desktop_pairs_again_while_still_pinned() {
        let cert = DeviceCert::generate("desktop");
        let daemon = test_daemon(&cert);
        let arm = PairingArm::default();
        let (wire, addr) = spawn(
            Arc::clone(&daemon),
            cert.clone(),
            "desktop".into(),
            arm.clone(),
            "127.0.0.1:0".parse().unwrap(),
            false,
            Adapters {
                clipboard_sink: Arc::new(|_: &str| true),
                notification_server: Arc::new(notifications::NoServer),
                notifications: Arc::new(notifications::Bridge::default()),
                download_dir: std::env::temp_dir().join("magnetita-test-downloads"),
                shares: Arc::new(share::ShareStore::default()),
                input: Arc::new(input::testing::Recorder::default()),
                mirror_player: Arc::new(mirror::testing::Recorder::default()),
            },
        )
        .unwrap();
        let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (phone, phone_fp) = rt.block_on(async { phone("phone2") });

        // First pairing, then the phone drops its session (and, in life, its pin).
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (first, fp) = rt.block_on(prove(&phone, phone_fp, &uri));
        assert_eq!(fp, desktop_fp);
        first.close("phone forgets");
        let deadline = Instant::now() + Duration::from_secs(5);
        while daemon.devices.lock_ok().contains_key("phone2") {
            assert!(Instant::now() < deadline, "the first session never left");
            thread::sleep(Duration::from_millis(50));
        }
        assert!(
            daemon.trust.lock_ok().is_trusted("phone2"),
            "the desktop still pins it"
        );

        // A new QR: the daemon admits the phone as trusted, yet answers its proof.
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (second, fp) = rt.block_on(prove(&phone, phone_fp, &uri));
        assert_eq!(fp, desktop_fp);
        rt.block_on(async {
            second
                .send_message(
                    capability::BATTERY,
                    BatteryStatus::KIND,
                    BatteryStatus {
                        level: 20,
                        charging: false,
                        low: false,
                    }
                    .encode(),
                )
                .await
                .unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let entry = daemon.devices.lock_ok().get("phone2").cloned();
            if entry.filter(|e| e.battery == 20).is_some() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the second session never carried the battery"
            );
            thread::sleep(Duration::from_millis(50));
        }
        second.close("done");
        drop(wire);
    }

    #[test]
    fn a_phone_pairs_by_qr_reports_battery_and_is_cut_by_forget() {
        let cert = DeviceCert::generate("desktop");
        let daemon = test_daemon(&cert);
        let arm = PairingArm::default();
        let (wire, addr) = spawn(
            Arc::clone(&daemon),
            cert.clone(),
            "desktop".into(),
            arm.clone(),
            "127.0.0.1:0".parse().unwrap(),
            false,
            Adapters {
                clipboard_sink: Arc::new(|_: &str| true),
                notification_server: Arc::new(notifications::NoServer),
                notifications: Arc::new(notifications::Bridge::default()),
                download_dir: std::env::temp_dir().join("magnetita-test-downloads"),
                shares: Arc::new(share::ShareStore::default()),
                input: Arc::new(input::testing::Recorder::default()),
                mirror_player: Arc::new(mirror::testing::Recorder::default()),
            },
        )
        .unwrap();
        let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let (phone, phone_fp) = rt.block_on(async { phone("phone1") });
        let session = rt.block_on(async {
            let scanned = QrPayload::parse_uri(&uri).unwrap();
            let target: SocketAddr = scanned.addresses[0].parse().unwrap();
            let (session, hello) = phone
                .connect(target, PeerExpect::Fingerprint(scanned.fingerprint))
                .await
                .unwrap();
            assert_eq!(hello.device_id, "desktop");
            let mut pairing =
                QrPairing::phone(&scanned, phone_fp, session.peer_fingerprint()).unwrap();
            session
                .send_message(
                    capability::PAIRING,
                    pair_kind::QR_PROOF,
                    pairing.proof_to_send().unwrap(),
                )
                .await
                .unwrap();
            let reply = session.recv().await.unwrap();
            assert_eq!(
                pairing.accept_reply(&reply.body).unwrap().peer_fingerprint,
                desktop_fp
            );
            session
                .send_message(
                    capability::BATTERY,
                    BatteryStatus::KIND,
                    BatteryStatus {
                        level: 61,
                        charging: true,
                        low: false,
                    }
                    .encode(),
                )
                .await
                .unwrap();
            session
        });

        // The pin is durable in the shared store, the entry is published, the battery lands.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let entry = daemon.devices.lock_ok().get("phone1").cloned();
            if let Some(e) = entry.filter(|e| e.battery == 61) {
                assert!(e.paired && e.connected && e.charging);
                assert_eq!(e.fingerprint, fingerprint_text(&phone_fp));
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the phone never appeared with its battery"
            );
            thread::sleep(Duration::from_millis(50));
        }
        assert!(daemon.trust.lock_ok().is_trusted("phone1"));

        // A Ring command reaches the phone as a FindRing envelope.
        daemon
            .commands
            .lock_ok()
            .get("phone1")
            .unwrap()
            .try_send(Command::Ring)
            .unwrap();
        let ring = rt.block_on(async {
            loop {
                let env = session.recv().await.unwrap();
                if env.capability == capability::FIND {
                    break env;
                }
            }
        });
        assert_eq!(
            (ring.capability, ring.kind),
            (capability::FIND, FindRing::KIND)
        );

        // Forget: the trust is gone and the session is cut within the tick.
        daemon
            .revocations
            .request_if_and_apply(
                "phone1",
                || true,
                || daemon.trust.lock_ok().forget("phone1"),
            )
            .unwrap();
        let closed = rt
            .block_on(async { tokio::time::timeout(Duration::from_secs(5), session.recv()).await });
        assert!(
            matches!(closed, Ok(Err(_))),
            "the daemon closes the session on Forget"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while daemon.devices.lock_ok().contains_key("phone1") {
            assert!(
                Instant::now() < deadline,
                "the entry never left the registry"
            );
            thread::sleep(Duration::from_millis(50));
        }
        assert!(!daemon.trust.lock_ok().is_trusted("phone1"));

        // Once forgotten, the same certificate is refused: no window is armed.
        let refused = rt.block_on(async {
            let snapshot = TrustStore::in_memory();
            phone
                .connect(addr, PeerExpect::Trusted(&snapshot))
                .await
                .map(|_| ())
        });
        assert!(refused.is_err());
        drop(wire);
    }
}
