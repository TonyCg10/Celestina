//! The own protocol inside the daemon: one runtime thread that listens,
//! pairs and runs sessions, publishing each phone into the registry
//! `org.celestina.Devices1` serves. The phone always dials: it finds the
//! desktop by the QR's address or by this daemon's mDNS advertisement, and
//! the desktop never browses for phones.
//!
//! The daemon stays thread-based; QUIC needs an async runtime, so this module
//! owns exactly one, on one thread, and everything the link does happens
//! there. What crosses to the rest of the daemon is a [`DeviceEntry`] in the
//! registry, a command channel, a [`SessionRegistration`] whose drop cleans
//! up, and the revocation barrier — `Forget` on `org.celestina.Devices1`
//! forgets the pin in the shared trust store, and the session's tick closes
//! the session and acknowledges the generation.
//!
//! Nothing a session's loop does waits on the phone or on the desktop: its
//! sends are queued for the control stream's one writer, which closes the
//! connection when a write outlives its deadline (`writer`), and the
//! adapters that block (the clipboard's `wl-copy`, notification calls) run
//! on their own thread (`desktop_worker`). So the tick that enforces Forget,
//! stop and supersede always runs.
//!
//! Pairing is armed from the app: [`PairingArm::arm`] draws a one-time
//! secret and returns the QR text; for two minutes an unpinned phone may
//! connect, and it is admitted only under the certificate it presents and
//! only until it proves the secret. Everything else unpinned is refused
//! before its first envelope. Who may become a session, and under which id,
//! is `admission`'s: a session runs under the id its certificate is pinned
//! under, and only a verified proof closes the pairing window.
//!
//! Every handshake runs in a task of its own: the accept loop only lets an
//! attempt in, under the link's bounded handshake slots, so a peer that
//! stalls its TLS never keeps the next one waiting.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use magnetita_link::endpoint::Expect;
use magnetita_link::trust::fingerprint_text;
use magnetita_link::{
    DeviceCert, Endpoint, EndpointConfig, LinkError, Session, TrustStore, TrustedPeer,
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
use magnetita_proto::pair::{kind as pair_kind, Fingerprint, QrPairing};
use magnetita_proto::phone::contacts::ContactsSync;
use magnetita_proto::phone::sms::{SmsConversations, SmsReceived, SmsThread};
use magnetita_proto::phone::telephony::{CallEvent, CallState};
use magnetita_proto::storage::StorageState;
use magnetita_proto::{capability, CapabilityVersion, DeviceKind, Hello, Negotiated};

use crate::devices::{command_channel, Command, DeviceEntry};
mod admission;
pub(crate) mod commands;
mod desktop_worker;
pub(crate) mod discovery;
pub(crate) mod input;
pub(crate) mod media;
pub(crate) mod mirror;
pub(crate) mod mirror_stream;
pub(crate) mod notifications;
pub(crate) mod phone;
pub(crate) mod share;
pub(crate) mod storage;
pub(crate) mod writer;

use crate::lock::LockOk;
use crate::runtime::log;
use crate::session_registration::SessionRegistration;
use crate::{ui_log, Daemon};
pub(crate) use admission::PairingArm;
use discovery::Advertisement;
use writer::Outbox;

/// How often a session checks the revocation barrier and its command queue.
const TICK: Duration = Duration::from_secs(1);
/// How long a newer session of the same phone waits for the older one to
/// leave: a tick to notice the order, and the cleanup after it.
const SUPERSEDE_WAIT: Duration = Duration::from_secs(5);
/// How long a stopping wire waits for its sessions to notice on their tick
/// and clean up: the unmount included.
const STOP_WAIT: Duration = Duration::from_secs(5);
/// How long a stopping session waits for its queued sends to be written,
/// then for the phone to acknowledge receiving them; together well inside
/// [`STOP_WAIT`].
const STOP_FLUSH: Duration = Duration::from_secs(1);
const STOP_FINISH: Duration = Duration::from_secs(2);

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

/// Lets every task end on its own, up to `limit`, then cuts what is left;
/// true when none had to be cut. A session sees the stop on its tick and runs
/// its whole cleanup, awaits included; cutting it drops it at an await.
async fn drain(tasks: &mut tokio::task::JoinSet<()>, limit: Duration) -> bool {
    let ended = tokio::time::timeout(limit, async { while tasks.join_next().await.is_some() {} })
        .await
        .is_ok();
    if !ended {
        tasks.shutdown().await;
    }
    ended
}

/// What this daemon offers on the own wire today, each at version 1: every
/// capability it sends or handles. A session uses only what the phone's
/// hello also offers ([`Negotiated`]).
pub(crate) fn offered() -> Vec<CapabilityVersion> {
    [
        capability::BATTERY,
        capability::FIND,
        capability::CLIPBOARD,
        capability::NOTIFICATIONS,
        capability::SHARE,
        capability::MEDIA,
        capability::CONTACTS,
        capability::SMS,
        capability::TELEPHONY,
        capability::COMMANDS,
        capability::INPUT,
        capability::MIRROR,
        capability::STORAGE,
    ]
    .into_iter()
    .map(|capability| CapabilityVersion {
        capability,
        version: 1,
    })
    .collect()
}

fn hello(device_id: &str) -> Hello {
    Hello {
        device_id: device_id.to_owned(),
        device_name: "Celestina".into(),
        device_kind: DeviceKind::Desktop,
        capabilities: offered(),
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
        Some(connection) => Arc::new(notifications::DbusServer::new(connection.clone())),
        None => Arc::new(notifications::NoServer),
    };
    let bridge = Arc::new(notifications::Bridge::default());
    notifications::spawn_signal_watch(Arc::clone(&bridge), Arc::clone(&daemon));
    match open_wire(
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

/// [`spawn`], then the sweep of the partials a previous run left in the
/// downloads directory. The sweep comes only once the port is bound: a second
/// daemon started by hand fails to bind and must not delete the live one's
/// transfers on its way out.
fn open_wire(
    daemon: Arc<Daemon>,
    cert: DeviceCert,
    device_id: String,
    pairing: PairingArm,
    bind: SocketAddr,
    advertise: bool,
    adapters: Adapters,
) -> Result<(LinkWire, SocketAddr), LinkError> {
    let download_dir = adapters.download_dir.clone();
    let opened = spawn(daemon, cert, device_id, pairing, bind, advertise, adapters)?;
    let swept = crate::incoming_file::sweep_partials(&download_dir);
    if swept > 0 {
        log(
            "link",
            &format!("removed {swept} partial file(s) a previous run left"),
        );
    }
    Ok(opened)
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
            let desktop = match desktop_worker::DesktopWorker::spawn() {
                Ok(worker) => worker,
                Err(e) => {
                    let _ = bound_tx.send(Err(LinkError::Io(e)));
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
                pairing_attempts: Arc::new(tokio::sync::Semaphore::new(
                    admission::MAX_PAIRING_ATTEMPTS,
                )),
                adapters,
                desktop,
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
    /// Unpinned connections waiting for their proof hold one of these.
    pairing_attempts: Arc<tokio::sync::Semaphore>,
    adapters: Adapters,
    /// Runs the adapters that block, in order, off the runtime.
    desktop: desktop_worker::DesktopWorker,
    stop: Arc<AtomicBool>,
}

impl Wire {
    async fn run(self: &Arc<Self>) {
        Arc::clone(self).accept_loop().await;
    }

    /// Lets each attempt in and hands it to a task of its own at once: the
    /// TLS handshake, the pin check and the hello all run there, inside the
    /// handshake budget, never here. The loop owns those tasks: on stop it
    /// lets every session see the stop on its tick and clean up (unmount,
    /// release input, leave the registry) before the endpoint closes, and
    /// only a session that outlives [`STOP_WAIT`] is cut.
    async fn accept_loop(self: Arc<Self>) {
        let mut tasks = tokio::task::JoinSet::new();
        loop {
            let next = tokio::select! {
                pending = self.endpoint.accept() => pending,
                Some(_) = tasks.join_next(), if !tasks.is_empty() => continue,
                _ = self.stopped() => break,
            };
            let Some(pending) = next else { break };
            let wire = Arc::clone(&self);
            tasks.spawn(async move {
                let address = pending.remote_address();
                match pending.handshake().await {
                    Ok(incoming) => wire.admit(incoming).await,
                    Err(e) => log("link", &format!("{address}: handshake: {e}")),
                }
            });
        }
        if !drain(&mut tasks, STOP_WAIT).await {
            log("link", "a session did not end in time; cut");
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
        let Some(secret) = self.pairing.live_secret() else {
            log(
                "link",
                &format!("{address}: unpinned and no pairing armed; refused"),
            );
            incoming.refuse();
            return;
        };
        if self.pairing.refuses(address.ip()) {
            log(
                "link",
                &format!("{address}: refused for the rest of this window after wrong proofs"),
            );
            incoming.refuse();
            return;
        }
        let Ok(attempt) = Arc::clone(&self.pairing_attempts).try_acquire_owned() else {
            log(
                "link",
                &format!("{address}: too many pairing attempts at once; refused"),
            );
            incoming.refuse();
            return;
        };
        match self.endpoint.admit(incoming, Expect::Fingerprint(fp)).await {
            Ok((session, hello)) => {
                self.pair_then_run(session, hello, secret, fp, attempt)
                    .await;
            }
            Err(e) => log("link", &format!("{address}: pairing admit: {e}")),
        }
    }

    /// The desktop half of the QR path over a fresh session; on success the
    /// phone is pinned and the session continues as a trusted one. The
    /// window closes only once the proof verifies. `attempt` is the pairing
    /// attempt this connection holds; it is given back once the phone is
    /// pinned, so a paired session never keeps another phone from pairing.
    async fn pair_then_run(
        &self,
        session: Session,
        hello: Hello,
        secret: [u8; 32],
        fp: Fingerprint,
        attempt: tokio::sync::OwnedSemaphorePermit,
    ) {
        let identity = admission::pairing_id(&self.daemon.trust.lock_ok(), &fp, &hello.device_id);
        if let Err(e) = identity {
            log(
                "link",
                &format!("{}: pairing refused: {e}", hello.device_name),
            );
            session.close("identity");
            return;
        }
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
                self.pairing.failed(&secret, session.remote_address().ip());
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
        if !self.pairing.consume(&secret) {
            log(
                "link",
                &format!("{}: the pairing window closed first", hello.device_name),
            );
            session.close("window closed");
            return;
        }
        let pin = {
            let mut trust = self.daemon.trust.lock_ok();
            admission::pairing_id(&trust, &pinned.peer_fingerprint, &hello.device_id)
                .map_err(|e| e.to_string())
                .and_then(|device_id| {
                    trust
                        .pin(TrustedPeer {
                            device_id,
                            device_name: hello.device_name.clone(),
                            fingerprint: fingerprint_text(&pinned.peer_fingerprint),
                        })
                        .map_err(|e| format!("cannot persist the pin: {e}"))
                })
        };
        if let Err(e) = pin {
            log("link", &format!("{}: {e}", hello.device_name));
            session.close("cannot pin");
            return;
        }
        drop(attempt);
        // The session's writer starts with the session; this one reply has
        // the same deadline.
        let sent = tokio::time::timeout(
            writer::SEND_DEADLINE,
            session.send_message(capability::PAIRING, pair_kind::QR_REPLY, reply),
        )
        .await;
        if let Err(e) = sent
            .map_err(|_| "past its deadline".to_owned())
            .and_then(|r| r.map_err(|e| e.to_string()))
        {
            log(
                "link",
                &format!("{}: pairing reply: {e}", hello.device_name),
            );
            session.close("pairing reply");
            return;
        }
        ui_log(&self.daemon, &hello.device_name, "emparejado", false);
        self.run_session(session, hello, "paired").await;
    }

    async fn stopped(&self) {
        while !self.stop.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    /// One pinned session, from publication to cleanup. It runs under the id
    /// its certificate is pinned under; a hello naming another is refused.
    async fn run_session(&self, session: Session, hello: Hello, how: &str) {
        let pinned = admission::session_id(
            &self.daemon.trust.lock_ok(),
            &session.peer_fingerprint(),
            &hello.device_id,
        );
        let device_id = match pinned {
            Ok(id) => id,
            Err(e) => {
                log("link", &format!("{}: refused: {e}", hello.device_name));
                session.close("identity");
                return;
            }
        };
        // Shared with the reader tasks: `select!` drops a losing branch's
        // future, and a control-stream read dropped mid-frame desynchronises
        // the stream, so the reads live in tasks of their own and the loop
        // receives from channels, which is cancel-safe.
        let session = Arc::new(session);
        let daemon = &self.daemon;
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
            // The pin is checked again here, under the registry's lock: a
            // Forget that landed while this session waited for the earlier
            // one to leave removed it, and that session's teardown cleared
            // the revocation's tombstone, so nothing else would stop this
            // session from registering unpinned. The registry lock is taken
            // before the trust lock everywhere both are held.
            let still = admission::session_id(
                &daemon.trust.lock_ok(),
                &session.peer_fingerprint(),
                &hello.device_id,
            );
            if still.as_deref() != Ok(device_id.as_str()) {
                log(
                    "link",
                    &format!("{name}: forgotten while it waited; refused"),
                );
                session.close("forgotten");
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
        mirror::own().set_host(session.remote_address().ip());
        // Every send from here on is queued for the control stream's one
        // writer; nothing below waits on the phone's flow control.
        // What both hellos offer is all this session uses: the outbox refuses
        // the rest on the way out and the loop below on the way in (MAG-8).
        let negotiated = Arc::new(Negotiated::between(&offered(), &hello.capabilities));
        let agreed: Vec<u16> = negotiated
            .capabilities()
            .iter()
            .map(|c| c.capability)
            .collect();
        log(
            "link",
            &format!("{name}: negotiated capabilities {agreed:?}"),
        );
        let (outbox, writer) =
            writer::spawn(Arc::clone(&session), name.clone(), Arc::clone(&negotiated));
        let queue = |env: magnetita_proto::Envelope, what: &str| {
            if let Err(e) = outbox.send(env) {
                log("link", &format!("{name}: {what}: {e}"));
            }
        };
        let allows = |capability| negotiated.allows(capability);
        if daemon.settings.lock_ok().clipboard && allows(capability::CLIPBOARD) {
            // The phone answers only while its application is in front.
            queue(
                magnetita_proto::Envelope {
                    capability: capability::CLIPBOARD,
                    kind: ClipboardRequest::KIND,
                    id: 0,
                    body: ClipboardRequest.encode(),
                },
                "clipboard request",
            );
        }

        let shares = share::SessionShare::new(
            Arc::clone(daemon),
            &device_id,
            &name,
            session.transfers(),
            Arc::clone(&self.adapters.shares),
            self.adapters.download_dir.clone(),
            outbox.clone(),
        );
        let storage_client = storage::StorageClient::new(outbox.clone());
        // Closes the client and takes it out of the registry however the
        // session ends, cut by the stop's drain included.
        let _registered_client = storage::register(&device_id, &storage_client);
        // The phone's files, mounted while it shares a root; dropping the
        // session unmounts, so a lost link never strands the directory.
        let mut phone_mount: Option<fuser::BackgroundSession> = None;
        // Dropped with the session, releasing what the phone still holds.
        let input = input::SessionInput::new(Arc::clone(&self.adapters.input));
        let media = Arc::new(media::SessionMedia::default());
        if daemon.settings.lock_ok().media && allows(capability::MEDIA) {
            queue(media::SessionMedia::request(), "media request");
        }
        {
            let settings = *daemon.settings.lock_ok();
            if settings.contacts && allows(capability::CONTACTS) {
                let since = phone::store().with(&device_id, |book| book.contacts_version);
                queue(phone::contacts_request(since), "phone request");
            }
            if settings.sms && allows(capability::SMS) {
                queue(phone::conversations_request(), "phone request");
            }
            if settings.commands && allows(capability::COMMANDS) {
                queue(commands::store().published(), "phone request");
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
                    match transfers.accept_or_skip().await {
                        Ok(magnetita_link::Accepted::Stream(id, stream)) => {
                            if streams_tx.send((id, stream)).is_err() {
                                break;
                            }
                        }
                        // One stream the phone reset or ended early; the next ones still come.
                        Ok(magnetita_link::Accepted::Skipped(why)) => {
                            log("link", &format!("{name}: a bulk stream was skipped: {why}"));
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
        // Set when the daemon stops: the session ends in order (see below)
        // instead of at once.
        let mut stopping = false;
        // A refused capability is logged once per session, not per message.
        let mut refused_logged = std::collections::HashSet::new();
        let mut refused = |capability: u16, what: &str| {
            if refused_logged.insert(capability) {
                log("link", &format!("{name}: refused {what} of capability {capability}: not negotiated; further ones are dropped quietly"));
            }
        };
        loop {
            tokio::select! {
                Some(envelope) = control_rx.recv() => match envelope {
                    Ok(env) if !allows(env.capability) => refused(env.capability, "a message"),
                    Ok(env) if env.capability == capability::PAIRING && env.kind == pair_kind::QR_PROOF => {
                        self.prove_again(&session, &outbox, &name, &env.body);
                    }
                    Ok(env) if env.capability == capability::MEDIA => {
                        self.handle_media(&device_id, &name, &media, &env);
                    }
                    Ok(env) if env.capability == capability::MIRROR && mirror::own().owned_by(&device_id) => match env.kind {
                        MirrorStarted::KIND => match MirrorStarted::decode(&env.body) {
                            Ok(started) => {
                                mirror::own().started(self.adapters.mirror_player.as_ref(), &started);
                                log("mirror", &format!("{name}: streaming {}x{} audio={}", started.width, started.height, started.audio));
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
                                // An unmount waits for the file system's
                                // thread, which may be inside a request to
                                // the phone; the loop does not wait with it.
                                drop(tokio::task::spawn_blocking(move || drop(mounted)));
                                daemon.set_mount(&device_id, None);
                                daemon.notify_change();
                            }
                        }
                    }
                    Ok(env) if env.capability == capability::STORAGE => storage_client.reply(env),
                    Ok(env) if env.capability == capability::INPUT => {
                        self.handle_input(&name, &input, env.kind, &env.body);
                    }
                    Ok(env) if env.capability == capability::COMMANDS && env.kind == CommandRun::KIND => {
                        if self.daemon.settings.lock_ok().commands {
                            if let Ok(run) = CommandRun::decode(&env.body) {
                                let outbox = outbox.clone();
                                let stop = Arc::clone(&self.stop);
                                let who = name.clone();
                                tokio::task::spawn_blocking(move || {
                                    let result = commands::store().run(run.id, &stop);
                                    log("link", &format!("{who}: command {} {}", run.id, if result.ok { "ok" } else { "failed" }));
                                    if let Err(e) = outbox.send(magnetita_proto::Envelope {
                                        capability: capability::COMMANDS,
                                        kind: CommandResult::KIND,
                                        id: 0,
                                        body: result.encode(),
                                    }) {
                                        log("link", &format!("{who}: command result: {e}"));
                                    }
                                });
                            }
                        }
                    }
                    Ok(env) if env.capability == capability::SHARE => {
                        if let Some(reply) = shares.handle(&env) {
                            queue(reply, "share");
                        }
                    }
                    Ok(env) => self.handle(&device_id, &name, env),
                    Err(e) => {
                        log("link", &format!("{name}: {e}"));
                        break;
                    }
                },
                Some((transfer, stream)) = streams_rx.recv() => match transfer {
                    mirror::VIDEO_STREAM if allows(capability::MIRROR) && mirror::own().owned_by(&device_id) => mirror::own().video_stream(stream),
                    mirror::AUDIO_STREAM if allows(capability::MIRROR) && mirror::own().owned_by(&device_id) => mirror::own().audio_stream(stream),
                    mirror::VIDEO_STREAM | mirror::AUDIO_STREAM => {}
                    _ if allows(capability::SHARE) => shares.stream_arrived(transfer, stream),
                    _ => refused(capability::SHARE, "a bulk stream"),
                },
                // Motion may arrive as datagrams: same body, no reliability.
                Some(datagram) = datagram_rx.recv() => match datagram {
                    Ok(env) if env.capability == capability::INPUT && allows(capability::INPUT) => {
                        self.handle_input(&name, &input, env.kind, &env.body);
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log("link", &format!("{name}: {e}"));
                        break;
                    }
                },
                _ = tick.tick() => {
                    shares.expire(Instant::now());
                    for env in mirror::own().tick(&device_id, &outbox) {
                        queue(env, "mirror");
                    }
                    media.set_active(daemon.settings.lock_ok().media && allows(capability::MEDIA));
                    for state in media.tick() {
                        queue(state, "media");
                    }
                    // The desktop's clipboard changes land in the shared slot
                    // for every device; this wire drains its own entry here.
                    let pending = daemon.pending_clipboards.take(&device_id);
                    if let Some(text) = pending.filter(|_| allows(capability::CLIPBOARD)) {
                        queue(
                            magnetita_proto::Envelope {
                                capability: capability::CLIPBOARD,
                                kind: ClipboardText::KIND,
                                id: 0,
                                body: ClipboardText { text }.encode(),
                            },
                            "clipboard",
                        );
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
                        self.command(&outbox, &shares, &media, &device_id, &name, command);
                    }
                    if superseded {
                        session.close("superseded");
                        log("link", &format!("{name}: superseded by a newer session"));
                        break;
                    }
                    if self.stop.load(Ordering::Relaxed) {
                        stopping = true;
                        break;
                    }
                }
            }
        }
        // The phone's files first: the requests in flight fail now instead of
        // at their timeout, and the unmount comes before the link's runtime
        // can stop under the file system's thread.
        storage_client.close();
        storage::unregister(&device_id, &storage_client);
        if let Some(mounted) = phone_mount.take() {
            let _ = tokio::task::spawn_blocking(move || drop(mounted)).await;
            daemon.set_mount(&device_id, None);
            daemon.notify_change();
        }
        if stopping {
            // What is already queued for the phone (a share's end, a
            // command's result) reaches it before the connection closes:
            // written by the writer, then the control stream finished and
            // its receipt acknowledged by the phone. A close alone lets the
            // phone drop what it had not yet received.
            let delivered = outbox.flush(STOP_FLUSH).await
                && matches!(
                    tokio::time::timeout(STOP_FINISH, session.finish_control()).await,
                    Ok(Ok(()))
                );
            if !delivered {
                log(
                    "link",
                    &format!("{name}: stopping before the phone received every send"),
                );
            }
            session.close("daemon stopping");
        }
        writer.abort();
        acceptor.abort();
        control_reader.abort();
        datagram_reader.abort();
        drop(input);
        if mirror::own().owned_by(&device_id) {
            mirror::own().stopped();
        }
        phone::store().forget_device(&device_id);
        daemon.pending_clipboards.clear(&device_id);
        let notifications = Arc::clone(&self.adapters.notifications);
        let gone = device_id.clone();
        self.desktop.run("notification cleanup", move || {
            notifications.forget_device(&gone)
        });
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
    /// session's governor, decoded at this boundary, and handed to the sink;
    /// a release of a held key or button always passes.
    fn handle_input(&self, name: &str, input: &input::SessionInput, kind: u16, body: &[u8]) {
        let enabled = self.daemon.settings.lock_ok().input;
        if let Err(e) = input.apply(enabled, kind, body) {
            log("link", &format!("{name}: input: {e}"));
        }
    }

    /// Shows or replaces one phone notification on the desktop worker.
    fn post_notification(&self, device_id: &str, name: &str, note: NotificationPosted) {
        let bridge = Arc::clone(&self.adapters.notifications);
        let server = Arc::clone(&self.adapters.notification_server);
        let daemon = Arc::clone(&self.daemon);
        let (device_id, name) = (device_id.to_owned(), name.to_owned());
        self.desktop.run("notification", move || {
            if let Some(line) = bridge.posted(server.as_ref(), &device_id, &name, &note) {
                ui_log(&daemon, &name, &line, false);
            }
        });
    }

    /// Withdraws one phone notification on the desktop worker.
    fn withdraw_notification(&self, device_id: &str, key: &str) {
        let bridge = Arc::clone(&self.adapters.notifications);
        let server = Arc::clone(&self.adapters.notification_server);
        let (device_id, key) = (device_id.to_owned(), key.to_owned());
        self.desktop.run("notification withdrawal", move || {
            bridge.dismissed(server.as_ref(), &device_id, &key)
        });
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
                        store.with(device_id, |book| book.set_conversations(list.conversations));
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
                            self.post_notification(device_id, name, note);
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
        match event.state {
            CallState::Ended => self.withdraw_notification(device_id, phone::CALL_KEY),
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
                self.post_notification(device_id, name, note);
            }
        }
    }

    /// A phone that forgot this desktop while the desktop still pins it is
    /// admitted as trusted, yet it sends a QR proof: answer it from the armed
    /// window so the phone can pin again, and keep the session. As for a
    /// first pairing, only a proof that verifies closes the window.
    fn prove_again(&self, session: &Session, outbox: &Outbox, name: &str, proof: &[u8]) {
        let Some(secret) = self.pairing.live_secret() else {
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
                self.pairing.failed(&secret, session.remote_address().ip());
                log("link", &format!("{name}: pairing again refused: {e}"));
                return;
            }
        };
        if !self.pairing.consume(&secret) {
            log("link", &format!("{name}: the pairing window closed first"));
            return;
        }
        match outbox.send(magnetita_proto::Envelope {
            capability: capability::PAIRING,
            kind: pair_kind::QR_REPLY,
            id: 0,
            body: reply,
        }) {
            Ok(()) => ui_log(&self.daemon, name, "emparejado de nuevo", false),
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
                    Ok(clip) if crate::clipboard::syncable(&clip.text) => {
                        // Record before writing so the watcher does not echo it back.
                        *self.daemon.last_clipboard.lock_ok() = clip.text.clone();
                        let sink = Arc::clone(&self.adapters.clipboard_sink);
                        let daemon = Arc::clone(&self.daemon);
                        let name = name.to_owned();
                        self.desktop.run("clipboard", move || {
                            if sink(&clip.text) {
                                ui_log(&daemon, &name, "portapapeles recibido", false);
                            }
                        });
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
                    Ok(note) => self.post_notification(device_id, name, note),
                    Err(e) => log("link", &format!("{name}: notification: {e}")),
                }
            }
            (capability::NOTIFICATIONS, NotificationDismissed::KIND) => {
                match NotificationDismissed::decode(&env.body) {
                    Ok(gone) => self.withdraw_notification(device_id, &gone.key),
                    Err(e) => log("link", &format!("{name}: notification: {e}")),
                }
            }
            (cap, kind) => log(
                "link",
                &format!("{name}: unhandled capability {cap} kind {kind}"),
            ),
        }
    }

    /// One action from `Devices1` for the phone, queued for the writer.
    fn command(
        &self,
        outbox: &Outbox,
        shares: &Arc<share::SessionShare>,
        media: &Arc<media::SessionMedia>,
        device_id: &str,
        name: &str,
        command: Command,
    ) {
        let env = match command {
            Command::Superseded => None,
            Command::CommandsChanged => Some(commands::store().published()),
            Command::SmsList => Some(phone::conversations_request()),
            Command::SmsThread { thread, before_ms } => {
                Some(phone::thread_request(thread, before_ms))
            }
            Command::SmsSend { thread, body } => Some(phone::sms_send(thread, &body)),
            Command::CallAction(action) => Some(phone::call_command(action)),
            // A reply on an SMS notification is a send in that thread; a
            // button on the call notification is a call action.
            Command::NotificationReply { key, text } if key.starts_with(phone::SMS_KEY_PREFIX) => {
                key[phone::SMS_KEY_PREFIX.len()..]
                    .parse::<u64>()
                    .ok()
                    .map(|thread| phone::sms_send(thread, &text))
            }
            Command::NotificationAction { key, action } if key == phone::CALL_KEY => {
                let state =
                    phone::store().with(device_id, |book| book.call.as_ref().map(|c| c.state));
                state.and_then(|state| {
                    phone::call_buttons(state)
                        .get(usize::from(action))
                        .map(|(_, call_action)| phone::call_command(*call_action))
                })
            }
            Command::NotificationDismiss { key }
                if key == phone::CALL_KEY || key.starts_with(phone::SMS_KEY_PREFIX) =>
            {
                None
            }
            Command::Media(action) => media.command_for_phone(action),
            Command::SendFile(path) => match shares.offer(path) {
                Ok(offer) => Some(offer),
                Err(e) => {
                    log("link", &format!("share: {e}"));
                    None
                }
            },
            Command::Ring => Some(envelope(
                capability::FIND,
                FindRing::KIND,
                FindRing.encode(),
            )),
            Command::NotificationAction { key, action } => Some(envelope(
                capability::NOTIFICATIONS,
                NotificationAction::KIND,
                NotificationAction { key, action }.encode(),
            )),
            Command::NotificationReply { key, text } => Some(envelope(
                capability::NOTIFICATIONS,
                NotificationReply::KIND,
                NotificationReply { key, text }.encode(),
            )),
            Command::NotificationDismiss { key } => Some(envelope(
                capability::NOTIFICATIONS,
                NotificationDismissed::KIND,
                NotificationDismissed { key }.encode(),
            )),
        };
        if let Some(env) = env {
            if let Err(e) = outbox.send(env) {
                log("link", &format!("{name}: command: {e}"));
            }
        }
    }
}

/// An envelope for the writer, which numbers it.
fn envelope(capability: u16, kind: u16, body: Vec<u8>) -> magnetita_proto::Envelope {
    magnetita_proto::Envelope {
        capability,
        kind,
        id: 0,
        body,
    }
}

#[cfg(test)]
mod admission_tests;

#[cfg(test)]
mod session_tests;

#[cfg(test)]
mod negotiation_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{Commands, Log, Registry};
    use crate::revocation::Revocations;
    use crate::settings::Settings;
    use magnetita_link::endpoint::Expect as PeerExpect;
    use magnetita_proto::pair::QrPayload;
    use std::collections::{BTreeMap, HashMap, VecDeque};

    pub(super) fn test_daemon(_cert: &DeviceCert) -> Arc<Daemon> {
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
            signals: crate::signals::Signals::on_bus(None),
            notifications: Default::default(),
            last_clipboard: Mutex::new(String::new()),
        })
    }

    /// A phone that offers every capability this daemon does.
    pub(super) fn phone(name: &str) -> (Endpoint, Fingerprint) {
        phone_offering(name, offered())
    }

    /// A phone whose hello offers exactly `capabilities`.
    pub(super) fn phone_offering(
        name: &str,
        capabilities: Vec<CapabilityVersion>,
    ) -> (Endpoint, Fingerprint) {
        let cert = DeviceCert::generate(name);
        let fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let hello = Hello {
            // The id a phone gives itself: its certificate's.
            device_id: magnetita_link::device_id_of(&fp),
            device_name: name.into(),
            device_kind: DeviceKind::Phone,
            capabilities,
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

    /// The phone half of the QR path: connect to the QR's address, prove, accept the reply.
    pub(super) async fn prove(
        phone: &Endpoint,
        phone_fp: Fingerprint,
        uri: &str,
    ) -> (Session, Fingerprint) {
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
        let id = magnetita_link::device_id_of(&phone_fp);
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
            .replace_for([id.clone()], "copied on the desk".into());
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
    fn an_offer_larger_than_any_payload_is_refused_before_a_byte() {
        use magnetita_proto::daily::share::{ShareOffer, ShareReject};
        let cert = DeviceCert::generate("desktop");
        let daemon = test_daemon(&cert);
        let arm = PairingArm::default();
        let downloads =
            std::env::temp_dir().join(format!("magnetita-share-cap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&downloads);
        let (wire, addr) = spawn(
            Arc::clone(&daemon),
            cert.clone(),
            "desktop".into(),
            arm.clone(),
            "127.0.0.1:0".parse().unwrap(),
            false,
            Adapters {
                download_dir: downloads.clone(),
                ..test_adapters()
            },
        )
        .unwrap();
        let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (phone, phone_fp) = rt.block_on(async { phone("share-cap") });
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));

        let offer = ShareOffer {
            transfer: 3,
            name: "endless.bin".into(),
            size: magnetita_net::MAX_PAYLOAD_SIZE as u64 + 1,
            mime: String::new(),
        };
        rt.block_on(session.send_message(capability::SHARE, ShareOffer::KIND, offer.encode()))
            .unwrap();
        let answer = rt.block_on(async {
            loop {
                let env = tokio::time::timeout(Duration::from_secs(5), session.recv())
                    .await
                    .expect("an answer in time")
                    .unwrap();
                if env.capability == capability::SHARE {
                    break env;
                }
            }
        });
        assert_eq!(answer.kind, ShareReject::KIND, "the offer is refused");
        assert_eq!(ShareReject::decode(&answer.body).unwrap().transfer, 3);
        let partials = std::fs::read_dir(&downloads).map_or(0, |dir| dir.count());
        assert_eq!(partials, 0, "no partial is created for it");
        assert!(
            daemon.payloads.try_acquire().is_some(),
            "no payload permit is held for it"
        );
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
        let id = magnetita_link::device_id_of(&phone_fp);
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
        while daemon.devices.lock_ok().contains_key(&id) {
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
            .get(&id)
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
        let id = magnetita_link::device_id_of(&phone_fp);
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
            let entry = daemon.devices.lock_ok().get(&id).cloned();
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
            .get(&id)
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
        let id = magnetita_link::device_id_of(&phone_fp);
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
        assert_eq!(phone::store().conversations(&id).len(), 1);

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
            let entry = daemon.devices.lock_ok().get(&id).cloned();
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
        assert_eq!(daemon.devices.lock_ok().get(&id).unwrap().call_state, "");
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
        let id = magnetita_link::device_id_of(&phone_fp);
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
        let command = commands::store()
            .set(0, "Loopback true", "true", vec![])
            .unwrap();
        daemon
            .commands
            .lock_ok()
            .get(&id)
            .unwrap()
            .try_send(Command::CommandsChanged)
            .unwrap();
        let published = next_of(capability::COMMANDS, CommandList::KIND);
        let list = CommandList::decode(&published.body).unwrap();
        assert_eq!(list.commands.len(), before + 1);
        assert!(list
            .commands
            .iter()
            .any(|c| c.id == command && c.name == "Loopback true"));

        rt.block_on(session.send_message(
            capability::COMMANDS,
            CommandRun::KIND,
            CommandRun { id: command }.encode(),
        ))
        .unwrap();
        let result =
            CommandResult::decode(&next_of(capability::COMMANDS, CommandResult::KIND).body)
                .unwrap();
        assert!(result.ok && result.id == command);
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
        commands::store().remove(command).unwrap();

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
        let _inbox = crate::mirror::TEST_INBOX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let id = magnetita_link::device_id_of(&phone_fp);
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
            &id,
            MirrorStart {
                max_size: 1440,
                fps: 60,
                bitrate_kbps: 6000,
                codec: Codec::Hevc,
                audio: false,
                screen_off: false,
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

    pub(super) fn test_adapters() -> Adapters {
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
        let id = magnetita_link::device_id_of(&phone_fp);
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));
        let session = Arc::new(session);
        let root = storage_root("storage");
        let server = serve_storage(&rt, Arc::clone(&session), root.clone(), false);

        let deadline = Instant::now() + Duration::from_secs(5);
        let client = loop {
            if let Some(c) = storage::clients().lock_ok().get(&id).cloned() {
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
            // A directory with entries is not deleted: the phone answers
            // that it is not empty, and nothing under it is lost.
            assert_eq!(
                client.delete("DCIM").await,
                Err(magnetita_proto::storage::ERROR_NOT_EMPTY.into())
            );
            assert!(root.join("DCIM/one.jpg").exists());
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
        // The runtime directory the spec requires, or the daemon mounts nothing.
        std::fs::set_permissions(
            &runtime_dir,
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .unwrap();
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
        let id = magnetita_link::device_id_of(&phone_fp);
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
                .get(&id)
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
        while daemon.devices.lock_ok().get(&id).is_some_and(|d| d.mounted) {
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
        let id = magnetita_link::device_id_of(&phone_fp);
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (first, _) = rt.block_on(prove(&phone, phone_fp, &uri));
        let deadline = Instant::now() + Duration::from_secs(5);
        while !daemon.devices.lock_ok().contains_key(&id) {
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
                .get(&id)
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
        let id = magnetita_link::device_id_of(&phone_fp);

        // First pairing, then the phone drops its session (and, in life, its pin).
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (first, fp) = rt.block_on(prove(&phone, phone_fp, &uri));
        assert_eq!(fp, desktop_fp);
        first.close("phone forgets");
        let deadline = Instant::now() + Duration::from_secs(5);
        while daemon.devices.lock_ok().contains_key(&id) {
            assert!(Instant::now() < deadline, "the first session never left");
            thread::sleep(Duration::from_millis(50));
        }
        assert!(
            daemon.trust.lock_ok().is_trusted(&id),
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
            let entry = daemon.devices.lock_ok().get(&id).cloned();
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
        let id = magnetita_link::device_id_of(&phone_fp);
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
            let entry = daemon.devices.lock_ok().get(&id).cloned();
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
        assert!(daemon.trust.lock_ok().is_trusted(&id));

        // A Ring command reaches the phone as a FindRing envelope.
        daemon
            .commands
            .lock_ok()
            .get(&id)
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
            .request_if_and_apply(&id, || true, || daemon.trust.lock_ok().forget(&id))
            .unwrap();
        let closed = rt
            .block_on(async { tokio::time::timeout(Duration::from_secs(5), session.recv()).await });
        assert!(
            matches!(closed, Ok(Err(_))),
            "the daemon closes the session on Forget"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while daemon.devices.lock_ok().contains_key(&id) {
            assert!(
                Instant::now() < deadline,
                "the entry never left the registry"
            );
            thread::sleep(Duration::from_millis(50));
        }
        assert!(!daemon.trust.lock_ok().is_trusted(&id));

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
