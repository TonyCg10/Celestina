//! What the bar shows of the author's phone.
//!
//! `org.celestina.Devices1` lists every paired device with its connection,
//! battery and media state. The bar wants one phone: this module picks it and
//! turns the raw row into a state that can be drawn without checking anything
//! again. It does no IO and knows nothing about D-Bus; the worker that reads
//! the bus hands in plain [`DeviceRow`] values.
//!
//! Everything in a row comes from another process, so text is cut to a bound
//! before it is kept and a battery reading outside 0..=100 is no reading.

use crate::bounded::bounded;

/// The longest device name kept, in UTF-16 code units.
pub const MAX_NAME_UNITS: usize = 64;
/// The longest media title or artist kept, in UTF-16 code units.
pub const MAX_MEDIA_UNITS: usize = 128;
/// The longest player name kept, in UTF-16 code units.
pub const MAX_PLAYER_UNITS: usize = 64;
/// The most rows read from one listing; a bus that returns more is hostile.
pub const MAX_DEVICES: usize = 64;

/// The device type that makes a device the phone.
const PHONE_TYPE: &str = "phone";

/// One device as `org.celestina.Devices1` lists it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceRow {
    /// Stable device id; the key for every action.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Device type, as the service names it (`phone`, `tablet`, ...).
    pub kind: String,
    /// Whether the device is reachable now.
    pub connected: bool,
    /// Battery charge in percent; `-1` when unknown.
    pub battery: i32,
    /// Whether the battery is charging.
    pub charging: bool,
    /// Name of the media player on the device; empty when there is none.
    pub media_player: String,
    /// Title of the current track.
    pub media_title: String,
    /// Artist of the current track.
    pub media_artist: String,
    /// Whether the player is playing.
    pub media_playing: bool,
    /// Whether the player accepts pause.
    pub media_can_pause: bool,
    /// Whether the player accepts next.
    pub media_can_next: bool,
    /// Whether the player accepts previous.
    pub media_can_previous: bool,
}

/// What a phone is playing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Media {
    /// The player's name.
    pub player: String,
    /// The track title; may be empty.
    pub title: String,
    /// The track artist; may be empty.
    pub artist: String,
    /// Whether it is playing.
    pub playing: bool,
    /// Whether pause is available.
    pub can_pause: bool,
    /// Whether next is available.
    pub can_next: bool,
    /// Whether previous is available.
    pub can_previous: bool,
}

/// The phone the bar shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Phone {
    /// The id every action on this device is keyed by.
    pub id: String,
    /// The display name, bounded.
    pub name: String,
    /// Charge in percent, or `None` when the device did not report one.
    pub battery: Option<u8>,
    /// Whether the battery is charging.
    pub charging: bool,
    /// The media state, when the device has a player.
    pub media: Option<Media>,
}

/// The bar's view of the phone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhoneState {
    /// No connected device: the piece draws nothing.
    Absent,
    /// A connected device.
    Present(Phone),
}

/// A battery reading as a percentage: `None` for the "unknown" value and for
/// anything outside the scale, so a bad number is never drawn as a charge.
#[must_use]
pub fn clean_battery(raw: i32) -> Option<u8> {
    u8::try_from(raw).ok().filter(|percent| *percent <= 100)
}

fn reduce_one(row: &DeviceRow) -> Phone {
    let media = (!row.media_player.is_empty()).then(|| Media {
        player: bounded(&row.media_player, MAX_PLAYER_UNITS),
        title: bounded(&row.media_title, MAX_MEDIA_UNITS),
        artist: bounded(&row.media_artist, MAX_MEDIA_UNITS),
        playing: row.media_playing,
        can_pause: row.media_can_pause,
        can_next: row.media_can_next,
        can_previous: row.media_can_previous,
    });
    Phone {
        id: row.id.clone(),
        name: bounded(&row.name, MAX_NAME_UNITS),
        battery: clean_battery(row.battery),
        charging: row.charging,
        media,
    }
}

/// The phone to show: the first connected device of type `phone`, else the
/// first connected device of any type, else [`PhoneState::Absent`]. Only the
/// first [`MAX_DEVICES`] rows are considered.
#[must_use]
pub fn reduce(rows: &[DeviceRow]) -> PhoneState {
    let rows = &rows[..rows.len().min(MAX_DEVICES)];
    let chosen = rows
        .iter()
        .find(|row| row.connected && row.kind == PHONE_TYPE)
        .or_else(|| rows.iter().find(|row| row.connected));
    chosen.map_or(PhoneState::Absent, |row| {
        PhoneState::Present(reduce_one(row))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, kind: &str, connected: bool) -> DeviceRow {
        DeviceRow {
            id: id.to_owned(),
            name: format!("name-{id}"),
            kind: kind.to_owned(),
            connected,
            battery: -1,
            ..DeviceRow::default()
        }
    }

    fn present(state: PhoneState) -> Phone {
        match state {
            PhoneState::Present(phone) => phone,
            PhoneState::Absent => panic!("expected a present phone"),
        }
    }

    #[test]
    fn no_devices_is_absent() {
        assert_eq!(reduce(&[]), PhoneState::Absent);
    }

    #[test]
    fn only_disconnected_devices_is_absent() {
        let rows = [row("a", "phone", false), row("b", "tablet", false)];
        assert_eq!(reduce(&rows), PhoneState::Absent);
    }

    #[test]
    fn the_first_connected_phone_wins_over_an_earlier_tablet() {
        let rows = [
            row("t", "tablet", true),
            row("p1", "phone", false),
            row("p2", "phone", true),
            row("p3", "phone", true),
        ];
        assert_eq!(present(reduce(&rows)).id, "p2");
    }

    #[test]
    fn without_a_phone_the_first_connected_device_is_used() {
        let rows = [
            row("p", "phone", false),
            row("t", "tablet", true),
            row("d", "desktop", true),
        ];
        assert_eq!(present(reduce(&rows)).id, "t");
    }

    #[test]
    fn battery_is_none_when_unknown_negative_or_over_100() {
        assert_eq!(clean_battery(-1), None);
        assert_eq!(clean_battery(-100), None);
        assert_eq!(clean_battery(101), None);
        assert_eq!(clean_battery(i32::MAX), None);
        assert_eq!(clean_battery(i32::MIN), None);
        assert_eq!(clean_battery(0), Some(0));
        assert_eq!(clean_battery(57), Some(57));
        assert_eq!(clean_battery(100), Some(100));
        let mut r = row("p", "phone", true);
        r.battery = 140;
        assert_eq!(present(reduce(&[r])).battery, None);
    }

    #[test]
    fn charging_and_battery_pass_through() {
        let mut r = row("p", "phone", true);
        r.battery = 80;
        r.charging = true;
        let phone = present(reduce(&[r]));
        assert_eq!(phone.battery, Some(80));
        assert!(phone.charging);
        assert_eq!(phone.media, None);
    }

    #[test]
    fn media_is_kept_only_with_a_player() {
        let mut r = row("p", "phone", true);
        r.media_title = "title".to_owned();
        assert_eq!(present(reduce(std::slice::from_ref(&r))).media, None);
        r.media_player = "player".to_owned();
        r.media_artist = "artist".to_owned();
        r.media_playing = true;
        r.media_can_pause = true;
        r.media_can_next = true;
        let media = present(reduce(&[r])).media.expect("media");
        assert_eq!(media.player, "player");
        assert_eq!(media.title, "title");
        assert_eq!(media.artist, "artist");
        assert!(media.playing && media.can_pause && media.can_next);
        assert!(!media.can_previous);
    }

    #[test]
    fn hostile_long_strings_are_bounded_without_splitting_characters() {
        let mut r = row("p", "phone", true);
        r.name = "n".repeat(100_000);
        r.media_player = "\u{1f600}".repeat(10_000);
        r.media_title = "t".repeat(100_000);
        r.media_artist = "\u{1f600}".repeat(10_000);
        let phone = present(reduce(&[r]));
        assert_eq!(phone.name.chars().count(), MAX_NAME_UNITS);
        let media = phone.media.expect("media");
        assert_eq!(media.player.encode_utf16().count(), MAX_PLAYER_UNITS);
        assert_eq!(media.title.encode_utf16().count(), MAX_MEDIA_UNITS);
        assert_eq!(media.artist.encode_utf16().count(), MAX_MEDIA_UNITS);
    }

    #[test]
    fn rows_beyond_the_listing_bound_are_ignored() {
        let mut rows: Vec<DeviceRow> = (0..MAX_DEVICES)
            .map(|i| row(&i.to_string(), "phone", false))
            .collect();
        rows.push(row("late", "phone", true));
        assert_eq!(reduce(&rows), PhoneState::Absent);
    }
}
