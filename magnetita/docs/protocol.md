# The Magnetita wire

The own protocol of [ADR 0001](decisions/0001-own-protocol-and-android-app.md),
as `magnetita-proto` implements it. The crate is the specification; this
document is its map. Every byte sequence named here is pinned by a test in
the crate, so a change to the wire is a change to a test, and the reverse.

## Transport and envelope

The link (`magnetita-link`, `MAG-P2`) is QUIC with mutual TLS on
self-signed certificates whose SHA-256 fingerprints the trust store pins.
On it, every message is one **envelope**, a CBOR map with integer keys:

| Key | Field | Type | Meaning |
|---|---|---|---|
| 0 | version | u8 | `1`. Refused before any other field is read when it is not. |
| 1 | capability | u16 | Which capability's module owns the body. |
| 2 | kind | u16 | Which message of that capability. |
| 3 | id | u32 | A per-connection counter; a reply names the id it answers. |
| 4 | body | bytes | The capability's message, at most 1 MiB. |

Vector: `a500010101021903e8031a00bc614e0443010203` is capability 1, kind
1000, id 12 345 678, body `01 02 03`.

Unknown keys are skipped in every map. A missing required key is refused by
name. Indefinite-length containers are refused. An input over 1 MiB + 4 KiB
is refused before decoding starts.

## The bound rule

No length chosen by the peer is trusted until it has been checked against
its bound, and the check happens on the CBOR header, before the value is
copied. The bounds:

| Bound | Bytes or items | Used for |
|---|---|---|
| `MAX_IDENT` | 128 | ids, names, keys, numbers, MIME types, addresses |
| `MAX_TEXT` | 4096 | titles, bodies, snippets, typed text |
| `MAX_LIST` | 256 | any list or map |
| `MAX_BYTES` | 1 MiB | an envelope body, a proof |
| `MAX_CLIPBOARD` | 256 KiB | clipboard text |
| `MAX_ICON` | 64 KiB | a notification icon |
| `MAX_FILENAME` | 255 | a file name, which may not contain `/` or NUL |
| `MAX_VCARD` | 16 KiB | one contact |

## Hello and negotiation

Capability `0` kind `0`, the first envelope both ways:

| Key | Field | Type |
|---|---|---|
| 0 | device_id | text ≤ 128 |
| 1 | device_name | text ≤ 128 |
| 2 | device_kind | u8: 0 desktop, 1 phone |
| 3 | capabilities | list of `[capability u16, version u16]`, ≤ 256, no duplicates |

Both sides keep the capabilities both offer, each at the lower version.
Anything only one side offers is not used and is not an error.

Vector: `a4007030656232316232386165373464353463016a4573637269746f72696f02000382820101820902`.

## Capabilities

| Id | Capability | Kinds |
|---|---|---|
| 0 | hello | 0 hello |
| 1 | battery | 1 status `{0 level u8, 1 charging, 2 low}`; 2 request `{}` |
| 2 | clipboard | 1 text `{0 text ≤ 256 KiB}`; 2 request `{}` |
| 3 | notifications | 1 posted `{0 key, 1 app_name, 2 title, 3 body, 4 timestamp_ms u64, 5 replyable, 6 actions [{0 label}], 7 icon bytes?}`; 2 dismissed `{0 key}`; 3 action `{0 key, 1 action u16}`; 4 reply `{0 key, 2 text}` |
| 4 | find | 1 ring `{}`; 2 stop `{}` |
| 5 | share | 1 offer `{0 transfer u32, 1 name, 2 size u64, 3 mime}`; 2 accept `{0 transfer, 1 offset u64}`; 3 reject `{0 transfer}`; 4 done `{0 transfer, 2 complete}`; 5 text `{0 text}` |
| 6 | media | 1 state `{0 player, 1 title, 2 artist, 3 album, 4 playing, 5 position_ms, 6 length_ms, 7 can_seek, 8 can_next, 9 can_previous, 10 volume u8}`; 2 command `{0 player, 1 button u8?, 2 seek_ms?, 3 volume?}` with at least one of the three; 3 request `{}` |
| 7 | commands | 1 list `{0 [{0 id u32, 1 name}]}`, ids unique; 2 run `{0 id}`; 3 result `{0 id, 1 ok}` |
| 8 | input | 1 move `{0 dx i16, 1 dy i16}` (also as a datagram); 2 button `{0 button u8, 1 pressed}`; 3 scroll `{0 dx, 1 dy}`; 4 key `{0 evdev code u16, 1 pressed}`; 5 text `{0 text}` |
| 9 | mirror | 1 start `{0 max_size u16, 1 fps u8 > 0, 2 bitrate_kbps u32, 3 codec u8, 4 audio}`; 2 started `{0 width, 1 height, 2 codec, 3 audio?}`; 3 stop `{}`; 4 touch `{0 action u8, 1 x, 2 y, 3 pointer u8}`; 5 key `{0 android keycode u16, 1 pressed}`; 6 global `{0 action u8: back, home, recents}` |
| 10 | sms | 1 conversations `{0 [{0 thread u64, 1 addresses [text], 2 snippet, 3 timestamp_ms, 4 unread u16}]}` (an empty list from the desktop asks for the phone's); 2 thread request `{0 thread, 1 before_ms?, 2 limit u16 in 1..=256}`; 3 thread `{0 thread, 1 [message]}`; 4 send `{0 thread, 1 body non-empty}`; 5 received `{0 thread, 1 message}` |
| 11 | contacts | 1 request `{0 since_version u64}`; 2 sync `{0 version, 1 [{0 id, 1 version, 2 vcard ≤ 16 KiB}], 2 removed [u64], 3 complete}` |
| 12 | telephony | 1 event `{0 state u8: ringing, answered, missed, ended; 1 number, 2 name?, 3 timestamp_ms}`; 2 command `{0 action u8: mute, answer, hang up}` |
| 13 | storage | 1 state `{0 available}` (phone, on session open and grant change); 2 list `{0 request u32, 1 path, 2 offset u32}`; 3 listing `{0 request, 1 [{0 name, 1 dir, 2 size u64, 3 mtime_ms u64}] ≤ 256, 2 more, 3 error}`; 4 stat `{0 request, 1 path}`; 5 stat reply `{0 request, 1 entry?}`; 6 read `{0 request, 1 path, 2 offset u64, 3 len u32 ≤ 1 MiB − 8 KiB}`; 7 data `{0 request, 1 bytes, 2 error}`; 8 write `{0 request, 1 path, 2 offset, 3 bytes ≤ 1 MiB − 8 KiB, 4 truncate}`; 9 done `{0 request, 1 ok, 2 error}`; 10 mkdir `{0 request, 1 path}`; 11 rename `{0 request, 1 from, 2 to}`; 12 delete `{0 request, 1 path}`. Paths are relative to the shared root, `/`-separated, no empty, `.` or `..` component; the root is the empty path |
| 14 | pairing | 1 qr proof `{0 mac 32}`; 2 qr reply `{0 mac 32}`; 3 code exchange `{0 spake2 ≤ 64}`; 4 code confirm `{0 mac 32}` |

An SMS `message` is `{0 id u64, 1 from_me, 2 address, 3 body, 4
timestamp_ms, 5 attachments [{0 transfer u32, 1 mime}]}`; an attachment's
bytes arrive on the `share` transfer of that id. The mirror's video and
audio, and every `share` transfer, travel on their own QUIC streams as raw
bytes; the messages above open, describe and close those streams. The
mirror's streams carry fixed transfer ids no share ever uses: video on
`0xFFFF0001`, audio on `0xFFFF0002`; the video is the encoder's raw Annex B
output (HEVC or H.264 as `started` says) with its parameter sets first,
and closing the stream is the picture's end.

## Pairing

Before trust, on capability 14. **QR:** the desktop shows
`magnetita://pair?v=1&id=…&fp=<sha256 hex>&secret=<32 bytes hex>&addr=ip:port…`;
the phone dials with the fingerprint pinned from the QR and sends
`HMAC-SHA256(secret, "magnetita-qr-proof" ‖ phone_fp ‖ desktop_fp)`; the
desktop verifies and answers with the same over `desktop_fp ‖ phone_fp`.
**Code:** six ASCII digits, SPAKE2 over Ed25519 with the desktop as side A
and identities `magnetita-desktop` / `magnetita-phone`; each side confirms
with `HMAC-SHA256(key, "magnetita-code-confirm-<role>" ‖ my_fp ‖ peer_fp)`.
Both paths pin the peer's fingerprint; every state machine ends on its
first failure.

## Versioning

Adding a key to a map, a kind to a capability, or a capability to the hello
is compatible and needs no version change: old peers skip, decline or
ignore. Changing the meaning of an existing key or kind is not; it bumps
the envelope version, and a peer that does not speak it is refused before
anything else is read.
