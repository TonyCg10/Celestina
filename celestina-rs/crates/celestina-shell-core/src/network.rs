//! How the session is actually online.
//!
//! NetworkManager knows every device; the routing table knows which one is
//! carrying traffic. The panel wants the second question answered, so this
//! reads both and reports the link the default route goes through — not the
//! first connected device, which on a machine with both cable and wifi would
//! be a coin toss.

use serde_json::Value;

use crate::bounded;
use crate::inventory::{Answer, Published, Reading};
use crate::snapshot::{Payload, MAX_ROW_ITEMS, MAX_TEXT_UNITS};

/// One device as `nmcli -t -f DEVICE,TYPE,STATE,CONNECTION device` lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub name: String,
    pub kind: String,
    pub connected: bool,
    pub connection: String,
}

/// The link the panel shows: what kind of connection carries the session, and
/// what it is called.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub kind: String,
    pub connection: String,
}

/// The device the default route goes through, from `ip route show default`.
#[must_use]
pub fn parse_default_route_device(routes: &str) -> Option<String> {
    routes
        .lines()
        .find(|line| line.trim_start().starts_with("default"))
        .and_then(|line| {
            let mut fields = line.split_whitespace();
            while let Some(field) = fields.next() {
                if field == "dev" {
                    return fields.next().map(str::to_owned);
                }
            }
            None
        })
}

/// Reads nmcli's terse device list. A connection name may contain spaces, so
/// only the field separator is trusted to split it.
#[must_use]
pub fn parse_devices(listing: &str) -> Vec<Device> {
    listing
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(4, ':');
            let name = fields.next()?.trim();
            let kind = fields.next()?.trim();
            let state = fields.next()?.trim();
            let connection = fields.next().unwrap_or("").trim();
            if name.is_empty() {
                return None;
            }

            Some(Device {
                name: name.to_owned(),
                kind: kind.to_owned(),
                // "connected (externally)" is loopback's state, and it is still
                // connected — the caller decides that loopback is not a link.
                connected: state.starts_with("connected"),
                connection: connection.to_owned(),
            })
        })
        .collect()
}

/// The link carrying the session, or `None` when nothing is.
///
/// The default route decides. Without one — no route at all, or a device
/// NetworkManager does not manage — there is no link to report, because a
/// connected device that carries nothing is not how the session is online.
#[must_use]
pub fn active_link(devices: &[Device], default_route_device: Option<&str>) -> Option<Link> {
    let carrying = default_route_device?;
    devices
        .iter()
        .find(|device| device.name == carrying && device.connected && device.kind != "loopback")
        .map(|device| Link {
            kind: device.kind.clone(),
            connection: device.connection.clone(),
        })
}

/// What one poll of the two commands managed to see.
///
/// The distinction this type exists for: `nmcli` on this machine normally
/// answers in four or five milliseconds and occasionally takes three seconds,
/// well past the shared tool deadline. A poll that did not finish saw nothing —
/// which is not the same as seeing that nothing is connected, and treating it
/// as such is what made the panel's Wi-Fi text blink out while the link stayed
/// up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    /// Both commands answered and named the link carrying the session.
    Carrying(Link),
    /// The routing table answered and there is no default route. This is the
    /// only positive evidence that nothing is carrying the session, and the
    /// only observation that may retire a link.
    Offline,
    /// Nothing conclusive was seen. Either a command did not answer inside its
    /// deadline, or the route named a device this poll could not describe —
    /// which is a session that *is* being carried by something this reading
    /// failed to name, and is therefore the opposite of offline.
    Unreadable,
}

/// What the routing table answered, before anything else is asked.
///
/// The order matters and is why this is its own step. The routing table alone
/// settles two of the three outcomes: it is the only thing that knows whether
/// anything carries the session, and it needs no help from `nmcli` to say so.
/// Requiring both commands to answer before classifying anything is what let a
/// real disconnection be held indefinitely — the route said "nothing", `nmcli`
/// said nothing at all, and the pair was read as "I could not look".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteReading {
    /// The command did not answer inside its deadline.
    Unreadable,
    /// It answered, and there is no default route. Nothing carries the session.
    NoDefault,
    /// It answered, and the default route goes through this device.
    Through(String),
}

/// Reads the routing table's answer. `None` is the command not answering, which
/// is not the same as it answering that there is no route.
#[must_use]
pub fn read_route(routes: Option<&str>) -> RouteReading {
    let Some(routes) = routes else {
        return RouteReading::Unreadable;
    };

    parse_default_route_device(routes).map_or(RouteReading::NoDefault, RouteReading::Through)
}

/// Whether the routing table's answer already settles the reading, so the
/// device list is a command not worth running.
///
/// Only a route that names a device leaves anything to ask about: the other two
/// answers are the observation.
#[must_use]
pub fn needs_device_list(route: &RouteReading) -> bool {
    matches!(route, RouteReading::Through(_))
}

/// The observation, from what each command answered.
///
/// `devices` is `None` both when `nmcli` did not answer and when it was never
/// asked because the route had already decided — and those cases are the same
/// here, because in the second one its answer is not consulted.
#[must_use]
pub fn observe_with(route: &RouteReading, devices: Option<&str>) -> Observation {
    match route {
        RouteReading::Unreadable => Observation::Unreadable,
        // The kernel answered that nothing carries this session. That is
        // evidence, and it does not need a second opinion from `nmcli` — which
        // is exactly the opinion that used to be unavailable when it mattered.
        RouteReading::NoDefault => Observation::Offline,
        RouteReading::Through(device) => devices.map_or(Observation::Unreadable, |listing| {
            observe(&parse_devices(listing), Some(device))
        }),
    }
}

/// What one poll saw of a session that *is* routed, from the device list.
///
/// A route naming a device `nmcli` did not report as connected used to fall
/// through to "nothing is carrying the session": that is how a Wi-Fi card
/// re-associating for two seconds — still routed, still carrying — was read as
/// a disconnection.
#[must_use]
pub fn observe(devices: &[Device], default_route_device: Option<&str>) -> Observation {
    let Some(carrying) = default_route_device else {
        // No default route at all: the kernel is saying nothing carries this
        // session, and it is the one thing here that knows.
        return Observation::Offline;
    };

    active_link(devices, Some(carrying)).map_or(Observation::Unreadable, Observation::Carrying)
}

/// How many confirmed-offline observations the last link survives before it
/// stops being published.
///
/// One, so the second **consecutive** one retires it: at the session poll's
/// five-second interval a real disconnection clears the panel in about ten
/// seconds, and a single flap does not.
///
/// Consecutive is meant literally. An unreadable poll keeps the link — nothing
/// that saw nothing may retire one — but it also ends any run of offline
/// evidence in progress, because an old confirmation must not stay armed across
/// an arbitrary gap and fire against a session that has reconnected since.
///
/// There is deliberately no matching bound for [`Observation::Unreadable`].
/// There used to be, and it was wrong: a long enough run of probes that saw
/// nothing became an assertion that the session was disconnected, purely by
/// repetition. Only evidence retires a link.
pub const OFFLINE_HOLD: u32 = 1;

/// The last link this session was confirmed to be on, and how long it may
/// outlive its last confirmation.
///
/// Pure and owned here rather than in the provider, because "when does an old
/// reading stop being true" is policy and the provider's job is to run two
/// commands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkTracker {
    confirmed: Option<Link>,
    /// Polls since the last confirmation, of any kind. Reporting only.
    unconfirmed: u32,
    /// Consecutive confirmed-offline polls. Anything that is not a confirmed
    /// offline poll — a link, or a poll that saw nothing — resets it, so a run
    /// is a real run and not a total.
    offline: u32,
}

impl LinkTracker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the last confirmed link is being published without a
    /// confirmation behind it. Reported so a record can say which it was.
    #[must_use]
    pub fn is_holding(&self) -> bool {
        self.confirmed.is_some() && self.unconfirmed > 0
    }

    /// The link to publish after this observation, or `None` to publish
    /// nothing.
    pub fn observe(&mut self, observation: Observation) -> Option<&Link> {
        match observation {
            Observation::Carrying(link) => {
                self.confirmed = Some(link);
                self.unconfirmed = 0;
                self.offline = 0;
            }
            // Held, however long this goes on. A probe that saw nothing is not
            // an observation of a disconnected session, and a thousand of them
            // are not one either — so this counts them only so a record can say
            // the reading is being held, and never retires anything.
            //
            // It does end a run of offline evidence, though. "Two consecutive"
            // has to mean consecutive: an offline confirmation left armed
            // across a long unreadable gap would retire a link on the strength
            // of something observed minutes ago, about a session that may have
            // reconnected in between.
            Observation::Unreadable => {
                self.unconfirmed = self.unconfirmed.saturating_add(1);
                self.offline = 0;
            }
            Observation::Offline => {
                self.unconfirmed = self.unconfirmed.saturating_add(1);
                self.offline = self.offline.saturating_add(1);
                if self.offline > OFFLINE_HOLD {
                    self.confirmed = None;
                    // Nothing is held any more, so nothing is owed a count.
                    self.unconfirmed = 0;
                    self.offline = 0;
                }
            }
        }

        self.confirmed.as_ref()
    }
}

/// Splits one line of `nmcli --terse` output into its fields.
///
/// The separator is `:`, and `nmcli` escapes a literal `:` inside a value as
/// `\:` and a literal backslash as `\\`. Splitting on the raw byte therefore
/// tears an access point called `Casa: cable` into two fields and shifts every
/// field after it — which, for a list keyed by identity, silently attributes
/// one network's name to another's UUID. The escape is honoured here so a name
/// chosen by somebody else cannot move a column.
#[must_use]
pub fn split_terse(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            current.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == ':' {
            fields.push(std::mem::take(&mut current));
        } else {
            current.push(character);
        }
    }
    fields.push(current);
    fields
}

/// What the last scan the session already had says about a saved network.
///
/// Three states rather than a signal number and a sentinel, because "no scan
/// result mentions this network" and "this poll never learned what the scan
/// saw" lead to different words on screen and this unit does not scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// Nothing was learned about what is in range. Not a claim of absence.
    Unknown,
    /// The scan results the session already had do not mention it.
    OutOfRange,
    /// They mention it, with this signal percentage.
    InRange(u8),
}

impl Availability {
    /// The protocol token. What a person reads is the surface's business.
    #[must_use]
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::OutOfRange => "out-of-range",
            Self::InRange(_) => "in-range",
        }
    }

    /// The signal percentage, when one was actually observed.
    #[must_use]
    pub fn signal(self) -> Option<u8> {
        match self {
            Self::InRange(signal) => Some(signal),
            Self::Unknown | Self::OutOfRange => None,
        }
    }
}

/// A Wi-Fi network this session already knows how to join.
///
/// Saved profiles only: a network that is merely in range would need a
/// password this shell has no business collecting, so it is not offered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownNetwork {
    /// NetworkManager's UUID. Stable across renames and unique per profile,
    /// which neither a name nor an SSID is — two saved profiles may share
    /// both.
    pub id: String,
    /// What the profile is called. A label, chosen by whoever saved it.
    pub name: String,
    /// The network the profile actually joins, when this session can learn it.
    ///
    /// Separate from `name` because they are different things and are only
    /// equal by convention. `nmcli connection show` cannot report an SSID:
    /// its field list is `NAME,UUID,TYPE,TIMESTAMP,TIMESTAMP-REAL,AUTOCONNECT,
    /// AUTOCONNECT-PRIORITY,READONLY,DBUS-PATH,ACTIVE,DEVICE,STATE,
    /// ACTIVE-PATH,PORT,FILENAME` and nothing there is one. Reading the SSID
    /// per profile would be one process per row, which this poll will not do.
    /// So it is `None` for every profile this session cannot relate honestly,
    /// and a `None` here is never guessed at from `name`.
    pub ssid: Option<String>,
    /// Attached to a device right now, as NetworkManager reports it.
    pub active: bool,
    pub availability: Availability,
}

/// Whether a terse `TYPE` field names a Wi-Fi profile. `nmcli` prints the
/// settings name in terse mode and the pretty alias in some versions.
fn is_wifi_type(kind: &str) -> bool {
    kind == "802-11-wireless" || kind == "wifi"
}

/// Reads saved profiles from `nmcli -t -f UUID,NAME,TYPE,DEVICE connection show`.
///
/// Bounded, deduplicated by UUID and truncated to the shared row limit before
/// anything leaves this function: the listing is another program's output and
/// its length is not this panel's to trust.
#[must_use]
pub fn parse_saved_wifi(listing: &str) -> Vec<KnownNetwork> {
    let mut networks: Vec<KnownNetwork> = Vec::new();
    for line in listing.lines() {
        if networks.len() >= MAX_ROW_ITEMS {
            break;
        }
        let fields = split_terse(line);
        let [uuid, name, kind, device] = &fields[..] else {
            continue;
        };
        let id = bounded(uuid.trim(), MAX_TEXT_UNITS);
        if id.is_empty() || !is_wifi_type(kind.trim()) {
            continue;
        }
        // A duplicate identity is a listing this panel cannot key on. The
        // first row wins rather than the last silently replacing it.
        if networks.iter().any(|known| known.id == id) {
            continue;
        }

        let device = device.trim();
        networks.push(KnownNetwork {
            id,
            name: bounded(name.trim(), MAX_TEXT_UNITS),
            // Not knowable from this listing, and not guessed at from `name`.
            ssid: None,
            // `--` is what `nmcli` prints for "no device" in some columns; an
            // empty field is what terse output normally gives.
            active: !device.is_empty() && device != "--",
            availability: Availability::Unknown,
        });
    }
    networks
}

/// One access point the session's existing scan results already mention.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Visible {
    pub ssid: String,
    pub signal: u8,
    /// The `*` in `IN-USE`: this is the access point the session is on.
    pub in_use: bool,
}

/// Reads `nmcli -t -f IN-USE,SSID,SIGNAL device wifi list --rescan no`, which
/// reports the scan results the session already had and starts no new scan.
///
/// The strongest reading of a repeated SSID wins, because several access
/// points on one network are one entry to the person choosing it.
#[must_use]
pub fn parse_visible_wifi(listing: &str) -> Vec<Visible> {
    let mut visible: Vec<Visible> = Vec::new();
    for line in listing.lines() {
        if visible.len() >= MAX_ROW_ITEMS {
            break;
        }
        let fields = split_terse(line);
        let [in_use, ssid, signal] = &fields[..] else {
            continue;
        };
        let ssid = bounded(ssid.trim(), MAX_TEXT_UNITS);
        if ssid.is_empty() {
            continue;
        }
        // Anything unparseable is nothing learned, not zero signal.
        let Ok(signal) = signal.trim().parse::<u8>() else {
            continue;
        };
        let signal = signal.min(100);
        let in_use = in_use.trim() == "*";

        if let Some(existing) = visible.iter_mut().find(|seen| seen.ssid == ssid) {
            existing.signal = existing.signal.max(signal);
            existing.in_use |= in_use;
        } else {
            visible.push(Visible {
                ssid,
                signal,
                in_use,
            });
        }
    }
    visible
}

/// The saved Wi-Fi networks, marked with what the session's existing scan
/// results say about each.
///
/// The saved list decides the reading: it is the one that says what may be
/// offered. Scan results only annotate it, so a missing or unreadable scan
/// costs an availability word rather than the whole list.
///
/// Availability is decided by SSID and never by profile name. A profile whose
/// network this session cannot name is `Unknown`, because the alternative —
/// treating "I do not know what this profile joins" as "it is not in range" —
/// is a claim about the radio drawn from a gap in NetworkManager's own output.
#[must_use]
pub fn read_known_networks(saved: &Answer, visible: &Answer) -> Reading<KnownNetwork> {
    let saved = match saved {
        Answer::Missing => return Reading::Unavailable,
        Answer::Unreadable => return Reading::Unreadable,
        Answer::Text(listing) => parse_saved_wifi(listing),
    };

    // An unread scan annotates nothing. The list still stands, with every row
    // saying that nothing is known about whether it is reachable.
    let Some(visible) = visible.text().map(parse_visible_wifi) else {
        return Reading::Listed(saved);
    };

    Reading::Listed(relate(saved, &visible))
}

/// Marks each saved profile with what a conclusive scan says about it.
///
/// Its own step because the rule it enforces is the one worth stating alone:
/// availability follows the SSID and never the label. A profile whose network
/// this session cannot name comes out `Unknown`, and only a profile with a
/// real SSID absent from a scan that answered comes out `OutOfRange`.
#[must_use]
pub fn relate(saved: Vec<KnownNetwork>, visible: &[Visible]) -> Vec<KnownNetwork> {
    // Without a device column on the scan rows there is only one honest
    // attribution: exactly one active Wi-Fi profile and exactly one network in
    // use. Multiple radios make every pairing ambiguous, so none is guessed.
    let active_profiles = saved.iter().filter(|known| known.active).count();
    let mut in_use = visible.iter().filter(|seen| seen.in_use);
    let attributable_ssid = match (active_profiles, in_use.next(), in_use.next()) {
        (1, Some(seen), None) => Some(seen.ssid.clone()),
        _ => None,
    };

    saved
        .into_iter()
        .map(|known| {
            let ssid = known
                .ssid
                .clone()
                .or_else(|| known.active.then(|| attributable_ssid.clone()).flatten());
            let availability = ssid.as_ref().map_or(
                // No SSID to look for. The scan answered, and it answered
                // about networks, not about this profile.
                Availability::Unknown,
                |ssid| {
                    visible
                        .iter()
                        .find(|seen| seen.ssid == *ssid)
                        // The scan was conclusive and this profile's real
                        // network is not in it. Only here is out-of-range
                        // something that was observed.
                        .map_or(Availability::OutOfRange, |seen| {
                            Availability::InRange(seen.signal)
                        })
                },
            );

            KnownNetwork {
                ssid,
                availability,
                ..known
            }
        })
        .collect()
}

/// The `network` provider's payload, or `None` when there is nothing true to
/// publish and the provider should be withdrawn.
///
/// Pure so the shape can be tested without a routing table, a process or a
/// radio. Additive by construction: `kind` and `connection` mean exactly what
/// they have always meant and appear only when a link is confirmed, so a
/// surface that reads nothing else is unaffected by everything below them.
///
/// A session with no default route still has saved networks, and that is
/// precisely the moment somebody wants to see them — so the absence of a link
/// withdraws the provider only when there is no conclusive inventory either.
#[must_use]
pub fn payload(link: Option<&Link>, networks: Published<'_, KnownNetwork>) -> Option<Payload> {
    // Pending says nothing has been learned. Unavailable is different: it is a
    // conclusive fact about this session and must reach the surface even when
    // there is no link beside it.
    if link.is_none() && matches!(networks, Published::Pending) {
        return None;
    }

    let mut payload = Payload::new();
    if let Some(link) = link {
        payload.insert("kind".to_owned(), Value::from(link.kind.clone()));
        payload.insert(
            "connection".to_owned(),
            Value::from(link.connection.clone()),
        );
    }

    payload.insert("networksState".to_owned(), Value::from(networks.as_token()));
    if let Some(rows) = networks.rows() {
        payload.insert(
            "networks".to_owned(),
            Value::Array(rows.iter().map(KnownNetwork::to_row).collect()),
        );
    }
    Some(payload)
}

impl KnownNetwork {
    /// One row of the published inventory.
    fn to_row(&self) -> Value {
        let mut row = Payload::new();
        row.insert("id".to_owned(), Value::from(self.id.clone()));
        row.insert("name".to_owned(), Value::from(self.name.clone()));
        row.insert("active".to_owned(), Value::from(self.active));
        row.insert(
            "availability".to_owned(),
            Value::from(self.availability.as_token()),
        );
        // Each absent rather than a sentinel: an SSID this session could not
        // learn is not an empty one, and a signal nobody read is not zero.
        if let Some(ssid) = &self.ssid {
            row.insert("ssid".to_owned(), Value::from(ssid.clone()));
        }
        if let Some(signal) = self.availability.signal() {
            row.insert("signal".to_owned(), Value::from(signal));
        }
        Value::Object(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory;

    fn wifi() -> Link {
        Link {
            kind: "wifi".to_owned(),
            connection: "Tonys 1".to_owned(),
        }
    }

    fn listed(reading: Reading<KnownNetwork>) -> Vec<KnownNetwork> {
        match reading {
            Reading::Listed(networks) => networks,
            other => panic!("expected a listing, got {other:?}"),
        }
    }

    #[test]
    fn a_valid_sample_is_published_as_itself() {
        let mut tracker = LinkTracker::new();

        assert_eq!(
            tracker.observe(Observation::Carrying(wifi())),
            Some(&wifi())
        );
        assert!(!tracker.is_holding());
    }

    #[test]
    fn a_slow_probe_does_not_erase_the_link_it_failed_to_read() {
        let mut tracker = LinkTracker::new();
        tracker.observe(Observation::Carrying(wifi()));

        // The exact live failure: one 3-second `nmcli` against a 750 ms
        // deadline, while the connection stayed in use throughout.
        assert_eq!(tracker.observe(Observation::Unreadable), Some(&wifi()));
        assert!(tracker.is_holding());
    }

    #[test]
    fn a_probe_that_answers_again_confirms_the_link_and_clears_the_hold() {
        let mut tracker = LinkTracker::new();
        tracker.observe(Observation::Carrying(wifi()));
        tracker.observe(Observation::Unreadable);
        tracker.observe(Observation::Unreadable);

        assert_eq!(
            tracker.observe(Observation::Carrying(wifi())),
            Some(&wifi())
        );
        assert!(!tracker.is_holding());
        // And the allowance is whole again, not spent by the patch before it.
        assert_eq!(tracker.observe(Observation::Unreadable), Some(&wifi()));
    }

    /// The correction this unit exists for. The live session kept losing its
    /// Wi-Fi text while the link stayed up, because a long enough run of probes
    /// that saw nothing used to expire the hold — which turned "I could not
    /// look" into "you are disconnected" by repetition alone.
    #[test]
    fn an_unreadable_run_never_becomes_a_claim_of_disconnection() {
        let mut tracker = LinkTracker::new();
        tracker.observe(Observation::Carrying(wifi()));

        // Far longer than any hold this ever had, and longer than any burst of
        // `nmcli` latency observed on this machine.
        for _ in 0..10_000 {
            assert_eq!(tracker.observe(Observation::Unreadable), Some(&wifi()));
        }
        assert!(tracker.is_holding());

        // And it is still the *last confirmed* link, not a fossil: one good
        // sample replaces it. The cable is taken from the session's own device
        // list rather than written out again, so this reads one fixture.
        let cable = active_link(&parse_devices(DEVICES), Some("enp5s0")).expect("a link");
        assert_eq!(
            tracker.observe(Observation::Carrying(cable.clone())),
            Some(&cable)
        );
        assert!(!tracker.is_holding());
    }

    /// "Two consecutive" is meant literally. An unreadable poll keeps the link
    /// and ends any run of offline evidence in progress, so a confirmation from
    /// minutes ago cannot fire against a session that reconnected since.
    #[test]
    fn an_unreadable_poll_keeps_the_link_and_ends_an_offline_run() {
        let mut tracker = LinkTracker::new();
        tracker.observe(Observation::Carrying(wifi()));

        assert_eq!(tracker.observe(Observation::Offline), Some(&wifi()));
        // Not evidence, so the run restarts rather than continuing.
        for _ in 0..50 {
            assert_eq!(tracker.observe(Observation::Unreadable), Some(&wifi()));
        }
        // One offline is now the first of a new run, not the second of an old.
        assert_eq!(tracker.observe(Observation::Offline), Some(&wifi()));
        // And the one after it retires the link.
        assert_eq!(tracker.observe(Observation::Offline), None);
    }

    #[test]
    fn a_confirmed_disconnection_ends_the_link_quickly() {
        let mut tracker = LinkTracker::new();
        tracker.observe(Observation::Carrying(wifi()));

        // One tolerated sample, because a route can be absent for an instant
        // while NetworkManager moves between access points.
        assert_eq!(tracker.observe(Observation::Offline), Some(&wifi()));
        // The second is evidence twice over, and much sooner than an
        // unreadable run would expire.
        assert_eq!(tracker.observe(Observation::Offline), None);
    }

    #[test]
    fn a_session_that_was_never_online_publishes_nothing() {
        let mut tracker = LinkTracker::new();

        assert_eq!(tracker.observe(Observation::Unreadable), None);
        assert_eq!(tracker.observe(Observation::Offline), None);
        assert!(!tracker.is_holding());
    }

    const DEVICES: &str = "enp5s0:ethernet:connected:Conexión cableada 1\n\
                           wlan0:wifi:connected:Tonys 1\n\
                           lo:loopback:connected (externally):lo\n\
                           p2p-dev-wlan0:wifi-p2p:disconnected:\n";

    #[test]
    fn the_default_route_names_the_device_carrying_the_session() {
        assert_eq!(
            parse_default_route_device(
                "default via 192.168.1.1 dev enp5s0 proto dhcp metric 100\n"
            )
            .as_deref(),
            Some("enp5s0")
        );
        // More than one default route: the first is the one in use.
        assert_eq!(
            parse_default_route_device(
                "default via 10.0.0.1 dev wlan0 metric 600\ndefault via 10.0.0.1 dev enp5s0 metric 700\n"
            )
            .as_deref(),
            Some("wlan0")
        );
        assert_eq!(parse_default_route_device(""), None);
        assert_eq!(parse_default_route_device("10.0.0.0/24 dev wlan0\n"), None);
    }

    #[test]
    fn a_connection_name_may_contain_spaces_and_colons() {
        let devices = parse_devices("enp5s0:ethernet:connected:Casa: cable 1\n");

        assert_eq!(devices[0].connection, "Casa: cable 1");
    }

    #[test]
    fn the_link_is_the_one_the_route_goes_through() {
        let devices = parse_devices(DEVICES);

        // Both cable and wifi are connected; only one carries the session.
        let link = active_link(&devices, Some("enp5s0")).expect("a link");
        assert_eq!(link.kind, "ethernet");
        assert_eq!(link.connection, "Conexión cableada 1");

        let link = active_link(&devices, Some("wlan0")).expect("a link");
        assert_eq!(link.kind, "wifi");
        assert_eq!(link.connection, "Tonys 1");
    }

    /// A route that names a device `nmcli` did not describe is a session that
    /// *is* being carried by something this poll could not name. It used to be
    /// classified as "nothing is carrying the session", which is how a Wi-Fi
    /// card re-associating for two seconds read as a disconnection.
    #[test]
    fn a_route_the_device_list_cannot_explain_is_unreadable_not_offline() {
        let devices = parse_devices(DEVICES);

        assert_eq!(
            observe(&devices, Some("wlan0")),
            Observation::Carrying(Link {
                kind: "wifi".to_owned(),
                connection: "Tonys 1".to_owned(),
            })
        );
        // Routed through something the list does not manage.
        assert_eq!(observe(&devices, Some("tun0")), Observation::Unreadable);
        // Routed through a device the list says is not connected yet.
        let associating = parse_devices("wlan0:wifi:connecting:Tonys 1\n");
        assert_eq!(
            observe(&associating, Some("wlan0")),
            Observation::Unreadable
        );
        // A device list that never arrived, with a route that did.
        assert_eq!(observe(&[], Some("wlan0")), Observation::Unreadable);
        // No default route: the kernel's own answer, and the only offline one.
        assert_eq!(observe(&devices, None), Observation::Offline);
    }

    /// Every combination the two commands can answer with, classified in the
    /// order the provider runs them.
    #[test]
    fn the_route_is_read_first_and_settles_two_of_the_three_outcomes() {
        const ROUTE: &str = "default via 192.168.1.1 dev wlan0 proto dhcp metric 600\n";

        // The routing table did not answer. Nothing is claimed either way, and
        // there is nothing to ask `nmcli` about.
        assert_eq!(read_route(None), RouteReading::Unreadable);
        assert!(!needs_device_list(&RouteReading::Unreadable));
        assert_eq!(
            observe_with(&RouteReading::Unreadable, None),
            Observation::Unreadable
        );

        // It answered, and there is no default route. That is the whole
        // observation: a real disconnection must retire the link on this
        // evidence alone, whether or not `nmcli` is answering at the time.
        assert_eq!(read_route(Some("")), RouteReading::NoDefault);
        assert_eq!(
            read_route(Some("10.0.0.0/24 dev wlan0 scope link\n")),
            RouteReading::NoDefault
        );
        assert!(!needs_device_list(&RouteReading::NoDefault));
        assert_eq!(
            observe_with(&RouteReading::NoDefault, None),
            Observation::Offline
        );

        // It answered with a device. Only now is the device list worth running.
        let routed = read_route(Some(ROUTE));
        assert_eq!(routed, RouteReading::Through("wlan0".to_owned()));
        assert!(needs_device_list(&routed));

        // The list did not answer: something carries the session and this poll
        // could not name it.
        assert_eq!(observe_with(&routed, None), Observation::Unreadable);
        // It answered and explains the routed device.
        assert_eq!(
            observe_with(&routed, Some(DEVICES)),
            Observation::Carrying(Link {
                kind: "wifi".to_owned(),
                connection: "Tonys 1".to_owned(),
            })
        );
        // It answered and does not explain it: still not a disconnection.
        assert_eq!(
            observe_with(&routed, Some("wlan0:wifi:connecting:Tonys 1\n")),
            Observation::Unreadable
        );
        assert_eq!(observe_with(&routed, Some("")), Observation::Unreadable);
    }

    const SAVED: &str = "9f1c-1:Tonys 1:802-11-wireless:wlan0\n\
                         9f1c-2:Cable:802-3-ethernet:enp5s0\n\
                         9f1c-3:Tonys 5G:802-11-wireless:\n";

    #[test]
    fn a_terse_field_separator_survives_a_name_that_contains_one() {
        // What `nmcli` really prints for a network called `A: B`.
        assert_eq!(
            split_terse(r"9f1c-1:A\: B:802-11-wireless:wlan0"),
            ["9f1c-1", "A: B", "802-11-wireless", "wlan0"]
        );
        // A literal backslash is escaped too, and does not eat the separator
        // after it.
        assert_eq!(split_terse(r"a\\:b"), [r"a\", "b"]);
        assert_eq!(split_terse(""), [""]);
    }

    #[test]
    fn only_saved_wireless_profiles_are_offered_and_the_attached_one_is_marked() {
        let saved = parse_saved_wifi(SAVED);

        // The ethernet profile is not something this menu can offer to join.
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[0].id, "9f1c-1");
        assert_eq!(saved[0].name, "Tonys 1");
        assert!(saved[0].active);
        // Nothing is claimed about being in range until a scan result says so.
        assert_eq!(saved[0].availability, Availability::Unknown);
        assert!(!saved[1].active);
    }

    #[test]
    fn a_listing_this_panel_cannot_key_on_keeps_only_its_first_row() {
        let duplicated = "9f1c-1:First:802-11-wireless:wlan0\n\
                          9f1c-1:Second:802-11-wireless:\n";

        let saved = parse_saved_wifi(duplicated);
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].name, "First");
        assert!(saved[0].active);
    }

    #[test]
    fn malformed_rows_are_skipped_rather_than_shifting_the_columns() {
        let hostile = "\n\
                       not a row\n\
                       :Nameless:802-11-wireless:wlan0\n\
                       9f1c-9:Short:802-11-wireless\n\
                       9f1c-8:Fine:802-11-wireless:\n";

        let saved = parse_saved_wifi(hostile);
        // Only the well-formed wireless row survives: a missing UUID has no
        // identity and a four-field row that is short is not a row.
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].id, "9f1c-8");
    }

    #[test]
    fn a_listing_longer_than_the_protocol_allows_is_cut_before_it_leaves() {
        let flood: String = (0..MAX_ROW_ITEMS * 4)
            .map(|index| format!("uuid-{index}:Net {index}:802-11-wireless:\n"))
            .collect();

        assert_eq!(parse_saved_wifi(&flood).len(), MAX_ROW_ITEMS);
    }

    #[test]
    fn hostile_text_is_bounded_in_the_units_the_host_counts() {
        // Every one of these is two UTF-16 units, so twice the limit of them
        // is four times what may cross the protocol.
        let dense = "😀".repeat(MAX_TEXT_UNITS * 2);
        let listing = format!("9f1c-1:{dense}:802-11-wireless:\n");

        let saved = parse_saved_wifi(&listing);
        assert_eq!(saved.len(), 1);
        assert!(saved[0].name.encode_utf16().count() <= MAX_TEXT_UNITS);
        // And the characters are whole, not half a surrogate pair.
        assert!(saved[0].name.chars().all(|character| character == '😀'));
    }

    #[test]
    fn a_scan_result_reports_the_strongest_reading_of_each_network() {
        let visible = parse_visible_wifi(
            " :Tonys 1:41\n\
             *:Tonys 1:77\n\
             :Tonys 5G:12\n",
        );

        assert_eq!(
            visible,
            [
                Visible {
                    ssid: "Tonys 1".to_owned(),
                    signal: 77,
                    in_use: true
                },
                Visible {
                    ssid: "Tonys 5G".to_owned(),
                    signal: 12,
                    in_use: false
                }
            ]
        );
        // A signal that is not a number is nothing learned, not zero.
        assert!(parse_visible_wifi(" :Tonys 1:strong\n").is_empty());
        // A hidden network prints an empty SSID and is not an entry.
        assert!(parse_visible_wifi(" ::63\n").is_empty());
        assert!(parse_visible_wifi("").is_empty());
    }

    /// The active profile is the one whose real network this session can name,
    /// because NetworkManager attaches one profile to a device at a time and
    /// the scan marks the access point in use.
    #[test]
    fn the_active_profile_is_related_to_the_access_point_in_use() {
        let networks = listed(read_known_networks(
            &Answer::Text(SAVED.to_owned()),
            &Answer::Text("*:Tonys 1:77\n :Guests:31\n".to_owned()),
        ));

        assert_eq!(networks[0].id, "9f1c-1");
        assert!(networks[0].active);
        assert_eq!(networks[0].ssid.as_deref(), Some("Tonys 1"));
        assert_eq!(networks[0].availability, Availability::InRange(77));
        assert_eq!(networks[0].availability.signal(), Some(77));
        assert_eq!(networks[0].availability.as_token(), "in-range");
    }

    /// A profile whose label is not its SSID. The old code compared `NAME`
    /// with the scanned SSIDs and reported this as out of range — a claim
    /// about the radio drawn from a naming convention.
    #[test]
    fn a_profile_named_differently_from_its_network_is_not_called_absent() {
        // The label is `Home`; the network in use is `Tonys 1`.
        let saved = "9f1c-7:Home:802-11-wireless:wlan0\n";
        let networks = listed(read_known_networks(
            &Answer::Text(saved.to_owned()),
            &Answer::Text("*:Tonys 1:77\n".to_owned()),
        ));

        // Its real network is read from the in-use row, not from its label.
        assert_eq!(networks[0].name, "Home");
        assert_eq!(networks[0].ssid.as_deref(), Some("Tonys 1"));
        assert_eq!(networks[0].availability, Availability::InRange(77));
    }

    /// The limitation, stated as a test. `nmcli connection show` has no SSID
    /// field, so an inactive profile's network cannot be learned in one
    /// bounded run — and `unknown` is what that must publish.
    #[test]
    fn a_profile_whose_network_cannot_be_named_is_unknown_not_out_of_range() {
        let networks = listed(read_known_networks(
            &Answer::Text(SAVED.to_owned()),
            &Answer::Text("*:Tonys 1:77\n".to_owned()),
        ));

        // `Tonys 5G` is saved and inactive. Its SSID is not knowable here, so
        // nothing is claimed about whether it is reachable.
        assert_eq!(networks[1].name, "Tonys 5G");
        assert_eq!(networks[1].ssid, None);
        assert_eq!(networks[1].availability, Availability::Unknown);
        assert_eq!(networks[1].availability.as_token(), "unknown");
        assert_eq!(networks[1].availability.signal(), None);
    }

    /// Out of range is only ever said about a network this session can name,
    /// after a scan that answered and did not contain it.
    #[test]
    fn out_of_range_needs_a_known_ssid_and_a_conclusive_scan() {
        let profile = KnownNetwork {
            id: "9f1c-1".to_owned(),
            name: "Home".to_owned(),
            ssid: Some("Tonys 1".to_owned()),
            active: false,
            availability: Availability::Unknown,
        };

        // The scan answered and this network is not in it. This is the only
        // way a row is ever called out of range.
        let scanned = parse_visible_wifi(" :Guests:31\n");
        let related = relate(vec![profile.clone()], &scanned);
        assert_eq!(related[0].availability, Availability::OutOfRange);
        assert_eq!(related[0].availability.as_token(), "out-of-range");

        // The same network, present in the scan.
        let scanned = parse_visible_wifi(" :Tonys 1:64\n");
        let related = relate(vec![profile.clone()], &scanned);
        assert_eq!(related[0].availability, Availability::InRange(64));

        // The same profile with no SSID: the scan cannot speak about it.
        let nameless = KnownNetwork {
            ssid: None,
            ..profile
        };
        let related = relate(vec![nameless], &parse_visible_wifi(" :Guests:31\n"));
        assert_eq!(related[0].availability, Availability::Unknown);
    }

    /// Two profiles for one network. The UUID separates them; the SSID does
    /// not, which is exactly why identity is the UUID.
    #[test]
    fn two_profiles_sharing_one_network_stay_two_rows() {
        let saved = "9f1c-a:Home:802-11-wireless:wlan0\n\
                     9f1c-b:Home spare:802-11-wireless:\n";
        let networks = listed(read_known_networks(
            &Answer::Text(saved.to_owned()),
            &Answer::Text("*:Tonys 1:77\n".to_owned()),
        ));

        assert_eq!(networks.len(), 2);
        assert_eq!(networks[0].id, "9f1c-a");
        assert_eq!(networks[1].id, "9f1c-b");
        // Only the attached one can be related to the network in use.
        assert_eq!(networks[0].ssid.as_deref(), Some("Tonys 1"));
        assert_eq!(networks[1].ssid, None);
        assert_eq!(networks[1].availability, Availability::Unknown);
    }

    #[test]
    fn multiple_active_radios_make_ssid_attribution_unknown() {
        let saved = "9f1c-a:Home:802-11-wireless:wlan0\n\
                     9f1c-b:Travel:802-11-wireless:wlan1\n";
        let networks = listed(read_known_networks(
            &Answer::Text(saved.to_owned()),
            &Answer::Text("*:Home AP:77\n*:Travel AP:66\n".to_owned()),
        ));

        assert_eq!(networks.len(), 2);
        assert!(networks.iter().all(|known| known.ssid.is_none()));
        assert!(networks
            .iter()
            .all(|known| known.availability == Availability::Unknown));
    }

    /// A scan with no access point in use names nothing, even for the profile
    /// that is attached.
    #[test]
    fn a_scan_with_nothing_in_use_names_no_profile() {
        let networks = listed(read_known_networks(
            &Answer::Text(SAVED.to_owned()),
            &Answer::Text(" :Guests:31\n".to_owned()),
        ));

        assert!(networks.iter().all(|known| known.ssid.is_none()));
        assert!(networks
            .iter()
            .all(|known| known.availability == Availability::Unknown));
    }

    /// A scan this poll could not read costs one word per row. It must not
    /// cost the list, and it must not be reported as "nothing is in range".
    #[test]
    fn an_unread_scan_leaves_availability_unknown_rather_than_absent() {
        for scan in [Answer::Missing, Answer::Unreadable] {
            let networks = listed(read_known_networks(&Answer::Text(SAVED.to_owned()), &scan));

            assert_eq!(networks.len(), 2);
            assert!(networks
                .iter()
                .all(|known| known.availability == Availability::Unknown));
            assert!(networks.iter().all(|known| known.ssid.is_none()));
        }
    }

    /// The three ways a list can fail to arrive, and the one that is a fact.
    #[test]
    fn a_tool_that_is_absent_is_not_a_tool_that_was_slow() {
        // Not installed: nothing will ever be listed here.
        assert_eq!(
            read_known_networks(&Answer::Missing, &Answer::Missing),
            Reading::Unavailable
        );
        // Installed and did not answer: nothing is concluded at all.
        assert_eq!(
            read_known_networks(&Answer::Unreadable, &Answer::Text(String::new())),
            Reading::Unreadable
        );
        // Answered with nothing saved: an empty list, as a fact.
        assert_eq!(
            read_known_networks(&Answer::Text(String::new()), &Answer::Text(String::new())),
            Reading::Listed(Vec::new())
        );
    }

    /// The provider survives having no default route. This is the defect the
    /// payload was rebuilt for: the future menu is most wanted precisely when
    /// there is nothing to be disconnected from.
    #[test]
    fn a_session_with_no_route_still_publishes_the_networks_it_could_join() {
        let networks = listed(read_known_networks(
            &Answer::Text(SAVED.to_owned()),
            &Answer::Text(String::new()),
        ));
        let mut held = inventory::Held::new();
        let published = held.observe(Reading::Listed(networks));

        let fields = payload(None, published).expect("an inventory is worth publishing");
        // No link, so no summary keys at all — not an empty string and not a
        // word like `offline` that nothing observed.
        assert!(!fields.contains_key("kind"));
        assert!(!fields.contains_key("connection"));
        assert_eq!(fields["networksState"], Value::from("fresh"));
        assert_eq!(fields["networks"].as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn a_confirmed_link_keeps_the_summary_keys_it_has_always_had() {
        let mut held = inventory::Held::new();
        let published = held.observe(Reading::Listed(Vec::new()));

        let fields = payload(Some(&wifi()), published).expect("a link is worth publishing");
        assert_eq!(fields["kind"], Value::from("wifi"));
        assert_eq!(fields["connection"], Value::from("Tonys 1"));
        // A confirmed empty inventory travels beside them as an empty array.
        assert_eq!(fields["networksState"], Value::from("fresh"));
        assert_eq!(fields["networks"], Value::Array(Vec::new()));
    }

    /// Nothing confirmed and nothing listed is nothing to publish. The
    /// provider is withdrawn rather than fabricating either half.
    #[test]
    fn no_link_and_no_conclusive_listing_publishes_nothing_at_all() {
        let mut held: inventory::Held<KnownNetwork> = inventory::Held::new();
        let pending = held.observe(Reading::Unreadable);

        assert_eq!(pending.as_token(), "pending");
        assert!(payload(None, pending).is_none());

        // A tool that is not installed is conclusive about itself, even though
        // it is neither a list nor a link, so that state crosses the protocol.
        let mut held: inventory::Held<KnownNetwork> = inventory::Held::new();
        let unavailable = held.observe(Reading::Unavailable);
        assert_eq!(unavailable.as_token(), "unavailable");
        let fields = payload(None, unavailable).expect("unavailability is publishable");
        assert_eq!(fields["networksState"], Value::from("unavailable"));
        assert!(!fields.contains_key("networks"));
        assert!(!fields.contains_key("kind"));
        assert!(!fields.contains_key("connection"));

        // With a link, the same states are published beside it rather than
        // pretending there is a list.
        let mut held: inventory::Held<KnownNetwork> = inventory::Held::new();
        let fields = payload(Some(&wifi()), held.observe(Reading::Unreadable))
            .expect("a link is worth publishing");
        assert_eq!(fields["networksState"], Value::from("pending"));
        assert!(!fields.contains_key("networks"));
    }

    /// A held list is published and says that it is held, so the surface never
    /// has to infer freshness from a length.
    #[test]
    fn a_held_list_travels_with_the_word_held() {
        let networks = listed(read_known_networks(
            &Answer::Text(SAVED.to_owned()),
            &Answer::Text(String::new()),
        ));
        let mut held = inventory::Held::new();
        held.observe(Reading::Listed(networks));

        let fields = payload(None, held.observe(Reading::Unreadable))
            .expect("the held inventory is worth publishing");
        assert_eq!(fields["networksState"], Value::from("held"));
        assert_eq!(fields["networks"].as_array().map(Vec::len), Some(2));
    }

    /// Each absent field is absent rather than a sentinel a surface would
    /// have to know to ignore.
    #[test]
    fn a_row_omits_what_was_never_read_instead_of_inventing_it() {
        let networks = listed(read_known_networks(
            &Answer::Text(SAVED.to_owned()),
            &Answer::Text("*:Tonys 1:77\n".to_owned()),
        ));
        let mut held = inventory::Held::new();
        let fields = payload(None, held.observe(Reading::Listed(networks)))
            .expect("an inventory is worth publishing");

        let rows = fields["networks"].as_array().expect("an array of rows");
        let active = rows[0].as_object().expect("a row");
        assert_eq!(active["ssid"], Value::from("Tonys 1"));
        assert_eq!(active["signal"], Value::from(77));
        assert_eq!(active["availability"], Value::from("in-range"));
        assert_eq!(active["id"], Value::from("9f1c-1"));

        // The inactive profile knows neither, and says so by omission.
        let unrelated = rows[1].as_object().expect("a row");
        assert!(!unrelated.contains_key("ssid"));
        assert!(!unrelated.contains_key("signal"));
        assert_eq!(unrelated["availability"], Value::from("unknown"));
    }

    /// The summary this widget has always published is untouched by any of the
    /// above: the same two commands still decide it, and the inventory is read
    /// beside it rather than through it.
    #[test]
    fn the_inventory_does_not_change_what_the_link_summary_says() {
        let routed = read_route(Some(
            "default via 192.168.1.1 dev wlan0 proto dhcp metric 600\n",
        ));

        assert_eq!(
            observe_with(&routed, Some(DEVICES)),
            Observation::Carrying(wifi())
        );
        assert_eq!(
            observe_with(&RouteReading::NoDefault, None),
            Observation::Offline
        );
        assert_eq!(
            observe_with(&RouteReading::Unreadable, None),
            Observation::Unreadable
        );
    }

    #[test]
    fn nothing_carrying_the_session_is_no_link_at_all() {
        let devices = parse_devices(DEVICES);

        // No default route: connected devices that carry nothing are not how
        // the session is online.
        assert_eq!(active_link(&devices, None), None);
        // A route through something NetworkManager does not manage.
        assert_eq!(active_link(&devices, Some("tun0")), None);
        // Loopback carries a route on a machine with nothing else, and is
        // still not a link.
        assert_eq!(active_link(&devices, Some("lo")), None);
    }
}
