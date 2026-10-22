# CEL-2-J — Phone and minimised windows: implementation plan

- **Date:** 2026-10-17
- **Status:** approved and executed by the author's go-ahead; kept as the unit's plan record, and where it differs from what was built [the evidence](2026-10-17-phone-and-bubbles.md) is authoritative
- **Artifact:** none; a plan record, not a build
- **Scope:** unit CEL-2-J, as planned before implementation
- **Environment:** none; nothing in this record was run
- **Evidence:** [2026-10-17-phone-and-bubbles.md](2026-10-17-phone-and-bubbles.md); decisions in [ADR 0020](../decisions/0020-phone-and-bubbles.md)
- **Unit:** CEL-2-J of the ledger [the ledger](../plans/active/2026-10-16-cel-2-parity.md)
  (prefix `celestina:`, project `celestina`)
- **Design:** [the design](../../../docs/superpowers/specs/2026-10-16-celestina-parity-program-design.md)
  (§4 principles, §5 row J, §7 row J, §8 extension points)
- **Goal:** two duties, `phone` and `bubbles`, both **off by default**
  (design §4.1): the Magnetita piece (battery, connection, media of the
  author's phone) with an Island page, and the bubble group of minimised
  windows with the `Mod+K` selector, over Melibea's socket.
- **Salvage (copy, never edit the halted tree; head comment records source
  path and commit):** `celestina-qt/src/provider_adapter/melibea.rs` as
  reference for `celestina/src/providers/melibea.rs`;
  `celestina-rs/crates/celestina-shell-core/src/melibea.rs` into
  `celestina-rs/crates/celestina-shell/src/melibea.rs`. The line for J must be
  on `main` (the "Salvamento CEL-2 restante" unit) before the first commit.
- **Safety:** nothing here is ever tested against the real niri, the real
  Melibea or the real Magnetita: tests use a fake Melibea socket
  (`MELIBEA_SOCKET` in a temp dir) and a private session bus. No real window
  is minimised in any test or smoke.

## Design decisions (closing spec §7 row J)

### D1 — Melibea protocol version: v1 by default, v2 per action, degrade to v1

- `subscribe`, `list`, plain `restore`, `close` and `minimize` speak **v1**
  (the daemon accepts 1 and 2; a v1-only Melibea keeps working).
- A request that carries a transition hint (anchored motion from the bubble's
  rectangle, or `disabled` when `Appearance` asks for reduced motion) speaks
  **v2**. If the daemon answers `incompatible_version` (its
  `supported_versions` lacks 2), the worker **remembers v1-only for the
  connection's life** and resends the same action as v1 without the hint.
  A transition is only a presentation hint, so degrading never changes the
  state asked for. Reconnecting forgets the memory (the daemon may have been
  upgraded).
- The anchor is sent whenever the bubble's rectangle is a usable one and the
  shell does not know the window to be on another output. The window is
  usually not in niri's model (Melibea keeps it), so the ordinary case is an
  unknown output and the anchor goes. When the model does name the window's
  output and it differs from the bubble's, no anchor is sent: `Disabled`
  under reduced motion, otherwise no hint (niri's ordinary local motion). An
  anchor on an output niri cannot place is degraded by Melibea or niri.
- Reply versions 1 and 2 are both read (`SUPPORTED_PROTOCOL_VERSIONS`).

### D2 — Guard when `melibea.service` is absent

The shell never starts, stops or probes the unit (no `systemctl`, no spawn).
The only observable is the socket: `$MELIBEA_SOCKET` (absolute path required)
else `$XDG_RUNTIME_DIR/melibea.sock`.

- The path must exist, be a socket (not a regular file; a symbolic link is
  **always refused**, whatever it points at), owned by the current uid and not group/other accessible;
  anything else is `Absent`/`Refused`, with one journal line per change of
  state, never per attempt.
- `Absent` is a normal state, not an error: the `bubbles` piece draws nothing,
  the selector opens with a one-line Spanish note, the duty stays on, and
  reconnection backs off 0.5 s → 30 s (doubling) with a flag-checked sleep so
  the worker joins in at most `JOIN_WAIT`.
- A connection lost mid-run keeps the last projection out of the UI
  (`Unavailable` withdraws the bubbles) and then follows the same backoff.
- Magnetita has the same shape: a missing bus name is `Absent`, the piece
  draws nothing, and `Changed`/owner-change signals bring it back live.

### D3 — Selector keyboard model

`Mod+K` runs `celestina bubbles select`. The selector **reuses the launcher's
overlay** (`celestina-overlay`, keyboard `Exclusive`, one card, a search field
and up to eight rows) with a *bubbles source* instead of the applications:

- Type to filter (title, then app id); `Up`/`Down` and `Tab`/`Shift+Tab` move
  the selection with wrap; `Enter` or a click **restores** the selected window
  and closes; `Delete` on a selected row **asks the window to close**
  (`close_requested`; the row leaves only when the authoritative revision
  removes it, as Melibea's protocol says), but only while the search field is
  empty or its caret is at the end with nothing selected, where `Delete`
  would do nothing in the field; anywhere else `Delete` edits the text. A
  second `Delete` on a window whose close is still pending asks for nothing;
  the pending request ends when its action settles, when the projection no
  longer holds the window, or when the link is withdrawn; `Escape`, a press outside the card
  and keyboard-focus loss close without acting. `celestina bubbles select`
  again while open closes it (toggle).
- Focus starts in the field; the first row is selected so `Mod+K, Enter`
  restores the most recently minimised window (Melibea's order).
- No digit shortcuts, no multi-select, no minimise-from-selector. A
  separate verb `celestina bubbles minimize` minimises the focused window
  (`window_id: null`) for the author's key binding.
- An empty or absent source shows one non-interactive Spanish note row.

## Architecture (where each part lives)

| Part | Destination | Why |
|---|---|---|
| Protocol, projection, request building | `celestina-rs/crates/celestina-shell/src/melibea.rs` (copy of the core's) | pure, tested domain |
| Phone reduction (pick device, clamp, bound text) | `celestina-shell/src/phone.rs` (new) | pure |
| Duties `phone`, `bubbles` | `celestina-shell/src/settings.rs` `Duties` | one sorted line each, additive |
| Melibea socket worker | `celestina/src/providers/melibea.rs` | blocking IO off the loop |
| Magnetita bus worker | `celestina/src/providers/magnetita.rs` | blocking zbus off the loop |
| Wiring, live duty toggle | `celestina/src/phone/mod.rs`, `celestina/src/bubbles/mod.rs` (handles like `EnergyHandle`) | duty on = worker, off = none |
| Pieces | `celestina/src/pieces/{phone,bubbles}.rs` + `PIECES` | contract of `pieces/mod.rs` |
| Phone page | `celestina/src/island/pages/phone.rs` | Island page |
| Selector | `celestina/src/launcher/` bubbles source + `Row::Bubble` | reuse overlay, no second surface |
| Spanish copy | `celestina/src/copy.rs` | product copy rule |
| Verbs | `celestina/src/command.rs` (`bubbles select\|minimize`, `island open phone`) | additive |

No new Cargo dependency is expected (`zbus`, `serde_json`, `rustix` exist).
`magnetita-*` crates are **not** imported: the shared contract is the stable
`org.celestina.Devices1` interface (architecture table, inter-process
contract), like Siderita's own reader; Task 3 records that comparison.

## Tasks

Each task lands tests in the same unit; `cargo fmt --check`, `cargo clippy
--all-targets -- -D warnings` and the crate's tests pass before the next.

1. **Pure domain.** Copy the core's `melibea.rs` (and its tests) into
   `celestina-shell` with the salvage head comment; add `phone.rs`; add the two
   duties (a file from 1.3.0 reads with both off; unknown values read as off).
   Tests: carried tests, phone reduction (no device, several, battery -1 and
   >100, hostile long strings), duties default.
2. **Melibea worker** (`providers/melibea.rs`): socket guard (D2), bounded
   line reads (64 KiB), subscription loop applying the projection, backoff,
   v1/v2 degrade (D1), bounded action queue with short-lived connections,
   timeouts, deterministic shutdown, publish only current results. Tests
   against a fake server: snapshot+changes, gap → resync, `unavailable`,
   `incompatible_version` degrade, regular-file path refused, missing socket
   backs off, hostile oversized line, action confirmed only by the revision.
3. **Magnetita worker** (`providers/magnetita.rs`): `ListDevices` +
   `Changed` + name-owner watch on the session bus, reconcile on any signal
   (coalesced), `MediaAction` and `Ring` out only for a device it listed,
   bounded dict parsing. Tests on a private bus with a fake service.
4. **Wiring.** `Message::{Melibea,Phone}` variants, app state, handles that
   start/stop the workers when `duties.phone`/`duties.bubbles` change on the
   one `shell.json` follower; `BarCx` gains `phone` and `bubbles` slices;
   verbs. Additive hunks only in `app.rs`, `command.rs`, `message.rs`.
5. **Pieces and page.** Phone piece (battery glyph + percent; connection;
   absent = nothing) and page (name, battery, charging, now playing with
   previous/play-pause/next); bubbles piece (one bubble per window, icon by
   `icon_name`/`app_id`, at most N visible, overflow bubble opens the
   selector, click restores, anchored transition from the node's rect). All
   words in Spanish via `copy.rs`; colours from `Theme`; every action also
   reachable from the page/selector/verb (the contract's keyboard rule).
6. **Selector** (D3) in the launcher module: bubbles source, `Row::Bubble`,
   Delete handling, toggle verb, tests for filter/selection/wrap/restore/
   close-request/empty/absent.
7. **Smoke, evidence, documents.** Three nested runs in `smoke.sh` (metrics, verbs, absent) with
   both duties on, `MELIBEA_SOCKET` at a fake server script (a one-window
   snapshot, `python3 -I`) in a temp `XDG_RUNTIME_DIR`, a private bus (no
   Magnetita → absent), `celestina bubbles select` opening the overlay and
   closing it; no frame at rest. Evidence file `docs/evidence/
   2026-10-17-phone-and-bubbles.md`, ADR `0020` recording D1–D3, ledger row
   (`active` → pending `done` figures filled by the landing), STATUS/ROADMAP
   and VALIDATION rows `VAL-CEL2-PHONE`, `VAL-CEL2-BUBBLES`,
   `VAL-CEL2-BUBBLES-KEYS` queued for the author.
8. **Review and verification.** `celestina/scripts/verify-production.sh` and
   `scripts/smoke.sh` green in the worktree; a final review by a model other
   than the implementers, high reasoning.

## Done means

Implemented and reviewed in its worktree, `verify-production` and smoke green,
evidence and `VAL-CEL2-*` rows queued. **No landing and no push** until the
author says so in the chat of this session.

## Procedure

See the Procedure of [the evidence](2026-10-17-phone-and-bubbles.md); this record runs nothing.

## Result

None: a plan record. The results are in the evidence.

## Limits

It records intent only; the evidence lists what was and was not proven.
