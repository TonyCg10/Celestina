# Contacts sync and name resolution on the own wire — MAG-P4-E

- **Date:** 2026-09-09
- **Scope:** `MAG-P4-E` of
  [`../plans/active/2026-09-09-daily-set.md`](../plans/active/2026-09-09-daily-set.md):
  the contact book in `celestina-rs/crates/magnetitad/src/link_wire/phone.rs`,
  the `contacts` setting, the session's greeting and handler in
  `link_wire/mod.rs`, the core's `send_contacts` and request decoding, the
  peer's `--contact`, this record
- **Environment:** the workspace's loopback tests; the daemon deployed
  through `magnetita/scripts/complete-production.sh`; the S25U with the
  application of `AND-2-E`
- **Artifact:** `magnetitad`, deployed

## Design

- The session opens by asking for the contacts changed since the version
  the daemon's book holds for that device (zero after a restart: nothing
  is kept on disk). Each page of vCards lands in the book: the `FN` and
  every `TEL`, digits only; a vCard without a name drops the contact, a
  removed id drops it, and the page that says `complete` sets the version.
- `resolve(number)` matches a number to a contact when the digits are
  equal or when the last nine digits are, so `+34 600 111 222` and
  `600111222` are the same line. SMS conversations and calls use it for
  their labels; nothing else reads the book.
- The book lives in one store per daemon, in memory, and is dropped with
  the session.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetitad
magnetita-peer connect 10.0.0.134:1760 --contact "Ana|+34600111222" --sms "600111222|hi" --hold 5
```

## Result

- **Exit:** 0. The book's tests pin the parsing, the matching with and
  without a country code, the removal, and the label of a conversation;
  the loopback test of `MAG-P4-F` and `-G` shows the resolved name on the
  SMS notification and the call.
- **On the S25U:** with the contact grant, the phone answered the contacts
  request as the session opened; the daemon's log names the count and
  the version.

## Limits

- Deletions are not seen until the daemon restarts and asks from zero:
  the phone's provider does not hand a one-way reader the removed ids.
- The book is the phone's names only; the desktop's own address book is
  not consulted.
