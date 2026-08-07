//! How the session is actually online.
//!
//! NetworkManager knows every device; the routing table knows which one is
//! carrying traffic. The panel wants the second question answered, so this
//! reads both and reports the link the default route goes through — not the
//! first connected device, which on a machine with both cable and wifi would
//! be a coin toss.

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

#[cfg(test)]
mod tests {
    use super::*;

    fn wifi() -> Link {
        Link {
            kind: "wifi".to_owned(),
            connection: "Tonys 1".to_owned(),
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
