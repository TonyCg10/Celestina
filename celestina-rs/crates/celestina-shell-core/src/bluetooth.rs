//! What is connected over Bluetooth, as `bluetoothctl` reports it.
//!
//! Read-only on purpose: R5's control centre is where turning the adapter on
//! and off belongs. Until then the panel answers one question — is anything
//! connected — and stays quiet the rest of the time.

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

#[cfg(test)]
mod tests {
    use super::*;

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
