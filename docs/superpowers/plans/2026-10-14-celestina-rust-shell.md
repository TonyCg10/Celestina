# Celestina Rust Shell (CEL-1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver CEL-1, the Celestina shell rebuilt in pure Rust on Wayland for niri: an Island on the top edge of every output that grows into pages, a launcher, a notification server with banners and a system tray, painted with the Celestina visual language over the compositor's glass, at 1.0.0.

**Architecture:** One process on a `calloop` loop with worker threads; three toolkit crates inside `celestina/crates` (`celestina-wayland` over smithay-client-toolkit with the only `unsafe` in `egl.rs`, `celestina-paint` over femtovg and cosmic-text behind a `Painter` contract, `celestina-ui` with a keyed node tree over taffy); the product in `celestina/src`; the pure domain salvaged by copy into `celestina-rs/crates/celestina-shell`; tokens generated from `celestina-style/tokens.toml` into QML and the `celestina-tokens` crate. The spike at `/home/toni/CODIGO/isla-spike` is the reference implementation of the runtime, painter, UI core and Island; every port below names the spike file it starts from and the ledger findings it closes.

**Tech Stack:** Rust (workspace toolchain `celestina-rs/rust-toolchain.toml`, 1.97.1; minimum 1.89), smithay-client-toolkit 0.21, wayland-client 0.31, wayland-protocols 0.32, calloop 0.14, calloop-wayland-source 0.4, wayland-egl 0.32, khronos-egl 6, glow 0.18, femtovg 0.27, cosmic-text 0.19, taffy 0.15, resvg 0.48, freedesktop-icons 0.4, zbus 5 (blocking), system-tray 0.8, niri-ipc = 26.4.0, serde_json 1, tracing; Python 3 (standard library only) for the token generator and the guards.

**Spec:** `docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md` (commit `d393c175`). The plan argues from the spec; executors read both. Section references below (§n) are the spec's.

## Global Constraints

- **Identifiers (spec head):** project `celestina`, path `celestina/`, commit prefix `celestina:`, registry kind `rust-shell`, unit prefix `CEL`, binary `celestina`, bus name `org.celestina.Shell`, object `/org/celestina/Shell`, interface `org.celestina.Shell1`; layer-shell namespaces `celestina-panel` (Island), `celestina-overlay` (launcher), `celestina-toasts` (banners). The halted Qt shell becomes id, path and prefix `celestina-qt`, still `kind = "qt-shell"` and `halted = "2026-09-27"`.
- **The halt (§3):** `celestina-qt/` and `celestina-rs/crates/celestina-shell-core` are never edited by an agent, in any unit of this plan. Only the modules the author's "Halted projects" edit names are copied; each copy's head comment records the source file and the commit of `celestina-qt` it came from. Until CEL-1-A lands, `celestina/` is the halted tree: no agent unit touches it; CEL-1-A0 touches only `scripts/`, `docs/` and the root documents.
- **Root `AGENTS.md`:** pure domain in `celestina-rs/`; typed errors with context; no production `unwrap`, `expect` or `panic!`; `unsafe` only in `celestina/crates/celestina-wayland/src/egl.rs` (`#![forbid(unsafe_code)]` in every other module and crate; the approval is §5.2 of the spec); every dependency justified by a comment in its `Cargo.toml`; background work bounded, cancellable and joined deterministically; D-Bus, compositor and file input treated as hostile and bounded.
- **Language:** development truth in English (code, comments, tests, documents, commit subjects); product copy in Spanish only in `celestina/src/copy.rs`, whose head line is `// language-contract: product-copy`, and in `celestina.desktop`'s locale-qualified keys. Agents speak to the author in Spanish.
- **Style (§3, `celestina-style/DESIGN.md`):** Haze = compositor blur (σ ≈ 12 px) + grain at **0.15** under the tint + canvas at **0.70**; no outline, no lit edge, no shadow; radii 20/12/8/10; rows 40, top bar 48; curve `(0.22, 0.25, 0, 1)`; durations 100/200/350 ms, exit 150 ms, ceiling 500 ms; press recoil 0.96; Inter Variable 400 and 600 with `tnum`; accent `#3e91ff`; reduced motion makes spatial and scale motion instant. Capsules are borderless; their own fill is transparent while the compositor blur is armed and the opaque canvas (`glassFallback`) when `ext_background_effect_v1` is absent; blur regions are finite and reapplied whenever a silhouette changes.
- **Wayland (§5.2, §5.5, §5.6):** `wp_viewporter` and `wp_fractional_scale_v1` required (exit with an error naming the missing global); EGL config with an 8-bit stencil; buffers premultiplied; swap interval 0; redraws paced by `wl_surface.frame` only; idle surface = zero frames; blur and input regions equal the painted silhouette, sliced in horizontal bands of at most 2 logical px, set and committed with the next frame; keyboard `None` at rest, `OnDemand` with a page open, `Exclusive` for the launcher; every open surface closes on Escape, click outside, focus loss, and `popup_done` for popups; the shell never depends on `focus-follows-mouse`.
- **Compositor integration (§5.6):** niri only; the event stream is parsed line by line, an unknown variant is logged once per name and skipped, only a closed socket reconnects after 1 s; requests through a bounded queue of 32 on a worker; well-known names claimed with `DoNotQueue` after their objects are served, `NameTaken` disables the provider and records the owner for the page; no retry.
- **Delivery (§7, `docs/contracts/landing.md`):** every unit is prepared in a session worktree (`scripts/worktree.sh open <owner> <unit>`, `--from unit/<owner>/<dependency>` when stacked), with an `active` ledger row and a dated evidence record under the owner's `docs/evidence/`; the landing bumps versions, writes inventories and commits. A session never bumps a version, never writes an inventory, never runs `build-production.sh`. Session commit subjects follow `<prefix>-maintenance: <English imperative>`; the author chooses the kind at landing. Owners: CEL-1-A0, CEL-1-A1, CEL-1-A and CEL-1-C are `suite:` units of `docs/plans/active/2026-10-14-rust-shell.md` (Plan ID `rust-shell`, checkpoint `RSH-1`); CEL-1-B, D, E, F, G and H are `celestina:` units of `celestina/docs/plans/active/2026-10-14-cel-1-island.md` (Plan ID `cel-1-island`, checkpoint `CEL-1`). CEL-1-A is the author's unit: the agent prepares its diff on `unit/suite/CEL-1-A` (stacked on `unit/suite/CEL-1-A1`) and stops; the author reviews, edits "Halted projects" in their own words, and lands it.
- **Version ladder:** CEL-1-A registers `celestina` unversioned (the landing's `unbump` cannot carry a baseline row); the author adopts the `0.1.0` baseline by hand on `main` right after A lands (registry `version_source` + mirror + the history row, `suite-maintenance`), as Cuprita's registration did; B, D, E, F and G land as `celestina-milestone` (0.2.0 … 0.6.0); H lands as `celestina-release` (1.0.0). A0, A1, A and C land as `suite-maintenance`.
- **Before every session commit, from the worktree root:** `bash scripts/check-architecture-contract.sh`, `python3 scripts/check-language-contract.py`, `bash scripts/check-documentation-contract.sh`, `python3 scripts/version_tool.py check`; from `celestina/`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`; from `celestina-rs/` when a crate there changed: `cargo fmt --all --check`, `cargo clippy --locked -p <crate> --all-targets -- -D warnings`, `cargo test --locked -p <crate>`; scope with `python3 scripts/commit_scope.py --check "<subject>"` (staged paths on stdin). Never `git stash`, never `cargo clean`.
- **Cargo:** `celestina/Cargo.toml` is its own workspace (`members = ["crates/*", "."]`), with `[workspace.lints.rust] unsafe_code = "deny"` (a workspace `forbid` cannot be relaxed by any inner attribute), every crate root and `src/main.rs` carrying `#![forbid(unsafe_code)]`, except `celestina-wayland`'s `lib.rs`, which carries `#![deny(unsafe_code)]` and lets only `egl.rs` declare `#![allow(unsafe_code)]`; the A skeleton keeps `forbid` at the workspace until B introduces `egl.rs`; the suite's `celestina-rs` crates are path dependencies. Dependency versions as the Tech Stack lists them; no async executor.
- **Tests:** pure logic first, red then green (`cargo test` names below are the contract); Wayland and GL code is exercised by `scripts/smoke.sh` (nested niri, skipped with a notice when `niri` is absent) and by the author's rows in `celestina/VALIDATION.md`.

## Review Focus

Inputs the spec implies but no task's tests exercise by default; each line's test is added to the owning task in that task's style:

1. **An output is unplugged while its Island page or popup is open** (CEL-1-B, Task 10): the runtime destroys popups before their parent, the Island's view is dropped, no frame callback for the dead surface is dispatched, and the process keeps running for the other outputs.
2. **A notification with a 50 000-character body, 1 000 actions or an `image-path` pointing outside the home directory** (CEL-1-F, Task 19): bounded by `celestina-shell::notifications` and the bus glue, shown truncated, never read as bytes.
3. **`NIRI_SOCKET` unset or the socket gone for minutes** (CEL-1-B, Task 9): the capsule shows the fallback label, the worker reconnects once per second and logs the failure once per state change, not once per attempt.
4. **The compositor has no `ext_background_effect_v1`** (CEL-1-B, Task 6): every Haze surface paints the opaque fallback canvas and stays readable; no blur request is sent; the metrics flag still counts frames.
5. **A launcher query that is only `/`, or `/` followed by an unknown word, or 10 000 characters** (CEL-1-E, Task 16): the first lists every switch, the second shows no rows and launches nothing, the third is clamped to 256 characters before ranking.

## Units, owners and tasks

| Unit | Owner plan | Landing kind | Tasks |
|---|---|---|---|
| CEL-1-A0 | suite | maintenance | the hook-runner migration path, dormant: `pre-commit` measures a staged language scanner migration against HEAD's rows; the suite plan |
| CEL-1-A1 | suite | maintenance | 1–4: the version-contract alias, the inventory-move rule, the halted-tree language rule, the rename rehearsal; stacked on CEL-1-A0 |
| CEL-1-A2 | suite | maintenance | the landing tool's rename awareness: the plans the rename moves unchanged, the links that follow them, a new plan with no open row; lands before CEL-1-A |
| CEL-1-A3 | suite | maintenance | the landing preflight's inventory check: the inventories the rename moves unchanged are not edits of tracked inventories; lands before CEL-1-A |
| CEL-1-A4 | suite | maintenance | the landing's unbump step: a renamed halted owner's version sources are read at their new paths against the former owner's at the fork point; lands before CEL-1-A |
| CEL-1-A | suite (author) | maintenance | 5: the rename diff, prepared by the agent |
| CEL-1-B | celestina | milestone | 6–11: workspace, `celestina-wayland`, `celestina-paint`, `celestina-ui`, the Island capsule, command surface, scripts and documents |
| CEL-1-C | suite | maintenance | 12–13: `tokens.toml`, the generator, both outputs, the drift checks, the shell on the crate |
| CEL-1-D | celestina | milestone | 14–15: Island pages, tabs, the overflow popup |
| CEL-1-E | celestina | milestone | 16–17: `Field`, the launcher |
| CEL-1-F | celestina | milestone | 18–19: notifications server, banners, history, do-not-disturb |
| CEL-1-G | celestina | milestone | 20–21: tray watcher and host, items, DBusMenu popups |
| CEL-1-H | celestina | release | 22: exit, 1.0.0, handover |

## Decisions this plan adds to the spec

1. **The rename needs two rule changes before it can land (CEL-1-A1).** Verified on 2026-10-14 against the committed guards: `scripts/version_contract.py` refuses a suite-maintenance commit that changes an existing owner's configuration (`_validate_policy_addition`) and refuses history rows whose owner is not registered, and `docs/version-history.tsv` is append-only byte for byte, so the 68 rows owned by `celestina` (0.1.0 … 1.3.2) can neither be re-owned nor stay as they are once `celestina` is a different project. `scripts/check-staged-units.py` refuses any staged inventory that exists at HEAD (`historical inventories are immutable`), so the 63 inventories under `celestina/docs/inventories/` cannot move with the tree. A1 adds the `history_owner` alias to the version contract (the author's choice: no new row kind, `docs/version-history.tsv` untouched by A0 and only appended by A) and inventory moves under a renamed halted owner to the staged-units guard, each dormant until A uses them (change policy, "Scope and unrelated work"). The id `celestina` for the Rust shell is the author's firm decision; the alias exists so that id can be reused. The landing tool needed the same rename awareness (CEL-1-A2), its preflight's inventory check (CEL-1-A3) and its unbump step (CEL-1-A4): on 2026-10-15 CEL-1-A's landing stopped at preflight on the moved plan (`the branch deletes its plan celestina/docs/plans/active/2026-08-20-persistent-carriers.md`), so `discover_unit` now leaves out the plans the rename moves unchanged and sets aside the plans that only follow it and the new shell plan, which has no open row.
2. **A halted project is history for the language scanner (CEL-1-A1).** `scripts/check-language-contract.py` counts legacy debt in 8 files under `celestina/` and refuses a new baseline row for any path, so renaming the rows would be refused as new entries. A1 makes `is_history` true for paths under a project the registry marks `halted`, removes the 8 rows in the same commit, and records the exact field ``- **Resolved language debt:** `scripts/check-language-contract.py` `` in its evidence, which is what the change policy requires for a scanner migration.
3. **Deploy path.** The new shell installs `celestina/target/release/celestina` to `$prefix/bin/celestina` and `celestina.desktop` to `$prefix/share/applications/`, through `production_install_xdg_application`. That overwrites the halted shell's launcher at `$prefix/bin/celestina`; its bundle at `$prefix/libexec/celestina/` is left untouched, so rollback is lifting the halt and running `celestina-qt/scripts/deploy-production.sh`. The author confirms this in A's review.
4. **Component prefixes.** `celestina-shell` is a component scope of `celestina` (its `commit_roots` include `celestina-rs/crates/celestina-shell/`, as §6.1 says, like `cuprita-core` under `cuprita`); `celestina-tokens` is a component scope of `celestina-rs` (it is generated from the style and will have other consumers). §7's sentence naming both under `celestina-rs` is read this way.
5. **CEL-1-C is one suite unit** because tokens-as-data is a cross-suite invariant (style, workspace and shell change together and the drift check spans two verifies). Its landing rebuilds every application whose production inputs include `celestina-style/*.qml`; the author may split it into `celestina-style-milestone` + `celestina-rs-maintenance` + `celestina-maintenance` instead, landed in that order.
6. **A registers the whole document and script set of `celestina/`**, with a stub binary, because the documentation contract requires every registered path to exist and the landing refuses a registered production input the tip lacks. The stubs are replaced in B; nothing stub-shaped survives B.

7. **Historical links are updated in place (the author's decision to confirm).** Eleven links into `celestina/` from `docs/history/`, `docs/plans/archive/`, two evidence records, `README.md` and `ROADMAP.md` would break the documentation contract after the move. The convention for historical records is to append a correction, not to rewrite; A changes only the link targets (same text, same meaning), which the plan treats as a mechanical consequence of the rename and lists in Task 5 item 5 for the author to accept or replace by errata.
8. **The documentation contract stops checking a renamed halted owner's records (the author's decision to confirm).** A0's rehearsal showed that `check_historical_plan_host` and the ledger checks cannot resolve a moved inventory's host plan by the old identity, so A0 excludes owners that carry both `halted` and `history_owner` from Scope matching and `check_ledger` (inventory claims, pathspecs, `self` rows, host plans, evidence links, archive transitions), while `check-staged-units.py` proves byte identity at the move. Lifting the halt reinstates those checks, which will then fail on the old identity unless an alias is implemented (accepting `history_owner` as the owner identity with path translation for `self` rows). The author accepts the exclusion for A or asks for the alias before lifting the halt.
9. **Output changes are a `Shell::output` call, not a `Message` variant.** §5.2 says they arrive as messages; they originate in the runtime's own `OutputHandler`, not in a worker, so the trait method keeps the channel for worker-originated data only. The product still creates and destroys one Island per output on those calls, as the spec requires.

---

## Unit CEL-1-A0 — the rules the rename needs (suite, agent)

Worktree: `scripts/worktree.sh open suite CEL-1-A0`. Every task commits as `suite-maintenance: …` in the worktree. The unit lands in two landings: `CEL-1-A0` seals only the hook-runner migration path (`scripts/git_hooks.py` measuring a staged scanner migration against HEAD's language rows, its hooks test, one sentence of `AGENTS.md`, the suite plan and `docs/evidence/2026-10-15-scanner-migration-hook.md`), dormant on today's tree, and `CEL-1-A1`, on `unit/suite/CEL-1-A1` stacked on `unit/suite/CEL-1-A0`, seals Tasks 1–4 (the rules, the 8 retired rows, the rehearsal, the plan and spec amendments) after it — because the change policy requires a semantics-changing rule migration to land compatible dormant behavior first and activate it later, and `main`'s old runner refuses the single squashed commit (the retired rows read as new debt against the staged baseline; the record is in A0's evidence).

### Task 1: version contract — the `history_owner` alias

**Files:**
- Modify: `scripts/version_contract.py` (`OwnerConfig`, `parse_registry`, `_validate_history`, `_validate_policy_addition`)
- Modify: `scripts/test-version-contract.py` (new test class `RenameTests`)
- Modify: `docs/contracts/versioning.md` (new section "Renaming a project")

**Interfaces:**
- Produces: registry key `history_owner = "<name>"` on a project table (the owner name its history rows carry; defaults to the id); `OwnerConfig.history_owner: str`; rule: rows named `N` belong to the project whose `history_owner` is `N` until a second `baseline` row named `N` appears, which starts the chain of the project registered as `N` itself; a suite-maintenance commit may rename owner `F` to `X` when `X.history_owner == F`, `X.versioned == F.versioned`, their version sources have the same kinds and names (paths may differ) and `after.versions[X] == before.versions[F]`; the id `F` may be re-registered in the same commit as a new owner with its own baseline row. `docs/version-history.tsv` is never rewritten: A0 appends nothing, A appends only the new owner's baseline.

- [ ] **Step 1: Write the failing tests.** Append to `scripts/test-version-contract.py`:

```python
class RenameTests(unittest.TestCase):
    """A project may change its registry id once, keeping its history rows."""

    def renamed_fixture(self) -> TransitionFixture:
        fixture = TransitionFixture()
        beta = next(p for p in fixture.index_registry["projects"] if p["id"] == "beta")
        beta["id"] = "beta-old"
        beta["commit_prefix"] = "beta-old"
        beta["history_owner"] = "beta"
        beta["version_source"] = {
            "kind": "cmake-project",
            "path": "beta-old/CMakeLists.txt",
            "project": "beta",
        }
        fixture.blobs["INDEX"].pop("beta/CMakeLists.txt")
        fixture.stage("beta-old/CMakeLists.txt", cmake_project("beta", "2.3.4"))
        fixture.changed_paths.update({"beta/CMakeLists.txt", "docs/projects.toml"})
        return fixture

    def new_beta(self, fixture: TransitionFixture) -> None:
        fixture.index_registry["projects"].append(
            {
                "id": "beta",
                "commit_prefix": "beta",
                "version_source": {
                    "kind": "cargo-package",
                    "path": "beta/Cargo.toml",
                    "package": "beta",
                },
                "component_commit_scopes": [],
            }
        )
        fixture.stage("beta/Cargo.toml", cargo_package("beta", "0.1.0"))

    def test_rename_passes_as_suite_maintenance_without_touching_the_history(self) -> None:
        fixture = self.renamed_fixture()
        fixture.validate("suite-maintenance", "maintenance", "Rename beta")
        self.assertEqual(fixture.blobs["INDEX"][HISTORY_PATH], fixture.blobs["HEAD"][HISTORY_PATH])
        model = contract.parse_registry(fixture.index_registry, "fixture")
        self.assertEqual(model.owner_map()["beta-old"].history_owner, "beta")

    def test_reusing_the_old_id_needs_a_baseline_after_the_old_chain(self) -> None:
        fixture = self.renamed_fixture()
        self.new_beta(fixture)
        with self.assertRaises(contract.VersionHistoryError):
            fixture.validate("suite-maintenance", "maintenance", "Rename beta")
        fixture.append_history("beta", contract.SemVer(0, 1, 0), "baseline", "R-1", "Adopt the new beta")
        fixture.validate("suite-maintenance", "maintenance", "Rename beta")
        snapshot = contract.validate_snapshot(
            contract.parse_registry(fixture.index_registry, "fixture"), "INDEX", fixture.read_blob
        )
        self.assertEqual(snapshot.versions["beta-old"], contract.SemVer(2, 3, 4))
        self.assertEqual(snapshot.versions["beta"], contract.SemVer(0, 1, 0))

    def test_a_later_delivery_of_the_new_owner_chains_from_its_own_baseline(self) -> None:
        fixture = self.renamed_fixture()
        self.new_beta(fixture)
        fixture.append_history("beta", contract.SemVer(0, 1, 0), "baseline", "R-1", "Adopt the new beta")
        fixture.validate("suite-maintenance", "maintenance", "Rename beta")
        fixture.head_registry = deepcopy(fixture.index_registry)
        fixture.blobs["HEAD"] = dict(fixture.blobs["INDEX"])
        fixture.changed_paths = set()
        fixture.deliver("beta", "milestone", "Add the first checkpoint", unit="B-1")
        fixture.validate("beta-milestone", "milestone", "Add the first checkpoint")

    def test_rename_rejections(self) -> None:
        with self.subTest(case="a rename that bumps"):
            fixture = self.renamed_fixture()
            fixture.stage("beta-old/CMakeLists.txt", cmake_project("beta", "2.4.0"))
            with self.assertRaises(contract.VersionContractError):
                fixture.validate("suite-maintenance", "maintenance", "Rename beta")
        with self.subTest(case="a rename under a product prefix"):
            fixture = self.renamed_fixture()
            with self.assertRaises(contract.VersionContractError):
                fixture.validate("alpha-maintenance", "maintenance", "Rename beta")
        with self.subTest(case="an alias with no rows behind it"):
            fixture = self.renamed_fixture()
            beta_old = next(p for p in fixture.index_registry["projects"] if p["id"] == "beta-old")
            beta_old["history_owner"] = "gamma"
            model = contract.parse_registry(fixture.index_registry, "fixture")
            with self.assertRaises(contract.VersionHistoryError):
                contract.validate_snapshot(model, "INDEX", fixture.read_blob)
        with self.subTest(case="two projects naming one history owner"):
            fixture = self.renamed_fixture()
            fixture.index_registry["projects"][0]["history_owner"] = "beta"
            with self.assertRaises(contract.VersionRegistryError):
                contract.parse_registry(fixture.index_registry, "fixture")
        with self.subTest(case="an alias naming itself"):
            fixture = self.renamed_fixture()
            fixture.index_registry["projects"][0]["history_owner"] = "alpha"
            with self.assertRaises(contract.VersionRegistryError):
                contract.parse_registry(fixture.index_registry, "fixture")
```

- [ ] **Step 2: Run them to see them fail.** `python3 scripts/test-version-contract.py -k Rename` → FAIL (`history_owner` attribute missing; the rename refused as "cannot change or remove existing owner config").

- [ ] **Step 3: Implement.** In `scripts/version_contract.py`, `OwnerConfig` gains `history_owner: str = ""` (after `mirrors`; `parse_registry` always fills it). In `parse_registry`, after `prefix` is read:

```python
        raw_alias = raw_project.get("history_owner")
        history_owner = owner
        if raw_alias is not None:
            history_owner = _required_string(raw_alias, f"{project_label}.history_owner")
            if history_owner == owner:
                raise VersionRegistryError(f"{project_label}.history_owner names itself")
            if history_owner in seen_aliases:
                raise VersionRegistryError(f'{label} repeats history_owner "{history_owner}"')
            seen_aliases.add(history_owner)
```

(`seen_aliases: set[str] = set()` next to `seen_ids`; `OwnerConfig(owner, prefix, versioned, source, mirrors, history_owner)`.)

Replace the first loop of `_validate_history`:

```python
def _validate_history(model, versions, rows, label) -> None:
    owners = model.owner_map()
    alias = {c.history_owner: c.owner for c in model.owners if c.history_owner != c.owner}
    grouped: dict[str, list[HistoryRow]] = defaultdict(list)
    for row in rows:
        owner_id = row.owner
        target = alias.get(row.owner)
        if target is not None:
            if row.kind == "baseline" and grouped.get(target):
                # The aliased chain already has its baseline: a second
                # baseline under this name starts the project registered
                # under the name itself.
                del alias[row.owner]
            else:
                owner_id = target
        config = owners.get(owner_id)
        if config is None:
            raise VersionHistoryError(f'{label}: history names unknown owner "{row.owner}"')
        if not config.versioned:
            raise VersionHistoryError(f'{label}: unversioned owner "{row.owner}" has history')
        grouped[owner_id].append(row)
    for config in model.owners:
        if config.history_owner != config.owner and not grouped.get(config.owner):
            raise VersionHistoryError(
                f'{label}: "{config.owner}" declares history_owner "{config.history_owner}" '
                "but no history row carries that name"
            )
    # the per-owner chain checks (baseline first, exact bumps, last == source) stay as they are
```

In `_validate_policy_addition`, replace the owner loop and the checks that follow:

```python
    head_owners = head.owner_map()
    index_owners = index.owner_map()
    successors = {
        config.history_owner: config
        for config in index.owners
        if config.history_owner != config.owner and config.history_owner in head_owners
        and config.owner not in head_owners
    }
    adopted: list[OwnerConfig] = []
    renamed: dict[str, OwnerConfig] = {}
    for owner, config in head_owners.items():
        after_config = index_owners.get(owner)
        successor = successors.get(owner)
        if successor is not None:
            same_shape = (
                successor.versioned == config.versioned
                and [(s.kind, s.name) for s in successor.sources]
                == [(s.kind, s.name) for s in config.sources]
                and (not config.versioned
                     or before.versions.get(owner) == after.versions.get(successor.owner))
            )
            if not same_shape:
                raise VersionRegistryError(
                    f'suite-maintenance rename "{owner}" -> "{successor.owner}" '
                    "must keep the version and the source shape"
                )
            renamed[owner] = successor
            if after_config is not None:
                adopted.append(after_config)  # the old id is a new owner now
            continue
        if after_config == config:
            continue
        adopts = (
            after_config is not None
            and not config.versioned
            and after_config.versioned
            and replace(after_config, versioned=False, source=None, mirrors=()) == config
        )
        if not adopts:
            raise VersionRegistryError(
                f'suite-maintenance cannot change or remove existing owner config "{owner}"'
            )
        adopted.append(after_config)
    added = [
        owner for owner in index.owners
        if owner.owner not in head_owners and owner not in renamed.values()
    ] + adopted
    if not added and not renamed:
        _validate_no_delta(head, index, before, after)
        return

    history_path = head.policy.history_file if head.policy is not None else ""
    new_rows = appended_history_rows(before, after, history_path)
    expected = {owner.owner for owner in added if owner.versioned}
    if len(new_rows) != len(expected) or {row.owner for row in new_rows} != expected:
        raise VersionHistoryError(
            "each newly versioned owner requires exactly one appended baseline row"
        )
    for row in new_rows:
        if row.kind != "baseline" or row.version != after.versions[row.owner]:
            raise VersionHistoryError(
                f'new owner "{row.owner}" requires a baseline at {after.versions[row.owner]}'
            )
    for owner, version in before.versions.items():
        current = renamed[owner].owner if owner in renamed else owner
        if after.versions.get(current) != version:
            raise VersionTransitionError(
                "suite-maintenance owner additions cannot bump existing versions"
            )
    if new_rows and history_path not in changed_paths:
        raise VersionTransitionError(f"{history_path} must be staged with new baselines")
```

- [ ] **Step 4: Run the whole file.** `python3 scripts/test-version-contract.py` → every test passes, including `test_current_repository_static_contract` (no project declares `history_owner` yet, so the repository is unchanged: the rule is dormant until CEL-1-A).

- [ ] **Step 5: Document.** Add to `docs/contracts/versioning.md`, after "Ownership and cross-suite changes":

```markdown
## Renaming a project

A registered project changes its id once, in a `suite-maintenance` commit, by
declaring `history_owner = "<old id>"` in its registry table: the history rows
that carry the old name are its rows, the version is unchanged and the sources
keep their kinds and names (their paths may change). History is never
rewritten. The old id may be registered again in the same commit as a new
project; its own baseline row, appended after the old chain, is where the old
name stops meaning the renamed project. `scripts/version_contract.py` refuses
a rename that bumps, a `history_owner` with no rows behind it, two projects
naming one history owner, and a rename under a product prefix.
```

- [ ] **Step 6: Commit.** `git add scripts/version_contract.py scripts/test-version-contract.py docs/contracts/versioning.md` · `suite-maintenance: Add the history_owner alias to the version contract`.

### Task 2: staged-units guard — inventories move with a renamed halted owner

**Files:**
- Modify: `scripts/check-staged-units.py` (new `registry_owner_roots`, `inventory_moves`; `validate_batch`)
- Modify: `scripts/test-staged-units.sh` (new case block at the end, before the final `echo`)
- Modify: `docs/governance/change-policy.md` ("Exact immutable inventories": one sentence)

**Interfaces:**
- Produces: in a staged batch, a pair (path `P` under HEAD owner `F`'s inventory root, deleted; path `Q` under INDEX owner `X`'s inventory root, where `X.history_owner == F` and `X` carries `halted`, added, bytes equal to `HEAD:P`) is a **move**: neither path counts as a staged inventory (no closure, no immutability error), both stay in the batch's path set and must be claimed by the batch's inventory (`P` as `deleted`, `Q` by its SHA-256).

- [ ] **Step 1: Write the failing test.** Append to `scripts/test-staged-units.sh` before its final success line:

```sh
# A renamed halted owner (docs/projects.toml `history_owner` + `halted`) may
# move its historical inventories with its tree; a moved copy with different
# bytes is an edit.
rename_root=$temporary/renamed-owner
mkdir -p "$rename_root/docs/plans/active" "$rename_root/app/docs/plans/archive" \
    "$rename_root/app/docs/inventories/2026-08-03-unit" "$rename_root/scripts"
cp "$case_root/scripts/"*.py "$rename_root/scripts/"
cp "$case_root/docs/projects.toml" "$rename_root/docs/projects.toml"
printf 'Base revision\t%s\nPathspec\tapp/\n' "$base" \
    > "$rename_root/app/docs/inventories/2026-08-03-unit/UNIT-A.numstat.tsv"
printf '# Old plan\n' > "$rename_root/app/docs/plans/archive/2026-08-03-unit.md"
git -C "$rename_root" init -q
git -C "$rename_root" config user.name "Staged Fixture"
git -C "$rename_root" config user.email "fixture@example.invalid"
git -C "$rename_root" add .
git -C "$rename_root" commit -qm "fixture: an owner with history"
git -C "$rename_root" mv app app-old
sed -i 's|^id = "app"$|id = "app-old"\nhistory_owner = "app"\nhalted = "2026-09-27"|; s|^commit_prefix = "app"$|commit_prefix = "app-old"|; s|^path = "app"$|path = "app-old"|; s|app/docs/plans/active|app-old/docs/plans/active|; s|commit_roots = \["app/"\]|commit_roots = ["app-old/"]|' \
    "$rename_root/docs/projects.toml"
git -C "$rename_root" add -A
if ! python3 "$checker" --root "$rename_root" --quiet; then
    fail "inventories moved with their renamed owner were refused"
fi
printf 'tampered\n' >> "$rename_root/app-old/docs/inventories/2026-08-03-unit/UNIT-A.numstat.tsv"
git -C "$rename_root" add -A
if python3 "$checker" --root "$rename_root" --quiet 2>"$temporary/tampered.err"; then
    fail "a moved inventory with changed bytes was accepted"
elif ! grep -q 'historical inventories are immutable' "$temporary/tampered.err"; then
    fail "the tampered move produced no immutability diagnostic"
fi
```

- [ ] **Step 2: Run it.** `sh scripts/test-staged-units.sh` → FAIL on the first new case (`historical inventories are immutable`).

- [ ] **Step 3: Implement.** In `scripts/check-staged-units.py`, next to `registry_delivery_layouts`:

```python
def registry_owner_roots(root: Path, revision: str) -> dict[str, tuple[str, str | None]]:
    """{owner id: (inventory root, history_owner or None, halted)} for one registry revision."""
    raw = index_bytes(root, "docs/projects.toml") if revision == "INDEX" \
        else git(root, "show", "HEAD:docs/projects.toml", check=False) or None
    if raw is None:
        return {}
    registry = tomllib.loads(raw.decode("utf-8"))
    roots: dict[str, tuple[str, str | None, bool]] = {}
    for owner in registry.get("projects", []):
        owner_id, active, former = owner.get("id"), owner.get("active_plans"), owner.get("history_owner")
        if not isinstance(owner_id, str) or not isinstance(active, str):
            continue
        docs_root = posixpath.dirname(posixpath.dirname(normalized_repo_path(active)))
        roots[owner_id] = (posixpath.join(docs_root, "inventories"), former if isinstance(former, str) else None, "halted" in owner)
    return roots


def inventory_moves(root: Path, staged_paths: set[str]) -> dict[str, str]:
    """Moved inventories, {deleted HEAD path: added INDEX path with equal bytes}."""
    head = registry_owner_roots(root, "HEAD")
    moves: dict[str, str] = {}
    for _owner, (new_root, former, halted) in registry_owner_roots(root, "INDEX").items():
        if former is None or former not in head or not halted:
            continue
        old_root = head[former][0]
        if old_root == new_root:
            continue
        for path in staged_paths:
            if not path.startswith(f"{old_root}/") or not path.endswith(".numstat.tsv"):
                continue
            if index_bytes(root, path) is not None or not path_exists_at_head(root, path):
                continue
            new_path = new_root + path[len(old_root):]
            if index_bytes(root, new_path) == git(root, "show", f"HEAD:{path}"):
                moves[path] = new_path
    return moves
```

In `validate_batch`, right after `staged_paths` is computed, add `moves = inventory_moves(root, staged_paths)` and `moved = set(moves) | set(moves.values())`; in the `all_staged_inventories` comprehension add `and path not in moved`; before the final `print` add `if moves and not quiet: print(f"staged-unit: {len(moves)} inventory file(s) moved with their renamed owner")`.

- [ ] **Step 4: Run.** `sh scripts/test-staged-units.sh` → OK, both new cases.

- [ ] **Step 5: Document.** In `docs/governance/change-policy.md`, "Exact immutable inventories", after "Tracked inventories are immutable…": "A halted project renamed through `history_owner` (see `docs/contracts/versioning.md`, "Renaming a project") moves its inventories unchanged with its tree; the guard treats a deleted path and an added path with the same bytes under the two inventory roots as one move, not as an edit."

- [ ] **Step 6: Commit.** `suite-maintenance: Let inventories move with a renamed halted owner in the staged-units guard`.

### Task 3: halted trees are history for the language scanner, and the rename rehearsal

**Files:**
- Modify: `scripts/check-language-contract.py` (`is_history`, `scan` reading the registry's halted paths), `scripts/test-language-contract.py` (one test), `scripts/language-baseline.tsv` (remove the 8 `celestina/` rows)
- Create: `scripts/rehearse-rename.sh` (throwaway helper kept under `scripts/` only until A lands; A deletes it) — or keep it in the evidence as a fenced script; choose the evidence (nothing to delete later)
- Possibly modify, if the rehearsal shows it: `scripts/documentation_contract.py` (`check_historical_plan_host`), `scripts/test-documentation-contract.sh`

**Interfaces:**
- Consumes: Task 1's `history_owner`, Task 2's moves.
- Produces: the exact list of files and edits that A needs (Task 5 copies it), proven by running every pre-commit guard on a staged rehearsal of the rename in a scratch clone.

- [ ] **Step 1: Failing language test.** In `scripts/test-language-contract.py`, add a test that writes a scratch registry with a halted project at `halted/` and a text file `halted/README.md` containing Spanish prose, and asserts `scan(root)` reports 0 debt for it and that `is_history("halted/README.md", registry)` is true; run → FAIL.

- [ ] **Step 2: Implement.** `is_history(path, halted_roots=())` returns true for `docs/history/` paths (as today) and for any path under a root in `halted_roots`; `scan(root)` computes `halted_roots` from `docs/projects.toml` through `project_registry.halted_projects` (the `path` of each halted project with a trailing slash) and passes it; `main`'s historical comparison is unchanged. Remove the 8 rows `celestina/...` from `scripts/language-baseline.tsv` (lines 37–44). Run `python3 scripts/check-language-contract.py` → OK, no "debt is gone" error, no new entry. `python3 scripts/test-language-contract.py` → OK. (`commit_scope.py` calls `is_history(path)` with one argument through `call_dynamic_rule`; keep that signature valid by making the second parameter optional, and read the halted roots inside `is_history` from the registry when the parameter is omitted, cached once per process.)

- [ ] **Step 3: Rehearse the rename in a scratch clone.** `git clone -q /home/toni/CODIGO/CELESTINA "$SCRATCH/rename"` (scratch under the session's scratchpad), `git config core.hooksPath .githooks`, copy this worktree's `scripts/` on top (Tasks 1–3's rules; the rehearsal runs them from the worktree, which is what HEAD will hold once A0 lands), then apply the whole of Task 5's edit list (`git mv celestina celestina-qt`, the registry, the references, the ratchets, the workflow, the new `celestina/` skeleton, the history rows), `git add -A`, and run:

```sh
python3 scripts/check-staged-units.py --root .
printf '%s\n' $(git diff --cached --name-only --no-renames) | python3 scripts/commit_scope.py --check 'suite-maintenance: Move the halted shell to celestina-qt and register the Rust shell'
python3 scripts/version_tool.py check
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
bash scripts/test-commit-scope.sh
sh scripts/test-documentation-contract.sh
python3 scripts/test-land-unit.py
bash scripts/test-architecture-scanners.sh
```

Expected: all exit 0. `commit_scope.py --check` must also pass without `AGENTS.md` in the list? No: the halt refuses the paths unless `AGENTS.md`'s "Halted projects" changes, so the list includes `AGENTS.md` (Task 5 edits it). Known unknown: `documentation_contract.check_historical_plan_host` reads `git log -1 -- <inventory>` for every tracked inventory and resolves the host plan at that commit; in the rehearsal the commit does not exist yet (index only), so the pre-commit run cannot show it. Commit the rehearsal in the scratch clone (`git commit --no-verify -qm rehearsal`) and run `bash scripts/check-documentation-contract.sh` on the committed tree: if it fails naming a `celestina-qt/docs/inventories/...` file, change `check_historical_plan_host` so that a plan path under a renamed owner's `history_owner` root is resolved through the rename (both roots are known from the registry at that endpoint), add the failing-then-passing case to `scripts/test-documentation-contract.sh` with a fixture under `scripts/fixtures/documentation/`, and rerun. Any other guard failure is fixed the same way: a rule change with its test, never an edit of the halted bytes.

- [ ] **Step 4: Record.** Write `docs/evidence/2026-10-14-rename-rules.md` (template `docs/templates/evidence.md`) with the commands and exits of Steps 1–3, the rehearsal's exact edit list (Task 5's list with any correction the rehearsal taught), and the exact line ``- **Resolved language debt:** `scripts/check-language-contract.py` `` in its Result section.

- [ ] **Step 5: Commit.** `suite-maintenance: Count no language debt in halted trees and rehearse the shell rename`.

### Task 4: the suite plan, roadmap and status

**Files:**
- Create: `docs/plans/active/2026-10-14-rust-shell.md`
- Modify: `docs/plans/active/README.md`, `ROADMAP.md` (status `active`, checkpoint `RSH-1`, one paragraph under a new "RSH-1" section), `STATUS.md` (current focus, implementation checkpoint `RSH-1`)

- [ ] **Step 1: Plan.** From `docs/templates/plan.md`:

```markdown
# RSH-1 — Register the Celestina shell in pure Rust

- **Opened:** 2026-10-14
- **Plan ID:** rust-shell
- **Status:** active
- **Authorization:** the author approved the design in brainstorming on
  2026-10-14 and asked for the implementation plan; the design is
  [the spec](../../superpowers/specs/2026-10-14-celestina-rust-shell-design.md)
  and the task breakdown is
  [the plan](../../superpowers/plans/2026-10-14-celestina-rust-shell.md)
- **Scope:** suite
- **Implementation checkpoint:** RSH-1
- **Author-validation checkpoint:** none

## Hypothesis

The halted Qt shell can move to `celestina-qt/` and the Rust shell take
`celestina/` without rewriting any inventory, history row or halted byte,
once the version contract knows a renamed owner and the staged-units guard
knows a moved inventory.

## Tangible outcome

`celestina-qt/` registered, halted and byte-identical to the old tree;
`celestina/` registered as `rust-shell` with its document set and a stub
binary at 0.1.0; the shared tokens as data consumed by QML and Rust.

## Scope

- `CEL-1-A0` — the two guard rules, the halted-tree language rule, the rehearsal.
- `CEL-1-A` — the author's rename and registration.
- `CEL-1-C` — `celestina-style/tokens.toml`, the generator, both outputs and the drift checks.

## Exclusions

- The shell's own units (`CEL-1-B`, `-D` to `-H`), which belong to
  `celestina/docs/plans/active/2026-10-14-cel-1-island.md`.
- Any edit of `celestina-qt/` or `celestina-rs/crates/celestina-shell-core`.

## Build order

1. `CEL-1-A0`, then `CEL-1-A` (the author), then `CEL-1-C` after `CEL-1-B`.

## Implementation exit

`CEL-1-C` landed: `celestina-style/scripts/verify-production.sh` and
`celestina-rs/scripts/verify-production.sh` both run the token drift check
and the shell reads `celestina-tokens`.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| CEL-1-A0 | `suite:` | active | `scripts/version_contract.py`, `scripts/check-staged-units.py`, `scripts/check-language-contract.py`, their tests, `scripts/language-baseline.tsv`, `docs/contracts/versioning.md`, `docs/governance/change-policy.md`, this plan, `ROADMAP.md`, `STATUS.md` | — | Add the history_owner alias to the version contract, inventory moves under a renamed halted owner, halted trees as history for the language scanner, and rehearse the shell rename | [evidence](../../evidence/2026-10-14-rename-rules.md) | None |
| CEL-1-A | `suite:` | planned | `celestina-qt/` (moved), `celestina/` (new), `docs/projects.toml`, `AGENTS.md`, `docs/version-history.tsv`, the references of the evidence's list | — | Move the halted shell to celestina-qt and register the Rust shell | `docs/evidence/2026-10-14-shell-rename.md` | None |
| CEL-1-C | `suite:` | planned | `celestina-style/tokens.toml`, `celestina-style/scripts/gen-tokens.py`, `celestina-style/CelestinaTokenData.qml`, `celestina-style/CelestinaTheme.qml`, `celestina-rs/crates/celestina-tokens`, both verifies, `docs/projects.toml`, `celestina/crates/celestina-paint/src/theme.rs` | — | Generate the style's tokens from one data file into QML and Rust under a drift check, and read them in the shell | `docs/evidence/2026-10-14-tokens-as-data.md` | None |
```

- [ ] **Step 2: Roadmap and status.** `ROADMAP.md`: `Status: active`, `Active implementation checkpoint: RSH-1`, a section "RSH-1 — Register the Celestina shell in pure Rust" linking the plan. `STATUS.md`: focus and `Implementation checkpoint: RSH-1`. `docs/plans/active/README.md`: name the active plan.

- [ ] **Step 3: Checks.** `bash scripts/check-documentation-contract.sh` → OK (the plan links a `planned` row's evidence by name only; a `planned` row needs no existing record). `sh scripts/test-documentation-contract.sh` → OK.

- [ ] **Step 4: Commit.** `suite-maintenance: Open the RSH-1 plan for the Rust shell registration`. Then the author lands A0 first (`python3 scripts/land-unit.py unit/suite/CEL-1-A0 --kind maintenance`) and A1 after it (`… unit/suite/CEL-1-A1 --kind maintenance`); the plan text above is the one the split left: A0's row `active` on A0's branch, A1's row `active` on A1's.

---

## Unit CEL-1-A — the rename (suite, the author; diff prepared by the agent)

### Task 5: prepare the rename diff on `unit/suite/CEL-1-A`

**Worktree:** `scripts/worktree.sh open suite CEL-1-A --from unit/suite/CEL-1-A1` (stacked until A1 lands; after A1 lands, open from `origin/main`). The agent commits nothing on this branch; it leaves the index staged (`git add -A`) and the evidence written, and stops with `git diff --cached --stat -M` in its report. The author reviews, writes "Halted projects" in their own words, commits in the worktree, and lands with `--kind maintenance`.

**Files (the complete edit list, verified against the checkout on 2026-10-14; the A1 rehearsal corrects it):**

1. **Move:** `git mv celestina celestina-qt`. Bytes unchanged, including `celestina-qt/docs/inventories/**` (63 files), `celestina-qt/docs/plans/active/2026-08-20-persistent-carriers.md` and every document that still says "Celestina" or links `../AGENTS.md`.
2. **`docs/projects.toml`:** the `celestina` table becomes `celestina-qt`:

```toml
[[projects]]
id = "celestina-qt"
name = "Celestina shell (Qt, halted)"
path = "celestina-qt"
kind = "qt-shell"
commit_prefix = "celestina-qt"
# Registered as `celestina` until 2026-10-14; the history rows that carry that
# name up to the Rust shell's baseline are this project's
# (docs/contracts/versioning.md, "Renaming a project").
history_owner = "celestina"
# (keep the existing halted comment) …
halted = "2026-09-27"
version_source = { kind = "cmake-project", path = "celestina-qt/CMakeLists.txt", project = "celestina" }
version_mirrors = [
  { kind = "cargo-package", path = "celestina-qt/Cargo.toml", package = "celestina-shell" },
  { kind = "cargo-lock", path = "celestina-qt/Cargo.lock", package = "celestina-shell" },
]
```

   and every other `celestina/` path of that table becomes `celestina-qt/` (`agents`…`validation`, `active_plans`, `context_documents`, `source_roots`, `commit_roots = ["celestina-qt/", "celestina-rs/crates/celestina-shell-core/"]`, the six scripts, `artifact_manifest`, `artifact_paths`, `cargo_manifests`, `production_inputs`, `verification_inputs`). The `celestina-shell-core` component scope stays. Then, immediately after it, the new table:

```toml
[[projects]]
id = "celestina"
name = "Celestina shell"
path = "celestina"
kind = "rust-shell"
commit_prefix = "celestina"
version_source = { kind = "cargo-package", path = "celestina/Cargo.toml", package = "celestina" }
version_mirrors = [
  { kind = "cargo-lock", path = "celestina/Cargo.lock", package = "celestina" },
]
agents = "celestina/AGENTS.md"
readme = "celestina/README.md"
status = "celestina/STATUS.md"
roadmap = "celestina/ROADMAP.md"
validation = "celestina/VALIDATION.md"
active_plans = "celestina/docs/plans/active"
context_documents = [
  "docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md",
  "celestina-style/DESIGN.md",
  "celestina/docs/decisions/README.md",
]
source_roots = ["celestina/src", "celestina/crates", "celestina-rs/crates/celestina-shell"]
commit_roots = ["celestina/", "celestina-rs/crates/celestina-shell/"]
include_workspace_manifests = true
production_role = "activatable-shell"
deployable = true
toolchains = ["rust"]
build_script = "celestina/scripts/build-production.sh"
complete_script = "celestina/scripts/complete-production.sh"
verify_script = "celestina/scripts/verify-production.sh"
deploy_script = "celestina/scripts/deploy-production.sh"
activate_script = "celestina/scripts/activate-production.sh"
status_script = "celestina/scripts/status-production.sh"
artifact_manifest = "celestina/target/production-artifact.toml"
artifact_paths = ["celestina/target/release/celestina"]
cargo_manifests = ["celestina/Cargo.toml"]
production_inputs = ["celestina/Cargo.toml", "celestina/Cargo.lock", "celestina/rust-toolchain.toml", "celestina/src", "celestina/crates", "celestina/celestina.desktop", "celestina-style/fonts", "celestina-style/icons", "celestina-rs/crates/celestina-core", "celestina-rs/crates/celestina-settings", "celestina-rs/crates/celestina-shell", "celestina-rs/crates/celestina-tokens"]
verification_inputs = ["celestina/scripts/smoke.sh", "celestina/scripts/dev-session.kdl", "celestina-rs/rust-toolchain.toml"]
component_commit_scopes = [
  { prefix = "celestina-shell", roots = ["celestina-rs/crates/celestina-shell/"], include_workspace_manifests = true },
]
```

   In the `celestina-rs` table, append `{ prefix = "celestina-tokens", roots = ["celestina-rs/crates/celestina-tokens/"], include_workspace_manifests = true },` to `component_commit_scopes`.
3. **`docs/version-history.tsv`:** append one row (nothing above it changes; the rows named `celestina` above it stay the halted shell's through the alias):

```text
celestina	0.1.0	baseline	CEL-1-A	Adopt the Rust shell skeleton as its verified baseline
```

4. **`AGENTS.md` "Halted projects" (the author's words; the agent drafts):** the halt covers `celestina-qt/` (moved from `celestina/` on 2026-10-14, id and prefix `celestina-qt`), `celestina-rs/crates/celestina-shell-core`, prefixes `celestina-qt:` and `celestina-shell-core:`, its scripts and production entries; the bullets as today; plus one new paragraph: "**Salvage by copy.** The Rust shell at `celestina/` may copy, never edit, these files of the halted tree and adapt them in its own crates, recording in each copy's head comment the source path and the commit it came from: `celestina-rs/crates/celestina-shell-core/src/{notifications,launcher,coalesce,workspace_groups,workspace_map,settings,calendar}.rs` and the `bounded` helper of that crate's `src/lib.rs` (there is no `bounded.rs`) into `celestina-rs/crates/celestina-shell`; `celestina-qt/src/provider_adapter/{worker,notifications,launcher}.rs` and `celestina-qt/src/niri_adapter.rs` as reference for `celestina/src/providers`; `celestina-qt/scripts/dev-session.kdl` and `dev-session.sh` for `celestina/scripts`. Later deliveries extend this list by the same edit."
5. **References outside the tree** (each `celestina/` → `celestina-qt/` unless noted): `README.md:19` (row becomes the new shell: "Niri shell in pure Rust: Island, launcher, notifications, tray; CEL-1 in progress"; add a row `[celestina-qt](celestina-qt/)` "The Qt/QML shell, **halted** since 2026-09-27 … Rust · C++20 · Qt/QML"); `ROADMAP.md:289` (two rows likewise); `HOST-HYGIENE.md:1084`; `magnetita/ROADMAP.md:89` and `magnetita/AGENTS.md:48` (the shell that consumed `magnetita-core` is halted: say `celestina-qt/scripts/complete-production.sh`, halted); `siderita/STATUS.md:236`; `cuprita/AGENTS.md:32`, `hematita/AGENTS.md:33`, `calcita/AGENTS.md:51`, `selenita/AGENTS.md:41` ("never reference `celestina-qt/` or `celestina-shell-core`"); `docs/history/suite-roadmap-through-2026-08-03.md` (6 links), `docs/plans/archive/2026-08-05-desktop-entry-registration.md` (1), `docs/evidence/2026-09-28-halt-shell.md` (1), `docs/evidence/2026-08-05-static-suite-audit.md` (1): link targets only, no prose change; `Celestina.code-workspace` (`"path": "celestina-qt"` and a new folder `"celestina"`); `scripts/check-architecture-contract.sh:327-328` (`celestina-qt/CMakeLists.txt celestina-qt/qml celestina`: the third argument is the CMake target, which keeps its name); `celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md` (one more link into the tree); `scripts/test-commit-scope.sh` needs no edit (A0 made its fixtures read the shell's identity from the registry); `.github/workflows/celestina.yml` is recreated for the Rust shell after the rename below frees the name: `name: celestina`, paths `celestina/**`, `celestina-rs/crates/celestina-shell/**`, `celestina-rs/crates/celestina-tokens/**`, a job running `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo test --workspace --locked` in `celestina/` on stable; `.github/workflows/celestina.yml` → `git mv` to `celestina-qt.yml`, `name: celestina-qt`, paths `celestina-qt/**`, `working-directory: celestina-qt`, cache paths and key; `scripts/radius-baseline.tsv:5` (`9	celestina` → `9	celestina-qt`; the new shell has no QML and gets no radius, qmllint or glass-canvas row).
6. **The new `celestina/` skeleton** (replaced in B; every file is real, nothing is a placeholder the guards would refuse):
   - `Cargo.toml`: `[workspace] members = ["crates/*", "."]`, `resolver = "2"`, `[workspace.package] edition = "2021", rust-version = "1.89", license = "GPL-3.0-or-later"`, `[workspace.lints.rust] unsafe_code = "forbid"`, `[workspace.lints.clippy] all = { level = "warn", priority = -1 }`; `[package] name = "celestina", version = "0.1.0"`, `[lints] workspace = true`, no dependencies.
   - `Cargo.lock` from `cargo generate-lockfile`; `rust-toolchain.toml` copied from `celestina-rs/rust-toolchain.toml`.
   - `crates/celestina-wayland`, `crates/celestina-paint`, `crates/celestina-ui`: each `Cargo.toml` (`name`, `version = "0.1.0"`, `edition.workspace = true`, `rust-version.workspace = true`, `license.workspace = true`, `[lints] workspace = true`, a `description`) and `src/lib.rs` holding one doc comment naming the spec section it will implement.
   - `src/main.rs`:

```rust
//! The Celestina shell. CEL-1-A registers the project; the shell itself
//! arrives in CEL-1-B. Until then the binary answers `--check` so the
//! production entries have something to verify.
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--check"] {
        return ExitCode::SUCCESS;
    }
    eprintln!("celestina: the shell is not delivered yet (CEL-1-B); only --check is accepted");
    ExitCode::from(2)
}
```

   - `celestina.desktop`: the halted one's text with the English `Comment` and `GenericName` rewritten for the Rust shell, their `[es]` locale-qualified keys translated the same way, `Exec=celestina`, `NoDisplay=true`, `StartupWMClass=celestina`.
   - `scripts/build-production.sh`, `verify-production.sh`, `status-production.sh`, `complete-production.sh`, `deploy-production.sh`: Selenita's five with `selenita` → `celestina`; the deploy entry does not use `production_install_xdg_application` (it requires an app icon and the shell has none): after `production_require_verified "$suite_root" celestina` it runs `production_install_file "$project_root/target/release/celestina" "$prefix/bin/celestina" 0755` and `production_install_file "$project_root/celestina.desktop" "$prefix/share/applications/celestina.desktop" 0644`. `verify-production.sh` runs, in the runner's internal mode: `test-production-artifacts.sh`, `check-architecture-contract.sh`, `cargo fmt --all --check`, `cargo clippy --workspace --release --all-targets --locked -- -D warnings`, `cargo test --workspace --release --locked`, then `scripts/smoke.sh --binary target/release/celestina`. `activate-production.sh`: `production_require_verified`, then refuses with exit 1 and `activate-production: the shell is not deliverable before CEL-1-B` (B replaces the body). `smoke.sh`: `"$bin" --check` must exit 0; prints `smoke: OK`. `dev-session.kdl`: the halted shell's file copied (permission 4), with the `celestina-dense-glass` layer rule removed (no dense companions in this design) and the Celestina `layer-rule` namespaces `^celestina-(panel|overlay|toasts)$`.
   - `AGENTS.md` (from `docs/templates/AGENTS.local.md`: required context = the spec §1–§6, DESIGN.md, landing contract; local boundary = the crate layout of §6.1, `unsafe` only in `crates/celestina-wayland/src/egl.rs`, never edit `celestina-qt/` or `celestina-shell-core`, copy only what the root halt section names; local verification = the five scripts and `smoke.sh`), `README.md` (what it is, the crates, how to run `celestina --check`, the command verbs to come), `STATUS.md` (`0.1.0`, skeleton, focus CEL-1-B), `ROADMAP.md` (`Status: active`, `Active implementation checkpoint: CEL-1`, the §7 table for B–H), `VALIDATION.md` (the §8 live rows as `pending`: `VAL-CEL-GLASS`, `VAL-CEL-ISLAND`, `VAL-CEL-REST`, `VAL-CEL-POPUP`, `VAL-CEL-KEYBOARD`, `VAL-CEL-IME`, `VAL-CEL-SCALE`, `VAL-CEL-NOCTALIA`), `docs/plans/active/2026-10-14-cel-1-island.md` (Plan ID `cel-1-island`, checkpoint `CEL-1`, rows CEL-1-B, D, E, F, G, H `planned`), `docs/plans/active/README.md`, `docs/plans/archive/README.md`, `docs/evidence/README.md`, `docs/decisions/README.md` with ADRs `0001-pure-rust-shell-on-wayland.md` … `0009-one-process-one-loop.md` (the nine decisions of §10, one file each, template `docs/templates/decision.md`, status `accepted`), `docs/discussions/README.md` (empty index).
7. **Workspace stubs in `celestina-rs`:** `crates/celestina-shell/{Cargo.toml,src/lib.rs}` (`name = "celestina-shell"`, dependency `celestina-core.workspace = true` with its justification comment, one doc comment) and `crates/celestina-tokens/{Cargo.toml,src/lib.rs}` (no dependencies; doc comment "generated by celestina-style/scripts/gen-tokens.py from CEL-1-C on"); both added to `members` in `celestina-rs/Cargo.toml`; `cargo check -p celestina-shell -p celestina-tokens --locked` (run once without `--locked` if the lock needs the two entries).
8. **Evidence:** `docs/evidence/2026-10-14-shell-rename.md` with the guard commands of Task 3 and their exits on this branch.

- [ ] **Step 1:** apply items 1–8 exactly; `git add -A`.
- [ ] **Step 2:** run the Task 3 command list from the worktree; every exit 0. In addition `cd celestina && cargo build --release --locked && ./target/release/celestina --check; echo $?` → `0`. The landing tool has no dry-run flag, so the landing evidence the spec's §9 asks for is: (a) A0's rehearsal record, (b) the hook chain (`pre-commit`, `commit-msg`) that the author's own commit in the worktree runs over this exact index, and (c) `python3 scripts/test-land-unit.py` on the branch; the evidence record cites the three.
- [ ] **Step 3:** report to the author: `git diff --cached --stat -M | tail -3`, the path of the evidence, and the four decisions to confirm (deploy path, the `history_owner` comment, the salvage list, the retargeted historical links). Stop. The author commits (`suite-maintenance: Move the halted shell to celestina-qt and register the Rust shell`) and lands with `--kind maintenance`. The landing builds and verifies `celestina` (its production inputs are new) and, after the push, deploys the stub binary to `$prefix/bin/celestina`.

---

## Unit CEL-1-B — skeleton: the three crates, the Island capsule, the command surface (celestina, milestone)

Worktree: `scripts/worktree.sh open celestina CEL-1-B` (after A lands). Ledger row CEL-1-B `active` in `celestina/docs/plans/active/2026-10-14-cel-1-island.md`; evidence `celestina/docs/evidence/2026-10-14-skeleton.md`.

### Task 6: `celestina-wayland` — the runtime

**Files:**
- Create: `celestina/crates/celestina-wayland/src/{lib.rs, runtime.rs, surface.rs, egl.rs, blur.rs, popup.rs, handlers.rs, input.rs, geometry.rs}`
- Modify: `celestina/crates/celestina-wayland/Cargo.toml` (dependencies: smithay-client-toolkit 0.21 with features `calloop` and `xkbcommon`, wayland-client 0.31, wayland-protocols 0.32 `client`+`staging`, wayland-egl 0.32, khronos-egl 6 `dynamic`, glow 0.18, femtovg 0.27 (only for `egl::femtovg_renderer`, the one `unsafe` constructor), calloop 0.14 `signals`, calloop-wayland-source 0.4, tracing, thiserror 2; each with a one-line justification)
- Test: unit tests inside `surface.rs`, `blur.rs`, `popup.rs`, `geometry.rs`, `runtime.rs`

**Start from:** spike `src/wayland/{mod,surface,egl,blur,popup,handlers}.rs` (commit `e660c9b` of `/home/toni/CODIGO/isla-spike`), kept in shape and changed as listed.

**Interfaces (produces):**

```rust
// geometry.rs
pub struct Rect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }           // logical px
impl Rect { pub fn contains(&self, x: f32, y: f32) -> bool; pub fn whole(size: (f32, f32)) -> Rect }
pub struct PixelRect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }
pub fn to_pixel_rects(rects: &[Rect]) -> Vec<PixelRect>                     // outward rounding, as the spike

// input.rs
pub enum InputEvent {
    PointerMotion { x: f32, y: f32 }, PointerPress { x: f32, y: f32, button: u32, serial: u32 },
    PointerRelease { x: f32, y: f32, button: u32, serial: u32 }, PointerLeave,
    Axis { x: f32, y: f32, horizontal: f32, vertical: f32 },                  // logical px per event; G uses it
    Key { keysym: u32, utf8: Option<String>, modifiers: Modifiers, pressed: bool, serial: u32 },
    FocusEnter, FocusLost,
    PopupClosed { id: PopupId, reason: CloseReason },
}
pub struct Modifiers { pub ctrl: bool, pub alt: bool, pub shift: bool, pub logo: bool }
pub enum CursorShape { Default, Pointer, Text }

// runtime.rs
pub struct Config { pub exit_after: Option<Duration>, pub no_blur: bool, pub metrics: bool }
pub struct OutputInfo { pub id: OutputId, pub name: String, pub logical_size: (u32, u32), pub scale: f64, pub refresh_mhz: i32 }
pub enum OutputEvent { Added(OutputInfo), Changed(OutputInfo), Removed(OutputId) }
pub enum Layer { Top, Overlay }
pub enum KeyboardInteractivity { None, OnDemand, Exclusive }
pub struct LayerSpec { pub output: OutputId, pub namespace: &'static str, pub layer: Layer, pub anchor_top: bool,
                       pub size: (u32, u32), pub exclusive_zone: i32, pub keyboard: KeyboardInteractivity }
pub struct Placement { pub anchor: Rect, pub anchor_edge: AnchorEdge, pub gravity: Gravity, pub offset: (i32, i32) }
pub enum CloseReason { Shell, Parent, Dismissed, Replaced, ParentGone, Failed, Exit }

pub trait Shell: 'static {
    type Message: Send + 'static;
    fn output(&mut self, ev: OutputEvent, ctl: &mut RuntimeCtl<'_>);
    fn frame(&mut self, cx: &mut FrameCx<'_>);
    fn input(&mut self, ev: InputEvent, ctl: &mut SurfaceCtl<'_>);
    fn message(&mut self, msg: Self::Message, ctl: &mut RuntimeCtl<'_>);
}
pub trait PopupShell: 'static {
    fn frame(&mut self, cx: &mut FrameCx<'_>);
    fn input(&mut self, ev: InputEvent, ctl: &mut SurfaceCtl<'_>) -> PopupAction;
    fn dismissed(&mut self);
}
pub struct FrameCx<'a> { pub surface: SurfaceId, pub output: &'a str, pub size: (f32, f32), pub scale: f64,
                         pub now: Instant, pub proc_address: &'a dyn Fn(&str) -> *const c_void,
                         pub blur_available: bool, pub ctl: SurfaceCtl<'a> }
impl SurfaceCtl<'_> {
    pub fn id(&self) -> SurfaceId; pub fn output_name(&self) -> &str;
    pub fn request_redraw(&mut self); pub fn redraw_after(&mut self, d: Duration);   // one timer per surface, re-armed
    pub fn set_blur_region(&mut self, rects: &[Rect]); pub fn set_input_region(&mut self, rects: &[Rect]);
    pub fn set_keyboard_interactivity(&mut self, k: KeyboardInteractivity); pub fn set_exclusive_zone(&mut self, zone: i32);
    pub fn set_cursor(&mut self, shape: CursorShape);
    pub fn open_popup(&mut self, placement: Placement, size: (f32, f32), shell: Box<dyn PopupShell>) -> PopupId;
    pub fn close_popup(&mut self, id: PopupId);
}
impl RuntimeCtl<'_> {
    pub fn create_layer_surface(&mut self, spec: LayerSpec) -> Result<SurfaceId, RuntimeError>;
    pub fn destroy_surface(&mut self, id: SurfaceId);
    pub fn surface(&mut self, id: SurfaceId) -> Option<SurfaceCtl<'_>>;
    pub fn surfaces(&self) -> Vec<SurfaceId>;
    pub fn stop(&mut self);
}
pub struct Sender<M>(calloop::channel::Sender<M>);  // Clone + Send: workers publish messages
pub fn run<S: Shell>(cfg: Config, shell: S) -> Result<RunReport, RuntimeError>;  // RunReport: metrics summary text when cfg.metrics
pub fn sender_for<S: Shell>(/* obtained from run's setup closure */)              // see Step 3
pub mod egl { pub fn femtovg_renderer(proc_address: &dyn Fn(&str) -> *const c_void) -> Result<femtovg::renderer::OpenGl, EglError>; }
```

- [ ] **Step 1: Pure tests first.** Port the spike's tests verbatim into the new modules (`buffer_size_*`, `preferred_scale_is_in_120ths`, `pacing_*` in `surface.rs`; `*_rounds_outwards`, `cache_*` in `blur.rs`; `positioner_geometry` tests and `menu_hangs_from_the_buttons_left_edge_with_a_small_gap` in `popup.rs`), and add: `redraw_after_rearms_one_timer_per_surface` (a `RedrawTimer` state machine: `arm(deadline)` twice keeps the earlier deadline; `fired()` clears), `regions_apply_only_with_the_next_frame` (a `PendingRegions` struct: `set_blur`, `set_input` store; `take_for_frame()` returns both once; nothing before). `cargo test -p celestina-wayland` → FAIL (modules missing).

- [ ] **Step 2: Port the runtime** from the spike with these changes, each closing a ledger finding or a spec line: (a) `Shell::Message` generic instead of `Box<dyn Any>`; `run` registers one `calloop::channel` and exposes its `Sender<M>` through `run(cfg, shell, setup: impl FnOnce(Sender<M>, &LoopHandle))` so the product starts its workers with a sender; (b) outputs: `OutputHandler::new_output/update_output/output_destroyed` become `Shell::output` calls with `OutputInfo` (name, logical size from the current mode and scale, refresh); the runtime no longer opens surfaces itself: the product calls `RuntimeCtl::create_layer_surface`; (c) `LayerSpec.layer`, `anchor_top`, `exclusive_zone`, `keyboard` honoured at creation; `set_exclusive_zone` later; (d) `KeyboardInteractivity::Exclusive` supported; (e) regions pending until the frame's swap (`PendingRegions` applied in `draw` before `swap_buffers`), `request_redraw` outside a frame still commits only a frame request; (f) `redraw_after` coalesced per surface (`RedrawTimer` keeps one calloop timer token); (g) key events carry `Modifiers` from `update_modifiers` and the serial; pointer press and key press update `last_input_serial`; (h) `Axis` events delivered (ledger T4 deferred: "redraw on axis"); (i) `wp_cursor_shape_v1` bound when present, `set_cursor` no-op otherwise; (j) `blur_available` on `FrameCx` (false without `ext_background_effect_v1`); (k) `Placement` with `AnchorEdge`/`Gravity` instead of the fixed `menu_placement`; (l) typed errors (`RuntimeError`, `EglError`) with `thiserror`, no `anyhow`; (m) output removal destroys that output's popups then its surfaces and reports `OutputEvent::Removed`; (n) metrics module from spike `src/metrics.rs` moved here as `metrics.rs` (CPU percentiles, `n=`, compositor `done` intervals, RSS), enabled by `Config.metrics`; (o) signals and `exit_after` as the spike; (p) `#![deny(unsafe_code)]` at the crate root, `#![forbid(unsafe_code)]` on every module except `egl.rs` (`#![allow(unsafe_code)]`, each `unsafe` with its `SAFETY:` invariant as the spike has them).

- [ ] **Step 3: Review-focus test (item 1).** In `runtime.rs` tests, a pure `Targets` table (surfaces and popups by output) with `remove_output(id)` returning the destroy order: popups of that output first, then its layer surfaces, other outputs untouched: `fn removing_an_output_destroys_its_popups_before_its_surfaces_and_nothing_else()`.

- [ ] **Step 4:** `cargo test -p celestina-wayland` → PASS; `cargo clippy -p celestina-wayland --all-targets --locked -- -D warnings` → clean. Commit `celestina-maintenance: Add the celestina-wayland runtime from the spike with typed messages, outputs and popups`.

### Task 7: `celestina-paint` — Painter, Haze, text atlas, icons

**Files:**
- Create: `celestina/crates/celestina-paint/src/{lib.rs, painter.rs, backend.rs (private), haze.rs, text.rs, atlas.rs, icon.rs, theme.rs, color.rs}`
- Modify: `celestina/crates/celestina-paint/Cargo.toml` (celestina-wayland path, femtovg 0.27, cosmic-text 0.19, swash through cosmic-text's re-export, resvg 0.48, freedesktop-icons 0.4, thiserror, tracing)
- Test: unit tests in `haze.rs`, `atlas.rs`, `text.rs`, `theme.rs`, `color.rs`; `tests/painter_contract.rs` (compiles against the public API without GL)

**Start from:** spike `src/paint/{mod,haze,text,icon,tokens}.rs`.

**Interfaces (produces):**

```rust
pub use celestina_wayland::Rect;
pub struct Color(pub u8, pub u8, pub u8, pub f32);
impl Color { pub fn hex(rgb: &str) -> Result<Color, ColorError>; pub fn with_alpha(self, a: f32) -> Color }
pub enum Role { Canvas, Card, TextHi, TextLo, TextFaint, Accent, AccentInk, Divider, ControlFill, ControlHover, ControlPressed,
                FocusRing, GlassTint, GlassFallback, Danger }
pub struct Theme { /* one Color per Role, plus: */ pub grain_alpha: f32, pub radius_lg: f32, pub radius_md: f32, pub radius_sm: f32,
                   pub radius_button: f32, pub row_height: f32, pub top_bar: f32, pub spacing: Spacing, pub type_scale: TypeScale,
                   pub motion: MotionMs, pub press_scale: f32 }
impl Theme { pub fn dark() -> Theme /* CEL-1-B: the spike's tokens.rs plus DESIGN.md §2/§5; CEL-1-C replaces it by From<&celestina_tokens::Scheme> */;
             pub fn color(&self, role: Role) -> Color; pub fn scaled(&self, text_scale: f64) -> Theme }
pub enum Weight { Regular, Semibold }   // 400 / 600
pub struct TextStyle { pub size: f32, pub weight: Weight, pub tabular: bool, pub role: Role }
pub struct TextRun { /* laid out once */ } impl TextRun { pub fn size(&self) -> (f32, f32); pub fn text(&self) -> &str }
pub struct TextSystem; impl TextSystem {
    pub fn new(font_dirs: &[PathBuf]) -> Result<TextSystem, TextError>;      // Inter Variable from celestina-style/fonts + system fallback
    pub fn layout(&mut self, text: &str, style: &TextStyle, max_width: Option<f32>, dpr: f32) -> TextRun;
    pub fn measure(&mut self, text: &str, style: &TextStyle, max_width: Option<f32>, dpr: f32) -> (f32, f32);
}
pub enum IconKey { Semantic(&'static str) /* celestina-style/icons/<name>.svg */, Theme(String) /* freedesktop-icons lookup */, Pixmap(PixmapId) }
pub struct Painter; impl Painter {
    pub fn new(proc_address: &dyn Fn(&str) -> *const c_void, theme: Theme, fonts: TextSystem) -> Result<Painter, PaintError>;
    pub fn begin(&mut self, logical: (f32, f32), dpr: f32, blur_available: bool); pub fn end(&mut self);
    pub fn theme(&self) -> &Theme; pub fn set_theme(&mut self, theme: Theme); pub fn text(&mut self) -> &mut TextSystem;
    pub fn fill_rounded(&mut self, rect: Rect, radius: f32, color: Color);
    pub fn stroke_rounded(&mut self, rect: Rect, radius: f32, width: f32, color: Color);
    pub fn haze(&mut self, rect: Rect, radius: f32);                          // glassTint + grain, or glassFallback without blur
    pub fn glyphs(&mut self, run: &TextRun, x: f32, y: f32, color: Color);
    pub fn icon(&mut self, key: &IconKey, rect: Rect, tint: Option<Color>);
    pub fn image(&mut self, id: PixmapId, rect: Rect, alpha: f32);
    pub fn upload_pixmap(&mut self, argb: &[u8], width: u32, height: u32) -> Result<PixmapId, PaintError>;
    pub fn push_clip(&mut self, rect: Rect, radius: f32); pub fn pop_clip(&mut self);
    pub fn push_state(&mut self); pub fn pop_state(&mut self); pub fn opacity(&mut self, multiply: f32);
    pub fn scale_about(&mut self, cx: f32, cy: f32, s: f32);
}
pub fn silhouette_slices(rect: Rect, radius: f32, step: f32) -> Vec<Rect>;
```

- [ ] **Step 1: Failing tests.** Port the spike's `silhouette_slices` tests, `grain_is_deterministic_noise`, `opacity_multiplies_and_is_scoped`, `color_converts_to_femtovg_keeping_alpha`; add in `atlas.rs`: `glyph_key_includes_font_size_in_device_px_and_subpixel_quarter` (keys for size 14 at dpr 1.5 and 1.0 differ; x offsets 0.1 and 0.3 share a quarter, 0.1 and 0.4 do not), `atlas_evicts_least_recent_when_a_page_is_full` (a 1024×1024 page filled with 64×64 cells: the 257th insert evicts the least recently used), `atlas_never_exceeds_one_page` (bounded, §9 risk); in `theme.rs`: `dark_matches_design_md` (`card == #0b0c10`, `glass_tint == card at 0.70`, `grain_alpha == 0.15`, accent `#3e91ff`, radii 20/12/8/10, motion 100/200/350/150, press 0.96), `scaled_multiplies_only_the_type_ladder` (text_scale 1.3 changes `type_scale` sizes by `round(base × 1.3)` and nothing else); in `text.rs`: `tabular_style_sets_tnum_and_semibold_sets_wght_600` (the cosmic-text `Attrs` built from a `TextStyle`); in `haze.rs`: `haze_without_blur_paints_the_opaque_fallback` (a pure `HazePlan::for_(blur_available) -> (Color, Option<f32>)` returns `(glass_tint, Some(grain))` with blur and `(glass_fallback, None)` without, Review Focus 4). `cargo test -p celestina-paint` → FAIL.

- [ ] **Step 2: Implement.** `backend.rs` wraps `femtovg::Canvas<OpenGl>` (private; the only module importing femtovg types besides `egl::femtovg_renderer`'s result); `painter.rs` as the spike plus `set_theme`, `glyphs`, `image`, `upload_pixmap`, `blur_available` in `begin`; `haze.rs`: tint and grain at 0.15 under the tint (draw the grain first, then the tint, as DESIGN.md §5.3 orders) and the fallback path; `text.rs`+`atlas.rs`: `cosmic_text::FontSystem` with `fontdb` loading `celestina-style/fonts/InterVariable.ttf` (path from `CELESTINA_FONTS` env or the compiled-in relative path of the checkout, then `$prefix/share/celestina/fonts` once deployed: `deploy-production.sh` installs the font there in this unit) and the system fonts; `SwashCache`; one RGBA atlas page 1024×1024 with a shelf packer and an LRU map keyed `(font id, glyph id, size_px: u32, subpixel_quarter: u8)`; `TextRun` holds `cosmic_text::Buffer` and its glyph quads; `measure` and `layout` share one buffer per call so taffy measures what is painted; `icon.rs`: `Semantic` from `celestina-style/icons` by name (same lookup path rule as fonts), `Theme` through `freedesktop_icons::lookup(name).with_size(size).find()` rasterised with resvg at the device size, SVG and PNG both; cache by `(key, device size)`; `theme.rs`: `Theme::dark()` from the spike's `tokens.rs` extended with every role above and DESIGN.md's values.

- [ ] **Step 3:** `cargo test -p celestina-paint` → PASS; clippy clean; commit `celestina-maintenance: Add celestina-paint with the Haze material, the cosmic-text atlas and icons`.

### Task 8: `celestina-ui` — keyed tree, layout, animation, interaction

**Files:**
- Create: `celestina/crates/celestina-ui/src/{lib.rs, curve.rs, anim.rs, key.rs, view.rs, tree.rs, reconcile.rs, layout.rs, paint.rs, input.rs, highlight.rs}`
- Modify: `celestina/crates/celestina-ui/Cargo.toml` (celestina-paint, celestina-wayland, taffy 0.15 default-features false + `flexbox`, `block_layout`, `std`; thiserror)
- Test: unit tests per module; `tests/reconcile.rs`

**Start from:** spike `src/ui/{curve,anim,node,input}.rs`, split along the seams the ledger named (paint walk → `paint.rs`, interaction → `input.rs`, highlight → `highlight.rs`).

**Interfaces (produces):**

```rust
pub struct CubicBezier(pub f32, pub f32, pub f32, pub f32);  pub const ONE_UI, SINE_IN_OUT_80, SINE_IN_OUT_90, EXIT, LINEAR;
pub enum Motion { Fast, Normal, Slow, Exit }                  // 100/200/350/150 ms from the Theme's MotionMs
pub struct Animated { … } impl Animated { pub fn new(v: f32); pub fn set(&mut self, v: f32);
    pub fn animate_to(&mut self, v: f32, motion: Motion, curve: CubicBezier, now: Instant, reduced: Reduced);
    pub fn value(&self, now: Instant) -> f32; pub fn target(&self) -> f32; pub fn is_animating(&self, now: Instant) -> bool }
pub enum Reduced { Spatial /* becomes a step */, Opacity /* keeps fading */ }   // chosen by the caller per property
pub struct Key(u64); impl Key { pub fn named(s: &str) -> Key; pub fn indexed(s: &str, i: u64) -> Key }
pub enum ViewKind { Box { fill: Option<Role>, haze: bool, radius: f32 }, Text { text: String, style: TextStyle, max_width: Option<f32> },
                    Icon { key: IconKey, tint: Option<Role>, size: f32 }, Image { id: PixmapId, alpha: f32 } }
pub struct View { pub key: Key, pub kind: ViewKind, pub style: taffy::Style, pub interactive: bool, pub focusable: bool,
                  pub clip: bool, pub opacity: f32, pub children: Vec<View> }
pub mod view { pub fn col(key: Key) -> View; pub fn row(key: Key) -> View; pub fn text(key: Key, s: &str, style: TextStyle) -> View;
               pub fn icon(key: Key, name: &'static str, size: f32) -> View; /* builder methods: .fill(Role) .haze(radius) .size(w,h)
               .padding(..) .gap(..) .grow() .interactive() .focusable() .clip() .opacity(f32) .child(View) .absolute(x,y) .hidden(bool) */ }
pub type NodeId = usize;
pub struct Tree; impl Tree {
    pub fn new(reduced_motion: bool) -> Tree; pub fn set_reduced_motion(&mut self, on: bool);
    pub fn apply(&mut self, root: View, now: Instant);                      // keyed reconciliation
    pub fn layout(&mut self, available: (f32, f32), text: &mut TextSystem, dpr: f32);
    pub fn layout_measured(&mut self, available: (f32, f32), measure: impl FnMut(&str, &TextStyle, Option<f32>) -> (f32, f32));
    pub fn paint(&mut self, painter: &mut Painter, now: Instant);
    pub fn paint_order(&self) -> Vec<NodeId>; pub fn rect(&self, id: NodeId) -> Rect; pub fn key_of(&self, id: NodeId) -> Key;
    pub fn node_by_key(&self, key: Key) -> Option<NodeId>; pub fn is_animating(&self, now: Instant) -> bool;
    pub fn highlight(&self) -> Highlight; pub fn effective_opacity(&self, id: NodeId, now: Instant) -> f32;
}
pub struct Highlight { pub hovered: Option<NodeId>, pub pressed: Option<NodeId>, pub focused: Option<NodeId> }
pub enum UiEvent { Click(Key), HoverChanged(Key, bool), Activate(Key), Escape, Scroll(Key, f32, f32) }
pub struct Interaction; impl Interaction { pub fn new() -> Interaction;
    pub fn pointer(&mut self, tree: &mut Tree, ev: &InputEvent, now: Instant) -> Vec<UiEvent>;
    pub fn key(&mut self, tree: &mut Tree, ev: &InputEvent) -> Vec<UiEvent>;   // Tab order, Enter/Space, Escape → UiEvent::Escape
}
```

- [ ] **Step 1: Failing tests.** Port the spike's curve, anim, node and input tests under the new names (keys instead of ids where the spike used ids); add in `tests/reconcile.rs`: `apply_keeps_hover_focus_and_running_animations_on_surviving_keys` (hover row `indexed("row", 2)`, re-apply with rows reordered: the same key stays hovered and its recoil animation continues), `apply_animates_a_moved_node_on_the_default_curve` (a row that moves from y 60 to 100 has an `offset` animation whose value at t=0 is −40 and at 200 ms is 0), `apply_fades_removed_and_inserted_nodes` (a removed key stays in the tree with opacity → 0 over `Exit` and is dropped when settled; an inserted key starts at 0 → 1 over `Normal`), `apply_updates_text_in_place_without_relayout_when_unchanged`, `reduced_motion_steps_spatial_and_keeps_opacity_fades` (`Reduced::Spatial` under reduced motion: `value(now) == target` at once; `Reduced::Opacity`: still animating), `hit_test_respects_clip_and_opacity_zero`, `tab_order_skips_hidden_and_unfocusable`, `escape_bubbles_as_an_event`. `cargo test -p celestina-ui` → FAIL.

- [ ] **Step 2: Implement.** `tree.rs` keeps the slab, taffy twins and rects; `reconcile.rs` matches children by key (in order, with a key → slot map per parent), updates kind/style in place (`set_text` only on change, `set_style` only on change), records the previous rect per node so `layout` can start an `offset` animation when a rect moves, inserts with opacity 0 → 1 (`Normal`, `LINEAR`), removes by marking `leaving` with opacity → 0 (`Exit`), pruned after settling; `anim.rs` adds `Reduced`; `paint.rs` is the spike's walk with `offset` added to the transform and `Image` kind; `input.rs` returns keys, maps `Axis` to `Scroll` on the topmost interactive node, and emits `Escape` on the Escape key (the surface decides what closes); `highlight.rs` holds `Highlight` and the hover/pressed fill mapping (`ControlHover`/`ControlPressed` for interactive boxes; focus ring stroked in `FocusRing`, 2 px, 2 px inside).

- [ ] **Step 3:** `cargo test -p celestina-ui` → PASS; clippy clean; commit `celestina-maintenance: Add celestina-ui with keyed reconciliation, layout, animation and interaction`.

### Task 9: the pure domain crate and the providers — niri stream, settings

**Files:**
- Create: `celestina-rs/crates/celestina-shell/src/{lib.rs, bounded.rs, coalesce.rs, workspace_groups.rs, workspace_map.rs, calendar.rs, settings.rs, launcher.rs, notifications.rs}` (copies, adapted) and `celestina-rs/crates/celestina-shell/tests/salvage.rs`
- Create: `celestina/src/providers/{mod.rs, worker.rs, niri.rs, settings.rs}`, `celestina/src/message.rs`
- Modify: `celestina-rs/crates/celestina-shell/Cargo.toml` (celestina-core; `serde` + `serde_json` only because `settings.rs` and `workspace_groups.rs` persist JSON — keep, justified), `celestina/Cargo.toml` (niri-ipc = 26.4.0, serde_json, celestina-core, celestina-settings, celestina-shell path deps)

**Interfaces (produces):**

```rust
// celestina-shell (re-exports the salvaged modules; each file head: "Copied from celestina-qt/… at <commit>, adapted: …")
pub mod notifications; pub mod launcher; pub mod coalesce; pub mod workspace_groups; pub mod workspace_map; pub mod settings; pub mod calendar; pub mod bounded;
// adaptations: no `snapshot::Payload`, no `ProviderId`; `settings::Settings` keeps only `do_not_disturb: bool` and `schema_version` (the
// tray and wallpaper fields of the old shell are dropped; `SCHEMA_VERSION = 1` for this crate's own file `~/.config/celestina/shell.json`)

// celestina/src/providers/worker.rs — the salvaged guard
pub struct Worker; impl Worker { pub fn spawn(what: &'static str, shutdown: &Arc<AtomicBool>, body: impl FnOnce() + Send + 'static) -> io::Result<Worker>; pub fn join(self) }
// celestina/src/providers/niri.rs
pub struct Workspace { pub id: u64, pub idx: u8, pub name: Option<String>, pub output: Option<String>, pub is_active: bool, pub is_focused: bool, pub columns: u16 }
pub struct Window { pub id: u64, pub title: Option<String>, pub app_id: Option<String>, pub workspace_id: Option<u64>, pub is_focused: bool }
pub struct NiriModel { pub workspaces: Vec<Workspace>, pub windows: Vec<Window>, pub outputs: Vec<String> }
pub enum NiriRequest { FocusWorkspace(u64), FocusWindow(u64) }
pub struct NiriProvider { pub requests: SyncSender<NiriRequest> /* bound 32 */ }
pub fn spawn(sender: Sender<Message>, shutdown: &Arc<AtomicBool>) -> io::Result<(NiriProvider, Worker, Worker)>; // stream worker + request worker
pub fn parse_event(line: &str) -> Result<Option<niri_ipc::Event>, ParseError>;      // None = unknown variant (logged once per name by the caller)
pub fn reduce(state: &niri_ipc::state::EventStreamState) -> NiriModel;
// celestina/src/message.rs
pub enum Message { Niri(NiriModel), Appearance(celestina_settings::Appearance), Command(Verb), NiriRequestRefused(NiriRequest),
                   /* F */ Notifications(NotificationsEvent), /* G */ Tray(TrayEvent), /* E */ DesktopIndex(Vec<DesktopEntry>) }
```

- [ ] **Step 1: Salvage tests first.** Copy the seven modules and their `#[cfg(test)]` tests from `celestina-rs/crates/celestina-shell-core/src/` (read-only source; the commit hash from `git log -1 --format=%H -- celestina-rs/crates/celestina-shell-core`), fix imports (`crate::bounded`, `MAX_TEXT_UNITS` becomes `bounded::MAX_TEXT_UNITS = 4096` copied from `snapshot.rs`'s constant), delete the `Payload` conversions, and in `tests/salvage.rs` add `settings_round_trips_only_do_not_disturb` and `notifications_history_cap_is_fifty_and_the_page_shows_twenty` (constant `PAGE_HISTORY: usize = 20` in the crate). `cd celestina-rs && cargo test -p celestina-shell` → FAIL until adapted, then PASS. `scripts/check-language-contract.py` may list the copied `launcher.rs` (11 legacy rows in the old file): fix the prose in the copy (the copy is new code, English only) rather than adding a baseline row.

- [ ] **Step 2: niri tests.** In `providers/niri.rs`: the spike's `unknown_events_are_skipped_not_fatal`, `reduce_sorts_by_output_then_index_and_keeps_flags_and_names`, `workspace_activated_moves_the_active_flag_on_its_output_only`, plus `reduce_lists_windows_with_their_workspace_and_focus`, `a_truncated_line_is_a_protocol_error_not_a_skip` (fixture: half a JSON object), `unknown_variant_name_is_taken_from_the_outer_key` (`{"MinimizedWindowsChanged":…}` → `Some("MinimizedWindowsChanged")` for the once-per-name log), `request_queue_refuses_the_33rd_pending_request` (a `sync_channel(32)` filled: `try_send` returns `Full`, reported as `Message::NiriRequestRefused`). Review Focus 3: `reconnect_backoff_logs_once_per_state_change` (a pure `ConnectionLog` that returns `true` for the first failure after a success and `false` for repeats). → FAIL, then implement the stream worker (the spike's `session`, extended with windows and outputs from `EventStreamState`), the request worker (short-lived `Socket::connect()` per request, typed `Request::Action`), and `spawn`.

- [ ] **Step 3: settings provider.** `providers/settings.rs`: `celestina_settings::follow(move |appearance| sender.send(Message::Appearance(appearance)))` kept as a `Follower` in `App`; the shell's own `Settings` (do-not-disturb) read at start from `celestina_core::xdg::config_home()/celestina/shell.json` through `celestina_shell::settings::Store`, written atomically with `celestina_core::atomic_file`.

- [ ] **Step 4:** both workspaces' tests pass; clippy clean in both; commit `celestina-maintenance: Salvage the pure shell domain by copy and add the niri and settings providers`.

### Task 10: the Island capsule, the app, the command surface, single instance

**Files:**
- Create: `celestina/src/{main.rs (replaced), app.rs, copy.rs, command.rs, bus.rs, metrics.rs (re-export), island/mod.rs, island/state.rs, island/view.rs, island/clock.rs, island/workspaces.rs}`
- Modify: `celestina/Cargo.toml` (zbus 5 `blocking-api`, tracing-subscriber `env-filter`, the three crates)
- Test: unit tests in `island/state.rs`, `island/clock.rs`, `island/workspaces.rs`, `command.rs`, `app.rs`; `tests/cli.rs`

**Start from:** spike `src/island/{mod,state,workspaces}.rs`, `src/main.rs`.

**Interfaces (produces):**

```rust
// command.rs
pub enum Page { Activity, Notifications, Tray }
pub enum Verb { IslandToggle, IslandOpen(Page), IslandClose, LauncherToggle, NotificationsToggle, NotificationsDismissAll, Quit }
impl Verb { pub fn parse(args: &[String]) -> Result<Verb, UsageError>; pub fn words(&self) -> Vec<&'static str> }
pub enum Mode { Shell(Config), Client(Verb), Check, Usage }     pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Mode, UsageError>
// bus.rs — org.celestina.Shell, /org/celestina/Shell, org.celestina.Shell1
pub fn claim_and_serve(sender: Sender<Message>, shutdown: &Arc<AtomicBool>) -> Result<Worker, BusError>;   // DoNotQueue; NameTaken → BusError::Running
pub fn send_verb(verb: &Verb) -> Result<(), BusError>;                                                      // 3 s timeout; method Command(as) -> (b, s)
// island/state.rs — the spike's IslandState plus pages
pub const CAPSULE_H: f32 = 40.0; pub const TOP_GAP: f32 = 8.0; pub const PAGE_SIZE: (f32, f32) = (420.0, 300.0);
pub struct IslandState; impl IslandState { pub fn new(capsule_w: f32) -> Self; pub fn set_capsule_w(&mut self, w: f32);
    pub fn open(&mut self, page: Page, now: Instant, reduced: bool); pub fn close(&mut self, now: Instant, reduced: bool); pub fn toggle(..);
    pub fn is_open(&self) -> bool; pub fn page(&self) -> Option<Page>; pub fn rect(&self, now: Instant, surface_w: f32) -> Rect;
    pub fn capsule_opacity(&self, now) -> f32; pub fn page_opacity(&self, now) -> f32; pub fn wants_frame(&self, now) -> bool;
    pub fn silhouette(&self, now: Instant, surface_w: f32, radius: f32) -> Vec<Rect>; pub fn exclusive_zone(&self) -> i32 /* CAPSULE_H + TOP_GAP */ }
// island/mod.rs
pub struct Island; impl Island { pub fn new(output: &str, size: (f32, f32)) -> Island;
    pub fn frame(&mut self, cx: &mut FrameCx<'_>, painter: &mut Painter, model: Option<&NiriModel>, reduced: bool);
    pub fn input(&mut self, ev: InputEvent, ctl: &mut SurfaceCtl<'_>, painter: &mut Painter) -> Vec<IslandAction>;
    pub fn command(&mut self, verb: &Verb, ctl: &mut SurfaceCtl<'_>) }
pub enum IslandAction { FocusWorkspace(u64), FocusWindow(u64), OpenMenu(Rect) /* D */ }
// app.rs: `struct App` implements celestina_wayland::Shell<Message = Message>: one Island per output on a `celestina-panel`
// Top layer surface anchored top, size (760, 560) logical at first (the largest page plus margin), exclusive zone CAPSULE_H + TOP_GAP.
```

- [ ] **Step 1: Failing tests.** Port the spike's `state.rs` tests (same assertions, `open(Page::Activity, …)`), the clock tests (`minute_boundary_is_computed_from_the_wall_clock`, `clock_tick_rearms_at_or_after_its_deadline…`, `clock_formats_hours_and_minutes_at_an_offset`, `utc_offset_parses_date_z_output`) into `island/clock.rs`, `workspaces.rs` tests (`rows_keep_only_the_islands_output…`), and add: `exclusive_zone_is_the_capsule_height_plus_its_margin` (48), `silhouette_follows_the_rect_every_frame` (slices of the rect at t=0, t=120 ms, t=350 ms cover the rect and nothing beyond 2 px), `reduced_motion_opens_in_one_step`; in `command.rs`: `every_verb_round_trips_through_words_and_parse` (table of the seven verbs), `unknown_verbs_are_usage_errors` (`celestina island fly`, `celestina island open menu`, `celestina`+200 args), `shell_mode_flags` (`--exit-after 3`, `--metrics`, `--no-blur`, `--check`); in `tests/cli.rs` (runs the binary): `check_exits_zero_without_a_compositor` (`celestina --check` with `WAYLAND_DISPLAY` unset → 0), `an_unknown_verb_exits_2_with_usage_on_stderr`. → FAIL.

- [ ] **Step 2: Implement.** `main.rs`: tracing to stderr (`RUST_LOG`, default `info`), `parse_args`; `Mode::Client` → `bus::send_verb` and exit 0/1; `Mode::Check` → load `Theme::dark()`, `TextSystem::new`, `celestina_settings::load()`, exit 0; `Mode::Shell` → `bus::claim_and_serve` (on `BusError::Running` print `celestina: the shell is running` and exit 0), start providers, `celestina_wayland::run(cfg, App::new(..), setup)`, on exit join workers (bounded 2 s each), print the metrics report when `--metrics`. `app.rs`: outputs → Islands (`OutputEvent::Added` creates the layer surface and the `Island`; `Removed` drops it); the painter is created on the first frame of any surface (`Painter::new(cx.proc_address, …)`) and shared; `message` dispatch (`Niri` → every Island, `Appearance` → `tree.set_reduced_motion`, `painter.set_theme(theme.scaled(text_scale.factor()))`, `Command` → the focused output's Island (the output with keyboard focus, else the first)); `Message::NiriRequestRefused` → `tracing::warn!` once. `island/`: the spike's view as a `View` function over `IslandState`, the clock (one `date +%z` at start, in-process formatting, monotonic minute tick), the workspace dots (6 px dots, the active one a 14×6 pill in `Accent`), `sync_ctl` with `KeyboardInteractivity::None` at rest and `OnDemand` open, input region = silhouette at rest and the whole surface open, Escape/click-outside/`FocusLost` close; `copy.rs` (head line `// language-contract: product-copy`): the fallback workspace label, the numbered workspace label, the three page titles, the close and overflow button labels, each as a Spanish string literal with its English doc comment. `bus.rs`: zbus blocking `Connection::session()`, `ObjectServer::at("/org/celestina/Shell", Shell1 { sender })` with `#[interface(name = "org.celestina.Shell1")] fn command(&self, words: Vec<String>) -> (bool, String)` (parse with `Verb::parse`, send through the channel, reply `(false, "unknown verb …")` on a parse error), then `request_name_with_flags("org.celestina.Shell", RequestNameFlags::DoNotQueue)`; the worker thread blocks on the connection and exits on shutdown (`Connection::close`? zbus blocking has none: the worker holds the connection and the `Worker` join drops it after `shutdown`; name release on drop). Metrics as the spike's final wave (`cpu_p50/p99/max`, `n=`, late and `comp_late`, RSS).

- [ ] **Step 3: Scripts and documents.** `scripts/smoke.sh`: when `niri` is on `PATH`, start a nested niri with `scripts/dev-session.kdl` (`niri --config … -- sh -c "$bin --exit-after 3 --metrics > $log"` through `scripts/dev-session.sh`, which also strips the patched-niri-only keys as the halted script did), assert exit 0, `frames=` ≥ 1 for the `winit` output and `late` reported, and that the last 500 ms show zero frames (the metrics report prints `rest_frames=0` for frames drawn while nothing animated after the first second); otherwise print `smoke: niri is absent; the nested session was not run` and exit 0. `activate-production.sh`: `production_require_verified`, refuse if `busctl --user status org.celestina.Shell` finds an owner (the shell is already running), then `setsid -f "$prefix/bin/celestina"`. `deploy-production.sh` additionally installs `celestina-style/fonts/InterVariable.ttf` and `celestina-style/icons/*.svg` under `$prefix/share/celestina/`. `README.md` (the verbs, `RUST_LOG`, the nested session), `STATUS.md`, `ROADMAP.md` (B done pending landing), `VALIDATION.md` (`VAL-CEL-ISLAND`, `VAL-CEL-REST`, `VAL-CEL-SCALE` procedures), the ledger row, evidence `celestina/docs/evidence/2026-10-14-skeleton.md` (commands, exits, the metrics line from the smoke, atlas size measured: `tracing` prints the atlas occupancy at exit under `--metrics`, §9 risk).

- [ ] **Step 4:** all tests and guards pass; `cargo build --release --locked`; `sh celestina/scripts/smoke.sh --binary …/release/celestina` → OK (or the skip notice); commit `celestina-maintenance: Add the Island capsule, the app, the Shell1 command surface and the production scripts`. The author lands with `--kind milestone` (0.2.0).

### Task 11: `celestina-tokens` consumer seam (prepares C)

**Files:**
- Modify: `celestina/crates/celestina-paint/src/theme.rs` (`Theme::dark()` reads from one private `const DARK: &[(Role, &str, f32)]` table and one `const SCALARS` block, laid out exactly as `celestina_tokens::Scheme` and the scalar modules will be, so C's switch is a one-file change)
- Test: `theme.rs` test `dark_table_covers_every_role` (every `Role` variant has a row)

- [ ] **Step 1:** test → FAIL on a missing role; **Step 2:** fill; **Step 3:** commit `celestina-maintenance: Lay the theme table out as the tokens crate will be generated` (part of B's batch).

---

## Unit CEL-1-C — tokens as data (suite)

Worktree: `scripts/worktree.sh open suite CEL-1-C` after B lands. Evidence `docs/evidence/2026-10-14-tokens-as-data.md`.

### Task 12: `tokens.toml`, the generator, the QML output, the Rust output

**Files:**
- Create: `celestina-style/tokens.toml`, `celestina-style/scripts/gen-tokens.py`, `celestina-style/scripts/test-gen-tokens.py`, `celestina-style/CelestinaTokenData.qml`, `celestina-rs/crates/celestina-tokens/src/generated.rs`
- Modify: `celestina-style/CelestinaTheme.qml` (literals → bindings to `CelestinaTokenData`), `celestina-style/qmldir`, `celestina-style/CMakeLists.txt` (register the new singleton), `celestina-rs/crates/celestina-tokens/src/lib.rs` (`pub mod generated; pub use generated::*;`)

**Interfaces (produces):**

```toml
# celestina-style/tokens.toml — every key carries a one-line comment that is its definition
[ref]            # primitive ramps, hex strings:   neutral_0 = "#050608" …
[scheme.dark]    # one key per ColorScheme role of CelestinaTheme.qml:  canvas = "#050608", card = "#14171c", … accent = "#3e91ff", accentInk = "#050608", glassTint = { color = "#0b0c10", alpha = 0.70 }, glassFallback = "#0b0c10", …
[radius]         lg = 20, md = 12, sm = 8, button = 10, input = 12, xs = 3, pill = 9999
[type]           mini = 10, caption = 11, rowSecondary = 12, body = 14, rowTitle = 17, title = 17, headerCollapsed = 20, headerExpanded = 30, display = 34; weightBody = 400, weightTitle = 600; scales = { compact = 0.9, normal = 1.0, large = 1.15, larger = 1.3 }
[spacing]        xs = 4, sm = 8, md = 12, lg = 16, cardInset = 20, sideMargin = 24, rowHeight = 40, topBarHeight = 48
[motion]         fast = 100, normal = 200, slow = 350, exit = 150, ceiling = 500; curveStandard = [0.22, 0.25, 0.0, 1.0], curveSineInOut80 = [0.33, 0.0, 0.2, 1.0], curveSineInOut90 = [0.33, 0.0, 0.1, 1.0]; pressScale = 0.96
[haze]           sigma = 12, grain = 0.15, tint = 0.70, contentStrength = 0.64, veilStrength = 0.12
[control]        # comp* anatomy metrics of CelestinaTheme.qml, one key each
```

```python
# gen-tokens.py (stdlib only): gen-tokens.py [--check] [--out-qml PATH] [--out-rs PATH]
# reads tokens.toml with tomllib; writes CelestinaTokenData.qml (`pragma Singleton` QtObject: `readonly property QtObject ref`, one
# `readonly property ColorScheme schemeDark` shaped like CelestinaTheme's component, the scalar properties) and generated.rs
# (`pub struct Rgba { pub r: u8, pub g: u8, pub b: u8, pub a: f32 }`, `pub struct Scheme { pub canvas: Rgba, … }`, `pub const DARK: Scheme`,
# `pub mod radius { pub const LG: f32 = 20.0; … }`, `pub mod type_scale`, `pub mod spacing`, `pub mod motion { pub const FAST_MS: u32 = 100; …
# pub const CURVE_STANDARD: [f32; 4] }`, `pub mod haze`, `pub mod control`, `impl Scheme { pub fn by_name(name: &str) -> Option<&'static Scheme> }`);
# --check regenerates into a temporary directory and exits 1 with a diff summary when either committed output differs.
```

- [ ] **Step 1: Extraction, then tests.** Run a throwaway script (not committed) over `celestina-style/CelestinaTheme.qml` that lists every `readonly property` whose value is a literal (`"#…"`, a number, `Qt.rgba(...)` with literal arguments) and the `ColorScheme` role list; write `tokens.toml` from it by hand with the comments; then `test-gen-tokens.py` (unittest): `every_scheme_role_of_celestina_theme_is_a_key` (parses the `component ColorScheme` block's `required property` names from the QML and compares with `[scheme.dark]` keys), `hex_colours_parse_to_rgba` (`#3e91ff` → `(62, 145, 255, 1.0)`, a `{ color, alpha }` table → alpha), `check_mode_fails_on_a_stale_output` (temp dir with a changed byte), `generated_rust_is_stable` (two runs identical). → FAIL; implement the generator; → PASS.

- [ ] **Step 2: Wire the QML.** `CelestinaTheme.qml`: `readonly property ColorScheme schemeDark: ColorScheme { canvas: CelestinaTokenData.schemeDark.canvas … }` for every role; scalar `readonly` properties bound to `CelestinaTokenData.<section><Key>`; the functions, `textScale`, `reducedMotion`, the icon recipes and `usagePalette` ordering stay in `CelestinaTheme.qml`. Register `CelestinaTokenData` in `qmldir` (`singleton CelestinaTokenData 1.0 CelestinaTokenData.qml`), in `CMakeLists.txt`'s `qt_add_qml_module` files and in every consumer's `fonts.qrc`/symlink set if the applications list style files explicitly (check `siderita/qml` for the symlink pattern: `ls -l siderita/qml | grep CelestinaTheme`). `tests/tst_tokendata.qml`: for each role name in a generated list, `compare(CelestinaTheme.scheme[role], CelestinaTokenData.schemeDark[role])`. Style guard, contrast guard (`celestina-style/scripts/check-contrast-contract.py`), `all_qmllint`, gallery: unchanged results.

- [ ] **Step 3: Verifies and registry.** `celestina-style/scripts/verify-production.sh` and `celestina-rs/scripts/verify-production.sh` run `python3 celestina-style/scripts/gen-tokens.py --check` first; `docs/projects.toml`: `celestina-style.production_inputs` gains `celestina-style/tokens.toml` and `celestina-style/scripts/gen-tokens.py`, `verification_inputs` gains `celestina-style/scripts/test-gen-tokens.py`; `celestina-rs.verification_inputs` gains `celestina-style/scripts/gen-tokens.py` and `celestina-style/tokens.toml`; `celestina.production_inputs` gains `celestina-style/tokens.toml`.

- [ ] **Step 4:** commit `suite-maintenance: Generate the style tokens from tokens.toml into QML and the celestina-tokens crate`.

### Task 13: the shell reads the crate

**Files:**
- Modify: `celestina/crates/celestina-paint/src/theme.rs` (`impl From<&celestina_tokens::Scheme> for Theme`; delete `DARK`/`SCALARS`; `Theme::dark()` becomes `Theme::from(&celestina_tokens::DARK)`), `celestina/crates/celestina-paint/Cargo.toml` (celestina-tokens path dep), `celestina/src/app.rs` (`Scheme::by_name(appearance.scheme.as_deref().unwrap_or("dark"))` with `dark` fallback once `celestina-settings` has a `scheme` key; until then `DARK`)

- [ ] **Step 1:** test `theme_from_tokens_matches_the_generated_dark_scheme` (spot-checks accent, card, glass tint alpha 0.70, grain 0.15, motion) → FAIL → implement → PASS; `cargo test --workspace` in `celestina`; commit `suite-maintenance: Read the shell's theme from celestina-tokens`. Land `--kind maintenance`.

---

## Unit CEL-1-D — Island pages (celestina, milestone)

Worktree from `origin/main` after C. Evidence `celestina/docs/evidence/2026-10-14-island-pages.md`.

### Task 14: pages, tabs and the slide

**Files:**
- Create: `celestina/src/island/pages/{mod.rs, activity.rs, notifications.rs, tray.rs, tabs.rs}`
- Modify: `celestina/src/island/{state.rs, view.rs, mod.rs}`, `celestina/src/copy.rs`, `celestina/src/providers/niri.rs` (app-id → `.desktop` icon lookup through `celestina_core::desktop_entry::find`), `celestina/src/app.rs` (ownership lines from F/G placeholders: `NameOwner { notifications: Option<String>, tray: Option<String> }`)

**Interfaces (produces):**

```rust
// state.rs additions
impl IslandState { pub fn switch(&mut self, page: Page, now: Instant, reduced: bool); pub fn slide(&self, now: Instant) -> f32 /* −1..1 */ ;
                   pub fn page_size(&self) -> (f32, f32) /* the open page's request, ≤ surface minus margins */ }
// pages/mod.rs
pub trait PageView { fn title(&self) -> &str; fn view(&self, cx: &PageCx<'_>) -> View; fn wants(&self) -> (f32, f32) }
pub struct PageCx<'a> { pub model: Option<&'a NiriModel>, pub output: &'a str, pub owners: &'a NameOwner, pub history: &'a [HistoryRow] /* F */, pub items: &'a [TrayItem] /* G */ }
```

- [ ] **Step 1: Failing tests.** `activity_rows_list_the_outputs_workspaces_then_the_active_workspaces_windows` (model with two outputs, three windows: rows keyed `indexed("ws", id)` and `indexed("win", id)` in order, titles truncated at 48 graphemes with «…»), `tabs_show_three_pages_with_the_open_one_selected`, `switching_pages_slides_content_horizontally` (`slide` goes from −1 to 0 over `Normal` when moving right, +1 to 0 when moving left), `notifications_page_shows_the_owner_line_while_noctalia_holds_the_name` (owners.notifications = Some("org.freedesktop.Notifications is held by :1.42 (noctalia)") → the page's single row text and the switch marked disabled), `tray_page_same`, `page_size_grows_to_the_pages_request_but_not_past_the_surface`. → FAIL → implement (`tabs.rs`: a bottom row of three `ControlFill` pills; `activity.rs`: workspace rows with column count and the windows under the active one with their `.desktop` icon by `app_id` through `IconKey::Theme(icon_name)` or `Semantic("app-window")`; clicks → `IslandAction::FocusWorkspace/FocusWindow`; `copy.rs` gains the do-not-disturb label, the two empty-state lines and the two owner lines, the latter as `pub fn notifications_owner(owner: &str) -> String` and `pub fn tray_owner(owner: &str) -> String`).

- [ ] **Step 2:** tests PASS; commit `celestina-maintenance: Add the Island's Activity, Notifications and Tray pages with tabs and the slide`.

### Task 15: the overflow popup

**Files:**
- Create: `celestina/src/island/menu.rs` (a `PopupShell`: Haze card `radius_md`, three rows: reopen the page, do not disturb (shows the switch state), quit; the Spanish labels live in `copy.rs`)
- Modify: `celestina/src/island/mod.rs` (`IslandAction::OpenMenu` → `ctl.open_popup(Placement { anchor: more_rect, anchor_edge: BottomLeft, gravity: BottomRight, offset: (0, 8) }, MENU_SIZE, Box::new(Menu::new(..)))`; `PopupClosed` handling as the spike: `Dismissed` closes the page, `Parent` does not), `celestina/src/app.rs` (menu results → `Message`-free direct calls: `Quit` → `ctl.stop()`, `DoNotDisturb` → settings store + F's server flag)

- [ ] **Step 1:** tests: `menu_rows_fill_the_menu` (spike's), `a_dismissed_menu_closes_the_page_but_an_island_press_does_not` (spike's), `menu_grabs_with_the_last_input_serial_pointer_or_key` (pure: `GrabSerial::latest(pointer_serial, key_serial)` picks the newer by arrival order). → FAIL → implement → PASS.
- [ ] **Step 2:** documents (`VALIDATION.md` `VAL-CEL-POPUP`, `VAL-CEL-KEYBOARD` procedures), ledger, evidence; commit `celestina-maintenance: Add the Island's overflow menu as an xdg_popup`. Land `--kind milestone` (0.3.0).

---

## Unit CEL-1-E — the launcher (celestina, milestone)

Evidence `celestina/docs/evidence/2026-10-14-launcher.md`.

### Task 16: `Field` in `celestina-ui`

**Files:**
- Create: `celestina/crates/celestina-ui/src/field.rs`; `celestina/crates/celestina-wayland/src/{text_input.rs, data_device.rs}` (zwp_text_input_v3 and the clipboard read through `wl_data_device`/`wl_data_offer`, `text/plain;charset=utf-8` only, bounded to 64 KiB)
- Modify: `celestina/crates/celestina-ui/src/{view.rs, tree.rs, paint.rs, input.rs}` (`ViewKind::Field { id: FieldId, placeholder: String, style: TextStyle }`), `celestina/crates/celestina-wayland/src/input.rs` (`InputEvent::Preedit { text, cursor }`, `InputEvent::CommitText(String)`, `InputEvent::Paste(String)`; `SurfaceCtl::request_paste()`, `SurfaceCtl::set_text_input_rect(Rect)`)

**Interfaces (produces):**

```rust
pub struct Field; impl Field { pub fn new() -> Field; pub fn text(&self) -> &str; pub fn set_text(&mut self, s: &str);
    pub fn edit(&mut self, op: EditOp) -> bool /* changed */; pub fn cursor(&self) -> usize; pub fn selection(&self) -> Option<Range<usize>>;
    pub fn preedit(&self) -> Option<&str> }
pub enum EditOp { Insert(String), Backspace, Delete, Left { select: bool, word: bool }, Right { .. }, Home { select: bool }, End { select: bool },
                  SelectAll, Paste(String), Preedit { text: String, cursor: usize }, CommitPreedit(String), PointerDown(f32), PointerDrag(f32) }
```

- [ ] **Step 1: Failing tests** (`field.rs`): `backspace_deletes_a_whole_grapheme` (a letter with a combining accent, U+0065 U+0301; a two-codepoint flag emoji), `word_jumps_stop_at_word_boundaries` (unicode-segmentation through cosmic-text's `Editor`), `shift_arrows_extend_the_selection_and_typing_replaces_it`, `paste_inserts_at_the_cursor_and_is_clamped_to_256_chars` (Review Focus 5's bound lives here as `MAX_CHARS = 256`), `preedit_shows_and_commit_replaces_it`, `pointer_down_places_the_cursor_by_x`. → FAIL → implement over `cosmic_text::Editor` with `Action`s → PASS.
- [ ] **Step 2:** wayland: `zwp_text_input_v3` enable/disable with keyboard focus, `set_cursor_rectangle`, `preedit_string`/`commit_string`/`done` → events; `request_paste` reads the current offer on a worker and delivers `Paste`. Tests for the pure parts (`TextInputState` serial bookkeeping: `done` applies only the pending preedit/commit of the matching serial). Commit `celestina-maintenance: Add the Field editor, IME and clipboard paste`.

### Task 17: the launcher surface, index, ranking, `/` commands, spawn

**Files:**
- Create: `celestina/src/launcher/{mod.rs, state.rs, view.rs, commands.rs, spawn.rs}`, `celestina/src/providers/desktop.rs`
- Modify: `celestina/src/app.rs` (`Verb::LauncherToggle` → open on the output with keyboard focus; the surface is created on demand on `celestina-overlay`, `Layer::Overlay`, centred (anchor none), size (560, 8 × 40 + field + paddings), `KeyboardInteractivity::Exclusive`, destroyed on close), `celestina/src/copy.rs` (the field placeholder and the four switches' labels)

**Interfaces (produces):**

```rust
pub enum Switch { Notificaciones, NoMolestar, Actividad, Bandeja }   // `/notificaciones` `/no-molestar` `/actividad` `/bandeja`
pub fn commands_for(query: &str) -> Option<Vec<Switch>>;               // Some(all) for "/", Some(matches) for "/x", None otherwise
pub struct LauncherState { pub query: Field, pub selected: usize, pub rows: Vec<Row> } pub enum Row { App(DesktopEntry), Switch(Switch) }
impl LauncherState { pub fn update(&mut self, index: &[DesktopEntry]); pub fn move_selection(&mut self, delta: i32); pub fn chosen(&self) -> Option<&Row> }
pub fn spawn(entry: &DesktopEntry) -> Result<(), SpawnError>;         // setsid + exec_argv, stdin/out/err null, launcher env
```

- [ ] **Step 1: Failing tests.** `ranking_orders_prefix_then_word_start_then_substring` (through `celestina_shell::launcher::rank_bounded` with 8 as the limit), `slash_alone_lists_every_switch`, `slash_unknown_lists_nothing_and_chooses_nothing` (Review Focus 5), `arrows_wrap_within_the_rows`, `recent_use_lifts_an_entry` (a `recent.json` of the last 50 launched ids under the config dir, as the halted launcher ranked), `spawn_builds_setsid_argv_from_exec_with_field_codes_stripped` (pure: `spawn_argv(entry) -> Vec<String>`). → FAIL → implement (`providers/desktop.rs`: `celestina_core::desktop_entry::scan` on a worker at start and every 5 min, result as `Message::DesktopIndex`; `state.rs`; `view.rs`: field on top, rows with icon (`IconKey::Theme`), name, generic name; Enter launches and closes, Escape closes, click outside closes, focus loss closes). → PASS.
- [ ] **Step 2:** `VALIDATION.md` `VAL-CEL-IME`; evidence; commit `celestina-maintenance: Add the launcher over the desktop index, the salvaged ranking and the slash switches`. Land `--kind milestone` (0.4.0).

---

## Unit CEL-1-F — notifications (celestina, milestone)

Evidence `celestina/docs/evidence/2026-10-14-notifications.md`.

### Task 18: the server

**Files:**
- Create: `celestina/src/providers/notifications.rs`, `celestina/tests/notifications_bus.rs`
- Modify: `celestina/src/message.rs` (`NotificationsEvent { Owner(Option<String>), Posted(Notification), Closed(u32, CloseReason), Expired(Vec<u32>) }`, commands `NotificationsCommand { Close(u32), Invoke(u32, String), SetQuiet(bool), ClearHistory }` through a `SyncSender` bound 32)

**Start from:** `celestina-qt/src/provider_adapter/notifications.rs` (the bus half; permission 4 of A) over `celestina_shell::notifications`.

- [ ] **Step 1: Failing tests.** `tests/notifications_bus.rs`: when `dbus-daemon` is on `PATH`, start a private bus (the config of Selenita's `smoke_release`), run the provider on it, call `Notify` through zbus with a summary, a body of 50 000 characters, 1 000 action strings and `image-path` `../../etc/passwd` (Review Focus 2): the posted notification's body is ≤ `MAX_BODY_CHARS`, actions ≤ 4, image `None`; `GetCapabilities` lists `capabilities()`; `CloseNotification` emits `NotificationClosed` with reason 3; a second provider on the same bus gets `Owner(Some(..))` and serves nothing; without `dbus-daemon` the test prints `skipped: dbus-daemon is absent` and passes. → FAIL → implement (serve the object, then `DoNotQueue`; `NameTaken` → `NotificationsEvent::Owner(Some(name))` from `GetNameOwner`; the 250 ms expiry tick; the state machine owned by the worker, snapshots sent as events). → PASS.
- [ ] **Step 2:** commit `celestina-maintenance: Add the notification server over the salvaged state machine`.

### Task 19: banners, history page, actions, do-not-disturb

**Files:**
- Create: `celestina/src/notifications/{mod.rs, banners.rs, history.rs}`
- Modify: `celestina/src/app.rs` (banner surface per focused output: `celestina-toasts`, `Overlay`, anchored top with margin = the Island's exclusive zone, size (420, 3 × 80 + gaps), input region only on the banners, `KeyboardInteractivity::None`; created when the first banner shows, destroyed when none remains), `celestina/src/island/pages/notifications.rs` (history rows with icon, summary, body (two lines, `max_width`), time (`calendar` for the date when not today), action buttons, a dismiss button per row and a dismiss-all button (labels in `copy.rs`), the do-not-disturb switch), `copy.rs`

- [ ] **Step 1: Failing tests.** `banners_stack_at_most_three_newest_first`, `urgency_expiry_low_5s_normal_10s_critical_never` (through the state machine's `expire` with the shell's defaults), `hover_pauses_expiry` (a `BannerClock` that excludes hovered time), `click_runs_the_default_action_or_opens_the_page`, `dismiss_all_closes_every_live_notification_with_reason_dismissed`, `do_not_disturb_hides_banners_but_keeps_history`, `history_page_lists_live_then_the_last_twenty_ended`. → FAIL → implement → PASS.
- [ ] **Step 2:** `Verb::NotificationsToggle` (opens the page) and `NotificationsDismissAll`; `VALIDATION.md` `VAL-CEL-GLASS` adds the banner; evidence; commit `celestina-maintenance: Add banners, the notifications history page, actions and do-not-disturb`. Land `--kind milestone` (0.5.0).

---

## Unit CEL-1-G — tray (celestina, milestone)

Evidence `celestina/docs/evidence/2026-10-14-tray.md`.

### Task 20: watcher, host and items

**Files:**
- Create: `celestina/src/providers/tray.rs`
- Modify: `celestina/Cargo.toml` (system-tray 0.8, justified: StatusNotifierWatcher/Host and the DBusMenu client in one crate), `celestina/src/message.rs` (`TrayEvent { Owner(Option<String>), Items(Vec<TrayItem>), Menu(ItemId, MenuTree) }`, `TrayCommand { Activate(ItemId), SecondaryActivate(ItemId), Scroll(ItemId, i32, Orientation), MenuEvent(ItemId, i32), OpenMenu(ItemId) }`)

```rust
pub struct TrayItem { pub id: ItemId, pub title: String, pub status: Status, pub icon: TrayIcon /* Name(String) | Pixmap { w, h, argb: Vec<u8> } */, pub has_menu: bool }
pub struct MenuTree { pub id: i32, pub entries: Vec<MenuEntry> } pub struct MenuEntry { pub id: i32, pub label: String, pub enabled: bool, pub toggle: Option<bool>, pub submenu: Option<MenuTree>, pub separator: bool }
```

- [ ] **Step 1: Failing tests.** Pure: `items_are_coalesced_to_one_update_per_100ms` (through `celestina_shell::coalesce::Coalescer`), `pixmap_bounds_reject_more_than_256_square` (a 4096×4096 pixmap is dropped with a warning, the item keeps a `Semantic("app-window")` icon), `menu_tree_depth_and_width_are_bounded` (depth ≤ 4, entries ≤ 64 per level, labels ≤ 128 chars), `owner_taken_disables_the_host` (the watcher half refuses as the notifications server does). → FAIL → implement over `system_tray::Client` on a worker (the crate's events mapped to `TrayEvent`), `DoNotQueue` for `org.kde.StatusNotifierWatcher` after the object is served. → PASS.
- [ ] **Step 2:** commit `celestina-maintenance: Add the tray watcher and host over system-tray`.

### Task 21: the Tray page and nested DBusMenu popups

**Files:**
- Create: `celestina/src/tray/{mod.rs, menu.rs}` (a `PopupShell` per menu level; a submenu opens as a popup of the popup, placed `RightTop`/`RightBottom` with slide/flip)
- Modify: `celestina/src/island/pages/tray.rs` (rows: icon from `IconKey::Theme(name)` or `Pixmap` uploaded once per item, title, status; left click `Activate`, right click `OpenMenu`, scroll → `Scroll`), `celestina/crates/celestina-wayland/src/popup.rs` (a popup may parent another popup: `SurfaceCtl::open_popup` on a popup's own ctl creates an `xdg_popup` whose parent is that popup's `xdg_surface`; the grab chain follows; closing a parent closes its children first)

- [ ] **Step 1: Failing tests.** `nested_popups_close_children_first` (pure order on the runtime's `Targets`), `menu_rows_map_dbusmenu_entries_with_separators_toggles_and_disabled_state`, `scroll_on_a_row_sends_scroll_with_the_delta_in_steps` (15 px per step). → FAIL → implement → PASS.
- [ ] **Step 2:** `VALIDATION.md` `VAL-CEL-NOCTALIA` (start while Noctalia runs, then after stopping it); evidence; commit `celestina-maintenance: Add the Tray page with nested DBusMenu popups and scroll`. Land `--kind milestone` (0.6.0).

---

## Unit CEL-1-H — exit and 1.0.0 (celestina, release)

### Task 22: close the program

**Files:**
- Modify: `celestina/{README.md, STATUS.md, ROADMAP.md, VALIDATION.md}`, `celestina/docs/plans/active/2026-10-14-cel-1-island.md` (rows), `celestina/docs/evidence/2026-10-14-exit.md`, `celestina/docs/handover.md` (new: what Noctalia still does for the session — OSD, media, lock, polkit, wallpaper, clipboard history, night light, caffeine — and which later delivery covers each, so the author knows what must exist before stopping Noctalia), `docs/plans/active/2026-10-14-rust-shell.md` → archived by its own administrative row if every suite row is done (otherwise it stays), root `ROADMAP.md`/`STATUS.md`

- [ ] **Step 1:** run every verification of §8 from the worktree and record it: `cargo test --workspace`, clippy, `celestina --check`, `smoke.sh` with the nested niri and `--metrics` (frames ≥ 1 per output, zero frames in rest, RSS printed), `gen-tokens.py --check`, the guards; keyboard-only walk of every surface written as the `VAL-CEL-KEYBOARD` procedure; every `VAL-CEL-*` row `pending` with its procedure, none blocking.
- [ ] **Step 2:** `STATUS.md` 1.0.0 (published by the landing), `ROADMAP.md` `Status: done`, checkpoint `none` once the plan's last row closes (the plan is archived by an administrative unit after H lands, as SEL-1-G did), the handover note; commit `celestina-maintenance: Close CEL-1 with the exit evidence and the Noctalia handover note`. Land `--kind release` (1.0.0).

---

## Self-review (2026-10-14)

- **Spec coverage.** §1 deliveries 1–5 → B+D (Island), E (launcher), F, G, C (tokens). §2 decisions → Global Constraints and Decisions 1–6. §3 constraints → Global Constraints. §4 facts → used in A (the six references, the shell role, the halt guard), B (viewporter/fractional required, stencil, `MinimizedWindowsChanged`), C (`CelestinaTheme.qml`, `appearance.toml`). §5.1 → Task 6 (loop, channel, signals) and Task 9 (`Worker`, bounded `sync_channel`, coalescing). §5.2 → Task 6 (every bullet, including cursor shape, output messages, regions with the frame, `Placement`, nested popups in Task 21). §5.3 → Task 7 (Haze order and fallback, atlas keyed and bounded, tabular/weight flags, icons three ways). §5.4 → Task 8 (kinds, keyed reconciliation, `Animated` with reduced motion, hit-test, Tab order, Escape) and Task 16 (`Field`). §5.5 table → Task 10 (Island surface and namespaces), Task 17 (launcher), Task 19 (banners), Tasks 15/21 (menus); the Island paragraph → Tasks 10/14; Activity → 14; Notifications page → 19; Tray page → 21; Launcher → 17; Banners → 19. §5.6 → Tasks 9 (stream and requests), 10 (focus policy, command surface, single instance), 18/20 (names claimed if free). §5.7 → Tasks 12–13. §6 crates, files, dependencies → the Files lists and `Cargo.toml` steps; `celestina.desktop`, scripts, documents → A and Task 10. §7 units → the units table; CEL-1-A's list → Task 5 items 1–8 (the spec's "ratchet rows" are the radius row rename and the language rows removed in A0). §8 → every test named above, `verify-production.sh` in A/Task 10, `smoke.sh` in Task 10, live rows in Tasks 10, 15, 17, 19, 21, 22, guards in Global Constraints. §9 risks → bounded atlas (Task 7), `--no-blur` kept (Task 6), tolerant parser (Task 9), `DoNotQueue` (18/20), A0's rehearsal, local constants in B deleted in C (Tasks 11/13). §10 decisions → nine ADRs in A.
- **Placeholders.** None: every step names its file, test or command; the two "known unknown" guard behaviours (documentation contract on moved inventories, the registry's halted key in fixtures) are resolved by Task 3's rehearsal with a rule change and a test, not deferred.
- **Type consistency.** `history_owner` of Task 1 is the key Tasks 2 and 5 use (no `renamed` row kind exists); `Rect` is defined once in `celestina-wayland::geometry` and re-exported by `celestina-paint`; `Role` and `Theme` in `celestina-paint::theme` are what `celestina-ui::ViewKind` and the product use; `Key`, `UiEvent` and `Interaction` names match between Tasks 8, 10, 14–21; `Message` variants added in Tasks 9, 17, 18, 20 are the ones `app.rs` dispatches; `Verb`/`Page` of Task 10 are what Tasks 14, 17, 19 consume; the `Theme`/`Role` names are shared by Tasks 7, 8, 11 and 13.
- **Review Focus.** Items 1–5 have their tests in Tasks 6, 19, 9, 7 and 16/17 respectively.
