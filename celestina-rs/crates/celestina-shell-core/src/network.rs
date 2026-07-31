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

#[cfg(test)]
mod tests {
    use super::*;

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
