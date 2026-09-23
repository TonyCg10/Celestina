//! Sensors, as hwmon lays them out: one directory per chip, one file per
//! channel attribute, integers in the kernel's fixed units. The caller reads
//! the files; this module says which ones matter, what they mean and what a
//! person's unit of each is.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChipListing {
    /// The directory name, `hwmonN`: stable for the session, not across boots.
    pub key: String,
    /// The kernel driver's name for the chip, from its `name` file.
    pub name: String,
    /// `(file name, contents)` for every file the caller could read.
    pub files: Vec<(String, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChannelKind {
    Temperature,
    Fan,
    Voltage,
    Power,
    Current,
}

impl ChannelKind {
    #[must_use]
    pub fn from_prefix(prefix: &str) -> Option<Self> {
        match prefix {
            "temp" => Some(Self::Temperature),
            "fan" => Some(Self::Fan),
            "in" => Some(Self::Voltage),
            "power" => Some(Self::Power),
            "curr" => Some(Self::Current),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Temperature => "temperature",
            Self::Fan => "fan",
            Self::Voltage => "voltage",
            Self::Power => "power",
            Self::Current => "current",
        }
    }

    #[must_use]
    pub fn unit(self) -> &'static str {
        match self {
            Self::Temperature => "°C",
            Self::Fan => "rpm",
            Self::Voltage => "V",
            Self::Power => "W",
            Self::Current => "A",
        }
    }
}

impl fmt::Display for ChannelKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub kind: ChannelKind,
    pub index: u32,
    pub label: String,
    pub value: f64,
    pub limit_max: Option<f64>,
    pub limit_crit: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chip {
    pub key: String,
    pub name: String,
    pub channels: Vec<Channel>,
}

/// `temp3_crit` → `(Temperature, 3, "crit")`; anything else `None`.
#[must_use]
pub fn parse_channel_file(name: &str) -> Option<(ChannelKind, u32, &str)> {
    let (head, attribute) = name.split_once('_')?;
    let digits_at = head.find(|c: char| c.is_ascii_digit())?;
    let (prefix, digits) = head.split_at(digits_at);
    let kind = ChannelKind::from_prefix(prefix)?;
    let index = digits.parse::<u32>().ok()?;
    (!attribute.is_empty()).then_some((kind, index, attribute))
}

/// The kernel's integer in a person's unit.
#[must_use]
pub fn convert(kind: ChannelKind, raw: i64) -> f64 {
    let raw = raw as f64;
    match kind {
        ChannelKind::Temperature | ChannelKind::Voltage | ChannelKind::Current => raw / 1000.0,
        ChannelKind::Fan => raw,
        ChannelKind::Power => raw / 1_000_000.0,
    }
}

/// The highest temperature limit taken as real, in °C. Drivers publish an
/// all-ones sentinel for "no limit" (an NVMe `temp_max` reads 65 261.85 °C),
/// and a limit nobody can reach is not a limit.
pub const TEMPERATURE_LIMIT_CEILING: f64 = 200.0;
/// The lowest temperature limit taken as real, in °C.
pub const TEMPERATURE_LIMIT_FLOOR: f64 = -100.0;
/// The highest fan limit taken as real, in rpm.
pub const FAN_LIMIT_CEILING: f64 = 100_000.0;
/// The highest voltage limit taken as real, in V.
pub const VOLTAGE_LIMIT_CEILING: f64 = 1_000.0;
/// The highest power limit taken as real, in W.
pub const POWER_LIMIT_CEILING: f64 = 100_000.0;

/// A converted limit, or `None` when it lies outside what the kind can
/// physically mean — a driver's sentinel rather than a limit. A current limit
/// has no known sentinel and is kept as read.
#[must_use]
pub fn plausible_limit(kind: ChannelKind, value: f64) -> Option<f64> {
    let plausible = match kind {
        ChannelKind::Temperature => {
            (TEMPERATURE_LIMIT_FLOOR..=TEMPERATURE_LIMIT_CEILING).contains(&value)
        }
        ChannelKind::Fan => value <= FAN_LIMIT_CEILING,
        ChannelKind::Voltage => value <= VOLTAGE_LIMIT_CEILING,
        ChannelKind::Power => value <= POWER_LIMIT_CEILING,
        ChannelKind::Current => true,
    };
    plausible.then_some(value)
}

/// One channel per readable `<kind><n>_input` (a power channel prefers
/// `_average`), with its label and limits when the chip has them, sorted by
/// kind then index. A value that is not an integer skips its channel: a chip
/// with one broken file still shows its other readings.
#[must_use]
pub fn discover(listing: &ChipListing) -> Chip {
    use std::collections::BTreeMap;
    // (kind, index) → attribute → contents
    let mut table: BTreeMap<(ChannelKind, u32), BTreeMap<&str, &str>> = BTreeMap::new();
    for (name, contents) in &listing.files {
        if let Some((kind, index, attribute)) = parse_channel_file(name) {
            table
                .entry((kind, index))
                .or_default()
                .insert(attribute, contents.as_str());
        }
    }
    let number =
        |text: Option<&&str>| -> Option<i64> { text.and_then(|t| t.trim().parse::<i64>().ok()) };
    let mut channels = Vec::new();
    for ((kind, index), attributes) in table {
        let value_file = if kind == ChannelKind::Power && attributes.contains_key("average") {
            "average"
        } else {
            "input"
        };
        let Some(raw) = number(attributes.get(value_file)) else {
            continue;
        };
        let limit_max = match kind {
            ChannelKind::Power => {
                number(attributes.get("cap")).or_else(|| number(attributes.get("max")))
            }
            _ => number(attributes.get("max")),
        }
        .and_then(|raw| plausible_limit(kind, convert(kind, raw)));
        let limit_crit = number(attributes.get("crit"))
            .and_then(|raw| plausible_limit(kind, convert(kind, raw)));
        channels.push(Channel {
            kind,
            index,
            label: attributes
                .get("label")
                .map_or(String::new(), |l| l.trim().to_owned()),
            value: convert(kind, raw),
            limit_max,
            limit_crit,
        });
    }
    Chip {
        key: listing.key.clone(),
        name: listing.name.clone(),
        channels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(files: &[(&str, &str)]) -> ChipListing {
        ChipListing {
            key: "hwmon3".to_owned(),
            name: "amdgpu".to_owned(),
            files: files
                .iter()
                .map(|(n, c)| ((*n).to_owned(), (*c).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn channel_files_name_their_kind_index_and_attribute() {
        assert_eq!(
            parse_channel_file("temp3_crit"),
            Some((ChannelKind::Temperature, 3, "crit"))
        );
        assert_eq!(
            parse_channel_file("in0_label"),
            Some((ChannelKind::Voltage, 0, "label"))
        );
        assert_eq!(
            parse_channel_file("power1_average"),
            Some((ChannelKind::Power, 1, "average"))
        );
        assert_eq!(
            parse_channel_file("curr2_input"),
            Some((ChannelKind::Current, 2, "input"))
        );
        assert_eq!(
            parse_channel_file("fan1_input"),
            Some((ChannelKind::Fan, 1, "input"))
        );
        assert_eq!(parse_channel_file("name"), None);
        assert_eq!(parse_channel_file("freq1_input"), None);
        assert_eq!(parse_channel_file("temp_input"), None);
        assert_eq!(parse_channel_file("tempx_input"), None);
    }

    #[test]
    fn units_are_converted_from_the_kernel_integers() {
        assert_eq!(convert(ChannelKind::Temperature, 45_750), 45.75);
        assert_eq!(convert(ChannelKind::Fan, 2777), 2777.0);
        assert_eq!(convert(ChannelKind::Voltage, 1308), 1.308);
        assert_eq!(convert(ChannelKind::Power, 48_000_000), 48.0);
        assert_eq!(convert(ChannelKind::Current, 1500), 1.5);
        assert_eq!(convert(ChannelKind::Temperature, -5_000), -5.0);
    }

    #[test]
    fn discovery_reads_inputs_with_labels_and_limits_in_kind_then_index_order() {
        let chip = discover(&listing(&[
            ("temp2_input", "60000\n"),
            ("temp2_label", "junction\n"),
            ("temp2_crit", "110000\n"),
            ("temp1_input", "45000\n"),
            ("temp1_label", "edge\n"),
            ("temp1_max", "100000\n"),
            ("fan1_input", "1200\n"),
            ("in0_input", "800\n"),
            ("in0_label", "vddgfx\n"),
            ("power1_average", "48000000\n"),
            ("power1_input", "999\n"),
            ("power1_label", "PPT\n"),
            ("power1_cap", "300000000\n"),
        ]));
        assert_eq!(chip.key, "hwmon3");
        assert_eq!(chip.name, "amdgpu");
        let kinds: Vec<(ChannelKind, u32)> =
            chip.channels.iter().map(|c| (c.kind, c.index)).collect();
        assert_eq!(
            kinds,
            vec![
                (ChannelKind::Temperature, 1),
                (ChannelKind::Temperature, 2),
                (ChannelKind::Fan, 1),
                (ChannelKind::Voltage, 0),
                (ChannelKind::Power, 1),
            ]
        );
        let edge = &chip.channels[0];
        assert_eq!(edge.label, "edge");
        assert_eq!(edge.value, 45.0);
        assert_eq!(edge.limit_max, Some(100.0));
        assert_eq!(edge.limit_crit, None);
        let junction = &chip.channels[1];
        assert_eq!(junction.limit_crit, Some(110.0));
        let power = &chip.channels[4];
        assert_eq!(power.value, 48.0, "average wins over input for power");
        assert_eq!(power.label, "PPT");
        assert_eq!(
            power.limit_max,
            Some(300.0),
            "the power cap is the power channel's max"
        );
        assert_eq!(chip.channels[2].label, "");
    }

    #[test]
    fn a_broken_value_skips_its_channel_and_nothing_else() {
        let chip = discover(&listing(&[
            ("temp1_input", "hot\n"),
            ("temp2_input", "50000\n"),
            ("temp2_max", "not a number\n"),
        ]));
        assert_eq!(chip.channels.len(), 1);
        assert_eq!(chip.channels[0].index, 2);
        assert_eq!(chip.channels[0].limit_max, None);
    }

    #[test]
    fn a_chip_with_no_channels_is_a_chip_with_no_channels() {
        let chip = discover(&listing(&[("name", "gigabyte_wmi\n")]));
        assert!(chip.channels.is_empty());
    }

    #[test]
    fn sentinel_limits_are_dropped_by_kind() {
        use ChannelKind::{Current, Fan, Power, Temperature, Voltage};
        assert_eq!(plausible_limit(Temperature, 65_261.85), None);
        assert_eq!(plausible_limit(Temperature, -273.15), None);
        assert_eq!(plausible_limit(Temperature, 200.0), Some(200.0));
        assert_eq!(plausible_limit(Temperature, -100.0), Some(-100.0));
        assert_eq!(plausible_limit(Fan, 3650.0), Some(3650.0));
        assert_eq!(plausible_limit(Fan, 100_001.0), None);
        assert_eq!(plausible_limit(Voltage, 1.2), Some(1.2));
        assert_eq!(plausible_limit(Voltage, 1_000.5), None);
        assert_eq!(plausible_limit(Power, 250.0), Some(250.0));
        assert_eq!(plausible_limit(Power, 100_000.5), None);
        assert_eq!(plausible_limit(Current, 1e9), Some(1e9));
    }

    #[test]
    fn discovery_drops_the_nvme_temp_max_sentinel_and_keeps_crit() {
        let listing = ChipListing {
            key: "hwmon1".to_owned(),
            name: "nvme".to_owned(),
            files: vec![
                ("temp1_input".to_owned(), "38850\n".to_owned()),
                ("temp1_max".to_owned(), "65261850\n".to_owned()),
                ("temp1_crit".to_owned(), "84850\n".to_owned()),
                ("temp1_min".to_owned(), "-273150\n".to_owned()),
            ],
        };
        let chip = discover(&listing);
        assert_eq!(chip.channels.len(), 1);
        assert_eq!(chip.channels[0].limit_max, None);
        assert_eq!(chip.channels[0].limit_crit, Some(84.85));
    }
}
