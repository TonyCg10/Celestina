//! What can go wrong on the link, typed so the daemon logs it and the peer
//! never authors a message.

use std::fmt;
use std::io;

use magnetita_proto::{DecodeError, PairError};

#[derive(Debug)]
pub enum LinkError {
    Io(io::Error),
    /// The QUIC connection failed or ended.
    Connection(String),
    /// The peer presented a certificate that is not pinned, or not the one
    /// this connection expected.
    Untrusted,
    /// The whole handshake did not finish within its budget.
    HandshakeTimeout,
    /// A message on the wire could not be decoded.
    Protocol(DecodeError),
    /// The pairing exchange failed.
    Pairing(PairError),
    /// The peer closed the stream or the connection.
    Closed,
    /// A frame declared more bytes than a message may carry.
    FrameTooLarge(u32),
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Connection(e) => write!(f, "connection: {e}"),
            Self::Untrusted => f.write_str("the peer's certificate is not pinned"),
            Self::HandshakeTimeout => f.write_str("the handshake did not finish in time"),
            Self::Protocol(e) => write!(f, "protocol: {e}"),
            Self::Pairing(e) => write!(f, "pairing: {e}"),
            Self::Closed => f.write_str("the peer closed"),
            Self::FrameTooLarge(n) => write!(f, "a frame of {n} bytes exceeds the message limit"),
        }
    }
}

impl std::error::Error for LinkError {}

impl From<io::Error> for LinkError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<DecodeError> for LinkError {
    fn from(e: DecodeError) -> Self {
        Self::Protocol(e)
    }
}

impl From<PairError> for LinkError {
    fn from(e: PairError) -> Self {
        Self::Pairing(e)
    }
}

impl From<quinn::ConnectionError> for LinkError {
    fn from(e: quinn::ConnectionError) -> Self {
        Self::Connection(e.to_string())
    }
}

impl From<quinn::ConnectError> for LinkError {
    fn from(e: quinn::ConnectError) -> Self {
        Self::Connection(e.to_string())
    }
}

impl From<quinn::WriteError> for LinkError {
    fn from(e: quinn::WriteError) -> Self {
        match e {
            quinn::WriteError::ConnectionLost(_) | quinn::WriteError::ClosedStream => Self::Closed,
            other => Self::Connection(other.to_string()),
        }
    }
}

impl From<quinn::ReadExactError> for LinkError {
    fn from(e: quinn::ReadExactError) -> Self {
        match e {
            quinn::ReadExactError::FinishedEarly(_) => Self::Closed,
            quinn::ReadExactError::ReadError(r) => Self::Connection(r.to_string()),
        }
    }
}
