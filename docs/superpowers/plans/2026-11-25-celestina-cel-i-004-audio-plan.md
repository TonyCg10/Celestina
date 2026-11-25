# CEL-I-004: device selector and per-application volume, implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The bar's volume popup lets the author pick the default output and input device and set each playing application's volume and mute.

**Architecture:**
- Cuprita's pure `wpctl status` parser leaves the `wpctl` feature of `cuprita-core` (unit `CUP-WPCTL-STATUS`).
- The shell's audio worker publishes a bounded `AudioGraph` read with that parser and takes four new commands.
- The level popup draws three new sections over the graph it had when it opened (unit `CEL-I-004`).

**Tech Stack:** Rust; the celestina workspace (pinned toolchain plus stable 1.100); `celestina-rs` workspace for `cuprita-core`; `wpctl`/`pactl` as processes; `celestina-ui` views.

**Spec:** `docs/superpowers/specs/2026-11-25-celestina-cel-i-004-audio-design.md`. Parent: the standing-tracks design §6.7.

## Global Constraints

- English in the repository; product copy Spanish in `celestina/src/copy.rs`; the author's conversation is Spanish.
- **One unit per worktree,** opened with `scripts/worktree.sh` from the canonical checkout; never edit the canonical checkout.
  - Commit subjects are `<prefix>-<kind>: <English imperative>`.
  - Landings need the author's direct authorisation in the session's own chat.
- **No `wpctl` call on the event loop's thread.** Every process sits behind the provider's `Environment` trait, and tests use fakes; no test runs `wpctl` or `pactl`.
- **Never write to the author's real PipeWire.** Nested runs put fake `wpctl`/`pactl` first on `PATH` for any write, and reading the real graph is allowed.
- **Never run `celestina` or `celestina-lock` against the live session.**
  - Nests only: `scripts/dev-session.sh --own-bus` or `smoke.sh`, clean environment, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`.
  - Bound a smoke with `timeout -k 60 2700`, never `timeout -s KILL`.
- **Shell checks run from `celestina/`:**
  - `cargo fmt --all -- --check`;
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`;
  - the same clippy on `+1.100`;
  - `cargo test --workspace --locked`.
- **Repository guards:**
  - `bash scripts/check-architecture-contract.sh`;
  - `scripts/check-documentation-contract.sh`;
  - `python3 scripts/check-language-contract.py`.
- **Bounds (exact):**

  | Constant | Value |
  |---|---|
  | `MAX_GRAPH_DEVICES` | 8 per direction |
  | `MAX_GRAPH_STREAMS` | 8 |
  | graph settle | 250 ms |
  | output read per call | 64 KiB (`wpctl status` is longer than one level) |
  | timeout per call | `WPCTL_TIMEOUT` (2 s), as today |

- **Review rounds:** at most 2 (ordinary, off the sensitive paths). The smoke scripts are sensitive; this plan does not change them.

## Review Focus

1. **The default device changes while the popup is open** (from the popup or elsewhere). The mark moves, the default sliders follow the new device, and no OSD appears.
2. **A stream ends while its slider is held.** The row greys out, the throttle's last value is dropped (the id is gone) and nothing panics.
3. **`wpctl status` prints something unexpected** (empty, a future format, 10 MB). The result is an empty or bounded graph and a journal line, never a hang. The level path keeps working.
4. **Twelve sinks** (HDMI ports multiply them). Only 8 rows show and the popup fits on a 1080 px output.
5. **The `pactl` child dies.** The graph is re-read by the existing poll fallback, at the poll cadence, without a second poll loop.

---

## Unit A — `CUP-WPCTL-STATUS` (project `cuprita`, `maintenance`)

**Worktree:** `scripts/worktree.sh open cuprita CUP-WPCTL-STATUS`. There is no active Cuprita plan, so the unit hosts its row in a one-row plan opened and closed in its own commit, as the suite unit ARC-1 did (`docs/plans/archive/2026-11-25-plan-archives.md`):

- `cuprita/docs/plans/archive/2026-11-25-wpctl-status.md`, with Plan ID `wpctl-status`, checkpoint `CUP-2`, row `CUP-WPCTL-STATUS`;
- `cuprita/ROADMAP.md` and `cuprita/STATUS.md` mention it the way ARC-1's rows did;
- the evidence goes in `cuprita/docs/evidence/2026-11-25-wpctl-status.md`.

### Task A1: Ungate the pure status parser

**Files:**
- Create: `celestina-rs/crates/cuprita-core/src/wpctl_status.rs`
- Modify: `celestina-rs/crates/cuprita-core/src/wpctl.rs`, `celestina-rs/crates/cuprita-core/src/lib.rs`

**Interfaces:**
- **Produces, available without any feature:**
  - `cuprita_core::wpctl_status::{Card, Status, parse_status, parse_volume}`, with unchanged signatures:
    - `pub fn parse_status(text: &str) -> Status`;
    - `pub fn parse_volume(text: &str) -> Option<(f32, bool)>`.
  - The private helpers (`Section`, `untree`, `numbered`, `volume_suffix`, `strip_api_tag`) move with them.
- **Keeps compatibility:** `wpctl.rs` re-exports the same names (`pub use crate::wpctl_status::{Card, Status, parse_status, parse_volume};`), so Cuprita's callers and `examples/snapshot.rs` compile unchanged.

- [ ] Move the items and their unit tests (the `parse_status`/`parse_volume`/`strip_api_tag` tests of `wpctl.rs`'s test module) into `wpctl_status.rs`. The module uses only `std` and `crate::model`/`crate::volume`.
- [ ] Declare `pub mod wpctl_status;` in `lib.rs` without a `cfg`.
- [ ] Run these from `celestina-rs/`. All must pass:
  - `cargo test -p cuprita-core`;
  - `cargo test -p cuprita-core --no-default-features --features nm,bluez`;
  - `cargo clippy -p cuprita-core --all-targets -- -D warnings`;
  - the same clippy with `--no-default-features --features nm,bluez`;
  - `cargo fmt --all -- --check`;
  - the three repository guards.
- [ ] Run `cargo check` in `cuprita/`, which consumes `cuprita-core` with default features. Expected: compiles. If the Qt toolchain is missing, record it as "not run".
- [ ] Write the plan, the evidence and the roadmap/status lines, then commit: `cuprita-maintenance: Move the pure wpctl status parser out of the wpctl feature of cuprita-core`.

---

## Unit B — `CEL-I-004` (project `celestina`, `milestone`)

**Worktree:** `scripts/worktree.sh open celestina CEL-I-004 --from unit/suite/CEL-TRACKS-OPEN`, then `git merge unit/cuprita/CUP-WPCTL-STATUS` (landing contract, "Stacked branches"). This unit opens the shell's standing plan.
- **Its first commit adds:**
  - `celestina/docs/plans/active/2026-11-25-cel-tracks.md`, from the draft `/home/toni/CODIGO/2026-11-25-celestina-cel-tracks-plan-draft.md`, with the `CEL-I-004` row set to `active`, Files `pending` and Diffstat `pending`;
  - `celestina/ROADMAP.md` set to status active with checkpoint `CEL-TRACKS`, plus a short section;
  - a line in `celestina/STATUS.md`;
  - `celestina/docs/plans/active/README.md` naming the plan.
- **Its own row is the only open one:** a landing changes exactly one active plan.
- **Its last task** adds `VAL-CEL-I-004` to `celestina/VALIDATION.md`.

### Task B1: The graph and the stream event, in the pure domain

**Files:**
- Modify: `celestina-rs/crates/celestina-shell/src/audio.rs` (tests in its test module).

**Interfaces:**
- **Produces:**
  - `AudioEvent::Stream`: `parse_pactl_event` maps the facility `sink-input` to it, with the same action and index rules. `source-output` stays no event, since recording streams are not shown.
  - `pub const MAX_GRAPH_DEVICES: usize = 8;` and `pub const MAX_GRAPH_STREAMS: usize = 8;`
  - `pub struct GraphDevice { pub id: u32, pub name: String, pub default: bool }`
  - `pub struct GraphStream { pub id: u32, pub name: String, pub level: Option<AudioLevel> }`
  - `#[derive(Clone, Debug, Default, PartialEq, Eq)] pub struct AudioGraph { pub outputs: Vec<GraphDevice>, pub inputs: Vec<GraphDevice>, pub streams: Vec<GraphStream> }`, with:
    - `pub fn bounded(self) -> Self`: truncates each list to its cap, keeping the default device even when it is past the cap, as the last row;
    - `pub fn has_stream(&self, id: u32) -> bool`;
    - `pub fn has_device(&self, id: u32) -> bool`.

- [ ] **Write the failing tests first:**
  - `a_sink_input_line_is_a_stream_event` (`"Event 'new' on sink-input #88"` → `Some(Stream)`; `"Event 'change' on source-output #3"` → `None`);
  - `bounded_keeps_eight_and_the_default`: 12 outputs, default at index 10, give 8 rows with the default last;
  - `has_stream_and_has_device_name_listed_ids_only`.
- [ ] Implement, then run `cargo test -p celestina-shell audio` from `celestina/`. It must pass.
- [ ] Commit: `celestina-milestone: Add the audio graph and the stream event to the shell domain`. Branch commits are free-form; the landing writes the unit's subject.

### Task B2: The worker reads the graph and takes the new commands

**Files:**
- Modify: `celestina/src/providers/audio.rs` and its tests.
- Modify: `celestina/src/message.rs` (new variant).
- Modify: the consumer gate where the worker learns about the volume piece. Today it is told nothing: add it beside the DDC gate in `src/osd/mod.rs` (`OsdRuntime::apply`).

**Interfaces:**
- **Consumes:** `cuprita_core::wpctl_status::{parse_status, Status}` from Unit A; the domain types from B1.
- **Produces:**
  - `Message::AudioGraph(AudioGraph)`.
  - `AudioCommand` gains:
    - `Graph(bool)`: the consumer gate. `true` reads now and keeps the graph current; `false` stops reading it.
    - `SetDefault { id: u32 }`
    - `SetStreamVolume { id: u32, percent: u8 }`
    - `ToggleStreamMute { id: u32 }`

    `AudioCommand` stays `Copy`.
  - `Environment` gains:
    - `fn status(&mut self) -> Option<String>`: runs `wpctl status`, output capped at 64 KiB;
    - `fn read_node(&mut self, id: u32) -> Option<AudioLevel>`: runs `wpctl get-volume <id>`;
    - `fn set_default(&mut self, id: u32) -> io::Result<()>`: runs `wpctl set-default <id>`;
    - `fn set_node_volume(&mut self, id: u32, percent: u8) -> io::Result<()>`: runs `wpctl set-volume <id> <fraction> -l 1.0`, formatted like `set_volume_args`;
    - `fn toggle_node_mute(&mut self, id: u32) -> io::Result<()>`: runs `wpctl set-mute <id> toggle`.

    The `pub fn …_args` helpers are tested for exact arguments, as `the_arguments_are_exact_fractions_with_the_limit` does.
  - `Timing` gains `graph_settle: Duration`, 250 ms by default.

- **Graph rules:**
  - **Conversion** (pure `fn graph_of(status: &Status, read: impl FnMut(u32) -> Option<AudioLevel>) -> AudioGraph` in the provider):
    - outputs are `EndpointKind::Sink`, inputs are `Source`, with `name` = `description` and `default` from the parser;
    - streams are the parser's streams whose `endpoint` is the id of a listed sink;
    - each stream gets `level` = `read(id)`;
    - the result is `.bounded()`.
  - **Reading:**
    - while the gate is on, a `Sink`, `Source`, `Server` or `Stream` event marks the graph pending, and it is read after `graph_settle`;
    - `Stream` events never mark the default levels;
    - a write command marks the graph pending too;
    - the graph is published only when it differs from the last one published;
    - with the gate off, nothing is read and no graph message is sent.
  - **Write refusals:** a write whose id is not in the last published graph (`has_device`/`has_stream`) is refused with a `tracing::debug!` line, and no process runs.
  - **Poll fallback:** when the `pactl` child is dead, the existing poll also re-reads the graph if the gate is on.

- [ ] **Write the failing tests first,** with the existing `FakeEnv` extended by a scripted status text and per-node levels:
  - `the_graph_is_read_once_when_the_gate_opens_and_never_while_closed`;
  - `a_burst_of_stream_events_is_one_graph_read`;
  - `stream_events_never_read_the_default_levels`;
  - `a_write_to_an_unlisted_id_runs_nothing`;
  - `set_default_and_stream_writes_use_exact_arguments`;
  - `an_unreadable_status_is_an_empty_graph`;
  - `an_unchanged_graph_is_not_published_twice`;
  - `graph_of_keeps_playback_streams_only` (a fixture of the author's real `wpctl status` shape: six sinks, two sources, three streams, one with `[init]`).
- [ ] Implement; run `cargo test -p celestina providers::audio`. All must pass.
- [ ] Commit.

### Task B3: The popup's three sections

**Files:**
- Modify: `celestina/src/osd/popup.rs`, `celestina/src/osd/control.rs`, `celestina/src/osd/readings.rs` (holds the latest graph), `celestina/src/app.rs` (route `Message::AudioGraph` to `Controls`), `celestina/src/copy.rs`.

**Interfaces:**
- **Consumes:** `Message::AudioGraph`, the B2 commands, and B1's `AudioGraph`.
- **Produces:**
  - `Control` gains `Stream(u32)`, so `slider_key`/`mute_key` give `Key::indexed("level-stream", id)`/`Key::indexed("level-stream-mute", id)`.
  - `Request` gains:
    - `SetDefault(u32)`
    - `Set(Control::Stream(id), u8)` and `ToggleMute(Control::Stream(id))`, which reuse the existing variants.

    `commands()` maps them to the B2 commands.
  - `pub fn popup_size(theme: &Theme, kind: Kind, microphone: bool, shape: Shape) -> (f32, f32)` with `#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Shape { pub outputs: usize, pub inputs: usize, pub streams: usize }`. For `Kind::Brightness` the shape is ignored.
  - `card(theme, kind, microphone, levels, graph: &AudioGraph, shape: Shape)` draws the sections in the design's §3 order:
    - device rows are clickable and keyed `Key::indexed("level-device", id)`;
    - the default row is drawn selected;
    - a stream that is no longer in `graph` draws disabled and ignores input.
  - **Copy** in `copy.rs`, Spanish:
    - `AUDIO_OUTPUT_DEVICE = "Salida"`;
    - `AUDIO_INPUT_DEVICE = "Entrada"`;
    - `AUDIO_APPS = "Aplicaciones"`;
    - `AUDIO_NO_APPS = "Ninguna aplicación suena"`.
  - **Size and gate:**
    - `Controls` opens the volume popup with `Shape` from the graph it holds at that moment;
    - the shared cell carries the graph next to `Levels`, and `Levels` stays `Copy`, so the graph goes in its own `Rc<RefCell<AudioGraph>>`;
    - the gate `AudioCommand::Graph(true)` is sent when the duty is on and a layout holds `volume`, and `Graph(false)` otherwise. This follows `brightness_consumer`.
  - **Throttle:** one entry per `Control`, `Stream(id)` included. A pending stream value whose id vanished is dropped, not sent.

- [ ] **Write the failing tests first:**
  - `the_volume_card_lists_devices_and_streams_in_order`;
  - `the_default_device_row_is_selected`;
  - `a_click_on_a_device_row_requests_set_default`;
  - `a_stream_slider_requests_its_own_level`;
  - `a_vanished_stream_row_is_disabled_and_its_pending_value_dropped`;
  - `the_size_is_fixed_at_open_and_fits_eight_rows_each_within_1080`;
  - `no_graph_message_reaches_the_osd`;
  - `the_graph_gate_follows_the_volume_piece`.
- [ ] Implement; run `cargo test -p celestina osd`. All must pass.
- [ ] Run the full shell checks and the guards. All must pass.
- [ ] Commit.

### Task B4: Nested look, evidence, validation row

**Files:**
- Modify: the `CEL-I-004` ledger row.
- Create: `celestina/docs/evidence/2026-11-25-cel-i-004-audio.md`.
- Modify: `celestina/VALIDATION.md` (new row `VAL-CEL-I-004`).

- [ ] Run the OSD smoke phase: `timeout -k 60 2700 sh scripts/smoke.sh --binary PATH --only osd` (from `celestina/`, debug binary). It must print `smoke: OK`. The fake `wpctl` does not answer `status`, which is the "unreadable status is an empty graph" path.
- [ ] Take a nested screenshot of the popup with a graph. Use a scratch fake `wpctl` (in the session scratchpad, not in the repository) that prints a fixed `status` shaped like the author's (6 sinks, 2 sources, 3 streams) and records writes. Put it first on `PATH` for the nest, open the volume popup, and take the screenshot through the nest's IPC.
- [ ] Write the evidence (commands, exits, test counts, screenshot path, what was not run). Add the validation row:
  - switch the output from the SNOWSKY to another sink and back, then the input;
  - lower Firefox or a game alone;
  - mute one application;
  - check that no OSD flashes while doing it.
- [ ] Commit, then report to the author for landing authorisation.

## Landing order

1. `CEL-TRACKS-OPEN` (suite).
2. `CUP-WPCTL-STATUS` (cuprita).
3. `CEL-I-004` (celestina, `--kind milestone`, next MINOR).

Each landing runs `python3 scripts/land-unit.py <branch> --kind <kind>` from the canonical checkout, after the author's direct authorisation. After `CEL-I-004` lands, the author restarts the shell and runs `VAL-CEL-I-004`.
