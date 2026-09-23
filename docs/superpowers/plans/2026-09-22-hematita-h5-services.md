<!-- language-contract: product-copy — the qsTr() samples below are product copy quoted inside code blocks -->
# Hematita H5 — Services, and privilege without a daemon

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give Hematita its Services page — every systemd unit of the user's session and of the system, with start, stop and restart — and let a person act on processes that are not theirs, both through polkit and without Hematita ever holding privilege; and write the decision that bounds how.

**Architecture:** A suite decision (ADR 0010) fixes the rule: privilege is one-shot and polkit-mediated — user units over the session bus need no authorisation; system units over the system bus are authorised by polkit through whatever authentication agent the session runs; foreign processes are signalled through `pkexec /usr/bin/kill`; no daemon, no setuid helper, no custom polkit action; and when the session has no agent, every privileged action fails with the typed reason `no-agent` that the page turns into a Spanish sentence. `hematita-core` gains a `services` module with the pure parts (unit kinds, actionability, filtering, the mapping of D-Bus error names and `pkexec` exit codes to outcomes). The sampler thread lists units on both buses every `SERVICE_TICKS` through `zbus` blocking proxies and ships a `ServiceSnapshot`; each action runs on a short-lived worker thread and queues its outcome back. `HematitaServices` publishes index-aligned lists with a `revision`; `HematitaProcesses` gains the foreign-process path. A shared `ConfirmDialog` replaces `KillDialog` and guards stop and restart too. H4's three deferrals ride the same build.

**Tech Stack:** Rust 1.97.1, cxx-qt 0.9.1, `zbus 5` (already a dependency), `rustix` (already), Qt 6.9+ QML, `celestina-style` symlinks. No new dependencies.

**Spec:** [docs/superpowers/specs/2026-09-21-hematita-design.md](../specs/2026-09-21-hematita-design.md) §1 item 5, §5 (`activation.rs` shape for zbus), §6 `ServicesPage`, §7 H5 row, §9 "no daemon". Carry-overs: the H2–H4 plans' list shape, the H1 plan's inventory generator and commit procedure.

## Global Constraints

- **The Celestina shell is in standby.** Never read, reuse, modify or reference `celestina/` or `celestina-rs/crates/celestina-shell-core`. In particular Hematita does not ship, register or depend on a polkit authentication agent: that is the shell's, and its absence is a state the page names.
- **Language contract:** identifiers, comments, docs, tests, script messages, commit subjects in English; product copy only as `qsTr()` literals in QML; no Rust file carries a user-visible string. Unit names and descriptions from systemd are data shown raw.
- **Build budget (author's rule):** two application build cycles in H5 — one at the end of H5-B (release build + release-profile tests/clippy + `verify-production.sh`), one inside `complete-production.sh` at H5-Z. H5-A is crate-and-documents only. Never `cargo` under `hematita/` outside those moments; never `cargo clean`; never a window on the live or nested session (`VAL-H5`).
- **Commits:** per unit after review; `hematita:` prefix for the units; the ADR and its index row are a separate `suite-maintenance:` commit (root `docs/decisions/` is outside Hematita's roots), landed before H5-A's commit; inventories through the H1 plan's generator (scratchpad `mkinv.py`); evidence under `hematita/docs/evidence/`, inventories under `hematita/docs/inventories/2026-09-22-h5-services/`; hooks never bypassed; explicit pathspecs.
- **Privilege invariants (from ADR 0010):** Hematita never runs as root, never keeps an authorised connection, never caches a credential; every privileged action is one call or one `pkexec` process; `pkexec`'s target is always `/usr/bin/kill` with a fixed argument shape (`-TERM`/`-KILL` and one numeric PID), never a shell; the outcome of every action is typed and shown, including `no-agent`.
- **Rust invariants:** no `unsafe`; no production `unwrap`/`expect`/`panic!`; typed errors; blocking IO and D-Bus calls never on the Qt thread (listing on the sampler thread, actions on named worker threads that queue their result back); stale generations dropped.
- **QML invariants:** every new file in `build.rs` `QML_FILES`; tokens only; `required property`; no tooltips; keyboard/AT reachability with the list as the single Tab stop; `reducedMotion` honoured; no lint suppressions; the `hematita` qmllint row is `0` and may not rise; dialogs contain and restore focus.
- **Constants once:** `SERVICE_TICKS = 5` in `sampler.rs` (units are listed every fifth tick); `PKEXEC = "/usr/bin/pkexec"`, `KILL = "/usr/bin/kill"` in `hematita/src/privilege.rs`.
- **Service contracts (index-aligned lists on `HematitaServices`):** `unitNames`, `unitDescriptions`, `unitScopes` (`system|user`), `unitActives` (`active|inactive|failed|activating|deactivating|reloading`), `unitSubs` (raw sub state, data), `unitKinds` (`service|socket|timer|target|mount|device|scope|slice|path|other`), `unitActionable` (`0/1`: services and sockets only). State: `filterText`, `showSystem`, `showUser`, `servicesOnly` (default true), `revision`, `systemAvailable`/`systemReasonKind`/`systemReasonPath`, `userAvailable`/`userReasonKind`/`userReasonPath`, `totalCount`, `shownCount`, `actionOutcome` (`""|done|no-agent|denied|failed|refused`), `actionUnit`, `actionKind` (`start|stop|restart`), `startFailed`.
- **Outcome contract (shared):** `done`, `no-agent` (no authentication agent in the session), `denied` (the person cancelled or was not authorised), `failed` (the call or the process errored), `refused` (Hematita refused before trying: not actionable). `HematitaProcesses.actionOutcome` gains the same vocabulary for the foreign path.

---

## File structure

| Path | Responsibility |
|---|---|
| `docs/decisions/0010-one-shot-privilege-through-polkit.md`, `docs/decisions/README.md` | the decision and its index row |
| `celestina-rs/crates/hematita-core/src/services.rs` | unit kinds, actionability, filter/sort projection, outcome mapping |
| `hematita/src/privilege.rs` | `pkexec` invocation on a worker, exit-code mapping; the only place `pkexec` is spelled |
| `hematita/src/sampler.rs` | `ServiceSnapshot` listed every `SERVICE_TICKS` from both buses |
| `hematita/src/services.rs` | `HematitaServices` hub: lists, state, `start`/`stop`/`restart` on worker threads |
| `hematita/src/processes.rs` | foreign-process path: `terminateForeign`/`killForeign` through `privilege.rs`; `actionOutcome` vocabulary |
| `hematita/qml/components/ConfirmDialog.qml` | the shared confirming dialog (replaces `KillDialog.qml`) |
| `hematita/qml/components/ServicesPage.qml`, `ServiceRow.qml` | the page and its row |
| `hematita/qml/components/ProcessTable.qml` | foreign actions enabled with their sentence; `ConfirmDialog` |
| `hematita/qml/Main.qml` | fifth section `Servicios`; `HematitaServices` hub |
| `hematita/src/sampler.rs`, `sensors.rs`, `publish.rs` | H4 deferrals: statics re-enumerated every 30 ticks; power graded against its cap; extremes kept while the chip is listed |
| `hematita/{ROADMAP,STATUS,VALIDATION,AGENTS}.md`, `hematita/docs/plans/active/2026-09-22-h5-services.md` | H5 documents |

Ledger units:

| Unit | Kind | Content | Build |
|---|---|---|---|
| (decision) | `suite-maintenance` | ADR 0010 and its index row | none |
| H5-A | `hematita-maintenance` | crate `services`; H5 opened in the documents | none |
| H5-B | `hematita-maintenance` | `privilege.rs`, sampler service section, `HematitaServices`, foreign-process path, `ConfirmDialog`, `ServicesPage`, `ServiceRow`, `Main.qml`; H4 deferrals | one |
| H5-Z | `hematita-milestone` | 0.6.0, `complete-production.sh`, documents closed, plan archived | one |

---

### Task 1: ADR 0010 and the H5 documents

**Files:**
- Create: `docs/decisions/0010-one-shot-privilege-through-polkit.md`; modify `docs/decisions/README.md`
- Create: `hematita/docs/plans/active/2026-09-22-h5-services.md`; modify `hematita/ROADMAP.md`, `STATUS.md`, `VALIDATION.md`, `docs/plans/active/README.md`

- [ ] **Step 1: Write the decision**

```markdown
# ADR 0010: Hematita acts with privilege one call at a time, through polkit, and never holds it

- **Date:** 2026-09-22
- **Status:** accepted

## Context

Hematita's fifth phase adds systemd services with start, stop and restart, and
lets a person end a process that is not theirs. Both need privilege the
monitor does not have, and the suite's rule is that nothing needs standing
privilege: no daemon, no setuid helper.

Three facts decide the shape.

systemd already authorises through polkit. `StartUnit`, `StopUnit` and
`RestartUnit` on the system bus are guarded by
`org.freedesktop.systemd1.manage-units`; polkit asks the session's
authentication agent, the person answers, and the call proceeds or is
refused. The user's own manager on the session bus needs no authorisation at
all. Hematita therefore needs no privilege of its own to offer services: it
asks systemd, and systemd asks the person.

Ending a foreign process has no such broker. The kernel refuses a signal to
another user's process, and the only sanctioned way to send one is to run
`kill` as root through `pkexec`, which again asks the session's agent under
`org.freedesktop.policykit.exec`. A polkit action of Hematita's own would only
change the wording of the prompt and would add an installed policy file to
the artifact; it is not needed for correctness.

The author's session runs `polkitd` and, today, no authentication agent: the
shell that will provide one is in standby by the author's order. Without an
agent every privileged action fails at the prompt. That is a state Hematita
must name, not hide, and not work around.

## Decision

- Privilege is one-shot and polkit-mediated. Hematita never runs as root,
  never keeps an authorised connection, never caches a credential, ships no
  daemon, no setuid binary and no polkit action file.
- User units are managed over the session bus without authorisation. System
  units are listed over the system bus without authorisation and managed
  through `StartUnit`/`StopUnit`/`RestartUnit` with mode `replace`, letting
  polkit authorise each call.
- A foreign process is signalled by spawning `/usr/bin/pkexec /usr/bin/kill
  -TERM <pid>` or `-KILL <pid>` — a fixed argument shape, never a shell —
  from a worker thread, after the same re-validation of uid and start time
  that guards the user's own processes.
- Every privileged action reports a typed outcome the page turns into words:
  `done`, `no-agent` (no authentication agent answered), `denied` (the
  person cancelled or was not authorised), `failed`, `refused` (Hematita did
  not try). A missing agent is not an error of Hematita's and is not
  retried.
- Nothing in this decision touches the Celestina shell; when the shell
  provides an agent, Hematita's prompts start working without a change.

## Consequences

- The Services page works fully for user units today and degrades honestly
  for system units and foreign processes until the session has an agent.
- The artifact gains no installed file beyond the binary and its desktop
  entry; the security surface is polkit's and systemd's, not Hematita's.
- `pkexec` sets the target's environment; only `kill` runs, so nothing of
  Hematita's inherits root.

## Revisit when

- The session gains an agent and the prompts' wording proves inadequate: a
  Hematita polkit action with its own message would then be justified.
- systemd exposes pidfd-based signalling through the bus, or the kernel adds a
  sanctioned kill-by-pidfd for foreign processes, which would remove
  `pkexec`.
```

Index row in `docs/decisions/README.md`: `| [0010](0010-one-shot-privilege-through-polkit.md) | accepted | Hematita acts with privilege one call at a time, through polkit, and never holds it |`.

- [ ] **Step 2: The H5 documents** — same shape as H4's Task 1: plan `2026-09-22-h5-services.md` (Plan ID `h5-services`, checkpoint `H5`, `VAL-H5`; hypothesis "Every systemd unit of the session and the system can be listed on the sampler thread from both buses, the user's own units started and stopped without authorisation, and every privileged action reported truthfully, including the absence of an agent"; scope the three units; exclusions "a polkit agent; unit files not loaded (`ListUnitFiles`); journal; timers' schedules; editing units"); roadmap `active`/`H5` with rows H5-A (dep H4-D), H5-B, H5-Z and `## H5 — opened 2026-09-22` replacing the "next checkpoint" sentence; STATUS; README; VALIDATION:

```markdown
## VAL-H5 — Services and foreign processes on the real session

- **Status:** pending
- **Related implementation:** H5
- **Requires:** the deployed Hematita 0.6.0; a session with or without an
  authentication agent (state which)
- **Procedure:** open Servicios; compare the count of user services with
  `systemctl --user list-units --type=service`; stop and start a harmless
  user service (`at-spi-dbus-bus.service` restarts on demand) and watch its
  state change; filter by name; toggle Sistema and Usuario; try to restart a
  system service and read the outcome sentence (with no agent it must name
  the missing agent, not fail silently); on Procesos, select a root process
  and press Terminar, then read the sentence; walk the list by keyboard and
  screen reader; confirm Escape cancels the confirming dialog
- **Pass condition:** counts match; the user service's state follows the
  action within a second; the filter and toggles narrow live; a system
  action without an agent says so in Spanish; with an agent, the prompt
  appears and the action follows the answer; the same for the root process;
  every row and control reachable and named; idle CPU under 2 % on Servicios
- **Result:** not run
- **Evidence:** none
```

Guard: `bash scripts/check-documentation-contract.sh` → OK. The ADR and index land as `suite-maintenance: Add the decision on one-shot privilege through polkit for Hematita` first; the Hematita documents land inside H5-A.

---

### Task 2: `hematita-core::services`

**Files:**
- Create: `celestina-rs/crates/hematita-core/src/services.rs`; modify `lib.rs`

**Interfaces:**
- `pub enum Scope { System, User }` with `as_str()`.
- `pub enum UnitKind { Service, Socket, Timer, Target, Mount, Device, Scope, Slice, Path, Other }` with `of_name(&str) -> Self` (by suffix) and `as_str()`.
- `pub struct Unit { pub name: String, pub description: String, pub scope: Scope, pub active: String, pub sub: String }`.
- `pub fn is_actionable(kind: UnitKind) -> bool` (Service, Socket).
- `pub fn project(units: &[Unit], filter: &str, show_system: bool, show_user: bool, services_only: bool) -> Vec<usize>` — filter by scope toggles, kind, case-insensitive substring on name or description; sorted by scope (user first), then active state (`failed` < `active` < others), then name.
- `pub enum Outcome { Done, NoAgent, Denied, Failed, Refused }` with `as_str()`.
- `pub fn outcome_of_dbus_error(name: &str) -> Outcome` — `org.freedesktop.DBus.Error.InteractiveAuthorizationRequired` and `org.freedesktop.PolicyKit1.Error.NotAuthorized` when no agent → `NoAgent`; `org.freedesktop.DBus.Error.AccessDenied` → `Denied`; `org.freedesktop.systemd1.NoSuchUnit` and anything else → `Failed`.
- `pub fn outcome_of_pkexec(status: Option<i32>) -> Outcome` — `Some(0)` → Done; `Some(126)` → Denied (dismissed); `Some(127)` → NoAgent (not authorised / no agent); `Some(1)` and others → Failed; `None` (signal) → Failed.

- [ ] **Step 1: Write the file test-first** (tests for `UnitKind::of_name` on `foo.service`, `bar.socket`, `x.timer`, `a.mount`, `dev-sda.device`, `app-x.scope`, `user.slice`, `y.path`, `z.target`, `weird`; `is_actionable`; `project` with a fixture of six units across both scopes and states, checking order, toggles, `services_only`, filter on description; both outcome mappers for every listed input).

The implementation is plain matching and a `sort_by` with a three-key tuple; write it in the same step (as `process_view` was) and say so in the evidence. Use the H3 `process_view` file as the shape.

- [ ] **Step 2: Run tests, fmt, clippy** — pass, clean. Close H5-A: evidence `2026-09-22-h5-core.md`, ledger, roadmap, STATUS, inventory `H5-A.numstat.tsv` (crate files + the H5 documents from Task 1 except the ADR), guards, commit `hematita-maintenance: Add the service unit projection and the outcome mapping`.

---

### Task 3: `privilege.rs`, the sampler's service section, `HematitaServices`

**Files:**
- Create: `hematita/src/privilege.rs`, `hematita/src/services.rs`; modify `sampler.rs`, `main.rs`, `build.rs`

- [ ] **Step 1: `privilege.rs`**

```rust
//! The one place Hematita runs something as root: `pkexec` around
//! `/usr/bin/kill`, on a worker thread, with a fixed argument shape. See
//! ADR 0010. Nothing here inherits privilege: pkexec runs `kill` and exits.

use std::process::Command;
use std::thread;

use hematita_core::services::{outcome_of_pkexec, Outcome};

const PKEXEC: &str = "/usr/bin/pkexec";
const KILL: &str = "/usr/bin/kill";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Terminate,
    Kill,
}

impl Signal {
    fn flag(self) -> &'static str {
        match self {
            Self::Terminate => "-TERM",
            Self::Kill => "-KILL",
        }
    }
}

/// Runs `pkexec kill -SIG PID` on a named thread and hands the outcome to
/// `report`, which is expected to queue it onto the Qt thread.
///
/// # Errors
///
/// The OS refused to create the thread.
pub fn signal_as_root(
    pid: u32,
    signal: Signal,
    report: impl FnOnce(Outcome) + Send + 'static,
) -> std::io::Result<()> {
    thread::Builder::new()
        .name("hematita-pkexec".to_owned())
        .spawn(move || {
            let status = Command::new(PKEXEC)
                .arg(KILL)
                .arg(signal.flag())
                .arg(pid.to_string())
                .status();
            report(match status {
                Ok(status) => outcome_of_pkexec(status.code()),
                Err(_) => Outcome::Failed,
            });
        })
        .map(|_| ())
}
```

`Command::status()` inherits stdio; `pkexec` prints its own message to stderr when no agent is present, which is fine on a terminal and invisible otherwise. The thread is detached by design: it lives as long as the prompt, and its only side effect is one queued closure that `qt.queue` drops harmlessly if the object is gone.

- [ ] **Step 2: The sampler's service section**

```rust
use hematita_core::services::{Scope, Unit};

/// Units change rarely; every fifth tick is live enough for a page and cheap
/// enough for two bus round-trips carrying a few hundred rows.
pub const SERVICE_TICKS: u64 = 5;

#[derive(Clone, Debug)]
pub struct ServiceSnapshot {
    pub system: Section<Vec<Unit>>,
    pub user: Section<Vec<Unit>>,
}

type UnitRow = (String, String, String, String, String, String, zbus::zvariant::OwnedObjectPath, u32, String, zbus::zvariant::OwnedObjectPath);

fn list_units(connection: &zbus::blocking::Connection, scope: Scope, bus_label: &str) -> Section<Vec<Unit>> {
    let proxy = match zbus::blocking::Proxy::new(
        connection,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    ) {
        Ok(proxy) => proxy,
        Err(_) => return Section::Unavailable(Reason { kind: ReasonKind::Unreadable, path: bus_label.to_owned() }),
    };
    match proxy.call::<_, _, Vec<UnitRow>>("ListUnits", &()) {
        Ok(rows) => Section::Available(
            rows.into_iter()
                .map(|(name, description, _load, active, sub, ..)| Unit { name, description, scope, active, sub })
                .collect(),
        ),
        Err(_) => Section::Unavailable(Reason { kind: ReasonKind::Malformed, path: bus_label.to_owned() }),
    }
}

fn sample_services(system: &mut Option<zbus::blocking::Connection>, user: &mut Option<zbus::blocking::Connection>) -> ServiceSnapshot {
    if system.is_none() {
        *system = zbus::blocking::Connection::system().ok();
    }
    if user.is_none() {
        *user = zbus::blocking::Connection::session().ok();
    }
    let unavailable = |label: &str| Section::Unavailable(Reason { kind: ReasonKind::Unreadable, path: label.to_owned() });
    ServiceSnapshot {
        system: system.as_ref().map_or_else(|| unavailable("system bus"), |c| list_units(c, Scope::System, "system bus")),
        user: user.as_ref().map_or_else(|| unavailable("session bus"), |c| list_units(c, Scope::User, "session bus")),
    }
}
```

`Snapshot.services: Option<ServiceSnapshot>` is `Some` on ticks where `generation % SERVICE_TICKS == 0` (and on generation 1, so the page fills at once); the two connections live in `run`'s locals; a connection that fails to open is retried on the next service tick. The smoke's session-bus address points nowhere, so the user section is `Unavailable` there and the page must render that state without error.

- [ ] **Step 3: `HematitaServices`**

Bridge properties as the contract lists; `Default` with `show_system: true`, `show_user: true`, `services_only: true`. `start()` subscribes as the other hubs. `apply(generation, snapshot)` stores the two sections, sets availability/reasons per bus, then `refresh()`. `refresh()` concatenates available units, calls `services::project`, publishes the lists (`unitKinds` from `UnitKind::of_name`, `unitActionable` from `is_actionable`), counts, `revision` last.

Actions: `#[qinvokable] fn start_unit(name: QString, scope: QString)`, `stop_unit`, `restart_unit` share `fn act(self, name, scope, method: &'static str, kind: &'static str)`: sets `actionUnit`/`actionKind`, clears `actionOutcome`, then spawns a named thread (`hematita-systemd`) that opens the right bus (`Connection::system()` / `session()`), builds the Manager proxy, calls `method` with `(name, "replace")`, maps `Ok` → `Outcome::Done` and `Err(zbus::Error::MethodError(name, ..))` → `services::outcome_of_dbus_error(name.as_str())`, other errors → `Failed`, and queues `set_action_outcome` back through `qt_thread()`. A unit whose kind is not actionable answers `refused` without a thread. A unit test covers the refusal and the `act`-side mapping helper (`fn outcome_of_call(result: Result<(), zbus::Error>) -> Outcome`, tested with a constructed `MethodError`).

`main.rs`: `mod privilege; mod services;`; `build.rs`: `.files([…, "src/services.rs"])`, rerun for `privilege.rs`.

---

### Task 4: The foreign-process path in `HematitaProcesses`

**Files:**
- Modify: `hematita/src/processes.rs`

- `terminate(pid)`/`kill(pid)` keep their own-process path. When `owned_pid` answers `None` because the uid is not ours (distinguish that from PID ≤ 1/self/unlisted: make `owned_pid` return `Result<Pid, Refusal>` with `Refusal::Foreign { start_ticks }` vs `Refusal::NotAllowed`), the method re-validates identity as today and then calls `privilege::signal_as_root(pid, signal, report)` where `report` queues `set_action_outcome(outcome.as_str())`; while the prompt is up, `actionOutcome` is `"pending"` (add it to the vocabulary: `pending` means "asked, waiting for the agent"). PID ≤ 1 and self stay `refused`. Tests: the refusal classification.
- The row's `actionable` stays `0` for foreign rows (the page uses it only to word the bar).

---

### Task 5: `ConfirmDialog`, `ServicesPage`, `ServiceRow`, `Main.qml`, the H4 deferrals, the one build, H5-B

**Files:**
- Create: `ConfirmDialog.qml`, `ServicesPage.qml`, `ServiceRow.qml`; delete `KillDialog.qml`; modify `ProcessTable.qml`, `Main.qml`, `build.rs`, `sampler.rs`, `sensors.rs`, `publish.rs`

- [ ] **Step 1: `ConfirmDialog.qml`** — `KillDialog` generalised: `required property Item backdrop`, `property string question`, `property string confirmText`, `property var payload: null`, `signal confirmed(var payload)`, `function ask(question, confirmText, payload)`; the same `CelestinaModalLayer` + `GlassCard` body; the confirm button `role: CelestinaButton.Destructive`. `ProcessTable` calls `confirm.ask(qsTr("¿Matar «%1» (%2)? El proceso no podrá guardar nada.").arg(name).arg(pid), qsTr("Matar"), pid)` and `onConfirmed: function(pid) { processes.kill(pid) }`.

- [ ] **Step 2: `ServiceRow.qml`** — an `AbstractButton` (`Qt.NoFocus`, `Accessible.role: ListItem`, name composed from the unit name, scope word and state word) with a state dot (`Rectangle` of `CelestinaTheme.compStatusIndicatorSize`, colour `success` for `active`, `danger` for `failed`, `warning` for `activating|deactivating|reloading`, `textFaint` otherwise), the name (`fontRowTitle`), the description (`fontRowSecondary`, muted), and a scope chip (`Text` in a `radiusPill` `Rectangle` with `badgeFill`: `qsTr("Sistema")`/`qsTr("Usuario")`); `CelestinaRowHighlight` Content family for hover/press/selection.

- [ ] **Step 3: `ServicesPage.qml`** — bar: `CelestinaTextField` (Search) with the same 150 ms debounce and `onAccepted`, a `CelestinaCapsule` with two checkable ghost icon buttons `monitor` (`helpText: qsTr("Servicios del sistema")`, bound to `services.showSystem`) and `app-window` (`qsTr("Servicios de usuario")`, `showUser`), a third checkable `view-list` (`qsTr("Solo servicios")`, `servicesOnly`); a second capsule with the actions `media-play` (`qsTr("Iniciar")`), `circle-stop` (`qsTr("Detener")`), `view-refresh` (`qsTr("Reiniciar")`), enabled when the selected row is actionable; stop and restart go through `ConfirmDialog` (`qsTr("¿Detener «%1»?")`, `qsTr("¿Reiniciar «%1»?")`); a sentence line composing availability per bus (`qsTr("No se pudo leer el bus del sistema")`, `qsTr("No se pudo leer el bus de sesión")`) and the action outcome: `done` → `qsTr("%1: hecho").arg(verb)`, `pending` → `qsTr("%1: esperando la autorización").arg(verb)`, `no-agent` → `qsTr("%1: esta sesión no tiene agente de autenticación; la acción sobre unidades del sistema necesita uno").arg(verb)`, `denied` → `qsTr("%1: autorización denegada o cancelada").arg(verb)`, `failed` → `qsTr("%1: systemd rechazó la acción").arg(verb)`, `refused` → `qsTr("%1: esta unidad no admite acciones").arg(verb)`; then a `ListView` (single Tab stop, arrows, `Accessible.role: List`, `Accessible.name: qsTr("Servicios")`) over `page.rows` woven on `revision` with an integer model, selection by unit name + scope key, the same `anchorCursor` pattern as `ProcessTable` (re-anchor after weave, release a vanished selection without clearing an outcome the person has not seen: clear only when the person moves the selection). `ProcessTable`'s not-actionable sentence changes to `qsTr("El proceso %1 pertenece a %2; terminarlo o matarlo pedirá autorización").arg(pid).arg(user)` and its outcome text gains `pending`, `no-agent`, `denied` in the same words.

- [ ] **Step 4: `Main.qml`** — fifth section `{ key: "services", icon: "toolbox", label: qsTr("Servicios") }`, `HematitaServices { id: serviceHub }`, `ServicesPage { services: serviceHub; backdrop: window.contentItem }`, `serviceHub.start()`; the section walk covers five sections automatically. Under `smokeSections`, print once `hematita-services <shownCount> <systemAvailable> <userAvailable>` when the walk reaches the page; `smoke.sh` asserts the line exists with `shownCount > 0` and `systemAvailable === true` (the session bus is deliberately absent in the smoke, so `userAvailable` is `false` there and the page must show its sentence without error).

- [ ] **Step 5: H4 deferrals** — `sampler.rs`: re-run `chip_facts` for every chip every 30 ticks (`SENSOR_FACTS_TICKS = 30`), replacing the cached entry; `sensors.rs`: extremes are retained while the chip key is still listed (keys prefixed by the chip's `chipKey/chipName/`), not only while the channel was read this tick; `publish.rs`: `power_load(value, cap: Option<f64>)` with the same thresholds, used for `ChannelKind::Power` (test it).

- [ ] **Step 6: `build.rs`** — add `ConfirmDialog.qml`, `ServiceRow.qml`, `ServicesPage.qml`; remove `KillDialog.qml` from `QML_FILES` and delete the file (`git rm`).

- [ ] **Step 7: The one build cycle; close H5-B**

`cargo fmt --all`; `build-production.sh`; release-profile `cargo test`/`clippy`/`fmt --check`; `verify-production.sh` (the smoke walks five sections and asserts three shape lines); the 12-second offscreen run with `HEMATITA_SMOKE_SHAPE=1 HEMATITA_SMOKE_SECTIONS=1 QT_ASSUME_STDERR_HAS_CONSOLE=1` grepping the error patterns. No signal and no unit action is sent during verification. Evidence `2026-09-22-h5-services-page.md` (contracts, the ADR's invariants checked against the code — grep that `pkexec` appears only in `privilege.rs` and `Command::new` only there —, Limits: no action performed, no agent in the session, `VAL-H5`). Inventory `H5-B.numstat.tsv`, guards, commit `hematita-maintenance: Add the Services page and the polkit-mediated paths for system units and foreign processes`.

---

### Task 6: Implementation exit — H5-Z

As H4-Z with: `bump hematita milestone --unit H5-Z --summary "Add the Services page and polkit-mediated actions"`; `complete-production.sh`; hashes; roadmap `idle`/`none` with `## H5 — closed 2026-09-22` and "the five phases of the design are delivered; further work opens with a new checkpoint"; STATUS `Delivered as 0.6.0: H5`; AGENTS.md bullet "Privilege follows ADR 0010: one-shot, polkit-mediated, `pkexec` spelled only in `privilege.rs`, every outcome typed; no agent is Hematita's to provide."; plan archived (`Successor: none`); READMEs; evidence `2026-09-22-h5-production-completion.md`; inventory with the deleted active path; guards; commit `hematita-milestone: Add the Services page and polkit-mediated actions`.

---

## Self-review

**Spec coverage (H5):** §1 item 5 services with start/stop → Tasks 3, 5 (plus restart, a natural third verb); §7 H5 row: system bus with polkit, foreign processes via `pkexec`, preceded by a written decision → Task 1 (ADR 0010), Tasks 3–5; §9 no daemon → the ADR and `privilege.rs`; §6 `ServicesPage` registered only in H5 → Task 5. The H4 deferrals → Task 5 Step 5. Deviation recorded: user units are included (the spec says "services over the system bus"; the session's own units are the ones a person can act on today, and listing both is one page).

**Placeholder scan:** none. **Type consistency:** `Scope`, `Unit`, `UnitKind`, `is_actionable`, `project`, `Outcome`, `outcome_of_dbus_error`, `outcome_of_pkexec` (Task 2) ↔ Tasks 3–4; `privilege::{Signal, signal_as_root}` ↔ Task 4; `ServiceSnapshot`/`Snapshot.services` ↔ Task 3; the QML property names are the camelCase of Task 3's contract and are what `ServicesPage.weave()` reads; `ConfirmDialog.ask(question, confirmText, payload)`/`confirmed(payload)` ↔ both pages.
