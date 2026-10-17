# CEL-2-G Clipboard history — implementation plan

- **Date:** 2026-10-17
- **Artifact:** none; a plan record, not a build
- **Scope:** unit CEL-2-G, as planned before implementation
- **Environment:** none; nothing in this record was run

- **Design:** [privacy design](2026-10-17-clipboard-privacy-design.md)
  (approved 2026-10-17); [program](../../../docs/superpowers/specs/2026-10-16-celestina-parity-program-design.md) §5 row G
- **Ledger:** `celestina/docs/plans/active/2026-10-16-cel-2-parity.md`, row CEL-2-G
- **Models:** Fable is unavailable. Opus 5.5 for the privacy-bearing tasks,
  Sonnet 5.5 for ordinary implementation, Haiku 5.5 for mechanical edits.
- **Models of the model:** the shapes to copy are CEL-2-B (`energy/`: handle
  per duty, `wire.rs` pump, `Spawner` seam, live apply) and CEL-2-E
  (`duties.*` in `settings.rs`, smoke, evidence).

## Gate before the first commit

`main` must contain, in `AGENTS.md` "Halted projects", the CEL-2-G line of the
salvage list (`celestina-qt/src/provider_adapter/clipboard.rs` and
`celestina-rs/crates/celestina-shell-core/src/clipboard.rs`). The session
"Salvamento CEL-2 restante" writes it. Work may go on in the worktree; the
commit waits. Every copied file records source path and commit in its head
comment.

## Shared points (minimal, additive)

`app.rs`, `command.rs`, `message.rs`, `main.rs`, `settings.rs`, `smoke.sh`,
`Cargo.toml`/`Cargo.lock`, the ledger and `STATUS.md`. One field, one arm, one
line per duty. No new Cargo dependency is expected (`wayland-protocols` with
`staging` already has `ext_data_control`; verify the feature before adding).

## Tasks

### T1 — Pure rules (`celestina-shell/src/clipboard.rs`) — Opus, high

Salvage the core's bounds and history list; add: `Entry` (text or PNG bytes,
capture time), the exclusion-hint table (§2), `is_recordable` for text and
for PNG size (§3), 7-day expiry (`now` passed in, never read inside), image
cap 20, text cap 200, duplicate-to-front, `select`/`remove`/`clear`, and the
persisted form: `encode(entries) -> bytes` (text only, schema-versioned) and
`decode(bytes) -> entries` that re-applies every rule and drops what fails.
Also `duties.clipboard` (alphabetical line) with tests in `settings.rs`.
Tests: every row of the design's bounds, hostile files, the hint table (one
test per entry), expiry at the boundary, no content in any `Debug`/error
output (a `Debug` that prints lengths only).

### T2 — Persistence (`celestina/src/clipboard/store.rs`) — Opus, high

Read bounded; refuse symlink, wrong owner or wrong mode; write temp file with
mode 0600 then rename; delete on request; the directory is created 0700.
Tests with a temporary `XDG_STATE_HOME`: round-trip, hostile file replaced on
next save, deletion, mode check, failure leaves the old file.

### T3 — Protocol worker (`celestina/src/clipboard/worker.rs`) — Opus, high

Its own connection through `energy/wire.rs` (move or re-export so the energy
and the clipboard share one pump; do not copy it). Binds `ext_data_control_manager_v1`
and the seat's device. On `selection`: skip without reading when the hint is
offered; otherwise read one text MIME or `image/png` with byte and time
bounds, chunked, on the worker (never the loop). Setting the selection from
an entry with source identity to ignore its own echo. Bounded command queue
(`select`, `remove`, `clear`, `pause`, `resume`, `stop`), deterministic
shutdown, results published to the loop as `Message::Clipboard(..)` carrying
previews only. Tests against a fake data-control compositor (selection with
and without hint, silent owner, oversized owner, echo of own source, flood).

### T4 — Wiring (`celestina/src/clipboard/mod.rs`, `app.rs`, `main.rs`, `message.rs`, `command.rs`) — Sonnet, medium

`ClipboardHandle` with a `Spawner` seam like `EnergyHandle`; `apply(&Settings)`
starts or stops the worker on `duties.clipboard` changes, live; turning off
empties memory and deletes the file. Verbs `clipboard select N|remove N|clear|pause|resume`
answering success/failure only. Command parser tests.

### T5 — Piece and page (`celestina/src/pieces/clipboard.rs`, `island/pages/clipboard.rs`, `copy.rs`) — Sonnet, medium

The piece in the end zone's default order, the Island page (previews,
thumbnails with the bounded decoder, select, delete, confirmed clear, pause
toggle), Spanish copy, accessible names bounded to the preview, keyboard
model. Characterisation of the existing pages first, unchanged after.

### T6 — Smoke, evidence and docs — Sonnet, low (Haiku for pure doc edits)

`smoke.sh` section with a fake client (no real clipboard, ever): history
grows, hinted adds nothing, `off` deletes the file, mode 0600 asserted. The
evidence record, `STATUS.md`, `README.md`, `VALIDATION.md` rows
`VAL-CEL2-CLIPBOARD-LIVE`, `-SECRET`, `-IMAGE`, the ledger row (`planned` ->
`active` -> `done` at landing), the inventory.

### T7 — Final review — a model different from the implementer, high

Opus implemented T1-T3: review them with Sonnet 5.5 at high. Sonnet
implemented T4-T6: review them with Opus 5.5 at high. Both reviews include a
privacy checklist: no content in logs, no disk path other than the one
decided, no verb that returns content, bounds enforced on every input path.
Then `cargo fmt --check`, `clippy -D warnings`, `scripts/verify-production.sh`
and the nested smoke. No landing and no push without the author's word.


## Procedure

None: this is a design and plan record, not a build. Its rules are tested by the
unit's tests and smoke, recorded in [the evidence](2026-10-17-clipboard.md).

## Result

Approved by the author on 2026-10-17 and executed; where the build differs,
[the evidence](2026-10-17-clipboard.md) is authoritative.

## Limits

Nothing here was executed. It records intent, not outcome.
