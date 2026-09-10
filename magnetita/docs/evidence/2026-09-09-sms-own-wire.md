# SMS conversations, send, receive on the own wire — MAG-P4-F

- **Date:** 2026-09-09
- **Scope:** `MAG-P4-F` of
  [`../plans/active/2026-09-09-daily-set.md`](../plans/active/2026-09-09-daily-set.md):
  the conversation and thread cache in `link_wire/phone.rs`, the handler
  and the commands in `link_wire/mod.rs`, `Devices1.SmsConversations`,
  `SmsThread` and `SmsSend`, the `sms` setting, the desktop app's
  messages page (`magnetita/src/messages.rs`, `qml/pages/MessagesPage.qml`,
  `qml/components/{ConversationRow,MessageBubble}.qml`, the header's
  toggle), the core's sends and decoding, the peer's `--sms`, the wire
  document's note on the list request, this record
- **Environment:** as `MAG-P4-E`
- **Artifact:** `magnetitad` and the desktop app, deployed

## Design

- The session asks for the conversation list as it opens (an empty list
  from the desktop is the request; only the phone fills it). The daemon
  keeps the list and the pages of threads it asked for, in memory; a
  received message goes to the front of its conversation, into the cached
  page, and counts as unread when it is not ours.
- A received message that is not ours is shown through the notification
  bridge under the key `sms:<thread>`, titled by the contact book's name
  or the number, replyable: a reply on it becomes `SmsSend` in that
  thread. `Devices1.SmsSend` does the same from the desktop app.
- The desktop app gains a messages page: the conversation list with
  unread counts, one thread as bubbles (ours in the selected surface,
  theirs on the grouped one), a field and a send glyph; it re-reads every
  three seconds while shown, since the daemon signals only `Changed`.
- MMS attachments are counted on the wire and not fetched yet.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetitad
cd magnetita && cargo test && cargo build --release --locked && ../scripts/qmllint-cxxqt.sh .
busctl --user --json=short call … SmsConversations s 4eb6f9984054dd25
```

## Result

- **Exit:** 0. The loopback test `contacts_name_the_sms_and_the_call_and_the_desktop_answers_both`
  sends a contact and a received SMS, sees the notification titled with
  the contact's name and replyable, replies on it and receives `SmsSend`
  with the thread and the text on the phone side. The desktop app builds
  with its new page; `qmllint` passes.
- **On the S25U:** with the SMS grant, the phone answered the conversation
  request as the session opened; `Devices1.SmsConversations` lists them
  (their count is recorded, not their content).

## Limits

- A message sent from the desktop appears in the desktop's thread through
  the wire, not in the phone's own messaging app: only the default SMS
  app may write the phone's provider.
- Attachments are announced, not transferred; the share streams for them
  are a later refinement.
- Sending and receiving with a real correspondent is `VAL-MAG-12`.
