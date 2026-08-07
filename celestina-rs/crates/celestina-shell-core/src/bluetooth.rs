//! What is connected over Bluetooth, as `bluetoothctl` reports it.
//!
//! Read-only on purpose: the control centre is where turning the adapter on and
//! off belongs. What the panel answers is narrower — is there an adapter, is it
//! on, and is anything on it — and those are three questions, not one. Reading
//! them as one is what made a powered adapter with nothing paired to it
//! indistinguishable from a machine with no Bluetooth at all.

/// The devices `bluetoothctl devices Connected` lists, by name.
///
/// Its lines are `Device <address> <name>`; a device with no name shows its
/// address, which is what `bluetoothctl` itself does.
#[must_use]
pub fn parse_connected(listing: &str) -> Vec<String> {
    listing
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("Device ")?;
            let (address, name) = rest.split_once(' ').unwrap_or((rest, ""));
            if address.is_empty() {
                return None;
            }
            Some(if name.trim().is_empty() {
                address.to_owned()
            } else {
                name.trim().to_owned()
            })
        })
        .collect()
}

/// Whether the default adapter is powered, from `bluetoothctl show`.
#[must_use]
pub fn parse_powered(details: &str) -> Option<bool> {
    details
        .lines()
        .find_map(|line| line.trim().strip_prefix("Powered:"))
        .map(|value| value.trim().eq_ignore_ascii_case("yes"))
}

/// What the default adapter is, as far as the panel is concerned.
///
/// Three states rather than a `bool`, because "there is no adapter" and "the
/// adapter is off" are different things to a person: one is a machine without
/// Bluetooth and the other is a switch they can flick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Adapter {
    /// `bluetoothctl` answered and named no default controller.
    Absent,
    Off,
    On,
}

impl Adapter {
    /// The word the panel publishes. It is the protocol token, not copy: what a
    /// person reads is the surface's business.
    #[must_use]
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Off => "off",
            Self::On => "on",
        }
    }
}

/// What the panel should show, given what the two commands answered.
///
/// `None` for either argument means that command did not answer at all — it
/// timed out, or was not there. A reading is then not published, because the
/// one thing this must never do is invent a state for an adapter nobody could
/// read: an unreadable adapter is not an absent one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    pub adapter: Adapter,
    pub connected: Vec<String>,
}

#[must_use]
pub fn reading(show: Option<&str>, devices: Option<&str>) -> Option<Reading> {
    let show = show?;
    let adapter = match parse_powered(show) {
        Some(true) => Adapter::On,
        Some(false) => Adapter::Off,
        // `bluetoothctl show` answered without a `Powered:` line, which is what
        // it does when there is no default controller to describe.
        None => Adapter::Absent,
    };

    // A powered adapter is the only one that can have anything on it, and the
    // device list is only trusted when it was actually read.
    let connected = match adapter {
        Adapter::On => parse_connected(devices?),
        _ => Vec::new(),
    };

    Some(Reading { adapter, connected })
}

#[cfg(test)]
mod tests {
    use super::*;

    const POWERED: &str = "Controller AA:BB:CC:DD:EE:FF cachyos [default]\n\tPowered: yes\n";
    const UNPOWERED: &str = "Controller AA:BB:CC:DD:EE:FF cachyos [default]\n\tPowered: no\n";
    const NO_CONTROLLER: &str = "No default controller available\n";

    #[test]
    fn a_powered_adapter_with_nothing_on_it_is_still_a_reading() {
        let reading = reading(Some(POWERED), Some("")).expect("an adapter answered");

        assert_eq!(reading.adapter, Adapter::On);
        assert!(reading.connected.is_empty());
    }

    #[test]
    fn a_powered_adapter_keeps_its_count_and_its_first_device() {
        let reading = reading(
            Some(POWERED),
            Some("Device 5C:DC:49:0D:D1:62 S25 Ultra de Antonio\nDevice AA:BB:CC:DD:EE:FF WH-1000XM4\n"),
        )
        .expect("an adapter answered");

        assert_eq!(reading.adapter, Adapter::On);
        assert_eq!(reading.connected, ["S25 Ultra de Antonio", "WH-1000XM4"]);
    }

    #[test]
    fn an_adapter_that_is_off_is_not_an_adapter_that_is_missing() {
        let off = reading(Some(UNPOWERED), Some("")).expect("an adapter answered");
        let missing = reading(Some(NO_CONTROLLER), Some("")).expect("bluetoothctl answered");

        assert_eq!(off.adapter, Adapter::Off);
        assert_eq!(missing.adapter, Adapter::Absent);
        assert_ne!(off.adapter, missing.adapter);
    }

    #[test]
    fn an_unreadable_query_invents_no_state() {
        // The adapter query itself did not answer.
        assert_eq!(reading(None, Some("")), None);
        // It answered `on`, and then the device list did not. Reporting zero
        // connections there would be a number nobody read.
        assert_eq!(reading(Some(POWERED), None), None);
        // An adapter that is off has no device list to miss.
        assert_eq!(
            reading(Some(UNPOWERED), None).map(|reading| reading.adapter),
            Some(Adapter::Off)
        );
    }

    #[test]
    fn each_state_publishes_its_own_token() {
        assert_eq!(Adapter::Absent.as_token(), "absent");
        assert_eq!(Adapter::Off.as_token(), "off");
        assert_eq!(Adapter::On.as_token(), "on");
    }

    #[test]
    fn connected_devices_are_named_the_way_bluetoothctl_names_them() {
        let connected = parse_connected(
            "Device 5C:DC:49:0D:D1:62 S25 Ultra de Antonio\n\
             Device AA:BB:CC:DD:EE:FF WH-1000XM4\n",
        );

        assert_eq!(connected, ["S25 Ultra de Antonio", "WH-1000XM4"]);
    }

    #[test]
    fn a_nameless_device_is_shown_as_its_address() {
        assert_eq!(
            parse_connected("Device AA:BB:CC:DD:EE:FF\n"),
            ["AA:BB:CC:DD:EE:FF"]
        );
    }

    #[test]
    fn nothing_connected_is_an_empty_list_not_a_guess() {
        assert!(parse_connected("").is_empty());
        assert!(parse_connected("No default controller available\n").is_empty());
    }

    #[test]
    fn a_powered_adapter_says_so_and_a_missing_one_says_nothing() {
        assert_eq!(
            parse_powered("\tName: cachyos\n\tPowered: yes\n"),
            Some(true)
        );
        assert_eq!(parse_powered("\tPowered: no\n"), Some(false));
        assert_eq!(parse_powered("No default controller available\n"), None);
    }
}
