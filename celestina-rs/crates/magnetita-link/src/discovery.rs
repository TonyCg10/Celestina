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

/// The port the daemon listens on: inside the 1714–1764 range the author's
/// firewall already admits, opened years ago for KDE Connect.
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

/// Most peers one browse reports; an advertisement flood is cut here.
const MAX_PEERS: usize = 64;

/// One advertisement as a resolver reports it, before any validation: the
/// service name, the host as an address literal, and the port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate<'a> {
    pub name: &'a str,
    pub host: &'a str,
    pub port: &'a str,
}

/// The discovery rule every browser applies, whatever resolves the service:
/// the name is a device id of at most 64 ASCII alphanumerics, the host an
/// address [`reachability_rank`] accepts and the port non-zero. What passes
/// comes back best address first, one entry per (id, address), stable
/// within a rank, and at most 64 peers.
pub fn rank_peers<'a>(candidates: impl IntoIterator<Item = Candidate<'a>>) -> Vec<Peer> {
    let mut found: Vec<(u8, Peer)> = Vec::new();
    for c in candidates {
        if found.len() >= MAX_PEERS {
            break;
        }
        if c.name.is_empty()
            || c.name.len() > MAX_DEVICE_ID
            || !c.name.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            continue;
        }
        let Ok(ip) = c.host.parse::<IpAddr>() else {
            continue;
        };
        let Ok(port) = c.port.parse::<u16>() else {
            continue;
        };
        if port == 0 {
            continue;
        }
        let Some(rank) = reachability_rank(ip) else {
            continue;
        };
        let peer = Peer {
            device_id: c.name.to_owned(),
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

/// The Magnetita peers in an `avahi-browse -rpt _magnetita._udp` output,
/// under [`rank_peers`].
pub fn parse_peers(output: &str) -> Vec<Peer> {
    rank_peers(
        output
            .lines()
            .filter_map(parse_resolved)
            .filter(|r| r.service_type == SERVICE_TYPE)
            .map(|r| Candidate {
                name: r.name,
                host: r.address,
                port: r.port,
            }),
    )
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
    fn candidates_from_any_resolver_follow_one_rule() {
        fn c<'a>(name: &'a str, host: &'a str, port: &'a str) -> Candidate<'a> {
            Candidate { name, host, port }
        }
        let long = "a".repeat(65);
        let peers = rank_peers([
            c("desk", "fd00::5", "1760"),
            c("desk", "192.168.1.5", "1760"),
            c("desk", "192.168.1.5", "1760"),
            c("desk", "fe80::1", "1760"),
            c("desk", "127.0.0.1", "1760"),
            c("desk", "desk.local", "1760"),
            c("desk", "192.168.1.6", "0"),
            c("desk", "192.168.1.6", "70000"),
            c("../x", "192.168.1.7", "1760"),
            c(&long, "192.168.1.8", "1760"),
        ]);
        assert_eq!(
            peers,
            vec![
                Peer {
                    device_id: "desk".into(),
                    address: "192.168.1.5:1760".parse().unwrap()
                },
                Peer {
                    device_id: "desk".into(),
                    address: "[fd00::5]:1760".parse().unwrap()
                },
            ]
        );
        let hosts: Vec<String> = (0..100).map(|n| format!("192.168.2.{n}")).collect();
        let many = rank_peers(hosts.iter().map(|h| c("desk", h, "1760")));
        assert_eq!(many.len(), MAX_PEERS);
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
