//! What an envelope from the desktop asks of the phone, as one typed value
//! per message (AND-5). The application matches on [`DesktopSignal`] and
//! never reads a capability or kind id, a default or a body: every one of
//! those decisions is made here, from the protocol crate's own decoders.
//!
//! A message this build does not handle, or whose body does not decode, is
//! [`DesktopSignal::Unhandled`]; one of a capability the session did not
//! negotiate is [`DesktopSignal::Declined`]. Neither is ever guessed into a
//! handled signal.

use magnetita_proto::bound::MAX_LIST;
use magnetita_proto::control::commands::{CommandList, CommandResult};
use magnetita_proto::daily::battery::BatteryRequest;
use magnetita_proto::daily::clipboard::{ClipboardRequest, ClipboardText};
use magnetita_proto::daily::find::{FindRing, FindStop};
use magnetita_proto::daily::media::{MediaCommand, MediaRequest, MediaState};
use magnetita_proto::daily::notifications::{
    NotificationAction, NotificationDismissed, NotificationReply,
};
use magnetita_proto::daily::share::{ShareAccept, ShareDone, ShareOffer, ShareReject, ShareText};
use magnetita_proto::mirror::{
    Codec, GlobalAction, MirrorGlobal, MirrorKey, MirrorKeyframe, MirrorStart, MirrorStop,
    MirrorTouch, TouchAction,
};
use magnetita_proto::phone::contacts::ContactsRequest;
use magnetita_proto::phone::sms::{SmsConversations, SmsSend, SmsThreadRequest};
use magnetita_proto::phone::telephony::{CallAction, CallCommand};
use magnetita_proto::{capability, Envelope};

use crate::mobile::{
    MobileCommand, MobileMediaCommand, MobileMediaState, MobileMirrorStart, MobileStorageRequest,
    MobileTouch,
};
use crate::storage::StorageRequest;

/// One thing the desktop asked, or told, this phone.
#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum DesktopSignal {
    Ring,
    StopRinging,
    BatteryRequested,
    /// The desktop's clipboard now holds this text.
    ClipboardText {
        text: String,
    },
    /// The desktop asks for this phone's clipboard; answered only in front.
    ClipboardRequested,
    /// The desktop dismissed one of this phone's notifications.
    NotificationDismiss {
        key: String,
    },
    /// The desktop pressed button `action` of one of this phone's notifications.
    NotificationAction {
        key: String,
        action: u16,
    },
    /// The desktop answered one of this phone's notifications inline.
    NotificationReply {
        key: String,
        text: String,
    },
    /// The desktop offers a file.
    ShareOffered {
        transfer: u32,
        name: String,
        size: u64,
    },
    /// The desktop accepted this phone's offer; send from `offset`.
    ShareAccepted {
        transfer: u32,
        offset: u64,
    },
    /// The desktop declined this phone's offer, or a transfer ended.
    ShareEnded {
        transfer: u32,
        complete: bool,
    },
    /// A file the core finished receiving on this phone.
    FileReceived {
        transfer: u32,
        path: String,
        complete: bool,
    },
    /// The desktop shared a URL or a snippet.
    ShareText {
        text: String,
    },
    /// One of the desktop's players; an empty player name means it left.
    DesktopMedia {
        state: MobileMediaState,
    },
    /// The desktop drives one of this phone's players.
    MediaControl {
        command: MobileMediaCommand,
    },
    /// The desktop asks for this phone's players.
    MediaRequested,
    /// Contacts changed since `since` are wanted; 0 for all.
    ContactsRequested {
        since: u64,
    },
    ConversationsRequested,
    /// A page of `thread`, at most `limit` messages older than `before_ms`.
    ThreadRequested {
        thread: u64,
        before_ms: Option<u64>,
        limit: u16,
    },
    SmsSendRequested {
        thread: u64,
        body: String,
    },
    /// 0 mute, 1 answer, 2 hang up.
    CallCommand {
        action: u8,
    },
    /// The desktop's registered commands.
    Commands {
        list: Vec<MobileCommand>,
    },
    /// How a run ended.
    CommandResult {
        id: u32,
        ok: bool,
    },
    /// The desktop wants to see this screen.
    MirrorStart {
        options: MobileMirrorStart,
    },
    MirrorStop,
    /// The desktop wants a key frame now.
    MirrorKeyframe,
    MirrorTouched {
        touch: MobileTouch,
    },
    MirrorKey {
        keycode: u16,
        pressed: bool,
    },
    /// 0 back, 1 home, 2 recents.
    MirrorGlobal {
        action: u8,
    },
    /// The desktop browses the shared root.
    Storage {
        request: MobileStorageRequest,
    },
    /// Refused: a capability the session did not negotiate, or an offer past
    /// this phone's bounds, already declined to the desktop.
    Declined {
        capability: u16,
        kind: u16,
        reason: String,
    },
    /// A message this build does not handle, or whose body did not decode.
    Unhandled {
        capability: u16,
        kind: u16,
    },
}

/// The signal one envelope from the desktop carries. The session has already
/// refused what it did not negotiate.
pub fn signal_of(env: &Envelope) -> DesktopSignal {
    let body = &env.body;
    let unhandled = || DesktopSignal::Unhandled {
        capability: env.capability,
        kind: env.kind,
    };
    let decoded = match (env.capability, env.kind) {
        (capability::FIND, FindRing::KIND) => {
            FindRing::decode(body).ok().map(|_| DesktopSignal::Ring)
        }
        (capability::FIND, FindStop::KIND) => FindStop::decode(body)
            .ok()
            .map(|_| DesktopSignal::StopRinging),
        (capability::BATTERY, BatteryRequest::KIND) => BatteryRequest::decode(body)
            .ok()
            .map(|_| DesktopSignal::BatteryRequested),
        (capability::CLIPBOARD, ClipboardText::KIND) => ClipboardText::decode(body)
            .ok()
            .map(|c| DesktopSignal::ClipboardText { text: c.text }),
        (capability::CLIPBOARD, ClipboardRequest::KIND) => ClipboardRequest::decode(body)
            .ok()
            .map(|_| DesktopSignal::ClipboardRequested),
        (capability::NOTIFICATIONS, NotificationDismissed::KIND) => {
            NotificationDismissed::decode(body)
                .ok()
                .map(|d| DesktopSignal::NotificationDismiss { key: d.key })
        }
        (capability::NOTIFICATIONS, NotificationAction::KIND) => NotificationAction::decode(body)
            .ok()
            .map(|a| DesktopSignal::NotificationAction {
                key: a.key,
                action: a.action,
            }),
        (capability::NOTIFICATIONS, NotificationReply::KIND) => NotificationReply::decode(body)
            .ok()
            .map(|r| DesktopSignal::NotificationReply {
                key: r.key,
                text: r.text,
            }),
        (capability::SHARE, ShareOffer::KIND) => {
            ShareOffer::decode(body)
                .ok()
                .map(|o| DesktopSignal::ShareOffered {
                    transfer: o.transfer,
                    name: o.name,
                    size: o.size,
                })
        }
        (capability::SHARE, ShareAccept::KIND) => {
            ShareAccept::decode(body)
                .ok()
                .map(|a| DesktopSignal::ShareAccepted {
                    transfer: a.transfer,
                    offset: a.offset,
                })
        }
        (capability::SHARE, ShareReject::KIND) => {
            ShareReject::decode(body)
                .ok()
                .map(|r| DesktopSignal::ShareEnded {
                    transfer: r.transfer,
                    complete: false,
                })
        }
        (capability::SHARE, ShareDone::KIND) => {
            ShareDone::decode(body)
                .ok()
                .map(|d| DesktopSignal::ShareEnded {
                    transfer: d.transfer,
                    complete: d.complete,
                })
        }
        (capability::SHARE, ShareText::KIND) => ShareText::decode(body)
            .ok()
            .map(|t| DesktopSignal::ShareText { text: t.text }),
        (capability::MEDIA, MediaState::KIND) => {
            MediaState::decode(body)
                .ok()
                .map(|s| DesktopSignal::DesktopMedia {
                    state: media_state_in(s),
                })
        }
        (capability::MEDIA, MediaCommand::KIND) => {
            MediaCommand::decode(body)
                .ok()
                .map(|c| DesktopSignal::MediaControl {
                    command: MobileMediaCommand {
                        player: c.player,
                        button: c.button.map(crate::phone::button_index),
                        seek_ms: c.seek_ms,
                        volume: c.volume,
                    },
                })
        }
        (capability::MEDIA, MediaRequest::KIND) => MediaRequest::decode(body)
            .ok()
            .map(|_| DesktopSignal::MediaRequested),
        (capability::CONTACTS, ContactsRequest::KIND) => {
            ContactsRequest::decode(body)
                .ok()
                .map(|r| DesktopSignal::ContactsRequested {
                    since: r.since_version,
                })
        }
        // The desktop asks for the list by sending an empty one.
        (capability::SMS, SmsConversations::KIND) => SmsConversations::decode(body)
            .ok()
            .filter(|l| l.conversations.is_empty())
            .map(|_| DesktopSignal::ConversationsRequested),
        (capability::SMS, SmsThreadRequest::KIND) => {
            SmsThreadRequest::decode(body)
                .ok()
                .map(|r| DesktopSignal::ThreadRequested {
                    thread: r.thread,
                    before_ms: r.before_ms,
                    limit: r.limit,
                })
        }
        (capability::SMS, SmsSend::KIND) => {
            SmsSend::decode(body)
                .ok()
                .map(|s| DesktopSignal::SmsSendRequested {
                    thread: s.thread,
                    body: s.body,
                })
        }
        (capability::TELEPHONY, CallCommand::KIND) => {
            CallCommand::decode(body)
                .ok()
                .map(|c| DesktopSignal::CallCommand {
                    action: match c.action {
                        CallAction::Mute => 0,
                        CallAction::Answer => 1,
                        CallAction::HangUp => 2,
                    },
                })
        }
        (capability::COMMANDS, CommandList::KIND) => {
            CommandList::decode(body)
                .ok()
                .map(|l| DesktopSignal::Commands {
                    list: l
                        .commands
                        .into_iter()
                        .map(|c| MobileCommand {
                            id: c.id,
                            name: c.name,
                        })
                        .collect(),
                })
        }
        (capability::COMMANDS, CommandResult::KIND) => CommandResult::decode(body)
            .ok()
            .map(|r| DesktopSignal::CommandResult { id: r.id, ok: r.ok }),
        (capability::MIRROR, MirrorStart::KIND) => {
            MirrorStart::decode(body)
                .ok()
                .map(|m| DesktopSignal::MirrorStart {
                    options: MobileMirrorStart {
                        max_size: m.max_size,
                        fps: m.fps,
                        bitrate_kbps: m.bitrate_kbps,
                        codec: match m.codec {
                            Codec::Hevc => 0,
                            Codec::H264 => 1,
                        },
                        audio: m.audio,
                        screen_off: m.screen_off,
                    },
                })
        }
        (capability::MIRROR, MirrorStop::KIND) => MirrorStop::decode(body)
            .ok()
            .map(|_| DesktopSignal::MirrorStop),
        (capability::MIRROR, MirrorKeyframe::KIND) => MirrorKeyframe::decode(body)
            .ok()
            .map(|_| DesktopSignal::MirrorKeyframe),
        (capability::MIRROR, MirrorTouch::KIND) => {
            MirrorTouch::decode(body)
                .ok()
                .map(|t| DesktopSignal::MirrorTouched {
                    touch: MobileTouch {
                        phase: match t.action {
                            TouchAction::Down => 0,
                            TouchAction::Move => 1,
                            TouchAction::Up => 2,
                        },
                        x: t.x,
                        y: t.y,
                        pointer: t.pointer,
                    },
                })
        }
        (capability::MIRROR, MirrorKey::KIND) => {
            MirrorKey::decode(body)
                .ok()
                .map(|k| DesktopSignal::MirrorKey {
                    keycode: k.keycode,
                    pressed: k.pressed,
                })
        }
        (capability::MIRROR, MirrorGlobal::KIND) => {
            MirrorGlobal::decode(body)
                .ok()
                .map(|g| DesktopSignal::MirrorGlobal {
                    action: match g.action {
                        GlobalAction::Back => 0,
                        GlobalAction::Home => 1,
                        GlobalAction::Recents => 2,
                    },
                })
        }
        (capability::STORAGE, _) => StorageRequest::decode(env).map(|r| DesktopSignal::Storage {
            request: storage_record(r),
        }),
        _ => None,
    };
    decoded.unwrap_or_else(unhandled)
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

/// A storage request as the record the application answers. A listing's
/// page size travels in `len`, so the page is the wire's, not a number the
/// application keeps.
fn storage_record(r: StorageRequest) -> MobileStorageRequest {
    let request = r.request();
    let blank = MobileStorageRequest {
        kind: 0,
        request,
        path: String::new(),
        to: String::new(),
        offset: 0,
        len: 0,
        bytes: Vec::new(),
        truncate: false,
    };
    match r {
        StorageRequest::List(m) => MobileStorageRequest {
            kind: 0,
            path: m.path,
            offset: u64::from(m.offset),
            len: MAX_LIST as u32,
            ..blank
        },
        StorageRequest::Stat(m) => MobileStorageRequest {
            kind: 1,
            path: m.path,
            ..blank
        },
        StorageRequest::Read(m) => MobileStorageRequest {
            kind: 2,
            path: m.path,
            offset: m.offset,
            len: m.len,
            ..blank
        },
        StorageRequest::Write(m) => MobileStorageRequest {
            kind: 3,
            path: m.path,
            offset: m.offset,
            bytes: m.bytes,
            truncate: m.truncate,
            ..blank
        },
        StorageRequest::Mkdir(m) => MobileStorageRequest {
            kind: 4,
            path: m.path,
            ..blank
        },
        StorageRequest::Rename(m) => MobileStorageRequest {
            kind: 5,
            path: m.from,
            to: m.to,
            ..blank
        },
        StorageRequest::Delete(m) => MobileStorageRequest {
            kind: 6,
            path: m.path,
            ..blank
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnetita_proto::control::commands::CommandEntry;
    use magnetita_proto::daily::media::MediaButton;
    use magnetita_proto::storage::{List, Read, Rename};

    fn env(capability: u16, kind: u16, body: Vec<u8>) -> Envelope {
        Envelope {
            capability,
            kind,
            id: 0,
            body,
        }
    }

    /// MAG-28, AND-5: every message the desktop sends maps to its typed
    /// signal, decoded here once; the application never sees an id.
    #[test]
    fn every_desktop_message_maps_to_its_signal() {
        use DesktopSignal as S;
        let state = MediaState {
            player: "mpv".into(),
            title: "T".into(),
            artist: "A".into(),
            album: String::new(),
            playing: true,
            position_ms: 1,
            length_ms: 2,
            can_seek: true,
            can_next: true,
            can_previous: false,
            volume: 30,
        };
        let cases: Vec<(Envelope, DesktopSignal)> = vec![
            (
                env(capability::FIND, FindRing::KIND, FindRing.encode()),
                S::Ring,
            ),
            (
                env(capability::FIND, FindStop::KIND, FindStop.encode()),
                S::StopRinging,
            ),
            (
                env(
                    capability::BATTERY,
                    BatteryRequest::KIND,
                    BatteryRequest.encode(),
                ),
                S::BatteryRequested,
            ),
            (
                env(
                    capability::CLIPBOARD,
                    ClipboardText::KIND,
                    ClipboardText { text: "hi".into() }.encode(),
                ),
                S::ClipboardText { text: "hi".into() },
            ),
            (
                env(
                    capability::CLIPBOARD,
                    ClipboardRequest::KIND,
                    ClipboardRequest.encode(),
                ),
                S::ClipboardRequested,
            ),
            (
                env(
                    capability::NOTIFICATIONS,
                    NotificationDismissed::KIND,
                    NotificationDismissed { key: "k1".into() }.encode(),
                ),
                S::NotificationDismiss { key: "k1".into() },
            ),
            (
                env(
                    capability::NOTIFICATIONS,
                    NotificationAction::KIND,
                    NotificationAction {
                        key: "k1".into(),
                        action: 1,
                    }
                    .encode(),
                ),
                S::NotificationAction {
                    key: "k1".into(),
                    action: 1,
                },
            ),
            (
                env(
                    capability::NOTIFICATIONS,
                    NotificationReply::KIND,
                    NotificationReply {
                        key: "k1".into(),
                        text: "ok".into(),
                    }
                    .encode(),
                ),
                S::NotificationReply {
                    key: "k1".into(),
                    text: "ok".into(),
                },
            ),
            (
                env(
                    capability::SHARE,
                    ShareOffer::KIND,
                    ShareOffer {
                        transfer: 4,
                        name: "a.jpg".into(),
                        size: 12,
                        mime: String::new(),
                    }
                    .encode(),
                ),
                S::ShareOffered {
                    transfer: 4,
                    name: "a.jpg".into(),
                    size: 12,
                },
            ),
            (
                env(
                    capability::SHARE,
                    ShareAccept::KIND,
                    ShareAccept {
                        transfer: 4,
                        offset: 7,
                    }
                    .encode(),
                ),
                S::ShareAccepted {
                    transfer: 4,
                    offset: 7,
                },
            ),
            (
                env(
                    capability::SHARE,
                    ShareReject::KIND,
                    ShareReject { transfer: 4 }.encode(),
                ),
                S::ShareEnded {
                    transfer: 4,
                    complete: false,
                },
            ),
            (
                env(
                    capability::SHARE,
                    ShareDone::KIND,
                    ShareDone {
                        transfer: 4,
                        complete: true,
                    }
                    .encode(),
                ),
                S::ShareEnded {
                    transfer: 4,
                    complete: true,
                },
            ),
            (
                env(
                    capability::SHARE,
                    ShareText::KIND,
                    ShareText {
                        text: "https://example.org".into(),
                    }
                    .encode(),
                ),
                S::ShareText {
                    text: "https://example.org".into(),
                },
            ),
            (
                env(capability::MEDIA, MediaState::KIND, state.encode()),
                S::DesktopMedia {
                    state: media_state_in(state.clone()),
                },
            ),
            (
                env(
                    capability::MEDIA,
                    MediaCommand::KIND,
                    MediaCommand {
                        player: "YT".into(),
                        button: Some(MediaButton::PlayPause),
                        seek_ms: None,
                        volume: None,
                    }
                    .encode(),
                ),
                S::MediaControl {
                    command: MobileMediaCommand {
                        player: "YT".into(),
                        button: Some(2),
                        seek_ms: None,
                        volume: None,
                    },
                },
            ),
            (
                env(capability::MEDIA, MediaRequest::KIND, MediaRequest.encode()),
                S::MediaRequested,
            ),
            (
                env(
                    capability::CONTACTS,
                    ContactsRequest::KIND,
                    ContactsRequest { since_version: 7 }.encode(),
                ),
                S::ContactsRequested { since: 7 },
            ),
            (
                env(
                    capability::SMS,
                    SmsConversations::KIND,
                    SmsConversations {
                        conversations: vec![],
                    }
                    .encode(),
                ),
                S::ConversationsRequested,
            ),
            (
                env(
                    capability::SMS,
                    SmsThreadRequest::KIND,
                    SmsThreadRequest {
                        thread: 4,
                        before_ms: None,
                        limit: 50,
                    }
                    .encode(),
                ),
                S::ThreadRequested {
                    thread: 4,
                    before_ms: None,
                    limit: 50,
                },
            ),
            (
                env(
                    capability::SMS,
                    SmsSend::KIND,
                    SmsSend {
                        thread: 4,
                        body: "hi".into(),
                    }
                    .encode(),
                ),
                S::SmsSendRequested {
                    thread: 4,
                    body: "hi".into(),
                },
            ),
            (
                env(
                    capability::TELEPHONY,
                    CallCommand::KIND,
                    CallCommand {
                        action: CallAction::HangUp,
                    }
                    .encode(),
                ),
                S::CallCommand { action: 2 },
            ),
            (
                env(
                    capability::COMMANDS,
                    CommandList::KIND,
                    CommandList {
                        commands: vec![CommandEntry {
                            id: 1,
                            name: "Lock".into(),
                        }],
                    }
                    .encode(),
                ),
                S::Commands {
                    list: vec![MobileCommand {
                        id: 1,
                        name: "Lock".into(),
                    }],
                },
            ),
            (
                env(
                    capability::COMMANDS,
                    CommandResult::KIND,
                    CommandResult { id: 1, ok: true }.encode(),
                ),
                S::CommandResult { id: 1, ok: true },
            ),
            (
                env(
                    capability::MIRROR,
                    MirrorStart::KIND,
                    MirrorStart {
                        max_size: 1080,
                        fps: 30,
                        bitrate_kbps: 8000,
                        codec: Codec::H264,
                        audio: true,
                        screen_off: false,
                    }
                    .encode(),
                ),
                S::MirrorStart {
                    options: MobileMirrorStart {
                        max_size: 1080,
                        fps: 30,
                        bitrate_kbps: 8000,
                        codec: 1,
                        audio: true,
                        screen_off: false,
                    },
                },
            ),
            (
                env(capability::MIRROR, MirrorStop::KIND, MirrorStop.encode()),
                S::MirrorStop,
            ),
            (
                env(
                    capability::MIRROR,
                    MirrorKeyframe::KIND,
                    MirrorKeyframe.encode(),
                ),
                S::MirrorKeyframe,
            ),
            (
                env(
                    capability::MIRROR,
                    MirrorTouch::KIND,
                    MirrorTouch {
                        action: TouchAction::Up,
                        x: 3,
                        y: 4,
                        pointer: 1,
                    }
                    .encode(),
                ),
                S::MirrorTouched {
                    touch: MobileTouch {
                        phase: 2,
                        x: 3,
                        y: 4,
                        pointer: 1,
                    },
                },
            ),
            (
                env(
                    capability::MIRROR,
                    MirrorKey::KIND,
                    MirrorKey {
                        keycode: 29,
                        pressed: true,
                    }
                    .encode(),
                ),
                S::MirrorKey {
                    keycode: 29,
                    pressed: true,
                },
            ),
            (
                env(
                    capability::MIRROR,
                    MirrorGlobal::KIND,
                    MirrorGlobal {
                        action: GlobalAction::Recents,
                    }
                    .encode(),
                ),
                S::MirrorGlobal { action: 2 },
            ),
        ];
        for (envelope, expected) in cases {
            assert_eq!(
                signal_of(&envelope),
                expected,
                "{} {}",
                envelope.capability,
                envelope.kind
            );
        }
    }

    /// A listing carries the wire's page size; a read its range.
    #[test]
    fn storage_requests_carry_what_the_application_answers() {
        let list = env(
            capability::STORAGE,
            List::KIND,
            List {
                request: 3,
                path: "DCIM".into(),
                offset: 256,
            }
            .encode(),
        );
        let DesktopSignal::Storage { request } = signal_of(&list) else {
            panic!("a listing is a storage request");
        };
        assert_eq!((request.kind, request.request), (0, 3));
        assert_eq!((request.offset, request.len), (256, MAX_LIST as u32));
        let read = env(
            capability::STORAGE,
            Read::KIND,
            Read {
                request: 4,
                path: "a".into(),
                offset: 10,
                len: 20,
            }
            .encode(),
        );
        let DesktopSignal::Storage { request } = signal_of(&read) else {
            panic!("a read is a storage request");
        };
        assert_eq!((request.kind, request.offset, request.len), (2, 10, 20));
        let rename = env(
            capability::STORAGE,
            Rename::KIND,
            Rename {
                request: 5,
                from: "a".into(),
                to: "b".into(),
            }
            .encode(),
        );
        let DesktopSignal::Storage { request } = signal_of(&rename) else {
            panic!("a rename is a storage request");
        };
        assert_eq!(
            (request.kind, request.path.as_str(), request.to.as_str()),
            (5, "a", "b")
        );
    }

    /// MAG-28: nothing is guessed. An unknown kind, a body that does not
    /// decode, a non-empty conversation list or an unknown capability is
    /// unhandled, never the nearest known signal.
    #[test]
    fn what_this_build_does_not_handle_is_never_guessed() {
        for envelope in [
            env(capability::FIND, 9, Vec::new()),
            env(capability::CLIPBOARD, ClipboardText::KIND, vec![0xff]),
            env(capability::SMS, SmsThreadRequest::KIND, Vec::new()),
            env(
                capability::SMS,
                SmsConversations::KIND,
                SmsConversations {
                    conversations: vec![magnetita_proto::phone::sms::Conversation {
                        thread: 1,
                        addresses: vec![],
                        snippet: String::new(),
                        timestamp_ms: 0,
                        unread: 0,
                    }],
                }
                .encode(),
            ),
            env(capability::STORAGE, 99, Vec::new()),
            env(999, 1, Vec::new()),
        ] {
            assert_eq!(
                signal_of(&envelope),
                DesktopSignal::Unhandled {
                    capability: envelope.capability,
                    kind: envelope.kind
                }
            );
        }
    }
}
