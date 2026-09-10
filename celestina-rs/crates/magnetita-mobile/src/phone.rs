//! A phone's identity, pins and sessions, as the peer and the app use them.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use magnetita_link::endpoint::Expect;
use magnetita_link::trust::fingerprint_text;
use magnetita_link::{
    fingerprint_of, DeviceCert, Endpoint, EndpointConfig, LinkError, SendStream, Session,
    Transfers, TrustStore, TrustedPeer,
};
use magnetita_proto::daily::battery::BatteryStatus;
use magnetita_proto::daily::clipboard::ClipboardText;
use magnetita_proto::daily::notifications::{
    NotificationAction, NotificationDismissed, NotificationPosted, NotificationReply,
};
use magnetita_proto::daily::share::{ShareAccept, ShareDone, ShareOffer, ShareReject, ShareText};
use magnetita_proto::pair::{kind as pair_kind, Fingerprint, Pinned, QrPairing, QrPayload};
use magnetita_proto::{capability, CapabilityVersion, DeviceKind, Envelope, Hello};

/// How long one of the QR's addresses gets before the next is tried.
const ADDRESS_ATTEMPT: Duration = Duration::from_secs(3);

/// What this build of the phone offers.
fn capabilities() -> Vec<CapabilityVersion> {
    vec![
        CapabilityVersion {
            capability: capability::BATTERY,
            version: 1,
        },
        CapabilityVersion {
            capability: capability::FIND,
            version: 1,
        },
    ]
}

/// One phone: its identity, its pins and its endpoint. The device id is
/// derived from the certificate, so identity and pin are one fact and no
/// second file can drift.
pub struct Phone {
    pub device_id: String,
    pub name: String,
    pub fingerprint: Fingerprint,
    trust: Mutex<TrustStore>,
    endpoint: Endpoint,
}

/// A session with a desktop, and what came out of pairing for it.
pub struct PhoneSession {
    pub session: Arc<Session>,
    pub desktop: Hello,
    share: Arc<ShareState>,
}

/// What [`PhoneSession::next`] yields: an envelope from the desktop, or a
/// file this side finished receiving on its own stream.
#[derive(Debug)]
pub enum Incoming {
    Envelope(Envelope),
    FileReceived {
        transfer: u32,
        path: PathBuf,
        complete: bool,
    },
}

/// The share half of a session: the streams being sent, the offers not yet
/// answered, and the completions the receive tasks report.
struct ShareState {
    transfers: Transfers,
    next_transfer: AtomicU32,
    sending: tokio::sync::Mutex<HashMap<u32, SendStream>>,
    offers_in: Mutex<HashMap<u32, ShareOffer>>,
    received_tx: tokio::sync::mpsc::UnboundedSender<Incoming>,
    received_rx: tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<Incoming>>,
}

impl PhoneSession {
    fn wrap(session: Session, desktop: Hello) -> Self {
        let (received_tx, received_rx) = tokio::sync::mpsc::unbounded_channel();
        let share = Arc::new(ShareState {
            transfers: session.transfers(),
            next_transfer: AtomicU32::new(1),
            sending: tokio::sync::Mutex::new(HashMap::new()),
            offers_in: Mutex::new(HashMap::new()),
            received_tx,
            received_rx: tokio::sync::Mutex::new(received_rx),
        });
        Self {
            session: Arc::new(session),
            desktop,
            share,
        }
    }
}

impl Phone {
    /// Loads or creates the identity under `dir` and binds an endpoint on
    /// an ephemeral port. Must run inside a tokio runtime.
    pub fn open(dir: &Path, name: &str) -> Result<Self, LinkError> {
        std::fs::create_dir_all(dir)?;
        let cert = DeviceCert::ensure(dir, name)?;
        let fingerprint = fingerprint_of(&cert.chain()?[0]);
        let device_id = fingerprint[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let trust = TrustStore::load(&dir.join("trust.json"))?;
        let hello = Hello {
            device_id: device_id.clone(),
            device_name: name.into(),
            device_kind: DeviceKind::Phone,
            capabilities: capabilities(),
        };
        let endpoint = Endpoint::bind(
            EndpointConfig { cert, hello },
            "0.0.0.0:0".parse().expect("literal"),
        )?;
        Ok(Self {
            device_id,
            name: name.into(),
            fingerprint,
            trust: Mutex::new(trust),
            endpoint,
        })
    }

    /// The desktops this phone has pinned.
    pub fn pinned(&self) -> Vec<TrustedPeer> {
        self.trust
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .peers()
            .collect()
    }

    /// Forgets a pinned desktop.
    pub fn forget(&self, device_id: &str) -> Result<(), LinkError> {
        Ok(self
            .trust
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .forget(device_id)?)
    }

    /// Scans the QR text, dials the desktop it names with its certificate
    /// pinned from the QR, proves the secret, pins the desktop, and keeps
    /// the session open.
    pub async fn pair(&self, uri: &str) -> Result<(Pinned, PhoneSession), LinkError> {
        let payload = QrPayload::parse_uri(uri)?;
        let mut last = LinkError::Connection("the QR names no address".into());
        for address in &payload.addresses {
            let Ok(target) = address.parse::<SocketAddr>() else {
                continue;
            };
            let attempt = tokio::time::timeout(
                ADDRESS_ATTEMPT,
                self.endpoint
                    .connect(target, Expect::Fingerprint(payload.fingerprint)),
            )
            .await
            .unwrap_or(Err(LinkError::HandshakeTimeout));
            match attempt {
                Ok((session, hello)) => {
                    let mut pairing =
                        QrPairing::phone(&payload, self.fingerprint, session.peer_fingerprint())?;
                    session
                        .send_message(
                            capability::PAIRING,
                            pair_kind::QR_PROOF,
                            pairing.proof_to_send()?,
                        )
                        .await?;
                    let reply =
                        tokio::time::timeout(magnetita_link::HANDSHAKE_BUDGET, session.recv())
                            .await
                            .map_err(|_| LinkError::HandshakeTimeout)??;
                    if reply.capability != capability::PAIRING || reply.kind != pair_kind::QR_REPLY
                    {
                        return Err(LinkError::Protocol(
                            magnetita_proto::DecodeError::Malformed("expected the pairing reply"),
                        ));
                    }
                    let pinned = pairing.accept_reply(&reply.body)?;
                    self.trust
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .pin(TrustedPeer {
                            device_id: hello.device_id.clone(),
                            device_name: hello.device_name.clone(),
                            fingerprint: fingerprint_text(&pinned.peer_fingerprint),
                        })?;
                    return Ok((pinned, PhoneSession::wrap(session, hello)));
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Dials a pinned desktop.
    pub async fn connect(&self, address: SocketAddr) -> Result<PhoneSession, LinkError> {
        let snapshot = {
            let trust = self.trust.lock().unwrap_or_else(|e| e.into_inner());
            let mut s = TrustStore::in_memory();
            for p in trust.peers() {
                let _ = s.pin(p);
            }
            s
        };
        let (session, desktop) = self
            .endpoint
            .connect(address, Expect::Trusted(&snapshot))
            .await?;
        Ok(PhoneSession::wrap(session, desktop))
    }
}

impl PhoneSession {
    /// Reports a battery level.
    pub async fn report_battery(&self, level: u8, charging: bool) -> Result<(), LinkError> {
        self.session
            .send_message(
                capability::BATTERY,
                BatteryStatus::KIND,
                BatteryStatus {
                    level,
                    charging,
                    low: level <= 15,
                }
                .encode(),
            )
            .await?;
        Ok(())
    }

    /// Sends the phone's clipboard text; the desktop writes it to its own.
    pub async fn send_clipboard(&self, text: &str) -> Result<(), LinkError> {
        self.session
            .send_message(
                capability::CLIPBOARD,
                ClipboardText::KIND,
                ClipboardText {
                    text: text.to_owned(),
                }
                .encode(),
            )
            .await?;
        Ok(())
    }

    /// A notification appeared or changed on the phone.
    pub async fn send_notification(&self, note: &NotificationPosted) -> Result<(), LinkError> {
        self.session
            .send_message(
                capability::NOTIFICATIONS,
                NotificationPosted::KIND,
                note.encode(),
            )
            .await?;
        Ok(())
    }

    /// A notification left the phone.
    pub async fn send_notification_gone(&self, key: &str) -> Result<(), LinkError> {
        self.session
            .send_message(
                capability::NOTIFICATIONS,
                NotificationDismissed::KIND,
                NotificationDismissed {
                    key: key.to_owned(),
                }
                .encode(),
            )
            .await?;
        Ok(())
    }

    /// Waits for the next envelope, up to `timeout`; `None` on timeout.
    pub async fn next(&self, timeout: Duration) -> Result<Option<Incoming>, LinkError> {
        let mut received = self.share.received_rx.lock().await;
        let got = tokio::select! {
            env = tokio::time::timeout(timeout, self.session.recv()) => match env {
                Ok(r) => Incoming::Envelope(r?),
                Err(_) => return Ok(None),
            },
            Some(done) = received.recv() => done,
        };
        if let Incoming::Envelope(env) = &got {
            if env.capability == capability::SHARE {
                self.note_share(env).await;
            }
        }
        Ok(Some(got))
    }

    /// Share control messages the session acts on before the caller sees
    /// them: an offer is remembered until answered; an acceptance opens the
    /// bulk stream the caller then writes to.
    async fn note_share(&self, env: &Envelope) {
        match env.kind {
            ShareOffer::KIND => {
                if let Ok(offer) = ShareOffer::decode(&env.body) {
                    self.share
                        .offers_in
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .insert(offer.transfer, offer);
                }
            }
            ShareAccept::KIND => {
                if let Ok(accept) = ShareAccept::decode(&env.body) {
                    if let Ok(stream) = self.share.transfers.open(accept.transfer).await {
                        self.share
                            .sending
                            .lock()
                            .await
                            .insert(accept.transfer, stream);
                    }
                }
            }
            _ => {}
        }
    }

    /// Offers a file; the desktop's `ShareAccept` names the offset to send
    /// from and, once seen by [`Self::next`], the stream is open for
    /// [`Self::write_transfer`].
    pub async fn offer_file(&self, name: &str, size: u64, mime: &str) -> Result<u32, LinkError> {
        let transfer = self.share.next_transfer.fetch_add(1, Ordering::Relaxed);
        self.session
            .send_message(
                capability::SHARE,
                ShareOffer::KIND,
                ShareOffer {
                    transfer,
                    name: name.to_owned(),
                    size,
                    mime: mime.to_owned(),
                }
                .encode(),
            )
            .await?;
        Ok(transfer)
    }

    /// Writes the next bytes of an accepted transfer.
    pub async fn write_transfer(&self, transfer: u32, bytes: &[u8]) -> Result<(), LinkError> {
        let mut sending = self.share.sending.lock().await;
        let stream = sending
            .get_mut(&transfer)
            .ok_or_else(|| LinkError::Connection("transfer not accepted".into()))?;
        stream
            .write_all(bytes)
            .await
            .map_err(|e| LinkError::Connection(e.to_string()))
    }

    /// Ends an accepted transfer and tells the desktop every byte went.
    pub async fn finish_transfer(&self, transfer: u32) -> Result<(), LinkError> {
        if let Some(mut stream) = self.share.sending.lock().await.remove(&transfer) {
            stream
                .finish()
                .map_err(|e| LinkError::Connection(e.to_string()))?;
            let _ = stream.stopped().await;
        }
        self.session
            .send_message(
                capability::SHARE,
                ShareDone::KIND,
                ShareDone {
                    transfer,
                    complete: true,
                }
                .encode(),
            )
            .await?;
        Ok(())
    }

    /// Accepts an offered file into `dir`, resuming a partial left there,
    /// and returns the receive to run on the caller's runtime; its end is
    /// reported through [`Self::next`] as [`Incoming::FileReceived`].
    pub async fn accept_file(
        &self,
        transfer: u32,
        dir: PathBuf,
    ) -> Result<impl std::future::Future<Output = ()> + Send + 'static, LinkError> {
        let offer = self
            .share
            .offers_in
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&transfer)
            .ok_or_else(|| LinkError::Connection("no such offer".into()))?;
        let name = std::path::Path::new(&offer.name)
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .unwrap_or("file")
            .to_owned();
        std::fs::create_dir_all(&dir).map_err(LinkError::Io)?;
        let partial = dir.join(format!(".{name}.part"));
        let offset = std::fs::metadata(&partial)
            .map(|m| m.len())
            .unwrap_or(0)
            .min(offer.size);
        self.session
            .send_message(
                capability::SHARE,
                ShareAccept::KIND,
                ShareAccept { transfer, offset }.encode(),
            )
            .await?;
        let share = Arc::clone(&self.share);
        let session = Arc::clone(&self.session);
        Ok(async move {
            let complete = receive_into(&share.transfers, transfer, &partial, offset, offer.size)
                .await
                .unwrap_or(false);
            let path = if complete {
                unique_name(&dir, &name)
                    .and_then(|final_path| {
                        std::fs::rename(&partial, &final_path).map(|_| final_path)
                    })
                    .unwrap_or(partial.clone())
            } else {
                partial.clone()
            };
            let _ = session
                .send_message(
                    capability::SHARE,
                    ShareDone::KIND,
                    ShareDone { transfer, complete }.encode(),
                )
                .await;
            let _ = share.received_tx.send(Incoming::FileReceived {
                transfer,
                path,
                complete,
            });
        })
    }

    /// Declines an offered file.
    pub async fn reject_file(&self, transfer: u32) -> Result<(), LinkError> {
        self.share
            .offers_in
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&transfer);
        self.session
            .send_message(
                capability::SHARE,
                ShareReject::KIND,
                ShareReject { transfer }.encode(),
            )
            .await?;
        Ok(())
    }

    /// Shares a URL or a snippet, no stream needed.
    pub async fn send_text(&self, text: &str) -> Result<(), LinkError> {
        self.session
            .send_message(
                capability::SHARE,
                ShareText::KIND,
                ShareText {
                    text: text.to_owned(),
                }
                .encode(),
            )
            .await?;
        Ok(())
    }

    pub fn close(&self, reason: &str) {
        self.session.close(reason);
    }
}

/// Appends one transfer's stream to `partial` from `offset`; true when
/// `size` bytes are there.
async fn receive_into(
    transfers: &Transfers,
    transfer: u32,
    partial: &PathBuf,
    offset: u64,
    size: u64,
) -> Result<bool, LinkError> {
    use tokio::io::{AsyncSeekExt, AsyncWriteExt};
    let (id, mut stream) = loop {
        let (id, stream) = transfers.accept().await?;
        if id == transfer {
            break (id, stream);
        }
    };
    let _ = id;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(partial)
        .await
        .map_err(LinkError::Io)?;
    file.set_len(offset).await.map_err(LinkError::Io)?;
    file.seek(std::io::SeekFrom::Start(offset))
        .await
        .map_err(LinkError::Io)?;
    let mut written = offset;
    while written < size {
        let want = (64 * 1024).min((size - written) as usize);
        let Some(chunk) = stream
            .read_chunk(want, true)
            .await
            .map_err(|e| LinkError::Connection(e.to_string()))?
        else {
            break;
        };
        file.write_all(&chunk.bytes).await.map_err(LinkError::Io)?;
        written += chunk.bytes.len() as u64;
    }
    file.sync_all().await.map_err(LinkError::Io)?;
    Ok(written == size)
}

/// `name` in `dir`, or `name (n)` when taken.
fn unique_name(dir: &std::path::Path, name: &str) -> std::io::Result<PathBuf> {
    let path = std::path::Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = path.extension().and_then(|e| e.to_str());
    for n in 0..10_000 {
        let candidate = dir.join(match (n, ext) {
            (0, _) => name.to_owned(),
            (_, Some(ext)) => format!("{stem} ({n}).{ext}"),
            (_, None) => format!("{stem} ({n})"),
        });
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(std::io::Error::other("no free name"))
}

/// The share fields a desktop envelope names, for the application.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShareFields {
    pub transfer: Option<u32>,
    /// An offer's file name, or a shared text.
    pub text: Option<String>,
    pub size: Option<u64>,
    pub offset: Option<u64>,
    /// `Some(false)` for a rejection or an abandoned transfer.
    pub complete: Option<bool>,
}

pub fn share_fields(env: &Envelope) -> ShareFields {
    if env.capability != capability::SHARE {
        return ShareFields::default();
    }
    match env.kind {
        ShareOffer::KIND => ShareOffer::decode(&env.body)
            .map(|o| ShareFields {
                transfer: Some(o.transfer),
                text: Some(o.name),
                size: Some(o.size),
                ..Default::default()
            })
            .unwrap_or_default(),
        ShareAccept::KIND => ShareAccept::decode(&env.body)
            .map(|a| ShareFields {
                transfer: Some(a.transfer),
                offset: Some(a.offset),
                ..Default::default()
            })
            .unwrap_or_default(),
        ShareReject::KIND => ShareFields {
            transfer: ShareReject::decode(&env.body).ok().map(|r| r.transfer),
            complete: Some(false),
            ..Default::default()
        },
        ShareDone::KIND => ShareDone::decode(&env.body)
            .map(|d| ShareFields {
                transfer: Some(d.transfer),
                complete: Some(d.complete),
                ..Default::default()
            })
            .unwrap_or_default(),
        ShareText::KIND => ShareFields {
            text: ShareText::decode(&env.body).ok().map(|t| t.text),
            ..Default::default()
        },
        _ => ShareFields::default(),
    }
}

/// The text a clipboard envelope carries, once decoded by the protocol crate.
pub fn clipboard_text(env: &Envelope) -> Option<String> {
    if env.capability == capability::CLIPBOARD && env.kind == ClipboardText::KIND {
        ClipboardText::decode(&env.body).ok().map(|c| c.text)
    } else {
        None
    }
}

/// What a notification envelope from the desktop names: the phone's key,
/// the button index for an action, the text for a reply.
pub fn notification_fields(env: &Envelope) -> (Option<String>, Option<u16>, Option<String>) {
    if env.capability != capability::NOTIFICATIONS {
        return (None, None, None);
    }
    match env.kind {
        NotificationDismissed::KIND => (
            NotificationDismissed::decode(&env.body).ok().map(|d| d.key),
            None,
            None,
        ),
        NotificationAction::KIND => match NotificationAction::decode(&env.body) {
            Ok(a) => (Some(a.key), Some(a.action), None),
            Err(_) => (None, None, None),
        },
        NotificationReply::KIND => match NotificationReply::decode(&env.body) {
            Ok(r) => (Some(r.key), None, Some(r.text)),
            Err(_) => (None, None, None),
        },
        _ => (None, None, None),
    }
}

/// One line per envelope, for a shell or a log.
pub fn describe(env: &Envelope) -> String {
    match (env.capability, env.kind) {
        (capability::FIND, 1) => "find: ring".into(),
        (capability::FIND, 2) => "find: stop".into(),
        (capability::BATTERY, 2) => "battery: requested".into(),
        (capability::CLIPBOARD, 1) => format!("clipboard: {} bytes", env.body.len()),
        (capability::CLIPBOARD, 2) => "clipboard: requested".into(),
        (capability::NOTIFICATIONS, 2) => "notification: dismiss".into(),
        (capability::NOTIFICATIONS, 3) => "notification: action".into(),
        (capability::NOTIFICATIONS, 4) => "notification: reply".into(),
        (capability::SHARE, 1) => "share: offer".into(),
        (capability::SHARE, 2) => "share: accepted".into(),
        (capability::SHARE, 3) => "share: rejected".into(),
        (capability::SHARE, 4) => "share: done".into(),
        (capability::SHARE, 5) => "share: text".into(),
        (cap, kind) => format!("capability {cap} kind {kind}, {} bytes", env.body.len()),
    }
}
