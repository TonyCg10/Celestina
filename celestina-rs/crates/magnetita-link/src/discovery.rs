//! What the LAN says about who is here: the `_magnetita._udp` service, and
//! the one parser for `avahi-browse -rpt` output that every consumer shares.
//!
//! Advertising and browsing are subprocess calls the daemon and the peer
//! own; this module owns only the pure parts — the service type, the shape
//! of a resolved line, and the ranking of addresses — so the daemon's mirror
//! discovery, its link discovery and the headless peer read Avahi through
//! one recipe.

use std::net::{IpAddr, SocketAddr};

/// The service both ends advertise while the link is listening.
pub const SERVICE_TYPE: &str = "_magnetita._udp";

/// The port the daemon listens on: inside the range the author's firewall
/// already admits for KDE Connect, and one no KDE Connect socket uses.
pub const PORT: u16 = 1760;

/// One resolved (`=`) line of `avahi-browse -rpt`, before any validation of
/// what it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved<'a> {
    pub name: &'a str,
    pub service_type: &'a str,
    pub address: &'a str,
    pub port: &'a str,
    pub txt: &'a str,
}

/// Splits one line; `None` for anything but a resolution.
pub fn parse_resolved(line: &str) -> Option<Resolved<'_>> {
    let mut fields = line.split(';');
    if fields.next()? != "=" {
        return None;
    }
    let _interface = fields.next()?;
    let _protocol = fields.next()?;
    let name = fields.next()?;
    let service_type = fields.next()?;
    let _domain = fields.next()?;
    let _host = fields.next()?;
    let address = fields.next()?;
    let port = fields.next()?;
    let txt = fields.next().unwrap_or("");
    Some(Resolved {
        name,
        service_type,
        address,
        port,
        txt,
    })
}

/// How likely this address is to reach the peer — lower is better; `None`
/// for one that cannot be it at all. Loopback is refused outright: a
/// service resolved on `lo` names this host, not the phone. Link-local
/// addresses need a scope this parser does not carry, so they are refused
/// too; on the author's LAN a phone always has a routable address.
pub fn reachability_rank(address: IpAddr) -> Option<u8> {
    match address {
        IpAddr::V4(v4) => {
            if v4.is_loopback() || v4.is_link_local() || v4.is_unspecified() {
                None
            } else {
                Some(0)
            }
        }
        IpAddr::V6(v6) => {
            let link_local = (v6.segments()[0] & 0xffc0) == 0xfe80;
            if v6.is_loopback() || v6.is_unspecified() || link_local {
                None
            } else {
                Some(1)
            }
        }
    }
}

/// A Magnetita peer the LAN advertises.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    /// The service name is the device id.
    pub device_id: String,
    pub address: SocketAddr,
}

/// Longest device id accepted from the network.
const MAX_DEVICE_ID: usize = 64;

/// The Magnetita peers in an `avahi-browse -rpt _magnetita._udp` output,
/// best address first, one entry per (id, address).
pub fn parse_peers(output: &str) -> Vec<Peer> {
    let mut found: Vec<(u8, Peer)> = Vec::new();
    for line in output.lines() {
        let Some(r) = parse_resolved(line) else {
            continue;
        };
        if r.service_type != SERVICE_TYPE {
            continue;
        }
        if r.name.is_empty()
            || r.name.len() > MAX_DEVICE_ID
            || !r.name.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            continue;
        }
        let Ok(ip) = r.address.parse::<IpAddr>() else {
            continue;
        };
        let Ok(port) = r.port.parse::<u16>() else {
            continue;
        };
        if port == 0 {
            continue;
        }
        let Some(rank) = reachability_rank(ip) else {
            continue;
        };
        let peer = Peer {
            device_id: r.name.to_owned(),
            address: SocketAddr::new(ip, port),
        };
        if found.iter().any(|(_, p)| *p == peer) {
            continue;
        }
        found.push((rank, peer));
    }
    found.sort_by_key(|(rank, _)| *rank);
    found.into_iter().map(|(_, p)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTPUT: &str = "\
+;wlan0;IPv4;0eb21b28ae74d54c;_magnetita._udp;local
=;lo;IPv4;0eb21b28ae74d54c;_magnetita._udp;local;desk.local;127.0.0.1;1760;\"name=Escritorio\"
=;wlan0;IPv6;0eb21b28ae74d54c;_magnetita._udp;local;desk.local;fe80::1;1760;\"name=Escritorio\"
=;wlan0;IPv4;0eb21b28ae74d54c;_magnetita._udp;local;desk.local;10.0.0.134;1760;\"name=Escritorio\"
=;wlan0;IPv4;0eb21b28ae74d54c;_magnetita._udp;local;desk.local;10.0.0.134;1760;\"name=Escritorio\"
=;wlan0;IPv4;../etc;_magnetita._udp;local;x.local;10.0.0.9;1760;
=;wlan0;IPv4;abc;_adb-tls-connect._tcp;local;x.local;10.0.0.9;5555;
=;wlan0;IPv4;abc;_magnetita._udp;local;x.local;10.0.0.9;0;
";

    #[test]
    fn peers_are_validated_ranked_and_deduplicated() {
        let peers = parse_peers(OUTPUT);
        assert_eq!(
            peers,
            vec![Peer {
                device_id: "0eb21b28ae74d54c".into(),
                address: "10.0.0.134:1760".parse().unwrap()
            }]
        );
    }

    #[test]
    fn a_resolved_line_splits_into_its_fields() {
        let r = parse_resolved("=;wlan0;IPv4;n;_t._udp;local;h.local;10.0.0.1;5;\"k=v\"").unwrap();
        assert_eq!(
            (r.name, r.service_type, r.address, r.port, r.txt),
            ("n", "_t._udp", "10.0.0.1", "5", "\"k=v\"")
        );
        assert!(parse_resolved("+;wlan0;IPv4;n;_t._udp;local").is_none());
    }
}
