//! Receiving a shared file over the phone's second TLS socket.
//!
//! A `kdeconnect.share.request` for a file names a `payloadTransferInfo` port
//! where the phone has opened a *separate* TLS socket serving the bytes. We dial
//! it as the TLS client, require the certificate fingerprint already pinned by
//! the main link, and stream the declared number of bytes to a caller-reserved
//! file. One socket, one file, then it closes.

use std::fs::{File, Metadata};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use rustls::pki_types::ServerName;
use rustls::{ClientConnection, ServerConnection, StreamOwned};

use celestina_core::CancellationToken;
use magnetita_core::{is_payload_port, PAYLOAD_PORT_MAX, PAYLOAD_PORT_MIN};

use crate::deadline::{is_retryable_timeout, remaining_before};
use crate::tls::{peer_leaf_fingerprint, TlsConfigs};

/// How long to wait for the payload socket to connect and for its reads.
const PAYLOAD_TIMEOUT: Duration = Duration::from_secs(30);

/// A scanning or stalled untrusted client must not consume the whole transfer
/// window before the intended phone can connect.
const PAYLOAD_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);

/// Blocking socket calls use this shorter timeout so revoking a session can
/// stop an in-flight transfer without waiting for the full payload timeout.
const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// The listener is non-blocking; this keeps its idle loop cheap while bounding
/// cancellation latency even before a peer has connected.
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// The most a declared payload may claim. Generous — phones legitimately share
/// multi-gigabyte videos — while refusing an absurd or negative declaration
/// before a single byte lands on disk.
pub const MAX_PAYLOAD_SIZE: i64 = 64 * 1024 * 1024 * 1024;

/// A paired peer may announce several transfers, but may not create an
/// unbounded number of long-lived socket workers.
const MAX_CONCURRENT_PAYLOADS: usize = 4;

struct PayloadLimit {
    active: AtomicUsize,
}

/// Shared, non-blocking admission control for payload workers.
#[derive(Clone)]
pub struct PayloadLimiter {
    inner: Arc<PayloadLimit>,
}

/// One active payload slot. Dropping it releases the slot on every error path.
pub struct PayloadPermit {
    inner: Arc<PayloadLimit>,
}

/// Network metadata announced by the peer for one incoming payload.
#[derive(Clone, Copy, Debug)]
pub struct PayloadSource<'a> {
    pub host: &'a str,
    pub port: u16,
    pub size: i64,
    pub expected_peer_fingerprint: &'a str,
}

impl PayloadLimiter {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(PayloadLimit {
                active: AtomicUsize::new(0),
            }),
        }
    }

    pub fn try_acquire(&self) -> Option<PayloadPermit> {
        let mut active = self.inner.active.load(Ordering::Relaxed);
        loop {
            if active >= MAX_CONCURRENT_PAYLOADS {
                return None;
            }
            match self.inner.active.compare_exchange_weak(
                active,
                active + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    return Some(PayloadPermit {
                        inner: Arc::clone(&self.inner),
                    });
                }
                Err(current) => active = current,
            }
        }
    }
}

impl Default for PayloadLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PayloadPermit {
    fn drop(&mut self) {
        self.inner.active.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Dial the phone's announced payload socket, TLS-wrap it, and stream the
/// declared bytes into `dest`. A negative or over-[`MAX_PAYLOAD_SIZE`]
/// declaration is refused up front — the share packet always names the real
/// size, so an unbounded read has no honest sender. Returns the bytes written.
pub fn receive_to_file(
    source: PayloadSource<'_>,
    tls: &TlsConfigs,
    cancellation: &CancellationToken,
    _permit: PayloadPermit,
    mut dest: File,
) -> io::Result<u64> {
    let PayloadSource {
        host,
        port,
        size,
        expected_peer_fingerprint,
    } = source;
    if !(0..=MAX_PAYLOAD_SIZE).contains(&size) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("refusing payload with declared size {size}"),
        ));
    }
    validate_payload_port(port)?;
    validate_expected_fingerprint(expected_peer_fingerprint)?;
    ensure_not_cancelled(cancellation)?;
    let addr = format!("{host}:{port}");
    let socket = addr
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad payload address"))?;
    let mut tcp = connect_cancellable(&socket, cancellation)?;
    set_cancellable_timeouts(&tcp)?;

    let server_name = ServerName::try_from(host.to_owned())
        .or_else(|_| ServerName::try_from("kdeconnect"))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid TLS server name"))?;
    let mut conn =
        ClientConnection::new(tls.client_config(), server_name).map_err(io::Error::other)?;
    complete_client_handshake(&mut conn, &mut tcp, cancellation)?;
    verify_peer_fingerprint(
        peer_leaf_fingerprint(conn.peer_certificates()).as_deref(),
        expected_peer_fingerprint,
    )?;

    let mut stream = StreamOwned::new(conn, tcp);
    let written = copy_exact_cancellable(&mut stream, &mut dest, size, cancellation)?;
    ensure_not_cancelled(cancellation)?;
    dest.flush()?;
    ensure_not_cancelled(cancellation)?;
    dest.sync_all()?;
    ensure_not_cancelled(cancellation)?;
    Ok(written)
}

fn connect_cancellable(
    socket: &std::net::SocketAddr,
    cancellation: &CancellationToken,
) -> io::Result<TcpStream> {
    let deadline = Instant::now() + PAYLOAD_TIMEOUT;
    loop {
        ensure_not_cancelled(cancellation)?;
        let remaining = remaining_before(deadline, "payload connection timed out")?;
        match TcpStream::connect_timeout(socket, remaining.min(CANCELLATION_POLL_INTERVAL)) {
            Ok(tcp) => return Ok(tcp),
            Err(error) if is_retryable_timeout(&error) => {}
            Err(error) => return Err(error),
        }
    }
}

fn set_cancellable_timeouts(tcp: &TcpStream) -> io::Result<()> {
    tcp.set_read_timeout(Some(CANCELLATION_POLL_INTERVAL))?;
    tcp.set_write_timeout(Some(CANCELLATION_POLL_INTERVAL))
}

fn complete_client_handshake(
    conn: &mut ClientConnection,
    tcp: &mut TcpStream,
    cancellation: &CancellationToken,
) -> io::Result<()> {
    retry_until(
        PAYLOAD_TIMEOUT,
        "payload TLS handshake timed out",
        cancellation,
        || {
            conn.complete_io(tcp)?;
            Ok(!conn.is_handshaking())
        },
    )
}

fn complete_server_handshake(
    conn: &mut ServerConnection,
    tcp: &mut TcpStream,
    cancellation: &CancellationToken,
) -> io::Result<()> {
    retry_until(
        PAYLOAD_HANDSHAKE_TIMEOUT,
        "payload TLS handshake timed out",
        cancellation,
        || {
            conn.complete_io(tcp)?;
            Ok(!conn.is_handshaking())
        },
    )
}

fn retry_until(
    timeout: Duration,
    timeout_message: &'static str,
    cancellation: &CancellationToken,
    mut operation: impl FnMut() -> io::Result<bool>,
) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        ensure_not_cancelled(cancellation)?;
        remaining_before(deadline, timeout_message)?;
        match operation() {
            Ok(true) => {
                ensure_not_cancelled(cancellation)?;
                return Ok(());
            }
            Ok(false) => {}
            Err(error) if is_retryable_timeout(&error) => {}
            Err(error) => return Err(error),
        }
    }
}

fn copy_exact_cancellable(
    reader: &mut impl Read,
    writer: &mut impl Write,
    declared: i64,
    cancellation: &CancellationToken,
) -> io::Result<u64> {
    let expected = u64::try_from(declared)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "negative payload size"))?;
    let mut transferred = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    let mut idle_deadline = Instant::now() + PAYLOAD_TIMEOUT;

    while transferred < expected {
        ensure_not_cancelled(cancellation)?;
        remaining_before(idle_deadline, "payload transfer stalled")?;
        let remaining = expected - transferred;
        let limit = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|_| io::Error::other("payload chunk does not fit in memory"))?;
        let read = match reader.read(&mut buffer[..limit]) {
            Ok(0) => return verify_payload_size(transferred, declared),
            Ok(read) => {
                idle_deadline = Instant::now() + PAYLOAD_TIMEOUT;
                read
            }
            Err(error) if is_retryable_timeout(&error) => continue,
            Err(error) => return Err(error),
        };

        let mut offset = 0;
        while offset < read {
            ensure_not_cancelled(cancellation)?;
            remaining_before(idle_deadline, "payload transfer stalled")?;
            match writer.write(&buffer[offset..read]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "payload destination stopped accepting bytes",
                    ));
                }
                Ok(written) => {
                    offset += written;
                    transferred += written as u64;
                    idle_deadline = Instant::now() + PAYLOAD_TIMEOUT;
                }
                Err(error) if is_retryable_timeout(&error) => {}
                Err(error) => return Err(error),
            }
        }
    }

    verify_payload_size(transferred, declared)
}

fn flush_cancellable(writer: &mut impl Write, cancellation: &CancellationToken) -> io::Result<()> {
    retry_until(
        PAYLOAD_TIMEOUT,
        "payload flush timed out",
        cancellation,
        || match writer.flush() {
            Ok(()) => Ok(true),
            Err(error) => Err(error),
        },
    )
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> io::Result<()> {
    if cancellation.is_cancelled() {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "payload transfer cancelled",
        ))
    } else {
        Ok(())
    }
}

fn verify_payload_size(written: u64, declared: i64) -> io::Result<u64> {
    if written == declared as u64 {
        Ok(written)
    } else {
        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("payload declared {declared} bytes but sent {written}"),
        ))
    }
}

/// The port and exact size to advertise in the control packet for a prepared
/// one-shot payload server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServedPayload {
    pub port: u16,
    pub size: i64,
}

/// Open `path` exactly once and serve that file handle to the already-pinned
/// phone over a one-shot TLS payload socket. The returned port and size come
/// from the opened object, so replacing the path after approval cannot swap the
/// bytes served. The worker sends at most the announced size and reports a
/// short read as failure.
pub fn serve_file(
    tls: &TlsConfigs,
    path: &Path,
    expected_peer_fingerprint: &str,
    cancellation: &CancellationToken,
    permit: PayloadPermit,
) -> io::Result<ServedPayload> {
    validate_expected_fingerprint(expected_peer_fingerprint)?;
    ensure_not_cancelled(cancellation)?;
    let (file, metadata) = open_payload(path)?;
    let size = payload_size(&metadata)?;
    ensure_not_cancelled(cancellation)?;
    let listener = bind_payload_port()?;
    let port = listener.local_addr()?.port();
    let tls = tls.clone();
    let path: PathBuf = path.to_owned();
    let expected_peer_fingerprint = expected_peer_fingerprint.to_owned();
    let cancellation = cancellation.clone();
    thread::Builder::new()
        .name("magnetita-payload-send".to_owned())
        .spawn(move || {
            let _permit = permit;
            match serve_one(
                &listener,
                &tls,
                file,
                size,
                &expected_peer_fingerprint,
                &cancellation,
            ) {
                Ok(bytes) => eprintln!(
                    "magnetita: payload send of {} ok ({bytes} bytes on port {port})",
                    path.display()
                ),
                Err(e) => eprintln!(
                    "magnetita: payload send of {} failed on port {port}: {e}",
                    path.display()
                ),
            }
        })?;
    Ok(ServedPayload { port, size })
}

fn open_payload(path: &Path) -> io::Result<(File, Metadata)> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "payload source is not a regular file",
        ));
    }
    Ok((file, metadata))
}

fn payload_size(metadata: &Metadata) -> io::Result<i64> {
    let size = i64::try_from(metadata.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "payload source is too large"))?;
    if size > MAX_PAYLOAD_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("payload source exceeds {MAX_PAYLOAD_SIZE} bytes"),
        ));
    }
    Ok(size)
}

/// KDE Connect serves payloads only on 1739–1764. If the complete range is in
/// use, fail explicitly instead of advertising an out-of-contract port.
fn bind_payload_port() -> io::Result<TcpListener> {
    for port in PAYLOAD_PORT_MIN..=PAYLOAD_PORT_MAX {
        if let Ok(listener) = TcpListener::bind(("0.0.0.0", port)) {
            return Ok(listener);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AddrNotAvailable,
        "every KDE Connect payload port is already in use",
    ))
}

fn serve_one(
    listener: &TcpListener,
    tls: &TlsConfigs,
    mut file: File,
    size: i64,
    expected_peer_fingerprint: &str,
    cancellation: &CancellationToken,
) -> io::Result<u64> {
    // Wait (bounded) for the pinned phone to dial the advertised port. A client
    // with another certificate is ignored rather than stealing the one shot.
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + PAYLOAD_TIMEOUT;
    loop {
        ensure_not_cancelled(cancellation)?;
        remaining_before(deadline, "the phone did not fetch the file")?;
        match listener.accept() {
            Ok((mut tcp, _)) => {
                tcp.set_nonblocking(false)?;
                set_cancellable_timeouts(&tcp)?;

                let mut conn =
                    ServerConnection::new(tls.server_config()).map_err(io::Error::other)?;
                if complete_server_handshake(&mut conn, &mut tcp, cancellation).is_err() {
                    ensure_not_cancelled(cancellation)?;
                    continue;
                }
                if verify_peer_fingerprint(
                    peer_leaf_fingerprint(conn.peer_certificates()).as_deref(),
                    expected_peer_fingerprint,
                )
                .is_err()
                {
                    continue;
                }

                let mut stream = StreamOwned::new(conn, tcp);
                let bytes = copy_exact_cancellable(&mut file, &mut stream, size, cancellation)?;
                flush_cancellable(&mut stream, cancellation)?;
                return Ok(bytes);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(e) => return Err(e),
        }
    }
}

fn validate_payload_port(port: u16) -> io::Result<()> {
    if is_payload_port(port) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("payload port {port} is outside the protocol range"),
        ))
    }
}

fn validate_expected_fingerprint(expected: &str) -> io::Result<()> {
    if expected.is_empty() {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "payload peer fingerprint is empty",
        ))
    } else {
        Ok(())
    }
}

fn verify_peer_fingerprint(actual: Option<&str>, expected: &str) -> io::Result<()> {
    match actual {
        Some(actual) if actual.eq_ignore_ascii_case(expected) => Ok(()),
        Some(_) => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "payload peer certificate does not match the trusted link",
        )),
        None => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "payload peer did not present a certificate",
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::Read;

    use celestina_core::CancellationToken;

    use super::{
        open_payload, receive_to_file, validate_payload_port, verify_payload_size,
        verify_peer_fingerprint, PayloadLimiter, PayloadSource, MAX_CONCURRENT_PAYLOADS,
        MAX_PAYLOAD_SIZE,
    };
    use crate::cert::DeviceCert;
    use crate::tls::TlsConfigs;

    #[test]
    fn absurd_declared_sizes_are_refused_before_dialing() {
        let cert = DeviceCert::generate("cccccccccccccccccccccccccccccccc");
        let tls = TlsConfigs::build(&cert).unwrap();
        let dest =
            std::env::temp_dir().join(format!("magnetita-payload-refused-{}", std::process::id()));
        // Nothing listens on the port: any error other than InvalidInput would
        // mean the network was tried before the size was checked.
        for size in [-1, MAX_PAYLOAD_SIZE + 1] {
            let file = File::create(&dest).unwrap();
            let permit = PayloadLimiter::new().try_acquire().unwrap();
            let cancellation = CancellationToken::new();
            let err = receive_to_file(
                PayloadSource {
                    host: "127.0.0.1",
                    port: 1740,
                    size,
                    expected_peer_fingerprint: "trusted",
                },
                &tls,
                &cancellation,
                permit,
                file,
            )
            .unwrap_err();
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        }
        let _ = fs::remove_file(dest);
    }

    #[test]
    fn a_short_payload_is_never_reported_as_complete() {
        assert_eq!(verify_payload_size(12, 12).unwrap(), 12);
        assert_eq!(
            verify_payload_size(11, 12).unwrap_err().kind(),
            std::io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn payload_ports_and_peer_identity_are_fail_closed() {
        assert!(validate_payload_port(1740).is_ok());
        assert_eq!(
            validate_payload_port(8080).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
        assert!(verify_peer_fingerprint(Some("AA:bb"), "aa:BB").is_ok());
        for actual in [Some("different"), None] {
            assert_eq!(
                verify_peer_fingerprint(actual, "trusted")
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::PermissionDenied
            );
        }
    }

    #[test]
    fn replacing_a_path_cannot_replace_the_opened_payload() {
        let root = std::env::temp_dir().join(format!(
            "magnetita-payload-open-once-{}",
            std::process::id()
        ));
        fs::write(&root, b"approved").unwrap();
        let (mut opened, metadata) = open_payload(&root).unwrap();
        fs::remove_file(&root).unwrap();
        fs::write(&root, b"replacement").unwrap();

        let mut bytes = Vec::new();
        opened.read_to_end(&mut bytes).unwrap();
        assert_eq!(metadata.len(), 8);
        assert_eq!(bytes, b"approved");
        let _ = fs::remove_file(root);
    }

    #[test]
    fn payload_concurrency_is_bounded_and_slots_are_released() {
        let limiter = PayloadLimiter::new();
        let permits: Vec<_> = (0..MAX_CONCURRENT_PAYLOADS)
            .map(|_| limiter.try_acquire().expect("slot is available"))
            .collect();
        assert!(limiter.try_acquire().is_none());
        drop(permits);
        assert!(limiter.try_acquire().is_some());
    }
}
