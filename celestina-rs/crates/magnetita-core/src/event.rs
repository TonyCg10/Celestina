//! Connection events — the vocabulary of the connection log.
//!
//! Every step of reaching a device, and every way it can fail, is one of these.
//! The transport (magnetita-net) emits the socket and TLS ones; the pure
//! [`Session`](crate::session::Session) emits the protocol ones; the app renders
//! them as the log that answers *"why won't it connect"*. The variants are
//! language-neutral — the human wording is the app's, the way [`DeviceType`]'s
//! label is — so the reason a link failed is a *type*, checkable, not a string
//! to grep.
//!
//! [`DeviceType`]: crate::identity::DeviceType

/// A step or an outcome in one device's connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionEvent {
    /// A UDP identity announce was heard on the network.
    Discovered,
    /// Opening the TCP link back to the device.
    Linking,
    /// The TLS handshake completed; the channel is encrypted from here.
    Secured,
    /// The peer's identity arrived over the link.
    Identified,
    /// A pairing exchange is underway — a request was sent or received.
    Pairing,
    /// Trust is established both ways.
    Paired,
    /// Trust was dropped, by either side.
    Unpaired,
    /// A ping arrived.
    Pinged,
    /// The link went away, or never formed, for this reason.
    Lost(LostReason),
}

/// Why a link failed or ended — the reason the log shows in the app's words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LostReason {
    /// Nothing answered on the network.
    NoReply,
    /// The device is not reachable — most often a different subnet or network.
    Unreachable,
    /// The TLS handshake was rejected.
    TlsFailed,
    /// The pinned certificate did not match — a possible impostor, so refused.
    CertChanged,
    /// The peer rejected the pairing request.
    PairRejected,
    /// The pairing request went unanswered past the ~30 s window.
    PairTimedOut,
    /// The device closed the link.
    PeerClosed,
}
