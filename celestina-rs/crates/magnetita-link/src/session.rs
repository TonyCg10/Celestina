//! One connection to one device: envelopes on a control stream, bulk bytes
//! on their own streams, motion as datagrams.
//!
//! Framing on the control stream is a 4-byte big-endian length followed by
//! one encoded [`Envelope`]; the length is checked against the protocol's
//! message limit before a byte of the body is read. Transfer streams carry
//! raw bytes and nothing else — the `share` messages on the control stream
//! say what they are.

use std::sync::Arc;
use std::time::Duration;

use magnetita_proto::bound::MAX_MESSAGE;
use magnetita_proto::pair::Fingerprint;
use magnetita_proto::{capability, Envelope, Hello};

use crate::demux::{Accepted, Demux, Expected};
use crate::error::LinkError;

/// The whole handshake — accept or dial, TLS, pin check, hello both ways —
/// must finish within this, measured from before the first byte.
pub const HANDSHAKE_BUDGET: Duration = Duration::from_secs(10);

/// The bulk-stream half of a session, cloneable and owned by the task that
/// moves the bytes. Dropping it does not close the connection. Incoming
/// streams are told apart by [`crate::demux`].
#[derive(Clone)]
pub struct Transfers {
    pub(crate) conn: quinn::Connection,
    demux: Arc<Demux>,
}

impl Transfers {
    /// Opens a unidirectional stream prefixed by `transfer`.
    pub async fn open(&self, transfer: u32) -> Result<quinn::SendStream, LinkError> {
        let mut s = self.conn.open_uni().await?;
        s.write_all(&transfer.to_be_bytes()).await?;
        Ok(s)
    }

    /// The next bulk stream nobody [`Self::expect`]s, with its transfer id.
    /// A stream the peer reset, ended or left silent before its id is
    /// [`Accepted::Skipped`], and ids are read side by side, so one such
    /// stream never delays or stops the streams after it. Only the
    /// connection's end is an error.
    pub async fn accept_or_skip(&self) -> Result<Accepted, LinkError> {
        self.demux.next().await
    }

    /// The next bulk stream with its transfer id, skipping streams that
    /// ended before their id ([`Self::accept_or_skip`]).
    pub async fn accept(&self) -> Result<(u32, quinn::RecvStream), LinkError> {
        loop {
            if let Accepted::Stream(id, r) = self.accept_or_skip().await? {
                return Ok((id, r));
            }
        }
    }

    /// Reserves the stream of `transfer` before the peer is told to send it:
    /// it reaches the returned waiter whenever it arrives, whatever other
    /// streams arrive around it. Must run inside the session's runtime.
    pub fn expect(&self, transfer: u32) -> Result<Expected, LinkError> {
        self.demux.expect(transfer)
    }

    /// Takes only reserved streams from now on: a stream nobody
    /// [`Self::expect`]s is stopped at once instead of queued for
    /// [`Self::accept`], so unread streams never hold the connection's
    /// stream slots. The phone, which receives only what it accepted, runs
    /// this way; the desktop reads the queue.
    pub fn reserve_only(&self) {
        self.demux.reserve_only();
    }
}

/// An open, pinned connection. Dropping it closes the connection.
pub struct Session {
    conn: quinn::Connection,
    demux: Arc<Demux>,
    pub(crate) control_send: tokio::sync::Mutex<quinn::SendStream>,
    control_recv: tokio::sync::Mutex<quinn::RecvStream>,
    peer_fingerprint: Fingerprint,
    next_id: std::sync::atomic::AtomicU32,
}

impl Session {
    pub(crate) fn new(
        conn: quinn::Connection,
        send: quinn::SendStream,
        recv: quinn::RecvStream,
        peer_fingerprint: Fingerprint,
    ) -> Self {
        Self {
            demux: Arc::new(Demux::new(conn.clone())),
            conn,
            control_send: tokio::sync::Mutex::new(send),
            control_recv: tokio::sync::Mutex::new(recv),
            peer_fingerprint,
            next_id: std::sync::atomic::AtomicU32::new(1),
        }
    }

    /// The certificate this connection was authenticated by.
    pub fn peer_fingerprint(&self) -> Fingerprint {
        self.peer_fingerprint
    }

    pub fn remote_address(&self) -> std::net::SocketAddr {
        self.conn.remote_address()
    }

    /// Sends one envelope on the control stream.
    pub async fn send(&self, envelope: &Envelope) -> Result<(), LinkError> {
        let bytes = envelope.encode();
        let len = u32::try_from(bytes.len()).map_err(|_| LinkError::FrameTooLarge(u32::MAX))?;
        let mut s = self.control_send.lock().await;
        s.write_all(&len.to_be_bytes()).await?;
        s.write_all(&bytes).await?;
        Ok(())
    }

    /// Ends the control stream after everything written to it and waits
    /// until the peer has acknowledged receiving all of it. A connection
    /// closed straight after a write may drop what the peer had not yet
    /// received; closed after this, the peer still reads every envelope.
    /// Nothing may be sent on the control stream afterwards.
    pub async fn finish_control(&self) -> Result<(), LinkError> {
        let mut s = self.control_send.lock().await;
        s.finish()
            .map_err(|e| LinkError::Connection(e.to_string()))?;
        s.stopped()
            .await
            .map_err(|e| LinkError::Connection(e.to_string()))?;
        Ok(())
    }

    /// Encodes `body` as the next envelope of `capability`/`kind` and sends it.
    pub async fn send_message(
        &self,
        capability: u16,
        kind: u16,
        body: Vec<u8>,
    ) -> Result<u32, LinkError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.send(&Envelope {
            capability,
            kind,
            id,
            body,
        })
        .await?;
        Ok(id)
    }

    /// Receives the next envelope on the control stream.
    pub async fn recv(&self) -> Result<Envelope, LinkError> {
        let mut r = self.control_recv.lock().await;
        let mut len = [0u8; 4];
        r.read_exact(&mut len).await?;
        let len = u32::from_be_bytes(len);
        if len as usize > MAX_MESSAGE {
            return Err(LinkError::FrameTooLarge(len));
        }
        let mut bytes = vec![0u8; len as usize];
        r.read_exact(&mut bytes).await?;
        Ok(Envelope::decode(&bytes)?)
    }

    /// Sends our hello and reads the peer's; the order is fixed so neither
    /// side waits on the other.
    pub(crate) async fn exchange_hello(&self, mine: &Hello) -> Result<Hello, LinkError> {
        self.send_message(capability::HELLO, Hello::KIND, mine.encode())
            .await?;
        let theirs = self.recv().await?;
        if theirs.capability != capability::HELLO || theirs.kind != Hello::KIND {
            return Err(LinkError::Protocol(
                magnetita_proto::DecodeError::Malformed("expected hello"),
            ));
        }
        Ok(Hello::decode(&theirs.body)?)
    }

    /// Opens a unidirectional stream for bulk bytes (a `share` transfer, the
    /// mirror's video). The first thing written must be the transfer id the
    /// control stream announced, so the receiver knows which it is.
    pub async fn open_transfer(&self, transfer: u32) -> Result<quinn::SendStream, LinkError> {
        self.transfers().open(transfer).await
    }

    /// Accepts the next bulk stream and reads its transfer id.
    pub async fn accept_transfer(&self) -> Result<(u32, quinn::RecvStream), LinkError> {
        self.transfers().accept().await
    }

    /// A handle for bulk streams that a task may own apart from the session:
    /// the connection is shared, so a transfer can run while the control
    /// stream keeps being read elsewhere.
    pub fn transfers(&self) -> Transfers {
        Transfers {
            conn: self.conn.clone(),
            demux: Arc::clone(&self.demux),
        }
    }

    /// Sends a small message unreliably: pointer motion where a late sample
    /// is worse than a lost one.
    pub fn send_datagram(
        &self,
        capability: u16,
        kind: u16,
        body: Vec<u8>,
    ) -> Result<(), LinkError> {
        let env = Envelope {
            capability,
            kind,
            id: 0,
            body,
        };
        self.conn
            .send_datagram(env.encode().into())
            .map_err(|e| LinkError::Connection(e.to_string()))
    }

    pub async fn recv_datagram(&self) -> Result<Envelope, LinkError> {
        let bytes = self.conn.read_datagram().await?;
        Ok(Envelope::decode(&bytes)?)
    }

    /// Whether the connection has ended, by either side or by its idle
    /// timeout; a failed call on an open session is a refusal, not an end.
    pub fn is_closed(&self) -> bool {
        self.conn.close_reason().is_some()
    }

    /// Closes with a reason the peer sees in its logs, never in its UI.
    pub fn close(&self, reason: &str) {
        self.conn.close(0u32.into(), reason.as_bytes());
    }
}
