# MAG-P0 — Spikes that verify the accepted choices

- **Opened:** 2026-09-07
- **Closed:** 2026-09-09
- **Plan ID:** own-protocol-spikes
- **Status:** done
- **Authorization:** the author accepted
  [ADR 0001](../../decisions/0001-own-protocol-and-android-app.md) on
  2026-09-04 and said "inicia" on 2026-09-07, which moved the roadmap's single
  active checkpoint from `MAG-S1` to `MAG-P0`
- **Scope:** magnetita
- **Implementation checkpoint:** MAG-P0
- **Author-validation checkpoint:** none
- **Successor:** MAG-P1. The nest half of `MAG-P0-D` — pointer motion and a
  typed sentence injected through `uinput` into the development nest — was
  not measured because no nest was open during the checkpoint; it is the
  exact proof `MAG-P5-B` requires and is delivered there, not as a dangling
  unit here

## Hypothesis

Every material choice in ADR 0001 has one measurement that verifies it, and
each can be taken before a single product crate exists, in a scratch
directory that is deleted afterwards with only its numbers kept.

## Tangible outcome

Five dated evidence records with numbers under `docs/evidence/`, each cited
from the discussion it verifies; any number that fails its threshold applies
the ADR's named *Revisit when* fallback before `MAG-P1` opens.

## Scope

- `MAG-P0-A` — QUIC on both ends. Desktop half: a scratch `quinn` client and
  server on loopback, round-trip of a 32-byte request on its own stream
  while a 200 MB stream is in flight, p50/p99, against the same test with
  no load. Phone half: the same core built for `aarch64-linux-android` with
  `cargo-ndk`, wrapped by UniFFI, exchanging a hello over the author's LAN
  and surviving a Wi-Fi toggle.
- `MAG-P0-B` — a throwaway Kotlin activity: `MediaProjection` →
  `MediaCodec` HEVC → QUIC → a throwaway desktop decoder; glass-to-glass
  latency at 1080p60 measured with a millisecond clock on both screens.
- `MAG-P0-C` — a throwaway accessibility service: tap and drag latency from
  a desktop event to the gesture landing.
- `MAG-P0-D` — `/dev/uinput` access on the author's host; the RemoteDesktop
  portal's presence under niri; a virtual pointer and keyboard created and
  destroyed; motion and a typed sentence observed in the development nest
  only, never the live session.
- `MAG-P0-E` — QR pairing on the S25U and typed pairing against a headless
  peer, each with no other input.
- `MAG-P0-F` — each evidence record cited from its discussion; any
  triggered fallback applied to the ADR.

## Exclusions

- Any code under `celestina-rs/crates/magnetita-*` or `magnetita/src`,
  `magnetita/qml`; spikes live in the session scratch directory.
- The installed daemon, the live session, the paired trust, the phone's
  settings.
- Host changes. The phone-side spikes need `rustup` with the
  `aarch64-linux-android` target, an Android NDK, an SDK with one platform
  and build-tools, and `cargo-ndk` (`uniffi-bindgen` is a per-crate binary,
  not a host tool). On 2026-09-08 the author installed `rustup 1.29.1`, the
  target, `cargo-ndk 4.1.2` and NDK 30.0.16248370 inside the Flatpak Android
  Studio's SDK (`~/.var/app/com.google.AndroidStudio/data/Android/Sdk`),
  and created the scratch project `AndroidStudioProjects/Magnetita` there;
  `HOST-HYGIENE.md` still has to record them. The agent changes nothing on
  the host.

## Build order

1. `MAG-P0-D` availability and device create/destroy, and the desktop half
   of `MAG-P0-A`: neither needs the phone or a host change.
2. The nest half of `MAG-P0-D` when a nest is running.
3. `MAG-P0-A` phone half once the toolchain exists; then `MAG-P0-E`, which
   reuses its build.
4. `MAG-P0-B` and `MAG-P0-C`, which need the author present for the consent
   dialog and the clock.
5. `MAG-P0-F`.

## Implementation exit

Every evidence record carries its measurement, each discussion's conclusion
cites its record, and any fallback the numbers trigger is applied to ADR 0001. Met on
2026-09-09: no fallback was triggered. The author judges the mirror and input numbers by reading them;
no `VAL-MAG` entry is opened because nothing here is installed or shipped.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| MAG-P0-A | `magnetita:` | done | [inventory](../../inventories/2026-09-07-own-protocol-spikes/MAG-P0-A.numstat.tsv) | 5 files, +369/-0 | QUIC round-trip under load on loopback and on the phone; a session across a Wi-Fi toggle | [record](../../evidence/2026-09-08-quic-on-the-phone.md) | None |
| MAG-P0-B | `magnetita:` | done | [inventory](../../inventories/2026-09-07-own-protocol-spikes/MAG-P0-B.numstat.tsv) | 4 files, +305/-0 | Own-app mirror latency on the S25U | [record](../../evidence/2026-09-09-own-mirror-latency.md) | None |
| MAG-P0-C | `magnetita:` | done | [inventory](../../inventories/2026-09-07-own-protocol-spikes/MAG-P0-C.numstat.tsv) | 3 files, +193/-0 | Accessibility-service input latency | [record](../../evidence/2026-09-09-accessibility-input-latency.md) | None |
| MAG-P0-D | `magnetita:` | done | [inventory](../../inventories/2026-09-07-own-protocol-spikes/MAG-P0-D.numstat.tsv) | 4 files, +240/-0 | `uinput` access, portal presence, device create/destroy | [record](../../evidence/2026-09-07-uinput-and-portal.md) | None |
| MAG-P0-E | `magnetita:` | done | [inventory](../../inventories/2026-09-07-own-protocol-spikes/MAG-P0-E.numstat.tsv) | 4 files, +259/-0 | QR and typed pairing each pair once | [record](../../evidence/2026-09-08-qr-and-code-pairing.md) | None |
| MAG-P0-F | `magnetita:` | done | [inventory](../../inventories/2026-09-07-own-protocol-spikes/MAG-P0-F.numstat.tsv) | 16 files, +1007/-149 | Conclusions cite records; no fallback triggered; program records pass the guards | [record](../../evidence/2026-09-09-spike-program-bookkeeping.md) | None |

`MAG-P0-A` through `-F` were measured on 2026-09-07 to 2026-09-09 and closed
in one batch (`ad26b3e`).
