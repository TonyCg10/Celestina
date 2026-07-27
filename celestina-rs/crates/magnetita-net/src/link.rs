//! The link — one TCP+TLS connection to a device, carrying framed packets.
//!
//! This is the KDE Connect handshake made exact. The device that heard the UDP
//! announce dials, and — this is the part that is easy to get backwards — the
//! **dialer is the TLS server**; the device that accepts the connection is the
//! TLS client. The v8 sequence, both roles:
//!
//! 1. The connector writes its [`Identity`] **in the clear**, tagged with the
//!    peer's `targetDeviceId` and `targetProtocolVersion`, then starts TLS as
//!    the **server**. The acceptor reads that plaintext line, then starts TLS as
//!    the **client**.
//! 2. Once the channel is encrypted, each side reads the other's certificate and
//!    pins it (the fingerprint the [`TrustStore`] checks).
//! 3. For protocol ≥ 8 the identities are re-sent **encrypted** and re-read, so
//!    the trusted name is one nobody could have forged before TLS. (For < 8 the
//!    plaintext identity stands.)
//!
//! After that a [`Link`] is just a line-delimited packet stream over TLS:
//! [`read_packet`] and [`send_packet`]. Blocking, one connection; the session
//! logic and the pairing timer live above it.
//!
//! [`Identity`]: magnetita_core::Identity
//! [`TrustStore`]: crate::trust::TrustStore
//! [`read_packet`]: Link::read_packet
//! [`send_packet`]: Link::send_packet

use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use rustls::pki_types::ServerName;
use rustls::{ClientConnection, ServerConnection, StreamOwned};
use serde_json::Value;

use magnetita_core::{Identity, NetworkPacket};

use crate::discovery::Announcement;
use crate::tls::{peer_leaf_fingerprint, TlsConfigs};

/// A blocking TLS transport — either role's connection, boxed so a [`Link`] is
/// one type regardless of whether we dialed or accepted.
trait Transport: Read + Write + Send {}
impl<T: Read + Write + Send> Transport for T {}

/// Read + Write together, so the identity exchange can run over a borrowed
/// [`rustls::Stream`] of either role without naming its type.
trait ReadWrite: Read + Write {}
impl<T: Read + Write + ?Sized> ReadWrite for T {}

/// An established, encrypted, trusted-by-fingerprint link to one device.
pub struct Link {
    reader: BufReader<Box<dyn Transport>>,
    /// Bytes of a packet line read so far but not yet newline-terminated —
    /// preserved across a read timeout so a line split by an idle tick is
    /// never dropped mid-frame.
    pending: Vec<u8>,
    /// A second handle on the same socket, held only to change the read timeout
    /// after the stream is boxed — so a caller's read loop can wake on idle.
    sock: TcpStream,
    peer: Identity,
    peer_fingerprint: String,
    peer_addr: SocketAddr,
}

impl Link {
    /// Dial a device we heard announce, as the **connector** (TLS server). Sends
    /// the plaintext identity with the peer-targeting fields, runs the TLS
    /// handshake, then the v8 encrypted identity exchange. `next_id` stamps the
    /// packets we send (a millisecond clock in production, a counter in tests).
    pub fn connect(
        announcement: &Announcement,
        ours: &Identity,
        tls: &TlsConfigs,
        next_id: &mut dyn FnMut() -> i64,
        timeout: Duration,
    ) -> Result<Link, LinkError> {
        let addr = announcement.link_addr().ok_or(LinkError::NoLinkAddress)?;
        let peer_proto = announcement.identity.protocol_version;

        let mut tcp = TcpStream::connect_timeout(&addr, timeout)?;
        tcp.set_nodelay(true).ok();
        tcp.set_read_timeout(Some(timeout))?;
        tcp.set_write_timeout(Some(timeout))?;

        // 1. Our identity in the clear, telling the phone we mean it specifically.
        let plaintext = plaintext_identity_line(
            ours,
            &announcement.identity.device_id,
            peer_proto,
            next_id(),
        );
        tcp.write_all(plaintext.as_bytes())?;
        tcp.flush()?;

        // 2. We dialed, so we are the TLS server.
        let mut conn = ServerConnection::new(tls.server_config()).map_err(LinkError::Tls)?;
        conn.complete_io(&mut tcp)?;
        if conn.is_handshaking() {
            return Err(LinkError::HandshakeIncomplete);
        }

        // 3. Pin the certificate the peer just proved it owns.
        let fingerprint =
            peer_leaf_fingerprint(conn.peer_certificates()).ok_or(LinkError::NoPeerCertificate)?;

        // 4. v8: re-exchange identities encrypted; else keep the announce's.
        let peer = {
            let mut stream = rustls::Stream::new(&mut conn, &mut tcp);
            exchange_identity(
                &mut stream,
                ours,
                next_id,
                peer_proto,
                announcement.identity.clone(),
            )?
        };

        tcp.set_read_timeout(None).ok();
        let sock = tcp.try_clone()?;
        Ok(Link {
            reader: BufReader::new(Box::new(StreamOwned::new(conn, tcp))),
            pending: Vec::new(),
            sock,
            peer,
            peer_fingerprint: fingerprint,
            peer_addr: addr,
        })
    }

    /// Take a connection a device opened to us, as the **acceptor** (TLS client).
    /// Reads the peer's plaintext identity, runs the TLS handshake as the client,
    /// then the v8 encrypted exchange.
    pub fn accept(
        mut tcp: TcpStream,
        ours: &Identity,
        tls: &TlsConfigs,
        next_id: &mut dyn FnMut() -> i64,
        timeout: Duration,
    ) -> Result<Link, LinkError> {
        let peer_addr = tcp.peer_addr()?;
        tcp.set_nodelay(true).ok();
        tcp.set_read_timeout(Some(timeout))?;
        tcp.set_write_timeout(Some(timeout))?;

        // 1. The peer's plaintext identity. Read exactly one line, byte by byte,
        //    so we do not swallow the TLS ClientHello we are about to send.
        let line = read_delimited_line(&mut tcp)?;
        let packet = NetworkPacket::parse(&line).map_err(LinkError::Parse)?;
        let pre = Identity::from_packet(&packet).ok_or(LinkError::PeerIdentityMissing)?;
        let peer_proto = pre.protocol_version;

        // 2. They dialed, so we are the TLS client.
        let server_name = ServerName::try_from(pre.device_id.clone())
            .unwrap_or_else(|_| ServerName::try_from("kdeconnect").unwrap());
        let mut conn =
            ClientConnection::new(tls.client_config(), server_name).map_err(LinkError::Tls)?;
        conn.complete_io(&mut tcp)?;
        if conn.is_handshaking() {
            return Err(LinkError::HandshakeIncomplete);
        }

        // 3. Pin the peer certificate.
        let fingerprint =
            peer_leaf_fingerprint(conn.peer_certificates()).ok_or(LinkError::NoPeerCertificate)?;

        // 4. v8 encrypted exchange (or keep the plaintext identity for < 8).
        let peer = {
            let mut stream = rustls::Stream::new(&mut conn, &mut tcp);
            exchange_identity(&mut stream, ours, next_id, peer_proto, pre)?
        };

        tcp.set_read_timeout(None).ok();
        let sock = tcp.try_clone()?;
        Ok(Link {
            reader: BufReader::new(Box::new(StreamOwned::new(conn, tcp))),
            pending: Vec::new(),
            sock,
            peer,
            peer_fingerprint: fingerprint,
            peer_addr,
        })
    }

    /// The peer's identity, as trusted after the handshake.
    pub fn peer(&self) -> &Identity {
        &self.peer
    }

    /// The peer certificate's fingerprint — what the trust store pins.
    pub fn peer_fingerprint(&self) -> &str {
        &self.peer_fingerprint
    }

    /// The address the link is to.
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    /// Bound how long [`read_packet`](Link::read_packet) blocks, so a caller's
    /// loop can wake on idle to check for a due pairing timeout or a queued
    /// command. `None` blocks until a packet or a close. A read that hits the
    /// bound surfaces as an [`io::ErrorKind::WouldBlock`]/`TimedOut` error, which
    /// the caller treats as "nothing this tick", not a disconnect.
    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.sock.set_read_timeout(dur)
    }

    /// Read the next packet, blocking. `Ok(None)` is a clean close by the peer.
    /// Blank keep-alive lines are skipped rather than reported as a close.
    /// A line may not exceed [`MAX_PACKET_LINE`], and one cut short by a read
    /// timeout is kept for the next call rather than dropped mid-frame.
    pub fn read_packet(&mut self) -> Result<Option<NetworkPacket>, LinkError> {
        loop {
            if !fill_line(&mut self.reader, &mut self.pending)? {
                return Ok(None);
            }
            let line = std::mem::take(&mut self.pending);
            let text = String::from_utf8_lossy(&line);
            let trimmed = text.trim();
            if trimmed.is_empty() {
                continue;
            }
            return NetworkPacket::parse(trimmed)
                .map(Some)
                .map_err(LinkError::Parse);
        }
    }

    /// Send a packet, line-delimited and flushed.
    pub fn send_packet(&mut self, packet: &NetworkPacket) -> Result<(), LinkError> {
        let line = format!("{}\n", packet.to_line());
        let w = self.reader.get_mut();
        w.write_all(line.as_bytes())?;
        w.flush()?;
        Ok(())
    }
}

/// Longest packet line [`read_packet`](Link::read_packet) accepts. Real packets
/// top out at a few KiB (notification and mpris metadata are the largest); the
/// bound only exists so a connected peer cannot grow one unterminated line —
/// and our memory with it — without limit.
const MAX_PACKET_LINE: usize = 512 * 1024;

/// Reads one bounded, `\n`-terminated line into `pending`, keeping whatever a
/// timed-out read already consumed. `Ok(true)` means a full line (newline
/// included) sits in `pending`; `Ok(false)` means the peer closed.
fn fill_line<R: BufRead>(reader: &mut R, pending: &mut Vec<u8>) -> Result<bool, LinkError> {
    while pending.last() != Some(&b'\n') {
        let budget = (MAX_PACKET_LINE + 1).saturating_sub(pending.len());
        if budget == 0 {
            pending.clear();
            return Err(LinkError::LineTooLong);
        }
        if reader
            .by_ref()
            .take(budget as u64)
            .read_until(b'\n', pending)?
            == 0
        {
            // EOF — a partial line without its newline died with the peer.
            return Ok(false);
        }
    }
    Ok(true)
}

/// The plaintext identity line the connector sends first: our identity plus the
/// `targetDeviceId`/`targetProtocolVersion` a v8 peer checks to know the dial is
/// meant for it.
fn plaintext_identity_line(ours: &Identity, target_id: &str, target_proto: i32, id: i64) -> String {
    let mut packet = ours.to_packet(id);
    if let Value::Object(map) = &mut packet.body {
        map.insert(
            "targetDeviceId".to_owned(),
            Value::String(target_id.to_owned()),
        );
        map.insert(
            "targetProtocolVersion".to_owned(),
            Value::from(target_proto),
        );
    }
    format!("{}\n", packet.to_line())
}

/// The post-TLS identity step. For protocol ≥ 8 we send our identity encrypted
/// and read the peer's encrypted identity, which becomes the trusted one; below
/// 8 the pre-TLS identity stands.
fn exchange_identity(
    stream: &mut dyn ReadWrite,
    ours: &Identity,
    next_id: &mut dyn FnMut() -> i64,
    peer_proto: i32,
    pre_tls_peer: Identity,
) -> Result<Identity, LinkError> {
    if peer_proto < 8 {
        return Ok(pre_tls_peer);
    }
    let line = format!("{}\n", ours.to_packet(next_id()).to_line());
    stream.write_all(line.as_bytes())?;
    stream.flush()?;

    let reply = read_delimited_line(stream)?;
    let packet = NetworkPacket::parse(&reply).map_err(LinkError::Parse)?;
    Identity::from_packet(&packet).ok_or(LinkError::PeerIdentityMissing)
}

/// Reads one `\n`-terminated line one byte at a time, so nothing past the line
/// is buffered where a later reader (or the TLS layer) cannot see it. Used for
/// the two identity reads, not the hot packet loop.
fn read_delimited_line<R: Read + ?Sized>(r: &mut R) -> Result<String, LinkError> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match r.read(&mut byte)? {
            0 => break,
            _ => {
                if byte[0] == b'\n' {
                    break;
                }
                buf.push(byte[0]);
                if buf.len() > 128 * 1024 {
                    return Err(LinkError::PeerIdentityMissing);
                }
            }
        }
    }
    if buf.is_empty() {
        return Err(LinkError::PeerIdentityMissing);
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// What can go wrong opening or reading a link.
#[derive(Debug)]
pub enum LinkError {
    /// The announcement named no TCP port, so there is nowhere to dial.
    NoLinkAddress,
    /// A socket or TLS-transport I/O error, including a handshake timeout.
    Io(io::Error),
    /// The TLS layer rejected the configuration or handshake.
    Tls(rustls::Error),
    /// A line off the wire was not a valid packet.
    Parse(serde_json::Error),
    /// The handshake ended with the connection still handshaking — the peer went
    /// away mid-handshake.
    HandshakeIncomplete,
    /// Mutual TLS completed but the peer presented no certificate to pin.
    NoPeerCertificate,
    /// The peer sent no usable identity where one was required.
    PeerIdentityMissing,
    /// A single packet line exceeded [`MAX_PACKET_LINE`] — reading on would
    /// grow memory without bound, so the link is dropped instead.
    LineTooLong,
}

impl From<io::Error> for LinkError {
    fn from(e: io::Error) -> Self {
        LinkError::Io(e)
    }
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::NoLinkAddress => write!(f, "the announcement named no TCP port to dial"),
            LinkError::Io(e) => write!(f, "link I/O error: {e}"),
            LinkError::Tls(e) => write!(f, "TLS error: {e}"),
            LinkError::Parse(e) => write!(f, "malformed packet: {e}"),
            LinkError::HandshakeIncomplete => write!(f, "the peer closed during the handshake"),
            LinkError::NoPeerCertificate => write!(f, "the peer presented no certificate"),
            LinkError::PeerIdentityMissing => write!(f, "the peer sent no usable identity"),
            LinkError::LineTooLong => {
                write!(f, "a packet line exceeded {MAX_PACKET_LINE} bytes")
            }
        }
    }
}

impl std::error::Error for LinkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LinkError::Io(e) => Some(e),
            LinkError::Tls(e) => Some(e),
            LinkError::Parse(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Link;
    use crate::cert::DeviceCert;
    use crate::discovery::Announcement;
    use crate::tls::TlsConfigs;
    use magnetita_core::{ping_packet, Identity};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    fn counter() -> impl FnMut() -> i64 {
        let mut n = 0;
        move || {
            n += 1;
            n
        }
    }

    #[test]
    fn two_links_complete_the_v8_handshake_and_exchange_a_ping() {
        // Two ends, each with its own certificate and 32-hex id.
        let phone_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let desk_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let phone_cert = DeviceCert::generate(phone_id);
        let desk_cert = DeviceCert::generate(desk_id);
        let phone_fp = phone_cert.fingerprint().unwrap();
        let desk_fp = desk_cert.fingerprint().unwrap();
        let phone_tls = TlsConfigs::build(&phone_cert).unwrap();
        let desk_tls = TlsConfigs::build(&desk_cert).unwrap();
        let phone_identity = Identity::desktop(phone_id, "Pretend Phone");
        let desk_identity = Identity::desktop(desk_id, "Celestina");

        // The phone listens and accepts (TLS client).
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
                    &mut counter(),
                    Duration::from_secs(5),
                )
            })
        };

        // The desktop dials (TLS server), from an announcement pointing at addr.
        let mut announced = phone_identity.clone();
        announced.tcp_port = Some(addr.port());
        let announcement = Announcement {
            identity: announced,
            source: addr,
        };
        let mut desk_link = Link::connect(
            &announcement,
            &desk_identity,
            &desk_tls,
            &mut counter(),
            Duration::from_secs(5),
        )
        .expect("desktop links to the phone");

        let mut phone_link = phone_accept
            .join()
            .unwrap()
            .expect("phone accepts the desktop");

        // Each learned the other's identity, and pinned the other's certificate.
        assert_eq!(desk_link.peer().device_id, phone_id);
        assert_eq!(phone_link.peer().device_id, desk_id);
        assert_eq!(desk_link.peer_fingerprint(), phone_fp);
        assert_eq!(phone_link.peer_fingerprint(), desk_fp);

        // The framed channel carries a packet: the desktop pings, the phone reads.
        desk_link.send_packet(&ping_packet(1)).unwrap();
        let got = phone_link
            .read_packet()
            .unwrap()
            .expect("a packet, not a close");
        assert!(got.is("kdeconnect.ping"));
    }

    #[test]
    fn a_line_past_the_cap_errors_instead_of_growing() {
        use super::{fill_line, LinkError, MAX_PACKET_LINE};
        use std::io::{BufReader, Cursor};

        let mut giant = vec![b'x'; MAX_PACKET_LINE + 10];
        giant.push(b'\n');
        let mut reader = BufReader::new(Cursor::new(giant));
        let mut pending = Vec::new();
        match fill_line(&mut reader, &mut pending) {
            Err(LinkError::LineTooLong) => {}
            other => panic!("expected LineTooLong, got {other:?}"),
        }
        assert!(pending.is_empty(), "an over-cap line must not accumulate");

        // A normal line right after is unaffected by the bound.
        let mut reader = BufReader::new(Cursor::new(b"{\"ok\":1}\n".to_vec()));
        let mut pending = Vec::new();
        assert!(fill_line(&mut reader, &mut pending).unwrap());
        assert_eq!(pending, b"{\"ok\":1}\n");
    }

    #[test]
    fn a_line_split_by_a_timeout_is_not_lost() {
        use super::{fill_line, LinkError};
        use std::collections::VecDeque;
        use std::io::{BufReader, Read};

        /// Yields its chunks one `read` at a time, including mid-line errors —
        /// the shape of a socket whose read timeout fires between TLS records.
        struct Choppy {
            chunks: VecDeque<std::io::Result<Vec<u8>>>,
        }
        impl Read for Choppy {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                match self.chunks.pop_front() {
                    Some(Ok(bytes)) => {
                        buf[..bytes.len()].copy_from_slice(&bytes);
                        Ok(bytes.len())
                    }
                    Some(Err(e)) => Err(e),
                    None => Ok(0),
                }
            }
        }

        let packet = br#"{"id":1,"type":"kdeconnect.ping","body":{}}"#;
        let (head, tail) = packet.split_at(10);
        let mut chunks = VecDeque::new();
        chunks.push_back(Ok(head.to_vec()));
        chunks.push_back(Err(std::io::Error::from(std::io::ErrorKind::WouldBlock)));
        chunks.push_back(Ok([tail, b"\n"].concat()));
        let mut reader = BufReader::new(Choppy { chunks });
        let mut pending = Vec::new();

        // The timeout surfaces as an error with half the line consumed…
        assert!(matches!(
            fill_line(&mut reader, &mut pending),
            Err(LinkError::Io(_))
        ));
        assert_eq!(pending, head, "the consumed half must be preserved");

        // …and the retry completes that same line, losing nothing.
        assert!(fill_line(&mut reader, &mut pending).unwrap());
        assert_eq!(&pending[..pending.len() - 1], packet);
    }
}
