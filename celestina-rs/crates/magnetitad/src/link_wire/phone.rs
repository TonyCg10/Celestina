//! The phone's contacts, SMS conversations and calls on the desktop, over
//! the own wire, and the desktop's replies, sends and call actions back.
//!
//! Everything lives in the daemon's memory for the session: the contact
//! book (names resolve numbers for SMS and calls), the conversation list
//! and the pages of threads the desktop asked for, and the current call.
//! Nothing lands on disk.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use magnetita_proto::phone::contacts::{ContactsRequest, ContactsSync};
use magnetita_proto::phone::sms::{
    Conversation, SmsConversations, SmsMessage, SmsSend, SmsThread, SmsThreadRequest,
};
use magnetita_proto::phone::telephony::{CallAction, CallCommand, CallEvent, CallState};
use magnetita_proto::{capability, Envelope};
use zbus::zvariant::{OwnedValue, Value};

use crate::lock::LockOk;

/// The page size the desktop asks a thread in.
pub(crate) const THREAD_PAGE: u16 = 50;

/// One contact as the book keeps it: the name and its numbers, digits only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Person {
    pub(crate) version: u64,
    pub(crate) name: String,
    pub(crate) numbers: Vec<String>,
}

/// The vCard's `FN` and every `TEL`; a vCard without a name is skipped.
pub(crate) fn parse_vcard(vcard: &str) -> Option<(String, Vec<String>)> {
    let mut name = None;
    let mut numbers = Vec::new();
    for raw in vcard.lines() {
        let line = raw.trim_end_matches('\r');
        let (key, value) = match line.split_once(':') {
            Some(parts) => parts,
            None => continue,
        };
        let property = key.split(';').next().unwrap_or("").to_ascii_uppercase();
        match property.as_str() {
            "FN" => name = Some(value.trim().to_owned()).filter(|n| !n.is_empty()),
            "TEL" => {
                let digits = digits_of(value.trim_start_matches("tel:"));
                if !digits.is_empty() {
                    numbers.push(digits);
                }
            }
            _ => {}
        }
    }
    name.map(|n| (n, numbers))
}

/// A number reduced to its digits, for matching.
pub(crate) fn digits_of(number: &str) -> String {
    number.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Whether two numbers name the same line: equal, or one ends with the
/// other's last nine digits (the country code is what varies).
fn same_line(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    fn tail(s: &str) -> &str {
        let n = s.len();
        &s[n.saturating_sub(9)..]
    }
    !a.is_empty() && a.len() >= 7 && b.len() >= 7 && tail(a) == tail(b)
}

/// One device's phone-side state.
#[derive(Default)]
pub(crate) struct DeviceBook {
    pub(crate) contacts_version: u64,
    pub(crate) people: BTreeMap<u64, Person>,
    pub(crate) conversations: Vec<Conversation>,
    pub(crate) threads: HashMap<u64, Vec<SmsMessage>>,
    pub(crate) call: Option<CallEvent>,
}

impl DeviceBook {
    /// Applies a page of the phone's contacts.
    pub(crate) fn sync(&mut self, page: &ContactsSync) {
        for c in &page.contacts {
            if let Some((name, numbers)) = parse_vcard(&c.vcard) {
                self.people.insert(
                    c.id,
                    Person {
                        version: c.version,
                        name,
                        numbers,
                    },
                );
            } else {
                self.people.remove(&c.id);
            }
        }
        for id in &page.removed {
            self.people.remove(id);
        }
        if page.complete {
            self.contacts_version = page.version;
        }
    }

    /// The contact name behind a number, when the book has one.
    pub(crate) fn resolve(&self, number: &str) -> Option<String> {
        let digits = digits_of(number);
        if digits.is_empty() {
            return None;
        }
        self.people
            .values()
            .find(|p| p.numbers.iter().any(|n| same_line(n, &digits)))
            .map(|p| p.name.clone())
    }

    /// A conversation's addresses as the person would read them.
    pub(crate) fn label(&self, addresses: &[String]) -> String {
        addresses
            .iter()
            .map(|a| self.resolve(a).unwrap_or_else(|| a.clone()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// A received message goes to the front of its conversation and into
    /// the cached page, if any.
    pub(crate) fn received(&mut self, thread: u64, message: &SmsMessage) {
        if let Some(page) = self.threads.get_mut(&thread) {
            if !page.iter().any(|m| m.id == message.id) {
                page.push(message.clone());
            }
        }
        match self.conversations.iter_mut().find(|c| c.thread == thread) {
            Some(c) => {
                c.snippet = message.body.clone();
                c.timestamp_ms = message.timestamp_ms;
                if !message.from_me {
                    c.unread = c.unread.saturating_add(1);
                }
            }
            None => self.conversations.insert(
                0,
                Conversation {
                    thread,
                    addresses: vec![message.address.clone()],
                    snippet: message.body.clone(),
                    timestamp_ms: message.timestamp_ms,
                    unread: u16::from(!message.from_me),
                },
            ),
        }
        self.conversations
            .sort_by_key(|c| std::cmp::Reverse(c.timestamp_ms));
    }
}

/// Every connected device's book, shared by the wire's sessions and the
/// `Devices1` methods that read it; one per daemon.
#[derive(Default)]
pub(crate) struct PhoneStore {
    books: Mutex<HashMap<String, DeviceBook>>,
}

static STORE: std::sync::LazyLock<PhoneStore> = std::sync::LazyLock::new(PhoneStore::default);

/// The daemon's one store.
pub(crate) fn store() -> &'static PhoneStore {
    &STORE
}

impl PhoneStore {
    pub(crate) fn with<T>(&self, device_id: &str, f: impl FnOnce(&mut DeviceBook) -> T) -> T {
        let mut books = self.books.lock_ok();
        f(books.entry(device_id.to_owned()).or_default())
    }

    /// The conversations as `Devices1` publishes them.
    pub(crate) fn conversations(&self, device_id: &str) -> Vec<HashMap<String, OwnedValue>> {
        self.with(device_id, |book| {
            book.conversations
                .iter()
                .map(|c| {
                    dict([
                        ("thread", Value::from(c.thread)),
                        ("label", Value::from(book.label(&c.addresses))),
                        ("addresses", Value::from(c.addresses.clone())),
                        ("snippet", Value::from(c.snippet.clone())),
                        ("timestamp", Value::from(c.timestamp_ms)),
                        ("unread", Value::from(u32::from(c.unread))),
                    ])
                })
                .collect()
        })
    }

    /// A thread's cached page as `Devices1` publishes it, oldest first.
    pub(crate) fn thread(&self, device_id: &str, thread: u64) -> Vec<HashMap<String, OwnedValue>> {
        self.with(device_id, |book| {
            let mut page = book.threads.get(&thread).cloned().unwrap_or_default();
            page.sort_by_key(|m| m.timestamp_ms);
            page.iter()
                .map(|m| {
                    dict([
                        ("id", Value::from(m.id)),
                        ("fromMe", Value::from(m.from_me)),
                        ("address", Value::from(m.address.clone())),
                        (
                            "name",
                            Value::from(book.resolve(&m.address).unwrap_or_default()),
                        ),
                        ("body", Value::from(m.body.clone())),
                        ("timestamp", Value::from(m.timestamp_ms)),
                        ("attachments", Value::from(m.attachments.len() as u32)),
                    ])
                })
                .collect()
        })
    }

    pub(crate) fn forget_device(&self, device_id: &str) {
        self.books.lock_ok().remove(device_id);
    }
}

fn dict<const N: usize>(fields: [(&str, Value<'_>); N]) -> HashMap<String, OwnedValue> {
    fields
        .into_iter()
        .map(|(k, v)| {
            (
                k.to_owned(),
                OwnedValue::try_from(v).expect("a basic value always converts"),
            )
        })
        .collect()
}

fn envelope(cap: u16, kind: u16, body: Vec<u8>) -> Envelope {
    Envelope {
        capability: cap,
        kind,
        id: 0,
        body,
    }
}

/// Desktop → phone: the contacts changed since the book's version.
pub(crate) fn contacts_request(since_version: u64) -> Envelope {
    envelope(
        capability::CONTACTS,
        ContactsRequest::KIND,
        ContactsRequest { since_version }.encode(),
    )
}

/// Desktop → phone: the conversation list. An empty list is the request;
/// only the phone fills it.
pub(crate) fn conversations_request() -> Envelope {
    envelope(
        capability::SMS,
        SmsConversations::KIND,
        SmsConversations {
            conversations: Vec::new(),
        }
        .encode(),
    )
}

pub(crate) fn thread_request(thread: u64, before_ms: Option<u64>) -> Envelope {
    envelope(
        capability::SMS,
        SmsThreadRequest::KIND,
        SmsThreadRequest {
            thread,
            before_ms,
            limit: THREAD_PAGE,
        }
        .encode(),
    )
}

pub(crate) fn sms_send(thread: u64, body: &str) -> Envelope {
    envelope(
        capability::SMS,
        SmsSend::KIND,
        SmsSend {
            thread,
            body: body.to_owned(),
        }
        .encode(),
    )
}

pub(crate) fn call_command(action: CallAction) -> Envelope {
    envelope(
        capability::TELEPHONY,
        CallCommand::KIND,
        CallCommand { action }.encode(),
    )
}

/// The desktop's word for a call action, as `Devices1` takes it.
pub(crate) fn call_action(word: &str) -> Option<CallAction> {
    Some(match word {
        "Mute" => CallAction::Mute,
        "Answer" => CallAction::Answer,
        "HangUp" => CallAction::HangUp,
        _ => return None,
    })
}

/// The buttons a call notification offers, in the order the action index
/// names them, and the key its notification is tracked under.
pub(crate) fn call_buttons(state: CallState) -> Vec<(&'static str, CallAction)> {
    match state {
        CallState::Ringing => vec![
            ("Silenciar", CallAction::Mute),
            ("Responder", CallAction::Answer),
            ("Colgar", CallAction::HangUp),
        ],
        CallState::Answered => vec![("Colgar", CallAction::HangUp)],
        CallState::Missed | CallState::Ended => Vec::new(),
    }
}

pub(crate) const CALL_KEY: &str = "call";
pub(crate) const SMS_KEY_PREFIX: &str = "sms:";

/// The line the registry and the notification show for a call.
pub(crate) fn call_line(event: &CallEvent, resolved: Option<&str>) -> (String, String) {
    let who = event
        .name
        .as_deref()
        .or(resolved)
        .filter(|n| !n.is_empty())
        .unwrap_or(&event.number)
        .to_owned();
    let what = match event.state {
        CallState::Ringing => "Llamada entrante",
        CallState::Answered => "En llamada",
        CallState::Missed => "Llamada perdida",
        CallState::Ended => "Llamada terminada",
    };
    (what.to_owned(), who)
}

/// The registry's word for a call state.
pub(crate) fn call_state_word(state: CallState) -> &'static str {
    match state {
        CallState::Ringing => "ringing",
        CallState::Answered => "answered",
        CallState::Missed => "missed",
        CallState::Ended => "ended",
    }
}

/// A thread page from the phone replaces or extends the cache.
pub(crate) fn merge_thread(book: &mut DeviceBook, page: &SmsThread) {
    let entry = book.threads.entry(page.thread).or_default();
    for m in &page.messages {
        if !entry.iter().any(|held| held.id == m.id) {
            entry.push(m.clone());
        }
    }
    entry.sort_by_key(|m| m.timestamp_ms);
    if entry.len() > 500 {
        let drop = entry.len() - 500;
        entry.drain(..drop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnetita_proto::phone::contacts::Contact;

    fn card(id: u64, name: &str, tel: &str) -> Contact {
        Contact {
            id,
            version: 1,
            vcard: format!(
                "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:{name}\r\nTEL;TYPE=cell:{tel}\r\nEND:VCARD\r\n"
            ),
        }
    }

    #[test]
    fn the_book_resolves_numbers_with_or_without_the_country_code() {
        let mut book = DeviceBook::default();
        book.sync(&ContactsSync {
            version: 7,
            contacts: vec![
                card(1, "Ana", "+34 600 111 222"),
                card(2, "Bo", "tel:912345678"),
            ],
            removed: vec![],
            complete: true,
        });
        assert_eq!(book.contacts_version, 7);
        assert_eq!(book.resolve("600111222").as_deref(), Some("Ana"));
        assert_eq!(book.resolve("+34600111222").as_deref(), Some("Ana"));
        assert_eq!(book.resolve("0034912345678").as_deref(), Some("Bo"));
        assert_eq!(book.resolve("600999999"), None);
        book.sync(&ContactsSync {
            version: 8,
            contacts: vec![],
            removed: vec![1],
            complete: true,
        });
        assert_eq!(book.resolve("600111222"), None);
        assert_eq!(book.label(&["912345678".into(), "5555".into()]), "Bo, 5555");
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn a_received_message_moves_its_conversation_to_the_front_and_counts_unread() {
        let mut book = DeviceBook::default();
        book.conversations = vec![
            Conversation {
                thread: 1,
                addresses: vec!["1".into()],
                snippet: "a".into(),
                timestamp_ms: 10,
                unread: 0,
            },
            Conversation {
                thread: 2,
                addresses: vec!["2".into()],
                snippet: "b".into(),
                timestamp_ms: 20,
                unread: 0,
            },
        ];
        book.received(
            1,
            &SmsMessage {
                id: 9,
                from_me: false,
                address: "1".into(),
                body: "new".into(),
                timestamp_ms: 30,
                attachments: vec![],
            },
        );
        assert_eq!(book.conversations[0].thread, 1);
        assert_eq!(
            (
                book.conversations[0].unread,
                book.conversations[0].snippet.as_str()
            ),
            (1, "new")
        );
        book.received(
            3,
            &SmsMessage {
                id: 10,
                from_me: true,
                address: "3".into(),
                body: "mine".into(),
                timestamp_ms: 40,
                attachments: vec![],
            },
        );
        assert_eq!(
            (book.conversations[0].thread, book.conversations[0].unread),
            (3, 0)
        );
    }

    #[test]
    fn call_lines_and_buttons_follow_the_state() {
        let event = CallEvent {
            state: CallState::Ringing,
            number: "600111222".into(),
            name: None,
            timestamp_ms: 0,
        };
        assert_eq!(
            call_line(&event, Some("Ana")),
            ("Llamada entrante".into(), "Ana".into())
        );
        assert_eq!(call_line(&event, None).1, "600111222");
        assert_eq!(call_buttons(CallState::Ringing).len(), 3);
        assert_eq!(call_buttons(CallState::Answered).len(), 1);
        assert!(call_buttons(CallState::Missed).is_empty());
        assert_eq!(call_action("HangUp"), Some(CallAction::HangUp));
        assert_eq!(call_action("Dance"), None);
    }
}
