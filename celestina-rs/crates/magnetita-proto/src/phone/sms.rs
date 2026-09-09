//! `sms` (capability 10): conversations and messages read from the phone,
//! and a send the desktop asks for by conversation. Only the phone knows a
//! number; the desktop names conversations by the phone's thread id.

use crate::bound::{self, MAX_IDENT, MAX_LIST, MAX_TEXT};
use crate::codec::{self, count, required, Map};
use crate::error::DecodeError;

/// One conversation in the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conversation {
    pub thread: u64,
    /// Numbers or names the phone shows, at most [`MAX_LIST`].
    pub addresses: Vec<String>,
    /// The last message, at most [`MAX_TEXT`] bytes.
    pub snippet: String,
    pub timestamp_ms: u64,
    pub unread: u16,
}

/// Phone → desktop: every conversation, newest first. Kind 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmsConversations {
    pub conversations: Vec<Conversation>,
}

/// Desktop → phone: a page of one thread, older than `before_ms`. Kind 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SmsThreadRequest {
    pub thread: u64,
    /// Absent for the newest page.
    pub before_ms: Option<u64>,
    pub limit: u16,
}

/// An attachment: its bytes come on a `share` transfer of that id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub transfer: u32,
    pub mime: String,
}

/// One message in a thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmsMessage {
    pub id: u64,
    pub from_me: bool,
    pub address: String,
    pub body: String,
    pub timestamp_ms: u64,
    pub attachments: Vec<Attachment>,
}

/// Phone → desktop: a page of a thread, oldest first. Kind 3.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmsThread {
    pub thread: u64,
    pub messages: Vec<SmsMessage>,
}

/// Desktop → phone: send `body` in `thread`. Kind 4.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmsSend {
    pub thread: u64,
    pub body: String,
}

/// Phone → desktop: a message arrived or was sent. Kind 5.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmsReceived {
    pub thread: u64,
    pub message: SmsMessage,
}

fn encode_message(m: &SmsMessage) -> Vec<u8> {
    let mut map = Map::new(6)
        .u64(0, m.id)
        .bool(1, m.from_me)
        .text(2, &m.address)
        .text(3, &m.body)
        .u64(4, m.timestamp_ms)
        .list(5, m.attachments.len());
    for a in &m.attachments {
        map = map.item(&Map::new(2).u32(0, a.transfer).text(1, &a.mime).finish());
    }
    map.finish()
}

fn decode_message(d: &mut minicbor::Decoder<'_>) -> Result<SmsMessage, DecodeError> {
    let (mut id, mut from_me, mut address, mut body, mut ts, mut attachments) =
        (None, None, None, None, None, None);
    codec::read_map(d, "message", |k, d| {
        match k {
            0 => id = Some(bound::u64(d, "id")?),
            1 => from_me = Some(bound::bool(d, "from_me")?),
            2 => address = Some(bound::text(d, "address", MAX_IDENT)?),
            3 => body = Some(bound::text(d, "body", MAX_TEXT)?),
            4 => ts = Some(bound::u64(d, "timestamp_ms")?),
            5 => {
                attachments = Some(codec::read_list(d, "attachments", MAX_LIST, |d| {
                    let (mut transfer, mut mime) = (None, None);
                    codec::read_map(d, "attachment", |k, d| {
                        match k {
                            0 => transfer = Some(bound::u32(d, "transfer")?),
                            1 => mime = Some(bound::text(d, "mime", MAX_IDENT)?),
                            _ => return Ok(false),
                        }
                        Ok(true)
                    })?;
                    Ok(Attachment {
                        transfer: required(transfer, "transfer")?,
                        mime: mime.unwrap_or_default(),
                    })
                })?)
            }
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok(SmsMessage {
        id: required(id, "id")?,
        from_me: required(from_me, "from_me")?,
        address: address.unwrap_or_default(),
        body: body.unwrap_or_default(),
        timestamp_ms: required(ts, "timestamp_ms")?,
        attachments: attachments.unwrap_or_default(),
    })
}

impl SmsConversations {
    pub const KIND: u16 = 1;

    pub fn encode(&self) -> Vec<u8> {
        let mut m = Map::new(1).list(0, self.conversations.len());
        for c in &self.conversations {
            let mut item = Map::new(5).u64(0, c.thread).list(1, c.addresses.len());
            for a in &c.addresses {
                item = item.text_item(a);
            }
            m = m.item(
                &item
                    .text(2, &c.snippet)
                    .u64(3, c.timestamp_ms)
                    .u16(4, c.unread)
                    .finish(),
            );
        }
        m.finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let mut list = None;
        codec::read(body, "conversations", |k, d| {
            match k {
                0 => {
                    list = Some(codec::read_list(d, "conversations", MAX_LIST, |d| {
                        let (mut thread, mut addresses, mut snippet, mut ts, mut unread) =
                            (None, None, String::new(), None, 0);
                        codec::read_map(d, "conversation", |k, d| {
                            match k {
                                0 => thread = Some(bound::u64(d, "thread")?),
                                1 => {
                                    addresses = Some(codec::read_texts(
                                        d,
                                        "addresses",
                                        MAX_LIST,
                                        MAX_IDENT,
                                    )?)
                                }
                                2 => snippet = bound::text(d, "snippet", MAX_TEXT)?,
                                3 => ts = Some(bound::u64(d, "timestamp_ms")?),
                                4 => unread = bound::u16(d, "unread")?,
                                _ => return Ok(false),
                            }
                            Ok(true)
                        })?;
                        Ok(Conversation {
                            thread: required(thread, "thread")?,
                            addresses: required(addresses, "addresses")?,
                            snippet,
                            timestamp_ms: required(ts, "timestamp_ms")?,
                            unread,
                        })
                    })?)
                }
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            conversations: required(list, "conversations")?,
        })
    }
}

impl SmsThreadRequest {
    pub const KIND: u16 = 2;

    pub fn encode(&self) -> Vec<u8> {
        let mut m = Map::new(count(2, &[self.before_ms.is_some()])).u64(0, self.thread);
        if let Some(b) = self.before_ms {
            m = m.u64(1, b);
        }
        m.u16(2, self.limit).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut thread, mut before, mut limit) = (None, None, None);
        codec::read(body, "thread request", |k, d| {
            match k {
                0 => thread = Some(bound::u64(d, "thread")?),
                1 => before = Some(bound::u64(d, "before_ms")?),
                2 => limit = Some(bound::u16(d, "limit")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        let limit = required(limit, "limit")?;
        if limit == 0 || limit as usize > MAX_LIST {
            return Err(DecodeError::OutOfRange("limit"));
        }
        Ok(Self {
            thread: required(thread, "thread")?,
            before_ms: before,
            limit,
        })
    }
}

impl SmsThread {
    pub const KIND: u16 = 3;

    pub fn encode(&self) -> Vec<u8> {
        let mut m = Map::new(2).u64(0, self.thread).list(1, self.messages.len());
        for msg in &self.messages {
            m = m.item(&encode_message(msg));
        }
        m.finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut thread, mut messages) = (None, None);
        codec::read(body, "thread", |k, d| {
            match k {
                0 => thread = Some(bound::u64(d, "thread")?),
                1 => messages = Some(codec::read_list(d, "messages", MAX_LIST, decode_message)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            thread: required(thread, "thread")?,
            messages: required(messages, "messages")?,
        })
    }
}

impl SmsSend {
    pub const KIND: u16 = 4;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2).u64(0, self.thread).text(1, &self.body).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut thread, mut text) = (None, None);
        codec::read(body, "send", |k, d| {
            match k {
                0 => thread = Some(bound::u64(d, "thread")?),
                1 => text = Some(bound::text(d, "body", MAX_TEXT)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        let text = required(text, "body")?;
        if text.is_empty() {
            return Err(DecodeError::Malformed("empty body"));
        }
        Ok(Self {
            thread: required(thread, "thread")?,
            body: text,
        })
    }
}

impl SmsReceived {
    pub const KIND: u16 = 5;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2)
            .u64(0, self.thread)
            .nested(1, &encode_message(&self.message))
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut thread, mut message) = (None, None);
        codec::read(body, "received", |k, d| {
            match k {
                0 => thread = Some(bound::u64(d, "thread")?),
                1 => message = Some(decode_message(d)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            thread: required(thread, "thread")?,
            message: required(message, "message")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::testing::{hex, unhex};

    fn message() -> SmsMessage {
        SmsMessage {
            id: 501,
            from_me: false,
            address: "+34600000000".into(),
            body: "Coming over?".into(),
            timestamp_ms: 1_788_927_033_443,
            attachments: vec![Attachment {
                transfer: 3,
                mime: "image/jpeg".into(),
            }],
        }
    }

    const THREAD: &str = "a2000c0181a6001901f501f4026c2b3334363030303030303030036c436f6d696e67206f7665723f041b000001a0845c40630581a20003016a696d6167652f6a706567";

    #[test]
    fn golden_vectors_round_trip() {
        let t = SmsThread {
            thread: 12,
            messages: vec![message()],
        };
        assert_eq!(hex(&t.encode()), THREAD);
        assert_eq!(SmsThread::decode(&unhex(THREAD)).unwrap(), t);
        let c = SmsConversations {
            conversations: vec![Conversation {
                thread: 12,
                addresses: vec!["Ana".into()],
                snippet: "Coming over?".into(),
                timestamp_ms: 1,
                unread: 2,
            }],
        };
        assert_eq!(SmsConversations::decode(&c.encode()).unwrap(), c);
        let r = SmsThreadRequest {
            thread: 12,
            before_ms: None,
            limit: 50,
        };
        assert_eq!(hex(&r.encode()), "a2000c021832");
        assert_eq!(SmsThreadRequest::decode(&r.encode()).unwrap(), r);
        let r2 = SmsThreadRequest {
            before_ms: Some(5),
            ..r
        };
        assert_eq!(SmsThreadRequest::decode(&r2.encode()).unwrap(), r2);
        let s = SmsSend {
            thread: 12,
            body: "On my way".into(),
        };
        assert_eq!(SmsSend::decode(&s.encode()).unwrap(), s);
        let rcv = SmsReceived {
            thread: 12,
            message: message(),
        };
        assert_eq!(SmsReceived::decode(&rcv.encode()).unwrap(), rcv);
    }

    #[test]
    fn nonsense_is_refused() {
        let empty = SmsSend {
            thread: 1,
            body: String::new(),
        };
        assert_eq!(
            SmsSend::decode(&empty.encode()),
            Err(DecodeError::Malformed("empty body"))
        );
        let zero = SmsThreadRequest {
            thread: 1,
            before_ms: None,
            limit: 0,
        };
        assert_eq!(
            SmsThreadRequest::decode(&zero.encode()),
            Err(DecodeError::OutOfRange("limit"))
        );
        let long = SmsSend {
            thread: 1,
            body: "x".repeat(MAX_TEXT + 1),
        };
        assert_eq!(
            SmsSend::decode(&long.encode()),
            Err(DecodeError::TooLong {
                what: "body",
                max: MAX_TEXT,
                len: MAX_TEXT + 1
            })
        );
    }
}
