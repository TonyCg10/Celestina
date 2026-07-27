//! The device runtime — the pure [`Session`] driven over a live [`Link`].
//!
//! [`magnetita-core`](magnetita_core) decides *what* a packet means; the [`Link`]
//! carries bytes. This is the thin band between them: it reads a packet, hands it
//! to the session, and puts the session's decisions — a pair reply, a ping — back
//! on the wire, turning each into a [`NetworkPacket`] with a stamped id. It also
//! runs the one clock the pure core refuses to: the ~30 s pairing timer, fired
//! from [`pump`](Device::pump) when its deadline passes.
//!
//! Everything observable comes out as [`ConnectionEvent`]s, so the app's log is
//! just the stream of what happened. The loop is the caller's: call
//! [`pump`](Device::pump) to advance, and the command methods
//! ([`request_pairing`](Device::request_pairing) and friends) to act on the
//! user's behalf. One [`Device`] is one connection.
//!
//! [`Session`]: magnetita_core::Session
//! [`NetworkPacket`]: magnetita_core::NetworkPacket

use std::io;
use std::time::{Duration, Instant};

use std::net::SocketAddr;

use magnetita_core::{
    pair_packet, ping_packet, ConnectionEvent, Identity, NetworkPacket, Outgoing, Reaction,
    Session, TIMEOUT_SECS,
};

use crate::link::{Link, LinkError};

/// One turn of the loop: the events it produced, whether the link is still open
/// (`false` once the peer has cleanly closed), and the raw packet read this turn.
#[derive(Clone, Debug)]
pub struct Pump {
    pub events: Vec<ConnectionEvent>,
    pub open: bool,
    /// The packet read this turn, if any. Pairing and ping are already handled
    /// into `events`; this is here so the daemon can act on plugin packets the
    /// pure session leaves untouched (sftp, battery, …).
    pub packet: Option<NetworkPacket>,
}

/// A live connection to one device: its [`Link`], its [`Session`], and the
/// pairing clock.
pub struct Device {
    link: Link,
    session: Session,
    next_id: i64,
    pairing_deadline: Option<Instant>,
}

impl Device {
    /// Wrap an established link and a session. `id_base` seeds the packet ids we
    /// stamp (a millisecond clock in production); `tick` bounds how long
    /// [`pump`](Device::pump) blocks so the pairing timer and any caller command
    /// are never more than `tick` from being serviced.
    pub fn new(link: Link, session: Session, id_base: i64, tick: Duration) -> io::Result<Device> {
        link.set_read_timeout(Some(tick))?;
        Ok(Device {
            link,
            session,
            next_id: id_base,
            pairing_deadline: None,
        })
    }

    /// The peer's trusted identity.
    pub fn peer(&self) -> &Identity {
        self.link.peer()
    }

    /// The peer certificate's fingerprint — what the trust store pins.
    pub fn peer_fingerprint(&self) -> &str {
        self.link.peer_fingerprint()
    }

    /// The address of the peer — its IP is where the phone's sftp server (and
    /// every other plugin service) lives.
    pub fn peer_addr(&self) -> SocketAddr {
        self.link.peer_addr()
    }

    pub fn is_paired(&self) -> bool {
        self.session.is_paired()
    }

    /// The peer asked to pair and awaits our answer — a headless caller can call
    /// [`accept_pairing`](Device::accept_pairing) on this, a UI can prompt.
    pub fn peer_wants_to_pair(&self) -> bool {
        self.session.peer_wants_to_pair()
    }

    /// Advance one turn: fire the pairing timeout if due, then read and process
    /// one packet. A read that hits the tick bound is an idle turn, not a close.
    pub fn pump(&mut self) -> Result<Pump, LinkError> {
        let mut events = Vec::new();

        if let Some(deadline) = self.pairing_deadline {
            if Instant::now() >= deadline {
                let reaction = self.session.pairing_timeout();
                events.extend(self.dispatch(reaction)?);
            }
        }

        match self.link.read_packet() {
            Ok(Some(packet)) => {
                let reaction = self.session.handle(&packet);
                events.extend(self.dispatch(reaction)?);
                Ok(Pump {
                    events,
                    open: true,
                    packet: Some(packet),
                })
            }
            Ok(None) => Ok(Pump {
                events,
                open: false,
                packet: None,
            }),
            Err(LinkError::Io(e)) if is_idle_timeout(&e) => Ok(Pump {
                events,
                open: true,
                packet: None,
            }),
            Err(e) => Err(e),
        }
    }

    /// Send a plugin packet the pure session does not own — the daemon builds it
    /// (e.g. an sftp request) and we stamp it with the next id and put it on the
    /// wire. Pairing and ping have their own methods; this is for everything else.
    pub fn send(&mut self, make: impl FnOnce(i64) -> NetworkPacket) -> Result<(), LinkError> {
        let id = self.next_id();
        self.link.send_packet(&make(id))
    }

    /// Ask the peer to pair (the user pressed "pair" on our side).
    pub fn request_pairing(&mut self) -> Result<Vec<ConnectionEvent>, LinkError> {
        let reaction = self.session.request_pairing();
        self.dispatch(reaction)
    }

    /// Accept a pairing request the peer made.
    pub fn accept_pairing(&mut self) -> Result<Vec<ConnectionEvent>, LinkError> {
        let reaction = self.session.accept_pairing();
        self.dispatch(reaction)
    }

    /// Reject a pairing request the peer made.
    pub fn reject_pairing(&mut self) -> Result<Vec<ConnectionEvent>, LinkError> {
        let reaction = self.session.reject_pairing();
        self.dispatch(reaction)
    }

    /// Drop an established pairing.
    pub fn unpair(&mut self) -> Result<Vec<ConnectionEvent>, LinkError> {
        let reaction = self.session.unpair();
        self.dispatch(reaction)
    }

    /// Send a ping — the CP0 liveness poke.
    pub fn send_ping(&mut self) -> Result<Vec<ConnectionEvent>, LinkError> {
        let reaction = self.session.send_ping();
        self.dispatch(reaction)
    }

    /// Put a reaction's packets on the wire and fold its events into the pairing
    /// clock, returning them for the log.
    fn dispatch(&mut self, reaction: Reaction) -> Result<Vec<ConnectionEvent>, LinkError> {
        for out in &reaction.send {
            let id = self.next_id();
            let packet = match out {
                Outgoing::Pair(pair) => pair_packet(id, *pair),
                Outgoing::Ping => ping_packet(id),
            };
            self.link.send_packet(&packet)?;
        }
        self.note_events(&reaction.events);
        Ok(reaction.events)
    }

    /// Keep the pairing deadline in step with what just happened: a pairing
    /// exchange starts the ~30 s clock; a resolution (paired, unpaired, or any
    /// loss) stops it.
    fn note_events(&mut self, events: &[ConnectionEvent]) {
        for event in events {
            match event {
                ConnectionEvent::Pairing => {
                    self.pairing_deadline =
                        Some(Instant::now() + Duration::from_secs(TIMEOUT_SECS));
                }
                ConnectionEvent::Paired | ConnectionEvent::Unpaired | ConnectionEvent::Lost(_) => {
                    self.pairing_deadline = None;
                }
                _ => {}
            }
        }
    }

    fn next_id(&mut self) -> i64 {
        self.next_id += 1;
        self.next_id
    }
}

/// A read that hit the tick bound rather than a real socket failure — treated as
/// an idle turn.
fn is_idle_timeout(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

#[cfg(test)]
mod tests {
    use super::Device;
    use crate::cert::DeviceCert;
    use crate::discovery::Announcement;
    use crate::link::Link;
    use crate::tls::TlsConfigs;
    use magnetita_core::{ConnectionEvent, Identity, Session};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    /// A desktop Device (dialed) and a phone Device (accepted), linked over
    /// loopback and ready to drive a pairing.
    fn linked_pair() -> (Device, Device) {
        let phone_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let desk_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let phone_tls = TlsConfigs::build(&DeviceCert::generate(phone_id)).unwrap();
        let desk_tls = TlsConfigs::build(&DeviceCert::generate(desk_id)).unwrap();
        let phone_identity = Identity::desktop(phone_id, "Pretend Phone");
        let desk_identity = Identity::desktop(desk_id, "Celestina");

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let phone_accept = {
            let phone_identity = phone_identity.clone();
            thread::spawn(move || {
                let (tcp, _) = listener.accept().unwrap();
                Link::accept(
                    tcp,
                    &phone_identity,
                    &phone_tls,
                    &mut {
                        let mut n = 0;
                        move || {
                            n += 1;
                            n
                        }
                    },
                    Duration::from_secs(5),
                )
                .unwrap()
            })
        };

        let mut announced = phone_identity;
        announced.tcp_port = Some(addr.port());
        let announcement = Announcement {
            identity: announced,
            source: addr,
        };
        let desk_link = Link::connect(
            &announcement,
            &desk_identity,
            &desk_tls,
            &mut {
                let mut n = 0;
                move || {
                    n += 1;
                    n
                }
            },
            Duration::from_secs(5),
        )
        .unwrap();
        let phone_link = phone_accept.join().unwrap();

        let tick = Duration::from_secs(2);
        let desk = Device::new(desk_link, Session::new(), 0, tick).unwrap();
        let phone = Device::new(phone_link, Session::new(), 1000, tick).unwrap();
        (desk, phone)
    }

    #[test]
    fn a_full_pairing_and_ping_run_over_two_devices() {
        let (mut desk, mut phone) = linked_pair();

        // The desktop user asks to pair.
        let ev = desk.request_pairing().unwrap();
        assert_eq!(ev, vec![ConnectionEvent::Pairing]);
        assert!(!desk.is_paired());

        // The phone hears the request and must prompt its user: this is an
        // incoming request, distinct from an outgoing one that looks the same.
        let pumped = phone.pump().unwrap();
        assert!(pumped.open);
        assert_eq!(pumped.events, vec![ConnectionEvent::Pairing]);
        assert!(!phone.is_paired());
        assert!(phone.peer_wants_to_pair());

        // The phone user accepts; the phone is paired and answers.
        let ev = phone.accept_pairing().unwrap();
        assert_eq!(ev, vec![ConnectionEvent::Paired]);
        assert!(phone.is_paired());
        assert!(!phone.peer_wants_to_pair());

        // The desktop hears the acceptance and is paired too.
        let pumped = desk.pump().unwrap();
        assert_eq!(pumped.events, vec![ConnectionEvent::Paired]);
        assert!(desk.is_paired());

        // A ping crosses and is noted.
        desk.send_ping().unwrap();
        let pumped = phone.pump().unwrap();
        assert_eq!(pumped.events, vec![ConnectionEvent::Pinged]);
    }

    #[test]
    fn an_idle_pump_is_open_with_no_events() {
        let (mut desk, _phone) = linked_pair();
        // Nothing was sent, so the read hits the tick bound: an idle, open turn.
        let pumped = desk.pump().unwrap();
        assert!(pumped.open);
        assert!(pumped.events.is_empty());
        assert!(pumped.packet.is_none());
    }

    #[test]
    fn a_plugin_packet_sends_and_arrives_raw_for_the_daemon() {
        let (mut desk, mut phone) = linked_pair();
        // The desktop sends an sftp request — a plugin packet the session does
        // not own.
        desk.send(magnetita_core::request_packet).unwrap();
        let pumped = phone.pump().unwrap();
        assert!(pumped.open);
        // The pure session produced nothing from it...
        assert!(pumped.events.is_empty());
        // ...but the raw packet is surfaced for the daemon to act on.
        let packet = pumped.packet.expect("the raw packet is surfaced");
        assert!(packet.is(magnetita_core::TYPE_SFTP_REQUEST));
    }
}
