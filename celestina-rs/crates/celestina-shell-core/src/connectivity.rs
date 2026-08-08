//! What the connectivity indicators may be asked to do, and how the helper
//! knows whether it happened.
//!
//! Everything here is deliberately narrow. The panel may join a network that is
//! already saved, power the Bluetooth adapter, and connect or disconnect a
//! device that is already known. It may not learn a password, create or edit a
//! profile, scan aggressively, discover, pair or trust — those need decisions
//! and secrets that belong to NetworkManager's and BlueZ's own agents.
//!
//! The other half of the narrowness is identity. A request names a row by the
//! stable `id` the inventory published — a NetworkManager UUID or a Bluetooth
//! address — and that id must still be in the last confirmed inventory. Nothing
//! is ever acted on by profile name, SSID, label, visible text or row position:
//! those are chosen by other people and other processes, they change under a
//! menu that is already open, and two of them can be identical.

use crate::bluetooth::{Adapter, KnownDevice};
use crate::network::KnownNetwork;
use crate::pending::Verdict;
use crate::snapshot::Payload;

/// Ask the provider to look again now.
pub const REFRESH: &str = "refresh";
/// Join a Wi-Fi network this session has already saved, by UUID.
pub const ACTIVATE_SAVED: &str = "activate-saved";
/// Turn the Bluetooth adapter on or off.
pub const SET_POWERED: &str = "set-powered";
/// Connect a device BlueZ already knows, by address.
pub const CONNECT_KNOWN: &str = "connect-known";
/// Disconnect a device BlueZ already knows, by address.
pub const DISCONNECT_KNOWN: &str = "disconnect-known";

/// The option carrying a row's stable identity. Named for the field the
/// inventory publishes it under, so a verb cannot be wired to the wrong one.
pub const ID_OPTION: &str = "id";
/// The option carrying the adapter's requested power.
pub const POWERED_OPTION: &str = "powered";

/// How long a request waits for the machine to show the state it asked for.
///
/// Four polls of the session provider's five-second interval. `nmcli
/// connection up` can return well before a link is usable and a device may take
/// several seconds to answer, so a shorter window would report failures that
/// were only slowness. Longer would leave a menu entry pending long after a
/// person has concluded it did not work.
pub const CONFIRMATION_WINDOW_MS: u64 = 20_000;

/// A request that has been validated and may be carried out.
///
/// Constructing one is the validation: there is no way to name a network or a
/// device here that was not in the inventory the panel was looking at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Look again now. Changes nothing on the machine.
    Refresh,
    ActivateSaved {
        uuid: String,
    },
    SetPowered(bool),
    ConnectKnown {
        address: String,
    },
    DisconnectKnown {
        address: String,
    },
}

/// The state a later observation must show for a request to have worked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expected {
    /// Any fresh observation of the provider. What `refresh` asked for is the
    /// observation itself.
    Observation,
    /// The saved profile with this UUID is attached to a device.
    NetworkActive {
        uuid: String,
    },
    AdapterPowered(bool),
    DeviceConnected {
        address: String,
        connected: bool,
    },
}

impl Action {
    /// What the machine must show before this request is called done.
    #[must_use]
    pub fn expects(&self) -> Expected {
        match self {
            Self::Refresh => Expected::Observation,
            Self::ActivateSaved { uuid } => Expected::NetworkActive { uuid: uuid.clone() },
            Self::SetPowered(powered) => Expected::AdapterPowered(*powered),
            Self::ConnectKnown { address } => Expected::DeviceConnected {
                address: address.clone(),
                connected: true,
            },
            Self::DisconnectKnown { address } => Expected::DeviceConnected {
                address: address.clone(),
                connected: false,
            },
        }
    }
}

/// Whether two expectations address the same mutable target.
///
/// Requested state is deliberately ignored. Turning the one adapter on and
/// off, or connecting and disconnecting the same device, are incompatible
/// requests rather than independent work that may remain pending together.
#[must_use]
pub fn same_target(left: &Expected, right: &Expected) -> bool {
    match (left, right) {
        (Expected::Observation, Expected::Observation)
        | (Expected::AdapterPowered(_), Expected::AdapterPowered(_)) => true,
        (Expected::NetworkActive { uuid: left }, Expected::NetworkActive { uuid: right }) => {
            left == right
        }
        (
            Expected::DeviceConnected { address: left, .. },
            Expected::DeviceConnected { address: right, .. },
        ) => left == right,
        _ => false,
    }
}

/// Reads the `id` option and checks it against the identities the inventory
/// really published.
///
/// The membership test is the security boundary: an id that is not in the list
/// never reaches a process. The leading-dash refusal is defence behind it —
/// nothing in either inventory can start with one, so a request that does is
/// either a bug or an attempt to smuggle an option into an argument list, and
/// it is refused before any program is chosen.
fn read_identity<'a, I>(options: &Payload, known: I) -> Result<String, String>
where
    I: IntoIterator<Item = &'a str>,
{
    let Some(wanted) = options.get(ID_OPTION).and_then(|id| id.as_str()) else {
        return Err(format!("the request carries no '{ID_OPTION}' to act on"));
    };
    if wanted.is_empty() {
        return Err("the request names an empty identity".to_owned());
    }
    if wanted.starts_with('-') {
        return Err("the request names an identity that looks like an option".to_owned());
    }

    if known.into_iter().any(|id| id == wanted) {
        return Ok(wanted.to_owned());
    }
    // The id is not echoed back. It came from outside and the host learns that
    // its menu is stale, which is the actionable part.
    Err("that entry is not in the last confirmed inventory".to_owned())
}

/// The network verbs, validated against the last confirmed inventory.
///
/// # Errors
///
/// Refuses an unknown verb, a missing or unusable identity, and any UUID that
/// is not in `known` — before anything is executed.
pub fn read_network_action(
    verb: &str,
    options: &Payload,
    known: &[KnownNetwork],
) -> Result<Action, String> {
    match verb {
        REFRESH => Ok(Action::Refresh),
        ACTIVATE_SAVED => {
            let uuid = read_identity(options, known.iter().map(|row| row.id.as_str()))?;
            Ok(Action::ActivateSaved { uuid })
        }
        other => Err(format!("'network' does not serve the verb '{other}'")),
    }
}

/// The Bluetooth verbs, validated against the last confirmed inventory.
///
/// # Errors
///
/// Refuses an unknown verb, a `powered` option that is not a real boolean, a
/// missing or unusable identity, and any address that is not in `known`.
pub fn read_bluetooth_action(
    verb: &str,
    options: &Payload,
    known: &[KnownDevice],
) -> Result<Action, String> {
    let identity = || read_identity(options, known.iter().map(|row| row.id.as_str()));

    match verb {
        REFRESH => Ok(Action::Refresh),
        SET_POWERED => {
            // A typed boolean only. `"true"`, `1` and `"on"` are a host that
            // guessed at this protocol, and guessing about a radio switch is
            // not something to be lenient with.
            let Some(powered) = options
                .get(POWERED_OPTION)
                .and_then(|value| value.as_bool())
            else {
                return Err(format!("the request carries no boolean '{POWERED_OPTION}'"));
            };
            Ok(Action::SetPowered(powered))
        }
        CONNECT_KNOWN => Ok(Action::ConnectKnown {
            address: identity()?,
        }),
        DISCONNECT_KNOWN => Ok(Action::DisconnectKnown {
            address: identity()?,
        }),
        other => Err(format!("'bluetooth' does not serve the verb '{other}'")),
    }
}

/// What a fresh network observation says about a waiting request.
///
/// Confirmation is by UUID, never by the link summary's visible name: two
/// profiles may carry the same name and a name may be edited between the
/// request and its answer.
#[must_use]
pub fn judge_network(expected: &Expected, networks: &[KnownNetwork]) -> Verdict {
    match expected {
        Expected::Observation => Verdict::Confirmed,
        Expected::NetworkActive { uuid } => {
            match networks.iter().find(|row| row.id == *uuid) {
                Some(row) if row.active => Verdict::Confirmed,
                // Still saved and not attached: NetworkManager may still be
                // associating.
                Some(_) => Verdict::Waiting,
                // A conclusive inventory without it means the profile is gone.
                // An unreadable poll does not reach here: the inventory holds
                // its last conclusive rows instead of emptying.
                None => Verdict::Contradicted,
            }
        }
        Expected::AdapterPowered(_) | Expected::DeviceConnected { .. } => Verdict::Waiting,
    }
}

/// What a fresh Bluetooth observation says about a waiting request.
#[must_use]
pub fn judge_bluetooth(expected: &Expected, adapter: Adapter, devices: &[KnownDevice]) -> Verdict {
    match expected {
        Expected::Observation => Verdict::Confirmed,
        Expected::AdapterPowered(wanted) => match (adapter, wanted) {
            (Adapter::On, true) | (Adapter::Off, false) => Verdict::Confirmed,
            // There is no adapter to power either way.
            (Adapter::Absent, _) => Verdict::Contradicted,
            _ => Verdict::Waiting,
        },
        Expected::DeviceConnected { address, connected } => {
            match devices.iter().find(|row| row.id == *address) {
                Some(row) if row.connected == *connected => Verdict::Confirmed,
                Some(_) => Verdict::Waiting,
                // Powering the adapter down empties this list, and a device
                // that is merely out of the list has not been observed to
                // disconnect. Inventing that confirmation is exactly the lie
                // this whole path exists to avoid, so it keeps waiting and its
                // deadline decides.
                None => Verdict::Waiting,
            }
        }
        Expected::NetworkActive { .. } => Verdict::Waiting,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::Value;

    fn options(pairs: &[(&str, Value)]) -> Payload {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect()
    }

    fn network(id: &str, active: bool) -> KnownNetwork {
        KnownNetwork {
            id: id.to_owned(),
            name: "Home".to_owned(),
            ssid: None,
            active,
            availability: crate::network::Availability::Unknown,
        }
    }

    fn device(id: &str, connected: bool) -> KnownDevice {
        KnownDevice {
            id: id.to_owned(),
            name: "WH-1000XM4".to_owned(),
            connected,
            paired: true,
        }
    }

    const UUID: &str = "9f1c-1";
    const ADDRESS: &str = "AA:BB:CC:DD:EE:01";

    #[test]
    fn every_verb_reads_into_its_own_typed_action() {
        let networks = [network(UUID, false)];
        let devices = [device(ADDRESS, false)];

        assert_eq!(
            read_network_action(REFRESH, &Payload::new(), &networks),
            Ok(Action::Refresh)
        );
        assert_eq!(
            read_network_action(
                ACTIVATE_SAVED,
                &options(&[(ID_OPTION, Value::from(UUID))]),
                &networks
            ),
            Ok(Action::ActivateSaved {
                uuid: UUID.to_owned()
            })
        );
        assert_eq!(
            read_bluetooth_action(
                SET_POWERED,
                &options(&[(POWERED_OPTION, Value::from(true))]),
                &devices
            ),
            Ok(Action::SetPowered(true))
        );
        assert_eq!(
            read_bluetooth_action(
                CONNECT_KNOWN,
                &options(&[(ID_OPTION, Value::from(ADDRESS))]),
                &devices
            ),
            Ok(Action::ConnectKnown {
                address: ADDRESS.to_owned()
            })
        );
        assert_eq!(
            read_bluetooth_action(
                DISCONNECT_KNOWN,
                &options(&[(ID_OPTION, Value::from(ADDRESS))]),
                &devices
            ),
            Ok(Action::DisconnectKnown {
                address: ADDRESS.to_owned()
            })
        );
    }

    #[test]
    fn a_verb_neither_provider_serves_is_refused_by_name() {
        assert!(read_network_action("set-powered", &Payload::new(), &[])
            .expect_err("refused")
            .contains("set-powered"));
        assert!(
            read_bluetooth_action("activate-saved", &Payload::new(), &[])
                .expect_err("refused")
                .contains("activate-saved")
        );
    }

    /// The security boundary. An identity that is not in the last confirmed
    /// inventory never reaches a process.
    #[test]
    fn an_identity_the_inventory_never_published_is_refused() {
        let networks = [network(UUID, false)];

        let refusal = read_network_action(
            ACTIVATE_SAVED,
            &options(&[(ID_OPTION, Value::from("9f1c-9"))]),
            &networks,
        )
        .expect_err("refused");
        assert!(refusal.contains("last confirmed inventory"));
        // The rejected id is not echoed back into a frame.
        assert!(!refusal.contains("9f1c-9"));

        // And an empty inventory can be asked for nothing at all.
        assert!(read_network_action(
            ACTIVATE_SAVED,
            &options(&[(ID_OPTION, Value::from(UUID))]),
            &[]
        )
        .is_err());
    }

    #[test]
    fn an_identity_that_is_empty_missing_or_option_shaped_is_refused() {
        let devices = [device(ADDRESS, false)];

        for id in ["", "-h", "--help", "-"] {
            assert!(read_bluetooth_action(
                CONNECT_KNOWN,
                &options(&[(ID_OPTION, Value::from(id))]),
                &devices
            )
            .is_err());
        }

        // Missing entirely, and present as the wrong type.
        assert!(read_bluetooth_action(CONNECT_KNOWN, &Payload::new(), &devices).is_err());
        assert!(read_bluetooth_action(
            CONNECT_KNOWN,
            &options(&[(ID_OPTION, Value::from(7))]),
            &devices
        )
        .is_err());
    }

    /// An option-shaped identity is refused even when a hostile inventory
    /// somehow carries one, because the refusal happens before the lookup.
    #[test]
    fn an_option_shaped_identity_is_refused_even_if_the_inventory_holds_it() {
        let devices = [device("--help", false)];

        assert!(read_bluetooth_action(
            CONNECT_KNOWN,
            &options(&[(ID_OPTION, Value::from("--help"))]),
            &devices
        )
        .is_err());
    }

    #[test]
    fn a_very_long_identity_is_refused_rather_than_truncated_into_another_row() {
        let devices = [device(ADDRESS, false)];
        let long = "A".repeat(4_096);

        // Truncating this to something that matched a real row would act on a
        // device nobody asked about.
        assert!(read_bluetooth_action(
            CONNECT_KNOWN,
            &options(&[(ID_OPTION, Value::from(long))]),
            &devices
        )
        .is_err());
    }

    #[test]
    fn the_adapter_switch_needs_a_real_boolean() {
        for value in [Value::from("true"), Value::from(1), Value::from("on")] {
            assert!(
                read_bluetooth_action(SET_POWERED, &options(&[(POWERED_OPTION, value)]), &[])
                    .is_err()
            );
        }
        assert_eq!(
            read_bluetooth_action(
                SET_POWERED,
                &options(&[(POWERED_OPTION, Value::from(false))]),
                &[]
            ),
            Ok(Action::SetPowered(false))
        );
    }

    #[test]
    fn each_action_knows_what_the_machine_must_show() {
        assert_eq!(Action::Refresh.expects(), Expected::Observation);
        assert_eq!(
            Action::ActivateSaved {
                uuid: UUID.to_owned()
            }
            .expects(),
            Expected::NetworkActive {
                uuid: UUID.to_owned()
            }
        );
        assert_eq!(
            Action::SetPowered(true).expects(),
            Expected::AdapterPowered(true)
        );
        assert_eq!(
            Action::DisconnectKnown {
                address: ADDRESS.to_owned()
            }
            .expects(),
            Expected::DeviceConnected {
                address: ADDRESS.to_owned(),
                connected: false
            }
        );
    }

    #[test]
    fn opposite_states_for_one_adapter_or_device_have_one_target() {
        assert!(same_target(
            &Expected::AdapterPowered(true),
            &Expected::AdapterPowered(false)
        ));
        assert!(same_target(
            &Expected::DeviceConnected {
                address: ADDRESS.to_owned(),
                connected: true,
            },
            &Expected::DeviceConnected {
                address: ADDRESS.to_owned(),
                connected: false,
            }
        ));
        assert!(!same_target(
            &Expected::DeviceConnected {
                address: ADDRESS.to_owned(),
                connected: true,
            },
            &Expected::DeviceConnected {
                address: "AA:BB:CC:DD:EE:02".to_owned(),
                connected: false,
            }
        ));
    }

    /// Confirmation is by UUID. A successful `nmcli` exit is not in this
    /// picture at all — only what a later inventory shows.
    #[test]
    fn a_network_request_is_confirmed_by_its_uuid_becoming_active() {
        let expected = Action::ActivateSaved {
            uuid: UUID.to_owned(),
        }
        .expects();

        // Saved, not attached yet: still trying.
        assert_eq!(
            judge_network(&expected, &[network(UUID, false)]),
            Verdict::Waiting
        );
        // Attached: done.
        assert_eq!(
            judge_network(&expected, &[network(UUID, true)]),
            Verdict::Confirmed
        );
        // Another profile went active instead. That is not this request.
        assert_eq!(
            judge_network(&expected, &[network(UUID, false), network("9f1c-2", true)]),
            Verdict::Waiting
        );
        // The profile is gone from a conclusive inventory.
        assert_eq!(
            judge_network(&expected, &[network("9f1c-2", true)]),
            Verdict::Contradicted
        );
    }

    #[test]
    fn the_adapter_switch_is_confirmed_only_by_the_state_it_asked_for() {
        let on = Expected::AdapterPowered(true);
        let off = Expected::AdapterPowered(false);

        assert_eq!(judge_bluetooth(&on, Adapter::On, &[]), Verdict::Confirmed);
        assert_eq!(judge_bluetooth(&on, Adapter::Off, &[]), Verdict::Waiting);
        assert_eq!(judge_bluetooth(&off, Adapter::Off, &[]), Verdict::Confirmed);
        assert_eq!(judge_bluetooth(&off, Adapter::On, &[]), Verdict::Waiting);
        // No adapter: neither request can ever succeed.
        assert_eq!(
            judge_bluetooth(&on, Adapter::Absent, &[]),
            Verdict::Contradicted
        );
        assert_eq!(
            judge_bluetooth(&off, Adapter::Absent, &[]),
            Verdict::Contradicted
        );
    }

    #[test]
    fn a_device_request_is_confirmed_by_that_device_and_no_other() {
        let connect = Action::ConnectKnown {
            address: ADDRESS.to_owned(),
        }
        .expects();

        assert_eq!(
            judge_bluetooth(&connect, Adapter::On, &[device(ADDRESS, true)]),
            Verdict::Confirmed
        );
        assert_eq!(
            judge_bluetooth(&connect, Adapter::On, &[device(ADDRESS, false)]),
            Verdict::Waiting
        );
        // A different device connected. Not this request.
        assert_eq!(
            judge_bluetooth(
                &connect,
                Adapter::On,
                &[device(ADDRESS, false), device("AA:BB:CC:DD:EE:02", true)]
            ),
            Verdict::Waiting
        );
    }

    /// Powering the adapter down empties the device list. A disconnect must
    /// not read that as its own confirmation, and neither may a connect read
    /// it as a contradiction — the deadline decides instead.
    #[test]
    fn an_empty_device_list_confirms_no_individual_disconnection() {
        let disconnect = Action::DisconnectKnown {
            address: ADDRESS.to_owned(),
        }
        .expects();

        assert_eq!(
            judge_bluetooth(&disconnect, Adapter::Off, &[]),
            Verdict::Waiting
        );
        assert_eq!(
            judge_bluetooth(&disconnect, Adapter::On, &[]),
            Verdict::Waiting
        );
        // It is confirmed only by seeing the device itself, disconnected.
        assert_eq!(
            judge_bluetooth(&disconnect, Adapter::On, &[device(ADDRESS, false)]),
            Verdict::Confirmed
        );
    }

    /// One provider's observation never settles the other's request, even
    /// though both share one ledger.
    #[test]
    fn neither_provider_settles_the_others_expectations() {
        let network_request = Expected::NetworkActive {
            uuid: UUID.to_owned(),
        };
        let bluetooth_request = Expected::AdapterPowered(true);

        assert_eq!(
            judge_bluetooth(&network_request, Adapter::On, &[]),
            Verdict::Waiting
        );
        assert_eq!(
            judge_network(&bluetooth_request, &[network(UUID, true)]),
            Verdict::Waiting
        );
    }
}
