# R7 — wallpaper and session look

- **Opened:** 2026-08-04
- **Plan ID:** r7-session-look
- **Closed:** 2026-08-04
- **Successor:** none; the roadmap is idle until a later checkpoint opens its own plan
- **Status:** done
- **Scope:** celestina
- **Implementation checkpoint:** R7
- **Author-validation checkpoint:** `VAL-R7` in [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

The look of this session has one source — the sealed `CelestinaTheme` contract —
and everything that paints from it can be derived rather than restated: the
wallpaper the shell maps, the values it serves to portals, and the colours Niri
itself draws with.

## Tangible outcome

Each output carries a wallpaper the shell maps itself, with an honest fallback
when there is no image to show; the session's colour scheme and accent are
answered from the shell rather than guessed by each application; and the Niri
colour include is generated from the same tokens the panel uses, so the
compositor's borders can never drift from the shell's.

## Scope

In scope: the pure wallpaper-selection policy and its fallback; per-output
background surfaces; the `Settings` portal values the shell owns; generation of
the Niri colour include from the sealed tokens, written to a file the author
chooses to include or not.

## Exclusions

Out of scope: editing the author's live Niri configuration — the include is
generated and left for the author to reference; slideshows, transitions between
wallpapers beyond a reduced-motion-aware fade, and per-workspace wallpapers;
becoming the session's only portal backend; and any change to the sealed theme
itself, which needs its own decision record.

## Build order

1. Add the pure wallpaper-selection policy and the Niri colour-include
   generator to `celestina-shell-core`, with tests.
2. Map one background surface per output, with the honest fallback and the
   reduced-motion path.
3. Serve the `Settings` portal values the shell owns.
4. Write the generated colour include where the author can reference it, with
   instructions and a rollback.

## Implementation exit

- Wallpaper selection, its fallback and its bounds are tested, including a
  directory with nothing showable in it.
- A background surface exists per output and survives hotplug; offscreen tests
  prove no compositor decision.
- The colour include is generated from the sealed tokens and is byte-identical
  across runs, proved by a test rather than by inspection.
- The portal values match the tokens they claim to come from.
- CMake registration, QML lint and CTest pass.
- Rust format, Clippy and package tests pass; the lockfile changes only by a
  dependency this plan declares.
- The architecture and documentation contracts pass.
- `scripts/complete-production.sh` builds once, verifies those exact bytes and
  updates the on-disk bundle; the live session is never replaced.

R7 implementation closes on this evidence. Real wallpaper appearance, hotplug on
physical monitors, portal consumers actually reading the values and Niri drawing
the generated colours remain an independent `VAL-R7` run.

## Change and commit ledger

Update before editing a slice and again when its diff is ready. Paths and
stable symbols are authoritative; line counts are a hand-off aid and may drift.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| R7-A | `celestina:` | done | [inventory](../../inventories/2026-08-04-r7-session-look/R7-A.numstat.tsv) | 27 files, +1684/-11 | Which image belongs on which screen and what an output with none shows; one background surface per output; the appearance keys answered under this shell's own backend name; and Niri's colours generated from the sealed tokens with a guard that refuses to let them drift | [R7 session look](../../evidence/2026-08-04-r7-session-look.md) | `VAL-R7` |

The four build-order steps closed as one unit, as R3's, R4's and R5's did: each
`done` unit needs one exclusive inventory *and* one exclusive evidence record,
and one verification run does not honestly produce four.

## Decisions and rollback

The wallpaper is *mapped by this shell*, not set on the compositor: a background
layer-shell surface is something the shell can withdraw, and withdrawing it
leaves the session exactly as it was. Nothing here writes to the author's Niri
configuration.

An output with no showable image gets a painted fallback rather than a black
rectangle pretending to be a photograph, and rather than the last output's
image: a wallpaper that silently belongs to another screen is a lie about which
file is being shown.

The Niri colour include is generated to a file and *referenced* by the author,
never injected. That keeps the rollback to deleting one `include` line, and
keeps this shell from editing a configuration it does not own.

The appearance backend claims `…impl.portal.desktop.celestina-shell`, not
`…desktop.celestina`. Siderita already owns the latter for the file chooser,
with its own installed `celestina.portal`. A shared name would have meant one
of the two failing to start, and the generated registration file — first
written as `celestina.portal` — would have overwritten Siderita's and taken the
session's file chooser away. Two backends serving different interfaces is what
the portal supports; sharing one name is not.

The generator carries the sealed values as constants, which is itself a second
place the palette is written down — the exact drift it exists to prevent. So
`scripts/check-sealed-colours.py` reads both the theme and the generator and
refuses a mismatch, and it runs inside the architecture contract. It earned its
place immediately: it caught this unit shipping the surface colour where the
compositor fallback belonged.
