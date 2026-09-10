//! `notifications` (capability 3): the phone's notifications on the
//! desktop, with their actions and inline replies going back.

use crate::bound::{self, MAX_ICON, MAX_IDENT, MAX_LIST, MAX_TEXT};
use crate::codec::{self, count, required, Map};
use crate::error::DecodeError;

/// One button on a notification, addressed by its index on the phone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub label: String,
}

/// Phone → desktop: a notification appeared or changed. Kind 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationPosted {
    /// The phone's key for it; every later message names it.
    pub key: String,
    pub app_name: String,
    pub title: String,
    pub body: String,
    /// Unix milliseconds on the phone.
    pub timestamp_ms: u64,
    /// Whether an inline reply is accepted.
    pub replyable: bool,
    /// At most [`MAX_LIST`].
    pub actions: Vec<Action>,
    /// PNG bytes, at most [`MAX_ICON`]; absent when unchanged or none.
    pub icon: Option<Vec<u8>>,
    /// A player's now-playing notification: dismissing it on the phone
    /// stops the playback, so the desktop never asks that, and shows it
    /// only when asked to. Absent means false.
    pub media: bool,
}

impl NotificationPosted {
    pub const KIND: u16 = 1;

    pub fn encode(&self) -> Vec<u8> {
        let mut m = Map::new(count(7, &[self.icon.is_some(), self.media]))
            .text(0, &self.key)
            .text(1, &self.app_name)
            .text(2, &self.title)
            .text(3, &self.body)
            .u64(4, self.timestamp_ms)
            .bool(5, self.replyable)
            .list(6, self.actions.len());
        for a in &self.actions {
            m = m.item(&Map::new(1).text(0, &a.label).finish());
        }
        if let Some(icon) = &self.icon {
            m = m.bytes(7, icon);
        }
        if self.media {
            m = m.bool(8, true);
        }
        m.finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut key, mut app, mut title, mut text, mut ts, mut replyable, mut actions, mut icon) =
            (None, None, None, None, None, None, None, None);
        let mut media = false;
        codec::read(body, "notification", |k, d| {
            match k {
                0 => key = Some(bound::text(d, "key", MAX_IDENT)?),
                1 => app = Some(bound::text(d, "app_name", MAX_IDENT)?),
                2 => title = Some(bound::text(d, "title", MAX_TEXT)?),
                3 => text = Some(bound::text(d, "body", MAX_TEXT)?),
                4 => ts = Some(bound::u64(d, "timestamp_ms")?),
                5 => replyable = Some(bound::bool(d, "replyable")?),
                6 => {
                    actions = Some(codec::read_list(d, "actions", MAX_LIST, |d| {
                        let mut label = None;
                        codec::read_map(d, "action", |k, d| {
                            match k {
                                0 => label = Some(bound::text(d, "label", MAX_IDENT)?),
                                _ => return Ok(false),
                            }
                            Ok(true)
                        })?;
                        Ok(Action {
                            label: required(label, "label")?,
                        })
                    })?)
                }
                7 => icon = Some(bound::bytes(d, "icon", MAX_ICON)?),
                8 => media = bound::bool(d, "media")?,
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            key: required(key, "key")?,
            app_name: required(app, "app_name")?,
            title: required(title, "title")?,
            body: required(text, "body")?,
            timestamp_ms: required(ts, "timestamp_ms")?,
            replyable: required(replyable, "replyable")?,
            actions: actions.unwrap_or_default(),
            icon,
            media,
        })
    }
}

/// Either direction: the notification is gone, or dismiss it. Kind 2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationDismissed {
    pub key: String,
}

/// Desktop → phone: press this action. Kind 3.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationAction {
    pub key: String,
    /// Index into [`NotificationPosted::actions`].
    pub action: u16,
}

/// Desktop → phone: the inline reply. Kind 4.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationReply {
    pub key: String,
    pub text: String,
}

fn decode_key(
    body: &[u8],
    what: &'static str,
) -> Result<(String, Option<u16>, Option<String>), DecodeError> {
    let (mut key, mut action, mut text) = (None, None, None);
    codec::read(body, what, |k, d| {
        match k {
            0 => key = Some(bound::text(d, "key", MAX_IDENT)?),
            1 => action = Some(bound::u16(d, "action")?),
            2 => text = Some(bound::text(d, "text", MAX_TEXT)?),
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok((required(key, "key")?, action, text))
}

impl NotificationDismissed {
    pub const KIND: u16 = 2;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(1).text(0, &self.key).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        Ok(Self {
            key: decode_key(body, "dismissed")?.0,
        })
    }
}

impl NotificationAction {
    pub const KIND: u16 = 3;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2).text(0, &self.key).u16(1, self.action).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (key, action, _) = decode_key(body, "action")?;
        Ok(Self {
            key,
            action: required(action, "action")?,
        })
    }
}

impl NotificationReply {
    pub const KIND: u16 = 4;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2).text(0, &self.key).text(2, &self.text).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (key, _, text) = decode_key(body, "reply")?;
        Ok(Self {
            key,
            text: required(text, "text")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::testing::{hex, unhex};

    fn sample() -> NotificationPosted {
        NotificationPosted {
            key: "0|com.whatsapp|1".into(),
            app_name: "WhatsApp".into(),
            title: "Ana".into(),
            body: "Coming over?".into(),
            timestamp_ms: 1_788_927_033_443,
            replyable: true,
            actions: vec![
                Action {
                    label: "Reply".into(),
                },
                Action {
                    label: "Mark as read".into(),
                },
            ],
            icon: Some(vec![0x89, 0x50, 0x4e, 0x47]),
            media: false,
        }
    }

    const VECTOR: &str = "a80070307c636f6d2e77686174736170707c31016857686174734170700263416e61036c436f6d696e67206f7665723f041b000001a0845c406305f50682a100655265706c79a1006c4d61726b2061732072656164074489504e47";

    #[test]
    fn golden_vector_round_trips() {
        assert_eq!(hex(&sample().encode()), VECTOR);
        assert_eq!(
            NotificationPosted::decode(&unhex(VECTOR)).unwrap(),
            sample()
        );
    }

    #[test]
    fn a_media_notification_says_so_and_the_flag_defaults_off() {
        let mut note = sample();
        assert!(!NotificationPosted::decode(&note.encode()).unwrap().media);
        note.media = true;
        assert!(NotificationPosted::decode(&note.encode()).unwrap().media);
    }

    #[test]
    fn the_icon_is_optional_and_actions_default_to_none() {
        let m = NotificationPosted {
            icon: None,
            actions: vec![],
            ..sample()
        };
        let bytes = m.encode();
        assert_eq!(bytes[0], 0xa7);
        assert_eq!(NotificationPosted::decode(&bytes).unwrap(), m);
    }

    #[test]
    fn an_icon_over_the_bound_is_refused() {
        let m = NotificationPosted {
            icon: Some(vec![0; MAX_ICON + 1]),
            ..sample()
        };
        assert_eq!(
            NotificationPosted::decode(&m.encode()),
            Err(DecodeError::TooLong {
                what: "icon",
                max: MAX_ICON,
                len: MAX_ICON + 1
            })
        );
    }

    #[test]
    fn an_action_without_a_label_is_refused() {
        let bytes = Map::new(7)
            .text(0, "k")
            .text(1, "a")
            .text(2, "t")
            .text(3, "b")
            .u64(4, 1)
            .bool(5, false)
            .list(6, 1)
            .item(&Map::new(0).finish())
            .finish();
        assert_eq!(
            NotificationPosted::decode(&bytes),
            Err(DecodeError::MissingField("label"))
        );
    }

    #[test]
    fn the_small_messages_round_trip() {
        let d = NotificationDismissed { key: "k".into() };
        assert_eq!(hex(&d.encode()), "a100616b");
        assert_eq!(NotificationDismissed::decode(&d.encode()).unwrap(), d);
        let a = NotificationAction {
            key: "k".into(),
            action: 1,
        };
        assert_eq!(hex(&a.encode()), "a200616b0101");
        assert_eq!(NotificationAction::decode(&a.encode()).unwrap(), a);
        let r = NotificationReply {
            key: "k".into(),
            text: "yes".into(),
        };
        assert_eq!(hex(&r.encode()), "a200616b0263796573");
        assert_eq!(NotificationReply::decode(&r.encode()).unwrap(), r);
        assert_eq!(
            NotificationReply::decode(&d.encode()),
            Err(DecodeError::MissingField("text"))
        );
    }
}
