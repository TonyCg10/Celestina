// Nothing calls this yet: MAG-R1-A delivers discovery and its verification,
// and MAG-R1-C is the unit that wires it to the link and the D-Bus contract.
#![allow(dead_code)]

//! Finding the phone's wireless-debugging ports on the LAN.
//!
//! Android randomises the ADB port every time the Wireless debugging switch is
//! toggled, and announces the current one over mDNS. This module asks the
//! running Avahi daemon what is advertised right now, and hands back validated
//! [`MirrorEndpoint`]s.
//!
//! **Why `avahi-browse` and not Avahi's D-Bus API.** The daemon already owns a
//! `zbus` connection, so calling Avahi over D-Bus is possible. But Avahi's
//! browse API is signal-driven: a browser object emits `ItemNew`/`ItemRemove`,
//! and `zbus`'s blocking `SignalIterator::next` has no timeout, so a watcher
//! thread parked on it cannot be shut down deterministically — exactly the
//! defect `MAG-M1` exists to remove elsewhere. `avahi-browse -rpt` terminates on
//! its own, is a small desktop tool like the `playerctl`/`sshfs`/`wl-paste` this
//! daemon already drives, and goes through [`subprocess`], which already owns
//! the deadline, the cancellation flag and the process-group reaping. Polling a
//! terminating command is the cheaper correctness.
//!
//! Everything this reads is peer-chosen text from an unauthenticated LAN, and
//! its address and port are about to become `adb` arguments, so a line that
//! does not parse exactly is refused rather than salvaged. Validation itself
//! lives in [`magnetita_core::mirror`], which is where these become typed.

use std::net::IpAddr;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use magnetita_core::mirror::{valid_service_name, AdbService, MirrorEndpoint};

use crate::subprocess;

/// How long a browse may take before it is abandoned. `avahi-browse -t` exits
/// once the daemon says the cache is exhausted, which is well inside this.
const BROWSE_BUDGET: Duration = Duration::from_secs(4);

/// One resolved advertisement, after validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Advertisement {
    pub(crate) service: AdbService,
    pub(crate) name: String,
    pub(crate) endpoint: MirrorEndpoint,
}

/// Asks Avahi what is advertising `service` right now, best candidate first.
///
/// Empty means nothing is advertised, which is the ordinary state when Wireless
/// debugging is off — not a fault, and not distinguishable from one here.
pub(crate) fn browse(service: AdbService, stopping: &AtomicBool) -> Vec<Advertisement> {
    let deadline = Instant::now() + BROWSE_BUDGET;
    let Some(output) = subprocess::command_output_from(
        "avahi-browse",
        &["-rpt", service.service_type()],
        deadline,
        stopping,
    ) else {
        return Vec::new();
    };
    // Avahi's parsable output is ASCII-safe, but a peer picks the service name,
    // so decode lossily rather than refusing the whole batch over one byte.
    parse_browse(&String::from_utf8_lossy(&output), service)
}

/// Turns `avahi-browse -rpt` output into ranked advertisements.
///
/// Only resolved (`=`) lines carry an address and port; the `+` lines that
/// precede them are announcements of existence only. One service resolves once
/// per interface and address family — on this host a single advertisement came
/// back five times, over `wlan0`, `enp9s0` and `lo`, in both families — so the
/// candidates are ranked and de-duplicated rather than taken first-seen.
pub(crate) fn parse_browse(output: &str, expected: AdbService) -> Vec<Advertisement> {
    let mut found: Vec<(u8, Advertisement)> = Vec::new();

    for line in output.lines() {
        let Some(advertisement) = parse_resolved_line(line, expected) else {
            continue;
        };
        let Some(rank) = reachability_rank(advertisement.endpoint.host) else {
            continue;
        };
        if found.iter().any(|(_, seen)| *seen == advertisement) {
            continue;
        }
        found.push((rank, advertisement));
    }

    found.sort_by_key(|(rank, _)| *rank);
    found.into_iter().map(|(_, found)| found).collect()
}

/// One `=` line, or `None` for anything that is not a well-formed resolution of
/// the service we asked for.
fn parse_resolved_line(line: &str, expected: AdbService) -> Option<Advertisement> {
    // `=;iface;proto;name;type;domain;host;address;port;txt`
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

    // The type must be the one asked for: a browse is per-type, but the field
    // is text from the network and is what tells pairing from connecting.
    if AdbService::from_service_type(service_type)? != expected {
        return None;
    }
    if !valid_service_name(name) {
        return None;
    }
    let port: u32 = port.parse().ok()?;
    let endpoint = MirrorEndpoint::parse(address, port).ok()?;

    Some(Advertisement {
        service: expected,
        name: name.to_owned(),
        endpoint,
    })
}

/// How likely this address is to actually reach the phone — lower is better.
/// `None` for an address that cannot be it at all.
///
/// Loopback is refused outright: on this host the advertisement resolved on
/// `lo` as `127.0.0.1`, and connecting `adb` there would target the desktop.
/// Link-local IPv6 is refused because it carries no scope here and would need
/// the interface index to be usable at all. Of the rest, IPv4 is preferred:
/// Android's wireless debugging is reached over IPv4 in practice, and it is the
/// family the author's working script used.
fn reachability_rank(host: IpAddr) -> Option<u8> {
    match host {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured verbatim from this host on 2026-08-19 with a stand-in service
    /// published by `avahi-publish -s adb-FAKE123-test _adb-tls-connect._tcp
    /// 37059`, which is how the parser was verified without the phone.
    const OBSERVED: &str = "\
+;wlan0;IPv6;adb-FAKE123-test;_adb-tls-connect._tcp;local
+;wlan0;IPv4;adb-FAKE123-test;_adb-tls-connect._tcp;local
+;lo;IPv4;adb-FAKE123-test;_adb-tls-connect._tcp;local
=;wlan0;IPv6;adb-FAKE123-test;_adb-tls-connect._tcp;local;cachyos-9.local;2601:403:c487:dad0::2fc1;37059;
=;wlan0;IPv4;adb-FAKE123-test;_adb-tls-connect._tcp;local;cachyos-9.local;10.0.0.134;37059;
=;enp9s0;IPv6;adb-FAKE123-test;_adb-tls-connect._tcp;local;cachyos-9.local;fe80::12ff:e0ff:feb7:9294;37059;
=;enp9s0;IPv4;adb-FAKE123-test;_adb-tls-connect._tcp;local;cachyos-9.local;10.50.0.1;37059;
=;lo;IPv4;adb-FAKE123-test;_adb-tls-connect._tcp;local;cachyos-9.local;127.0.0.1;37059;
";

    #[test]
    fn the_observed_output_yields_only_usable_addresses() {
        let found = parse_browse(OBSERVED, AdbService::Connect);
        let addresses: Vec<String> = found.iter().map(|found| found.endpoint.serial()).collect();

        // Loopback would point adb at this desktop; the link-local IPv6 has no
        // scope here. Neither survives.
        assert_eq!(
            addresses,
            vec![
                "10.0.0.134:37059",
                "10.50.0.1:37059",
                "[2601:403:c487:dad0::2fc1]:37059",
            ]
        );
        assert!(found.iter().all(|f| f.name == "adb-FAKE123-test"));
        assert!(found.iter().all(|f| f.service == AdbService::Connect));
    }

    /// The real S25U, captured on 2026-08-19 with Wireless debugging on. Kept
    /// as a fixture because it differs from the stand-in in three ways that
    /// each could have broken the parser: the phone answers on one interface
    /// only, both the IPv4 and IPv6 rows resolve to the *same* A record, and
    /// every line carries a TXT block the stand-in had none of.
    const OBSERVED_PHONE: &str = "\
+;wlan0;IPv6;adb-RFCY60WBFAH-Cjvgoe;_adb-tls-connect._tcp;local
+;wlan0;IPv4;adb-RFCY60WBFAH-Cjvgoe;_adb-tls-connect._tcp;local
=;wlan0;IPv6;adb-RFCY60WBFAH-Cjvgoe;_adb-tls-connect._tcp;local;Android.local;10.0.0.190;39799;\"api=36.1\" \"name=SM-S938U\" \"v=1\"
=;wlan0;IPv4;adb-RFCY60WBFAH-Cjvgoe;_adb-tls-connect._tcp;local;Android.local;10.0.0.190;39799;\"api=36.1\" \"name=SM-S938U\" \"v=1\"
";

    #[test]
    fn the_real_phone_resolves_to_one_endpoint() {
        let found = parse_browse(OBSERVED_PHONE, AdbService::Connect);
        // Two resolved lines, one endpoint: the address is the same A record
        // under both protocols, and a duplicate must not become a second
        // candidate to try.
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].endpoint.serial(), "10.0.0.190:39799");
        assert_eq!(found[0].name, "adb-RFCY60WBFAH-Cjvgoe");
    }

    #[test]
    fn a_txt_record_does_not_disturb_the_fields_before_it() {
        // The TXT block is the last field and carries quotes and spaces; the
        // port must still be read from its own field, not from the tail.
        let found = parse_browse(OBSERVED_PHONE, AdbService::Connect);
        assert_eq!(found[0].endpoint.port, 39799);
    }

    #[test]
    fn announcement_lines_carry_no_endpoint_and_are_ignored() {
        let announcements: String = OBSERVED
            .lines()
            .filter(|line| line.starts_with('+'))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(parse_browse(&announcements, AdbService::Connect).is_empty());
    }

    #[test]
    fn a_browse_for_one_service_never_returns_the_other() {
        assert!(parse_browse(OBSERVED, AdbService::Pairing).is_empty());
    }

    #[test]
    fn a_malformed_line_is_refused_not_salvaged() {
        for line in [
            "=;wlan0;IPv4;phone;_adb-tls-connect._tcp;local;h;10.0.0.85",
            "=;wlan0;IPv4;phone;_adb-tls-connect._tcp;local;h;10.0.0.85;notaport;",
            "=;wlan0;IPv4;phone;_adb-tls-connect._tcp;local;h;10.0.0.85;0;",
            "=;wlan0;IPv4;phone;_adb-tls-connect._tcp;local;h;phone.local;37059;",
            "=;wlan0;IPv4;;_adb-tls-connect._tcp;local;h;10.0.0.85;37059;",
            "=;wlan0;IPv4;phone;_workstation._tcp;local;h;10.0.0.85;37059;",
        ] {
            assert!(
                parse_browse(line, AdbService::Connect).is_empty(),
                "accepted: {line}"
            );
        }
    }

    #[test]
    fn the_same_endpoint_on_two_interfaces_is_reported_once() {
        let twice = "\
=;wlan0;IPv4;phone;_adb-tls-connect._tcp;local;h;10.0.0.85;37059;
=;enp9s0;IPv4;phone;_adb-tls-connect._tcp;local;h;10.0.0.85;37059;
";
        assert_eq!(parse_browse(twice, AdbService::Connect).len(), 1);
    }

    #[test]
    fn pairing_advertisements_parse_the_same_way() {
        let pairing =
            "=;wlan0;IPv4;adb-39121FDJ-Xy7Kq2;_adb-tls-pairing._tcp;local;h;10.0.0.85;44311;\n";
        let found = parse_browse(pairing, AdbService::Pairing);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].endpoint.serial(), "10.0.0.85:44311");
    }
}
