//! Receiving a shared file over the phone's second TLS socket.
//!
//! A `kdeconnect.share.request` for a file names a `payloadTransferInfo` port
//! where the phone has opened a *separate* TLS socket serving the bytes. We dial
//! it as the TLS client (our cert, the same accept-any-but-verify verifier as the
//! main link — the device is already trusted there), and stream the declared
//! number of bytes to a file. One socket, one file, then it closes.

use std::fs::File;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use rustls::pki_types::ServerName;
use rustls::{ClientConnection, ServerConnection, StreamOwned};

use crate::tls::TlsConfigs;

/// How long to wait for the payload socket to connect and for its reads.
const PAYLOAD_TIMEOUT: Duration = Duration::from_secs(30);

/// The most a declared payload may claim. Generous — phones legitimately share
/// multi-gigabyte videos — while refusing an absurd or negative declaration
/// before a single byte lands on disk.
pub const MAX_PAYLOAD_SIZE: i64 = 64 * 1024 * 1024 * 1024;

/// Dial the phone's payload socket at `host:port`, TLS-wrap it, and stream the
/// declared `size` bytes into `dest`. A negative or over-[`MAX_PAYLOAD_SIZE`]
/// declaration is refused up front — the share packet always names the real
/// size, so an unbounded read has no honest sender. Returns the bytes written.
pub fn receive_to_file(
    host: &str,
    port: u16,
    size: i64,
    tls: &TlsConfigs,
    dest: &Path,
) -> io::Result<u64> {
    if !(0..=MAX_PAYLOAD_SIZE).contains(&size) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("refusing payload with declared size {size}"),
        ));
    }
    let addr = format!("{host}:{port}");
    let socket = addr
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad payload address"))?;
    let mut tcp = TcpStream::connect_timeout(&socket, PAYLOAD_TIMEOUT)?;
    tcp.set_read_timeout(Some(PAYLOAD_TIMEOUT))?;

    let server_name = ServerName::try_from(host.to_owned())
        .unwrap_or_else(|_| ServerName::try_from("kdeconnect").unwrap());
    let mut conn =
        ClientConnection::new(tls.client_config(), server_name).map_err(io::Error::other)?;
    conn.complete_io(&mut tcp)?;

    let stream = StreamOwned::new(conn, tcp);
    let mut file = File::create(dest)?;
    let written = io::copy(&mut stream.take(size as u64), &mut file)?;
    file.flush()?;
    Ok(written)
}

/// Serve `path` to a phone over a one-shot TLS payload socket (the send
/// direction). Binds an ephemeral port — returned so the caller can name it in
/// the outgoing `share.request` — then, on its own thread, accepts one
/// connection (we are the TLS *server* here, since we opened the port), streams
/// the file, and closes. The phone reads the declared number of bytes.
pub fn serve_file(tls: &TlsConfigs, path: &Path) -> io::Result<u16> {
    let listener = bind_payload_port()?;
    let port = listener.local_addr()?.port();
    let tls = tls.clone();
    let path: PathBuf = path.to_owned();
    thread::spawn(move || match serve_one(&listener, &tls, &path) {
        Ok(bytes) => eprintln!(
            "magnetita: payload send of {} ok ({bytes} bytes on port {port})",
            path.display()
        ),
        Err(e) => eprintln!(
            "magnetita: payload send of {} failed on port {port}: {e}",
            path.display()
        ),
    });
    Ok(port)
}

/// KDE Connect serves payloads on a port in 1739–1764; bind the first free one so
/// the phone connects where its own implementation expects, falling back to an
/// ephemeral port only if the whole range is taken.
fn bind_payload_port() -> io::Result<TcpListener> {
    for port in 1739..=1764 {
        if let Ok(listener) = TcpListener::bind(("0.0.0.0", port)) {
            return Ok(listener);
        }
    }
    TcpListener::bind(("0.0.0.0", 0))
}

fn serve_one(listener: &TcpListener, tls: &TlsConfigs, path: &Path) -> io::Result<u64> {
    // Wait (bounded) for the phone to dial the port we advertised.
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + PAYLOAD_TIMEOUT;
    let mut tcp = loop {
        match listener.accept() {
            Ok((tcp, _)) => break tcp,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "the phone did not fetch the file",
                    ));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e),
        }
    };
    tcp.set_nonblocking(false)?;
    tcp.set_write_timeout(Some(PAYLOAD_TIMEOUT))?;

    let mut conn = ServerConnection::new(tls.server_config()).map_err(io::Error::other)?;
    conn.complete_io(&mut tcp)?;

    let mut stream = StreamOwned::new(conn, tcp);
    let mut file = File::open(path)?;
    let bytes = io::copy(&mut file, &mut stream)?;
    stream.flush()?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{receive_to_file, MAX_PAYLOAD_SIZE};
    use crate::cert::DeviceCert;
    use crate::tls::TlsConfigs;

    #[test]
    fn absurd_declared_sizes_are_refused_before_dialing() {
        let cert = DeviceCert::generate("cccccccccccccccccccccccccccccccc");
        let tls = TlsConfigs::build(&cert).unwrap();
        let dest = std::env::temp_dir().join("magnetita-payload-refused");
        // Nothing listens on the port: any error other than InvalidInput would
        // mean the network was tried before the size was checked.
        for size in [-1, MAX_PAYLOAD_SIZE + 1] {
            let err = receive_to_file("127.0.0.1", 9, size, &tls, &dest).unwrap_err();
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        }
    }
}
