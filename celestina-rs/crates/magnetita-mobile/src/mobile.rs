//! The UniFFI face: one object per Rust object, blocking calls Kotlin runs
//! off its main thread, and records instead of tuples.
//!
//! The runtime lives inside [`MobilePhone`], so a Kotlin caller never sees
//! tokio. Every method blocks on that runtime; the application calls them
//! from its foreground service's coroutine, not from the UI thread.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use magnetita_link::trust::fingerprint_text;

use crate::phone::{button_from_index, describe, Incoming, Phone, PhoneSession};
use crate::signal::{signal_of, DesktopSignal};
use crate::storage;
use magnetita_link::LinkError;
use magnetita_proto::control::input::Button;
use magnetita_proto::daily::media::{MediaButton, MediaCommand, MediaState};
use magnetita_proto::daily::notifications::{Action, NotificationPosted};
use magnetita_proto::mirror::{Codec, MirrorStarted};
use magnetita_proto::phone::contacts::{Contact, ContactsSync};
use magnetita_proto::phone::sms::{
    Attachment, Conversation, SmsConversations, SmsMessage, SmsReceived, SmsThread,
};
use magnetita_proto::phone::telephony::{CallEvent, CallState};
use magnetita_proto::storage::Entry;

/// Why a call failed, as Kotlin sees it.
#[derive(Debug, PartialEq, Eq, uniffi::Error)]
#[uniffi(flat_error)]
pub enum MobileError {
    /// The link failed: a connection, a handshake, the pairing.
    Link(String),
    /// The desktop did not negotiate the capability this call needs.
    Declined(String),
    /// A bound of this phone refused the call before anything was sent.
    Refused(String),
    /// A small integer code Kotlin passed names nothing; it is refused, never
    /// mapped to the nearest value.
    UnknownCode(String),
}

impl std::fmt::Display for MobileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Link(m) | Self::Declined(m) | Self::Refused(m) | Self::UnknownCode(m) => {
                f.write_str(m)
            }
        }
    }
}

impl std::error::Error for MobileError {}

impl From<LinkError> for MobileError {
    fn from(e: LinkError) -> Self {
        match e {
            LinkError::Declined(_) => Self::Declined(e.to_string()),
            LinkError::Refused(_) => Self::Refused(e.to_string()),
            other => Self::Link(other.to_string()),
        }
    }
}

fn unknown(what: &str, code: u8) -> MobileError {
    MobileError::UnknownCode(format!("unknown {what} code {code}"))
}

/// 0 ringing, 1 answered, 2 missed, 3 ended.
fn call_state(code: u8) -> Result<CallState, MobileError> {
    Ok(match code {
        0 => CallState::Ringing,
        1 => CallState::Answered,
        2 => CallState::Missed,
        3 => CallState::Ended,
        _ => return Err(unknown("call state", code)),
    })
}

/// 0 left, 1 right, 2 middle.
fn pointer_button(code: u8) -> Result<Button, MobileError> {
    Ok(match code {
        0 => Button::Left,
        1 => Button::Right,
        2 => Button::Middle,
        _ => return Err(unknown("pointer button", code)),
    })
}

/// 0 HEVC, 1 H.264.
fn codec_of(code: u8) -> Result<Codec, MobileError> {
    Ok(match code {
        0 => Codec::Hevc,
        1 => Codec::H264,
        _ => return Err(unknown("codec", code)),
    })
}

/// 0 play, 1 pause, 2 play/pause, 3 next, 4 previous, 5 stop.
fn media_button(code: u8) -> Result<MediaButton, MobileError> {
    button_from_index(code).ok_or_else(|| unknown("media button", code))
}

/// Whether `text` may go to the desktop as clipboard: the wire's one rule
/// (MAG-19), so the application shows a refusal instead of "sent".
#[uniffi::export]
pub fn clipboard_syncable(text: String) -> bool {
    magnetita_proto::daily::clipboard::syncable(&text)
}

/// Who this phone is.
#[derive(uniffi::Record)]
pub struct Identity {
    pub device_id: String,
    pub name: String,
    /// Colon-separated hex SHA-256 of the certificate.
    pub fingerprint: String,
}

/// A desktop this phone has pinned.
#[derive(uniffi::Record)]
pub struct PinnedDesktop {
    pub device_id: String,
    pub name: String,
    pub fingerprint: String,
}

/// One envelope from the desktop: a line for the log and what it asks.
#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct DesktopEvent {
    pub description: String,
    pub signal: DesktopSignal,
}

/// One storage request: `kind` 0 list, 1 stat, 2 read, 3 write, 4 mkdir,
/// 5 rename (`to` set), 6 delete. `offset` is the first entry for a list and
/// the byte for a read or write; `len` a list's page size or a read's
/// length; `bytes` the write's. Paths were checked at the wire.
#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct MobileStorageRequest {
    pub kind: u8,
    pub request: u32,
    pub path: String,
    pub to: String,
    pub offset: u64,
    pub len: u32,
    pub bytes: Vec<u8>,
    pub truncate: bool,
}

/// One file or directory as the wire carries it.
#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct MobileEntry {
    pub name: String,
    pub dir: bool,
    pub size: u64,
    pub mtime_ms: u64,
}

fn entry_from(e: MobileEntry) -> Entry {
    Entry {
        name: e.name,
        dir: e.dir,
        size: e.size,
        mtime_ms: e.mtime_ms,
    }
}

/// What the desktop asks the mirror to be.
#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct MobileMirrorStart {
    pub max_size: u16,
    pub fps: u8,
    pub bitrate_kbps: u32,
    /// 0 HEVC, 1 H.264.
    pub codec: u8,
    pub audio: bool,
    pub screen_off: bool,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct MobileTouch {
    pub phase: u8,
    pub x: u16,
    pub y: u16,
    pub pointer: u8,
}

/// The transfer ids the mirror's streams carry.
pub const MIRROR_VIDEO_STREAM: u32 = 0xFFFF_0001;
pub const MIRROR_AUDIO_STREAM: u32 = 0xFFFF_0002;

/// One registered command as the phone sees it.
#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct MobileCommand {
    pub id: u32,
    pub name: String,
}

/// One contact as its vCard.
#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct MobileContact {
    pub id: u64,
    pub version: u64,
    pub vcard: String,
}

/// One conversation in the list.
#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct MobileConversation {
    pub thread: u64,
    pub addresses: Vec<String>,
    pub snippet: String,
    pub timestamp_ms: u64,
    pub unread: u16,
}

/// One message; attachments are counted by the mime types they carry.
#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct MobileSmsMessage {
    pub id: u64,
    pub from_me: bool,
    pub address: String,
    pub body: String,
    pub timestamp_ms: u64,
    pub attachment_mimes: Vec<String>,
}

fn message_out(m: MobileSmsMessage) -> SmsMessage {
    SmsMessage {
        id: m.id,
        from_me: m.from_me,
        address: m.address,
        body: m.body,
        timestamp_ms: m.timestamp_ms,
        attachments: m
            .attachment_mimes
            .into_iter()
            .map(|mime| Attachment { transfer: 0, mime })
            .collect(),
    }
}

/// One player's state, either side's.
#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct MobileMediaState {
    pub player: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub position_ms: u64,
    pub length_ms: u64,
    pub can_seek: bool,
    pub can_next: bool,
    pub can_previous: bool,
    pub volume: u8,
}

/// A button (0 play, 1 pause, 2 play/pause, 3 next, 4 previous, 5 stop),
/// a seek or a volume for one player.
#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct MobileMediaCommand {
    pub player: String,
    pub button: Option<u8>,
    pub seek_ms: Option<u64>,
    pub volume: Option<u8>,
}

fn media_state_out(s: MobileMediaState) -> MediaState {
    MediaState {
        player: s.player,
        title: s.title,
        artist: s.artist,
        album: s.album,
        playing: s.playing,
        position_ms: s.position_ms,
        length_ms: s.length_ms,
        can_seek: s.can_seek,
        can_next: s.can_next,
        can_previous: s.can_previous,
        volume: s.volume,
    }
}

/// One of the phone's notifications, as the listener sees it.
#[derive(uniffi::Record)]
pub struct MobileNotification {
    pub key: String,
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub timestamp_ms: u64,
    pub replyable: bool,
    pub actions: Vec<String>,
    /// PNG bytes, absent when none or unchanged.
    pub icon: Option<Vec<u8>>,
    /// A player's now-playing notification.
    pub media: bool,
}

/// The phone, held by the application's service for its lifetime.
#[derive(uniffi::Object)]
pub struct MobilePhone {
    runtime: tokio::runtime::Runtime,
    phone: Phone,
}

#[uniffi::export]
impl MobilePhone {
    /// Opens the identity under `dir` (the app's files directory).
    #[uniffi::constructor]
    pub fn open(dir: String, name: String) -> Result<Arc<Self>, MobileError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| MobileError::Link(e.to_string()))?;
        let phone = runtime.block_on(async { Phone::open(&PathBuf::from(dir), &name) })?;
        Ok(Arc::new(Self { runtime, phone }))
    }

    pub fn identity(&self) -> Identity {
        Identity {
            device_id: self.phone.device_id.clone(),
            name: self.phone.name.clone(),
            fingerprint: fingerprint_text(&self.phone.fingerprint),
        }
    }

    pub fn pinned(&self) -> Vec<PinnedDesktop> {
        self.phone
            .pinned()
            .into_iter()
            .map(|p| PinnedDesktop {
                device_id: p.device_id,
                name: p.device_name,
                fingerprint: p.fingerprint,
            })
            .collect()
    }

    pub fn forget(&self, device_id: String) -> Result<(), MobileError> {
        Ok(self.phone.forget(&device_id)?)
    }

    /// Pairs with the desktop a scanned QR names; the session stays open.
    pub fn pair(&self, uri: String) -> Result<Arc<MobileSession>, MobileError> {
        let (_, session) = self.runtime.block_on(self.phone.pair(&uri))?;
        Ok(Arc::new(MobileSession {
            handle: self.runtime.handle().clone(),
            inner: session,
        }))
    }

    /// Connects to a pinned desktop at `address` (`ip:port`).
    pub fn connect(&self, address: String) -> Result<Arc<MobileSession>, MobileError> {
        let address = address
            .parse()
            .map_err(|_| MobileError::Link("bad address".into()))?;
        let session = self.runtime.block_on(self.phone.connect(address))?;
        Ok(Arc::new(MobileSession {
            handle: self.runtime.handle().clone(),
            inner: session,
        }))
    }
}

/// One open session with a desktop.
#[derive(uniffi::Object)]
pub struct MobileSession {
    handle: tokio::runtime::Handle,
    inner: PhoneSession,
}

#[uniffi::export]
impl MobileSession {
    pub fn desktop_id(&self) -> String {
        self.inner.desktop.device_id.clone()
    }

    pub fn desktop_name(&self) -> String {
        self.inner.desktop.device_name.clone()
    }

    pub fn report_battery(&self, level: u8, charging: bool) -> Result<(), MobileError> {
        Ok(self
            .handle
            .block_on(self.inner.report_battery(level, charging))?)
    }

    /// Sends the phone's clipboard text.
    pub fn send_clipboard(&self, text: String) -> Result<(), MobileError> {
        Ok(self.handle.block_on(self.inner.send_clipboard(&text))?)
    }

    /// A notification appeared or changed on the phone.
    pub fn send_notification(&self, note: MobileNotification) -> Result<(), MobileError> {
        let posted = NotificationPosted {
            key: note.key,
            app_name: note.app_name,
            title: note.title,
            body: note.body,
            timestamp_ms: note.timestamp_ms,
            replyable: note.replyable,
            actions: note
                .actions
                .into_iter()
                .map(|label| Action { label })
                .collect(),
            icon: note.icon,
            media: note.media,
        };
        Ok(self
            .handle
            .block_on(self.inner.send_notification(&posted))?)
    }

    /// A notification left the phone.
    pub fn send_notification_gone(&self, key: String) -> Result<(), MobileError> {
        Ok(self
            .handle
            .block_on(self.inner.send_notification_gone(&key))?)
    }

    /// Offers a file; wait for the accepted event before writing.
    pub fn offer_file(&self, name: String, size: u64, mime: String) -> Result<u32, MobileError> {
        Ok(self
            .handle
            .block_on(self.inner.offer_file(&name, size, &mime))?)
    }

    /// Writes the next bytes of an accepted transfer.
    pub fn write_transfer(&self, transfer: u32, bytes: Vec<u8>) -> Result<(), MobileError> {
        Ok(self
            .handle
            .block_on(self.inner.write_transfer(transfer, &bytes))?)
    }

    /// Ends an accepted transfer.
    pub fn finish_transfer(&self, transfer: u32) -> Result<(), MobileError> {
        Ok(self.handle.block_on(self.inner.finish_transfer(transfer))?)
    }

    /// Accepts an offered file into `dir`, where `usable_bytes` are free; its
    /// end arrives as [`DesktopSignal::FileReceived`]. An offer that would not
    /// fit is declined to the desktop and refused here.
    pub fn accept_file(
        &self,
        transfer: u32,
        dir: String,
        usable_bytes: u64,
    ) -> Result<(), MobileError> {
        let task = self.handle.block_on(self.inner.accept_file(
            transfer,
            PathBuf::from(dir),
            Some(usable_bytes),
        ))?;
        self.handle.spawn(task);
        Ok(())
    }

    /// Gives up an accepted transfer whose source failed; the desktop is
    /// told it was abandoned.
    pub fn abandon_transfer(&self, transfer: u32) -> Result<(), MobileError> {
        Ok(self
            .handle
            .block_on(self.inner.abandon_transfer(transfer))?)
    }

    /// Whether the session has ended. A call that failed while this is
    /// false was refused, and the session goes on.
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    pub fn reject_file(&self, transfer: u32) -> Result<(), MobileError> {
        Ok(self.handle.block_on(self.inner.reject_file(transfer))?)
    }

    /// Reports one of this phone's players; an empty player name clears it.
    pub fn send_media_state(&self, state: MobileMediaState) -> Result<(), MobileError> {
        Ok(self
            .handle
            .block_on(self.inner.send_media_state(&media_state_out(state)))?)
    }

    /// Drives one of the desktop's players.
    pub fn send_media_command(&self, command: MobileMediaCommand) -> Result<(), MobileError> {
        let command = MediaCommand {
            player: command.player,
            button: command.button.map(media_button).transpose()?,
            seek_ms: command.seek_ms,
            volume: command.volume,
        };
        Ok(self
            .handle
            .block_on(self.inner.send_media_command(&command))?)
    }

    /// Asks the desktop for its players' states.
    pub fn request_media(&self) -> Result<(), MobileError> {
        Ok(self.handle.block_on(self.inner.request_media())?)
    }

    /// A page of this phone's contacts changed since the desktop's version.
    pub fn send_contacts(
        &self,
        version: u64,
        contacts: Vec<MobileContact>,
        removed: Vec<u64>,
        complete: bool,
    ) -> Result<(), MobileError> {
        let page = ContactsSync {
            version,
            contacts: contacts
                .into_iter()
                .map(|c| Contact {
                    id: c.id,
                    version: c.version,
                    vcard: c.vcard,
                })
                .collect(),
            removed,
            complete,
        };
        Ok(self.handle.block_on(self.inner.send_contacts(&page))?)
    }

    /// Every conversation, newest first.
    pub fn send_sms_conversations(&self, list: Vec<MobileConversation>) -> Result<(), MobileError> {
        let list = SmsConversations {
            conversations: list
                .into_iter()
                .map(|c| Conversation {
                    thread: c.thread,
                    addresses: c.addresses,
                    snippet: c.snippet,
                    timestamp_ms: c.timestamp_ms,
                    unread: c.unread,
                })
                .collect(),
        };
        Ok(self
            .handle
            .block_on(self.inner.send_sms_conversations(&list))?)
    }

    /// A page of one thread, oldest first.
    pub fn send_sms_thread(
        &self,
        thread: u64,
        messages: Vec<MobileSmsMessage>,
    ) -> Result<(), MobileError> {
        let page = SmsThread {
            thread,
            messages: messages.into_iter().map(message_out).collect(),
        };
        Ok(self.handle.block_on(self.inner.send_sms_thread(&page))?)
    }

    /// A message arrived or was sent.
    pub fn send_sms_received(
        &self,
        thread: u64,
        message: MobileSmsMessage,
    ) -> Result<(), MobileError> {
        let received = SmsReceived {
            thread,
            message: message_out(message),
        };
        Ok(self
            .handle
            .block_on(self.inner.send_sms_received(&received))?)
    }

    /// A call changed state: 0 ringing, 1 answered, 2 missed, 3 ended.
    pub fn send_call_event(
        &self,
        state: u8,
        number: String,
        name: Option<String>,
        timestamp_ms: u64,
    ) -> Result<(), MobileError> {
        let state = call_state(state)?;
        let event = CallEvent {
            state,
            number,
            name,
            timestamp_ms,
        };
        Ok(self.handle.block_on(self.inner.send_call_event(&event))?)
    }

    /// Runs the desktop's registered command `id`.
    pub fn run_command(&self, id: u32) -> Result<(), MobileError> {
        Ok(self.handle.block_on(self.inner.send_command_run(id))?)
    }

    /// Pointer motion, unreliable and cheap.
    pub fn pointer_move(&self, dx: i16, dy: i16) -> Result<(), MobileError> {
        Ok(self.inner.send_pointer_move(dx, dy)?)
    }

    /// 0 left, 1 right, 2 middle.
    pub fn pointer_button(&self, button: u8, pressed: bool) -> Result<(), MobileError> {
        let button = pointer_button(button)?;
        Ok(self
            .handle
            .block_on(self.inner.send_pointer_button(button, pressed))?)
    }

    /// Scroll in 1/120 wheel steps.
    pub fn scroll(&self, dx: i16, dy: i16) -> Result<(), MobileError> {
        Ok(self.handle.block_on(self.inner.send_scroll(dx, dy))?)
    }

    /// A Linux evdev key code, down or up.
    pub fn key(&self, code: u16, pressed: bool) -> Result<(), MobileError> {
        Ok(self.handle.block_on(self.inner.send_key(code, pressed))?)
    }

    /// Types text on the desktop.
    pub fn type_text(&self, text: String) -> Result<(), MobileError> {
        Ok(self.handle.block_on(self.inner.send_typed_text(&text))?)
    }

    /// Opens the mirror's video (or audio) stream for `write_transfer`.
    pub fn open_stream(&self, id: u32) -> Result<(), MobileError> {
        Ok(self.handle.block_on(self.inner.open_stream(id))?)
    }

    /// Ends a stream opened by `open_stream`.
    pub fn close_stream(&self, id: u32) {
        self.handle.block_on(self.inner.close_stream(id));
    }

    /// The mirror streams with this shape; codec 0 HEVC, 1 H.264.
    pub fn send_mirror_started(
        &self,
        width: u16,
        height: u16,
        codec: u8,
        audio: bool,
    ) -> Result<(), MobileError> {
        let started = MirrorStarted {
            width,
            height,
            codec: codec_of(codec)?,
            audio,
        };
        Ok(self
            .handle
            .block_on(self.inner.send_mirror_started(&started))?)
    }

    pub fn send_mirror_stop(&self) -> Result<(), MobileError> {
        Ok(self.handle.block_on(self.inner.send_mirror_stop())?)
    }

    /// Whether a root is shared; sent on connect and when the grant changes.
    pub fn send_storage_state(&self, available: bool) -> Result<(), MobileError> {
        Ok(self
            .handle
            .block_on(self.inner.send_storage(storage::state(available)))?)
    }

    /// A page of a directory; `error` non-empty when the listing failed.
    pub fn send_listing(
        &self,
        request: u32,
        entries: Vec<MobileEntry>,
        more: bool,
        error: String,
    ) -> Result<(), MobileError> {
        let entries = entries.into_iter().map(entry_from).collect();
        Ok(self.handle.block_on(
            self.inner
                .send_storage(storage::listing(request, entries, more, &error)),
        )?)
    }

    pub fn send_stat_reply(
        &self,
        request: u32,
        entry: Option<MobileEntry>,
    ) -> Result<(), MobileError> {
        Ok(self.handle.block_on(
            self.inner
                .send_storage(storage::stat_reply(request, entry.map(entry_from))),
        )?)
    }

    pub fn send_data(
        &self,
        request: u32,
        bytes: Vec<u8>,
        error: String,
    ) -> Result<(), MobileError> {
        Ok(self.handle.block_on(
            self.inner
                .send_storage(storage::data(request, bytes, &error)),
        )?)
    }

    pub fn send_done(&self, request: u32, ok: bool, error: String) -> Result<(), MobileError> {
        Ok(self
            .handle
            .block_on(self.inner.send_storage(storage::done(request, ok, &error)))?)
    }

    /// Shares a URL or a snippet with the desktop.
    pub fn send_text(&self, text: String) -> Result<(), MobileError> {
        Ok(self.handle.block_on(self.inner.send_text(&text))?)
    }

    /// Blocks up to `timeout_ms` for what the desktop sends next; `None` on
    /// timeout.
    pub fn next(&self, timeout_ms: u64) -> Result<Option<DesktopEvent>, MobileError> {
        let incoming = self
            .handle
            .block_on(self.inner.next(Duration::from_millis(timeout_ms)))?;
        Ok(incoming.map(event_of))
    }

    pub fn close(&self, reason: String) {
        self.inner.close(&reason);
    }
}

/// What one [`Incoming`] asks of the application.
fn event_of(incoming: Incoming) -> DesktopEvent {
    match incoming {
        Incoming::Envelope(env) => DesktopEvent {
            description: describe(&env),
            signal: signal_of(&env),
        },
        Incoming::FileReceived {
            transfer,
            path,
            complete,
        } => DesktopEvent {
            description: format!("share: received {}", path.display()),
            signal: DesktopSignal::FileReceived {
                transfer,
                path: path.to_string_lossy().into_owned(),
                complete,
            },
        },
        Incoming::Refused {
            capability,
            kind,
            reason,
        } => DesktopEvent {
            description: format!("capability {capability} kind {kind} refused: {reason}"),
            signal: DesktopSignal::Declined {
                capability,
                kind,
                reason: reason.to_owned(),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnetita_link::endpoint::Expect;
    use magnetita_link::{fingerprint_of, DeviceCert, Endpoint, EndpointConfig, Session};
    use magnetita_proto::daily::clipboard::ClipboardText;
    use magnetita_proto::mirror::MirrorStop;
    use magnetita_proto::pair::{kind as pair_kind, QrPairing, QrPayload};
    use magnetita_proto::{capability, CapabilityVersion, DeviceKind, Hello};

    /// MAG-28: a code Kotlin passes that names nothing is refused with a
    /// typed error, never mapped to the nearest value.
    #[test]
    fn unknown_ffi_codes_are_refused() {
        assert_eq!(call_state(3), Ok(CallState::Ended));
        assert_eq!(pointer_button(2), Ok(Button::Middle));
        assert_eq!(codec_of(1), Ok(Codec::H264));
        assert_eq!(media_button(5), Ok(MediaButton::Stop));
        assert_eq!(
            call_state(4),
            Err(MobileError::UnknownCode("unknown call state code 4".into()))
        );
        assert!(matches!(
            pointer_button(3),
            Err(MobileError::UnknownCode(_))
        ));
        assert!(matches!(codec_of(2), Err(MobileError::UnknownCode(_))));
        assert!(matches!(media_button(6), Err(MobileError::UnknownCode(_))));
        for code in [u8::MAX, 200] {
            assert!(call_state(code).is_err());
            assert!(pointer_button(code).is_err());
            assert!(codec_of(code).is_err());
            assert!(media_button(code).is_err());
        }
    }

    #[test]
    fn link_refusals_reach_kotlin_as_their_own_errors() {
        assert!(matches!(
            MobileError::from(LinkError::Declined(2)),
            MobileError::Declined(_)
        ));
        assert!(matches!(
            MobileError::from(LinkError::Refused("too big")),
            MobileError::Refused(_)
        ));
        assert!(matches!(
            MobileError::from(LinkError::Closed),
            MobileError::Link(_)
        ));
    }

    #[test]
    fn what_the_session_refused_or_received_reaches_kotlin_typed() {
        let refused = event_of(Incoming::Refused {
            capability: 9,
            kind: 3,
            reason: "not negotiated",
        });
        assert_eq!(
            refused.signal,
            DesktopSignal::Declined {
                capability: 9,
                kind: 3,
                reason: "not negotiated".into()
            }
        );
        let received = event_of(Incoming::FileReceived {
            transfer: 4,
            path: PathBuf::from("/x/a.jpg"),
            complete: true,
        });
        assert_eq!(
            received.signal,
            DesktopSignal::FileReceived {
                transfer: 4,
                path: "/x/a.jpg".into(),
                complete: true
            }
        );
    }

    /// A desktop on its own runtime that offers `capabilities`, pairs the
    /// phone whose QR it prints and hands its session back.
    fn desktop(
        rt: &tokio::runtime::Runtime,
        capabilities: Vec<CapabilityVersion>,
    ) -> (String, tokio::task::JoinHandle<(Endpoint, Session)>) {
        let cert = DeviceCert::generate("desktop");
        let desktop_fp = fingerprint_of(&cert.chain().unwrap()[0]);
        let endpoint = rt.block_on(async {
            Endpoint::bind(
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
            .unwrap()
        });
        let secret = [9u8; 32];
        let uri = QrPayload {
            device_id: "desktop".into(),
            fingerprint: desktop_fp,
            secret,
            addresses: vec![endpoint.local_addr().unwrap().to_string()],
        }
        .to_uri();
        let task = rt.spawn(async move {
            let incoming = endpoint.accept().await.unwrap().handshake().await.unwrap();
            let fp = incoming.peer_fingerprint();
            let (session, _) = endpoint
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
            (endpoint, session)
        });
        (uri, task)
    }

    /// The FFI face end to end over loopback, against a desktop that offers
    /// only clipboard and battery: typed events, the declined capability
    /// refused both ways, and an unknown code refused.
    #[test]
    fn the_ffi_session_gates_on_what_the_desktop_offered() {
        let dir = std::env::temp_dir().join(format!("magnetita-ffi-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let offered = [capability::CLIPBOARD, capability::BATTERY]
            .into_iter()
            .map(|capability| CapabilityVersion {
                capability,
                version: 1,
            })
            .collect();
        let (uri, desktop) = desktop(&rt, offered);
        let phone = MobilePhone::open(dir.to_string_lossy().into_owned(), "ffi".into()).unwrap();
        let session = phone.pair(uri).unwrap();
        let (_endpoint, desktop) = rt.block_on(desktop).unwrap();

        rt.block_on(async {
            desktop
                .send_message(
                    capability::CLIPBOARD,
                    ClipboardText::KIND,
                    ClipboardText { text: "hi".into() }.encode(),
                )
                .await
                .unwrap();
            desktop
                .send_message(capability::MIRROR, MirrorStop::KIND, MirrorStop.encode())
                .await
                .unwrap();
        });
        let first = session.next(5_000).unwrap().unwrap();
        assert_eq!(
            first.signal,
            DesktopSignal::ClipboardText { text: "hi".into() }
        );
        assert_eq!(first.description, "clipboard: 5 bytes");
        let second = session.next(5_000).unwrap().unwrap();
        assert!(matches!(
            second.signal,
            DesktopSignal::Declined {
                capability: capability::MIRROR,
                ..
            }
        ));

        session.send_clipboard("from the phone".into()).unwrap();
        session.report_battery(50, true).unwrap();
        assert!(matches!(
            session.run_command(1),
            Err(MobileError::Declined(_))
        ));
        assert!(matches!(
            session.pointer_button(7, true),
            Err(MobileError::UnknownCode(_))
        ));
        assert!(matches!(
            session.send_clipboard(String::new()),
            Err(MobileError::Refused(_))
        ));
        // Refusals leave the session open; only its end is an end.
        assert!(!session.is_closed());
        session.close("done".into());
        assert!(session.is_closed());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
