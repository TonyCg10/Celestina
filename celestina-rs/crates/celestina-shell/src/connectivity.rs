// `next_profile` and its tests (here `ProfileSection::cycle` and
// `cycling_wraps_and_refuses_what_it_cannot_place`) are adapted from
// `celestina-rs/crates/celestina-shell-core/src/power.rs` at
// 02b70221e2f142f4e59238181db4de8710d43333: the list is typed
// (`ProfileName`) instead of `String`, and the `powerprofilesctl` listing
// parsers (`parse_profiles`, `parse_active`) are dropped because the daemon is
// read over D-Bus. Everything else in this file is new (CEL-2-F). Of the
// halted `connectivity.rs` only the idea of an `Expected` outcome is reused;
// none of its `nmcli`/`bluetoothctl` types are.

//! What the bar's network, Bluetooth, battery and power-profile pieces know,
//! and the closed list of things their pages may ask for.
//!
//! The workers (in the product) read NetworkManager, BlueZ, UPower and
//! power-profiles-daemon and publish the sections of this module; the pieces
//! and pages read them. Nothing here does IO and nothing here knows D-Bus.
//!
//! # Contract for the other layers
//!
//! * **Everything from another process is data.** Names, aliases, ids and
//!   addresses are cleaned and bounded on construction ([`clean_text`],
//!   [`RowId::new`], [`BtAddress::parse`], [`ProfileName::new`]). A value of
//!   one of these types is therefore already safe to keep and show; the
//!   constructors are the only way to build one. Text is shown as a name in a
//!   row, never interpreted and never put inside a sentence built from it.
//! * **Sections are bounded and ordered.** A section keeps at most
//!   [`MAX_ROWS`] rows, ids unique, the most relevant rows first (active
//!   before saved before the rest, then by signal or alias). The worker may
//!   hand over any number of rows; the constructor decides which survive.
//! * **A section exists only when its service answered.** "Service absent" is
//!   [`Service::Unavailable`], never an empty section; "no battery" is
//!   `None` from [`BatterySection::new`].
//! * **Actions are a closed enum** ([`Action`]). No variant can carry free
//!   text: ids and addresses are validated types and a request is checked
//!   against the last confirmed [`Inventory`] by [`validate`] before any
//!   worker touches a backend. A passphrase has no place to travel.
//! * **No optimistic state.** After a [`Checked`] request the row is pending
//!   ([`PendingSet`]) until an observation shows the [`Expected`] state or
//!   [`PENDING_WINDOW`] ends, in which case the page shows
//!   [`Failure::Timeout`] for that row.

use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

use crate::bounded::MAX_TEXT_UNITS;

/// Most rows a network or Bluetooth section keeps.
pub const MAX_ROWS: usize = 24;
/// Most candidate rows a constructor looks at before it stops reading input;
/// a hostile service cannot make the shell sort an unbounded list.
pub const MAX_SCANNED_ROWS: usize = 1024;
/// Longest display name, in grapheme clusters (approximated, see
/// [`clean_text`]).
pub const MAX_NAME_CLUSTERS: usize = 48;
/// Longest row id, in bytes.
pub const MAX_ID_BYTES: usize = 128;
/// Most profiles the daemon may list.
pub const MAX_PROFILES: usize = 8;
/// Longest profile name, in bytes.
pub const MAX_PROFILE_BYTES: usize = 32;
/// How long a requested change may stay unconfirmed before it is a failure.
pub const PENDING_WINDOW: Duration = Duration::from_secs(20);
/// Most requests the pending set tracks at once.
pub const MAX_PENDING: usize = 8;
/// Longest remaining or full-charge time the battery section believes.
pub const MAX_BATTERY_TIME: Duration = Duration::from_secs(30 * 24 * 3600);

/// Most characters one grapheme cluster may keep; the rest are dropped so a
/// run of combining marks cannot make one "cluster" unbounded.
const MAX_CLUSTER_CHARS: usize = 8;

// ---------------------------------------------------------------------------
// Text and identity bounds
// ---------------------------------------------------------------------------

/// Which of the four services a section or request is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    /// NetworkManager.
    Network,
    /// BlueZ.
    Bluetooth,
    /// UPower's display device.
    Battery,
    /// power-profiles-daemon.
    Profiles,
}

/// A service's published state: it answered, or it did not (D10). Workers
/// publish `Unavailable` once and retry; pieces draw nothing for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Service<T> {
    /// The service is not on the bus, the bus is absent, or the first read
    /// failed.
    Unavailable,
    /// The service answered; this is its last confirmed state.
    Ready(T),
}

impl<T> Service<T> {
    /// The state when the service answered.
    #[must_use]
    pub fn ready(&self) -> Option<&T> {
        match self {
            Self::Ready(state) => Some(state),
            Self::Unavailable => None,
        }
    }
}

/// Whether `c` is a bidirectional-control character, which can reorder the
/// text around it (a name that renders as another name).
fn is_bidi_control(c: char) -> bool {
    matches!(
        c,
        '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
    )
}

/// Whether `c` renders as nothing on its own and joins nothing a name needs:
/// zero width space, word joiner, byte order mark and soft hyphen. Dropped,
/// so a name made only of them cleans to empty.
fn is_invisible(c: char) -> bool {
    matches!(c, '\u{200b}' | '\u{2060}' | '\u{feff}' | '\u{00ad}')
}

/// Whether `c` extends the cluster before it instead of starting a new one.
/// An approximation of the Unicode rules (marks, variation selectors, joiners,
/// emoji modifiers, tags); this crate carries no segmentation tables, and a
/// cut one cluster early or late is harmless for a display name.
fn extends_cluster(c: char) -> bool {
    matches!(c,
        '\u{0300}'..='\u{036f}'
        | '\u{1ab0}'..='\u{1aff}'
        | '\u{1dc0}'..='\u{1dff}'
        | '\u{20d0}'..='\u{20ff}'
        | '\u{fe00}'..='\u{fe0f}'
        | '\u{fe20}'..='\u{fe2f}'
        | '\u{200c}'
        | '\u{200d}'
        | '\u{1f3fb}'..='\u{1f3ff}'
        | '\u{e0020}'..='\u{e007f}'
        | '\u{e0100}'..='\u{e01ef}')
}

/// Whether `c` is a regional indicator (two of them make one flag).
fn is_regional(c: char) -> bool {
    matches!(c, '\u{1f1e6}'..='\u{1f1ff}')
}

/// Service-supplied display text made safe to keep and show: control
/// characters, bidirectional overrides and invisible characters dropped, a
/// joining character with no base before it dropped, surrounding whitespace
/// trimmed, inner whitespace runs collapsed to one space, and the result cut
/// to at most [`MAX_NAME_CLUSTERS`] grapheme clusters (and, as a second
/// bound, [`MAX_TEXT_UNITS`] UTF-16 units). A name that cleans to nothing is
/// the empty string; the page shows its own placeholder.
///
/// Input is `&str`: bytes that were not UTF-8 are the caller's lossy
/// conversion, and the replacement character it leaves is kept as ordinary
/// text.
#[must_use]
pub fn clean_text(raw: &str) -> String {
    let mut out = String::new();
    let mut clusters = 0;
    let mut in_cluster = 0;
    let mut units = 0;
    let mut after_joiner = false;
    let mut regionals = 0;
    let mut pending_space = false;
    for c in raw.chars() {
        if c.is_whitespace() {
            pending_space = !out.is_empty();
            // Whitespace ends the cluster: nothing after it joins across.
            after_joiner = false;
            regionals = 0;
            continue;
        }
        if c.is_control() || is_bidi_control(c) || is_invisible(c) {
            continue;
        }
        let joins = extends_cluster(c)
            || after_joiner
            || (is_regional(c) && regionals % 2 == 1 && in_cluster > 0);
        if !joins {
            // A new cluster begins; the pending space is its own cluster.
            if pending_space {
                clusters += 1;
                if clusters > MAX_NAME_CLUSTERS {
                    break;
                }
                units += 1;
                out.push(' ');
                pending_space = false;
            }
            clusters += 1;
            if clusters > MAX_NAME_CLUSTERS {
                break;
            }
            in_cluster = 0;
            regionals = 0;
        } else if pending_space || out.is_empty() {
            // A joining character after whitespace, or before any base, has
            // nothing to join.
            continue;
        }
        after_joiner = c == '\u{200d}';
        if in_cluster >= MAX_CLUSTER_CHARS {
            continue;
        }
        units += c.len_utf16();
        if units > MAX_TEXT_UNITS {
            break;
        }
        if is_regional(c) {
            regionals += 1;
        }
        in_cluster += 1;
        out.push(c);
    }
    out
}

/// The stable identity of a network row: a NetworkManager connection UUID or
/// an access-point identity, as the worker builds it. Non-empty, at most
/// [`MAX_ID_BYTES`] bytes, no control characters. The worker compares ids for
/// equality only.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RowId(String);

impl RowId {
    /// `None` when `raw` is empty, longer than [`MAX_ID_BYTES`] or contains a
    /// control character. Ids are never trimmed or altered: an id that is
    /// not exactly what the service said would name another row.
    #[must_use]
    pub fn new(raw: &str) -> Option<Self> {
        let sound =
            !raw.is_empty() && raw.len() <= MAX_ID_BYTES && !raw.chars().any(char::is_control);
        sound.then(|| Self(raw.to_owned()))
    }

    /// The id as text, for a worker to look the row up.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A Bluetooth device address in its canonical form `XX:XX:XX:XX:XX:XX`,
/// upper-case hexadecimal.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BtAddress(String);

impl BtAddress {
    /// Parses `XX:XX:XX:XX:XX:XX` (either hex case); `None` for anything else.
    /// The canonical form is upper case, so two spellings of one address are
    /// equal after parsing.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let bytes = raw.as_bytes();
        let sound = bytes.len() == 17
            && bytes.iter().enumerate().all(|(at, byte)| {
                if at % 3 == 2 {
                    *byte == b':'
                } else {
                    byte.is_ascii_hexdigit()
                }
            });
        sound.then(|| Self(raw.to_ascii_uppercase()))
    }

    /// The canonical text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A power profile as the daemon names it (`performance`, `balanced`,
/// `power-saver`, or a future one): lower-case ASCII letters, digits, `-` and
/// `_`, 1 to [`MAX_PROFILE_BYTES`] bytes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProfileName(String);

impl ProfileName {
    /// `None` when `raw` is outside the grammar above.
    #[must_use]
    pub fn new(raw: &str) -> Option<Self> {
        let sound = !raw.is_empty()
            && raw.len() <= MAX_PROFILE_BYTES
            && raw
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
        sound.then(|| Self(raw.to_owned()))
    }

    /// The name as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Network
// ---------------------------------------------------------------------------

/// What kind of link a network row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetKind {
    /// A wireless access point.
    Wifi,
    /// A wired connection.
    Ethernet,
    /// A VPN. Shown, never activated by the shell.
    Vpn,
}

/// How a network protects itself, as far as the shell cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetSecurity {
    /// No secret needed.
    Open,
    /// WPA/WPA2/WPA3 personal: a passphrase NetworkManager may hold.
    Passphrase,
    /// 802.1X: needs credentials the shell never handles.
    Enterprise,
    /// WEP: obsolete; the shell does not join it.
    Wep,
}

/// Where a network row is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetState {
    /// Not active.
    Disconnected,
    /// Activation in progress.
    Connecting,
    /// Active.
    Connected,
}

impl NetState {
    /// Whether the row is active or activating, so it can be left. A join is
    /// met by it (NetworkManager took the activation); its later outcome is
    /// the network lane's to follow (`Connected`, or a failure by kind).
    #[must_use]
    pub fn is_active(self) -> bool {
        !matches!(self, Self::Disconnected)
    }
}

/// One network row: read-only, bounded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetRow {
    id: RowId,
    kind: NetKind,
    name: String,
    state: NetState,
    signal: u8,
    security: NetSecurity,
    known: bool,
    address: Option<Ipv4Addr>,
}

/// The raw fields of a network row, as a worker read them, before bounds are
/// applied by [`NetRow::new`].
#[derive(Clone, Copy, Debug)]
pub struct NetDraft<'a> {
    /// The stable identity (connection UUID or access-point identity).
    pub id: &'a str,
    /// Wi-Fi, Ethernet or VPN.
    pub kind: NetKind,
    /// The service's display name; cleaned by [`clean_text`].
    pub name: &'a str,
    /// Disconnected, connecting or connected.
    pub state: NetState,
    /// Signal strength; clamped to 0..=100.
    pub signal: u8,
    /// The security class.
    pub security: NetSecurity,
    /// Whether NetworkManager holds a saved profile for it.
    pub known: bool,
    /// The first IPv4 address of the active link.
    pub address: Option<Ipv4Addr>,
}

impl NetRow {
    /// Builds a row from `draft`, cleaning the name with [`clean_text`] and
    /// clamping the signal to 0..=100. `None` when the id is not a valid
    /// [`RowId`].
    #[must_use]
    pub fn new(draft: NetDraft<'_>) -> Option<Self> {
        Some(Self {
            id: RowId::new(draft.id)?,
            kind: draft.kind,
            name: clean_text(draft.name),
            state: draft.state,
            signal: draft.signal.min(100),
            security: draft.security,
            known: draft.known,
            address: draft.address,
        })
    }

    /// The stable identity; what an [`Action`] names.
    #[must_use]
    pub fn id(&self) -> &RowId {
        &self.id
    }
    /// Wi-Fi, Ethernet or VPN.
    #[must_use]
    pub fn kind(&self) -> NetKind {
        self.kind
    }
    /// The cleaned display name; may be empty (hidden network).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Disconnected, connecting or connected.
    #[must_use]
    pub fn state(&self) -> NetState {
        self.state
    }
    /// Signal strength, 0..=100 (0 for a wired link).
    #[must_use]
    pub fn signal(&self) -> u8 {
        self.signal
    }
    /// The security class.
    #[must_use]
    pub fn security(&self) -> NetSecurity {
        self.security
    }
    /// Whether a saved profile exists.
    #[must_use]
    pub fn known(&self) -> bool {
        self.known
    }
    /// The first IPv4 address of the active link, when there is one.
    #[must_use]
    pub fn address(&self) -> Option<Ipv4Addr> {
        self.address
    }
}

/// NetworkManager's published state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetSection {
    wifi_enabled: bool,
    wifi_hardware_blocked: bool,
    rows: Vec<NetRow>,
}

impl NetSection {
    /// Builds the section from whatever rows the worker read: reads at most
    /// [`MAX_SCANNED_ROWS`] (later rows are ignored, whatever they say), drops a repeated id (the first wins), orders by
    /// relevance (active, then saved, then stronger signal, then name, then
    /// id) and keeps [`MAX_ROWS`]. `wifi_hardware_blocked` is the radio kill
    /// switch; the shell never clears it.
    #[must_use]
    pub fn new(
        wifi_enabled: bool,
        wifi_hardware_blocked: bool,
        rows: impl IntoIterator<Item = NetRow>,
    ) -> Self {
        let mut kept: Vec<NetRow> = Vec::new();
        for row in rows.into_iter().take(MAX_SCANNED_ROWS) {
            if !kept.iter().any(|seen| seen.id == row.id) {
                kept.push(row);
            }
        }
        kept.sort_by(|a, b| {
            (!a.state.is_active())
                .cmp(&!b.state.is_active())
                .then_with(|| (!a.known).cmp(&!b.known))
                .then_with(|| b.signal.cmp(&a.signal))
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.id.0.cmp(&b.id.0))
        });
        kept.truncate(MAX_ROWS);
        Self {
            wifi_enabled,
            wifi_hardware_blocked,
            rows: kept,
        }
    }

    /// The Wi-Fi software switch.
    #[must_use]
    pub fn wifi_enabled(&self) -> bool {
        self.wifi_enabled
    }
    /// The Wi-Fi hardware kill switch.
    #[must_use]
    pub fn wifi_hardware_blocked(&self) -> bool {
        self.wifi_hardware_blocked
    }
    /// The rows, most relevant first, at most [`MAX_ROWS`].
    #[must_use]
    pub fn rows(&self) -> &[NetRow] {
        &self.rows
    }
    /// The row with `id`, when the section holds it.
    #[must_use]
    pub fn row(&self, id: &RowId) -> Option<&NetRow> {
        self.rows.iter().find(|row| &row.id == id)
    }
}

// ---------------------------------------------------------------------------
// Bluetooth
// ---------------------------------------------------------------------------

/// A device's icon class, from BlueZ's `Icon` property; anything unknown is
/// [`BtIcon::Other`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BtIcon {
    /// Headphones, headset, speaker and other audio devices.
    Audio,
    /// Keyboard.
    Keyboard,
    /// Mouse, tablet, gamepad and other pointing or input devices.
    Pointer,
    /// Phone.
    Phone,
    /// Computer.
    Computer,
    /// Anything else, including an absent or hostile value.
    Other,
}

impl BtIcon {
    /// Classifies BlueZ's `Icon` text; never fails.
    #[must_use]
    pub fn from_bluez(icon: &str) -> Self {
        match icon {
            "audio-card" | "audio-headphones" | "audio-headset" | "audio-speakers" => Self::Audio,
            "input-keyboard" => Self::Keyboard,
            "input-mouse" | "input-tablet" | "input-gaming" => Self::Pointer,
            "phone" => Self::Phone,
            "computer" => Self::Computer,
            _ => Self::Other,
        }
    }
}

/// One Bluetooth device row: read-only, bounded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BtRow {
    address: BtAddress,
    alias: String,
    icon: BtIcon,
    paired: bool,
    connected: bool,
    trusted: bool,
    battery: Option<u8>,
}

impl BtRow {
    /// Builds a row, cleaning `alias` with [`clean_text`] and clamping
    /// `battery` to 0..=100. `None` when `address` is not a valid
    /// [`BtAddress`].
    #[must_use]
    pub fn new(
        address: &str,
        alias: &str,
        icon: BtIcon,
        paired: bool,
        connected: bool,
        trusted: bool,
        battery: Option<u8>,
    ) -> Option<Self> {
        Some(Self {
            address: BtAddress::parse(address)?,
            alias: clean_text(alias),
            icon,
            paired,
            connected,
            trusted,
            battery: battery.map(|level| level.min(100)),
        })
    }

    /// The identity; what an [`Action`] names.
    #[must_use]
    pub fn address(&self) -> &BtAddress {
        &self.address
    }
    /// The cleaned alias; may be empty.
    #[must_use]
    pub fn alias(&self) -> &str {
        &self.alias
    }
    /// The icon class.
    #[must_use]
    pub fn icon(&self) -> BtIcon {
        self.icon
    }
    /// Whether the device is paired (the shell connects only paired ones).
    #[must_use]
    pub fn paired(&self) -> bool {
        self.paired
    }
    /// Whether the device is connected.
    #[must_use]
    pub fn connected(&self) -> bool {
        self.connected
    }
    /// Whether the device is trusted (shown only).
    #[must_use]
    pub fn trusted(&self) -> bool {
        self.trusted
    }
    /// Battery percentage when `Battery1` reports one, 0..=100.
    #[must_use]
    pub fn battery(&self) -> Option<u8> {
        self.battery
    }
}

/// BlueZ's published state. It exists only when an adapter does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BtSection {
    powered: bool,
    discovering: bool,
    rows: Vec<BtRow>,
}

impl BtSection {
    /// Builds the section: reads at most [`MAX_SCANNED_ROWS`], drops a
    /// repeated address (first wins), orders connected, then paired, then by
    /// alias, then address, and keeps [`MAX_ROWS`]. `discovering` is shown,
    /// never started by the shell.
    #[must_use]
    pub fn new(powered: bool, discovering: bool, rows: impl IntoIterator<Item = BtRow>) -> Self {
        let mut kept: Vec<BtRow> = Vec::new();
        for row in rows.into_iter().take(MAX_SCANNED_ROWS) {
            if !kept.iter().any(|seen| seen.address == row.address) {
                kept.push(row);
            }
        }
        kept.sort_by(|a, b| {
            (!a.connected)
                .cmp(&!b.connected)
                .then_with(|| (!a.paired).cmp(&!b.paired))
                .then_with(|| a.alias.cmp(&b.alias))
                .then_with(|| a.address.cmp(&b.address))
        });
        kept.truncate(MAX_ROWS);
        Self {
            powered,
            discovering,
            rows: kept,
        }
    }

    /// The adapter's `Powered`.
    #[must_use]
    pub fn powered(&self) -> bool {
        self.powered
    }
    /// The adapter's `Discovering`.
    #[must_use]
    pub fn discovering(&self) -> bool {
        self.discovering
    }
    /// The devices, at most [`MAX_ROWS`].
    #[must_use]
    pub fn rows(&self) -> &[BtRow] {
        &self.rows
    }
    /// The device with `address`, when the section holds it.
    #[must_use]
    pub fn row(&self, address: &BtAddress) -> Option<&BtRow> {
        self.rows.iter().find(|row| &row.address == address)
    }
}

// ---------------------------------------------------------------------------
// Battery
// ---------------------------------------------------------------------------

/// UPower's `State` for the display device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChargeState {
    /// Value 0, or one this shell does not know.
    Unknown,
    /// Charging.
    Charging,
    /// Discharging.
    Discharging,
    /// Empty.
    Empty,
    /// Fully charged.
    Full,
    /// Plugged in but waiting to charge.
    PendingCharge,
    /// Unplugged but waiting to discharge.
    PendingDischarge,
}

impl ChargeState {
    /// Maps UPower's numeric `State`; anything outside 1..=6 is `Unknown`.
    #[must_use]
    pub fn from_upower(state: u32) -> Self {
        match state {
            1 => Self::Charging,
            2 => Self::Discharging,
            3 => Self::Empty,
            4 => Self::Full,
            5 => Self::PendingCharge,
            6 => Self::PendingDischarge,
            _ => Self::Unknown,
        }
    }
}

/// UPower's display device, when a battery is present.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatterySection {
    state: ChargeState,
    percentage: u8,
    time_to_empty: Option<Duration>,
    time_to_full: Option<Duration>,
}

impl BatterySection {
    /// `None` when no battery is present (`is_present` false) or the
    /// percentage is not a finite number. The percentage is rounded and
    /// clamped to 0..=100; a time of zero or less, or beyond
    /// [`MAX_BATTERY_TIME`], is `None` (UPower says 0 for "unknown").
    #[must_use]
    pub fn new(
        is_present: bool,
        state: ChargeState,
        percentage: f64,
        time_to_empty_secs: i64,
        time_to_full_secs: i64,
    ) -> Option<Self> {
        if !is_present || !percentage.is_finite() {
            return None;
        }
        let time = |secs: i64| {
            u64::try_from(secs)
                .ok()
                .filter(|secs| *secs > 0)
                .map(Duration::from_secs)
                .filter(|time| *time <= MAX_BATTERY_TIME)
        };
        // Clamped to 0..=100 first, so the cast cannot truncate or wrap.
        let percentage = percentage.round().clamp(0.0, 100.0) as u8;
        Some(Self {
            state,
            percentage,
            time_to_empty: time(time_to_empty_secs),
            time_to_full: time(time_to_full_secs),
        })
    }

    /// The charge state.
    #[must_use]
    pub fn state(&self) -> ChargeState {
        self.state
    }
    /// Charge, 0..=100.
    #[must_use]
    pub fn percentage(&self) -> u8 {
        self.percentage
    }
    /// Time until empty while discharging, when known.
    #[must_use]
    pub fn time_to_empty(&self) -> Option<Duration> {
        self.time_to_empty
    }
    /// Time until full while charging, when known.
    #[must_use]
    pub fn time_to_full(&self) -> Option<Duration> {
        self.time_to_full
    }
}

// ---------------------------------------------------------------------------
// Power profiles
// ---------------------------------------------------------------------------

/// Why the daemon says performance is degraded (`PerformanceDegraded`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Degraded {
    /// Not degraded (the daemon's empty string).
    No,
    /// `lap-detected`.
    LapDetected,
    /// `high-operating-temperature`.
    HighTemperature,
    /// A reason this shell does not know.
    Other,
}

impl Degraded {
    /// Classifies the daemon's text; never fails.
    #[must_use]
    pub fn from_daemon(reason: &str) -> Self {
        match reason {
            "" => Self::No,
            "lap-detected" => Self::LapDetected,
            "high-operating-temperature" => Self::HighTemperature,
            _ => Self::Other,
        }
    }
}

/// power-profiles-daemon's published state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileSection {
    active: Option<ProfileName>,
    profiles: Vec<ProfileName>,
    degraded: Degraded,
}

impl ProfileSection {
    /// Builds the section from the daemon's own text: profiles that are not
    /// valid [`ProfileName`]s are dropped, repeats too, in the daemon's
    /// order, at most [`MAX_PROFILES`] (the scan stops after
    /// [`MAX_SCANNED_ROWS`] entries). `active` is `None` when the daemon's
    /// value is not a valid name; it may be a profile the list lacks, which
    /// [`ProfileSection::cycle`] and [`validate`] treat as a place to cycle
    /// from nowhere.
    #[must_use]
    pub fn new<S: AsRef<str>>(
        active: &str,
        profiles: impl IntoIterator<Item = S>,
        degraded: Degraded,
    ) -> Self {
        let mut kept: Vec<ProfileName> = Vec::new();
        for raw in profiles.into_iter().take(MAX_SCANNED_ROWS) {
            if let Some(name) = ProfileName::new(raw.as_ref()) {
                if !kept.contains(&name) {
                    kept.push(name);
                    if kept.len() == MAX_PROFILES {
                        break;
                    }
                }
            }
        }
        Self {
            active: ProfileName::new(active),
            profiles: kept,
            degraded,
        }
    }

    /// The active profile, when the daemon named a valid one.
    #[must_use]
    pub fn active(&self) -> Option<&ProfileName> {
        self.active.as_ref()
    }
    /// The profiles the daemon offers, in its order.
    #[must_use]
    pub fn profiles(&self) -> &[ProfileName] {
        &self.profiles
    }
    /// Why performance is degraded, if it is.
    #[must_use]
    pub fn degraded(&self) -> Degraded {
        self.degraded
    }

    /// The profile to ask for next, wrapping. `None` when there is nothing to
    /// cycle through (fewer than two profiles), or when the active one is not
    /// among them: a daemon reporting a profile it does not offer is not one
    /// to guess for.
    #[must_use]
    pub fn cycle(&self) -> Option<&ProfileName> {
        if self.profiles.len() < 2 {
            return None;
        }
        let at = self
            .profiles
            .iter()
            .position(|profile| Some(profile) == self.active.as_ref())?;
        self.profiles.get((at + 1) % self.profiles.len())
    }
}

// ---------------------------------------------------------------------------
// Actions and their validation
// ---------------------------------------------------------------------------

/// The editing UI an `OpenEditor` request launches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Editor {
    /// `nm-connection-editor`.
    Network,
    /// `blueman-manager`.
    Bluetooth,
}

impl Editor {
    /// The fixed program name, looked up on `PATH` by the caller; never
    /// built from data and always run without arguments from data.
    #[must_use]
    pub fn program(self) -> &'static str {
        match self {
            Self::Network => "nm-connection-editor",
            Self::Bluetooth => "blueman-manager",
        }
    }
}

/// Everything a page may ask for (design D3). A closed list: no variant
/// carries free text, so no secret has a place to travel, and nothing can
/// pair, forget, scan, edit a profile or touch a VPN. Every request is
/// checked by [`validate`] before a worker acts on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Re-read one section.
    Refresh(Section),
    /// Turn the Wi-Fi radio on or off.
    SetWifiEnabled(bool),
    /// Activate the saved profile of a Wi-Fi or Ethernet row.
    JoinSaved(RowId),
    /// Deactivate the active Wi-Fi or Ethernet row.
    Leave(RowId),
    /// Power the Bluetooth adapter on or off.
    SetBluetoothPowered(bool),
    /// Connect a paired device.
    ConnectDevice(BtAddress),
    /// Disconnect a paired device.
    DisconnectDevice(BtAddress),
    /// Ask the daemon for a listed profile.
    SetProfile(ProfileName),
    /// Ask the daemon for the next listed profile.
    CycleProfile,
    /// Launch the fixed editing UI.
    OpenEditor(Editor),
}

/// What a request was refused for. Typed so the page can say why by kind;
/// the product's sentence comes from `copy.rs`, never from service text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The service's state is not known yet (or the service is absent).
    NoInventory(Section),
    /// The id or address is not in the last confirmed inventory.
    UnknownId,
    /// The row has no saved profile to join.
    NotKnown,
    /// The row's kind is not allowed for this action (a VPN).
    KindNotAllowed,
    /// Enterprise and WEP networks need credentials the shell never handles.
    NeedsSecretClass,
    /// Leave was asked for a row that is not active.
    NotActive,
    /// A device request for a device that is not paired.
    NotPaired,
    /// Joining a Wi-Fi row while the radio is off.
    WifiDisabled,
    /// Turning Wi-Fi on while the hardware kill switch is set.
    HardwareBlocked,
    /// The profile is not in the daemon's `Profiles`.
    NoSuchProfile,
    /// `CycleProfile` with fewer than two profiles or an unlisted active one.
    NothingToCycle,
}

impl Refusal {
    /// A stable token for the diagnostics journal (English, no data).
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::NoInventory(_) => "no-inventory",
            Self::UnknownId => "unknown-id",
            Self::NotKnown => "not-known",
            Self::KindNotAllowed => "kind-not-allowed",
            Self::NeedsSecretClass => "needs-secret-class",
            Self::NotActive => "not-active",
            Self::NotPaired => "not-paired",
            Self::WifiDisabled => "wifi-disabled",
            Self::HardwareBlocked => "hardware-blocked",
            Self::NoSuchProfile => "no-such-profile",
            Self::NothingToCycle => "nothing-to-cycle",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "request refused: {}", self.key())
    }
}

impl std::error::Error for Refusal {}

/// The last confirmed state of each service, borrowed. A section is `None`
/// when that service is unavailable or has not answered yet.
#[derive(Clone, Copy, Debug, Default)]
pub struct Inventory<'a> {
    /// NetworkManager.
    pub network: Option<&'a NetSection>,
    /// BlueZ.
    pub bluetooth: Option<&'a BtSection>,
    /// power-profiles-daemon.
    pub profiles: Option<&'a ProfileSection>,
}

/// A request that passed [`validate`]: what the worker may do, with
/// `CycleProfile` already resolved to a concrete profile. Only [`validate`]
/// builds one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    step: Step,
}

/// The concrete step of a [`Checked`] request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Re-read a section.
    Refresh(Section),
    /// Set the Wi-Fi radio.
    SetWifiEnabled(bool),
    /// Activate the saved profile of this row. Call the backend with no
    /// secret (`None`).
    Join(RowId),
    /// Deactivate this row.
    Leave(RowId),
    /// Set the adapter's `Powered`.
    SetBluetoothPowered(bool),
    /// Connect this paired device.
    ConnectDevice(BtAddress),
    /// Disconnect this paired device.
    DisconnectDevice(BtAddress),
    /// Set the daemon's `ActiveProfile` to this listed profile.
    SetProfile(ProfileName),
    /// Launch this editor.
    OpenEditor(Editor),
}

impl Checked {
    /// What to do.
    #[must_use]
    pub fn step(&self) -> &Step {
        &self.step
    }

    /// The step, by value.
    #[must_use]
    pub fn into_step(self) -> Step {
        self.step
    }

    /// The observation that will confirm this request, for the pending
    /// window; `None` for requests with nothing to wait for (a refresh, an
    /// editor).
    #[must_use]
    pub fn expected(&self) -> Option<Expected> {
        match &self.step {
            Step::SetWifiEnabled(on) => Some(Expected::WifiEnabled(*on)),
            Step::Join(id) => Some(Expected::NetActive {
                id: id.clone(),
                active: true,
            }),
            Step::Leave(id) => Some(Expected::NetActive {
                id: id.clone(),
                active: false,
            }),
            Step::SetBluetoothPowered(on) => Some(Expected::BtPowered(*on)),
            Step::ConnectDevice(address) => Some(Expected::BtConnected {
                address: address.clone(),
                connected: true,
            }),
            Step::DisconnectDevice(address) => Some(Expected::BtConnected {
                address: address.clone(),
                connected: false,
            }),
            Step::SetProfile(name) => Some(Expected::Profile(name.clone())),
            Step::Refresh(_) | Step::OpenEditor(_) => None,
        }
    }
}

fn net<'a>(inventory: &Inventory<'a>) -> Result<&'a NetSection, Refusal> {
    inventory
        .network
        .ok_or(Refusal::NoInventory(Section::Network))
}

fn bluetooth<'a>(inventory: &Inventory<'a>) -> Result<&'a BtSection, Refusal> {
    inventory
        .bluetooth
        .ok_or(Refusal::NoInventory(Section::Bluetooth))
}

fn profiles<'a>(inventory: &Inventory<'a>) -> Result<&'a ProfileSection, Refusal> {
    inventory
        .profiles
        .ok_or(Refusal::NoInventory(Section::Profiles))
}

/// A row the shell may activate or deactivate: Wi-Fi or Ethernet.
fn net_row<'a>(section: &'a NetSection, id: &RowId) -> Result<&'a NetRow, Refusal> {
    let row = section.row(id).ok_or(Refusal::UnknownId)?;
    if row.kind() == NetKind::Vpn {
        return Err(Refusal::KindNotAllowed);
    }
    Ok(row)
}

/// Checks `action` against the last confirmed `inventory` and returns the
/// concrete step, or why it was refused. Nothing is refused for being a no-op:
/// a request whose state already holds is idempotent and its pending entry
/// resolves on the next observation. A refused request must be dropped with a
/// journal line (`Refusal::key`) and never reach a backend.
///
/// # Errors
///
/// A [`Refusal`] naming the first rule the request broke; see its variants.
pub fn validate(action: &Action, inventory: &Inventory<'_>) -> Result<Checked, Refusal> {
    let step = match action {
        Action::Refresh(section) => Step::Refresh(*section),
        Action::SetWifiEnabled(on) => {
            let section = net(inventory)?;
            if *on && section.wifi_hardware_blocked() {
                return Err(Refusal::HardwareBlocked);
            }
            Step::SetWifiEnabled(*on)
        }
        Action::JoinSaved(id) => {
            let section = net(inventory)?;
            let row = net_row(section, id)?;
            if !row.known() {
                return Err(Refusal::NotKnown);
            }
            if matches!(row.security(), NetSecurity::Enterprise | NetSecurity::Wep) {
                return Err(Refusal::NeedsSecretClass);
            }
            if row.kind() == NetKind::Wifi && !section.wifi_enabled() {
                return Err(Refusal::WifiDisabled);
            }
            Step::Join(id.clone())
        }
        Action::Leave(id) => {
            let row = net_row(net(inventory)?, id)?;
            if !row.state().is_active() {
                return Err(Refusal::NotActive);
            }
            Step::Leave(id.clone())
        }
        Action::SetBluetoothPowered(on) => {
            bluetooth(inventory)?;
            Step::SetBluetoothPowered(*on)
        }
        Action::ConnectDevice(address) | Action::DisconnectDevice(address) => {
            let row = bluetooth(inventory)?
                .row(address)
                .ok_or(Refusal::UnknownId)?;
            if !row.paired() {
                return Err(Refusal::NotPaired);
            }
            if matches!(action, Action::ConnectDevice(_)) {
                Step::ConnectDevice(address.clone())
            } else {
                Step::DisconnectDevice(address.clone())
            }
        }
        Action::SetProfile(name) => {
            if !profiles(inventory)?.profiles().contains(name) {
                return Err(Refusal::NoSuchProfile);
            }
            Step::SetProfile(name.clone())
        }
        Action::CycleProfile => {
            let next = profiles(inventory)?
                .cycle()
                .ok_or(Refusal::NothingToCycle)?;
            Step::SetProfile(next.clone())
        }
        Action::OpenEditor(editor) => Step::OpenEditor(*editor),
    };
    Ok(Checked { step })
}

// ---------------------------------------------------------------------------
// Pending window
// ---------------------------------------------------------------------------

/// Why a requested change did not happen, by kind. Workers map their backend
/// errors onto these; the page picks its Spanish sentence by variant. Detail
/// (service text) goes to the journal only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// [`PENDING_WINDOW`] ended with no confirming observation.
    Timeout,
    /// The saved profile needs a secret the system does not hold; offer the
    /// editor.
    NeedsSecret,
    /// The system denied the change (polkit or the service).
    Denied,
    /// The device or network is gone or out of range.
    NotFound,
    /// The service went away.
    Unreachable,
    /// The worker's queue was full.
    Busy,
    /// Refused by [`validate`] before any call.
    Refused(Refusal),
    /// Any other backend error.
    Other,
}

/// The state a request waits to observe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expected {
    /// Wi-Fi software switch equals this.
    WifiEnabled(bool),
    /// The row is active (`active`) or not.
    NetActive {
        /// The row.
        id: RowId,
        /// Wanted activity.
        active: bool,
    },
    /// The adapter's `Powered` equals this.
    BtPowered(bool),
    /// The device's `Connected` equals this.
    BtConnected {
        /// The device.
        address: BtAddress,
        /// Wanted connection state.
        connected: bool,
    },
    /// The daemon's active profile is this.
    Profile(ProfileName),
}

/// The thing an [`Expected`] is about; a new request for a subject replaces
/// the older pending one.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Subject {
    Wifi,
    Net(RowId),
    Powered,
    Device(BtAddress),
    Profile,
}

impl Expected {
    fn subject(&self) -> Subject {
        match self {
            Self::WifiEnabled(_) => Subject::Wifi,
            Self::NetActive { id, .. } => Subject::Net(id.clone()),
            Self::BtPowered(_) => Subject::Powered,
            Self::BtConnected { address, .. } => Subject::Device(address.clone()),
            Self::Profile(_) => Subject::Profile,
        }
    }

    /// Whether `inventory` shows the expected state. A section that is gone
    /// or a row that vanished never counts as met.
    #[must_use]
    pub fn is_met(&self, inventory: &Inventory<'_>) -> bool {
        match self {
            Self::WifiEnabled(on) => inventory
                .network
                .is_some_and(|net| net.wifi_enabled() == *on),
            Self::NetActive { id, active } => inventory
                .network
                .and_then(|net| net.row(id))
                .is_some_and(|row| row.state().is_active() == *active),
            Self::BtPowered(on) => inventory.bluetooth.is_some_and(|bt| bt.powered() == *on),
            Self::BtConnected { address, connected } => inventory
                .bluetooth
                .and_then(|bt| bt.row(address))
                .is_some_and(|row| row.connected() == *connected),
            Self::Profile(name) => inventory
                .profiles
                .is_some_and(|section| section.active() == Some(name)),
        }
    }
}

/// How one pending request ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// A later observation showed the expected state.
    Confirmed(Expected),
    /// The request failed, observed or reported; show the failure on its row.
    Failed(Expected, Failure),
}

/// The requests still waiting for their confirming observation. Pure: the
/// caller passes the clock, so tests need no sleeping. Holds at most
/// [`MAX_PENDING`] entries; a request for the subject of an existing entry
/// replaces it, and when full the oldest entry is dropped.
#[derive(Clone, Debug, Default)]
pub struct PendingSet {
    entries: Vec<(Expected, Instant)>,
}

impl PendingSet {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts waiting for `expected` from `now`.
    pub fn start(&mut self, expected: Expected, now: Instant) {
        let subject = expected.subject();
        self.entries.retain(|(held, _)| held.subject() != subject);
        if self.entries.len() == MAX_PENDING {
            self.entries.remove(0);
        }
        self.entries.push((expected, now));
    }

    /// Whether `expected` is still waited for; pages draw the row pending
    /// while this holds.
    #[must_use]
    pub fn is_pending(&self, expected: &Expected) -> bool {
        self.entries.iter().any(|(held, _)| held == expected)
    }

    /// Whether anything is pending (a worker keeps its fast cadence while so).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The earliest moment an entry times out, so a worker can sleep until
    /// then; `None` when nothing is pending.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        self.entries
            .iter()
            .map(|(_, since)| *since + PENDING_WINDOW)
            .min()
    }

    /// Records that the backend reported `failure` for the pending entry of
    /// `expected`'s subject; the entry resolves as failed. `None` when
    /// nothing was pending for it.
    pub fn fail(&mut self, expected: &Expected, failure: Failure) -> Option<Resolution> {
        let subject = expected.subject();
        let at = self
            .entries
            .iter()
            .position(|(held, _)| held.subject() == subject)?;
        let (held, _) = self.entries.remove(at);
        Some(Resolution::Failed(held, failure))
    }

    /// Compares every entry with a new observation: met entries are
    /// confirmed, entries older than [`PENDING_WINDOW`] at `now` fail with
    /// [`Failure::Timeout`], the rest stay. Returns the resolutions in the
    /// order the entries were started.
    pub fn observe(&mut self, inventory: &Inventory<'_>, now: Instant) -> Vec<Resolution> {
        let mut resolved = Vec::new();
        self.entries.retain(|(expected, since)| {
            if expected.is_met(inventory) {
                resolved.push(Resolution::Confirmed(expected.clone()));
                false
            } else if now.saturating_duration_since(*since) >= PENDING_WINDOW {
                resolved.push(Resolution::Failed(expected.clone(), Failure::Timeout));
                false
            } else {
                true
            }
        });
        resolved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: &str) -> RowId {
        RowId::new(raw).expect("valid id")
    }

    fn addr(raw: &str) -> BtAddress {
        BtAddress::parse(raw).expect("valid address")
    }

    fn profile(raw: &str) -> ProfileName {
        ProfileName::new(raw).expect("valid profile")
    }

    fn net_row_of(
        raw_id: &str,
        kind: NetKind,
        state: NetState,
        security: NetSecurity,
        known: bool,
    ) -> NetRow {
        NetRow::new(NetDraft {
            id: raw_id,
            kind,
            name: raw_id,
            state,
            signal: 50,
            security,
            known,
            address: None,
        })
        .expect("row")
    }

    fn bt_row_of(address: &str, paired: bool, connected: bool) -> BtRow {
        BtRow::new(
            address,
            "dev",
            BtIcon::Other,
            paired,
            connected,
            false,
            None,
        )
        .expect("row")
    }

    fn network() -> NetSection {
        NetSection::new(
            true,
            false,
            [
                net_row_of(
                    "home",
                    NetKind::Wifi,
                    NetState::Connected,
                    NetSecurity::Passphrase,
                    true,
                ),
                net_row_of(
                    "cafe",
                    NetKind::Wifi,
                    NetState::Disconnected,
                    NetSecurity::Passphrase,
                    true,
                ),
                net_row_of(
                    "stranger",
                    NetKind::Wifi,
                    NetState::Disconnected,
                    NetSecurity::Passphrase,
                    false,
                ),
                net_row_of(
                    "office",
                    NetKind::Wifi,
                    NetState::Disconnected,
                    NetSecurity::Enterprise,
                    true,
                ),
                net_row_of(
                    "old",
                    NetKind::Wifi,
                    NetState::Disconnected,
                    NetSecurity::Wep,
                    true,
                ),
                net_row_of(
                    "vpn",
                    NetKind::Vpn,
                    NetState::Disconnected,
                    NetSecurity::Open,
                    true,
                ),
                net_row_of(
                    "wire",
                    NetKind::Ethernet,
                    NetState::Disconnected,
                    NetSecurity::Open,
                    true,
                ),
            ],
        )
    }

    fn bluetooth_section() -> BtSection {
        BtSection::new(
            true,
            false,
            [
                bt_row_of("AA:BB:CC:DD:EE:01", true, true),
                bt_row_of("AA:BB:CC:DD:EE:02", true, false),
                bt_row_of("AA:BB:CC:DD:EE:03", false, false),
            ],
        )
    }

    fn profile_section() -> ProfileSection {
        ProfileSection::new(
            "balanced",
            ["performance", "balanced", "power-saver"],
            Degraded::No,
        )
    }

    struct World {
        net: NetSection,
        bt: BtSection,
        profiles: ProfileSection,
    }

    impl World {
        fn new() -> Self {
            Self {
                net: network(),
                bt: bluetooth_section(),
                profiles: profile_section(),
            }
        }
        fn inventory(&self) -> Inventory<'_> {
            Inventory {
                network: Some(&self.net),
                bluetooth: Some(&self.bt),
                profiles: Some(&self.profiles),
            }
        }
    }

    fn refused(action: &Action) -> Refusal {
        validate(action, &World::new().inventory()).expect_err("must be refused")
    }

    #[test]
    fn every_refusal_has_its_cause() {
        assert_eq!(refused(&Action::JoinSaved(id("ghost"))), Refusal::UnknownId);
        assert_eq!(
            refused(&Action::JoinSaved(id("stranger"))),
            Refusal::NotKnown
        );
        assert_eq!(
            refused(&Action::JoinSaved(id("vpn"))),
            Refusal::KindNotAllowed
        );
        assert_eq!(refused(&Action::Leave(id("vpn"))), Refusal::KindNotAllowed);
        assert_eq!(
            refused(&Action::JoinSaved(id("office"))),
            Refusal::NeedsSecretClass
        );
        assert_eq!(
            refused(&Action::JoinSaved(id("old"))),
            Refusal::NeedsSecretClass
        );
        assert_eq!(refused(&Action::Leave(id("cafe"))), Refusal::NotActive);
        assert_eq!(refused(&Action::Leave(id("ghost"))), Refusal::UnknownId);
        assert_eq!(
            refused(&Action::ConnectDevice(addr("AA:BB:CC:DD:EE:03"))),
            Refusal::NotPaired
        );
        assert_eq!(
            refused(&Action::DisconnectDevice(addr("AA:BB:CC:DD:EE:03"))),
            Refusal::NotPaired
        );
        assert_eq!(
            refused(&Action::ConnectDevice(addr("AA:BB:CC:DD:EE:99"))),
            Refusal::UnknownId
        );
        assert_eq!(
            refused(&Action::SetProfile(profile("turbo"))),
            Refusal::NoSuchProfile
        );
    }

    #[test]
    fn wifi_switch_rules() {
        let mut world = World::new();
        world.net = NetSection::new(false, true, world.net.rows().to_vec());
        let inventory = world.inventory();
        assert_eq!(
            validate(&Action::SetWifiEnabled(true), &inventory),
            Err(Refusal::HardwareBlocked)
        );
        // Turning it off under a kill switch is a no-op, not a refusal.
        assert!(validate(&Action::SetWifiEnabled(false), &inventory).is_ok());
        // Joining Wi-Fi with the radio off is refused; Ethernet is not.
        assert_eq!(
            validate(&Action::JoinSaved(id("cafe")), &inventory),
            Err(Refusal::WifiDisabled)
        );
        assert!(validate(&Action::JoinSaved(id("wire")), &inventory).is_ok());
    }

    #[test]
    fn a_missing_inventory_refuses_everything_that_needs_it() {
        let empty = Inventory::default();
        assert_eq!(
            validate(&Action::SetWifiEnabled(true), &empty),
            Err(Refusal::NoInventory(Section::Network))
        );
        assert_eq!(
            validate(&Action::JoinSaved(id("home")), &empty),
            Err(Refusal::NoInventory(Section::Network))
        );
        assert_eq!(
            validate(&Action::SetBluetoothPowered(true), &empty),
            Err(Refusal::NoInventory(Section::Bluetooth))
        );
        assert_eq!(
            validate(&Action::ConnectDevice(addr("AA:BB:CC:DD:EE:01")), &empty),
            Err(Refusal::NoInventory(Section::Bluetooth))
        );
        assert_eq!(
            validate(&Action::CycleProfile, &empty),
            Err(Refusal::NoInventory(Section::Profiles))
        );
        // Re-reading and opening an editor need no inventory.
        assert!(validate(&Action::Refresh(Section::Battery), &empty).is_ok());
        assert!(validate(&Action::OpenEditor(Editor::Network), &empty).is_ok());
    }

    #[test]
    fn allowed_requests_become_concrete_steps() {
        let world = World::new();
        let inventory = world.inventory();
        let step = |action: Action| validate(&action, &inventory).expect("allowed").into_step();
        assert_eq!(step(Action::JoinSaved(id("cafe"))), Step::Join(id("cafe")));
        assert_eq!(step(Action::Leave(id("home"))), Step::Leave(id("home")));
        assert_eq!(
            step(Action::ConnectDevice(addr("aa:bb:cc:dd:ee:02"))),
            Step::ConnectDevice(addr("AA:BB:CC:DD:EE:02"))
        );
        assert_eq!(
            step(Action::SetProfile(profile("performance"))),
            Step::SetProfile(profile("performance"))
        );
        assert_eq!(
            step(Action::CycleProfile),
            Step::SetProfile(profile("power-saver"))
        );
        assert_eq!(
            step(Action::SetBluetoothPowered(false)),
            Step::SetBluetoothPowered(false)
        );
    }

    #[test]
    fn expected_follows_the_step() {
        let world = World::new();
        let inventory = world.inventory();
        let expected = |action: Action| validate(&action, &inventory).expect("ok").expected();
        assert_eq!(
            expected(Action::JoinSaved(id("cafe"))),
            Some(Expected::NetActive {
                id: id("cafe"),
                active: true
            })
        );
        assert_eq!(
            expected(Action::Leave(id("home"))),
            Some(Expected::NetActive {
                id: id("home"),
                active: false
            })
        );
        assert_eq!(
            expected(Action::CycleProfile),
            Some(Expected::Profile(profile("power-saver")))
        );
        assert_eq!(expected(Action::Refresh(Section::Network)), None);
        assert_eq!(expected(Action::OpenEditor(Editor::Bluetooth)), None);
    }

    /// What an `Action` variant may carry. A new variant must be classified
    /// here (the match is exhaustive), and none may be free text: a secret
    /// would have nowhere to travel.
    #[derive(Debug, PartialEq, Eq)]
    enum Payload {
        Nothing,
        Flag,
        Section,
        ValidatedId,
        ValidatedAddress,
        ValidatedProfile,
        FixedEditor,
    }

    fn payload(action: &Action) -> Payload {
        match action {
            Action::Refresh(_) => Payload::Section,
            Action::SetWifiEnabled(_) | Action::SetBluetoothPowered(_) => Payload::Flag,
            Action::JoinSaved(_) | Action::Leave(_) => Payload::ValidatedId,
            Action::ConnectDevice(_) | Action::DisconnectDevice(_) => Payload::ValidatedAddress,
            Action::SetProfile(_) => Payload::ValidatedProfile,
            Action::CycleProfile => Payload::Nothing,
            Action::OpenEditor(_) => Payload::FixedEditor,
        }
    }

    #[test]
    fn no_action_carries_free_text() {
        let all = [
            Action::Refresh(Section::Network),
            Action::SetWifiEnabled(true),
            Action::JoinSaved(id("a")),
            Action::Leave(id("a")),
            Action::SetBluetoothPowered(true),
            Action::ConnectDevice(addr("AA:BB:CC:DD:EE:01")),
            Action::DisconnectDevice(addr("AA:BB:CC:DD:EE:01")),
            Action::SetProfile(profile("balanced")),
            Action::CycleProfile,
            Action::OpenEditor(Editor::Network),
        ];
        // Ten variants; a new one fails the exhaustive `payload` match first.
        assert_eq!(all.len(), 10);
        for action in &all {
            // The only classes are fixed or validated grammars; there is no
            // `Text` class to construct.
            assert!(matches!(
                payload(action),
                Payload::Nothing
                    | Payload::Flag
                    | Payload::Section
                    | Payload::ValidatedId
                    | Payload::ValidatedAddress
                    | Payload::ValidatedProfile
                    | Payload::FixedEditor
            ));
        }
        // The validated types reject what a passphrase looks like.
        assert!(BtAddress::parse("correct horse battery").is_none());
        assert!(ProfileName::new("Hunter2 passphrase!").is_none());
        assert!(RowId::new("line\nbreak").is_none());
    }

    #[test]
    fn editors_are_fixed_programs() {
        assert_eq!(Editor::Network.program(), "nm-connection-editor");
        assert_eq!(Editor::Bluetooth.program(), "blueman-manager");
    }

    #[test]
    fn identity_grammars() {
        assert!(RowId::new("").is_none());
        assert!(RowId::new(&"x".repeat(MAX_ID_BYTES)).is_some());
        assert!(RowId::new(&"x".repeat(MAX_ID_BYTES + 1)).is_none());
        assert!(RowId::new("a\u{0}b").is_none());
        assert!(RowId::new("a\tb").is_none());
        // Ids are exact: a leading space is part of the id, not trimmed away.
        assert_eq!(RowId::new(" a").expect("kept").as_str(), " a");

        assert!(BtAddress::parse("AA:BB:CC:DD:EE:FF").is_some());
        assert_eq!(
            BtAddress::parse("aa:bb:cc:dd:ee:ff").expect("hex").as_str(),
            "AA:BB:CC:DD:EE:FF"
        );
        for bad in [
            "",
            "AA:BB:CC:DD:EE",
            "AA:BB:CC:DD:EE:FF:00",
            "AA-BB-CC-DD-EE-FF",
            "AA:BB:CC:DD:EE:GG",
            "AA:BB:CC:DD:EE:F\u{ff}",
            "AA:BB:CC:DD:EE:FF\n",
        ] {
            assert!(BtAddress::parse(bad).is_none(), "{bad:?}");
        }

        assert!(ProfileName::new("power-saver").is_some());
        assert!(ProfileName::new("Balanced").is_none());
        assert!(ProfileName::new("").is_none());
        assert!(ProfileName::new(&"p".repeat(MAX_PROFILE_BYTES + 1)).is_none());
    }

    #[test]
    fn text_is_cleaned_and_cut() {
        assert_eq!(clean_text("  Home\t Wi-Fi \n"), "Home Wi-Fi");
        assert_eq!(clean_text("a\u{0}b\u{7}c"), "abc");
        // Bidirectional overrides cannot reorder a name.
        assert_eq!(clean_text("evil\u{202e}gpj.exe"), "evilgpj.exe");
        assert_eq!(clean_text("\u{0}\u{1}"), "");
        let long = "n".repeat(500);
        assert_eq!(clean_text(&long).chars().count(), MAX_NAME_CLUSTERS);
    }

    #[test]
    fn clusters_are_not_split() {
        // 'e' + combining acute is one cluster; 48 of them fit, 49 do not.
        let accented = "e\u{301}".repeat(60);
        let cut = clean_text(&accented);
        assert_eq!(cut.chars().count(), MAX_NAME_CLUSTERS * 2);
        assert!(!cut.ends_with('e'));
        // A family joined with ZWJ is one cluster.
        let family = "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}";
        assert_eq!(clean_text(family), family);
        // A flag is one cluster of two regional indicators.
        let flag = "\u{1f1ea}\u{1f1f8}";
        assert_eq!(clean_text(&flag.repeat(10)), flag.repeat(10));
        assert_eq!(
            clean_text(&flag.repeat(60)).chars().count(),
            MAX_NAME_CLUSTERS * 2
        );
    }

    #[test]
    fn whitespace_ends_a_joined_or_flagged_cluster() {
        // A joiner before a space joins nothing past it.
        assert_eq!(clean_text("Home\u{200d} Net"), "Home\u{200d} Net");
        assert_eq!(clean_text("A\u{200d} B C"), "A\u{200d} B C");
        // A lone regional indicator does not pair with one past a space.
        assert_eq!(clean_text("\u{1f1ea} \u{1f1f8} x"), "\u{1f1ea} \u{1f1f8} x");
    }

    #[test]
    fn a_name_of_invisible_characters_only_cleans_to_empty() {
        assert_eq!(clean_text("  \u{200d}  "), "");
        assert_eq!(clean_text("\u{200b}\u{200b}"), "");
        assert_eq!(clean_text("\u{2060}\u{feff}\u{00ad}"), "");
        assert_eq!(clean_text("\u{200d}\u{200b} \u{301}"), "");
        // A leading mark has no base; the text after it is kept.
        assert_eq!(clean_text("\u{301}abc"), "abc");
        // Invisible characters inside a name go; the name stays.
        assert_eq!(clean_text("Ho\u{200b}me\u{00ad}Net"), "HomeNet");
        // A joiner after a base still joins.
        assert_eq!(clean_text("e\u{301}"), "e\u{301}");
    }

    #[test]
    fn the_arabic_letter_mark_is_a_bidi_control() {
        assert_eq!(clean_text("a\u{061c}b"), "ab");
        assert!(is_bidi_control('\u{061c}'));
    }

    #[test]
    fn an_activating_row_meets_a_join_and_not_a_leave() {
        // The lane confirms a join once NetworkManager takes the activation
        // (`Connecting`) and keeps following it to `Connected` or a failure
        // on its own; a leave is met only by `Disconnected`.
        assert!(NetState::Connecting.is_active());
        assert!(NetState::Connected.is_active());
        assert!(!NetState::Disconnected.is_active());
        let mut rows = World::new().net.rows().to_vec();
        for row in &mut rows {
            if row.id().as_str() == "cafe" {
                row.state = NetState::Connecting;
            }
        }
        let net = NetSection::new(true, false, rows);
        let inventory = Inventory {
            network: Some(&net),
            ..Inventory::default()
        };
        let join = Expected::NetActive {
            id: id("cafe"),
            active: true,
        };
        let leave = Expected::NetActive {
            id: id("cafe"),
            active: false,
        };
        assert!(join.is_met(&inventory));
        assert!(!leave.is_met(&inventory));
    }

    #[test]
    fn a_run_of_combining_marks_stays_bounded() {
        let hostile = format!("e{}", "\u{301}".repeat(100_000));
        assert!(clean_text(&hostile).chars().count() <= MAX_CLUSTER_CHARS);
        let hostile = "\u{301}".repeat(100_000);
        assert!(clean_text(&hostile).chars().count() <= MAX_CLUSTER_CHARS);
        let astral = "\u{1f600}".repeat(40);
        assert!(clean_text(&astral).encode_utf16().count() <= MAX_TEXT_UNITS);
    }

    #[test]
    fn lossy_non_utf8_is_kept_as_text() {
        let lossy = String::from_utf8_lossy(&[b'a', 0xff, 0xfe, b'b']).into_owned();
        assert_eq!(clean_text(&lossy), "a\u{fffd}\u{fffd}b");
    }

    #[test]
    fn a_hostile_network_inventory_is_bounded_and_unique() {
        let hostile_name = "\u{202e}ssid\u{0}".repeat(100);
        let mut rows: Vec<NetRow> = (0..5_000)
            .filter_map(|n| {
                NetRow::new(NetDraft {
                    id: &format!("ap-{}", n % 3_000),
                    kind: NetKind::Wifi,
                    name: &hostile_name,
                    state: NetState::Disconnected,
                    signal: u8::try_from(n % 256).unwrap_or(0),
                    security: NetSecurity::Passphrase,
                    known: n % 7 == 0,
                    address: None,
                })
            })
            .collect();
        // Rows the constructor refuses never exist.
        let draft = |id: &'static str| NetDraft {
            id,
            kind: NetKind::Wifi,
            name: "n",
            state: NetState::Disconnected,
            signal: 1,
            security: NetSecurity::Open,
            known: false,
            address: None,
        };
        assert!(NetRow::new(draft("")).is_none());
        let too_long = "x".repeat(MAX_ID_BYTES + 1);
        assert!(NetRow::new(NetDraft {
            id: &too_long,
            ..draft("")
        })
        .is_none());
        // The active row, deep in the input but inside the scan, survives the
        // cut; rows beyond `MAX_SCANNED_ROWS` are never read at all.
        rows.insert(
            MAX_SCANNED_ROWS - 1,
            net_row_of(
                "the-active-one",
                NetKind::Ethernet,
                NetState::Connected,
                NetSecurity::Open,
                false,
            ),
        );
        let section = NetSection::new(true, false, rows);
        assert_eq!(section.rows().len(), MAX_ROWS);
        assert_eq!(section.rows()[0].id().as_str(), "the-active-one");
        let mut ids: Vec<&str> = section.rows().iter().map(|r| r.id().as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), MAX_ROWS);
        for row in section.rows() {
            assert!(row.name().chars().count() <= MAX_NAME_CLUSTERS * MAX_CLUSTER_CHARS);
            assert!(!row.name().contains('\u{202e}') && !row.name().contains('\u{0}'));
            assert!(row.signal() <= 100);
        }
    }

    #[test]
    fn duplicate_ids_keep_the_first_row() {
        let make = |name, state, signal, known| {
            NetRow::new(NetDraft {
                id: "same",
                kind: NetKind::Wifi,
                name,
                state,
                signal,
                security: NetSecurity::Open,
                known,
                address: None,
            })
            .expect("row")
        };
        let first = make("first", NetState::Disconnected, 10, false);
        let second = make("second", NetState::Connected, 90, true);
        let section = NetSection::new(true, false, [first, second]);
        assert_eq!(section.rows().len(), 1);
        assert_eq!(section.rows()[0].name(), "first");
    }

    #[test]
    fn network_rows_order_by_relevance() {
        let section = network();
        let order: Vec<&str> = section.rows().iter().map(|r| r.id().as_str()).collect();
        // Active first; the saved ones (same signal) by name; the unsaved last.
        assert_eq!(order[0], "home");
        assert_eq!(order.last().copied(), Some("stranger"));
        assert!(section.row(&id("cafe")).is_some());
        assert!(section.row(&id("ghost")).is_none());
    }

    #[test]
    fn a_hostile_bluetooth_inventory_is_bounded() {
        assert!(BtRow::new("nope", "x", BtIcon::Other, true, false, false, None).is_none());
        let rows: Vec<BtRow> = (0..3_000u32)
            .filter_map(|n| {
                BtRow::new(
                    &format!(
                        "AA:BB:CC:{:02X}:{:02X}:{:02X}",
                        n >> 16,
                        (n >> 8) & 0xff,
                        n & 0xff
                    ),
                    &"alias\u{0}".repeat(200),
                    BtIcon::from_bluez("hostile"),
                    true,
                    false,
                    false,
                    Some(250),
                )
            })
            .collect();
        let section = BtSection::new(true, true, rows);
        assert_eq!(section.rows().len(), MAX_ROWS);
        assert!(section.rows().iter().all(|r| r.battery() == Some(100)));
        assert!(section.discovering());
    }

    #[test]
    fn bluetooth_rows_order_connected_first_and_dedupe() {
        let section = BtSection::new(
            true,
            false,
            [
                bt_row_of("AA:BB:CC:DD:EE:03", false, false),
                bt_row_of("AA:BB:CC:DD:EE:02", true, false),
                bt_row_of("aa:bb:cc:dd:ee:02", true, true),
                bt_row_of("AA:BB:CC:DD:EE:01", true, true),
            ],
        );
        let order: Vec<&str> = section
            .rows()
            .iter()
            .map(|r| r.address().as_str())
            .collect();
        // The lower-case spelling of :02 is the same device as the first :02.
        assert_eq!(
            order,
            [
                "AA:BB:CC:DD:EE:01",
                "AA:BB:CC:DD:EE:02",
                "AA:BB:CC:DD:EE:03"
            ]
        );
        assert!(!section.rows()[1].connected());
    }

    #[test]
    fn icons_classify_without_failing() {
        assert_eq!(BtIcon::from_bluez("audio-headset"), BtIcon::Audio);
        assert_eq!(BtIcon::from_bluez("input-mouse"), BtIcon::Pointer);
        assert_eq!(BtIcon::from_bluez("input-keyboard"), BtIcon::Keyboard);
        assert_eq!(BtIcon::from_bluez(""), BtIcon::Other);
        assert_eq!(BtIcon::from_bluez("../../etc/passwd"), BtIcon::Other);
    }

    #[test]
    fn battery_is_absent_or_bounded() {
        assert!(BatterySection::new(false, ChargeState::Full, 100.0, 0, 0).is_none());
        assert!(BatterySection::new(true, ChargeState::Full, f64::NAN, 0, 0).is_none());
        assert!(BatterySection::new(true, ChargeState::Full, f64::INFINITY, 0, 0).is_none());
        let over =
            BatterySection::new(true, ChargeState::Charging, 250.0, -5, 3_600).expect("some");
        assert_eq!(over.percentage(), 100);
        assert_eq!(over.time_to_empty(), None);
        assert_eq!(over.time_to_full(), Some(Duration::from_secs(3_600)));
        let under = BatterySection::new(true, ChargeState::Empty, -3.0, 0, i64::MAX).expect("some");
        assert_eq!(under.percentage(), 0);
        assert_eq!(under.time_to_full(), None);
        let rounded =
            BatterySection::new(true, ChargeState::Discharging, 41.6, 90, 0).expect("some");
        assert_eq!(rounded.percentage(), 42);
        assert_eq!(rounded.time_to_empty(), Some(Duration::from_secs(90)));
        assert_eq!(ChargeState::from_upower(1), ChargeState::Charging);
        assert_eq!(ChargeState::from_upower(99), ChargeState::Unknown);
    }

    #[test]
    fn profile_section_is_bounded_and_typed() {
        let hostile = (0..100)
            .map(|n| format!("p{n}"))
            .chain(["BAD NAME".to_owned(), "p1".to_owned()]);
        let section = ProfileSection::new("p0", hostile, Degraded::from_daemon("other"));
        assert_eq!(section.profiles().len(), MAX_PROFILES);
        assert_eq!(section.active().map(ProfileName::as_str), Some("p0"));
        assert_eq!(section.degraded(), Degraded::Other);
        assert_eq!(
            ProfileSection::new("Bad!", ["a"], Degraded::No).active(),
            None
        );
        assert_eq!(Degraded::from_daemon(""), Degraded::No);
        assert_eq!(Degraded::from_daemon("lap-detected"), Degraded::LapDetected);
    }

    #[test]
    fn cycling_wraps_and_refuses_what_it_cannot_place() {
        let section = profile_section();
        assert_eq!(
            section.cycle().map(ProfileName::as_str),
            Some("power-saver")
        );

        let at = |active: &str| {
            ProfileSection::new(
                active,
                ["performance", "balanced", "power-saver"],
                Degraded::No,
            )
        };
        assert_eq!(
            at("performance").cycle().map(ProfileName::as_str),
            Some("balanced")
        );
        assert_eq!(
            at("power-saver").cycle().map(ProfileName::as_str),
            Some("performance")
        );
        // A profile the daemon does not offer is not a place to cycle from.
        assert_eq!(at("turbo").cycle(), None);
        assert_eq!(
            ProfileSection::new("balanced", ["balanced"], Degraded::No).cycle(),
            None
        );
        assert_eq!(
            ProfileSection::new("balanced", Vec::<&str>::new(), Degraded::No).cycle(),
            None
        );
    }

    #[test]
    fn cycling_with_nothing_to_cycle_is_refused() {
        let world = World {
            profiles: ProfileSection::new("balanced", ["balanced"], Degraded::No),
            ..World::new()
        };
        assert_eq!(
            validate(&Action::CycleProfile, &world.inventory()),
            Err(Refusal::NothingToCycle)
        );
    }

    #[test]
    fn refusals_have_journal_keys_and_display() {
        let all = [
            Refusal::NoInventory(Section::Network),
            Refusal::UnknownId,
            Refusal::NotKnown,
            Refusal::KindNotAllowed,
            Refusal::NeedsSecretClass,
            Refusal::NotActive,
            Refusal::NotPaired,
            Refusal::WifiDisabled,
            Refusal::HardwareBlocked,
            Refusal::NoSuchProfile,
            Refusal::NothingToCycle,
        ];
        let mut keys: Vec<&str> = all.iter().map(|r| r.key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), all.len());
        assert_eq!(
            Refusal::UnknownId.to_string(),
            "request refused: unknown-id"
        );
    }

    #[test]
    fn service_exposes_only_what_answered() {
        let ready = Service::Ready(3);
        assert_eq!(ready.ready(), Some(&3));
        assert_eq!(Service::<u8>::Unavailable.ready(), None);
    }

    #[test]
    fn pending_confirms_on_observation() {
        let t0 = Instant::now();
        let world = World::new();
        let mut pending = PendingSet::new();
        let want = Expected::NetActive {
            id: id("cafe"),
            active: true,
        };
        pending.start(want.clone(), t0);
        assert!(pending.is_pending(&want));
        assert!(pending
            .observe(&world.inventory(), t0 + Duration::from_secs(5))
            .is_empty());

        let mut joined = world.net.rows().to_vec();
        for row in &mut joined {
            if row.id().as_str() == "cafe" {
                row.state = NetState::Connected;
            }
        }
        let net = NetSection::new(true, false, joined);
        let inventory = Inventory {
            network: Some(&net),
            ..Inventory::default()
        };
        let resolved = pending.observe(&inventory, t0 + Duration::from_secs(6));
        assert_eq!(resolved, [Resolution::Confirmed(want.clone())]);
        assert!(!pending.is_pending(&want));
        assert!(pending.is_empty());
    }

    #[test]
    fn pending_times_out_after_the_window() {
        let t0 = Instant::now();
        let world = World::new();
        let mut pending = PendingSet::new();
        let want = Expected::BtConnected {
            address: addr("AA:BB:CC:DD:EE:02"),
            connected: true,
        };
        pending.start(want.clone(), t0);
        assert_eq!(pending.next_deadline(), Some(t0 + PENDING_WINDOW));
        let almost = t0 + PENDING_WINDOW - Duration::from_millis(1);
        assert!(pending.observe(&world.inventory(), almost).is_empty());
        let resolved = pending.observe(&world.inventory(), t0 + PENDING_WINDOW);
        assert_eq!(resolved, [Resolution::Failed(want, Failure::Timeout)]);
        assert_eq!(pending.next_deadline(), None);
    }

    #[test]
    fn a_vanished_service_or_row_never_confirms() {
        let none = Inventory::default();
        assert!(!Expected::WifiEnabled(true).is_met(&none));
        assert!(!Expected::WifiEnabled(false).is_met(&none));
        assert!(!Expected::BtPowered(false).is_met(&none));
        let world = World::new();
        let gone = Expected::NetActive {
            id: id("ghost"),
            active: false,
        };
        assert!(!gone.is_met(&world.inventory()));
        assert!(Expected::Profile(profile("balanced")).is_met(&world.inventory()));
        assert!(!Expected::Profile(profile("performance")).is_met(&world.inventory()));
    }

    #[test]
    fn a_new_request_replaces_the_old_for_its_subject_and_the_set_is_bounded() {
        let t0 = Instant::now();
        let mut pending = PendingSet::new();
        pending.start(Expected::BtPowered(true), t0);
        pending.start(Expected::BtPowered(false), t0 + Duration::from_secs(1));
        assert!(!pending.is_pending(&Expected::BtPowered(true)));
        assert!(pending.is_pending(&Expected::BtPowered(false)));

        for n in 0..(MAX_PENDING * 3) {
            pending.start(
                Expected::NetActive {
                    id: id(&format!("row-{n}")),
                    active: true,
                },
                t0,
            );
        }
        assert!(pending.entries.len() <= MAX_PENDING);
        // The oldest was dropped, the newest kept.
        assert!(pending.is_pending(&Expected::NetActive {
            id: id(&format!("row-{}", MAX_PENDING * 3 - 1)),
            active: true,
        }));
    }

    #[test]
    fn a_backend_failure_resolves_the_pending_entry() {
        let t0 = Instant::now();
        let mut pending = PendingSet::new();
        let want = Expected::Profile(profile("performance"));
        pending.start(want.clone(), t0);
        assert_eq!(
            pending.fail(&want, Failure::Denied),
            Some(Resolution::Failed(want.clone(), Failure::Denied))
        );
        assert_eq!(pending.fail(&want, Failure::Denied), None);
        assert!(pending.is_empty());
        assert_eq!(PENDING_WINDOW, Duration::from_secs(20));
    }
}
