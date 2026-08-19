//! Pure projection from confirmed daemon snapshots to QML-friendly values.
//!
//! language-contract: product-copy
//!
//! The mirror labels below are the words the author reads on the device row;
//! everything else in this file is development truth as usual.

use magnetita_core::{playback_progress, PlaybackProgress};

use crate::devices::Device;

pub(crate) fn battery_label(device: &Device) -> String {
    if device.battery < 0 {
        String::new()
    } else if device.charging {
        format!("🔋 {} % ⚡", device.battery)
    } else {
        format!("🔋 {} %", device.battery)
    }
}

pub(crate) fn media_label(device: &Device) -> String {
    if device.media_player.is_empty() {
        return String::new();
    }
    match (device.media_artist.as_str(), device.media_title.as_str()) {
        ("", "") if !device.media_now_playing.is_empty() => device.media_now_playing.clone(),
        ("", "") if !device.media_album.is_empty() => device.media_album.clone(),
        ("", "") => device.media_player.clone(),
        ("", title) => title.to_owned(),
        (artist, "") => artist.to_owned(),
        (artist, title) => format!("{artist} — {title}"),
    }
}

pub(crate) fn progress_fields(device: &Device) -> (i64, i64, &'static str) {
    match playback_progress(device.media_position, device.media_length) {
        PlaybackProgress::Unavailable => (-1, -1, "unavailable"),
        PlaybackProgress::Finite {
            position_ms,
            length_ms,
        } => (position_ms, length_ms, "finite"),
        PlaybackProgress::Live => (-1, -1, "live"),
    }
}

pub(crate) fn flag(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

pub(crate) fn next_toggle_value(confirmed: bool, pending: Option<bool>) -> bool {
    !pending.unwrap_or(confirmed)
}

pub(crate) fn state_label(device: &Device) -> &'static str {
    if device.mounted {
        "montado"
    } else if device.connected {
        "conectando…"
    } else {
        "desconectado"
    }
}

/// The mirror's state in the author's words. The daemon's contract names are
/// language-neutral by design, so the wording lives here, where it is tested
/// without a bus and without a phone.
pub(crate) fn mirror_label(state: &str, reason: &str) -> String {
    match state {
        "idle" => "Activa la depuración inalámbrica en el móvil".to_owned(),
        "available" => "Listo para reflejar".to_owned(),
        "pairing" => "Vinculando…".to_owned(),
        "connecting" => "Conectando…".to_owned(),
        "connected" => "Abriendo el espejo…".to_owned(),
        "mirroring" => "Reflejando".to_owned(),
        "failed" => mirror_reason_label(reason),
        "" => "Espejo no disponible".to_owned(),
        // A daemon newer than this app: say so rather than render a raw token.
        _ => "Estado del espejo desconocido".to_owned(),
    }
}

/// Why the last attempt failed, phrased as what to do about it.
pub(crate) fn mirror_reason_label(reason: &str) -> String {
    match reason {
        "not-advertised" => {
            "No se ve el móvil. Activa la depuración inalámbrica y comprueba que estáis en la misma red".to_owned()
        }
        "pair-rejected" => "El móvil rechazó el código de vinculación".to_owned(),
        "connect-failed" => "No se pudo conectar con el móvil".to_owned(),
        "device-offline" => "El móvil respondió pero no llegó a estar listo".to_owned(),
        "mirror-failed" => "scrcpy no pudo abrirse".to_owned(),
        "tool-missing" => "Faltan adb o scrcpy en el sistema".to_owned(),
        "bad-address" | "bad-port" | "bad-service-name" => {
            "El anuncio del móvil no era válido".to_owned()
        }
        _ => "El espejo falló".to_owned(),
    }
}

/// Whether the Mirror control should offer to start or to stop.
pub(crate) fn mirror_is_active(state: &str) -> bool {
    matches!(state, "pairing" | "connecting" | "connected" | "mirroring")
}

#[cfg(test)]
mod tests {
    use super::{media_label, mirror_is_active, mirror_label, next_toggle_value, progress_fields};
    use crate::devices::Device;

    #[test]
    fn a_confirmed_player_is_visible_without_split_metadata() {
        let player_only = Device {
            media_player: "Spotify".to_owned(),
            ..Device::default()
        };
        assert_eq!(media_label(&player_only), "Spotify");

        let combined = Device {
            media_player: "Spotify".to_owned(),
            media_now_playing: "Band - Song".to_owned(),
            ..Device::default()
        };
        assert_eq!(media_label(&combined), "Band - Song");
    }

    #[test]
    fn live_progress_is_classified_before_qml() {
        let device = Device {
            media_position: 4_796_000_000,
            media_length: 4_796_000_000,
            ..Device::default()
        };
        assert_eq!(progress_fields(&device), (-1, -1, "live"));
    }

    #[test]
    fn finite_progress_is_clamped_before_qml() {
        let device = Device {
            media_position: 150_000,
            media_length: 120_000,
            ..Device::default()
        };
        assert_eq!(progress_fields(&device), (120_000, 120_000, "finite"));
    }

    #[test]
    fn rapid_toggle_intents_alternate_before_a_snapshot_returns() {
        let first = next_toggle_value(true, None);
        let second = next_toggle_value(true, Some(first));
        assert!(!first);
        assert!(second);
    }

    #[test]
    fn every_contract_state_gets_its_own_words() {
        let states = [
            "idle",
            "available",
            "pairing",
            "connecting",
            "connected",
            "mirroring",
        ];
        let labels: Vec<String> = states.iter().map(|s| mirror_label(s, "")).collect();
        assert!(labels.iter().all(|label| !label.is_empty()));
        // No two states may read the same, or the card would say nothing.
        for (i, a) in labels.iter().enumerate() {
            for b in &labels[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn a_failure_is_phrased_as_what_to_do_about_it() {
        assert!(mirror_label("failed", "not-advertised").contains("depuración inalámbrica"));
        assert!(mirror_label("failed", "tool-missing").contains("scrcpy"));
    }

    #[test]
    fn a_state_this_app_does_not_know_never_renders_a_raw_token() {
        let label = mirror_label("teleporting", "");
        assert!(!label.contains("teleporting"));
        assert!(!label.is_empty());
        let reason = mirror_label("failed", "cosmic-rays");
        assert!(!reason.contains("cosmic-rays"));
        assert!(!reason.is_empty());
    }

    #[test]
    fn the_control_offers_to_stop_only_while_something_is_running() {
        assert!(!mirror_is_active("idle"));
        assert!(!mirror_is_active("available"));
        assert!(!mirror_is_active("failed"));
        assert!(mirror_is_active("connecting"));
        assert!(mirror_is_active("mirroring"));
    }
}
