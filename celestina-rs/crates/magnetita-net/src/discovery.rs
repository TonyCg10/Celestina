//! Discovery — hearing the phone announce itself, and announcing back.
//!
//! KDE Connect finds devices over UDP: a device broadcasts its [`Identity`] to
//! the whole subnet on port 1716, and whoever hears it learns who is there and
//! the TCP port to dial back on. So this side does both — it listens for those
//! broadcasts and periodically sends its own, so the phone lists Magnetita the
//! same way Magnetita lists the phone.
//!
//! A broadcast is *only* an announcement: it carries no secret and proves
//! nothing, so all it yields is an [`Announcement`] — who claims to be there and
//! where to reach them. The real link, its TLS and its trust come after, when we
//! dial [`link_addr`]. We ignore our own echo (same device id) so a broadcast we
//! sent does not read back as a peer.
//!
//! [`Identity`]: magnetita_core::Identity
//! [`link_addr`]: Announcement::link_addr

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;

use magnetita_core::{Identity, NetworkPacket, DEFAULT_PORT};

/// The port KDE Connect broadcasts and listens on.
pub const DISCOVERY_PORT: u16 = DEFAULT_PORT;

/// The subnet broadcast address our announce goes to.
pub const BROADCAST: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), DISCOVERY_PORT);

/// How often to re-announce so a phone that opens its app mid-session still finds
/// us. KDE Connect announces on a similar cadence.
pub const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(5);

/// A device heard on the network: who it claims to be, and the address the
/// datagram came from.
#[derive(Clone, Debug)]
pub struct Announcement {
    pub identity: Identity,
    pub source: SocketAddr,
}

impl Announcement {
    /// Where to open the TCP link: the announcer's own IP (where the datagram
    /// came from) with the `tcpPort` it named. `None` if it named no port, which
    /// a real KDE Connect announce always does.
    pub fn link_addr(&self) -> Option<SocketAddr> {
        Some(SocketAddr::new(self.source.ip(), self.identity.tcp_port?))
    }
}

/// Reads a received datagram into an [`Announcement`], returning `None` for our
/// own echo (same device id), a non-identity packet, or bytes that do not parse
/// — none of which is a peer to act on.
pub fn parse_announcement(
    datagram: &[u8],
    from: SocketAddr,
    our_device_id: &str,
) -> Option<Announcement> {
    let text = std::str::from_utf8(datagram).ok()?;
    let packet = NetworkPacket::parse(text).ok()?;
    let identity = Identity::from_packet(&packet)?;
    if identity.device_id == our_device_id {
        return None;
    }
    Some(Announcement {
        identity,
        source: from,
    })
}

/// The UDP socket that hears announces and sends ours. Blocking; the daemon runs
/// [`recv`](Discovery::recv) on one thread and announces on another (each holds a
/// [`try_clone`](Discovery::try_clone) of the socket).
pub struct Discovery {
    socket: UdpSocket,
    our_device_id: String,
}

impl Discovery {
    /// Bind for discovery. In production `addr` is `0.0.0.0:1716` to catch subnet
    /// broadcasts; tests bind `127.0.0.1:0`. Broadcast sending is enabled so our
    /// announce can reach the whole subnet.
    pub fn bind(addr: SocketAddr, our_device_id: impl Into<String>) -> io::Result<Discovery> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_broadcast(true)?;
        Ok(Discovery {
            socket,
            our_device_id: our_device_id.into(),
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Bound the blocking [`recv`](Discovery::recv) so a listen loop can wake to
    /// re-announce or shut down instead of blocking forever.
    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.socket.set_read_timeout(dur)
    }

    /// A second handle on the same socket, for running listen and announce on
    /// separate threads.
    pub fn try_clone(&self) -> io::Result<Discovery> {
        Ok(Discovery {
            socket: self.socket.try_clone()?,
            our_device_id: self.our_device_id.clone(),
        })
    }

    /// Block for the next datagram and parse it. `Ok(None)` means something
    /// arrived but was our own echo or not an identity — the caller loops.
    pub fn recv(&self) -> io::Result<Option<Announcement>> {
        let mut buf = [0u8; 8192];
        let (n, from) = self.socket.recv_from(&mut buf)?;
        Ok(parse_announcement(&buf[..n], from, &self.our_device_id))
    }

    /// Broadcast our identity to the subnet so phones list us.
    pub fn announce(&self, identity: &Identity, id: i64) -> io::Result<()> {
        self.send(identity, id, BROADCAST)
    }

    /// Send our identity to one address — a unicast announce, and what tests use.
    pub fn send(&self, identity: &Identity, id: i64, to: SocketAddr) -> io::Result<()> {
        let line = format!("{}\n", identity.to_packet(id).to_line());
        self.socket.send_to(line.as_bytes(), to)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_announcement, Discovery};
    use magnetita_core::Identity;
    use std::net::{Ipv4Addr, SocketAddr};

    fn phone_datagram() -> Vec<u8> {
        let raw = r#"{"id":1700000000000,"type":"kdeconnect.identity","body":{
            "deviceId":"689da02afffe4b1282577c0a2f0ed5e3","deviceName":"Galaxy S25 Ultra",
            "deviceType":"phone","protocolVersion":8,"tcpPort":1716,
            "incomingCapabilities":[],"outgoingCapabilities":["kdeconnect.ping"]}}"#;
        format!("{raw}\n").into_bytes()
    }

    fn from(port: u16) -> SocketAddr {
        SocketAddr::new(Ipv4Addr::new(10, 0, 0, 85).into(), port)
    }

    #[test]
    fn a_phone_announce_becomes_a_dialable_link_address() {
        let ann = parse_announcement(&phone_datagram(), from(43210), "us").unwrap();
        assert_eq!(ann.identity.device_name, "Galaxy S25 Ultra");
        // Dial the announcer's IP on the tcpPort it named, not the source port.
        assert_eq!(
            ann.link_addr().unwrap(),
            SocketAddr::new(Ipv4Addr::new(10, 0, 0, 85).into(), 1716)
        );
    }

    #[test]
    fn our_own_echo_is_ignored() {
        // Same device id as ours → not a peer.
        assert!(parse_announcement(
            &phone_datagram(),
            from(43210),
            "689da02afffe4b1282577c0a2f0ed5e3"
        )
        .is_none());
    }

    #[test]
    fn garbage_and_non_identity_yield_nothing() {
        assert!(parse_announcement(b"not a packet", from(1), "us").is_none());
        let ping = magnetita_core::ping_packet(1).to_line();
        assert!(parse_announcement(ping.as_bytes(), from(1), "us").is_none());
    }

    #[test]
    fn an_announce_with_no_tcp_port_has_no_link_address() {
        let raw = br#"{"id":1,"type":"kdeconnect.identity","body":{
            "deviceId":"x","deviceName":"No Port","protocolVersion":8}}"#;
        let ann = parse_announcement(raw, from(1), "us").unwrap();
        assert!(ann.link_addr().is_none());
    }

    #[test]
    fn a_real_datagram_travels_loopback_and_parses() {
        let ours = Discovery::bind("127.0.0.1:0".parse().unwrap(), "us-desktop").unwrap();
        let phone = Discovery::bind("127.0.0.1:0".parse().unwrap(), "the-phone").unwrap();

        let phone_identity = Identity::desktop("the-phone", "Pretend Phone");
        phone
            .send(&phone_identity, 1, ours.local_addr().unwrap())
            .unwrap();

        let heard = ours.recv().unwrap().expect("a peer, not our echo");
        assert_eq!(heard.identity.device_id, "the-phone");
        assert_eq!(heard.source.ip(), Ipv4Addr::LOCALHOST);
    }
}
