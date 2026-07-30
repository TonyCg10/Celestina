//! `magnetitad` — Magnetita's CP0 daemon: the trusted channel to the phone.
//!
//! This is the headless proof of the whole stack below it. It makes (once) our
//! device id and certificate, then keeps a phone reachable three ways at the
//! same time, the way KDE Connect peers do:
//!
//! - **announce** our identity over UDP, so the phone lists and can find us;
//! - **accept** TCP connections the phone opens to us — this is how a *paired*
//!   phone reconnects when its app comes back, so it is what makes "connected"
//!   stick rather than flicker;
//! - **dial** a phone we hear announce, so a fresh device links without waiting.
//!
//! Every path runs the same KDE Connect v8 handshake and session. Unknown
//! phones stay pending until the local UI accepts the request; trusted peers
//! reconnect with their pinned certificate. Only one link survives per device.
//!
//! The UI and `org.celestina.Devices1` use this same trusted link.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::error::Error;
use std::fs;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use celestina_core::{xdg, Generation, GenerationClock};
use magnetita_core::{
    read_album_art, read_battery, read_clipboard, read_mpris, read_mpris_request,
    read_notification, read_sftp, read_share, request_packet, ConnectionEvent, Identity,
    LostReason, Notification, Session, SftpReply,
};
use magnetita_net::discovery::ANNOUNCE_INTERVAL;
use magnetita_net::{
    Announcement, Device, DeviceCert, Discovery, Link, PayloadLimiter, TlsConfigs, TrustCheck,
    TrustStore, TrustedPeer,
};

mod admission;
mod artwork;
mod clipboard;
mod device_identity;
mod devices;
mod incoming_file;
mod link_commands;
mod lock;
mod media;
mod mount;
mod notify;
mod payload_handlers;
mod remote_media;
mod revocation;
mod runtime;
mod session_registration;
mod settings;
use admission::{Admission, Permit};
use devices::{
    command_channel, push_log, set_verification_key, Command, Commands, DeviceEntry, Devices, Log,
    LogEntry, Registry,
};
use lock::LockOk;
use mount::Mount;
use remote_media::{RemoteMedia, Report as MediaReport};
use revocation::Revocations;
use runtime::{event_line, id_source, is_disconnect, log, log_event, millis, type_label};
use settings::Settings;

/// The KDE Connect port: UDP announce/listen and TCP link.
const PORT: u16 = 1716;

/// How long the TCP+TLS handshake may take before we give up on a link.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// How often [`Device::pump`] wakes to check the pairing clock while idle.
const TICK: Duration = Duration::from_secs(1);

/// An unknown peer must pair or leave; idle LAN clients cannot live forever.
const UNTRUSTED_LINK_TIMEOUT: Duration = Duration::from_secs(60);

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            log("error", &e.to_string());
            std::process::ExitCode::FAILURE
        }
    }
}

/// The shared state every link thread reads: who we are, how to wrap TLS, the
/// trust store, the connected-device registry the contract exposes, and the
/// session-bus connection used to announce changes.
struct Daemon {
    identity: Identity,
    tls: TlsConfigs,
    /// Shared with the served `Devices1` object so its Settings surface can list
    /// and forget paired peers, not only the link threads that pin them.
    trust: Arc<Mutex<TrustStore>>,
    /// The per-plugin toggles, shared the same way: the app writes them (through
    /// the served object, which owns persistence), the link threads read them to
    /// gate each plugin's behaviour.
    settings: Arc<Mutex<Settings>>,
    devices: Registry,
    log: Log,
    commands: Commands,
    pending_clipboards: link_commands::PendingClipboards,
    artwork_completions: link_commands::PendingArtworkCompletions,
    revocations: Arc<Revocations>,
    generation_clock: Mutex<GenerationClock>,
    admission: Arc<Admission>,
    payloads: PayloadLimiter,
    dbus: Option<zbus::blocking::Connection>,
    /// phone-notification-id → freedesktop-server-id, so an update replaces and a
    /// cancel withdraws the right desktop notification.
    notifications: Mutex<HashMap<String, u32>>,
    /// The last clipboard value we synced (sent or received), so our own
    /// wl-copy of a received clipboard is not echoed back and no loop forms.
    last_clipboard: Mutex<String>,
}

fn run() -> Result<(), Box<dyn Error>> {
    let dir = xdg::config_home()
        .ok_or("no XDG config home to store the device identity")?
        .join("magnetita");
    fs::create_dir_all(&dir)?;
    if let Err(error) = artwork::sweep() {
        log("artwork", &format!("cache unavailable: {error}"));
    }

    let device_id = device_identity::ensure(&dir)?;
    let cert = DeviceCert::ensure(&dir, &device_id)?;

    // The trust store and plugin settings are shared with the served interface,
    // so the app can list/forget paired peers and toggle plugins, not just the
    // link threads. Built before serving so both sides hold the same handle.
    let trust = Arc::new(Mutex::new(TrustStore::load(&dir.join("trust.json"))?));
    let settings_path = dir.join("settings.json");
    let settings = Arc::new(Mutex::new(Settings::load(&settings_path)));

    // Serve org.celestina.Devices1 (best-effort: no session bus just means
    // Siderita cannot draw the phone, not that the link fails).
    let registry: Registry = Arc::new(Mutex::new(BTreeMap::new()));
    let event_log: Log = Arc::new(Mutex::new(VecDeque::new()));
    let commands: Commands = Arc::new(Mutex::new(HashMap::new()));
    let revocations = Arc::new(Revocations::new());
    let dbus = match serve_devices(
        Arc::clone(&registry),
        Arc::clone(&event_log),
        Arc::clone(&commands),
        Arc::clone(&trust),
        Arc::clone(&revocations),
        Arc::clone(&settings),
        settings_path,
    ) {
        Ok(connection) => {
            log("dbus", &format!("serving {}", devices::INTERFACE));
            Some(connection)
        }
        Err(e) => {
            log("dbus", &format!("unavailable: {e}"));
            None
        }
    };

    let daemon = Arc::new(Daemon {
        identity: Identity::desktop(&device_id, "Celestina"),
        tls: TlsConfigs::build(&cert)?,
        trust,
        settings,
        devices: registry,
        log: event_log,
        commands,
        pending_clipboards: link_commands::PendingClipboards::default(),
        artwork_completions: link_commands::PendingArtworkCompletions::default(),
        revocations,
        generation_clock: Mutex::new(GenerationClock::default()),
        admission: Arc::new(Admission::new()),
        payloads: PayloadLimiter::new(),
        dbus,
        notifications: Mutex::new(HashMap::new()),
        last_clipboard: Mutex::new(String::new()),
    });

    // Watch the desktop clipboard and push changes to connected phones.
    let clipboard_daemon = Arc::clone(&daemon);
    clipboard::spawn_watch(move |text| clipboard_daemon.push_clipboard(text));

    log("id", &device_id);
    log("cert", &cert.fingerprint()?);

    // A previous run killed mid-mount can leave a dead sshfs behind; clear it.
    mount::clear_stale();

    // UDP: announce ourselves, and hear phones announce.
    let discovery = Discovery::bind(SocketAddr::from(([0, 0, 0, 0], PORT)), &device_id)
        .map_err(|e| format!("cannot bind UDP {PORT} ({e}) — is Valent or kdeconnectd running?"))?;
    spawn_announcer(&discovery, Arc::clone(&daemon));

    // TCP: accept the connections a paired phone opens to reconnect.
    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], PORT)))
        .map_err(|e| format!("cannot bind TCP {PORT} ({e}) — is Valent or kdeconnectd running?"))?;
    spawn_accepter(listener, Arc::clone(&daemon));

    // Hear + dial (this thread): a phone we hear and are not linked to, we dial.
    log(
        "ready",
        "listening for the phone — keep KDE Connect open on it",
    );
    loop {
        let announcement = match discovery.recv() {
            Ok(Some(a)) => a,
            Ok(None) => continue,
            Err(e) => {
                log("discovery", &format!("recv error: {e}"));
                continue;
            }
        };
        if announcement.link_addr().is_none() {
            continue;
        }
        if daemon
            .devices
            .lock_ok()
            .contains_key(&announcement.identity.device_id)
        {
            continue; // already linked by one path or the other
        }
        let device_id = &announcement.identity.device_id;
        if !daemon.admission.allow_dial(device_id, Instant::now()) {
            continue;
        }
        let Some(address) = announcement.link_addr() else {
            continue;
        };
        let Some(permit) = daemon.admission.try_acquire(address.ip()) else {
            continue;
        };
        spawn_dialer(Arc::clone(&daemon), announcement, permit);
    }
}

/// Re-broadcast our identity so a phone that opens its app finds us — but only
/// while nothing is linked. A phone treats each announce as "this device is
/// (re)available" and opens a fresh connection, dropping the old one; announcing
/// on a timer while already connected drives it into a reconnect loop. The TCP
/// link is the liveness, not the broadcast, so we go quiet once connected and
/// speak up again only if the link drops.
fn spawn_announcer(discovery: &Discovery, daemon: Arc<Daemon>) {
    if let Ok(announcer) = discovery.try_clone() {
        thread::spawn(move || loop {
            if !daemon
                .devices
                .lock_ok()
                .values()
                .any(|device| device.paired)
            {
                let _ = announcer.announce(&daemon.identity, millis());
            }
            thread::sleep(ANNOUNCE_INTERVAL);
        });
    }
}

/// Accept incoming TCP links (the phone dialing us) and serve each.
fn spawn_accepter(listener: TcpListener, daemon: Arc<Daemon>) {
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(tcp) = stream else { continue };
            let Ok(address) = tcp.peer_addr() else {
                continue;
            };
            let Some(permit) = daemon.admission.try_acquire(address.ip()) else {
                continue;
            };
            let daemon = Arc::clone(&daemon);
            thread::spawn(move || match accept_link(&daemon, tcp) {
                Ok(link) => daemon.serve(link, "accepted", permit),
                Err(e) => {
                    log("accept", &format!("handshake failed: {e}"));
                    ui_log(
                        &daemon,
                        "un dispositivo",
                        &format!("un intento de conexión falló: {e}"),
                        true,
                    );
                }
            });
        }
    });
}

/// Dial a heard phone and serve the link, off the discovery thread.
fn spawn_dialer(daemon: Arc<Daemon>, announcement: Announcement, permit: Permit) {
    thread::spawn(move || {
        let mut next_id = id_source();
        match Link::connect(
            &announcement,
            &daemon.identity,
            &daemon.tls,
            &mut next_id,
            HANDSHAKE_TIMEOUT,
        ) {
            Ok(link) => daemon.serve(link, "dialed", permit),
            Err(e) => {
                let name = &announcement.identity.device_name;
                log("dial", &format!("{name}: {e}"));
                // A dial that loses the race to the accept path — the phone dialed
                // us first, so it ignores our dial and the handshake times out — is
                // expected once we are connected, not a failure to surface.
                let connected = daemon
                    .devices
                    .lock_ok()
                    .contains_key(&announcement.identity.device_id);
                if !connected {
                    ui_log(&daemon, name, &format!("no se pudo conectar: {e}"), true);
                }
            }
        }
    });
}

fn accept_link(daemon: &Daemon, tcp: TcpStream) -> Result<Link, magnetita_net::LinkError> {
    let mut next_id = id_source();
    Link::accept(
        tcp,
        &daemon.identity,
        &daemon.tls,
        &mut next_id,
        HANDSHAKE_TIMEOUT,
    )
}

impl Daemon {
    /// Take ownership of a freshly-formed link: keep only one per device, then
    /// run its session until it closes.
    fn serve(self: &Arc<Self>, link: Link, how: &'static str, permit: Permit) {
        let peer_id = link.peer().device_id.clone();
        let peer_name = link.peer().device_name.clone();
        let peer_type = type_label(link.peer().device_type);
        let fingerprint = link.peer_fingerprint().to_owned();

        let (sender, commands) = command_channel();
        {
            // Hold the lock across check-and-insert so two paths cannot both
            // pass; one link per device, the loser dropped here.
            let mut devices = self.devices.lock_ok();
            if devices.contains_key(&peer_id) {
                return;
            }
            // Install command delivery before publishing `connected`; Forget
            // can never observe a live entry whose Unpair command would vanish.
            self.commands.lock_ok().insert(peer_id.clone(), sender);
            devices.insert(
                peer_id.clone(),
                DeviceEntry::connected(peer_id.clone(), peer_name.clone(), peer_type, fingerprint),
            );
        }
        let _registration = session_registration::SessionRegistration::new(
            Arc::clone(self),
            peer_id.clone(),
            peer_name.clone(),
        );
        self.notify_change();
        log(how, &format!("{peer_name} at {}", link.peer_addr()));
        log(
            "secured",
            &format!("fingerprint {}", link.peer_fingerprint()),
        );
        ui_log(self, &peer_name, "conectado y cifrado", false);

        if let Err(e) = self.run_link(link, &peer_id, &peer_name, commands, permit) {
            let message = e.to_string();
            log("link", &format!("{peer_name}: {message}"));
            // A reset / broken pipe / EOF is the phone dropping the link — a
            // disconnect, which the "desconectado" line below already reports.
            // Only a genuinely unexpected error is worth the red banner.
            if !is_disconnect(&message) {
                ui_log(
                    self,
                    &peer_name,
                    &format!("error de enlace: {message}"),
                    true,
                );
            }
        }
    }

    /// Trust-check one link, expose its pairing request for explicit local
    /// acceptance, persist trust and — once trusted — mount its storage.
    fn run_link(
        self: &Arc<Self>,
        link: Link,
        peer_id: &str,
        peer_name: &str,
        commands: mpsc::Receiver<Command>,
        mut admission: Permit,
    ) -> Result<(), Box<dyn Error>> {
        let peer_fp = link.peer_fingerprint().to_owned();
        let link_host = link.peer_addr().ip().to_string();
        let protocol_version = link.peer().protocol_version;

        let mut trusted = false;
        let trust_check = self
            .revocations
            .if_pairing_allowed(peer_id, || self.trust.lock_ok().check(peer_id, &peer_fp))
            .unwrap_or(TrustCheck::Unknown);
        let session = match trust_check {
            TrustCheck::Changed => {
                log(
                    "REFUSED",
                    &format!("{peer_name}: certificate changed — unpair and re-pair on purpose"),
                );
                ui_log(
                    self,
                    peer_name,
                    "RECHAZADO: el certificado cambió — vuelve a emparejar a propósito",
                    true,
                );
                return Ok(());
            }
            TrustCheck::Trusted => {
                log("trust", &format!("{peer_name} is already paired"));
                trusted = true;
                Session::restored(protocol_version)
            }
            TrustCheck::Unknown => {
                log(
                    "pair",
                    &format!("link up — tap \"Vincular\"/Pair on {peer_name} to trust it"),
                );
                Session::new(protocol_version)
            }
        };

        if trusted {
            admission.release();
        }
        let mut untrusted_deadline = (!trusted).then(|| Instant::now() + UNTRUSTED_LINK_TIMEOUT);
        let peer_address = link.peer_addr().ip();
        let mut device = Device::new(link, session, millis(), TICK)?;
        let mut pair_generation = if trusted {
            self.next_generation()?
        } else {
            Generation::INITIAL
        };
        self.set_paired(peer_id, trusted, pair_generation);
        self.notify_change();

        // The mount lives as long as this link: dropping it (here or on unpair)
        // unmounts, so a lost link never strands a dead mount.
        let mut mount: Option<Mount> = None;
        let mut asked_sftp = false;
        let mut remote_media = RemoteMedia::default();
        let mut desktop_media: Option<media::Worker> = None;
        let mut payload_scope = payload_handlers::PayloadScope::new();

        loop {
            if untrusted_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                ui_log(self, peer_name, "enlace no emparejado expirado", true);
                return Ok(());
            }
            // A snapshot of the toggles for this turn; a plugin switched off is
            // one we neither drive nor react to.
            let settings = *self.settings.lock_ok();
            let mut events = Vec::new();
            let revocation_pending = self.revocations.pending(peer_id).is_some();
            let mut notify_revocation = false;
            if revocation_pending {
                let (revoked, notify) = device.revoke_pairing_local();
                events.extend(revoked);
                notify_revocation |= notify;
            }

            // Local controls take priority over the next peer packet.
            events.extend(self.drain_link_commands(
                &commands,
                &mut device,
                &mut remote_media,
                settings,
                link_commands::PeerContext::new(
                    peer_id,
                    peer_name,
                    &peer_fp,
                    pair_generation,
                    payload_scope.token(),
                ),
            )?);

            let pumped = if !revocation_pending {
                let pumped = device.pump()?;
                events.extend(pumped.events.clone());
                Some(pumped)
            } else {
                None
            };
            // Forget may have crossed the blocking read. Revoke again before a
            // Paired event or plugin packet from that read can be applied.
            if self.revocations.current(peer_id).is_some() && device.is_paired() {
                let (revoked, notify) = device.revoke_pairing_local();
                events.extend(revoked);
                notify_revocation |= notify;
            }

            for event in &events {
                if self.revocations.suppresses(peer_id, event) {
                    continue;
                }
                let verification_key = match event {
                    ConnectionEvent::Pairing => device.verification_key()?,
                    ConnectionEvent::Paired
                    | ConnectionEvent::Unpaired
                    | ConnectionEvent::Lost(LostReason::PairRejected)
                    | ConnectionEvent::Lost(LostReason::PairTimedOut)
                    | ConnectionEvent::Lost(LostReason::PairInvalid) => Some(String::new()),
                    _ => None,
                };
                if let Some(key) = verification_key {
                    if set_verification_key(&self.devices, peer_id, &key) {
                        self.notify_change();
                    }
                }
                log_event(event);
                if let Some((message, failure)) = event_line(event) {
                    ui_log(self, peer_name, message, failure);
                }
                match event {
                    ConnectionEvent::Paired => {
                        let pin = self.revocations.if_pairing_allowed(peer_id, || {
                            self.trust.lock_ok().pin(TrustedPeer {
                                device_id: peer_id.to_owned(),
                                device_name: peer_name.to_owned(),
                                fingerprint: peer_fp.clone(),
                            })
                        });
                        if let Some(pin) = pin {
                            pin?;
                            log("pinned", &format!("{peer_name} trusted; fingerprint saved"));
                            payload_scope.renew();
                            pair_generation = self.next_generation()?;
                            self.set_paired(peer_id, true, pair_generation);
                            self.notify_change();
                            device.send_ping()?;
                        }
                    }
                    ConnectionEvent::Unpaired => {
                        if self.revocations.current(peer_id).is_none() {
                            self.trust.lock_ok().forget(peer_id)?;
                        }
                        log(
                            "unpaired",
                            &format!("{peer_name} dropped the pairing; forgot it"),
                        );
                        mount = None; // drop → unmount
                        asked_sftp = false;
                        self.set_mount(peer_id, None);
                        payload_scope.cancel();
                        pair_generation = Generation::INITIAL;
                        self.set_paired(peer_id, false, pair_generation);
                        self.notify_change();
                    }
                    ConnectionEvent::Pinged => {
                        // KDE Connect shows a notification for a received ping.
                        if let Some(connection) = &self.dbus {
                            notify::post(
                                connection,
                                peer_name,
                                0,
                                "Ping",
                                &format!("{peer_name} te hizo ping"),
                            );
                        }
                        ui_log(self, peer_name, "ping recibido", false);
                    }
                    _ => {}
                }
            }

            if let Some(generation) = self.revocations.pending(peer_id) {
                let (_, notify) = device.revoke_pairing_local();
                notify_revocation |= notify;
                mount = None;
                asked_sftp = false;
                desktop_media = None;
                payload_scope.cancel();
                self.set_mount(peer_id, None);
                pair_generation = Generation::INITIAL;
                self.set_paired(peer_id, false, pair_generation);
                let _ = set_verification_key(&self.devices, peer_id, "");
                self.notify_change();
                self.revocations.acknowledge(peer_id, generation);
            }
            if notify_revocation && self.revocations.current(peer_id).is_some() {
                // Durable local state and the D-Bus barrier are already complete;
                // a broken peer notification may close this link but cannot undo
                // Forget or make its caller wait on the socket write.
                if let Err(error) = device.notify_revocation() {
                    return Err(Box::new(error));
                }
            }

            let media_active =
                settings.media && device.is_paired() && self.revocations.current(peer_id).is_none();
            media::set_active(&mut desktop_media, media_active)?;
            if media_active {
                remote_media.poll(&mut device, millis())?;
            }
            while let Some(reply) = desktop_media.as_ref().and_then(media::Worker::try_reply) {
                match reply {
                    media::Reply::Players(players) => {
                        device.send(|id| magnetita_core::mpris::player_list_packet(id, &players))?
                    }
                    media::Reply::State(state) => {
                        device.send(|id| magnetita_core::mpris::state_packet(id, &state))?
                    }
                }
            }

            // Prime a restored or newly-paired link only after local commands;
            // a concurrent Forget therefore cannot leak one last plugin request.
            if device.is_paired() && !asked_sftp {
                device.send(request_packet)?;
                if settings.battery {
                    device.send(magnetita_core::battery::request)?;
                }
                if settings.media {
                    device.send(magnetita_core::mpris::request_player_list)?;
                }
                if settings.clipboard {
                    self.send_clipboard_connect(&mut device)?;
                }
                asked_sftp = true;
                log("sftp", &format!("requesting {peer_name}'s storage"));
            }

            if device.is_paired() {
                admission.release();
                untrusted_deadline = None;
            } else if untrusted_deadline.is_none() {
                let Some(next_permit) = self.admission.try_acquire(peer_address) else {
                    ui_log(self, peer_name, "demasiados enlaces sin emparejar", true);
                    return Ok(());
                };
                admission = next_permit;
                untrusted_deadline = Some(Instant::now() + UNTRUSTED_LINK_TIMEOUT);
            }

            // Plugin packets the session leaves for us: the phone's battery and
            // its sftp reply.
            if let Some(packet) = pumped
                .as_ref()
                .filter(|_| device.is_paired() && self.revocations.current(peer_id).is_none())
                .and_then(|pumped| pumped.packet.as_ref())
            {
                if settings.battery {
                    if let Some(battery) = read_battery(packet) {
                        self.set_battery(peer_id, battery.charge, battery.charging);
                        self.notify_change();
                    }
                }
                if settings.notifications {
                    if let Some(note) = read_notification(packet) {
                        self.mirror_notification(peer_id, peer_name, note);
                    }
                }
                if settings.clipboard {
                    if let Some(text) = read_clipboard(packet) {
                        // Record before wl-copy so the watcher does not echo it back.
                        *self.last_clipboard.lock_ok() = text.clone();
                        if clipboard::write(&text) {
                            ui_log(self, peer_name, "portapapeles recibido", false);
                        }
                    }
                }
                if settings.share {
                    if let Some(file) = read_share(packet) {
                        self.spawn_file_receive(
                            payload_handlers::PayloadPeer {
                                device_id: peer_id,
                                device_name: peer_name,
                                host: &link_host,
                                fingerprint: &peer_fp,
                                pair_generation,
                                cancellation: payload_scope.token(),
                            },
                            file,
                        );
                    }
                }
                // Media, both ways — only while the plugin is enabled.
                if settings.media {
                    if let Some(incoming) = read_album_art(packet) {
                        if let Some((player, source)) = self.spawn_artwork_receive(
                            payload_handlers::PayloadPeer {
                                device_id: peer_id,
                                device_name: peer_name,
                                host: &link_host,
                                fingerprint: &peer_fp,
                                pair_generation,
                                cancellation: payload_scope.token(),
                            },
                            incoming,
                        ) {
                            remote_media.artwork_failed(&player, &source, millis());
                        }
                    }
                    // The phone reporting its media (for the now-playing card).
                    if let Some(update) = read_mpris(packet) {
                        let report = remote_media.handle(update, &mut device, millis())?;
                        let change = match report {
                            MediaReport::NoChange => None,
                            MediaReport::Cleared => {
                                Some(devices::set_media(&self.devices, peer_id, None))
                            }
                            MediaReport::State(state) => {
                                Some(devices::set_media(&self.devices, peer_id, Some(&state)))
                            }
                        };
                        if let Some(stale) = change {
                            if let Some(path) = stale {
                                artwork::discard(&path);
                            }
                            self.notify_change();
                        }
                    }
                    // The phone driving *our* players (its media remote).
                    if let Some(request) = read_mpris_request(packet) {
                        if let Some(worker) = &desktop_media {
                            worker.submit(request);
                        }
                    }
                }
                match read_sftp(packet) {
                    Some(SftpReply::Mount(info)) if mount.is_none() => {
                        let host = info.ip.clone().unwrap_or_else(|| link_host.clone());
                        match Mount::open(peer_id, &host, &info) {
                            Ok(m) => {
                                log("mounted", &format!("{peer_name} at {}", m.path().display()));
                                ui_log(self, peer_name, "archivos montados", false);
                                self.set_mount(peer_id, Some(m.path()));
                                self.notify_change();
                                mount = Some(m);
                            }
                            Err(e) => {
                                log("mount", &format!("{peer_name}: {e}"));
                                ui_log(self, peer_name, &format!("no se pudo montar: {e}"), true);
                            }
                        }
                    }
                    Some(SftpReply::Error(message)) => {
                        log("sftp", &format!("{peer_name} refused: {message}"));
                        ui_log(
                            self,
                            peer_name,
                            &format!("el móvil rechazó el montaje: {message}"),
                            true,
                        );
                    }
                    _ => {}
                }
            }

            if pumped.as_ref().is_some_and(|pumped| !pumped.open) {
                return Ok(()); // mount drops here → unmount
            }
        }
    }

    /// Reflect a device's pairing state into the registry.
    fn set_paired(&self, device_id: &str, paired: bool, generation: Generation) {
        if let Some(entry) = self.devices.lock_ok().get_mut(device_id) {
            entry.paired = paired;
            entry.pair_generation = generation;
        }
    }

    fn next_generation(&self) -> Result<Generation, celestina_core::GenerationExhausted> {
        self.generation_clock.lock_ok().issue()
    }

    /// Send our current clipboard as a `clipboard.connect` on connect — the
    /// handshake that tells the phone we are a clipboard peer, so it syncs its
    /// clipboard to us too. Recorded as last-synced so the watcher does not
    /// immediately re-send it.
    fn send_clipboard_connect(&self, device: &mut Device) -> Result<(), magnetita_net::LinkError> {
        let clip = clipboard::read();
        *self.last_clipboard.lock_ok() = clip.clone();
        device.send(|id| magnetita_core::clipboard::clipboard_connect_packet(id, &clip, millis()))
    }

    /// A desktop clipboard change: push it to every connected device — unless it
    /// is the value we just received from a phone (our own wl-copy echo), which
    /// would otherwise loop back and forth forever.
    fn push_clipboard(&self, text: String) {
        if text.is_empty() || !self.settings.lock_ok().clipboard {
            return;
        }
        {
            let mut last = self.last_clipboard.lock_ok();
            if *last == text {
                return;
            }
            *last = text.clone();
        }
        let device_ids: Vec<_> = self.commands.lock_ok().keys().cloned().collect();
        self.pending_clipboards.replace_for(device_ids, text);
    }

    /// Reflect a device's battery report into the registry.
    fn set_battery(&self, device_id: &str, charge: i32, charging: bool) {
        if let Some(entry) = self.devices.lock_ok().get_mut(device_id) {
            entry.battery = charge;
            entry.charging = charging;
        }
    }

    /// Mirror a phone notification to the desktop's notification server, keeping
    /// the id map so an update replaces and a cancel withdraws the right one.
    fn mirror_notification(&self, device_id: &str, device_name: &str, note: Notification) {
        let Some(connection) = &self.dbus else {
            return;
        };
        let key = format!("{device_id}\u{0}{}", note.id);
        if note.is_cancel {
            if let Some(server_id) = self.notifications.lock_ok().remove(&key) {
                notify::close(connection, server_id);
            }
            return;
        }
        let app = if note.app_name.is_empty() {
            device_name
        } else {
            &note.app_name
        };
        let summary = if note.title.is_empty() {
            app.to_owned()
        } else {
            note.title.clone()
        };
        let replaces = self.notifications.lock_ok().get(&key).copied().unwrap_or(0);
        if let Some(server_id) = notify::post(connection, app, replaces, &summary, &note.text) {
            self.notifications.lock_ok().insert(key, server_id);
            ui_log(self, device_name, &format!("🔔 {app}: {summary}"), false);
        }
    }

    /// Reflect a device's mount state into the registry.
    fn set_mount(&self, device_id: &str, path: Option<&Path>) {
        if let Some(entry) = self.devices.lock_ok().get_mut(device_id) {
            match path {
                Some(path) => {
                    entry.mounted = true;
                    entry.mount_path = path.to_string_lossy().into_owned();
                }
                None => {
                    entry.mounted = false;
                    entry.mount_path.clear();
                }
            }
        }
    }

    /// Tell consumers of the contract that the device set or a device's state
    /// changed, so they re-read it. Best-effort; a broken bus is not fatal.
    fn notify_change(&self) {
        if let Some(connection) = &self.dbus {
            let _ = connection.emit_signal(
                Option::<&str>::None,
                devices::OBJECT_PATH,
                devices::INTERFACE,
                devices::CHANGED_SIGNAL,
                &(),
            );
        }
    }
}

/// Start serving `org.celestina.Devices1` on the session bus.
fn serve_devices(
    registry: Registry,
    log: Log,
    commands: Commands,
    trust: Arc<Mutex<TrustStore>>,
    revocations: Arc<Revocations>,
    settings: Arc<Mutex<Settings>>,
    settings_path: PathBuf,
) -> zbus::Result<zbus::blocking::Connection> {
    zbus::blocking::connection::Builder::session()?
        .name(devices::BUS_NAME)?
        .serve_at(
            devices::OBJECT_PATH,
            Devices::new(
                registry,
                log,
                commands,
                trust,
                revocations,
                settings,
                settings_path,
            ),
        )?
        .build()
}

/// Record a connection-log line for the app, and signal that a new entry landed.
/// Best-effort on the bus; the entry is kept regardless so the app sees it on
/// its next read.
fn ui_log(daemon: &Daemon, device: &str, message: &str, failure: bool) {
    push_log(
        &daemon.log,
        LogEntry {
            device: device.to_owned(),
            message: message.to_owned(),
            failure,
            time_ms: millis(),
        },
    );
    if let Some(connection) = &daemon.dbus {
        let _ = connection.emit_signal(
            Option::<&str>::None,
            devices::OBJECT_PATH,
            devices::INTERFACE,
            devices::EVENT_SIGNAL,
            &(),
        );
    }
}
