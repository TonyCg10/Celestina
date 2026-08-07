//! What is connected over Bluetooth, as `bluetoothctl` reports it.
//!
//! Read-only on purpose: the control centre is where turning the adapter on and
//! off belongs. What the panel answers is narrower — is there an adapter, is it
//! on, and is anything on it — and those are three questions, not one. Reading
//! them as one is what made a powered adapter with nothing paired to it
//! indistinguishable from a machine with no Bluetooth at all.

use serde_json::Value;

use crate::bounded;
use crate::inventory::{self, Answer};
use crate::snapshot::{Payload, MAX_ROW_ITEMS, MAX_TEXT_UNITS};

/// One line of a `bluetoothctl devices ...` listing: an address and whatever
/// name BlueZ has for it.
///
/// The address is the identity. A name is chosen by the other end of the radio
/// and two devices may well answer to the same one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub address: String,
    pub name: String,
}

impl Listed {
    /// What to show for it: its name, or its address when it has none — which
    /// is what `bluetoothctl` itself falls back to.
    #[must_use]
    pub fn display_name(&self) -> String {
        if self.name.is_empty() {
            self.address.clone()
        } else {
            self.name.clone()
        }
    }
}

/// The devices a `bluetoothctl devices ...` listing names.
///
/// Its lines are `Device <address> <name>`. Both fields come from outside this
/// process, so both are bounded here and the count is capped at the shared row
/// limit rather than at whatever the tool felt like printing.
#[must_use]
pub fn parse_listed(listing: &str) -> Vec<Listed> {
    let mut devices: Vec<Listed> = Vec::new();
    for line in listing.lines() {
        if devices.len() >= MAX_ROW_ITEMS {
            break;
        }
        let Some(rest) = line.trim().strip_prefix("Device ") else {
            continue;
        };
        let (address, name) = rest.split_once(' ').unwrap_or((rest, ""));
        let address = bounded(address.trim(), MAX_TEXT_UNITS);
        if address.is_empty() {
            continue;
        }
        // A repeated address is one device listed twice, not two devices.
        if devices.iter().any(|device| device.address == address) {
            continue;
        }
        devices.push(Listed {
            address,
            name: bounded(name.trim(), MAX_TEXT_UNITS),
        });
    }
    devices
}

/// The devices `bluetoothctl devices Connected` lists, by name.
#[must_use]
pub fn parse_connected(listing: &str) -> Vec<String> {
    parse_listed(listing)
        .iter()
        .map(Listed::display_name)
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

/// Everything one poll of the Bluetooth listings concluded.
///
/// One observation rather than two, because the summary keys and the inventory
/// describe the same devices and used to be built from two separate runs of
/// `bluetoothctl devices Connected`. Two runs can disagree — a device may
/// connect between them — and the panel would then have shown a count that
/// contradicted the list beside it, for no reason other than having asked
/// twice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub adapter: Adapter,
    /// The connected devices by display name, for the summary keys. Built from
    /// the same answer the inventory's `connected` flags come from.
    pub connected: Vec<String>,
    pub devices: inventory::Reading<KnownDevice>,
}

/// What the panel should show, given what the three listings answered.
///
/// [`Answer::Missing`] or [`Answer::Unreadable`] for `show` means nothing is
/// published at all, because the one thing this must never do is invent a
/// state for an adapter nobody could read: an unreadable adapter is not an
/// absent one.
///
/// An adapter that is on and a connected listing that did not answer is the
/// same: this has always withdrawn the widget rather than publish a summary,
/// and publishing an inventory of devices all claiming to be disconnected would
/// be worse. Only `Paired` may fail on its own, and it costs the inventory
/// while leaving the summary standing.
#[must_use]
pub fn observe(show: &Answer, paired: &Answer, connected: &Answer) -> Option<Observation> {
    let adapter = match parse_powered(show.text()?) {
        Some(true) => Adapter::On,
        Some(false) => Adapter::Off,
        // `bluetoothctl show` answered without a `Powered:` line, which is what
        // it does when there is no default controller to describe.
        None => Adapter::Absent,
    };

    if adapter != Adapter::On {
        // An adapter that is not powered has nothing on it. A conclusion, not
        // a gap — and one that costs no process to reach.
        return Some(Observation {
            adapter,
            connected: Vec::new(),
            devices: inventory::Reading::Listed(Vec::new()),
        });
    }

    // Read once, used twice. This is the single answer both the summary and
    // the inventory's connected flags are derived from.
    let online = parse_listed(connected.text()?);

    let devices = match paired {
        Answer::Missing => inventory::Reading::Unavailable,
        Answer::Unreadable => inventory::Reading::Unreadable,
        Answer::Text(listing) => inventory::Reading::Listed(merge(parse_listed(listing), &online)),
    };

    Some(Observation {
        connected: online.iter().map(Listed::display_name).collect(),
        adapter,
        devices,
    })
}

/// The known devices: everything paired, marked with whether it is connected,
/// plus anything connected that was never paired.
fn merge(paired: Vec<Listed>, online: &[Listed]) -> Vec<KnownDevice> {
    let mut devices: Vec<KnownDevice> = paired
        .into_iter()
        .map(|device| KnownDevice {
            connected: online.iter().any(|seen| seen.address == device.address),
            paired: true,
            // Named here rather than in the surface: what a nameless device is
            // called is a decision, and QML does not make those.
            name: device.display_name(),
            id: device.address,
        })
        .collect();

    // A device can be connected without being in the paired listing. It is
    // still something the person can be offered a disconnect for.
    for device in online {
        if devices.len() >= MAX_ROW_ITEMS {
            break;
        }
        if devices.iter().any(|known| known.id == device.address) {
            continue;
        }
        devices.push(KnownDevice {
            id: device.address.clone(),
            name: device.display_name(),
            connected: true,
            paired: false,
        });
    }
    devices
}

/// The `bluetooth` provider's payload, or `None` when there is nothing
/// publishable and the provider should be withdrawn.
///
/// Pure, so the shape can be tested without a radio. `adapter`, `count` and
/// `first` mean exactly what they have always meant and come from the same
/// observation the inventory does, so the two can no longer contradict.
#[must_use]
pub fn payload(
    observation: &Observation,
    devices: inventory::Published<'_, KnownDevice>,
) -> Payload {
    let mut payload = Payload::new();
    payload.insert(
        "adapter".to_owned(),
        Value::from(observation.adapter.as_token()),
    );
    payload.insert(
        "count".to_owned(),
        Value::from(u32::try_from(observation.connected.len()).unwrap_or(u32::MAX)),
    );
    if let Some(first) = observation.connected.first() {
        payload.insert("first".to_owned(), Value::from(first.clone()));
    }

    payload.insert("devicesState".to_owned(), Value::from(devices.as_token()));
    if let Some(rows) = devices.rows() {
        payload.insert(
            "devices".to_owned(),
            Value::Array(rows.iter().map(KnownDevice::to_row).collect()),
        );
    }
    payload
}

/// A device this session already knows, and the two states BlueZ confirms
/// about it.
///
/// Known means paired or connected. Nothing that would have to be discovered
/// first is listed, because discovery and pairing are not this unit's to start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownDevice {
    /// The Bluetooth address. Stable, and the only field here that is.
    pub id: String,
    pub name: String,
    pub connected: bool,
    pub paired: bool,
}

impl KnownDevice {
    /// One row of the published inventory.
    fn to_row(&self) -> Value {
        let mut row = Payload::new();
        row.insert("id".to_owned(), Value::from(self.id.clone()));
        row.insert("name".to_owned(), Value::from(self.name.clone()));
        row.insert("connected".to_owned(), Value::from(self.connected));
        row.insert("paired".to_owned(), Value::from(self.paired));
        Value::Object(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POWERED: &str = "Controller AA:BB:CC:DD:EE:FF cachyos [default]\n\tPowered: yes\n";
    const UNPOWERED: &str = "Controller AA:BB:CC:DD:EE:FF cachyos [default]\n\tPowered: no\n";
    const NO_CONTROLLER: &str = "No default controller available\n";
    const PAIRED: &str = "Device 5C:DC:49:0D:D1:62 S25 Ultra\n\
                          Device AA:BB:CC:DD:EE:01 WH-1000XM4\n";
    const CONNECTED: &str = "Device AA:BB:CC:DD:EE:01 WH-1000XM4\n";

    fn answer(text: &str) -> Answer {
        Answer::Text(text.to_owned())
    }

    fn observed(show: &str, paired: &str, connected: &str) -> Observation {
        observe(&answer(show), &answer(paired), &answer(connected)).expect("bluetoothctl answered")
    }

    fn listed(reading: &inventory::Reading<KnownDevice>) -> &[KnownDevice] {
        match reading {
            inventory::Reading::Listed(devices) => devices,
            other => panic!("expected a listing, got {other:?}"),
        }
    }

    #[test]
    fn a_powered_adapter_with_nothing_on_it_is_still_a_reading() {
        let observation = observed(POWERED, "", "");

        assert_eq!(observation.adapter, Adapter::On);
        assert!(observation.connected.is_empty());
        // Still a published widget, and a confirmed empty inventory.
        assert!(listed(&observation.devices).is_empty());
    }

    #[test]
    fn a_powered_adapter_keeps_its_count_and_its_first_device() {
        let observation = observed(
            POWERED,
            PAIRED,
            "Device 5C:DC:49:0D:D1:62 S25 Ultra\nDevice AA:BB:CC:DD:EE:01 WH-1000XM4\n",
        );

        assert_eq!(observation.adapter, Adapter::On);
        assert_eq!(observation.connected, ["S25 Ultra", "WH-1000XM4"]);
    }

    #[test]
    fn an_adapter_that_is_off_is_not_an_adapter_that_is_missing() {
        let off = observed(UNPOWERED, "", "");
        let missing = observed(NO_CONTROLLER, "", "");

        assert_eq!(off.adapter, Adapter::Off);
        assert_eq!(missing.adapter, Adapter::Absent);
        assert_ne!(off.adapter, missing.adapter);
    }

    /// An adapter nobody could read is not an adapter that is absent, and an
    /// adapter that is on whose devices nobody could read is not an adapter
    /// with nothing on it.
    #[test]
    fn an_unreadable_query_invents_no_state() {
        // The adapter query itself did not answer.
        assert_eq!(observe(&Answer::Unreadable, &answer(""), &answer("")), None);
        assert_eq!(observe(&Answer::Missing, &answer(""), &answer("")), None);
        // It answered `on`, and then the connected listing did not. Reporting
        // zero connections there would be a number nobody read.
        assert_eq!(
            observe(&answer(POWERED), &answer(PAIRED), &Answer::Unreadable),
            None
        );
        // An adapter that is off has no device listing to miss.
        assert_eq!(
            observe(&answer(UNPOWERED), &Answer::Unreadable, &Answer::Unreadable)
                .map(|observation| observation.adapter),
            Some(Adapter::Off)
        );
    }

    /// The defect this shape was rewritten for. `count`, `first` and every
    /// `connected` flag in the inventory now come from one answer, so the
    /// summary can no longer contradict the list beside it.
    #[test]
    fn the_summary_and_the_inventory_come_from_the_same_answer() {
        let observation = observed(POWERED, PAIRED, CONNECTED);

        assert_eq!(observation.connected, ["WH-1000XM4"]);
        let devices = listed(&observation.devices);
        let online: Vec<&str> = devices
            .iter()
            .filter(|device| device.connected)
            .map(|device| device.name.as_str())
            .collect();
        assert_eq!(online, observation.connected);
        assert_eq!(u32::try_from(observation.connected.len()), Ok(1));
    }

    #[test]
    fn a_known_device_carries_the_two_states_bluez_confirms() {
        let observation = observed(POWERED, PAIRED, CONNECTED);
        let devices = listed(&observation.devices);

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id, "5C:DC:49:0D:D1:62");
        assert_eq!(devices[0].name, "S25 Ultra");
        assert!(devices[0].paired);
        assert!(!devices[0].connected);
        assert!(devices[1].paired && devices[1].connected);
    }

    #[test]
    fn a_device_connected_without_being_paired_is_still_listed_once() {
        let observation = observed(POWERED, PAIRED, "Device AA:BB:CC:DD:EE:99 Car\n");
        let devices = listed(&observation.devices);

        assert_eq!(devices.len(), 3);
        let car = devices.iter().find(|device| device.name == "Car");
        assert_eq!(
            car.map(|device| (device.connected, device.paired)),
            Some((true, false))
        );
        // And the summary agrees with it, from the same answer.
        assert_eq!(observation.connected, ["Car"]);
    }

    /// A paired listing that failed costs the inventory and nothing else: the
    /// summary was read and stays true.
    #[test]
    fn a_failed_paired_listing_costs_the_inventory_not_the_summary() {
        let observation = observe(&answer(POWERED), &Answer::Unreadable, &answer(CONNECTED))
            .expect("bluetoothctl answered");

        assert_eq!(observation.connected, ["WH-1000XM4"]);
        assert_eq!(observation.devices, inventory::Reading::Unreadable);

        let missing = observe(&answer(POWERED), &Answer::Missing, &answer(CONNECTED))
            .expect("bluetoothctl answered");
        assert_eq!(missing.devices, inventory::Reading::Unavailable);
    }

    /// An adapter that is not powered concludes an empty list without asking
    /// anything, so the listings are not even consulted.
    #[test]
    fn an_adapter_that_is_not_on_lists_nothing_without_asking() {
        for show in [UNPOWERED, NO_CONTROLLER] {
            let observation = observe(&answer(show), &Answer::Missing, &Answer::Missing)
                .expect("bluetoothctl answered");

            assert_eq!(observation.devices, inventory::Reading::Listed(Vec::new()));
            assert!(observation.connected.is_empty());
        }
    }

    #[test]
    fn one_address_listed_twice_is_one_device() {
        let repeated = "Device AA:BB:CC:DD:EE:01 First\n\
                        Device AA:BB:CC:DD:EE:01 Second\n";

        let devices = parse_listed(repeated);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "First");
    }

    #[test]
    fn malformed_and_oversized_listings_are_bounded_before_they_leave() {
        // Lines BlueZ never printed, and one with no address.
        let hostile = "\nController AA:BB:CC:DD:EE:FF host [default]\nDevice \n";
        assert!(parse_listed(hostile).is_empty());

        let flood: String = (0..MAX_ROW_ITEMS * 3)
            .map(|index| format!("Device AA:BB:CC:DD:{index:02X}:{index:02X} Device {index}\n"))
            .collect();
        assert_eq!(parse_listed(&flood).len(), MAX_ROW_ITEMS);

        // A name from another device, in characters that cost two UTF-16 units
        // each.
        let dense = "😀".repeat(MAX_TEXT_UNITS * 2);
        let devices = parse_listed(&format!("Device AA:BB:CC:DD:EE:01 {dense}\n"));
        assert_eq!(devices.len(), 1);
        assert!(devices[0].name.encode_utf16().count() <= MAX_TEXT_UNITS);
    }

    #[test]
    fn a_nameless_known_device_is_named_by_the_provider_not_the_surface() {
        let observation = observed(POWERED, "Device AA:BB:CC:DD:EE:01\n", "");

        assert_eq!(listed(&observation.devices)[0].name, "AA:BB:CC:DD:EE:01");
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
            "Device 5C:DC:49:0D:D1:62 S25 Ultra\n\
             Device AA:BB:CC:DD:EE:FF WH-1000XM4\n",
        );

        assert_eq!(connected, ["S25 Ultra", "WH-1000XM4"]);
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

    /// The summary keys keep their meaning, and the inventory travels beside
    /// them under its own keys.
    #[test]
    fn the_payload_keeps_the_summary_keys_and_adds_the_inventory_beside_them() {
        let observation = observed(POWERED, PAIRED, CONNECTED);
        let mut held = inventory::Held::new();
        let published = held.observe(observation.devices.clone());

        let fields = payload(&observation, published);
        assert_eq!(fields["adapter"], Value::from("on"));
        assert_eq!(fields["count"], Value::from(1));
        assert_eq!(fields["first"], Value::from("WH-1000XM4"));
        assert_eq!(fields["devicesState"], Value::from("fresh"));
        assert_eq!(fields["devices"].as_array().map(Vec::len), Some(2));

        // A powered adapter with nothing on it publishes no `first` at all,
        // rather than an empty name.
        let bare = observed(POWERED, "", "");
        let mut held = inventory::Held::new();
        let published = held.observe(bare.devices.clone());
        let fields = payload(&bare, published);
        assert_eq!(fields["count"], Value::from(0));
        assert!(!fields.contains_key("first"));
        assert_eq!(fields["devices"], Value::Array(Vec::new()));
    }

    /// A list that has never been read is not an empty list, and the payload
    /// says so rather than shipping `[]`.
    #[test]
    fn a_payload_omits_the_inventory_until_there_is_one() {
        let observation = observed(POWERED, PAIRED, CONNECTED);
        let mut held = inventory::Held::new();

        let fields = payload(&observation, held.observe(inventory::Reading::Unreadable));
        assert_eq!(fields["devicesState"], Value::from("pending"));
        assert!(!fields.contains_key("devices"));
        // The summary is unaffected: it was read this poll.
        assert_eq!(fields["count"], Value::from(1));
    }
}
