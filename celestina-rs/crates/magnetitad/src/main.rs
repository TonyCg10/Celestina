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
//! Whichever path forms the link, it runs the same KDE Connect v8 handshake and
//! the same session: for a phone we have not met we wait for it to ask and
//! accept (letting the phone drive avoids a double-request that pairs then
//! unpairs); then we pin its certificate and can ping. One link per device — the
//! second path to arrive is dropped. Every step prints a line, because *"why
//! won't it connect"* is the feature.
//!
//! CP1 wraps this in a window; CP2 hangs the sftp mount and
//! `org.celestina.Devices1` off the same trusted link.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::error::Error;
use std::fs;
use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use celestina_core::xdg;
use magnetita_core::{
    read_battery, read_clipboard, read_notification, read_sftp, read_share, request_packet,
    ConnectionEvent, DeviceType, Identity, IncomingFile, LostReason, Notification, Session,
    SftpReply,
};
use magnetita_net::discovery::ANNOUNCE_INTERVAL;
use magnetita_net::{
    Announcement, Device, DeviceCert, Discovery, Link, TlsConfigs, TrustCheck, TrustStore,
    TrustedPeer,
};

mod devices;
mod mount;
mod notify;
use devices::{push_log, Command, Commands, DeviceEntry, Devices, Log, LogEntry, Registry};
use mount::Mount;

/// The KDE Connect port: UDP announce/listen and TCP link.
const PORT: u16 = 1716;

/// How long the TCP+TLS handshake may take before we give up on a link.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// How often [`Device::pump`] wakes to check the pairing clock while idle.
const TICK: Duration = Duration::from_secs(1);

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
    trust: Mutex<TrustStore>,
    devices: Registry,
    log: Log,
    commands: Commands,
    dbus: Option<zbus::blocking::Connection>,
    /// phone-notification-id → freedesktop-server-id, so an update replaces and a
    /// cancel withdraws the right desktop notification.
    notifications: Mutex<HashMap<String, u32>>,
}

fn run() -> Result<(), Box<dyn Error>> {
    let dir = xdg::config_home()
        .ok_or("no XDG config home to store the device identity")?
        .join("magnetita");
    fs::create_dir_all(&dir)?;

    let device_id = ensure_device_id(&dir)?;
    let cert = DeviceCert::ensure(&dir, &device_id)?;

    // Serve org.celestina.Devices1 (best-effort: no session bus just means
    // Siderita cannot draw the phone, not that the link fails).
    let registry: Registry = Arc::new(Mutex::new(BTreeMap::new()));
    let event_log: Log = Arc::new(Mutex::new(VecDeque::new()));
    let commands: Commands = Arc::new(Mutex::new(HashMap::new()));
    let dbus = match serve_devices(
        Arc::clone(&registry),
        Arc::clone(&event_log),
        Arc::clone(&commands),
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
        trust: Mutex::new(TrustStore::load(&dir.join("trust.json"))?),
        devices: registry,
        log: event_log,
        commands,
        dbus,
        notifications: Mutex::new(HashMap::new()),
    });

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
    log("ready", "listening for the phone — keep KDE Connect open on it");
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
            .lock()
            .unwrap()
            .contains_key(&announcement.identity.device_id)
        {
            continue; // already linked by one path or the other
        }
        spawn_dialer(Arc::clone(&daemon), announcement);
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
            if daemon.devices.lock().unwrap().is_empty() {
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
            let daemon = Arc::clone(&daemon);
            thread::spawn(move || match accept_link(&daemon, tcp) {
                Ok(link) => daemon.serve(link, "accepted"),
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
fn spawn_dialer(daemon: Arc<Daemon>, announcement: Announcement) {
    thread::spawn(move || {
        let mut next_id = id_source();
        match Link::connect(
            &announcement,
            &daemon.identity,
            &daemon.tls,
            &mut next_id,
            HANDSHAKE_TIMEOUT,
        ) {
            Ok(link) => daemon.serve(link, "dialed"),
            Err(e) => {
                let name = &announcement.identity.device_name;
                log("dial", &format!("{name}: {e}"));
                ui_log(&daemon, name, &format!("no se pudo conectar: {e}"), true);
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
    fn serve(self: &Arc<Self>, link: Link, how: &'static str) {
        let peer_id = link.peer().device_id.clone();
        let peer_name = link.peer().device_name.clone();
        let peer_type = type_label(link.peer().device_type);
        let fingerprint = link.peer_fingerprint().to_owned();

        {
            // Hold the lock across check-and-insert so two paths cannot both
            // pass; one link per device, the loser dropped here.
            let mut devices = self.devices.lock().unwrap();
            if devices.contains_key(&peer_id) {
                return;
            }
            devices.insert(
                peer_id.clone(),
                DeviceEntry {
                    id: peer_id.clone(),
                    name: peer_name.clone(),
                    device_type: peer_type,
                    connected: true,
                    mounted: false,
                    paired: false,
                    mount_path: String::new(),
                    battery: -1,
                    charging: false,
                    fingerprint,
                },
            );
        }
        self.notify_change();
        log(how, &format!("{peer_name} at {}", link.peer_addr()));
        log("secured", &format!("fingerprint {}", link.peer_fingerprint()));
        ui_log(self, &peer_name, "conectado y cifrado", false);

        if let Err(e) = self.run_link(link, &peer_id, &peer_name) {
            log("link", &format!("{peer_name}: {e}"));
            ui_log(self, &peer_name, &format!("error de enlace: {e}"), true);
        }
        self.devices.lock().unwrap().remove(&peer_id);
        self.commands.lock().unwrap().remove(&peer_id);
        self.notify_change();
        log("closed", &format!("{peer_name} disconnected"));
        ui_log(self, &peer_name, "desconectado", false);
    }

    /// Drive one link's session: trust-check it, let the phone drive pairing,
    /// keep the trust store honest, and — once trusted — mount its storage.
    fn run_link(
        self: &Arc<Self>,
        link: Link,
        peer_id: &str,
        peer_name: &str,
    ) -> Result<(), Box<dyn Error>> {
        let peer_fp = link.peer_fingerprint().to_owned();
        let link_host = link.peer_addr().ip().to_string();

        let mut trusted = false;
        let session = match self.trust.lock().unwrap().check(peer_id, &peer_fp) {
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
                Session::restored()
            }
            TrustCheck::Unknown => {
                log(
                    "pair",
                    &format!("link up — tap \"Vincular\"/Pair on {peer_name} to trust it"),
                );
                Session::new()
            }
        };

        let mut device = Device::new(link, session, millis(), TICK)?;

        // Register a command channel so the app can drive pair/unpair on this
        // live link. serve() removes it when the link ends.
        let (tx, commands) = mpsc::channel::<Command>();
        self.commands.lock().unwrap().insert(peer_id.to_owned(), tx);
        self.set_paired(peer_id, trusted);
        self.notify_change();

        // The mount lives as long as this link: dropping it (here or on unpair)
        // unmounts, so a lost link never strands a dead mount.
        let mut mount: Option<Mount> = None;
        let mut asked_sftp = false;

        // A device we already trust: ask for its storage right away.
        if trusted {
            device.send(request_packet)?;
            device.send(magnetita_core::battery::request)?;
            asked_sftp = true;
            log("sftp", &format!("requesting {peer_name}'s storage"));
        }

        loop {
            let pumped = device.pump()?;
            let mut events = pumped.events;

            // Commands the app forwarded for this device (its "Emparejar" /
            // "Desvincular" buttons).
            while let Ok(command) = commands.try_recv() {
                match command {
                    Command::RequestPair => events.extend(device.request_pairing()?),
                    Command::Unpair => events.extend(device.unpair()?),
                    Command::Ring => {
                        device.send(magnetita_core::findmyphone::request)?;
                        ui_log(self, peer_name, "sonando el móvil", false);
                    }
                }
            }

            // The phone asked and awaits us — accept (one clean exchange).
            if device.peer_wants_to_pair() {
                log("pair", &format!("{peer_name} asked to pair — accepting"));
                events.extend(device.accept_pairing()?);
            }

            for event in &events {
                log_event(event);
                if let Some((message, failure)) = event_line(event) {
                    ui_log(self, peer_name, message, failure);
                }
                match event {
                    ConnectionEvent::Paired => {
                        self.trust.lock().unwrap().pin(TrustedPeer {
                            device_id: peer_id.to_owned(),
                            device_name: peer_name.to_owned(),
                            fingerprint: peer_fp.clone(),
                        })?;
                        log("pinned", &format!("{peer_name} trusted; fingerprint saved"));
                        self.set_paired(peer_id, true);
                        self.notify_change();
                        device.send_ping()?;
                        if !asked_sftp {
                            device.send(request_packet)?;
                            device.send(magnetita_core::battery::request)?;
                            asked_sftp = true;
                            log("sftp", &format!("requesting {peer_name}'s storage"));
                        }
                    }
                    ConnectionEvent::Unpaired => {
                        self.trust.lock().unwrap().forget(peer_id)?;
                        log("unpaired", &format!("{peer_name} dropped the pairing; forgot it"));
                        mount = None; // drop → unmount
                        self.set_mount(peer_id, None);
                        self.set_paired(peer_id, false);
                        self.notify_change();
                    }
                    _ => {}
                }
            }

            // Plugin packets the session leaves for us: the phone's battery and
            // its sftp reply.
            if let Some(packet) = &pumped.packet {
                if let Some(battery) = read_battery(packet) {
                    self.set_battery(peer_id, battery.charge, battery.charging);
                    self.notify_change();
                }
                if let Some(note) = read_notification(packet) {
                    self.mirror_notification(peer_id, peer_name, note);
                }
                if let Some(text) = read_clipboard(packet) {
                    if set_clipboard(&text) {
                        ui_log(self, peer_name, "portapapeles recibido", false);
                    }
                }
                if let Some(file) = read_share(packet) {
                    // A file streams over a second socket; receive it off the
                    // pump thread so a big transfer never freezes the link.
                    let daemon = Arc::clone(self);
                    let host = link_host.clone();
                    let name = peer_name.to_owned();
                    thread::spawn(move || daemon.receive_file(name, host, file));
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
                        ui_log(self, peer_name, &format!("el móvil rechazó el montaje: {message}"), true);
                    }
                    _ => {}
                }
            }

            if !pumped.open {
                return Ok(()); // mount drops here → unmount
            }
        }
    }

    /// Reflect a device's pairing state into the registry.
    fn set_paired(&self, device_id: &str, paired: bool) {
        if let Some(entry) = self.devices.lock().unwrap().get_mut(device_id) {
            entry.paired = paired;
        }
    }

    /// Reflect a device's battery report into the registry.
    fn set_battery(&self, device_id: &str, charge: i32, charging: bool) {
        if let Some(entry) = self.devices.lock().unwrap().get_mut(device_id) {
            entry.battery = charge;
            entry.charging = charging;
        }
    }

    /// Receive a shared file into the downloads dir, then notify and log. Runs on
    /// its own thread. The name is reduced to its base component so the phone
    /// cannot write outside the target dir, and a half-received file is removed.
    fn receive_file(self: Arc<Self>, device_name: String, host: String, file: IncomingFile) {
        let name = safe_filename(&file.filename);
        let dir = download_dir();
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let dest = unique_path(&dir, &name);
        ui_log(&self, &device_name, &format!("recibiendo {name}…"), false);
        match magnetita_net::receive_to_file(&host, file.port, file.size, &self.tls, &dest) {
            Ok(_) => {
                ui_log(&self, &device_name, &format!("recibido: {name}"), false);
                if let Some(connection) = &self.dbus {
                    notify::post(connection, &device_name, 0, "Archivo recibido", &name);
                }
            }
            Err(e) => {
                ui_log(&self, &device_name, &format!("fallo al recibir {name}: {e}"), true);
                let _ = std::fs::remove_file(&dest);
            }
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
            if let Some(server_id) = self.notifications.lock().unwrap().remove(&key) {
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
        let replaces = self
            .notifications
            .lock()
            .unwrap()
            .get(&key)
            .copied()
            .unwrap_or(0);
        if let Some(server_id) = notify::post(connection, app, replaces, &summary, &note.text) {
            self.notifications.lock().unwrap().insert(key, server_id);
            ui_log(self, device_name, &format!("🔔 {app}: {summary}"), false);
        }
    }

    /// Reflect a device's mount state into the registry.
    fn set_mount(&self, device_id: &str, path: Option<&Path>) {
        if let Some(entry) = self.devices.lock().unwrap().get_mut(device_id) {
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
) -> zbus::Result<zbus::blocking::Connection> {
    zbus::blocking::connection::Builder::session()?
        .name(devices::BUS_NAME)?
        .serve_at(
            devices::OBJECT_PATH,
            Devices::new(registry, log, commands),
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

/// A connection event as a log line for the app, or `None` for the noisy or
/// purely-internal ones. The `bool` marks a failure worth showing in red.
fn event_line(event: &ConnectionEvent) -> Option<(&'static str, bool)> {
    use ConnectionEvent::*;
    Some(match event {
        Pairing => ("emparejamiento en curso", false),
        Paired => ("emparejado", false),
        Unpaired => ("desemparejado", false),
        Lost(LostReason::NoReply) => ("sin respuesta", true),
        Lost(LostReason::Unreachable) => ("inalcanzable (¿otra red?)", true),
        Lost(LostReason::TlsFailed) => ("falló el cifrado TLS", true),
        Lost(LostReason::CertChanged) => ("el certificado cambió — posible impostor", true),
        Lost(LostReason::PairRejected) => ("emparejamiento rechazado", true),
        Lost(LostReason::PairTimedOut) => ("el emparejamiento expiró", true),
        Lost(LostReason::PeerClosed) => ("el dispositivo cerró el enlace", true),
        // Discovered / Linking / Secured / Identified / Pinged: too low-level or
        // too noisy for the log; the daemon logs its own milestones instead.
        _ => return None,
    })
}

/// The contract's device-type label for a peer's declared type.
fn type_label(device_type: DeviceType) -> String {
    match device_type {
        DeviceType::Phone => "phone",
        DeviceType::Tablet => "tablet",
        DeviceType::Laptop => "laptop",
        DeviceType::Desktop => "desktop",
        DeviceType::Tv => "tv",
        DeviceType::Unknown => "unknown",
    }
    .to_owned()
}

/// Our stable 32-hex device id (a UUID with the dashes removed, the shape KDE
/// Connect expects), generated once and reused.
fn ensure_device_id(dir: &Path) -> Result<String, Box<dyn Error>> {
    let path = dir.join("device_id");
    if let Ok(existing) = fs::read_to_string(&path) {
        let existing = existing.trim().to_owned();
        if !existing.is_empty() {
            return Ok(existing);
        }
    }
    let uuid = fs::read_to_string("/proc/sys/kernel/random/uuid")?;
    let id: String = uuid.trim().chars().filter(|c| *c != '-').collect();
    fs::write(&path, &id)?;
    Ok(id)
}

/// Puts text on the desktop (Wayland) clipboard via wl-copy. Returns whether it
/// ran. wl-copy reads stdin, forks a background server to hold the selection, and
/// its foreground process exits — so we write the text, close stdin, and reap it.
fn set_clipboard(text: &str) -> bool {
    use std::process::{Command, Stdio};
    let Ok(mut child) = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    let _ = child.wait();
    true
}

/// The XDG downloads dir for received files: `$XDG_DOWNLOAD_DIR`, else what
/// `xdg-user-dir` reports, else `~/Downloads`.
fn download_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_DOWNLOAD_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(output) = std::process::Command::new("xdg-user-dir")
        .arg("DOWNLOAD")
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !path.is_empty() {
                return PathBuf::from(path);
            }
        }
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join("Downloads")
}

/// Only the file-name component of a shared name, so a crafted path cannot
/// escape the downloads dir.
fn safe_filename(name: &str) -> String {
    Path::new(name)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("archivo")
        .to_owned()
}

/// A path in `dir` that does not clobber an existing file: `a.jpg`, then
/// `a (1).jpg`, and so on.
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = path.extension().and_then(|e| e.to_str());
    for n in 1..10_000 {
        let alt = match ext {
            Some(ext) => format!("{stem} ({n}).{ext}"),
            None => format!("{stem} ({n})"),
        };
        let candidate = dir.join(alt);
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(name)
}

/// Millisecond wall clock for packet ids.
fn millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// A strictly-increasing id source seeded by the clock, so packets within one
/// connection never collide even inside the same millisecond.
fn id_source() -> impl FnMut() -> i64 {
    let mut last = millis();
    move || {
        last += 1;
        last
    }
}

fn log(tag: &str, message: &str) {
    println!("[{tag}] {message}");
    let _ = std::io::stdout().flush();
}

fn log_event(event: &ConnectionEvent) {
    log("event", &format!("{event:?}"));
}
