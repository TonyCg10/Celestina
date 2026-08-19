# Siderita implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** SID-A1
- **Related author validation:** `VAL-SID-G7`, `VAL-SID-04` and `VAL-SID-07` in
  [VALIDATION.md](VALIDATION.md); none of them blocks implementation

`SID-M1` remains the next settled checkpoint after `SID-G7` and `SID-A1`, and
has no active execution plan.

## SID-G7 — Shared reading surface in both text panes

The falsifiable problem the author demonstrated: Siderita opens text in two
places — the embedded Grafita editor and the quick look's text pane — and
neither gives a reader a line to refer to, a scroll position drawn by the suite,
or the text size they chose in Grafita. The quick look additionally reached for
raw `QtQuick.Controls` `ScrollView` and `TextArea`, which the architecture
baseline carries as debt.

The boundary is `grafita-core` for every document rule and Siderita's own `src/`
for the Qt marshalling, exactly as each host already adapts `DocumentSession`.
The tangible outcome is two text surfaces that number their lines, scroll with
the shared bar, report the caret's line and character column, honour the stored
text size, and retire two raw-control baseline rows instead of adding any.

The plan is
[Shared reading surface](docs/plans/active/2026-08-04-shared-reading-surface.md).
It excludes a settings surface, any preference Siderita owns for itself, and the
components themselves, which are `STYLE-G7`'s.

## SID-A1 — Compressing and extracting archives

The falsifiable problem: Siderita could not open a `.zip` or make one. Every
other verb it owns is loss-free and cancellable, and the two archive verbs a
file manager is expected to have were simply absent, so the person left the
application to run `tar` by hand.

The boundary is a new pure crate, `siderita-archive`, beside `siderita-ops` and
holding the same guarantees: nothing existing is overwritten, nothing partial is
left claiming to be complete, and — the rule only an archive needs — no member
may be written outside the folder the person chose. The containers are pure
Rust, so an extraction never spawns a process and never depends on a tool being
installed. The tangible outcome is an extract verb and a compress verb on the entry
menu, running on the same progress surface and Cancel button a paste uses.

The plan is
[Compressing and extracting archives](docs/plans/active/2026-08-18-archive-compression.md).
It excludes browsing inside an archive, encrypted or split containers, `.rar`
and `.7z`, and single-file `.gz`/`.xz`/`.zst`.

## SID-M1 — Parent portal pickers on Wayland

## Hypothesis and tangible outcome

Importing the portal's `wayland:` parent handle through a bounded
`xdg-foreign` adapter will make each picker a transient child of its requester
without coupling the portal contract to QML ids or blocking D-Bus. The tangible
outcome is an inspectable picker lifecycle that accepts valid handles, degrades
on invalid/unsupported ones and still answers every request.

## Scope

- Parse and validate the portal parent handle without trusting arbitrary input.
- Add the smallest Qt/Wayland seam required to import the foreign parent and
  apply it before the picker maps.
- Preserve concurrent picker requests, cancellation and reply delivery.
- Degrade to the current free-floating picker when the protocol or handle is
  unavailable; never fail the file request solely because parenting failed.
- Add focused parser/lifecycle tests and update the portal contract/status.

## Exclusions

- Changing portal routing or the author's `portals.conf`.
- Installing/activating the backend during verification.
- Redesigning picker browsing, adding file operations or moving it into the main
  window.
- Manual requester/Wayland acceptance, tracked as `VAL-SID-04`.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| SID-M1-A | planned | none | Typed bounded parser and invalid-handle fallbacks | Focused Rust/C++ tests |
| SID-M1-B | planned | SID-M1-A | Minimal imported-parent lifecycle wired before map | Qt lifecycle test where headless support permits |
| SID-M1-C | planned | SID-M1-B | Portal remains compatible and the author's binary carries the verified bytes | `scripts/complete-production.sh` |

## Implementation exit

Close `SID-M1` when valid/invalid/unsupported handle paths are covered, portal
requests still answer and `scripts/complete-production.sh` builds, verifies and
deploys those exact bytes so the author's normal binary needs no rebuild. Do not
keep the checkpoint open for the real Wayland parent-child observation; that
result belongs to `VAL-SID-04`.

## Closed evidence

CP0-CP7, including both content integrations, are preserved in the
[roadmap history](docs/history/roadmap-through-2026-08-03.md).
