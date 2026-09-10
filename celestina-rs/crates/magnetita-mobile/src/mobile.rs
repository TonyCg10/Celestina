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

use crate::phone::{
    button_from_index, button_index, clipboard_text, describe, media_fields, notification_fields,
    phone_fields, share_fields, Incoming, Phone, PhoneSession,
};
use magnetita_proto::daily::media::{MediaCommand, MediaState};
use magnetita_proto::daily::notifications::{Action, NotificationPosted};
use magnetita_proto::phone::contacts::{Contact, ContactsSync};
use magnetita_proto::phone::sms::{
    Attachment, Conversation, SmsConversations, SmsMessage, SmsReceived, SmsThread,
};
use magnetita_proto::phone::telephony::{CallEvent, CallState};

/// Why a call failed, as Kotlin sees it.
#[derive(Debug, uniffi::Error)]
#[uniffi(flat_error)]
pub enum MobileError {
    Link(String),
}

impl std::fmt::Display for MobileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Link(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for MobileError {}

impl From<magnetita_link::LinkError> for MobileError {
    fn from(e: magnetita_link::LinkError) -> Self {
        Self::Link(e.to_string())
    }
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

/// One envelope, described for a log and carried for a handler.
#[derive(uniffi::Record)]
pub struct Event {
    pub capability: u16,
    pub kind: u16,
    pub description: String,
    pub body: Vec<u8>,
    /// The clipboard text, or a notification reply's, decoded here so
    /// Kotlin never reads the wire.
    pub text: Option<String>,
    /// The phone's notification key a desktop message names.
    pub key: Option<String>,
    /// The button index of a notification action.
    pub action: Option<u16>,
    /// The transfer a share message names.
    pub transfer: Option<u32>,
    /// An offered file's size.
    pub size: Option<u64>,
    /// The offset an acceptance asks to send from.
    pub offset: Option<u64>,
    /// Whether a finished transfer carried every byte.
    pub complete: Option<bool>,
    /// Where a received file landed on this phone.
    pub path: Option<String>,
    /// One of the desktop's players, when the message is its state.
    pub media: Option<MobileMediaState>,
    /// The desktop's command for one of this phone's players.
    pub media_command: Option<MobileMediaCommand>,
    /// Contacts changed since this version are wanted.
    pub contacts_since: Option<u64>,
    /// The conversation list is wanted.
    pub conversations_wanted: bool,
    /// A page of this thread is wanted, older than `before_ms`.
    pub thread: Option<u64>,
    pub before_ms: Option<u64>,
    pub limit: Option<u16>,
    /// Send `text` in `thread` (with `text`).
    pub sms_send: bool,
    /// 0 mute, 1 answer, 2 hang up.
    pub call_action: Option<u8>,
}

/// One contact as its vCard.
#[derive(uniffi::Record, Clone)]
pub struct MobileContact {
    pub id: u64,
    pub version: u64,
    pub vcard: String,
}

/// One conversation in the list.
#[derive(uniffi::Record, Clone)]
pub struct MobileConversation {
    pub thread: u64,
    pub addresses: Vec<String>,
    pub snippet: String,
    pub timestamp_ms: u64,
    pub unread: u16,
}

/// One message; attachments are counted by the mime types they carry.
#[derive(uniffi::Record, Clone)]
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
#[derive(uniffi::Record, Clone)]
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
#[derive(uniffi::Record, Clone)]
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

fn media_state_in(s: MediaState) -> MobileMediaState {
    MobileMediaState {
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

/// The kind [`MobileSession::next`] uses for a file this phone finished
/// receiving; not a wire kind, so it cannot collide with one.
pub const FILE_RECEIVED_KIND: u16 = 100;

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

    /// Accepts an offered file into `dir`; its end arrives as an event of
    /// kind [`FILE_RECEIVED_KIND`] with the path.
    pub fn accept_file(&self, transfer: u32, dir: String) -> Result<(), MobileError> {
        let task = self
            .handle
            .block_on(self.inner.accept_file(transfer, PathBuf::from(dir)))?;
        self.handle.spawn(task);
        Ok(())
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
            button: command.button.and_then(button_from_index),
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
        let state = match state {
            0 => CallState::Ringing,
            1 => CallState::Answered,
            2 => CallState::Missed,
            _ => CallState::Ended,
        };
        let event = CallEvent {
            state,
            number,
            name,
            timestamp_ms,
        };
        Ok(self.handle.block_on(self.inner.send_call_event(&event))?)
    }

    /// Shares a URL or a snippet with the desktop.
    pub fn send_text(&self, text: String) -> Result<(), MobileError> {
        Ok(self.handle.block_on(self.inner.send_text(&text))?)
    }

    /// Blocks up to `timeout_ms` for the next envelope; `None` on timeout.
    pub fn next(&self, timeout_ms: u64) -> Result<Option<Event>, MobileError> {
        let env = self
            .handle
            .block_on(self.inner.next(Duration::from_millis(timeout_ms)))?;
        Ok(env.map(|incoming| match incoming {
            Incoming::Envelope(e) => {
                let (key, action, reply) = notification_fields(&e);
                let share = share_fields(&e);
                let media = media_fields(&e);
                let phone = phone_fields(&e);
                let text = clipboard_text(&e).or(reply).or(share.text);
                Event {
                    capability: e.capability,
                    kind: e.kind,
                    description: describe(&e),
                    key,
                    action,
                    transfer: share.transfer,
                    size: share.size,
                    offset: share.offset,
                    complete: share.complete,
                    path: None,
                    media: media.state.map(media_state_in),
                    media_command: media.command.map(|c| MobileMediaCommand {
                        player: c.player,
                        button: c.button.map(button_index),
                        seek_ms: c.seek_ms,
                        volume: c.volume,
                    }),
                    contacts_since: phone.contacts_since,
                    conversations_wanted: phone.conversations_wanted,
                    thread: phone
                        .thread_request
                        .as_ref()
                        .map(|r| r.thread)
                        .or(phone.sms_send.as_ref().map(|s| s.thread)),
                    before_ms: phone.thread_request.as_ref().and_then(|r| r.before_ms),
                    limit: phone.thread_request.as_ref().map(|r| r.limit),
                    sms_send: phone.sms_send.is_some(),
                    call_action: phone.call_action,
                    text: text.or(phone.sms_send.map(|s| s.body)),
                    body: e.body,
                }
            }
            Incoming::FileReceived {
                transfer,
                path,
                complete,
            } => Event {
                capability: magnetita_proto::capability::SHARE,
                kind: FILE_RECEIVED_KIND,
                description: format!("share: received {}", path.display()),
                text: None,
                key: None,
                action: None,
                transfer: Some(transfer),
                size: None,
                offset: None,
                complete: Some(complete),
                path: Some(path.to_string_lossy().into_owned()),
                media: None,
                media_command: None,
                contacts_since: None,
                conversations_wanted: false,
                thread: None,
                before_ms: None,
                limit: None,
                sms_send: false,
                call_action: None,
                body: Vec::new(),
            },
        }))
    }

    pub fn close(&self, reason: String) {
        self.inner.close(&reason);
    }
}
