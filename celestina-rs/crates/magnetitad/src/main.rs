//! `magnetitad` — Magnetita's CP0 daemon: the trusted channel to the phone.
//!
//! This is the headless proof of the whole stack below it. It makes (once) our
//! device id and certificate, announces itself over UDP, listens for the phone,
//! and when it hears one it dials, runs the KDE Connect v8 handshake, and — for a
//! phone we have not met — asks to pair and waits for the tap on the phone. Once
//! trusted it pins the certificate and pings. Everything it does prints as a
//! plain line, because *"why won't it connect"* is the feature: the log is the
//! answer.
//!
//! CP1 will wrap this in a real window; CP2 will hang the sftp mount and the
//! `org.celestina.Devices1` service off the same trusted link. For now it is the
//! smallest thing that can earn a phone's trust and prove the transport is real.

use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use celestina_core::xdg;
use magnetita_core::{ConnectionEvent, Identity, Session};
use magnetita_net::discovery::ANNOUNCE_INTERVAL;
use magnetita_net::{Device, DeviceCert, Discovery, Link, TlsConfigs, TrustCheck, TrustStore, TrustedPeer};

/// How long to wait for the TCP+TLS handshake before giving up on a dial.
const DIAL_TIMEOUT: Duration = Duration::from_secs(10);

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

fn run() -> Result<(), Box<dyn Error>> {
    let dir = xdg::config_home()
        .ok_or("no XDG config home to store the device identity")?
        .join("magnetita");
    fs::create_dir_all(&dir)?;

    let device_id = ensure_device_id(&dir)?;
    let cert = DeviceCert::ensure(&dir, &device_id)?;
    let tls = TlsConfigs::build(&cert)?;
    let mut trust = TrustStore::load(&dir.join("trust.json"))?;
    let identity = Identity::desktop(&device_id, "Celestina");

    log("id", &device_id);
    log("cert", &cert.fingerprint()?);

    let discovery = Discovery::bind("0.0.0.0:1716".parse()?, &device_id).map_err(|e| {
        format!("cannot bind UDP 1716 ({e}) — is Valent or kdeconnectd still running?")
    })?;

    // Announce ourselves so the phone lists Magnetita, too.
    let announcer = discovery.try_clone()?;
    let announced = identity.clone();
    thread::spawn(move || loop {
        let _ = announcer.announce(&announced, millis());
        thread::sleep(ANNOUNCE_INTERVAL);
    });

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
        let Some(addr) = announcement.link_addr() else {
            continue;
        };
        log(
            "heard",
            &format!(
                "{} at {addr} (proto {})",
                announcement.identity.device_name, announcement.identity.protocol_version
            ),
        );

        if let Err(e) = serve(&announcement, &identity, &tls, &mut trust) {
            log("link", &format!("{}: {e}", announcement.identity.device_name));
        }
        // A beat before we might re-hear the same phone and dial again.
        thread::sleep(Duration::from_millis(500));
    }
}

/// Dial a heard device, run the handshake, and — if it is new — pair, then stay
/// on the link logging what arrives until it closes.
fn serve(
    announcement: &magnetita_net::Announcement,
    identity: &Identity,
    tls: &TlsConfigs,
    trust: &mut TrustStore,
) -> Result<(), Box<dyn Error>> {
    let addr = announcement.link_addr().expect("caller checked link_addr");
    log("dial", &format!("connecting to {addr}"));

    let mut next_id = id_source();
    let link = Link::connect(announcement, identity, tls, &mut next_id, DIAL_TIMEOUT)?;
    log("secured", &format!("fingerprint {}", link.peer_fingerprint()));

    let peer_id = link.peer().device_id.clone();
    let peer_name = link.peer().device_name.clone();
    let peer_fp = link.peer_fingerprint().to_owned();

    let session = match trust.check(&peer_id, &peer_fp) {
        TrustCheck::Changed => {
            log(
                "REFUSED",
                &format!("{peer_name}: certificate changed — unpair and re-pair on purpose"),
            );
            return Ok(());
        }
        TrustCheck::Trusted => {
            log("trust", &format!("{peer_name} is already paired"));
            Session::restored()
        }
        TrustCheck::Unknown => {
            // We do NOT request pairing ourselves: the phone requests when the
            // user taps "Vincular", and two simultaneous requests make KDE
            // Connect pair then immediately unpair. We establish the link and
            // let the phone drive; we accept its request when it arrives.
            log(
                "pair",
                &format!("link up — tap \"Vincular\"/Pair on {peer_name} to trust it"),
            );
            Session::new()
        }
    };

    let mut device = Device::new(link, session, millis(), TICK)?;

    loop {
        let pumped = device.pump()?;
        let mut events = pumped.events;

        // The phone asked and awaits us — accept it (one clean exchange).
        if device.peer_wants_to_pair() {
            log("pair", &format!("{peer_name} asked to pair — accepting"));
            events.extend(device.accept_pairing()?);
        }

        for event in &events {
            log_event(event);
            match event {
                ConnectionEvent::Paired => {
                    trust.pin(TrustedPeer {
                        device_id: peer_id.clone(),
                        device_name: peer_name.clone(),
                        fingerprint: peer_fp.clone(),
                    })?;
                    log("pinned", &format!("{peer_name} trusted; fingerprint saved"));
                    device.send_ping()?;
                    log("ping", "sent");
                }
                ConnectionEvent::Unpaired => {
                    // Keep the trust store honest with what the peer believes.
                    trust.forget(&peer_id)?;
                    log("unpaired", &format!("{peer_name} dropped the pairing; forgot it"));
                }
                _ => {}
            }
        }

        if !pumped.open {
            log("closed", &format!("{peer_name} closed the link"));
            return Ok(());
        }
    }
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

/// Millisecond wall clock for packet ids.
fn millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// A strictly-increasing id source seeded by the clock, so packets within one
/// connection never collide even when sent inside the same millisecond.
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
