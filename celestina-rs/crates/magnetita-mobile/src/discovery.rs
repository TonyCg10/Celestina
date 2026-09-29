//! Finding the desktop again (AND-5): the rule that ranks what the phone's
//! resolver saw, and the schedule between attempts, both the link's own.
//! Android's `NsdManager` resolves `_magnetita._udp` and hands the raw
//! results here; nothing about which of them to dial is decided in Kotlin.

use std::sync::{Arc, Mutex};

use magnetita_link::discovery::{rank_peers, Candidate};
use magnetita_link::Backoff;

/// One resolved advertisement as the platform reports it.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    /// The service name, which is the desktop's device id.
    pub name: String,
    /// The host as an address literal; a scope suffix (`%wlan0`) or a
    /// leading `/` is tolerated and dropped.
    pub host: String,
    pub port: u16,
}

/// A desktop worth dialling, best first.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct AdvertisedDesktop {
    pub device_id: String,
    /// `ip:port`, IPv6 in brackets, as [`crate::mobile::MobilePhone::connect`]
    /// takes it.
    pub address: String,
}

/// The resolutions the link's discovery rule keeps, best address first and
/// one entry per (id, address).
#[uniffi::export]
pub fn rank_advertised(found: Vec<Resolution>) -> Vec<AdvertisedDesktop> {
    let hosts: Vec<(String, &str, String)> = found
        .iter()
        .map(|r| {
            let bare = r.host.strip_prefix('/').unwrap_or(&r.host);
            let bare = bare.split_once('%').map_or(bare, |(ip, _)| ip);
            (bare.to_owned(), r.name.as_str(), r.port.to_string())
        })
        .collect();
    rank_peers(
        hosts
            .iter()
            .map(|(host, name, port)| Candidate { name, host, port }),
    )
    .into_iter()
    .map(|p| AdvertisedDesktop {
        device_id: p.device_id,
        address: p.address.to_string(),
    })
    .collect()
}

/// The reconnection schedule, the desktop link's [`Backoff`]: a quarter
/// second doubling to a minute, reset by a session that was established.
#[derive(uniffi::Object, Default)]
pub struct ReconnectSchedule {
    backoff: Mutex<Backoff>,
}

#[uniffi::export]
impl ReconnectSchedule {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// How long to wait before the next attempt, in ms; counts the attempt.
    pub fn next_delay_ms(&self) -> u64 {
        let delay = self
            .backoff
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .next_delay();
        u64::try_from(delay.as_millis()).unwrap_or(u64::MAX)
    }

    /// A session was established: the next failure starts over.
    pub fn reset(&self) {
        self.backoff
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(name: &str, host: &str, port: u16) -> Resolution {
        Resolution {
            name: name.into(),
            host: host.into(),
            port,
        }
    }

    #[test]
    fn the_resolver_results_follow_the_links_rule() {
        let ranked = rank_advertised(vec![
            seen("desk", "fd00::5%wlan0", 1760),
            seen("desk", "/192.168.1.5", 1760),
            seen("desk", "192.168.1.5", 1760),
            seen("desk", "fe80::1%wlan0", 1760),
            seen("desk", "127.0.0.1", 1760),
            seen("desk", "192.168.1.6", 0),
            seen("../etc", "192.168.1.7", 1760),
            seen(&"a".repeat(65), "192.168.1.8", 1760),
        ]);
        assert_eq!(
            ranked,
            vec![
                AdvertisedDesktop {
                    device_id: "desk".into(),
                    address: "192.168.1.5:1760".into()
                },
                AdvertisedDesktop {
                    device_id: "desk".into(),
                    address: "[fd00::5]:1760".into()
                },
            ]
        );
    }

    #[test]
    fn the_schedule_doubles_to_a_minute_and_resets() {
        let schedule = ReconnectSchedule::new();
        let delays: Vec<u64> = (0..11).map(|_| schedule.next_delay_ms()).collect();
        assert_eq!(&delays[..5], &[250, 500, 1000, 2000, 4000]);
        assert_eq!(delays[9], 60_000);
        assert_eq!(delays[10], 60_000);
        schedule.reset();
        assert_eq!(schedule.next_delay_ms(), 250);
    }
}
