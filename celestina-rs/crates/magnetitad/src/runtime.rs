//! Small process-level helpers shared by the daemon coordinator.

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use magnetita_core::{ConnectionEvent, DeviceType, LostReason};

pub(crate) fn event_line(event: &ConnectionEvent) -> Option<(&'static str, bool)> {
    use ConnectionEvent::*;
    Some(match event {
        Pairing => (
            "emparejamiento pendiente; confirma Emparejar en Magnetita",
            false,
        ),
        Paired => ("emparejado", false),
        Unpaired => ("desemparejado", false),
        Lost(LostReason::NoReply) => ("sin respuesta", true),
        Lost(LostReason::Unreachable) => ("inalcanzable (¿otra red?)", true),
        Lost(LostReason::TlsFailed) => ("falló el cifrado TLS", true),
        Lost(LostReason::CertChanged) => ("el certificado cambió — posible impostor", true),
        Lost(LostReason::PairRejected) => ("emparejamiento rechazado", true),
        Lost(LostReason::PairTimedOut) => ("el emparejamiento expiró", true),
        Lost(LostReason::PairInvalid) => ("solicitud de emparejamiento inválida", true),
        Lost(LostReason::PeerClosed) => ("el dispositivo cerró el enlace", true),
        _ => return None,
    })
}

pub(crate) fn is_disconnect(message: &str) -> bool {
    let message = message.to_lowercase();
    message.contains("reset by peer")
        || message.contains("broken pipe")
        || message.contains("os error 104")
        || message.contains("os error 32")
        || message.contains("unexpected end")
        || message.contains("unexpectedeof")
}

pub(crate) fn type_label(device_type: DeviceType) -> String {
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

pub(crate) fn millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

pub(crate) fn id_source() -> impl FnMut() -> i64 {
    let mut last = millis();
    move || {
        last += 1;
        last
    }
}

pub(crate) fn log(tag: &str, message: &str) {
    println!("[{tag}] {message}");
    let _ = std::io::stdout().flush();
}

pub(crate) fn log_event(event: &ConnectionEvent) {
    log("event", &format!("{event:?}"));
}

/// Record that an untrusted-link slot was refused. Exhaustion denies the real
/// phone, so a silent skip makes the denial undiagnosable — the one symptom
/// the author would otherwise have to guess at.
pub(crate) fn log_admission_exhausted(path: &str, address: std::net::IpAddr) {
    log(
        "admission",
        &format!("{path}: no untrusted-link slot left for {address}"),
    );
}
