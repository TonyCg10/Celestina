//! What the pieces and pages read of connectivity: the latest section of each
//! service, and per row what is pending or has just failed (CEL-2-F, D5).
//!
//! Pure and synchronous. The event loop owns one [`ConnectivityState`] and
//! calls [`ConnectivityState::apply`] (and [`ConnectivityState::apply_power`])
//! for every event a worker publishes; the Island lends it read-only to the
//! bar's pieces and the pages ([`crate::pieces::BarCx::connectivity`],
//! [`crate::island::pages::PageCx::connectivity`]).
//!
//! A section starts `Unavailable` and stays so until the worker's first news
//! (D10): a piece draws nothing for it and its page says why. A request is
//! shown *pending* from the moment it is sent ([`ConnectivityState::begin`])
//! until the worker reports it [`Resolution::Confirmed`] or failed; nothing
//! about a row's own state is ever guessed.

use celestina_shell::connectivity::{
    validate, Action, BatterySection, BtAddress, BtSection, Editor, Expected, Failure, Inventory,
    NetSection, ProfileName, ProfileSection, Refusal, Resolution, RowId, Service, MAX_PENDING,
};

use super::{power::PowerEvent, ConnectivityEvent};

/// What a row shows about the request that targets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowStatus {
    /// Nothing is pending and nothing failed.
    Idle,
    /// A request was sent and no observation has settled it yet.
    Pending,
    /// The last request failed, by kind.
    Failed(Failure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Pending,
    Failed(Failure),
}

impl Outcome {
    fn status(self) -> RowStatus {
        match self {
            Self::Pending => RowStatus::Pending,
            Self::Failed(failure) => RowStatus::Failed(failure),
        }
    }
}

/// The latest state of the four services and of the requests in flight.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConnectivityState {
    network: Option<Service<NetSection>>,
    bluetooth: Option<Service<BtSection>>,
    battery: Option<Service<BatterySection>>,
    profiles: Option<Service<ProfileSection>>,
    /// At most [`MAX_PENDING`] entries, one per subject, oldest first.
    entries: Vec<(Expected, Outcome)>,
    /// Editors a launch found missing from `PATH`.
    editors_missing: Vec<Editor>,
}

/// Whether two expectations are about the same thing (a new request for a
/// subject replaces the older one; there is one profile subject).
fn same_subject(a: &Expected, b: &Expected) -> bool {
    match (a, b) {
        (Expected::WifiEnabled(_), Expected::WifiEnabled(_))
        | (Expected::BtPowered(_), Expected::BtPowered(_))
        | (Expected::Profile(_), Expected::Profile(_)) => true,
        (Expected::NetActive { id: a, .. }, Expected::NetActive { id: b, .. }) => a == b,
        (Expected::BtConnected { address: a, .. }, Expected::BtConnected { address: b, .. }) => {
            a == b
        }
        _ => false,
    }
}

/// What `action` waits to observe, for the actions that change something.
fn expected_of(action: &Action) -> Option<Expected> {
    match action {
        Action::SetWifiEnabled(on) => Some(Expected::WifiEnabled(*on)),
        Action::JoinSaved(id) => Some(Expected::NetActive {
            id: id.clone(),
            active: true,
        }),
        Action::Leave(id) => Some(Expected::NetActive {
            id: id.clone(),
            active: false,
        }),
        Action::SetBluetoothPowered(on) => Some(Expected::BtPowered(*on)),
        Action::ConnectDevice(address) => Some(Expected::BtConnected {
            address: address.clone(),
            connected: true,
        }),
        Action::DisconnectDevice(address) => Some(Expected::BtConnected {
            address: address.clone(),
            connected: false,
        }),
        Action::SetProfile(name) => Some(Expected::Profile(name.clone())),
        Action::Refresh(_) | Action::CycleProfile | Action::OpenEditor(_) => None,
    }
}

impl ConnectivityState {
    /// NetworkManager's state: `Unavailable` until its first news.
    #[must_use]
    pub fn network(&self) -> Service<&NetSection> {
        service(self.network.as_ref())
    }

    /// BlueZ's state.
    #[must_use]
    pub fn bluetooth(&self) -> Service<&BtSection> {
        service(self.bluetooth.as_ref())
    }

    /// UPower's display device.
    #[must_use]
    pub fn battery(&self) -> Service<&BatterySection> {
        service(self.battery.as_ref())
    }

    /// The power profile daemon.
    #[must_use]
    pub fn profiles(&self) -> Service<&ProfileSection> {
        service(self.profiles.as_ref())
    }

    /// The last confirmed state, as [`validate`] reads it.
    #[must_use]
    pub fn inventory(&self) -> Inventory<'_> {
        Inventory {
            network: self.network().ready().copied(),
            bluetooth: self.bluetooth().ready().copied(),
            profiles: self.profiles().ready().copied(),
        }
    }

    /// Takes one worker event; whether anything a view reads changed.
    pub fn apply(&mut self, event: &ConnectivityEvent) -> bool {
        match event {
            ConnectivityEvent::Network(section) => replace(&mut self.network, section.clone()),
            ConnectivityEvent::Bluetooth(section) => replace(&mut self.bluetooth, section.clone()),
            ConnectivityEvent::Pending(expected) => self.set(expected.clone(), Outcome::Pending),
            ConnectivityEvent::Resolved(resolution) => self.resolve(resolution),
            ConnectivityEvent::Refused(action, refusal) => {
                self.set_failure(action, Failure::Refused(*refusal))
            }
            ConnectivityEvent::Power(event) => self.apply_power(event),
        }
    }

    /// Takes one power worker event (the power worker publishes its own
    /// type, which reaches here as [`ConnectivityEvent::Power`]). A power
    /// refusal names no action; the only power request a row waits for is a
    /// profile change ([`ConnectivityState::begin`] marked it), so a refusal
    /// fails the pending profile request, if there is one.
    pub fn apply_power(&mut self, event: &PowerEvent) -> bool {
        match event {
            PowerEvent::Battery(section) => replace(&mut self.battery, section.clone()),
            PowerEvent::Profiles(section) => replace(&mut self.profiles, section.clone()),
            PowerEvent::Resolved(resolution) => self.resolve(resolution),
            PowerEvent::Refused(refusal) => {
                let pending = self.entries.iter().find_map(|(expected, outcome)| {
                    (matches!(expected, Expected::Profile(_)) && *outcome == Outcome::Pending)
                        .then(|| expected.clone())
                });
                pending.is_some_and(|expected| {
                    self.set(expected, Outcome::Failed(Failure::Refused(*refusal)))
                })
            }
        }
    }

    /// A request is about to be sent: checks it against the last confirmed
    /// state and shows its row pending, or, when the page's own state
    /// already refuses it, shows the refusal on the row. The caller sends
    /// the request to its worker only on `Ok`; the worker's own check
    /// repeats this one and stays the authority.
    ///
    /// # Errors
    ///
    /// The [`Refusal`] [`validate`] gave.
    pub fn begin(&mut self, action: &Action) -> Result<(), Refusal> {
        match validate(action, &self.inventory()) {
            Ok(checked) => {
                if let Some(expected) = checked.expected() {
                    self.set(expected, Outcome::Pending);
                }
                Ok(())
            }
            Err(refusal) => {
                self.set_failure(action, Failure::Refused(refusal));
                Err(refusal)
            }
        }
    }

    /// A request could not be delivered (a full or stopped queue): its row
    /// shows `failure` instead of waiting for news that will not come. A
    /// cycle is resolved to the profile [`ConnectivityState::begin`] marked.
    pub fn fail(&mut self, action: &Action, failure: Failure) -> bool {
        match action {
            Action::CycleProfile => validate(action, &self.inventory())
                .ok()
                .and_then(|checked| checked.expected())
                .is_some_and(|expected| self.set(expected, Outcome::Failed(failure))),
            _ => self.set_failure(action, failure),
        }
    }

    /// The Wi-Fi switch's request status.
    #[must_use]
    pub fn wifi_status(&self) -> RowStatus {
        self.status(|e| matches!(e, Expected::WifiEnabled(_)))
    }

    /// The network row `id`'s request status.
    #[must_use]
    pub fn net_status(&self, id: &RowId) -> RowStatus {
        self.status(|e| matches!(e, Expected::NetActive { id: held, .. } if held == id))
    }

    /// The Bluetooth adapter switch's request status.
    #[must_use]
    pub fn powered_status(&self) -> RowStatus {
        self.status(|e| matches!(e, Expected::BtPowered(_)))
    }

    /// The device `address`'s request status.
    #[must_use]
    pub fn device_status(&self, address: &BtAddress) -> RowStatus {
        self.status(|e| matches!(e, Expected::BtConnected { address: held, .. } if held == address))
    }

    /// The status of the request for profile `name` (only the profile last
    /// asked for has one).
    #[must_use]
    pub fn profile_status(&self, name: &ProfileName) -> RowStatus {
        self.status(|e| matches!(e, Expected::Profile(held) if held == name))
    }

    /// Whether a launch of `editor` found its program missing.
    #[must_use]
    pub fn editor_missing(&self, editor: Editor) -> bool {
        self.editors_missing.contains(&editor)
    }

    /// Records that `editor`'s program is not on `PATH` (or that it is, with
    /// `missing` false); whether that changed anything.
    pub fn set_editor_missing(&mut self, editor: Editor, missing: bool) -> bool {
        let held = self.editor_missing(editor);
        match (held, missing) {
            (false, true) => self.editors_missing.push(editor),
            (true, false) => self.editors_missing.retain(|e| *e != editor),
            _ => return false,
        }
        true
    }

    fn status(&self, wanted: impl Fn(&Expected) -> bool) -> RowStatus {
        self.entries
            .iter()
            .find(|(expected, _)| wanted(expected))
            .map_or(RowStatus::Idle, |(_, outcome)| outcome.status())
    }

    fn set(&mut self, expected: Expected, outcome: Outcome) -> bool {
        let before = self.entries.clone();
        self.entries
            .retain(|(held, _)| !same_subject(held, &expected));
        if self.entries.len() == MAX_PENDING {
            self.entries.remove(0);
        }
        self.entries.push((expected, outcome));
        self.entries != before
    }

    fn set_failure(&mut self, action: &Action, failure: Failure) -> bool {
        expected_of(action).is_some_and(|expected| self.set(expected, Outcome::Failed(failure)))
    }

    fn resolve(&mut self, resolution: &Resolution) -> bool {
        match resolution {
            Resolution::Confirmed(expected) => {
                let before = self.entries.len();
                self.entries
                    .retain(|(held, _)| !same_subject(held, expected));
                self.entries.len() != before
            }
            Resolution::Failed(expected, failure) => {
                self.set(expected.clone(), Outcome::Failed(*failure))
            }
        }
    }
}

fn service<T>(held: Option<&Service<T>>) -> Service<&T> {
    match held {
        Some(Service::Ready(section)) => Service::Ready(section),
        Some(Service::Unavailable) | None => Service::Unavailable,
    }
}

fn replace<T: PartialEq>(slot: &mut Option<Service<T>>, new: Service<T>) -> bool {
    if slot.as_ref() == Some(&new) {
        return false;
    }
    *slot = Some(new);
    true
}

/// Fixtures for the tests of the pieces and pages.
#[cfg(test)]
pub(crate) mod sample {
    use std::net::Ipv4Addr;

    use celestina_shell::connectivity::{
        BtIcon, BtRow, ChargeState, Degraded, NetDraft, NetKind, NetRow, NetSecurity, NetState,
    };

    use super::*;

    pub fn net(
        id: &str,
        kind: NetKind,
        name: &str,
        state: NetState,
        security: NetSecurity,
        known: bool,
    ) -> NetRow {
        NetRow::new(NetDraft {
            id,
            kind,
            name,
            state,
            signal: 70,
            security,
            known,
            address: (state == NetState::Connected).then_some(Ipv4Addr::new(192, 168, 1, 20)),
        })
        .expect("fixture row")
    }

    /// Home Wi-Fi connected, a saved one, an unsaved one, a VPN.
    pub fn network() -> NetSection {
        NetSection::new(
            true,
            false,
            [
                net(
                    "home",
                    NetKind::Wifi,
                    "HomeNet",
                    NetState::Connected,
                    NetSecurity::Passphrase,
                    true,
                ),
                net(
                    "cafe",
                    NetKind::Wifi,
                    "Cafe",
                    NetState::Disconnected,
                    NetSecurity::Open,
                    true,
                ),
                net(
                    "work",
                    NetKind::Wifi,
                    "Work",
                    NetState::Disconnected,
                    NetSecurity::Enterprise,
                    false,
                ),
                net(
                    "vpn",
                    NetKind::Vpn,
                    "Office VPN",
                    NetState::Disconnected,
                    NetSecurity::Open,
                    true,
                ),
            ],
        )
    }

    pub fn bluetooth() -> BtSection {
        BtSection::new(
            true,
            false,
            [
                BtRow::new(
                    "AA:BB:CC:DD:EE:01",
                    "Headset",
                    BtIcon::Audio,
                    true,
                    true,
                    true,
                    Some(80),
                ),
                BtRow::new(
                    "AA:BB:CC:DD:EE:02",
                    "Keyboard",
                    BtIcon::Keyboard,
                    true,
                    false,
                    true,
                    None,
                ),
                BtRow::new(
                    "AA:BB:CC:DD:EE:03",
                    "Stranger",
                    BtIcon::Other,
                    false,
                    false,
                    false,
                    None,
                ),
            ]
            .into_iter()
            .flatten(),
        )
    }

    pub fn battery(state: ChargeState, percent: f64) -> Option<BatterySection> {
        BatterySection::new(true, state, percent, 7200, 0)
    }

    pub fn profiles(active: &str) -> ProfileSection {
        ProfileSection::new(
            active,
            ["power-saver", "balanced", "performance"],
            Degraded::No,
        )
    }

    /// A state with every service ready.
    pub fn ready() -> ConnectivityState {
        let mut state = ConnectivityState::default();
        state.apply(&ConnectivityEvent::Network(Service::Ready(network())));
        state.apply(&ConnectivityEvent::Bluetooth(Service::Ready(bluetooth())));
        state.apply_power(&PowerEvent::Battery(
            battery(ChargeState::Discharging, 64.0).map_or(Service::Unavailable, Service::Ready),
        ));
        state.apply_power(&PowerEvent::Profiles(Service::Ready(profiles("balanced"))));
        state
    }

    pub fn id(raw: &str) -> RowId {
        RowId::new(raw).expect("fixture id")
    }

    pub fn address(raw: &str) -> BtAddress {
        BtAddress::parse(raw).expect("fixture address")
    }

    pub fn profile(raw: &str) -> ProfileName {
        ProfileName::new(raw).expect("fixture profile")
    }
}

#[cfg(test)]
mod tests {
    use celestina_shell::connectivity::{ChargeState, Section};

    use super::{sample::*, *};

    #[test]
    fn a_new_state_knows_nothing_and_every_section_is_unavailable() {
        let state = ConnectivityState::default();
        assert_eq!(state.network(), Service::Unavailable);
        assert_eq!(state.bluetooth(), Service::Unavailable);
        assert_eq!(state.battery(), Service::Unavailable);
        assert_eq!(state.profiles(), Service::Unavailable);
        assert_eq!(state.wifi_status(), RowStatus::Idle);
        let inventory = state.inventory();
        assert!(inventory.network.is_none() && inventory.bluetooth.is_none());
    }

    #[test]
    fn events_fill_the_sections_and_only_a_change_is_reported() {
        let mut state = ConnectivityState::default();
        let event = ConnectivityEvent::Network(Service::Ready(network()));
        assert!(state.apply(&event));
        assert!(!state.apply(&event), "the same news changes nothing");
        assert_eq!(state.network().ready().copied(), Some(&network()));
        // Unavailable is news too, and `Unavailable` twice is not.
        assert!(state.apply(&ConnectivityEvent::Network(Service::Unavailable)));
        assert!(!state.apply(&ConnectivityEvent::Network(Service::Unavailable)));
        assert!(state.apply(&ConnectivityEvent::Bluetooth(Service::Ready(bluetooth()))));
        assert!(state.apply_power(&PowerEvent::Profiles(Service::Ready(profiles("balanced")))));
        assert!(!state.apply_power(&PowerEvent::Profiles(Service::Ready(profiles("balanced")))));
        assert!(state.apply_power(&PowerEvent::Battery(Service::Ready(
            battery(ChargeState::Charging, 20.0).expect("battery")
        ))));
        assert_eq!(
            state.battery().ready().map(|b| b.state()),
            Some(ChargeState::Charging)
        );
    }

    #[test]
    fn a_request_is_pending_from_the_moment_it_is_sent_until_it_resolves() {
        let mut state = ready();
        let join = Action::JoinSaved(id("cafe"));
        assert_eq!(state.net_status(&id("cafe")), RowStatus::Idle);
        assert_eq!(state.begin(&join), Ok(()));
        assert_eq!(state.net_status(&id("cafe")), RowStatus::Pending);
        assert_eq!(state.net_status(&id("home")), RowStatus::Idle);
        // The worker's own news about the same request changes nothing.
        let expected = Expected::NetActive {
            id: id("cafe"),
            active: true,
        };
        assert!(!state.apply(&ConnectivityEvent::Pending(expected.clone())));
        assert!(
            state.apply(&ConnectivityEvent::Resolved(Resolution::Confirmed(
                expected
            )))
        );
        assert_eq!(state.net_status(&id("cafe")), RowStatus::Idle);
    }

    #[test]
    fn a_failure_stays_on_its_row_until_the_next_request_for_it() {
        let mut state = ready();
        let expected = Expected::NetActive {
            id: id("cafe"),
            active: true,
        };
        state.apply(&ConnectivityEvent::Pending(expected.clone()));
        assert!(state.apply(&ConnectivityEvent::Resolved(Resolution::Failed(
            expected,
            Failure::NeedsSecret
        ))));
        assert_eq!(
            state.net_status(&id("cafe")),
            RowStatus::Failed(Failure::NeedsSecret)
        );
        assert_eq!(state.net_status(&id("home")), RowStatus::Idle);
        assert_eq!(state.begin(&Action::JoinSaved(id("cafe"))), Ok(()));
        assert_eq!(state.net_status(&id("cafe")), RowStatus::Pending);
    }

    #[test]
    fn a_refused_request_shows_the_refusal_on_its_row_and_nothing_is_pending() {
        let mut state = ready();
        let work = Action::JoinSaved(id("work"));
        assert_eq!(state.begin(&work), Err(Refusal::NotKnown));
        assert_eq!(
            state.net_status(&id("work")),
            RowStatus::Failed(Failure::Refused(Refusal::NotKnown))
        );
        // A worker's refusal does the same.
        let vpn = Action::JoinSaved(id("vpn"));
        assert!(state.apply(&ConnectivityEvent::Refused(vpn, Refusal::KindNotAllowed)));
        assert_eq!(
            state.net_status(&id("vpn")),
            RowStatus::Failed(Failure::Refused(Refusal::KindNotAllowed))
        );
        // A refresh or an editor has no row and waits for nothing.
        assert_eq!(
            state.begin(&Action::Refresh(Section::Network)),
            Ok(()),
            "refresh is not refused"
        );
        assert_eq!(state.begin(&Action::OpenEditor(Editor::Network)), Ok(()));
    }

    #[test]
    fn a_request_with_no_inventory_is_refused_on_its_subject() {
        let mut state = ConnectivityState::default();
        assert_eq!(
            state.begin(&Action::SetWifiEnabled(true)),
            Err(Refusal::NoInventory(Section::Network))
        );
        assert_eq!(
            state.wifi_status(),
            RowStatus::Failed(Failure::Refused(Refusal::NoInventory(Section::Network)))
        );
    }

    #[test]
    fn each_subject_has_its_own_entry_and_a_new_request_replaces_the_old() {
        let mut state = ready();
        let headset = address("AA:BB:CC:DD:EE:01");
        let keyboard = address("AA:BB:CC:DD:EE:02");
        assert_eq!(
            state.begin(&Action::DisconnectDevice(headset.clone())),
            Ok(())
        );
        assert_eq!(
            state.begin(&Action::ConnectDevice(keyboard.clone())),
            Ok(())
        );
        assert_eq!(state.begin(&Action::SetBluetoothPowered(false)), Ok(()));
        assert_eq!(state.device_status(&headset), RowStatus::Pending);
        assert_eq!(state.device_status(&keyboard), RowStatus::Pending);
        assert_eq!(state.powered_status(), RowStatus::Pending);
        // Failing the headset leaves the keyboard pending.
        state.fail(&Action::DisconnectDevice(headset.clone()), Failure::Busy);
        assert_eq!(
            state.device_status(&headset),
            RowStatus::Failed(Failure::Busy)
        );
        assert_eq!(state.device_status(&keyboard), RowStatus::Pending);
        // There is one profile subject: the second request replaces the first.
        assert_eq!(
            state.begin(&Action::SetProfile(profile("performance"))),
            Ok(())
        );
        assert_eq!(
            state.begin(&Action::SetProfile(profile("power-saver"))),
            Ok(())
        );
        assert_eq!(
            state.profile_status(&profile("performance")),
            RowStatus::Idle
        );
        assert_eq!(
            state.profile_status(&profile("power-saver")),
            RowStatus::Pending
        );
    }

    #[test]
    fn cycling_marks_the_profile_the_daemon_will_be_asked_for() {
        let mut state = ready();
        assert_eq!(state.begin(&Action::CycleProfile), Ok(()));
        assert_eq!(
            state.profile_status(&profile("performance")),
            RowStatus::Pending,
            "balanced cycles to performance"
        );
        let mut empty = ConnectivityState::default();
        assert_eq!(
            empty.begin(&Action::CycleProfile),
            Err(Refusal::NoInventory(Section::Profiles))
        );
    }

    #[test]
    fn the_set_never_holds_more_than_the_pending_bound() {
        let mut state = ready();
        for n in 0..(MAX_PENDING * 3) {
            let row = id(&format!("row-{n}"));
            state.apply(&ConnectivityEvent::Pending(Expected::NetActive {
                id: row,
                active: true,
            }));
        }
        assert_eq!(state.entries.len(), MAX_PENDING);
        assert_eq!(
            state.net_status(&id("row-0")),
            RowStatus::Idle,
            "the oldest was dropped"
        );
        assert_eq!(
            state.net_status(&id(&format!("row-{}", MAX_PENDING * 3 - 1))),
            RowStatus::Pending
        );
    }

    #[test]
    fn power_resolutions_settle_the_profile_row() {
        let mut state = ready();
        assert_eq!(
            state.begin(&Action::SetProfile(profile("performance"))),
            Ok(())
        );
        let expected = Expected::Profile(profile("performance"));
        assert!(state.apply_power(&PowerEvent::Resolved(Resolution::Failed(
            expected,
            Failure::Denied
        ))));
        assert_eq!(
            state.profile_status(&profile("performance")),
            RowStatus::Failed(Failure::Denied)
        );
        assert!(
            state.apply_power(&PowerEvent::Resolved(Resolution::Confirmed(
                Expected::Profile(profile("performance"))
            )))
        );
        assert_eq!(
            state.profile_status(&profile("performance")),
            RowStatus::Idle
        );
        assert!(!state.apply_power(&PowerEvent::Refused(Refusal::NoSuchProfile)));
    }

    #[test]
    fn a_power_refusal_fails_the_pending_profile_request_and_power_news_arrives_wrapped() {
        let mut state = ready();
        assert_eq!(state.begin(&Action::CycleProfile), Ok(()));
        // Balanced cycles to performance: that row waits.
        assert_eq!(
            state.profile_status(&profile("performance")),
            RowStatus::Pending
        );
        // The worker's own check refused it (the list moved under it).
        assert!(state.apply(&ConnectivityEvent::Power(PowerEvent::Refused(
            Refusal::NoSuchProfile
        ))));
        assert_eq!(
            state.profile_status(&profile("performance")),
            RowStatus::Failed(Failure::Refused(Refusal::NoSuchProfile))
        );
        // Nothing pending: a refusal has no row to fail.
        assert!(!state.apply(&ConnectivityEvent::Power(PowerEvent::Refused(
            Refusal::NoSuchProfile
        ))));
        // The wrapped sections are the power sections.
        assert!(state.apply(&ConnectivityEvent::Power(PowerEvent::Profiles(
            Service::Unavailable
        ))));
        assert_eq!(state.profiles(), Service::Unavailable);
    }

    #[test]
    fn an_undelivered_cycle_fails_the_profile_it_marked() {
        let mut state = ready();
        assert_eq!(state.begin(&Action::CycleProfile), Ok(()));
        assert!(state.fail(&Action::CycleProfile, Failure::Busy));
        assert_eq!(
            state.profile_status(&profile("performance")),
            RowStatus::Failed(Failure::Busy)
        );
    }

    #[test]
    fn a_missing_editor_is_remembered_until_one_launches() {
        let mut state = ConnectivityState::default();
        assert!(!state.editor_missing(Editor::Network));
        assert!(state.set_editor_missing(Editor::Network, true));
        assert!(!state.set_editor_missing(Editor::Network, true));
        assert!(state.editor_missing(Editor::Network));
        assert!(!state.editor_missing(Editor::Bluetooth));
        assert!(state.set_editor_missing(Editor::Network, false));
        assert!(!state.editor_missing(Editor::Network));
    }
}
