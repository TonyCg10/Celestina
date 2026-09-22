//! Interfaces, as `/proc/net/dev` and `/sys/class/net` report them.
//!
//! Loopback is dropped: traffic to oneself is not what a person means by
//! "network". Wireless is a fact of the interface, read from the presence of
//! its `wireless` directory by the caller; the link speed file is absent or
//! invalid on Wi-Fi and on a cable that is down, which is why speed is an
//! `Option` and never an error.

use std::fmt;

/// `ARPHRD_ETHER`: Ethernet and Wi-Fi both report it.
pub const ARPHRD_ETHER: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceStat {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkError {
    TooFewFields { line: String },
    UnreadableNumber { line: String },
}

impl fmt::Display for NetworkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewFields { line } => write!(formatter, "net/dev line is too short: {line}"),
            Self::UnreadableNumber { line } => {
                write!(formatter, "network value is not a number: {line}")
            }
        }
    }
}

impl std::error::Error for NetworkError {}

/// Parses `/proc/net/dev`, dropping `lo`. The two header lines are skipped
/// by shape (no `:`), not by count.
///
/// # Errors
///
/// A data line with fewer than the sixteen counters, or a non-numeric byte
/// count.
pub fn parse_net_dev(text: &str) -> Result<Vec<InterfaceStat>, NetworkError> {
    let mut interfaces = Vec::new();
    for line in text.lines() {
        let Some((name, counters)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name == "lo" {
            continue;
        }
        let fields: Vec<&str> = counters.split_whitespace().collect();
        // 8 receive + 8 transmit counters; bytes are the first of each half.
        if fields.len() < 16 {
            return Err(NetworkError::TooFewFields {
                line: line.to_owned(),
            });
        }
        let number = |index: usize| -> Result<u64, NetworkError> {
            fields[index]
                .parse::<u64>()
                .map_err(|_| NetworkError::UnreadableNumber {
                    line: line.to_owned(),
                })
        };
        interfaces.push(InterfaceStat {
            name: name.to_owned(),
            rx_bytes: number(0)?,
            tx_bytes: number(8)?,
        });
    }
    Ok(interfaces)
}

/// `/sys/class/net/IF/type`.
///
/// # Errors
///
/// Not one integer.
pub fn parse_link_type(text: &str) -> Result<u32, NetworkError> {
    text.trim()
        .parse::<u32>()
        .map_err(|_| NetworkError::UnreadableNumber {
            line: text.trim().to_owned(),
        })
}

/// `/sys/class/net/IF/operstate` is `up`.
#[must_use]
pub fn parse_operstate(text: &str) -> bool {
    text.trim() == "up"
}

/// `/sys/class/net/IF/speed` in Mbit/s; `None` when the kernel does not know.
#[must_use]
pub fn parse_speed_mbit(text: &str) -> Option<u32> {
    text.trim().parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NET_DEV: &str = "\
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 1157448090 20765012    0    0    0     0          0         0 1157448090 20765012    0    0    0     0       0          0
enp9s0: 21272700923 25711259    0    0    0     0          0      2003 96284682 1092621    0   73    0     0       0          0
 wlan0: 266919562  395078    0    0    0     0          0         0 186592009  316705    0    0    0     0       0          0
";

    #[test]
    fn net_dev_drops_loopback_and_reads_bytes_both_ways() {
        let interfaces = parse_net_dev(NET_DEV).expect("readable net/dev");
        assert_eq!(interfaces.len(), 2);
        assert_eq!(
            interfaces[0],
            InterfaceStat {
                name: "enp9s0".to_owned(),
                rx_bytes: 21_272_700_923,
                tx_bytes: 96_284_682
            }
        );
        assert_eq!(interfaces[1].name, "wlan0");
        assert_eq!(interfaces[1].tx_bytes, 186_592_009);
    }

    #[test]
    fn a_short_or_unreadable_net_dev_line_is_refused() {
        assert!(matches!(
            parse_net_dev("eth0: 1 2 3\n"),
            Err(NetworkError::TooFewFields { .. })
        ));
        assert!(matches!(
            parse_net_dev("eth0: x 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16\n"),
            Err(NetworkError::UnreadableNumber { .. })
        ));
        assert_eq!(parse_net_dev("").expect("empty is fine"), vec![]);
    }

    #[test]
    fn sysfs_helpers_tolerate_what_wifi_and_a_down_cable_report() {
        assert_eq!(parse_link_type("1\n"), Ok(ARPHRD_ETHER));
        assert!(matches!(
            parse_link_type("ether\n"),
            Err(NetworkError::UnreadableNumber { .. })
        ));
        assert!(parse_operstate("up\n"));
        assert!(!parse_operstate("down\n"));
        assert!(!parse_operstate("unknown\n"));
        assert_eq!(parse_speed_mbit("1000\n"), Some(1000));
        assert_eq!(parse_speed_mbit("-1\n"), None);
        assert_eq!(parse_speed_mbit(""), None);
    }
}
