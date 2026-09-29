//! What a pairing link offers, read for the person before anything is
//! dialled (R-A16, AND-5). The consent screen shows [`preview`]'s offer and
//! pairs only after the person confirms it; the preview reads the link with
//! the very parse [`crate::Phone::pair`] then uses, so the screen can never
//! show one desktop while the core dials another.
//!
//! Beyond the parse, a link must name only addresses of a home LAN (RFC 1918
//! IPv4 or IPv6 unique-local) and none of this phone's own: the core dials
//! every address in order, and an address of this phone would be answered
//! by another application on it.

use std::net::{IpAddr, SocketAddr};

use magnetita_link::trust::fingerprint_text;
use magnetita_proto::QrPayload;

/// Why a pairing link was refused before the person is asked.
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairRefusal {
    /// Not a Magnetita pairing link, or longer than one may be.
    NotMagnetita,
    /// A Magnetita link the pairing parse refuses.
    Malformed,
    /// The link names no address.
    NoAddress,
    /// An address is not on a home LAN.
    NotLan,
    /// An address is one of this phone's own.
    ThisPhone,
    /// None of this phone's own addresses could be read, so a link to itself
    /// could not be told apart; refused rather than guessed.
    LocalUnknown,
}

/// A link the person may confirm.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct PairOffer {
    pub uri: String,
    /// The id the desktop's QR names.
    pub device_id: String,
    /// The pinned certificate's SHA-256 as colon-separated hex pairs, as the
    /// desktop prints fingerprints.
    pub fingerprint: String,
    /// Every `ip:port` the core would dial, in order.
    pub addresses: Vec<String>,
}

/// A link read for the consent screen.
#[derive(uniffi::Enum, Clone, Debug, PartialEq, Eq)]
pub enum PairPreview {
    Offer { offer: PairOffer },
    Refused { reason: PairRefusal },
}

/// Whether `text` is a Magnetita pairing link at all, for the scanner.
#[uniffi::export]
pub fn pair_link_accepts(text: String) -> bool {
    QrPayload::is_link(&text)
}

/// Reads `uri` for the consent screen. `local` holds this phone's interface
/// addresses as the platform prints them (`192.168.1.30`, `/fd00::5`,
/// `fd00::5%wlan0`); entries that are not addresses are ignored.
#[uniffi::export]
pub fn preview_pairing(uri: String, local: Vec<String>) -> PairPreview {
    match preview(&uri, &local) {
        Ok(offer) => PairPreview::Offer { offer },
        Err(reason) => PairPreview::Refused { reason },
    }
}

/// [`preview_pairing`] as a `Result`.
pub fn preview(uri: &str, local: &[String]) -> Result<PairOffer, PairRefusal> {
    if !QrPayload::is_link(uri) {
        return Err(PairRefusal::NotMagnetita);
    }
    let payload = QrPayload::parse_uri(uri).map_err(|_| PairRefusal::Malformed)?;
    if payload.addresses.is_empty() {
        return Err(PairRefusal::NoAddress);
    }
    let mut targets = Vec::with_capacity(payload.addresses.len());
    for address in &payload.addresses {
        match lan_address(address) {
            Some(target) => targets.push(target),
            None => return Err(PairRefusal::NotLan),
        }
    }
    let own: Vec<IpAddr> = local.iter().filter_map(|h| local_host(h)).collect();
    if own.is_empty() {
        return Err(PairRefusal::LocalUnknown);
    }
    if targets.iter().any(|t| own.contains(&t.ip())) {
        return Err(PairRefusal::ThisPhone);
    }
    Ok(PairOffer {
        uri: uri.to_owned(),
        device_id: payload.device_id,
        fingerprint: fingerprint_text(&payload.fingerprint),
        addresses: payload.addresses,
    })
}

/// `address` as the core dials it, when it is a literal `ip:port` on a home
/// LAN: RFC 1918 IPv4 or IPv6 `fc00::/7`, with a port and no scope. Loopback,
/// link-local, shared (CGNAT), public and IPv4-mapped addresses and host
/// names are refused.
pub fn lan_address(address: &str) -> Option<SocketAddr> {
    let target: SocketAddr = address.parse().ok()?;
    if target.port() == 0 {
        return None;
    }
    let lan = match target {
        SocketAddr::V4(v4) => v4.ip().is_private(),
        SocketAddr::V6(v6) => v6.scope_id() == 0 && (v6.ip().segments()[0] & 0xfe00) == 0xfc00,
    };
    lan.then_some(target)
}

/// One of this phone's own addresses as the platform prints it: a leading
/// `/` and a scope id are dropped, since a local address is this phone's on
/// any interface.
fn local_host(host: &str) -> Option<IpAddr> {
    let bare = host.strip_prefix('/').unwrap_or(host);
    let bare = bare.split_once('%').map_or(bare, |(ip, _)| ip);
    bare.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FP: &str = "00112233445566778899aabbccddeeff00112233445566778899AABBCCDDEEFF";

    fn secret() -> String {
        "ab".repeat(32)
    }

    fn link_as(id: &str, addresses: &[&str]) -> String {
        let mut uri = format!("magnetita://pair?v=1&id={id}&fp={FP}&secret={}", secret());
        for a in addresses {
            uri.push_str("&addr=");
            uri.push_str(a);
        }
        uri
    }

    fn link(addresses: &[&str]) -> String {
        link_as("desk1", addresses)
    }

    fn phone() -> Vec<String> {
        vec!["127.0.0.1".into(), "192.168.1.30".into()]
    }

    #[test]
    fn a_lan_link_is_offered_as_the_core_reads_it() {
        let uri = link(&["192.168.1.20:1760"]);
        let offer = preview(&uri, &phone()).unwrap();
        assert_eq!(offer.uri, uri);
        assert_eq!(offer.device_id, "desk1");
        assert_eq!(offer.addresses, ["192.168.1.20:1760"]);
        assert_eq!(
            offer.fingerprint,
            "00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff"
        );
        assert_eq!(preview_pairing(uri, phone()), PairPreview::Offer { offer });
    }

    #[test]
    fn links_the_core_would_refuse_are_refused() {
        let p = phone();
        assert_eq!(
            preview("https://example.org/?addr=192.168.1.2:1", &p),
            Err(PairRefusal::NotMagnetita)
        );
        let long = link(&["192.168.1.2:1760"]) + &"x".repeat(5000);
        assert_eq!(preview(&long, &p), Err(PairRefusal::NotMagnetita));
        assert_eq!(preview(&link(&[]), &p), Err(PairRefusal::NoAddress));
        let malformed = [
            format!(
                "magnetita://pair?v=1&fp={FP}&secret={}&addr=192.168.1.2:1760",
                secret()
            ),
            format!(
                "magnetita://pair?v=2&id=desk1&fp={FP}&secret={}&addr=192.168.1.2:1760",
                secret()
            ),
            format!(
                "magnetita://pair?id=desk1&fp={FP}&secret={}&addr=192.168.1.2:1760",
                secret()
            ),
            format!("magnetita://pair?v=1&id=desk1&fp={FP}&addr=192.168.1.2:1760"),
            format!("magnetita://pair?v=1&id=desk1&fp={FP}&secret=abc&addr=192.168.1.2:1760"),
            format!(
                "magnetita://pair?v=1&id=desk1&fp=abc&secret={}&addr=192.168.1.2:1760",
                secret()
            ),
            link_as("bad-id", &["192.168.1.2:1760"]),
            link(&["192.168.1.2:1760"]) + "&",
            link(&["192.168.1.2:1760"]) + "&addr=",
            // A repeated field would let the screen show one value and the core use another.
            link(&["192.168.1.2:1760"]) + "&id=evil",
            link(&["192.168.1.2:1760"]) + "&fp=" + &"0".repeat(64),
            link(&["192.168.1.2:1760"]) + "&v=1",
            link(&["192.168.1.2:1760"]) + "&secret=" + &secret(),
        ];
        for uri in &malformed {
            assert_eq!(preview(uri, &p), Err(PairRefusal::Malformed), "{uri}");
        }
    }

    #[test]
    fn a_public_address_refuses_the_whole_link() {
        let p = phone();
        assert_eq!(
            preview(&link(&["8.8.8.8:1760"]), &p),
            Err(PairRefusal::NotLan)
        );
        // The core dials every address, so one public address refuses all.
        assert_eq!(
            preview(&link(&["192.168.1.20:1760", "203.0.113.9:1760"]), &p),
            Err(PairRefusal::NotLan)
        );
    }

    #[test]
    fn an_address_of_this_phone_is_refused() {
        let local: Vec<String> = [
            "192.168.1.30",
            "/fd00:0:0:0:0:0:0:30%wlan0",
            "127.0.0.1",
            "wlan0",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        for own in [
            link(&["192.168.1.30:1760"]),
            link(&["[fd00::30]:1760"]),
            link(&["192.168.1.20:1760", "192.168.1.30:1760"]),
        ] {
            assert_eq!(preview(&own, &local), Err(PairRefusal::ThisPhone), "{own}");
        }
        assert!(preview(&link(&["192.168.1.20:1760"]), &local).is_ok());
    }

    #[test]
    fn no_readable_address_of_this_phone_refuses_every_link() {
        let uri = link(&["192.168.1.20:1760"]);
        assert_eq!(preview(&uri, &[]), Err(PairRefusal::LocalUnknown));
        assert_eq!(
            preview(&uri, &["wlan0".to_owned()]),
            Err(PairRefusal::LocalUnknown)
        );
    }

    #[test]
    fn only_private_and_unique_local_addresses_are_on_the_lan() {
        for lan in [
            "10.0.0.1:1760",
            "10.255.255.254:1",
            "172.16.0.1:1760",
            "172.31.255.1:1760",
            "192.168.0.10:65535",
            "[fd12:3456::1]:1760",
            "[fc00::1]:1760",
            "[fd00::192.168.1.2]:1760",
        ] {
            assert!(lan_address(lan).is_some(), "{lan}");
        }
        for refused in [
            "8.8.8.8:1760",
            "172.15.0.1:1760",
            "172.32.0.1:1760",
            "192.169.0.1:1760",
            "100.64.0.1:1760",
            "127.0.0.1:1760",
            "169.254.1.1:1760",
            "0.0.0.0:1760",
            "255.255.255.255:1760",
            "[2001:db8::1]:1760",
            "[fe80::1]:1760",
            "[::1]:1760",
            "[::ffff:192.168.1.2]:1760",
            "[fd00::1%2]:1760",
            "192.168.1.2",
            "192.168.1.2:0",
            "192.168.1.2:65536",
            "192.168.1.2:+1",
            "192.168.01.2:1760",
            "192.168.1:1760",
            "192.168.1.2.3:1760",
            "fd00::1:1760",
            "[fd00::1]1760",
            "[fd00:::1]:1760",
            "[fd00::1::2]:1760",
            "[1:2:3:4:5:6:7:8:9]:1760",
            "desktop.local:1760",
            "",
            ":1760",
        ] {
            assert!(lan_address(refused).is_none(), "{refused}");
        }
    }

    #[test]
    fn the_scanner_knows_a_pairing_link() {
        assert!(pair_link_accepts(link(&["10.0.0.1:1760"])));
        assert!(!pair_link_accepts("https://example.org".into()));
        assert!(!pair_link_accepts(
            "magnetita://pair?".to_owned() + &"x".repeat(5000)
        ));
    }
}
