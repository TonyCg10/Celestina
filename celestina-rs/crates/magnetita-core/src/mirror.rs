//! The wireless screen mirror — where the phone is, and what to do about it.
//!
//! Mirroring is not KDE Connect. It rides on Android's own wireless debugging:
//! the phone, once the user enables that switch, advertises two mDNS services
//! on the LAN — `_adb-tls-pairing._tcp` while its pairing screen is open, and
//! `_adb-tls-connect._tcp` for as long as debugging is on. Both carry the
//! address and a **port Android randomises every time the switch is toggled**,
//! which is precisely why a hardcoded port cannot work and a discovery watcher
//! can.
//!
//! This module is the pure half: it turns what a watcher heard into a validated
//! [`MirrorEndpoint`], and drives a [`MirrorLink`] state machine that answers
//! one question — *given what we know, what should the daemon run next?* The
//! answer is a typed [`MirrorAction`], never a command line. Browsing mDNS,
//! spawning `adb` and owning the `scrcpy` process are the daemon's I/O, tested
//! there.
//!
//! **An advertisement is hostile input.** It arrives unauthenticated from any
//! host on the LAN, and its address and port are about to become arguments to a
//! subprocess — the same shape as the SFTP reply that could once redirect the
//! mount ([`sftp`](crate::sftp)). So a host must parse as an IP literal, a port
//! must be a non-zero `u16`, and a pairing service is only ever accepted under
//! the exact name this host generated for its own pairing attempt. Discovery
//! says *where to look*; ADB's TLS pairing is what says *who answered*.

use std::net::IpAddr;

/// The mDNS service Android advertises for as long as wireless debugging is on.
pub const SERVICE_CONNECT: &str = "_adb-tls-connect._tcp";

/// The mDNS service Android advertises only while its pairing screen is open.
pub const SERVICE_PAIRING: &str = "_adb-tls-pairing._tcp";

/// The port `adb tcpip` puts the device on, and the one this daemon pins.
///
/// Wireless debugging and this are **two different listeners**. Android turns
/// the first off constantly — on every reboot, and on its own whenever it feels
/// like it — and with it goes the mDNS advertisement this daemon discovers by.
/// A device moved to `adb tcpip` keeps answering here regardless: measured on
/// the author's S25U, turning wireless debugging off stopped the advertisement
/// dead while this port stayed open and the device stayed `device`.
///
/// So the mirror remembers the last host it reached and dials this port
/// directly, which is what lets it work when there is nothing to discover.
/// It does not survive the *phone* rebooting — `persist.adb.tcp.port` is the
/// only thing that would, and setting it needs root the author's phone
/// does not have.
pub const FIXED_PORT: u16 = 5555;

/// Longest service name accepted from the network. Real ADB names are far
/// shorter; the bound only stops a peer handing us a name no honest phone
/// would advertise.
const MAX_SERVICE_NAME: usize = 128;

/// Which of the two ADB services an advertisement belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdbService {
    /// Wireless debugging is on and reachable here.
    Connect,
    /// A pairing screen is open on the phone.
    Pairing,
}

impl AdbService {
    /// The mDNS service type as advertised.
    pub fn service_type(self) -> &'static str {
        match self {
            AdbService::Connect => SERVICE_CONNECT,
            AdbService::Pairing => SERVICE_PAIRING,
        }
    }

    /// The service an advertised type names, or `None` for anything else.
    pub fn from_service_type(service_type: &str) -> Option<Self> {
        match service_type {
            SERVICE_CONNECT => Some(AdbService::Connect),
            SERVICE_PAIRING => Some(AdbService::Pairing),
            _ => None,
        }
    }
}

/// A validated place to reach ADB. Constructing one is the only way an address
/// from the network becomes a subprocess argument, so every check lives here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MirrorEndpoint {
    pub host: IpAddr,
    pub port: u16,
}

impl MirrorEndpoint {
    /// Validates a host and port exactly as a watcher heard them.
    ///
    /// The host must parse as an IP literal: a name would be resolved later, by
    /// something else, against whatever the network says at that moment, and
    /// this is a value chosen by an unauthenticated peer. The port must be a
    /// real, non-zero port.
    pub fn parse(host: &str, port: u32) -> Result<Self, MirrorError> {
        let host: IpAddr = host.parse().map_err(|_| MirrorError::BadAddress)?;
        let port = u16::try_from(port).map_err(|_| MirrorError::BadPort)?;
        if port == 0 {
            return Err(MirrorError::BadPort);
        }
        Ok(Self { host, port })
    }

    /// The `host:port` ADB names this endpoint by. Safe to build because both
    /// halves were validated at construction; it is still passed as one
    /// argument in a vector, never through a shell.
    pub fn serial(&self) -> String {
        match self.host {
            IpAddr::V4(v4) => format!("{v4}:{}", self.port),
            IpAddr::V6(v6) => format!("[{v6}]:{}", self.port),
        }
    }
}

/// Checks a service name from the network before it is compared or logged.
/// Bounded, non-empty, and free of control characters — a name is peer-chosen
/// text that ends up in a log line and in a match against our own.
pub fn valid_service_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= MAX_SERVICE_NAME && !name.chars().any(|c| c.is_control())
}

/// Why the mirror could not proceed. Language-neutral, like [`LostReason`]: the
/// human wording belongs to the app.
///
/// [`LostReason`]: crate::event::LostReason
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MirrorError {
    /// The advertised host was not an IP literal.
    BadAddress,
    /// The advertised port was zero or out of range.
    BadPort,
    /// The advertised service name was empty, over-long or had control characters.
    BadServiceName,
    /// Nothing is advertising — wireless debugging is off, or the phone is on
    /// another network. This is the expected state, not a fault.
    NotAdvertised,
    /// The phone refused the pairing code.
    PairRejected,
    /// `adb connect` did not reach the advertised endpoint.
    ConnectFailed,
    /// The device was reachable but never became `device` state.
    DeviceOffline,
    /// scrcpy could not start, or exited before the mirror was up.
    MirrorFailed,
    /// `adb` or `scrcpy` is not installed.
    ToolMissing,
    /// There is no graphical session to open the mirror window on. The daemon
    /// can outlive and even predate the compositor, so this is a state it must
    /// be able to say rather than a fault to retry.
    NoDisplay,
}

/// What the daemon should do next. Never a command line — the daemon owns how
/// each of these becomes a bounded, reaped subprocess it can kill by pid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MirrorAction {
    /// Nothing to do in this state.
    None,
    /// Run `adb pair` against a discovered pairing endpoint with this code.
    Pair {
        endpoint: MirrorEndpoint,
        code: String,
    },
    /// Run `adb connect` against a discovered connect endpoint.
    Connect { endpoint: MirrorEndpoint },
    /// Start scrcpy for this ADB serial.
    StartMirror { serial: String },
    /// Stop the scrcpy this daemon started, by its pid.
    StopMirror,
}

/// Where the mirror stands. The app renders this and nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MirrorState {
    /// No ADB service is advertised. Wireless debugging is off.
    Idle,
    /// A connect service is advertised but we have not been asked to mirror.
    Available,
    /// A pairing code is being exchanged.
    Pairing,
    /// `adb connect` is in flight.
    Connecting,
    /// ADB has the device; scrcpy has not started.
    Connected,
    /// scrcpy is up.
    Mirroring,
    /// The last attempt failed for this reason and needs a fresh request.
    Failed(MirrorError),
}

/// Something the daemon observed. The state machine consumes these and nothing
/// else, so its whole behaviour is testable without a phone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MirrorEvent {
    /// A validated advertisement appeared.
    ServiceFound {
        service: AdbService,
        endpoint: MirrorEndpoint,
    },
    /// An advertisement went away.
    ServiceLost { service: AdbService },
    /// The author pressed Mirror.
    MirrorRequested,
    /// The author supplied a pairing code.
    CodeEntered { code: String },
    /// `adb pair` succeeded or failed.
    PairFinished { paired: bool },
    /// `adb connect` reached `device` state, or did not.
    /// A connection attempt finished. `Some` carries the endpoint actually
    /// reached, which is not always the one dialled: pinning the fixed port
    /// restarts `adbd` and moves the device onto it.
    ConnectFinished { endpoint: Option<MirrorEndpoint> },
    /// scrcpy started under this pid.
    MirrorStarted { pid: u32 },
    /// scrcpy exited, whether by the author closing the window or by failing.
    MirrorExited { failed: bool },
    /// The author asked to stop.
    StopRequested,
    /// A tool the mirror needs is not installed.
    ToolMissing,
    /// No graphical session could be found to open the window on.
    DisplayMissing,
}

/// The mirror's whole decision-making, as a pure machine.
///
/// It holds the last *advertised* endpoints because Android re-randomises the
/// discovery port on every toggle: the machine must act on the current
/// advertisement, never a remembered one. Losing the connect service therefore
/// drops that endpoint rather than keeping a stale port to retry — the exact
/// failure the author's script worked around with a port cache.
///
/// [`remembered`](Self::remembered) is the deliberate exception, and it is not
/// the same thing. It is never a discovered port: it is the host we last
/// reached, on [`FIXED_PORT`], which `adb tcpip` keeps open whether or not
/// wireless debugging is on and whether or not anything is being advertised.
/// Remembering a *discovered* port would resurrect the stale-cache bug;
/// remembering the fixed one is what lets the mirror work with nothing to
/// discover.
#[derive(Clone, Debug, Default)]
pub struct MirrorLink {
    connect: Option<MirrorEndpoint>,
    pairing: Option<MirrorEndpoint>,
    /// The host last reached, on [`FIXED_PORT`]. Survives the advertisement.
    remembered: Option<MirrorEndpoint>,
    /// What the in-flight connect is dialling, so a failure knows whether the
    /// remembered port is still worth trying.
    dialling: Option<MirrorEndpoint>,
    state: MirrorStateInner,
    /// Set while the author's intent to mirror outlives a reconnection.
    wanted: bool,
    mirror_pid: Option<u32>,
    /// Set when *we* stopped the mirror because the phone went away, so the
    /// exit that follows is not read as the author closing the window.
    stopped_by_loss: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum MirrorStateInner {
    #[default]
    Idle,
    Pairing,
    Connecting,
    Connected,
    Mirroring,
    Failed(MirrorError),
}

impl MirrorLink {
    pub fn new() -> Self {
        Self::default()
    }

    /// A link that already knows where the phone answered last time.
    ///
    /// Only a [`FIXED_PORT`] endpoint is accepted: a remembered discovery port
    /// is exactly the stale cache this design refuses.
    pub fn with_remembered(endpoint: MirrorEndpoint) -> Self {
        let mut link = Self::default();
        if endpoint.port == FIXED_PORT {
            link.remembered = Some(endpoint);
        }
        link
    }

    /// The host last reached on the fixed port, if any. The daemon persists
    /// this so a restart does not lose it.
    pub fn remembered(&self) -> Option<MirrorEndpoint> {
        self.remembered
    }

    /// Where a mirror request should dial: whatever is advertised now, and
    /// otherwise the remembered fixed port.
    fn target(&self) -> Option<MirrorEndpoint> {
        self.connect.or(self.remembered)
    }

    /// What the app shows.
    pub fn state(&self) -> MirrorState {
        match self.state {
            MirrorStateInner::Idle => {
                // A remembered fixed port is as real a way in as an
                // advertisement, so the control must not read `Idle` — there is
                // something to press.
                if self.target().is_some() {
                    MirrorState::Available
                } else {
                    MirrorState::Idle
                }
            }
            MirrorStateInner::Pairing => MirrorState::Pairing,
            MirrorStateInner::Connecting => MirrorState::Connecting,
            MirrorStateInner::Connected => MirrorState::Connected,
            MirrorStateInner::Mirroring => MirrorState::Mirroring,
            MirrorStateInner::Failed(reason) => MirrorState::Failed(reason),
        }
    }

    /// True while a pairing screen is open on the phone, which is the only
    /// moment a code can be exchanged.
    pub fn can_pair(&self) -> bool {
        self.pairing.is_some()
    }

    /// The pid of the scrcpy this link started, and the only one it may kill.
    pub fn mirror_pid(&self) -> Option<u32> {
        self.mirror_pid
    }

    /// Advances the machine and says what to run next.
    pub fn handle(&mut self, event: MirrorEvent) -> MirrorAction {
        match event {
            MirrorEvent::ServiceFound { service, endpoint } => {
                match service {
                    AdbService::Connect => self.connect = Some(endpoint),
                    AdbService::Pairing => self.pairing = Some(endpoint),
                }
                // The port changes on every toggle, so a standing intent to
                // mirror is honoured the moment a *current* endpoint exists.
                if service == AdbService::Connect
                    && self.wanted
                    && matches!(
                        self.state,
                        MirrorStateInner::Idle | MirrorStateInner::Failed(_)
                    )
                {
                    self.state = MirrorStateInner::Connecting;
                    self.dialling = Some(endpoint);
                    return MirrorAction::Connect { endpoint };
                }
                MirrorAction::None
            }
            MirrorEvent::ServiceLost { service } => {
                match service {
                    AdbService::Connect => self.connect = None,
                    AdbService::Pairing => self.pairing = None,
                }
                if service == AdbService::Connect && self.mirror_pid.is_some() {
                    // The phone is gone; the mirror cannot survive it. The
                    // author did not ask for this, so the standing intent
                    // stands and the next advertisement reconnects.
                    self.state = MirrorStateInner::Failed(MirrorError::NotAdvertised);
                    self.stopped_by_loss = true;
                    return MirrorAction::StopMirror;
                }
                if service == AdbService::Connect
                    && matches!(
                        self.state,
                        MirrorStateInner::Connecting | MirrorStateInner::Connected
                    )
                {
                    self.state = MirrorStateInner::Failed(MirrorError::NotAdvertised);
                }
                MirrorAction::None
            }
            MirrorEvent::MirrorRequested => {
                self.wanted = true;
                // Asking for a mirror that is already up is not a request to
                // restart it. Two surfaces can ask — the app's control and the
                // shell's phone menu — and neither should tear down a window
                // the author is looking at.
                if matches!(self.state, MirrorStateInner::Mirroring) {
                    return MirrorAction::None;
                }
                match self.target() {
                    Some(endpoint) => {
                        self.state = MirrorStateInner::Connecting;
                        self.dialling = Some(endpoint);
                        MirrorAction::Connect { endpoint }
                    }
                    None => {
                        self.state = MirrorStateInner::Failed(MirrorError::NotAdvertised);
                        MirrorAction::None
                    }
                }
            }
            MirrorEvent::CodeEntered { code } => match self.pairing {
                Some(endpoint) => {
                    self.state = MirrorStateInner::Pairing;
                    MirrorAction::Pair { endpoint, code }
                }
                None => {
                    self.state = MirrorStateInner::Failed(MirrorError::NotAdvertised);
                    MirrorAction::None
                }
            },
            MirrorEvent::PairFinished { paired } => {
                if !paired {
                    self.state = MirrorStateInner::Failed(MirrorError::PairRejected);
                    return MirrorAction::None;
                }
                // Pairing closes the phone's pairing screen, so that service
                // is gone whether or not the watcher has noticed yet.
                self.pairing = None;
                match self.connect {
                    Some(endpoint) => {
                        self.state = MirrorStateInner::Connecting;
                        self.dialling = Some(endpoint);
                        MirrorAction::Connect { endpoint }
                    }
                    None => {
                        self.state = MirrorStateInner::Idle;
                        MirrorAction::None
                    }
                }
            }
            MirrorEvent::ConnectFinished { endpoint } => {
                let dialled = self.dialling.take();
                let Some(endpoint) = endpoint else {
                    // An advertisement can outlive the thing it advertises:
                    // Avahi served a cached record for a minute after Android
                    // turned wireless debugging off, and dialling that dead
                    // port used to end the attempt while the fixed port was
                    // answering. Try the remembered one before giving up —
                    // once, so a dead pair cannot loop.
                    if let Some(remembered) = self.remembered {
                        if dialled != Some(remembered) {
                            self.state = MirrorStateInner::Connecting;
                            self.dialling = Some(remembered);
                            return MirrorAction::Connect {
                                endpoint: remembered,
                            };
                        }
                    }
                    self.state = MirrorStateInner::Failed(MirrorError::ConnectFailed);
                    return MirrorAction::None;
                };
                // Remember only the fixed port. That is the one that answers
                // when nothing is advertised; a discovered port would be stale
                // by the next toggle.
                if endpoint.port == FIXED_PORT {
                    self.remembered = Some(endpoint);
                }
                self.state = MirrorStateInner::Connected;
                MirrorAction::StartMirror {
                    serial: endpoint.serial(),
                }
            }
            MirrorEvent::MirrorStarted { pid } => {
                self.mirror_pid = Some(pid);
                self.state = MirrorStateInner::Mirroring;
                MirrorAction::None
            }
            MirrorEvent::MirrorExited { failed } => {
                self.mirror_pid = None;
                // Only a *clean* exit is the author closing the window, and
                // only that clears the standing intent. A failed exit is
                // collateral — and against the real phone it is the common
                // case: toggling Wireless debugging kills the adb link at
                // once, so scrcpy dies several seconds before the mDNS record
                // lapses. Reading that first exit as a decision left the mirror
                // dark until the button was pressed again, which is precisely
                // the input this feature exists to remove.
                self.stopped_by_loss = false;
                if !failed {
                    self.wanted = false;
                }
                self.state = if !failed {
                    MirrorStateInner::Idle
                } else if matches!(self.state, MirrorStateInner::Failed(reason) if reason == MirrorError::NotAdvertised)
                {
                    // Keep the reason that actually explains it: the phone
                    // went away. scrcpy dying was the consequence.
                    self.state
                } else {
                    MirrorStateInner::Failed(MirrorError::MirrorFailed)
                };
                MirrorAction::None
            }
            MirrorEvent::StopRequested => {
                self.wanted = false;
                self.stopped_by_loss = false;
                self.state = MirrorStateInner::Idle;
                if self.mirror_pid.is_some() {
                    MirrorAction::StopMirror
                } else {
                    MirrorAction::None
                }
            }
            MirrorEvent::ToolMissing => {
                self.wanted = false;
                self.state = MirrorStateInner::Failed(MirrorError::ToolMissing);
                MirrorAction::None
            }
            MirrorEvent::DisplayMissing => {
                // Not retried: a missing session is not something the phone
                // reappearing can fix, and a mirror that respawns scrcpy every
                // few seconds against no display is the loop this reports
                // instead of.
                self.wanted = false;
                self.mirror_pid = None;
                self.state = MirrorStateInner::Failed(MirrorError::NoDisplay);
                MirrorAction::None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(port: u16) -> MirrorEndpoint {
        MirrorEndpoint {
            host: "10.0.0.85".parse().unwrap(),
            port,
        }
    }

    fn found(service: AdbService, port: u16) -> MirrorEvent {
        MirrorEvent::ServiceFound {
            service,
            endpoint: endpoint(port),
        }
    }

    #[test]
    fn endpoint_takes_only_ip_literals() {
        assert!(MirrorEndpoint::parse("10.0.0.85", 37059).is_ok());
        assert!(MirrorEndpoint::parse("fe80::1", 37059).is_ok());
        // A name would be resolved later against whatever the network says.
        assert_eq!(
            MirrorEndpoint::parse("phone.local", 37059),
            Err(MirrorError::BadAddress)
        );
        assert_eq!(
            MirrorEndpoint::parse("10.0.0.85 --exit-on-error", 37059),
            Err(MirrorError::BadAddress)
        );
    }

    #[test]
    fn endpoint_refuses_impossible_ports() {
        assert_eq!(
            MirrorEndpoint::parse("10.0.0.85", 0),
            Err(MirrorError::BadPort)
        );
        assert_eq!(
            MirrorEndpoint::parse("10.0.0.85", 70000),
            Err(MirrorError::BadPort)
        );
    }

    #[test]
    fn serial_brackets_ipv6() {
        assert_eq!(endpoint(5555).serial(), "10.0.0.85:5555");
        let v6 = MirrorEndpoint::parse("fe80::1", 5555).unwrap();
        assert_eq!(v6.serial(), "[fe80::1]:5555");
    }

    #[test]
    fn service_names_are_bounded_and_printable() {
        assert!(valid_service_name("adb-39121FDJH00KJP-Xy7Kq2"));
        assert!(!valid_service_name(""));
        assert!(!valid_service_name("adb\nrogue"));
        assert!(!valid_service_name(&"a".repeat(MAX_SERVICE_NAME + 1)));
    }

    #[test]
    fn mirror_without_an_advertisement_explains_rather_than_guesses() {
        let mut link = MirrorLink::new();
        assert_eq!(link.state(), MirrorState::Idle);
        assert_eq!(
            link.handle(MirrorEvent::MirrorRequested),
            MirrorAction::None
        );
        assert_eq!(
            link.state(),
            MirrorState::Failed(MirrorError::NotAdvertised)
        );
    }

    #[test]
    fn one_press_reaches_a_mirror() {
        let mut link = MirrorLink::new();
        link.handle(found(AdbService::Connect, 37059));
        assert_eq!(link.state(), MirrorState::Available);

        assert_eq!(
            link.handle(MirrorEvent::MirrorRequested),
            MirrorAction::Connect {
                endpoint: endpoint(37059)
            }
        );
        assert_eq!(
            link.handle(MirrorEvent::ConnectFinished {
                endpoint: Some(endpoint(37059))
            }),
            MirrorAction::StartMirror {
                serial: "10.0.0.85:37059".to_owned()
            }
        );
        link.handle(MirrorEvent::MirrorStarted { pid: 4242 });
        assert_eq!(link.state(), MirrorState::Mirroring);
        assert_eq!(link.mirror_pid(), Some(4242));
    }

    #[test]
    fn asking_again_never_tears_down_a_running_mirror() {
        let mut link = MirrorLink::new();
        link.handle(found(AdbService::Connect, 37059));
        link.handle(MirrorEvent::MirrorRequested);
        link.handle(MirrorEvent::ConnectFinished {
            endpoint: Some(endpoint(37059)),
        });
        link.handle(MirrorEvent::MirrorStarted { pid: 4242 });

        assert_eq!(
            link.handle(MirrorEvent::MirrorRequested),
            MirrorAction::None
        );
        assert_eq!(link.state(), MirrorState::Mirroring);
        assert_eq!(link.mirror_pid(), Some(4242));
    }

    #[test]
    fn a_toggled_switch_reconnects_on_the_new_port_without_input() {
        let mut link = MirrorLink::new();
        link.handle(found(AdbService::Connect, 37059));
        link.handle(MirrorEvent::MirrorRequested);
        link.handle(MirrorEvent::ConnectFinished {
            endpoint: Some(endpoint(37059)),
        });
        link.handle(MirrorEvent::MirrorStarted { pid: 4242 });

        // Wireless debugging off: the mirror cannot outlive the phone.
        assert_eq!(
            link.handle(MirrorEvent::ServiceLost {
                service: AdbService::Connect
            }),
            MirrorAction::StopMirror
        );
        link.handle(MirrorEvent::MirrorExited { failed: true });

        // Back on, with the new random port Android just picked. The standing
        // intent is honoured with no press, and never on the stale port.
        assert_eq!(
            link.state(),
            MirrorState::Failed(MirrorError::NotAdvertised)
        );
        assert_eq!(
            link.handle(found(AdbService::Connect, 41887)),
            MirrorAction::Connect {
                endpoint: endpoint(41887)
            }
        );
    }

    #[test]
    fn scrcpy_dying_before_the_advertisement_lapses_still_reconnects() {
        // Observed against the real phone: toggling Wireless debugging kills
        // the adb link at once, so scrcpy exits *before* the mDNS record has
        // had time to lapse. The exit therefore arrives first and must not be
        // read as the author closing the window.
        let mut link = MirrorLink::new();
        link.handle(found(AdbService::Connect, 39799));
        link.handle(MirrorEvent::MirrorRequested);
        link.handle(MirrorEvent::ConnectFinished {
            endpoint: Some(endpoint(39799)),
        });
        link.handle(MirrorEvent::MirrorStarted { pid: 4242 });

        link.handle(MirrorEvent::MirrorExited { failed: true });
        link.handle(MirrorEvent::ServiceLost {
            service: AdbService::Connect,
        });

        assert_eq!(
            link.handle(found(AdbService::Connect, 45461)),
            MirrorAction::Connect {
                endpoint: endpoint(45461)
            }
        );
    }

    #[test]
    fn a_stop_the_author_asked_for_is_not_reconnected() {
        let mut link = MirrorLink::new();
        link.handle(found(AdbService::Connect, 37059));
        link.handle(MirrorEvent::MirrorRequested);
        link.handle(MirrorEvent::ConnectFinished {
            endpoint: Some(endpoint(37059)),
        });
        link.handle(MirrorEvent::MirrorStarted { pid: 4242 });

        link.handle(MirrorEvent::StopRequested);
        link.handle(MirrorEvent::MirrorExited { failed: false });
        link.handle(MirrorEvent::ServiceLost {
            service: AdbService::Connect,
        });
        assert_eq!(
            link.handle(found(AdbService::Connect, 41887)),
            MirrorAction::None
        );
    }

    #[test]
    fn closing_the_window_is_a_decision_and_is_not_undone() {
        let mut link = MirrorLink::new();
        link.handle(found(AdbService::Connect, 37059));
        link.handle(MirrorEvent::MirrorRequested);
        link.handle(MirrorEvent::ConnectFinished {
            endpoint: Some(endpoint(37059)),
        });
        link.handle(MirrorEvent::MirrorStarted { pid: 4242 });

        link.handle(MirrorEvent::MirrorExited { failed: false });
        assert_eq!(link.state(), MirrorState::Available);
        assert_eq!(link.mirror_pid(), None);

        // A re-advertisement must not silently reopen a window the author closed.
        assert_eq!(
            link.handle(found(AdbService::Connect, 37059)),
            MirrorAction::None
        );
    }

    #[test]
    fn stopping_only_ever_targets_our_own_pid() {
        let mut link = MirrorLink::new();
        // Nothing of ours is running, so there is nothing to kill — a stop must
        // never become a kill-by-name of somebody else's scrcpy.
        assert_eq!(link.handle(MirrorEvent::StopRequested), MirrorAction::None);

        link.handle(found(AdbService::Connect, 37059));
        link.handle(MirrorEvent::MirrorRequested);
        link.handle(MirrorEvent::ConnectFinished {
            endpoint: Some(endpoint(37059)),
        });
        link.handle(MirrorEvent::MirrorStarted { pid: 4242 });
        assert_eq!(
            link.handle(MirrorEvent::StopRequested),
            MirrorAction::StopMirror
        );
    }

    #[test]
    fn a_code_needs_an_open_pairing_screen() {
        let mut link = MirrorLink::new();
        assert!(!link.can_pair());
        assert_eq!(
            link.handle(MirrorEvent::CodeEntered {
                code: "123456".to_owned()
            }),
            MirrorAction::None
        );
        assert_eq!(
            link.state(),
            MirrorState::Failed(MirrorError::NotAdvertised)
        );

        link.handle(found(AdbService::Pairing, 44311));
        assert!(link.can_pair());
        assert_eq!(
            link.handle(MirrorEvent::CodeEntered {
                code: "123456".to_owned()
            }),
            MirrorAction::Pair {
                endpoint: endpoint(44311),
                code: "123456".to_owned()
            }
        );
    }

    #[test]
    fn pairing_flows_straight_into_the_connection() {
        let mut link = MirrorLink::new();
        link.handle(found(AdbService::Pairing, 44311));
        link.handle(found(AdbService::Connect, 37059));
        link.handle(MirrorEvent::CodeEntered {
            code: "123456".to_owned(),
        });
        assert_eq!(link.state(), MirrorState::Pairing);
        assert_eq!(
            link.handle(MirrorEvent::PairFinished { paired: true }),
            MirrorAction::Connect {
                endpoint: endpoint(37059)
            }
        );
        assert!(!link.can_pair());
    }

    #[test]
    fn a_refused_code_says_so() {
        let mut link = MirrorLink::new();
        link.handle(found(AdbService::Pairing, 44311));
        link.handle(MirrorEvent::CodeEntered {
            code: "000000".to_owned(),
        });
        link.handle(MirrorEvent::PairFinished { paired: false });
        assert_eq!(link.state(), MirrorState::Failed(MirrorError::PairRejected));
    }

    #[test]
    fn no_graphical_session_is_stated_and_not_retried() {
        let mut link = MirrorLink::new();
        link.handle(found(AdbService::Connect, 39799));
        link.handle(MirrorEvent::MirrorRequested);
        link.handle(MirrorEvent::ConnectFinished {
            endpoint: Some(endpoint(39799)),
        });

        link.handle(MirrorEvent::DisplayMissing);
        assert_eq!(link.state(), MirrorState::Failed(MirrorError::NoDisplay));
        // The phone reappearing cannot fix a missing session, so it must not
        // start the spawn-and-die loop again.
        assert_eq!(
            link.handle(MirrorEvent::ServiceLost {
                service: AdbService::Connect
            }),
            MirrorAction::None
        );
        assert_eq!(
            link.handle(found(AdbService::Connect, 45461)),
            MirrorAction::None
        );
    }

    fn fixed(host_port: u16) -> MirrorEndpoint {
        MirrorEndpoint {
            host: "10.0.0.85".parse().unwrap(),
            port: host_port,
        }
    }

    #[test]
    fn a_remembered_fixed_port_is_a_way_in_when_nothing_is_advertised() {
        // The case this exists for: Android turned wireless debugging off, so
        // there is no advertisement at all — but `adb tcpip` left the fixed
        // port open, which was measured to survive exactly that.
        let mut link = MirrorLink::with_remembered(fixed(FIXED_PORT));
        assert_eq!(link.state(), MirrorState::Available);
        assert_eq!(
            link.handle(MirrorEvent::MirrorRequested),
            MirrorAction::Connect {
                endpoint: fixed(FIXED_PORT)
            }
        );
    }

    #[test]
    fn a_live_advertisement_wins_over_the_remembered_port() {
        // Discovery is the fresher truth: the phone is right there saying
        // where it is, so dial that rather than a port from last week.
        let mut link = MirrorLink::with_remembered(fixed(FIXED_PORT));
        link.handle(found(AdbService::Connect, 37059));
        assert_eq!(
            link.handle(MirrorEvent::MirrorRequested),
            MirrorAction::Connect {
                endpoint: endpoint(37059)
            }
        );
    }

    #[test]
    fn only_the_fixed_port_is_ever_remembered() {
        // Remembering a discovered port would rebuild the stale-cache defect
        // the author's own script suffered from.
        let mut link = MirrorLink::new();
        link.handle(found(AdbService::Connect, 37059));
        link.handle(MirrorEvent::MirrorRequested);
        link.handle(MirrorEvent::ConnectFinished {
            endpoint: Some(endpoint(37059)),
        });
        assert_eq!(link.remembered(), None);

        link.handle(MirrorEvent::ConnectFinished {
            endpoint: Some(fixed(FIXED_PORT)),
        });
        assert_eq!(link.remembered(), Some(fixed(FIXED_PORT)));
    }

    #[test]
    fn a_remembered_discovery_port_is_refused_at_construction() {
        assert_eq!(
            MirrorLink::with_remembered(endpoint(37059)).remembered(),
            None
        );
        assert_eq!(
            MirrorLink::with_remembered(endpoint(37059)).state(),
            MirrorState::Idle
        );
    }

    #[test]
    fn the_mirror_starts_on_the_endpoint_actually_reached() {
        // Pinning the fixed port restarts adbd and moves the device onto it,
        // so the serial scrcpy is given must be the one that answered, not the
        // one dialled.
        let mut link = MirrorLink::new();
        link.handle(found(AdbService::Connect, 37059));
        link.handle(MirrorEvent::MirrorRequested);
        assert_eq!(
            link.handle(MirrorEvent::ConnectFinished {
                endpoint: Some(fixed(FIXED_PORT))
            }),
            MirrorAction::StartMirror {
                serial: format!("10.0.0.85:{FIXED_PORT}")
            }
        );
    }

    #[test]
    fn losing_the_advertisement_does_not_forget_the_fixed_port() {
        let mut link = MirrorLink::with_remembered(fixed(FIXED_PORT));
        link.handle(found(AdbService::Connect, 37059));
        link.handle(MirrorEvent::ServiceLost {
            service: AdbService::Connect,
        });
        assert_eq!(link.remembered(), Some(fixed(FIXED_PORT)));
        // And it is still a way in.
        assert_eq!(link.state(), MirrorState::Available);
    }

    #[test]
    fn a_stale_advertisement_falls_back_to_the_remembered_port() {
        // Measured against the real phone: Android turned wireless debugging
        // off, but Avahi still served the advertisement from its cache for
        // another minute. Dialling that dead port and giving up left the mirror
        // failed while the fixed port was answering the whole time.
        let mut link = MirrorLink::with_remembered(fixed(FIXED_PORT));
        link.handle(found(AdbService::Connect, 42293));
        assert_eq!(
            link.handle(MirrorEvent::MirrorRequested),
            MirrorAction::Connect {
                endpoint: endpoint(42293)
            }
        );
        // The stale port is dead; the remembered one is the way in.
        assert_eq!(
            link.handle(MirrorEvent::ConnectFinished { endpoint: None }),
            MirrorAction::Connect {
                endpoint: fixed(FIXED_PORT)
            }
        );
        assert_eq!(link.state(), MirrorState::Connecting);
    }

    #[test]
    fn the_fallback_is_tried_once_and_then_the_failure_is_real() {
        let mut link = MirrorLink::with_remembered(fixed(FIXED_PORT));
        link.handle(found(AdbService::Connect, 42293));
        link.handle(MirrorEvent::MirrorRequested);
        link.handle(MirrorEvent::ConnectFinished { endpoint: None });
        // The fixed port failed too: stop, rather than dial in a circle.
        assert_eq!(
            link.handle(MirrorEvent::ConnectFinished { endpoint: None }),
            MirrorAction::None
        );
        assert_eq!(
            link.state(),
            MirrorState::Failed(MirrorError::ConnectFailed)
        );
    }

    #[test]
    fn dialling_the_remembered_port_first_has_no_fallback_to_make() {
        let mut link = MirrorLink::with_remembered(fixed(FIXED_PORT));
        link.handle(MirrorEvent::MirrorRequested);
        assert_eq!(
            link.handle(MirrorEvent::ConnectFinished { endpoint: None }),
            MirrorAction::None
        );
        assert_eq!(
            link.state(),
            MirrorState::Failed(MirrorError::ConnectFailed)
        );
    }

    #[test]
    fn service_types_round_trip() {
        for service in [AdbService::Connect, AdbService::Pairing] {
            assert_eq!(
                AdbService::from_service_type(service.service_type()),
                Some(service)
            );
        }
        assert_eq!(AdbService::from_service_type("_workstation._tcp"), None);
    }
}
