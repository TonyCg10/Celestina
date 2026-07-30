//! Pure projection from confirmed daemon snapshots to QML-friendly values.

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

#[cfg(test)]
mod tests {
    use super::{media_label, next_toggle_value, progress_fields};
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
}
