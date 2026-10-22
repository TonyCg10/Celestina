//! The network provider: NetworkManager read through Cuprita's client
//! (`cuprita_core::nm::NmNetwork`) behind `Box<dyn Network>`, adapted into
//! the shell's bounded [`NetSection`] (CEL-2-F, decisions D2-D6, D10).
//!
//! The provider is driven by the lane loop in `super` ([`super::Provider`]):
//! it opens the client and its signal watcher, reads a snapshot and turns it
//! into the shell's types, and performs the steps [`validate`] let through.
//! Of the client's commands only four are ever called: `snapshot`,
//! `set_wifi_enabled`, `connect` (always with `None` for the secret: D4) and
//! `disconnect`. `forget`, `set_airplane` and `set_vpn_active` have no step
//! that reaches them, and `connect` is reached only for a row whose saved
//! profile exists ([`validate`] refuses the rest), so the client never takes
//! its `AddAndActivateConnection` path.
//!
//! Cuprita's model has no hardware kill switch; the section reports it as not
//! blocked, so NetworkManager's own refusal (the radio stays off) is what the
//! pending window observes. Its `Failed` row state (the last join of a saved
//! profile ended without the network) is not a shell state: the row reads
//! `Disconnected`, and a join of that row fails by kind.
//!
//! A join is confirmed as soon as NetworkManager takes the activation (the
//! row reads `Connecting`), but its outcome comes later: NetworkManager may
//! still give up on it (no secret, a wrong one, out of reach). The provider
//! therefore follows each Wi-Fi join (Cuprita marks a failure only for
//! Wi-Fi rows) until its row reads `Connected` or `Failed`, for at most
//! [`JOIN_FOLLOW`], and a failure observed after the confirmation is still
//! reported (the page then says `NeedsSecret` and offers the editor). A
//! row that already read `Failed` when the join began keeps that mark until
//! NetworkManager takes the new activation; if it still reads `Failed` once
//! the pending window is over, the mark is taken as this join's.
//!
//! Signal strength is kept only to the step the bar and the page draw
//! ([`SIGNAL_STEP`]): an access point's strength moves a little with every
//! scan, and a change nobody sees must not publish a new section (and so
//! redraw the bar).
//!
//! [`validate`]: celestina_shell::connectivity::validate

use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use celestina_shell::connectivity::{
    Expected, Failure, Inventory, NetDraft, NetKind, NetRow, NetSection, NetSecurity, NetState,
    Resolution, RowId, Section, Service, Step, PENDING_WINDOW,
};
use cuprita_core::error::NetworkError;
use cuprita_core::model::{
    Network as NetworkEntry, NetworkKind, NetworkSnapshot, NetworkState, Security,
};
use cuprita_core::network::Network;

use super::{ConnectivityEvent, Provider, WatchGuard};

/// Opens a client: production connects to the system bus.
pub type NetworkOpener = Box<dyn FnMut() -> Result<Box<dyn Network>, NetworkError> + Send>;
/// Starts a change watcher that raises the flag it is given on every
/// coalesced burst; `None` when it could not start (the lane then relies on
/// its periodic re-read).
pub type NetworkWatcher = Box<dyn FnMut(Arc<AtomicBool>) -> Option<WatchGuard> + Send>;

/// The longest a join is followed to its outcome. NetworkManager decides
/// every activation under its own timeouts (a secret agent's prompt waits up
/// to two minutes); past this a later failure mark is not this join's.
pub const JOIN_FOLLOW: Duration = Duration::from_secs(180);

/// The signal steps kept: four bars of 25 each, as the piece and the page
/// draw them (`island::pages::rows::signal_bars`).
pub const SIGNAL_STEP: u8 = 25;

/// `signal` (0..=100) rounded up to its bar step, so that 1..=25 is one bar
/// and 76..=100 four, as the bars are lit.
#[must_use]
pub fn signal_step(signal: u8) -> u8 {
    signal.min(100).div_ceil(SIGNAL_STEP) * SIGNAL_STEP
}

/// What a join under way knows about its row's failure mark.
#[derive(Clone, Copy, Debug)]
struct Join {
    /// The row read `Failed` before this join and has not read otherwise
    /// since: that mark is the previous join's, not this one's.
    stale: bool,
    /// The row's security class when the join started.
    security: NetSecurity,
    /// When the join was asked for.
    since: Instant,
}

/// The network lane's provider.
pub struct NetworkProvider {
    opener: NetworkOpener,
    watcher: NetworkWatcher,
    backend: Option<Box<dyn Network>>,
    /// Rows whose last join failed, as of the last snapshot.
    failed: HashSet<RowId>,
    /// Joins started and not yet at their outcome, by row.
    joins: HashMap<RowId, Join>,
    /// Joins whose failure was observed, waiting to be taken.
    observed: Vec<(Expected, Failure)>,
}

impl std::fmt::Debug for NetworkProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkProvider")
            .field("open", &self.backend.is_some())
            .field("following", &self.joins.len())
            .finish_non_exhaustive()
    }
}

impl NetworkProvider {
    /// A provider over `opener` and `watcher`; tests pass fakes.
    #[must_use]
    pub fn new(opener: NetworkOpener, watcher: NetworkWatcher) -> Self {
        Self {
            opener,
            watcher,
            backend: None,
            failed: HashSet::new(),
            joins: HashMap::new(),
            observed: Vec::new(),
        }
    }

    /// The provider over the system bus: `NmNetwork::connect_system` and
    /// `cuprita_core::nm::watch`. No notice callback is installed, so the
    /// client's Spanish notices (which quote network names) are never shown
    /// or logged by the shell.
    #[must_use]
    pub fn system() -> Self {
        Self::new(
            Box::new(|| {
                cuprita_core::nm::NmNetwork::connect_system()
                    .map(|client| Box::new(client) as Box<dyn Network>)
            }),
            Box::new(|dirty: Arc<AtomicBool>| {
                match cuprita_core::nm::watch(move || dirty.store(true, Ordering::Release)) {
                    Ok(handle) => Some(Box::new(handle) as WatchGuard),
                    Err(err) => {
                        tracing::warn!(
                            kind = error_kind(&err),
                            "network: no change watcher; re-reading periodically"
                        );
                        None
                    }
                }
            }),
        )
    }

    fn backend(&mut self) -> Result<&mut Box<dyn Network>, Failure> {
        self.backend.as_mut().ok_or(Failure::Unreachable)
    }

    fn convert(&mut self, snapshot: NetworkSnapshot, now: Instant) -> NetSection {
        let mut failed = HashSet::new();
        let rows: Vec<NetRow> = snapshot
            .networks
            .iter()
            .filter_map(|entry| {
                let row = row_of(entry)?;
                if entry.state == NetworkState::Failed {
                    failed.insert(row.id().clone());
                }
                Some(row)
            })
            .collect();
        let state_of = |id: &RowId| rows.iter().find(|row| row.id() == id).map(NetRow::state);
        let observed = &mut self.observed;
        self.joins.retain(|id, join| {
            // Followed to its end: the row reached the network, vanished,
            // or the join is too old for a later mark to be its own.
            match state_of(id) {
                None | Some(NetState::Connected) => return false,
                Some(_) if now.saturating_duration_since(join.since) >= JOIN_FOLLOW => {
                    return false
                }
                Some(_) => {}
            }
            if !failed.contains(id) {
                join.stale = false;
                return true;
            }
            // A mark that predates the join is not its own while the join
            // is young. One that still stands once the pending window is
            // over is: NetworkManager failed the new activation before any
            // read saw it `Connecting` (the mark was set again, not kept).
            if join.stale && now.saturating_duration_since(join.since) < PENDING_WINDOW {
                return true;
            }
            // A saved passphrase network that NetworkManager gave up on most
            // often lacks (or holds a wrong) secret; the page offers the
            // editor. Anything else is out of reach.
            let failure = if join.security == NetSecurity::Passphrase {
                Failure::NeedsSecret
            } else {
                Failure::Unreachable
            };
            observed.push((
                Expected::NetActive {
                    id: id.clone(),
                    active: true,
                },
                failure,
            ));
            false
        });
        self.failed = failed;
        NetSection::new(snapshot.wifi_enabled, false, rows)
    }
}

/// One of Cuprita's rows as a shell row; `None` for an id the shell's bounds
/// refuse (the row is then not shown and cannot be named by a request).
fn row_of(entry: &NetworkEntry) -> Option<NetRow> {
    NetRow::new(NetDraft {
        id: &entry.id,
        kind: match entry.kind {
            NetworkKind::Wifi => NetKind::Wifi,
            NetworkKind::Ethernet => NetKind::Ethernet,
            NetworkKind::Vpn => NetKind::Vpn,
        },
        name: &entry.name,
        state: match entry.state {
            NetworkState::Connected => NetState::Connected,
            NetworkState::Connecting => NetState::Connecting,
            NetworkState::Disconnected | NetworkState::Failed => NetState::Disconnected,
        },
        signal: signal_step(entry.signal.unwrap_or(0)),
        security: match entry.security {
            Security::Open => NetSecurity::Open,
            Security::Psk => NetSecurity::Passphrase,
            Security::Enterprise => NetSecurity::Enterprise,
            Security::Wep => NetSecurity::Wep,
        },
        known: entry.known,
        address: entry
            .address
            .as_deref()
            .and_then(|text| text.parse::<Ipv4Addr>().ok()),
    })
}

/// A client error by kind. The client's `Failed` detail is Spanish product
/// copy of Cuprita's or service text; it is matched only against the client's
/// own constructors and never shown or logged.
pub(super) fn failure_of(err: &NetworkError) -> Failure {
    match err {
        NetworkError::Unavailable => Failure::Unreachable,
        NetworkError::Denied => Failure::Denied,
        NetworkError::NotFound(_) => Failure::NotFound,
        NetworkError::Busy => Failure::Busy,
        NetworkError::Failed(_) if *err == NetworkError::password_required() => {
            Failure::NeedsSecret
        }
        NetworkError::Failed(_) => Failure::Other,
    }
}

/// A stable journal token for `err`, without its payload (which can hold an
/// id or service text).
pub(super) fn error_kind(err: &NetworkError) -> &'static str {
    match err {
        NetworkError::Unavailable => "unavailable",
        NetworkError::Denied => "denied",
        NetworkError::NotFound(_) => "not-found",
        NetworkError::Busy => "busy",
        NetworkError::Failed(_) => "failed",
    }
}

impl Provider for NetworkProvider {
    type State = NetSection;
    const WHAT: &'static str = "network";
    const SECTION: Section = Section::Network;

    fn open(&mut self) -> Result<(), Failure> {
        let backend = (self.opener)().map_err(|err| {
            tracing::debug!(kind = error_kind(&err), "network: cannot open the client");
            failure_of(&err)
        })?;
        self.backend = Some(backend);
        Ok(())
    }

    fn close(&mut self) {
        self.backend = None;
        self.failed.clear();
        self.joins.clear();
        self.observed.clear();
    }

    fn read(&mut self) -> Result<NetSection, Failure> {
        let snapshot = self.backend()?.snapshot().map_err(|err| {
            tracing::debug!(kind = error_kind(&err), "network: snapshot failed");
            failure_of(&err)
        })?;
        Ok(self.convert(snapshot, Instant::now()))
    }

    fn watch(&mut self, dirty: Arc<AtomicBool>) -> Option<WatchGuard> {
        (self.watcher)(dirty)
    }

    fn inventory(state: Option<&NetSection>) -> Inventory<'_> {
        Inventory {
            network: state,
            ..Inventory::default()
        }
    }

    fn perform(&mut self, step: &Step, state: Option<&NetSection>) -> Result<(), Failure> {
        let result = match step {
            Step::SetWifiEnabled(on) => self.backend()?.set_wifi_enabled(*on),
            Step::Join(id) => {
                // Only a Wi-Fi join is followed: Cuprita marks a failure only
                // for Wi-Fi rows, so a wired or VPN join could never report
                // one and would only keep the lane re-reading.
                if let Some(row) = state
                    .and_then(|section| section.row(id))
                    .filter(|row| row.kind() == NetKind::Wifi)
                {
                    self.joins.insert(
                        id.clone(),
                        Join {
                            stale: self.failed.contains(id),
                            security: row.security(),
                            since: Instant::now(),
                        },
                    );
                }
                // D4: never a secret. The step has nowhere to carry one and
                // the client is told there is none.
                self.backend()?.connect(id.as_str(), None)
            }
            Step::Leave(id) => {
                self.joins.remove(id);
                self.backend()?.disconnect(id.as_str())
            }
            other => {
                tracing::warn!(
                    step = super::step_kind(other),
                    "network: a step for another lane was dropped"
                );
                return Err(Failure::Other);
            }
        };
        result.map_err(|err| {
            tracing::info!(kind = error_kind(&err), "network: request failed");
            failure_of(&err)
        })
    }

    fn take_observed(&mut self) -> Vec<(Expected, Failure)> {
        std::mem::take(&mut self.observed)
    }

    fn settled(&mut self, resolution: &Resolution) {
        // A confirmed join is still followed (`convert` ends it); a failed
        // one, or a confirmed leave, has nothing left to follow.
        match resolution {
            Resolution::Confirmed(Expected::NetActive { active: true, .. }) => {}
            // A join that met a stale mark and timed out is kept: the next
            // read decides whether the mark that still stands is its own.
            Resolution::Failed(Expected::NetActive { id, active: true }, Failure::Timeout)
                if self.joins.get(id).is_some_and(|join| join.stale) => {}
            Resolution::Confirmed(Expected::NetActive { id, .. })
            | Resolution::Failed(Expected::NetActive { id, .. }, _) => {
                self.joins.remove(id);
            }
            Resolution::Confirmed(_) | Resolution::Failed(..) => {}
        }
    }

    fn following(&self) -> bool {
        !self.joins.is_empty()
    }

    fn event(state: Service<NetSection>) -> ConnectivityEvent {
        ConnectivityEvent::Network(state)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use cuprita_core::fake::FakeNetwork;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// Every call the provider made into the client.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) enum NetCall {
        Snapshot,
        SetWifiEnabled(bool),
        SetAirplane(bool),
        /// `secret` records only whether one was passed, never its value.
        Connect {
            id: String,
            secret: bool,
        },
        Disconnect(String),
        Forget(String),
        SetVpnActive(String, bool),
    }

    /// What a test controls about the recording client.
    #[derive(Default)]
    pub(crate) struct NetScript {
        pub(crate) calls: Vec<NetCall>,
        /// Errors the next commands (not snapshots) return, in order.
        pub(crate) fail_next: VecDeque<NetworkError>,
        /// While set, every call answers `Unavailable` (the service left).
        pub(crate) gone: bool,
        /// The next snapshot marks this row `Failed` (a join that ended
        /// without the network).
        pub(crate) mark_failed: Option<String>,
        /// While set, every snapshot shows this row in this state (an
        /// activation NetworkManager is still deciding, then its outcome).
        pub(crate) force_state: Option<(String, NetworkState)>,
        /// While set, every snapshot shows this row with this signal.
        pub(crate) force_signal: Option<(String, u8)>,
        /// While set, the client opens but its snapshots answer
        /// `Unavailable` (BlueZ with no adapter, NetworkManager not ready).
        pub(crate) unreadable: bool,
        /// How long every open and snapshot takes (a slow or loaded service).
        pub(crate) delay: Duration,
    }

    pub(crate) type Script = Arc<Mutex<NetScript>>;

    /// `FakeNetwork` behind a recorder. The fake is shared, so it outlives a
    /// reconnect as a real service would.
    pub(crate) struct Recording {
        pub(crate) fake: Arc<Mutex<FakeNetwork>>,
        pub(crate) script: Script,
    }

    impl Recording {
        fn note(&self, call: NetCall) -> Result<(), NetworkError> {
            let mut script = self.script.lock().expect("script");
            script.calls.push(call.clone());
            if script.gone {
                return Err(NetworkError::Unavailable);
            }
            if call != NetCall::Snapshot {
                if let Some(err) = script.fail_next.pop_front() {
                    return Err(err);
                }
            }
            Ok(())
        }

        fn fake(&self) -> std::sync::MutexGuard<'_, FakeNetwork> {
            self.fake.lock().expect("fake")
        }
    }

    impl Network for Recording {
        fn snapshot(&mut self) -> Result<NetworkSnapshot, NetworkError> {
            let delay = self.script.lock().expect("script").delay;
            std::thread::sleep(delay);
            self.note(NetCall::Snapshot)?;
            let mut snapshot = self.fake().snapshot()?;
            let mut script = self.script.lock().expect("script");
            if script.unreadable {
                return Err(NetworkError::Unavailable);
            }
            if let Some(id) = script.mark_failed.take() {
                for entry in &mut snapshot.networks {
                    if entry.id == id {
                        entry.state = NetworkState::Failed;
                    }
                }
            }
            for entry in &mut snapshot.networks {
                if let Some((id, state)) = &script.force_state {
                    if entry.id == *id {
                        entry.state = *state;
                    }
                }
                if let Some((id, signal)) = &script.force_signal {
                    if entry.id == *id {
                        entry.signal = Some(*signal);
                    }
                }
            }
            Ok(snapshot)
        }
        fn set_wifi_enabled(&mut self, on: bool) -> Result<(), NetworkError> {
            self.note(NetCall::SetWifiEnabled(on))?;
            self.fake().set_wifi_enabled(on)
        }
        fn set_airplane(&mut self, on: bool) -> Result<(), NetworkError> {
            self.note(NetCall::SetAirplane(on))?;
            self.fake().set_airplane(on)
        }
        fn connect(&mut self, id: &str, password: Option<&str>) -> Result<(), NetworkError> {
            self.note(NetCall::Connect {
                id: id.to_owned(),
                secret: password.is_some(),
            })?;
            self.fake().connect(id, password)
        }
        fn disconnect(&mut self, id: &str) -> Result<(), NetworkError> {
            self.note(NetCall::Disconnect(id.to_owned()))?;
            self.fake().disconnect(id)
        }
        fn forget(&mut self, id: &str) -> Result<(), NetworkError> {
            self.note(NetCall::Forget(id.to_owned()))?;
            self.fake().forget(id)
        }
        fn set_vpn_active(&mut self, id: &str, on: bool) -> Result<(), NetworkError> {
            self.note(NetCall::SetVpnActive(id.to_owned(), on))?;
            self.fake().set_vpn_active(id, on)
        }
    }

    /// A provider over a recording `FakeNetwork::scripted()` that every
    /// connection shares, and a counter of watchers started. While the
    /// script says `gone`, opening fails as with no NetworkManager.
    pub(crate) fn recording(script: &Script) -> (NetworkProvider, Arc<Mutex<usize>>) {
        let fake = Arc::new(Mutex::new(FakeNetwork::scripted()));
        let watchers = Arc::new(Mutex::new(0));
        let opener_script = Arc::clone(script);
        let opener: NetworkOpener = Box::new(move || {
            let delay = opener_script.lock().expect("script").delay;
            std::thread::sleep(delay);
            if opener_script.lock().expect("script").gone {
                return Err(NetworkError::Unavailable);
            }
            Ok(Box::new(Recording {
                fake: Arc::clone(&fake),
                script: Arc::clone(&opener_script),
            }) as Box<dyn Network>)
        });
        let counter = Arc::clone(&watchers);
        let watcher: NetworkWatcher = Box::new(move |_dirty| {
            *counter.lock().expect("counter") += 1;
            Some(Box::new(()) as WatchGuard)
        });
        (NetworkProvider::new(opener, watcher), watchers)
    }

    fn id(raw: &str) -> RowId {
        RowId::new(raw).expect("id")
    }

    fn opened(script: &Script) -> NetworkProvider {
        let (mut provider, _) = recording(script);
        provider.open().expect("opens");
        provider
    }

    #[test]
    fn the_scripted_snapshot_becomes_a_bounded_section() {
        let script = Script::default();
        let mut provider = opened(&script);

        let section = provider.read().expect("reads");

        assert!(section.wifi_enabled());
        assert!(!section.wifi_hardware_blocked());
        let wired = section.row(&id("wired")).expect("wired row");
        assert_eq!(wired.kind(), NetKind::Ethernet);
        assert_eq!(wired.state(), NetState::Connected);
        assert_eq!(wired.address(), Some(Ipv4Addr::new(192, 168, 1, 23)));
        let home = section.row(&id("home-wifi")).expect("home row");
        assert_eq!(home.security(), NetSecurity::Passphrase);
        // 72 is kept as its bar step.
        assert_eq!(home.signal(), 75);
        assert!(home.known());
        assert_eq!(
            section.row(&id("office-vpn")).map(NetRow::kind),
            Some(NetKind::Vpn)
        );
    }

    #[test]
    fn a_hostile_row_is_bounded_or_dropped() {
        let entry = |id: &str, name: &str, address: Option<&str>| NetworkEntry {
            id: id.to_owned(),
            kind: NetworkKind::Wifi,
            name: name.to_owned(),
            state: NetworkState::Disconnected,
            signal: Some(250),
            security: Security::Psk,
            known: false,
            address: address.map(str::to_owned),
        };
        assert!(row_of(&entry("", "x", None)).is_none());
        assert!(row_of(&entry(&"a".repeat(129), "x", None)).is_none());
        assert!(row_of(&entry("bad\nid", "x", None)).is_none());
        let row = row_of(&entry(
            "ok",
            &"\u{202e}x\u{7}".repeat(100),
            Some("not an ip"),
        ))
        .expect("kept");
        assert_eq!(row.signal(), 100);
        assert!(row.address().is_none());
        assert!(!row.name().contains('\u{202e}'));
        assert!(row.name().chars().count() <= 48 * 8);
    }

    #[test]
    fn a_join_passes_no_secret_and_a_leave_disconnects() {
        let script = Script::default();
        let mut provider = opened(&script);
        let section = provider.read().expect("reads");

        provider
            .perform(&Step::Join(id("home-wifi")), Some(&section))
            .expect("joins");
        provider
            .perform(&Step::Leave(id("wired")), Some(&section))
            .expect("leaves");

        let calls = script.lock().expect("script").calls.clone();
        assert!(calls.contains(&NetCall::Connect {
            id: "home-wifi".to_owned(),
            secret: false,
        }));
        assert!(calls.contains(&NetCall::Disconnect("wired".to_owned())));
    }

    #[test]
    fn steps_of_other_lanes_never_reach_the_client() {
        let script = Script::default();
        let mut provider = opened(&script);
        let before = script.lock().expect("script").calls.len();

        let err = provider.perform(&Step::SetBluetoothPowered(true), None);

        assert_eq!(err, Err(Failure::Other));
        assert_eq!(script.lock().expect("script").calls.len(), before);
    }

    #[test]
    fn client_errors_map_by_kind() {
        assert_eq!(failure_of(&NetworkError::Unavailable), Failure::Unreachable);
        assert_eq!(failure_of(&NetworkError::Denied), Failure::Denied);
        assert_eq!(
            failure_of(&NetworkError::NotFound("x".to_owned())),
            Failure::NotFound
        );
        assert_eq!(failure_of(&NetworkError::Busy), Failure::Busy);
        assert_eq!(
            failure_of(&NetworkError::password_required()),
            Failure::NeedsSecret
        );
        assert_eq!(failure_of(&NetworkError::service_failed()), Failure::Other);
    }

    #[test]
    fn a_failed_mark_fails_the_join_only_once_it_is_this_joins() {
        let script = Script::default();
        let mut provider = opened(&script);
        // The row's previous join failed: the mark predates this one.
        script.lock().expect("script").mark_failed = Some("home-wifi".to_owned());
        let section = provider.read().expect("reads");
        assert_eq!(
            section.row(&id("home-wifi")).map(NetRow::state),
            Some(NetState::Disconnected)
        );
        script
            .lock()
            .expect("script")
            .fail_next
            .push_back(NetworkError::service_failed());
        let _ = provider.perform(&Step::Join(id("home-wifi")), Some(&section));
        script.lock().expect("script").mark_failed = Some("home-wifi".to_owned());
        provider.read().expect("reads");
        assert!(
            provider.take_observed().is_empty(),
            "a stale mark is not this join's"
        );

        // Read once without the mark, then the mark is this join's.
        provider.read().expect("reads");
        script.lock().expect("script").mark_failed = Some("home-wifi".to_owned());
        provider.read().expect("reads");
        assert_eq!(
            provider.take_observed(),
            vec![(
                Expected::NetActive {
                    id: id("home-wifi"),
                    active: true,
                },
                Failure::NeedsSecret,
            )]
        );
    }

    #[test]
    fn only_a_wifi_join_is_followed() {
        let script = Script::default();
        let mut provider = opened(&script);
        let section = provider.read().expect("reads");
        assert_eq!(
            section.row(&id("wired")).map(NetRow::kind),
            Some(NetKind::Ethernet)
        );
        let _ = provider.perform(&Step::Join(id("wired")), Some(&section));
        assert!(!provider.following(), "a wired join reports no failure");
        provider
            .perform(&Step::Join(id("home-wifi")), Some(&section))
            .expect("joins");
        assert!(provider.following());
    }

    #[test]
    fn a_stale_mark_that_outlasts_the_pending_window_is_this_joins() {
        let script = Script::default();
        let mut provider = opened(&script);
        let join = Expected::NetActive {
            id: id("home-wifi"),
            active: true,
        };
        // The previous join failed, and NetworkManager fails the new one
        // before any read sees it `Connecting`: the row never stops reading
        // `Failed`.
        script.lock().expect("script").force_state =
            Some(("home-wifi".to_owned(), NetworkState::Failed));
        let section = provider.read().expect("reads");
        provider
            .perform(&Step::Join(id("home-wifi")), Some(&section))
            .expect("joins");
        provider.read().expect("reads");
        assert!(
            provider.take_observed().is_empty(),
            "young: not yet its own"
        );
        // The lane's window ends in a timeout; the join is still followed.
        provider.settled(&Resolution::Failed(join.clone(), Failure::Timeout));
        assert!(provider.following());
        let snapshot = provider
            .backend()
            .expect("open")
            .snapshot()
            .expect("snapshot");
        provider.convert(snapshot, Instant::now() + PENDING_WINDOW);
        assert_eq!(provider.take_observed(), vec![(join, Failure::NeedsSecret)]);
        assert!(!provider.following());
    }

    #[test]
    fn signal_is_kept_to_its_bar_step() {
        assert_eq!(signal_step(0), 0);
        assert_eq!(signal_step(1), 25);
        assert_eq!(signal_step(25), 25);
        assert_eq!(signal_step(26), 50);
        assert_eq!(signal_step(72), 75);
        assert_eq!(signal_step(76), 100);
        assert_eq!(signal_step(100), 100);
        assert_eq!(signal_step(250), 100);
    }

    #[test]
    fn strength_jitter_inside_a_step_reads_as_the_same_section() {
        let script = Script::default();
        let mut provider = opened(&script);
        script.lock().expect("script").force_signal = Some(("home-wifi".to_owned(), 71));
        let before = provider.read().expect("reads");
        script.lock().expect("script").force_signal = Some(("home-wifi".to_owned(), 74));
        let jittered = provider.read().expect("reads");
        assert_eq!(before, jittered, "a change nobody sees is no change");
        script.lock().expect("script").force_signal = Some(("home-wifi".to_owned(), 80));
        let stronger = provider.read().expect("reads");
        assert_ne!(before, stronger, "a bar more is a change");
    }

    #[test]
    fn a_join_confirmed_while_connecting_still_reports_its_failure() {
        let script = Script::default();
        let mut provider = opened(&script);
        let section = provider.read().expect("reads");
        let join = Expected::NetActive {
            id: id("home-wifi"),
            active: true,
        };
        script.lock().expect("script").force_state =
            Some(("home-wifi".to_owned(), NetworkState::Connecting));
        provider
            .perform(&Step::Join(id("home-wifi")), Some(&section))
            .expect("joins");
        provider.read().expect("reads");
        // The lane confirms the join at `Connecting`; the provider keeps
        // following it.
        provider.settled(&Resolution::Confirmed(join.clone()));
        assert!(provider.following());
        assert!(provider.take_observed().is_empty());

        // NetworkManager gives up on it (no secret).
        script.lock().expect("script").force_state =
            Some(("home-wifi".to_owned(), NetworkState::Failed));
        provider.read().expect("reads");
        assert_eq!(provider.take_observed(), vec![(join, Failure::NeedsSecret)]);
        assert!(!provider.following());
    }

    #[test]
    fn a_join_is_followed_until_connected_or_too_old() {
        let script = Script::default();
        let mut provider = opened(&script);
        let section = provider.read().expect("reads");
        let join = Expected::NetActive {
            id: id("home-wifi"),
            active: true,
        };
        script.lock().expect("script").force_state =
            Some(("home-wifi".to_owned(), NetworkState::Connecting));
        provider
            .perform(&Step::Join(id("home-wifi")), Some(&section))
            .expect("joins");
        provider.read().expect("reads");
        provider.settled(&Resolution::Confirmed(join.clone()));
        script.lock().expect("script").force_state =
            Some(("home-wifi".to_owned(), NetworkState::Connected));
        provider.read().expect("reads");
        assert!(!provider.following(), "reached the network");

        // A join that never ends is let go after the bound, and a later
        // failure mark is then not its own.
        script.lock().expect("script").force_state =
            Some(("home-wifi".to_owned(), NetworkState::Connecting));
        provider
            .perform(&Step::Join(id("home-wifi")), Some(&section))
            .expect("joins");
        provider.settled(&Resolution::Confirmed(join));
        let snapshot = |provider: &mut NetworkProvider| {
            provider
                .backend()
                .expect("open")
                .snapshot()
                .expect("snapshot")
        };
        let later = Instant::now() + JOIN_FOLLOW;
        let current = snapshot(&mut provider);
        provider.convert(current, later);
        assert!(!provider.following(), "too old to follow");
        script.lock().expect("script").force_state =
            Some(("home-wifi".to_owned(), NetworkState::Failed));
        let current = snapshot(&mut provider);
        provider.convert(current, later);
        assert!(provider.take_observed().is_empty());
    }
}
