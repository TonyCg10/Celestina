//! Shared protocol limits for KDE Connect's out-of-band payload sockets.

/// First port reserved by the KDE Connect protocol for payload transfers.
pub const PAYLOAD_PORT_MIN: u16 = 1739;

/// Last port reserved by the KDE Connect protocol for payload transfers.
pub const PAYLOAD_PORT_MAX: u16 = 1764;

/// Whether a peer-supplied port stays inside KDE Connect's payload range.
pub const fn is_payload_port(port: u16) -> bool {
    port >= PAYLOAD_PORT_MIN && port <= PAYLOAD_PORT_MAX
}

#[cfg(test)]
mod tests {
    use super::{is_payload_port, PAYLOAD_PORT_MAX, PAYLOAD_PORT_MIN};

    #[test]
    fn only_the_protocol_payload_range_is_accepted() {
        assert!(is_payload_port(PAYLOAD_PORT_MIN));
        assert!(is_payload_port(PAYLOAD_PORT_MAX));
        assert!(!is_payload_port(PAYLOAD_PORT_MIN - 1));
        assert!(!is_payload_port(PAYLOAD_PORT_MAX + 1));
    }
}
