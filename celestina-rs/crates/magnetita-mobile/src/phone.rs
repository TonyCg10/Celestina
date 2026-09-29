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
    device_id_of, fingerprint_of, DeviceCert, Endpoint, EndpointConfig, Expected, LinkError,
    SendStream, Session, Transfers, TrustStore, TrustedPeer,
};
use magnetita_proto::control::commands::CommandRun;
use magnetita_proto::control::input::{Button, Key, PointerButton, PointerMove, Scroll, Text};
use magnetita_proto::daily::battery::BatteryStatus;
use magnetita_proto::daily::clipboard::ClipboardText;
use magnetita_proto::daily::media::{MediaButton, MediaCommand, MediaRequest, MediaState};
use magnetita_proto::daily::notifications::{NotificationDismissed, NotificationPosted};
use magnetita_proto::daily::share::{
    safe_filename, ShareAccept, ShareDone, ShareOffer, ShareReject, ShareText,
};
use magnetita_proto::mirror::{MirrorStarted, MirrorStop};
use magnetita_proto::pair::{kind as pair_kind, Fingerprint, Pinned, QrPairing, QrPayload};
use magnetita_proto::phone::contacts::ContactsSync;
use magnetita_proto::phone::sms::{SmsConversations, SmsReceived, SmsThread};
use magnetita_proto::phone::telephony::CallEvent;
use magnetita_proto::{capability, CapabilityVersion, DeviceKind, Envelope, Hello, Negotiated};

/// How long one of the QR's addresses gets before the next is tried.
const ADDRESS_ATTEMPT: Duration = Duration::from_secs(3);

/// What this build of the phone offers, each at version 1: every capability
/// it sends or handles (MAG-8). A session uses only what the desktop's hello
/// also offers.
pub fn offered() -> Vec<CapabilityVersion> {
    [
        capability::BATTERY,
        capability::CLIPBOARD,
        capability::NOTIFICATIONS,
        capability::FIND,
        capability::SHARE,
        capability::MEDIA,
        capability::COMMANDS,
        capability::INPUT,
        capability::MIRROR,
        capability::SMS,
        capability::CONTACTS,
        capability::TELEPHONY,
        capability::STORAGE,
    ]
    .into_iter()
    .map(|capability| CapabilityVersion {
        capability,
        version: 1,
    })
    .collect()
}

/// The largest file this phone receives: the payload limit the desktop
/// applies to the files it receives, so both directions share one bound.
pub const MAX_RECEIVED: u64 = magnetita_net::MAX_PAYLOAD_SIZE.unsigned_abs();

/// Storage a received file must leave free on the phone.
pub const ROOM_RESERVE: u64 = 64 * 1024 * 1024;

/// How long an accepted download waits for the desktop to open its stream;
/// past it the download ends incomplete and its partial stays for a resume.
#[cfg(not(test))]
pub const STREAM_WAIT: Duration = Duration::from_secs(30);
#[cfg(test)]
pub const STREAM_WAIT: Duration = Duration::from_secs(1);

/// How many of the desktop's offers may wait for an answer at once; past
/// it a new offer is declined at once, so a flood of offers holds no memory.
pub const MAX_PENDING_OFFERS: usize = 8;

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

/// How many envelopes the control stream's reader holds for [`PhoneSession::next`]
/// before it stops reading; past it, QUIC flow control holds the desktop.
const CONTROL_QUEUE: usize = 64;

/// A session with a desktop, and what came out of pairing for it.
pub struct PhoneSession {
    pub session: Arc<Session>,
    pub desktop: Hello,
    /// What this phone and the desktop both offered: every send and every
    /// envelope handed to the caller is gated on it.
    negotiated: Negotiated,
    share: Arc<ShareState>,
    /// What the control stream's one reader task read, in order. A
    /// control-stream read is not cancel-safe: a read dropped mid-frame
    /// loses the bytes it had taken and the next read takes body bytes for a
    /// length. So the reads live in one task that is never cancelled, and
    /// [`Self::next`] waits on this channel, which is cancel-safe.
    control: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Result<Envelope, LinkError>>>,
    reader: tokio::task::JoinHandle<()>,
}

impl Drop for PhoneSession {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

/// What [`PhoneSession::next`] yields: an envelope from the desktop, a
/// file this side finished receiving on its own stream, or an envelope this
/// side refused.
#[derive(Debug)]
pub enum Incoming {
    Envelope(Envelope),
    FileReceived {
        transfer: u32,
        path: PathBuf,
        complete: bool,
    },
    /// Never acted on: a capability the session did not negotiate, or an
    /// offer past this phone's bounds, which was already declined.
    Refused {
        capability: u16,
        kind: u16,
        reason: &'static str,
    },
}

/// The share half of a session: the streams being sent, the offers not yet
/// answered, and the completions the receive tasks report.
struct ShareState {
    transfers: Transfers,
    next_transfer: AtomicU32,
    sending: tokio::sync::Mutex<HashMap<u32, SendStream>>,
    offers_in: Mutex<HashMap<u32, ShareOffer>>,
    /// Accepted downloads still waiting for their stream, each with the way
    /// to end it when the desktop abandons it first.
    receiving: Mutex<HashMap<u32, tokio::sync::oneshot::Sender<()>>>,
    received_tx: tokio::sync::mpsc::UnboundedSender<Incoming>,
    received_rx: tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<Incoming>>,
}

impl PhoneSession {
    /// Must run inside the tokio runtime the session belongs to: the
    /// control stream's reader is spawned on it.
    fn wrap(session: Session, desktop: Hello) -> Self {
        let session = Arc::new(session);
        // The phone receives only the streams of downloads it accepted; any
        // other stream is stopped at once, so none holds a stream slot.
        session.transfers().reserve_only();
        let (control_tx, control_rx) = tokio::sync::mpsc::channel(CONTROL_QUEUE);
        let reader = {
            let session = Arc::clone(&session);
            tokio::spawn(async move {
                loop {
                    let next = session.recv().await;
                    let failed = next.is_err();
                    if control_tx.send(next).await.is_err() || failed {
                        break;
                    }
                }
            })
        };
        let (received_tx, received_rx) = tokio::sync::mpsc::unbounded_channel();
        let share = Arc::new(ShareState {
            transfers: session.transfers(),
            next_transfer: AtomicU32::new(1),
            sending: tokio::sync::Mutex::new(HashMap::new()),
            offers_in: Mutex::new(HashMap::new()),
            receiving: Mutex::new(HashMap::new()),
            received_tx,
            received_rx: tokio::sync::Mutex::new(received_rx),
        });
        let negotiated = Negotiated::between(&offered(), &desktop.capabilities);
        Self {
            session,
            desktop,
            negotiated,
            share,
            control: tokio::sync::Mutex::new(control_rx),
            reader,
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
        let device_id = device_id_of(&fingerprint);
        let trust = TrustStore::load(&dir.join("trust.json"))?;
        let hello = Hello {
            device_id: device_id.clone(),
            device_name: name.into(),
            device_kind: DeviceKind::Phone,
            capabilities: offered(),
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
    /// What this phone and the desktop agreed on for this session.
    pub fn negotiated(&self) -> &Negotiated {
        &self.negotiated
    }

    /// Refuses a capability the session did not negotiate.
    fn gate(&self, capability: u16) -> Result<(), LinkError> {
        if self.negotiated.allows(capability) {
            Ok(())
        } else {
            Err(LinkError::Declined(capability))
        }
    }

    /// Sends one message of a negotiated capability on the control stream.
    async fn send(&self, capability: u16, kind: u16, body: Vec<u8>) -> Result<(), LinkError> {
        self.gate(capability)?;
        self.session.send_message(capability, kind, body).await?;
        Ok(())
    }

    /// Reports a battery level.
    pub async fn report_battery(&self, level: u8, charging: bool) -> Result<(), LinkError> {
        self.send(
            capability::BATTERY,
            BatteryStatus::KIND,
            BatteryStatus {
                level,
                charging,
                low: level <= 15,
            }
            .encode(),
        )
        .await
    }

    /// Sends the phone's clipboard text; the desktop writes it to its own.
    /// Text the wire's clipboard rule refuses is refused here, before it is
    /// sent, so the phone can say so instead of "sent" (MAG-19).
    pub async fn send_clipboard(&self, text: &str) -> Result<(), LinkError> {
        if !magnetita_proto::daily::clipboard::syncable(text) {
            return Err(LinkError::Refused(
                "clipboard text must be non-empty, without NUL and within the wire's bound",
            ));
        }
        self.send(
            capability::CLIPBOARD,
            ClipboardText::KIND,
            ClipboardText {
                text: text.to_owned(),
            }
            .encode(),
        )
        .await
    }

    /// A notification appeared or changed on the phone.
    pub async fn send_notification(&self, note: &NotificationPosted) -> Result<(), LinkError> {
        self.send(
            capability::NOTIFICATIONS,
            NotificationPosted::KIND,
            note.encode(),
        )
        .await
    }

    /// A notification left the phone.
    pub async fn send_notification_gone(&self, key: &str) -> Result<(), LinkError> {
        self.send(
            capability::NOTIFICATIONS,
            NotificationDismissed::KIND,
            NotificationDismissed {
                key: key.to_owned(),
            }
            .encode(),
        )
        .await
    }

    /// Waits for the next envelope, up to `timeout`; `None` on timeout.
    /// Only channel receives are raced here, so a timeout never cuts a frame.
    pub async fn next(&self, timeout: Duration) -> Result<Option<Incoming>, LinkError> {
        let mut control = self.control.lock().await;
        let mut received = self.share.received_rx.lock().await;
        let got = tokio::select! {
            env = tokio::time::timeout(timeout, control.recv()) => match env {
                Ok(Some(r)) => Incoming::Envelope(r?),
                Ok(None) => {
                    return Err(LinkError::Connection("the control stream has ended".into()))
                }
                Err(_) => return Ok(None),
            },
            Some(done) = received.recv() => done,
        };
        let Incoming::Envelope(env) = got else {
            return Ok(Some(got));
        };
        if !self.negotiated.allows(env.capability) {
            return Ok(Some(Incoming::Refused {
                capability: env.capability,
                kind: env.kind,
                reason: "not negotiated",
            }));
        }
        if env.capability == capability::SHARE {
            if let Some(reason) = self.note_share(&env).await {
                return Ok(Some(Incoming::Refused {
                    capability: env.capability,
                    kind: env.kind,
                    reason,
                }));
            }
        }
        Ok(Some(Incoming::Envelope(env)))
    }

    /// Share control messages the session acts on before the caller sees
    /// them: an offer is remembered until answered; an acceptance opens the
    /// bulk stream the caller then writes to. An offer past this phone's
    /// bounds is declined here and its reason returned (AND-9).
    async fn note_share(&self, env: &Envelope) -> Option<&'static str> {
        match env.kind {
            ShareOffer::KIND => {
                let offer = ShareOffer::decode(&env.body).ok()?;
                let transfer = offer.transfer;
                let refused = if offer.size > MAX_RECEIVED {
                    Some("the offered file is larger than the phone receives")
                } else {
                    let mut offers = self
                        .share
                        .offers_in
                        .lock()
                        .unwrap_or_else(|p| p.into_inner());
                    if offers.len() >= MAX_PENDING_OFFERS && !offers.contains_key(&transfer) {
                        Some("too many offers wait for an answer")
                    } else {
                        offers.insert(transfer, offer);
                        None
                    }
                };
                if let Some(why) = refused {
                    self.decline(transfer, why).await;
                }
                return refused;
            }
            // The desktop gave up a download it was to send (its file could
            // not be read): the phone stops waiting for that stream.
            ShareReject::KIND | ShareDone::KIND => {
                let abandoned = if env.kind == ShareReject::KIND {
                    ShareReject::decode(&env.body).ok().map(|r| r.transfer)
                } else {
                    ShareDone::decode(&env.body)
                        .ok()
                        .filter(|d| !d.complete)
                        .map(|d| d.transfer)
                };
                if let Some(transfer) = abandoned {
                    let waiting = self
                        .share
                        .receiving
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .remove(&transfer);
                    if let Some(end) = waiting {
                        let _ = end.send(());
                    }
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
        None
    }

    /// Offers a file; the desktop's `ShareAccept` names the offset to send
    /// from and, once seen by [`Self::next`], the stream is open for
    /// [`Self::write_transfer`].
    pub async fn offer_file(&self, name: &str, size: u64, mime: &str) -> Result<u32, LinkError> {
        let transfer = self.share.next_transfer.fetch_add(1, Ordering::Relaxed);
        self.send(
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

    /// Opens a bulk stream of a fixed id (the mirror's video or audio) for
    /// [`Self::write_transfer`]; [`Self::close_stream`] ends it quietly.
    pub async fn open_stream(&self, id: u32) -> Result<(), LinkError> {
        self.gate(capability::MIRROR)?;
        let stream = self.share.transfers.open(id).await?;
        self.share.sending.lock().await.insert(id, stream);
        Ok(())
    }

    /// Ends a stream opened by [`Self::open_stream`] without a share message.
    pub async fn close_stream(&self, id: u32) {
        if let Some(mut stream) = self.share.sending.lock().await.remove(&id) {
            let _ = stream.finish();
        }
    }

    /// The mirror is streaming with this shape.
    pub async fn send_mirror_started(&self, started: &MirrorStarted) -> Result<(), LinkError> {
        self.send(capability::MIRROR, MirrorStarted::KIND, started.encode())
            .await
    }

    /// A storage reply (or the state) built by [`crate::storage`].
    pub async fn send_storage(&self, env: Envelope) -> Result<(), LinkError> {
        self.send(env.capability, env.kind, env.body).await
    }

    /// The mirror stopped on this side.
    pub async fn send_mirror_stop(&self) -> Result<(), LinkError> {
        self.send(capability::MIRROR, MirrorStop::KIND, MirrorStop.encode())
            .await
    }

    /// Ends an accepted transfer and tells the desktop every byte went.
    pub async fn finish_transfer(&self, transfer: u32) -> Result<(), LinkError> {
        if let Some(mut stream) = self.share.sending.lock().await.remove(&transfer) {
            stream
                .finish()
                .map_err(|e| LinkError::Connection(e.to_string()))?;
            let _ = stream.stopped().await;
        }
        self.send(
            capability::SHARE,
            ShareDone::KIND,
            ShareDone {
                transfer,
                complete: true,
            }
            .encode(),
        )
        .await
    }

    /// Gives up an accepted transfer this side cannot finish (its source
    /// failed): the stream ends short and the desktop is told the transfer
    /// was abandoned, so it keeps its partial for a later resume. The stream
    /// is finished, not reset: a desktop that reads a short stream marks it
    /// incomplete, while a reset before the transfer id stopped older
    /// desktops from accepting any later stream.
    pub async fn abandon_transfer(&self, transfer: u32) -> Result<(), LinkError> {
        if let Some(mut stream) = self.share.sending.lock().await.remove(&transfer) {
            let _ = stream.finish();
        }
        self.send(
            capability::SHARE,
            ShareDone::KIND,
            ShareDone {
                transfer,
                complete: false,
            }
            .encode(),
        )
        .await
    }

    /// Whether the session has ended; a call that failed while it is still
    /// open was refused (not negotiated, over a bound), and the session goes on.
    pub fn is_closed(&self) -> bool {
        self.session.is_closed()
    }

    /// Accepts an offered file into `dir`, resuming a partial left there,
    /// and returns the receive to run on the caller's runtime; its end is
    /// reported through [`Self::next`] as [`Incoming::FileReceived`].
    ///
    /// `room` is the free storage under `dir` when the caller knows it: an
    /// offer whose remaining bytes would leave less than [`ROOM_RESERVE`]
    /// free is declined to the desktop and refused here (AND-9).
    pub async fn accept_file(
        &self,
        transfer: u32,
        dir: PathBuf,
        room: Option<u64>,
    ) -> Result<impl std::future::Future<Output = ()> + Send + 'static, LinkError> {
        self.gate(capability::SHARE)?;
        let offer = self
            .share
            .offers_in
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&transfer)
            .ok_or_else(|| LinkError::Connection("no such offer".into()))?;
        let Some(name) = safe_filename(&offer.name).map(str::to_owned) else {
            return Err(self
                .decline(transfer, "the offered name is not a file name")
                .await);
        };
        std::fs::create_dir_all(&dir).map_err(LinkError::Io)?;
        let partial = dir.join(format!(".{name}.part"));
        let offset = std::fs::metadata(&partial)
            .map(|m| m.len())
            .unwrap_or(0)
            .min(offer.size);
        if !fits(offer.size, offset, room) {
            return Err(self
                .decline(transfer, "the offered file does not fit on the phone")
                .await);
        }
        // The stream is reserved before the desktop is told to send it, so it
        // reaches this receive in whatever order the transfers' streams come.
        let expected = match self.share.transfers.expect(transfer) {
            Ok(expected) => expected,
            Err(LinkError::Refused(why)) => return Err(self.decline(transfer, why).await),
            Err(e) => return Err(e),
        };
        let (end, ended) = tokio::sync::oneshot::channel();
        self.share
            .receiving
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(transfer, end);
        self.send(
            capability::SHARE,
            ShareAccept::KIND,
            ShareAccept { transfer, offset }.encode(),
        )
        .await?;
        let share = Arc::clone(&self.share);
        let session = Arc::clone(&self.session);
        Ok(async move {
            let complete = receive_into(expected, ended, &partial, offset, offer.size)
                .await
                .unwrap_or(false);
            share
                .receiving
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&transfer);
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

    /// Declines `transfer` to the desktop, best effort, and returns why for
    /// the caller.
    async fn decline(&self, transfer: u32, why: &'static str) -> LinkError {
        let _ = self
            .send(
                capability::SHARE,
                ShareReject::KIND,
                ShareReject { transfer }.encode(),
            )
            .await;
        LinkError::Refused(why)
    }

    /// Declines an offered file.
    pub async fn reject_file(&self, transfer: u32) -> Result<(), LinkError> {
        self.share
            .offers_in
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&transfer);
        self.send(
            capability::SHARE,
            ShareReject::KIND,
            ShareReject { transfer }.encode(),
        )
        .await
    }

    /// Reports one of this phone's players; an empty player name clears it.
    pub async fn send_media_state(&self, state: &MediaState) -> Result<(), LinkError> {
        self.send(capability::MEDIA, MediaState::KIND, state.encode())
            .await
    }

    /// Drives one of the desktop's players.
    pub async fn send_media_command(&self, command: &MediaCommand) -> Result<(), LinkError> {
        self.send(capability::MEDIA, MediaCommand::KIND, command.encode())
            .await
    }

    /// Asks the desktop for its players' states.
    pub async fn request_media(&self) -> Result<(), LinkError> {
        self.send(capability::MEDIA, MediaRequest::KIND, MediaRequest.encode())
            .await
    }

    /// A page of this phone's contacts.
    pub async fn send_contacts(&self, page: &ContactsSync) -> Result<(), LinkError> {
        self.send(capability::CONTACTS, ContactsSync::KIND, page.encode())
            .await
    }

    /// Every conversation, newest first.
    pub async fn send_sms_conversations(&self, list: &SmsConversations) -> Result<(), LinkError> {
        self.send(capability::SMS, SmsConversations::KIND, list.encode())
            .await
    }

    /// A page of one thread, oldest first.
    pub async fn send_sms_thread(&self, page: &SmsThread) -> Result<(), LinkError> {
        self.send(capability::SMS, SmsThread::KIND, page.encode())
            .await
    }

    /// A message arrived or was sent.
    pub async fn send_sms_received(&self, received: &SmsReceived) -> Result<(), LinkError> {
        self.send(capability::SMS, SmsReceived::KIND, received.encode())
            .await
    }

    /// A call changed state.
    pub async fn send_call_event(&self, event: &CallEvent) -> Result<(), LinkError> {
        self.send(capability::TELEPHONY, CallEvent::KIND, event.encode())
            .await
    }

    /// Runs the desktop's registered command `id`.
    pub async fn send_command_run(&self, id: u32) -> Result<(), LinkError> {
        self.send(
            capability::COMMANDS,
            CommandRun::KIND,
            CommandRun { id }.encode(),
        )
        .await
    }

    /// Pointer motion, as a datagram: a late sample is worse than a lost one.
    pub fn send_pointer_move(&self, dx: i16, dy: i16) -> Result<(), LinkError> {
        self.gate(capability::INPUT)?;
        self.session.send_datagram(
            capability::INPUT,
            PointerMove::KIND,
            PointerMove { dx, dy }.encode(),
        )
    }

    pub async fn send_pointer_button(
        &self,
        button: Button,
        pressed: bool,
    ) -> Result<(), LinkError> {
        self.send(
            capability::INPUT,
            PointerButton::KIND,
            PointerButton { button, pressed }.encode(),
        )
        .await
    }

    pub async fn send_scroll(&self, dx: i16, dy: i16) -> Result<(), LinkError> {
        self.send(capability::INPUT, Scroll::KIND, Scroll { dx, dy }.encode())
            .await
    }

    pub async fn send_key(&self, code: u16, pressed: bool) -> Result<(), LinkError> {
        self.send(capability::INPUT, Key::KIND, Key { code, pressed }.encode())
            .await
    }

    pub async fn send_typed_text(&self, text: &str) -> Result<(), LinkError> {
        self.send(
            capability::INPUT,
            Text::KIND,
            Text {
                text: text.to_owned(),
            }
            .encode(),
        )
        .await
    }

    /// Shares a URL or a snippet, no stream needed.
    pub async fn send_text(&self, text: &str) -> Result<(), LinkError> {
        self.send(
            capability::SHARE,
            ShareText::KIND,
            ShareText {
                text: text.to_owned(),
            }
            .encode(),
        )
        .await
    }

    pub fn close(&self, reason: &str) {
        self.session.close(reason);
    }
}

/// Appends one transfer's stream to `partial` from `offset`; true when
/// `size` bytes are there.
async fn receive_into(
    expected: Expected,
    ended: tokio::sync::oneshot::Receiver<()>,
    partial: &PathBuf,
    offset: u64,
    size: u64,
) -> Result<bool, LinkError> {
    use tokio::io::{AsyncSeekExt, AsyncWriteExt};
    // The stream comes within its budget unless the desktop abandons the
    // download first; either way the partial stays for a later resume and
    // the reservation is freed with `expected`.
    let mut stream = tokio::select! {
        stream = tokio::time::timeout(STREAM_WAIT, expected.stream()) => match stream {
            Ok(stream) => stream?,
            Err(_) => return Ok(false),
        },
        _ = ended => return Ok(false),
    };
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

/// Whether an offer of `size` bytes, `offset` of them already held, may be
/// received: within [`MAX_RECEIVED`], and, when the free storage `room` is
/// known, leaving at least [`ROOM_RESERVE`] of it free.
pub fn fits(size: u64, offset: u64, room: Option<u64>) -> bool {
    size <= MAX_RECEIVED
        && room.is_none_or(|room| {
            size.saturating_sub(offset)
                .checked_add(ROOM_RESERVE)
                .is_some_and(|needed| needed <= room)
        })
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

/// The wire's button index, for the application's side of the enum.
pub fn button_index(button: MediaButton) -> u8 {
    match button {
        MediaButton::Play => 0,
        MediaButton::Pause => 1,
        MediaButton::PlayPause => 2,
        MediaButton::Next => 3,
        MediaButton::Previous => 4,
        MediaButton::Stop => 5,
    }
}

pub fn button_from_index(index: u8) -> Option<MediaButton> {
    Some(match index {
        0 => MediaButton::Play,
        1 => MediaButton::Pause,
        2 => MediaButton::PlayPause,
        3 => MediaButton::Next,
        4 => MediaButton::Previous,
        5 => MediaButton::Stop,
        _ => return None,
    })
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
        (capability::MEDIA, 1) => "media: state".into(),
        (capability::MEDIA, 2) => "media: command".into(),
        (capability::MEDIA, 3) => "media: requested".into(),
        (capability::CONTACTS, 1) => "contacts: requested".into(),
        (capability::SMS, 1) => "sms: conversations requested".into(),
        (capability::SMS, 2) => "sms: thread requested".into(),
        (capability::SMS, 4) => "sms: send".into(),
        (capability::TELEPHONY, 2) => "call: command".into(),
        (capability::COMMANDS, 1) => "commands: list".into(),
        (capability::MIRROR, 1) => "mirror: start".into(),
        (capability::MIRROR, 3) => "mirror: stop".into(),
        (capability::MIRROR, 4) => "mirror: touch".into(),
        (capability::MIRROR, 5) => "mirror: key".into(),
        (capability::MIRROR, 6) => "mirror: global".into(),
        (capability::MIRROR, 7) => "mirror: keyframe".into(),
        (capability::STORAGE, 2) => "storage: list".into(),
        (capability::STORAGE, 4) => "storage: stat".into(),
        (capability::STORAGE, 6) => "storage: read".into(),
        (capability::STORAGE, 8) => "storage: write".into(),
        (capability::STORAGE, 10) => "storage: mkdir".into(),
        (capability::STORAGE, 11) => "storage: rename".into(),
        (capability::STORAGE, 12) => "storage: delete".into(),
        (capability::COMMANDS, 3) => "commands: result".into(),
        (cap, kind) => format!("capability {cap} kind {kind}, {} bytes", env.body.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnetita_proto::pair::QrPairing;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "magnetita-mobile-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// A desktop on loopback that offers what this phone offers, pairs the
    /// phone by QR and then hands its endpoint and session to the test.
    async fn paired(dir: &Path) -> (Endpoint, Session, PhoneSession) {
        paired_offering(dir, offered()).await
    }

    /// As [`paired`], with a desktop whose hello offers `capabilities`.
    async fn paired_offering(
        dir: &Path,
        capabilities: Vec<CapabilityVersion>,
    ) -> (Endpoint, Session, PhoneSession) {
        let cert = DeviceCert::generate("desktop");
        let desktop_fp = fingerprint_of(&cert.chain().unwrap()[0]);
        let desktop = Endpoint::bind(
            EndpointConfig {
                cert,
                hello: Hello {
                    device_id: "desktop".into(),
                    device_name: "Celestina".into(),
                    device_kind: DeviceKind::Desktop,
                    capabilities,
                },
            },
            "127.0.0.1:0".parse().unwrap(),
        )
        .unwrap();
        let secret = [7u8; 32];
        let uri = QrPayload {
            device_id: "desktop".into(),
            fingerprint: desktop_fp,
            secret,
            addresses: vec![desktop.local_addr().unwrap().to_string()],
        }
        .to_uri();
        let phone = Phone::open(dir, "slow reader").unwrap();
        let desktop_side = async {
            let incoming = desktop.accept().await.unwrap().handshake().await.unwrap();
            let fp = incoming.peer_fingerprint();
            let (session, _) = desktop
                .admit(incoming, Expect::Fingerprint(fp))
                .await
                .unwrap();
            let mut pairing = QrPairing::desktop(secret, desktop_fp, fp);
            let proof = session.recv().await.unwrap();
            let (reply, _) = pairing.accept_proof(&proof.body).unwrap();
            session
                .send_message(capability::PAIRING, pair_kind::QR_REPLY, reply)
                .await
                .unwrap();
            session
        };
        let (desktop_session, paired) = tokio::join!(desktop_side, phone.pair(&uri));
        (desktop, desktop_session, paired.unwrap().1)
    }

    /// MAG-4: a timeout that fires while a large frame is still arriving
    /// must not lose the bytes already read. Every envelope arrives whole
    /// and in order however short the caller's wait.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_slow_frame_survives_the_callers_timeouts() {
        let dir = scratch("slow");
        let (_endpoint, desktop, phone) = paired(&dir).await;
        const FRAMES: usize = 24;
        let sender = tokio::spawn(async move {
            for n in 0..FRAMES {
                let text = char::from(b'a' + (n % 26) as u8)
                    .to_string()
                    .repeat(200 * 1024);
                desktop
                    .send_message(
                        capability::CLIPBOARD,
                        ClipboardText::KIND,
                        ClipboardText { text }.encode(),
                    )
                    .await
                    .unwrap();
            }
            desktop
        });
        let mut got = Vec::new();
        let until = tokio::time::Instant::now() + Duration::from_secs(30);
        while got.len() < FRAMES && tokio::time::Instant::now() < until {
            match phone.next(Duration::from_millis(1)).await {
                Ok(Some(Incoming::Envelope(env))) => got.push(env),
                Ok(Some(other)) => panic!("unexpected {other:?}"),
                Ok(None) => {}
                Err(e) => panic!("the control stream fell out of step: {e}"),
            }
        }
        assert_eq!(got.len(), FRAMES, "every frame arrives");
        for (n, env) in got.iter().enumerate() {
            let text = ClipboardText::decode(&env.body).unwrap().text;
            assert_eq!(text.len(), 200 * 1024);
            assert!(text.bytes().all(|b| b == b'a' + (n % 26) as u8));
        }
        let _desktop = sender.await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn offering(ids: &[u16]) -> Vec<CapabilityVersion> {
        ids.iter()
            .map(|&capability| CapabilityVersion {
                capability,
                version: 1,
            })
            .collect()
    }

    /// Waits for the next envelope the desktop receives of `capability`.
    async fn desktop_receives(desktop: &Session, capability: u16) -> Envelope {
        loop {
            let env = tokio::time::timeout(Duration::from_secs(5), desktop.recv())
                .await
                .expect("an envelope in time")
                .unwrap();
            if env.capability == capability {
                return env;
            }
        }
    }

    async fn next_incoming(phone: &PhoneSession) -> Incoming {
        phone
            .next(Duration::from_secs(5))
            .await
            .unwrap()
            .expect("something in time")
    }

    /// MAG-8: a desktop that offers battery and find, as an older build of
    /// either side did, keeps both and is refused everything else both ways:
    /// the phone neither sends nor acts on a capability the desktop declined.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_desktop_that_declines_a_capability_is_refused_both_ways() {
        let dir = scratch("declined");
        let (_endpoint, desktop, phone) =
            paired_offering(&dir, offering(&[capability::BATTERY, capability::FIND])).await;
        let agreed: Vec<u16> = phone
            .negotiated()
            .capabilities()
            .iter()
            .map(|c| c.capability)
            .collect();
        assert_eq!(agreed, [capability::BATTERY, capability::FIND]);

        // Outbound: declined before anything is written.
        match phone.send_clipboard("copied").await {
            Err(LinkError::Declined(c)) => assert_eq!(c, capability::CLIPBOARD),
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert!(matches!(
            phone.send_pointer_move(1, 1),
            Err(LinkError::Declined(capability::INPUT))
        ));
        assert!(matches!(
            phone.open_stream(crate::mobile::MIRROR_VIDEO_STREAM).await,
            Err(LinkError::Declined(capability::MIRROR))
        ));
        // What both offered still flows.
        phone.report_battery(40, false).await.unwrap();
        let battery = desktop_receives(&desktop, capability::BATTERY).await;
        assert_eq!(BatteryStatus::decode(&battery.body).unwrap().level, 40);

        // Inbound: a declined capability is refused, never handed on.
        desktop
            .send_message(
                capability::CLIPBOARD,
                ClipboardText::KIND,
                ClipboardText {
                    text: "from the desk".into(),
                }
                .encode(),
            )
            .await
            .unwrap();
        desktop
            .send_message(
                capability::FIND,
                magnetita_proto::daily::find::FindRing::KIND,
                magnetita_proto::daily::find::FindRing.encode(),
            )
            .await
            .unwrap();
        match next_incoming(&phone).await {
            Incoming::Refused {
                capability, reason, ..
            } => {
                assert_eq!(capability, capability::CLIPBOARD);
                assert_eq!(reason, "not negotiated");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        match next_incoming(&phone).await {
            Incoming::Envelope(env) => assert_eq!(env.capability, capability::FIND),
            other => panic!("expected the ring, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn offer(transfer: u32, size: u64) -> Vec<u8> {
        ShareOffer {
            transfer,
            name: format!("f{transfer}.bin"),
            size,
            mime: String::new(),
        }
        .encode()
    }

    /// AND-9: an offer past the payload limit, or past the number that may
    /// wait, is declined to the desktop and refused to the application.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn received_offers_are_bounded() {
        let dir = scratch("offers");
        let (_endpoint, desktop, phone) = paired(&dir).await;
        desktop
            .send_message(
                capability::SHARE,
                ShareOffer::KIND,
                offer(1, MAX_RECEIVED + 1),
            )
            .await
            .unwrap();
        assert!(matches!(
            next_incoming(&phone).await,
            Incoming::Refused {
                capability: capability::SHARE,
                ..
            }
        ));
        let reject = desktop_receives(&desktop, capability::SHARE).await;
        assert_eq!(reject.kind, ShareReject::KIND);
        assert_eq!(ShareReject::decode(&reject.body).unwrap().transfer, 1);

        for transfer in 10..10 + MAX_PENDING_OFFERS as u32 {
            desktop
                .send_message(capability::SHARE, ShareOffer::KIND, offer(transfer, 10))
                .await
                .unwrap();
            assert!(matches!(next_incoming(&phone).await, Incoming::Envelope(_)));
        }
        desktop
            .send_message(capability::SHARE, ShareOffer::KIND, offer(99, 10))
            .await
            .unwrap();
        assert!(matches!(
            next_incoming(&phone).await,
            Incoming::Refused { .. }
        ));
        let reject = desktop_receives(&desktop, capability::SHARE).await;
        assert_eq!(ShareReject::decode(&reject.body).unwrap().transfer, 99);

        // An offer that would not leave the reserve free is declined too.
        let refused = phone
            .accept_file(10, dir.join("in"), Some(ROOM_RESERVE + 9))
            .await
            .map(|_| ());
        assert!(matches!(refused, Err(LinkError::Refused(_))));
        let reject = desktop_receives(&desktop, capability::SHARE).await;
        assert_eq!(ShareReject::decode(&reject.body).unwrap().transfer, 10);
        // Once answered, the offer no longer holds a slot.
        desktop
            .send_message(capability::SHARE, ShareOffer::KIND, offer(100, 10))
            .await
            .unwrap();
        assert!(matches!(next_incoming(&phone).await, Incoming::Envelope(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_fits_under_the_payload_limit_and_the_reserve() {
        assert!(fits(10, 0, None));
        assert!(fits(MAX_RECEIVED, 0, None));
        assert!(!fits(MAX_RECEIVED + 1, 0, None));
        assert!(fits(10, 0, Some(ROOM_RESERVE + 10)));
        assert!(!fits(10, 0, Some(ROOM_RESERVE + 9)));
        // Only the bytes still to come need room.
        assert!(fits(10, 4, Some(ROOM_RESERVE + 6)));
        assert!(!fits(10, 0, Some(0)));
    }

    /// MAG-19: the phone refuses what the wire's clipboard rule refuses
    /// before it is sent, so it can say so.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn clipboard_text_the_rule_refuses_is_not_sent() {
        let dir = scratch("clip");
        let (_endpoint, _desktop, phone) = paired(&dir).await;
        let over = "x".repeat(magnetita_proto::bound::MAX_CLIPBOARD + 1);
        for text in ["", "a\0b", over.as_str()] {
            assert!(matches!(
                phone.send_clipboard(text).await,
                Err(LinkError::Refused(_))
            ));
        }
        phone.send_clipboard("fine").await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An upload whose source fails is abandoned: the desktop is told, and
    /// the session goes on.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_abandoned_upload_tells_the_desktop_and_keeps_the_session() {
        let dir = scratch("abandon");
        let (_endpoint, desktop, phone) = paired(&dir).await;
        let transfer = phone.offer_file("a.bin", 100, "").await.unwrap();
        let offered = desktop_receives(&desktop, capability::SHARE).await;
        assert_eq!(offered.kind, ShareOffer::KIND);
        desktop
            .send_message(
                capability::SHARE,
                ShareAccept::KIND,
                ShareAccept {
                    transfer,
                    offset: 0,
                }
                .encode(),
            )
            .await
            .unwrap();
        assert!(matches!(next_incoming(&phone).await, Incoming::Envelope(_)));
        phone.abandon_transfer(transfer).await.unwrap();
        let done = desktop_receives(&desktop, capability::SHARE).await;
        assert_eq!(done.kind, ShareDone::KIND);
        assert_eq!(
            ShareDone::decode(&done.body).unwrap(),
            ShareDone {
                transfer,
                complete: false
            }
        );
        assert!(!phone.is_closed());
        // The abandoned stream ends short rather than reset: even a desktop
        // that stops at a reset stream reads it as a short transfer.
        let transfers = desktop.transfers();
        let accept = || async {
            tokio::time::timeout(Duration::from_secs(5), transfers.accept_or_skip())
                .await
                .expect("a bulk stream in time")
                .unwrap()
        };
        match accept().await {
            magnetita_link::Accepted::Stream(id, mut stream) => {
                assert_eq!(id, transfer);
                assert!(stream.read_to_end(1024).await.unwrap().is_empty());
            }
            magnetita_link::Accepted::Skipped(why) => {
                panic!("the abandoned stream was reset: {why}")
            }
        }
        // A later upload on the same session reaches the desktop whole.
        let later = phone.offer_file("b.bin", 3, "").await.unwrap();
        let _ = desktop_receives(&desktop, capability::SHARE).await;
        desktop
            .send_message(
                capability::SHARE,
                ShareAccept::KIND,
                ShareAccept {
                    transfer: later,
                    offset: 0,
                }
                .encode(),
            )
            .await
            .unwrap();
        assert!(matches!(next_incoming(&phone).await, Incoming::Envelope(_)));
        phone.write_transfer(later, b"abc").await.unwrap();
        phone.finish_transfer(later).await.unwrap();
        match accept().await {
            magnetita_link::Accepted::Stream(id, mut stream) => {
                assert_eq!(id, later);
                assert_eq!(stream.read_to_end(1024).await.unwrap(), b"abc");
            }
            magnetita_link::Accepted::Skipped(why) => panic!("the later stream was skipped: {why}"),
        }
        phone.report_battery(10, false).await.unwrap();
        phone.close("done");
        assert!(phone.is_closed());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two files received at once, their streams arriving in the opposite
    /// order of the offers: each receive gets its own stream and both finish.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_downloads_each_get_their_own_stream() {
        let dir = scratch("concurrent");
        let (_endpoint, desktop, phone) = paired(&dir).await;
        for (transfer, name) in [(1u32, "one.bin"), (2, "two.bin")] {
            desktop
                .send_message(
                    capability::SHARE,
                    ShareOffer::KIND,
                    ShareOffer {
                        transfer,
                        name: name.into(),
                        size: 4,
                        mime: String::new(),
                    }
                    .encode(),
                )
                .await
                .unwrap();
            assert!(matches!(next_incoming(&phone).await, Incoming::Envelope(_)));
        }
        for transfer in [1u32, 2] {
            let receive = phone
                .accept_file(transfer, dir.join("in"), None)
                .await
                .unwrap();
            tokio::spawn(receive);
        }
        for _ in 0..2 {
            assert_eq!(
                desktop_receives(&desktop, capability::SHARE).await.kind,
                ShareAccept::KIND
            );
        }
        // Transfer 2's stream first, then transfer 1's.
        for transfer in [2u32, 1] {
            let mut stream = desktop.open_transfer(transfer).await.unwrap();
            stream.write_all(&[transfer as u8; 4]).await.unwrap();
            stream.finish().unwrap();
        }
        let mut done = Vec::new();
        while done.len() < 2 {
            if let Incoming::FileReceived {
                transfer,
                path,
                complete,
            } = next_incoming(&phone).await
            {
                assert!(complete, "transfer {transfer} finished whole");
                assert_eq!(std::fs::read(&path).unwrap(), vec![transfer as u8; 4]);
                done.push(transfer);
            }
        }
        done.sort_unstable();
        assert_eq!(done, [1, 2]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    async fn offer_and_accept(desktop: &Session, phone: &PhoneSession, dir: &Path, transfer: u32) {
        desktop
            .send_message(
                capability::SHARE,
                ShareOffer::KIND,
                ShareOffer {
                    transfer,
                    name: "late.bin".into(),
                    size: 4,
                    mime: String::new(),
                }
                .encode(),
            )
            .await
            .unwrap();
        assert!(matches!(next_incoming(phone).await, Incoming::Envelope(_)));
        tokio::spawn(
            phone
                .accept_file(transfer, dir.join("in"), None)
                .await
                .unwrap(),
        );
        // The phone's report of an earlier, ended download may come first.
        while desktop_receives(desktop, capability::SHARE).await.kind != ShareAccept::KIND {}
    }

    /// The desktop abandons a download before opening its stream (its file
    /// could not be read): the phone ends that download at once and frees
    /// its reservation, so the same transfer can be offered again.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_download_the_desktop_abandons_ends_at_once() {
        let dir = scratch("abandoned-download");
        let (_endpoint, desktop, phone) = paired(&dir).await;
        offer_and_accept(&desktop, &phone, &dir, 4).await;
        desktop
            .send_message(
                capability::SHARE,
                ShareDone::KIND,
                ShareDone {
                    transfer: 4,
                    complete: false,
                }
                .encode(),
            )
            .await
            .unwrap();
        let started = std::time::Instant::now();
        loop {
            match tokio::time::timeout(STREAM_WAIT / 2, phone.next(Duration::from_secs(5)))
                .await
                .expect("the download ended before its stream budget")
                .unwrap()
                .expect("something")
            {
                Incoming::FileReceived {
                    transfer, complete, ..
                } => {
                    assert_eq!((transfer, complete), (4, false));
                    break;
                }
                _ => continue,
            }
        }
        assert!(started.elapsed() < STREAM_WAIT);
        // The reservation is free: the same transfer is accepted again.
        offer_and_accept(&desktop, &phone, &dir, 4).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A download whose stream never comes ends incomplete after its
    /// budget; its reservation is freed.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_download_whose_stream_never_comes_ends_after_its_budget() {
        let dir = scratch("silent-download");
        let (_endpoint, desktop, phone) = paired(&dir).await;
        offer_and_accept(&desktop, &phone, &dir, 6).await;
        let ended = tokio::time::timeout(STREAM_WAIT * 3, async {
            loop {
                if let Some(Incoming::FileReceived {
                    transfer, complete, ..
                }) = phone.next(Duration::from_millis(200)).await.unwrap()
                {
                    break (transfer, complete);
                }
            }
        })
        .await
        .expect("the download ended after its budget");
        assert_eq!(ended, (6, false));
        offer_and_accept(&desktop, &phone, &dir, 6).await;
        let _ = std::fs::remove_dir_all(&dir);
    }
}
