# Evidence: CEL-2-DOCS-SYNC — the root documents after CEL-2

- **Date:** 2026-11-24
- **Scope:** documents only, all outside `celestina/` (the `celestina:`
  prefix does not cover them): `README.md`, `STATUS.md`, `ROADMAP.md`,
  `HOST-HYGIENE.md`, the CEL-2-M cutover design and its live validation
  order, the session lock design and the polkit agent design under
  `docs/superpowers/`, and the plan's new `CEL-2-DOCS-SYNC` row. No source,
  registry, manifest, version or inventory changed.
- **Environment:** CachyOS, worktree
  `/home/toni/CODIGO/CELESTINA.worktrees/suite-CEL-2-DOCS-SYNC`, branch
  `unit/suite/CEL-2-DOCS-SYNC` cut from `main` at `44b8cc2a` (celestina
  1.14.0) and rebased before its landing onto `c83807be` (celestina 1.15.0,
  CEL-2-M-LOCK-POLKIT), against which the statements were checked again
- **Artifact:** not built; no production entry ran in the session
- **Method:** every statement written was first checked against the tree at
  `44b8cc2a` and, read only, against the live session. Nothing was started,
  stopped or edited outside the worktree.

## What each statement rests on

| Statement | Checked against |
|---|---|
| CEL-2 is complete, CEL-2-M landed as celestina 1.14.0 | `git log origin/main` (`b99a22b4`), `celestina/Cargo.toml` `version = "1.14.0"`, the last rows of `docs/version-history.tsv`, every row of `celestina/docs/plans/active/2026-10-16-cel-2-parity.md` `done` (the plan itself is still active, awaiting its archive) |
| The session runs Celestina and not Noctalia since 2026-11-24 | `~/.config/niri/config.kdl` line 71 `spawn-at-startup "sh" "-c" "exec systemd-cat -t celestina celestina"`; `config.kdl.pre-celestina-cutover` present; `pgrep -a noctalia` empty; `busctl --user list` names `celestina` as the owner of `org.freedesktop.Notifications` and `org.kde.StatusNotifierWatcher`; `pacman -Qs noctalia` still lists the package |
| No `bar` duty; every duty on by default since 1.15.0 (`lock` and `polkit` off until then) | `celestina/docs/decisions/0024-cutover-defaults.md` decisions 1 and 4 and its 2026-11-24 amendment; `celestina/docs/decisions/0026-lock-and-polkit-on-by-default.md`; `c83807be`, `celestina/Cargo.toml` `version = "1.15.0"`; the `duties.bar` comments in `celestina-rs/crates/celestina-shell/src/settings.rs` |
| Caffeine holds a logind `idle` lock only | `celestina/docs/decisions/0025-caffeine-inhibits-idle-only.md`; `VAL-CEL2-CAFFEINE` in `celestina/VALIDATION.md` |
| Caffeine switched on sends `IdleResumed` | `IdleChain::set_caffeine` in `celestina-rs/crates/celestina-shell/src/idle.rs` returns `IdleAction::Resumed` when the period had started `LockAndSuspend`; `perform_idle` in `celestina/src/energy/mod.rs` turns it into `LockEvent::IdleResumed`; test `caffeine_switched_on_withdraws_the_idle_suspend_and_never_a_persons_own` |
| `session suspend` is always refused with the lock text | `SUSPEND_VIA_LOCK` in `celestina/src/energy/logind.rs` |
| The polkit conversation is lazy | the 2026-11-23 amendment of `celestina/docs/decisions/0021-polkit-agent.md` (decision 25) |
| The Qt shell is retired, no project halted | `AGENTS.md` "Halted projects", `docs/projects.toml` `[[retired_projects]]`, `93a17029` |
| Every `VAL-CEL2-*` row is still `pending` in the file, while ADR 0026 records the author's report that five lock and polkit rows passed | `celestina/VALIDATION.md`; ADR 0026 Context |

## Changed

- `docs/superpowers/plans/2026-10-23-cel-2-m-live-validation-order.md`: a
  status note at the top (the swap done on 2026-11-24, 1.14.0; the rows still
  pending, `celestina/VALIDATION.md` the authority; the Noctalia steps no
  longer apply). Three steps that are still instructions corrected: row 8
  (no `bar` duty), row 22 (`idle`, not `idle:sleep`), row 24 (the refusal
  text of `session suspend`).
- `docs/superpowers/specs/2026-10-23-celestina-cel-2-m-cutover-design.md`: a
  status note at the top (done 2026-11-24 with 1.14.0; what the landed code
  changed against the design: ADR 0024, ADR 0025, the ADR 0021 amendment;
  §6 still holds as the way back), and an amendment under §5.1 saying the
  `"bar"` key of `duties` is read past.
- `docs/superpowers/specs/2026-10-17-celestina-cel-2-h-session-lock-design.md`:
  the status bullet, the `IdleResumed` row of the policy table (§4.4), the
  idle trigger of §4.9 and the `IdleChain::resumed` bullet now say that
  switching caffeine on also sends `IdleResumed` (CEL-2-B-CAFFEINE, ADR 0025).
- After the rebase onto `c83807be`: the cutover design's and the validation
  order's notes, the lock design's status bullet and H-11, the polkit
  design's status bullet and its two "`off` by default" lines, `STATUS.md`
  and `ROADMAP.md` also name CEL-2-M-LOCK-POLKIT (1.15.0, ADR 0026).
- `docs/superpowers/specs/2026-10-17-celestina-polkit-agent-design.md`: the
  status bullet and a note after the data flow of §4.3 refer to the lazy
  conversation of CEL-2-I-CANCEL (ADR 0021, decision 25), which governs where
  the design opens the helper connection at the card's readiness.
- `STATUS.md`: date, current focus (Celestina is the session's shell, CEL-2
  complete at 1.14.0), the RSH-1 paragraph (the ledger also carried the CEL-2
  suite units and the Qt retirement), and the blockers paragraph, which still
  said the Celestina shell was halted.
- `ROADMAP.md`: a paragraph under RSH-1 naming the CEL-2 and Qt-retirement
  suite units the ledger carried and the program's close. The checkpoint
  stays RSH-1, active, because its plan is not archived.
- `README.md`: the `celestina` row of the projects table said "CEL-1 in
  progress".
- `HOST-HYGIENE.md`: the Noctalia decision said "pending … the shell is
  halted"; it now records the 2026-11-24 cutover and that removing the
  package stays the author's action.

## Left as history, with the reason

`rg` over the tree outside `celestina/` and `docs/inventories/` for
`idle:sleep`, `off by default`, `duties.bar`, `"bar": "on"` and `Noctalia`:

| Match | Why it stays |
|---|---|
| `docs/version-history.tsv` rows 1.1.0–1.13.0 ("off by default") | append-only history; true when each version landed |
| `docs/superpowers/specs/2026-10-23-celestina-cel-2-m-cutover-design.md` §2.3 ("off by default", `duties.bar` `Option<Duty>`), §2.5, §4, §5.1's JSON, §7, §9, §10 | the facts and plan of 2026-10-23; the new status note and the §5.1 amendment say what changed |
| `docs/superpowers/specs/2026-10-17-celestina-cel-2-h-session-lock-design.md` H-11 ("`duties.lock`, off by default") and the polkit design's two "`off` by default" lines | kept as written and marked **amended 2026-11-24** with ADR 0026 (on by default since 1.15.0), after the rebase onto `c83807be` |
| `docs/superpowers/specs/2026-10-16-celestina-parity-program-design.md` decision 11 and its other Noctalia mentions | the program's design of 2026-10-16; its decision became the shell's ADR 0011, whose "a missing key means `off`" ADR 0024 supersedes for the shipped duties |
| `docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md`, `2026-10-17-cel-2-k-glance-design.md` and the plans `2026-10-14-celestina-rust-shell.md`, `2026-10-17-celestina-polkit-agent.md`, `2026-10-17-celestina-cel-2-h-session-lock.md`, `2026-10-23-cel-2-m-cutover.md` | designs and task plans of landed units, written while Noctalia ran; none gives a current instruction about the live session |
| `docs/evidence/2026-10-23-cel-2-m-docs.md`, `docs/evidence/2026-10-08-cuprita-handover.md`, `cuprita/docs/evidence/2026-10-08-audio.md` | evidence of their date |
| `docs/history/suite-roadmap-through-2026-08-03.md`, `celestina-style/docs/history/roadmap-through-2026-08-03.md` | history |
| `HOST-HYGIENE.md` "Transitional shell authorities" observed role, consumers and decision order item 8 | the observed state of that audit; only its decision line is current and was updated |
| `STATUS.md` AUD-1 paragraph and `ROADMAP.md` AUD-1 and RSH-1 text naming the halted shell | the record of those programs, true when they ran |
| `celestina-rs/crates/celestina-shell/src/settings.rs`, `src/lock.rs`, `tests/salvage.rs`; `celestina-rs/crates/cuprita-core/tests/*` | code and fixtures, outside a documents unit; the settings comments already describe the cutover |

## Not changed, found on the way (outside this unit's scope)

- `celestina/STATUS.md` still says CEL-2-M is "implemented on its branch, not
  landed" and is dated 2026-11-23; `celestina/ROADMAP.md` still names CEL-2
  the active checkpoint.
- `VAL-CEL2-SESSION` in `celestina/VALIDATION.md` still expects
  `refused: no locker yet`; the code answers with `SUSPEND_VIA_LOCK`.
- Both are `celestina:` documents for a unit of that project.

## Procedure

Run from the worktree root, each command recorded with its exit code:

1. `bash scripts/check-architecture-contract.sh`
2. `python3 scripts/check-language-contract.py`
3. `bash scripts/check-documentation-contract.sh`

## Result

| Command | Exit | Output |
|---|---|---|
| `bash scripts/check-architecture-contract.sh` | 0 | `Architecture contract: OK` (sealed colour, contrast, QML visual, radius, glass-canvas and activation contracts OK) |
| `python3 scripts/check-language-contract.py` | 0 | `Language contract: OK (128 legacy file(s) ratcheted)` |
| `bash scripts/check-documentation-contract.sh` | 0 | `Documentation contract: OK`; the `erratum (unbounded-pathspec …)` lines it prints concern `siderita/docs/inventories/2026-08-04-shared-reading-surface/SID-G7-D.numstat.tsv`, an immutable inventory, and are not errors of this unit |

## Limits

- Documents only; no production entry ran and nothing was deployed.
- The live session was read, never changed.
- Nothing was stashed.

## Landing

- **Base revision:** `c83807be01db39a41b767d8e67303f1c9b766230`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
