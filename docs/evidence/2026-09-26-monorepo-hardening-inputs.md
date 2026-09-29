# Evidence: production inputs that hold every crate an app links

- **Date:** 2026-09-26
- **Scope:** `AUD-1-D` (program unit P-2) of the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md), closing TOOL-3, TOOL-4, TOOL-21 and TOOL-22 of the [tooling audit](2026-09-26-monorepo-audit-tooling.md), MAG-9 of the [Magnetita audit](2026-09-26-monorepo-audit-magnetita.md) and FLU-21 of the [Fluorita audit](2026-09-26-monorepo-audit-fluorita.md); session worktree `Celestina.worktrees/suite-AUD-1-D`, branch `unit/suite/AUD-1-D`, stacked on `unit/suite/AUD-1-B` at `4111a8d`
- **Environment:** Linux container (kernel 6.18) running as uid 0; Python 3.11.15, Git 2.43.0; cargo 1.94.1 as the rustup default and 1.97.1 selected by `celestina-rs/rust-toolchain.toml` and `siderita/rust-toolchain.toml`; no Qt 6 SDK, CMake build of the shell, libmpv, Gradle or Android SDK
- **Artifact:** the landing builds it. The unit changes `docs/projects.toml`, `scripts/production_artifact.py` and the architecture guard, which are verification inputs of every registered project, and the production inputs of Magnetita, Magnetita Android, Grafita, Fluorita and Hematita

## Decision

The closure is guarded, not derived at fingerprint time. The fingerprint stays
a pure function of the declared `production_inputs`: it never asks Cargo for
the graph, so two runs over the same bytes agree, `check`, deploy and status
cost no metadata call, and the landing's affected-project matching
(`scripts/landing.py`, which reads `production_input_patterns`) and
`scripts/agent-context.py` read the same list that the guard proves complete.
A fingerprint-time derivation would have given those readers a second,
invisible input set. The guard costs about 0.6 s over the whole registry, one
`cargo metadata --no-deps --offline` call per workspace, and it runs in
`scripts/check-architecture-contract.sh`, so CI and every landing's
`pre_guards` and `run_guards` fail when an app gains a path dependency its
registry entry does not name.

Canonical owner: `scripts/cargo_closure.py` reads the path-package closure
from Cargo; `scripts/production_artifact.py` owns the rule
(`input_contract_errors`, `required_cargo_inputs`, `hashed_by`,
`declared_production_inputs`) and the `check-inputs` command;
`scripts/agent-context.py` reuses `production_input_patterns`,
`expand_patterns` and `hashed_by` instead of a second matcher. Equivalent
recipes searched: `rg "cargo metadata" scripts` found the dependency-direction
check in the architecture guard (which scans dependency names, not paths) and
the landing's lockfile merge; neither computes a closure.

## Procedure

Tests first, on the unchanged `production_artifact.py`:

```sh
python3 scripts/test-production-artifacts.py
bash scripts/test-documentation-contract.sh
```

The guard over the real registry with `cargo_manifests` declared but the old
`production_inputs` (the defect TOOL-3 describes):

```sh
python3 scripts/production_artifact.py check-inputs
```

Fingerprints under the old (`git show HEAD:docs/projects.toml`, before this
unit) and the new registry, per project, with `production_artifact.py`'s own
`digest_paths` and `production_fingerprint` on this tree.

The architecture fixture without the guard call (the line
`check_production_inputs` removed from `scripts/check-architecture-contract.sh`,
restored after):

```sh
bash scripts/test-architecture-scanners.sh
```

Every check the dispatch and the plan name, on the committed tree `63d2e5b`:

```sh
python3 scripts/test-production-artifacts.py
bash scripts/test-production-artifacts.sh
sh scripts/check-documentation-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/test-land-unit.sh
python3 scripts/version_tool.py check
bash scripts/test-architecture-scanners.sh
bash scripts/test-documentation-contract.sh
python3 scripts/test-version-contract.py
bash scripts/test-commit-scope.sh
bash scripts/test-staged-units.sh
python3 scripts/test-language-contract.py
```

## Result

- **Exit (RED, tests before the change):** `test-production-artifacts.py`
  exit 1, 42 tests, 13 failures, every one in the 12 new tests:
  `check-inputs` was an invalid choice (exit 2) for the seven guard tests and
  the repository-registry test; a registry with `production_inputs = []`
  built, sealed and checked current; a changed `rustc` left
  `check --require-verified` at `artifact: demo current`; and a failed check
  printed one line without naming `demo/src` or `demo/tests/new.txt`. The
  new agent-context fixture failed:
  `FAIL: agent-context omitted the project whose production inputs hold the crate`.
- **Exit (RED, guard over the old inputs):** `check-inputs` exit 1 with
  exactly the audit's list: magnetita misses `fluorita-core`,
  `fluorita-engine`, `fluorita-qt`, `magnetita-link`, `magnetita-proto` and
  `siderita-ops`; magnetita-android declares no `production_inputs`; grafita
  misses `celestina-core`; fluorita misses `siderita-ops`; hematita misses
  `celestina-core` and `siderita-ops`.
- **Exit (RED, mutation):** without the guard call the architecture fixtures
  exit 1: `the architecture guard accepted production inputs that omit a
  linked crate`.
- **Exit (GREEN):** every command above exit 0.
  `test-production-artifacts.py` 42 tests OK (30 before, 12 new);
  `test-production-artifacts.sh` also `production-common fixtures: OK`;
  `test-land-unit.sh` 56 tests OK; `test-version-contract.py` 22 tests OK;
  `test-language-contract.py` 12 tests OK; `Architecture contract: OK`,
  `Architecture fixtures: OK`, `Documentation contract: OK`,
  `Documentation contract and agent-context: OK`,
  `Language contract: OK (148 legacy file(s) ratcheted)`,
  `version-contract: OK (8 owners)`, `Commit scope: OK`,
  `Staged inventories: OK`. `check-inputs` prints
  `production inputs: every buildable project declares them, with each Cargo path package its artifact links`
  in 0.57 s.
- **Fingerprints, old against new registry:** magnetita, magnetita-android,
  grafita, fluorita and hematita change (stale); celestina, celestina-style,
  celestina-rs and siderita are unchanged, because `cargo_manifests` is not
  part of the fingerprinted contract.

## Observed facts

- **TOOL-3, MAG-9, FLU-21.** `docs/projects.toml` now declares
  `cargo_manifests` for every Rust build: each app's own `Cargo.toml`,
  `celestina-rs/Cargo.toml` (virtual, so all members),
  `celestina-rs/crates/magnetitad/Cargo.toml` for Magnetita, and
  `celestina-rs/crates/magnetita-mobile/Cargo.toml` for the APK. The missing
  crates are added to Magnetita, Grafita, Fluorita and Hematita. The guard
  (`production_artifact.py check-inputs`, run by
  `check-architecture-contract.sh`) requires, per project: every path
  package outside the project directory whole (so `fluorita-qt/cpp` counts),
  the project's own packages by manifest and compiled target sources, each
  inherited workspace manifest, each workspace lockfile, and the nearest
  `rust-toolchain.toml`. Development dependencies are left out
  (`magnetitad`'s dev-only `magnetita-mobile` is not required); build
  dependencies are included (`fluorita-qt`).
- **MAG-9, Gradle half.** `magnetita-android/app/build.gradle.kts`'s
  `buildNative` listed three `src` directories as inputs, missing
  `magnetita-net`, `celestina-core`, the manifests and the lockfile, so
  Gradle could skip the native build after a rebuild the fingerprint asked
  for. The task now has `outputs.upToDateWhen { false }`, and Cargo's own
  incremental build decides what to recompile.
- **TOOL-4.** Magnetita Android declares production inputs (`app/src/main`,
  both `build.gradle.kts`, `settings.gradle.kts`, `gradle.properties`,
  `gradle/`, `gradlew`, `scripts/build-native.sh`, the workspace manifest,
  lock and toolchain file, and the five crates of `magnetita-mobile`'s
  closure) and verification inputs (`app/src/test`, `app/lint.xml`,
  `magnetita-peer`, the toolchain file). A buildable project with no or an
  empty `production_inputs` is refused by `run-build`, `check` and
  `check-inputs` (`declared_production_inputs`).
- **TOOL-22.** The manifest's Rust probes now run where each Cargo build
  runs: the directory of each declared `cargo_manifests` entry, or the
  project directory without one (fix round 1; the first version used the
  project directory, which gave Magnetita and Magnetita Android the default
  1.94.1 although `magnetitad` and `build-native.sh` build on the pinned
  1.97.1). Here siderita, celestina-rs and magnetita-android record 1.97.1;
  magnetita records `rustc@magnetita` 1.94.1 and
  `rustc@celestina-rs/crates/magnetitad` 1.97.1; grafita records 1.94.1.
  Every probe has a 30 s timeout. `check` compares them; a difference adds
  `the toolchain changed since the build; run build-production.sh`, which is
  not a verification error, so the landing rebuilds instead of re-verifying.
- **Changed-input report.** `run-build` records
  `[production_input_digests]` and `[verification_input_digests]`, one digest
  per expanded input; `run-verification` refreshes the second. A failed
  `check` keeps its first line (the one `scripts/landing.py` reads) and adds
  one line per new, removed or changed input, up to 20, and per changed
  toolchain probe. A manifest from before this unit says it records no
  digests.
- **TOOL-21.** `agent-context.py` adds, after the lexical owners, every
  project whose production inputs hold the path or lie below it
  (`agent-context.py celestina-rs/crates` lists every Rust app and the APK).
  `agent-context.py celestina-rs/crates/siderita-ops` now prints Magnetita,
  Fluorita and Hematita next to celestina-rs and Siderita;
  `celestina-rs/crates/magnetita-link/src/lib.rs` prints Magnetita Android.
- `scripts/test-land-unit.py` copies `cargo_closure.py` into its fixture
  repositories, because `production_artifact.py` imports it.
- **Landing consequence.** The landing of this unit changes
  `docs/projects.toml` and `scripts/production_artifact.py`, verification
  inputs of every project, so it checks all nine. Magnetita, Magnetita
  Android, Grafita, Fluorita and Hematita are stale by fingerprint and are
  rebuilt: that needs Qt, CMake, libmpv and the Android SDK and NDK on the
  author's machine. A manifest whose recorded probes differ from the new
  probes is stale by toolchain: Siderita and celestina-rs are rebuilt too
  whenever the author's default rustc is not the pinned 1.97.1. The others
  re-verify. An old manifest has no input digests, so its
  first failed check says so instead of naming the changed inputs.

## Fix round 1

Review findings fixed on top of `86f2cbd`:

- Rust probes run in each declared Cargo manifest's directory
  (`rust_probe_directories`); toolchain keys are quoted in the manifest,
  so `rustc@<dir>` is valid TOML. New fixture
  `test_toolchain_is_probed_where_the_build_runs` (a fake `rustc` prints
  `pwd -P`). RED before the fix: `KeyError: 'rustc@demo'`. Mutation: making
  the directory helper return the root fails that test.
- New fixtures: `test_input_guard_requires_the_selected_toolchain_file`
  (mutation: disabling `if toolchain is not None` in `cargo_closure.py`
  fails it), `test_input_guard_reports_a_manifest_cargo_cannot_read` and
  `test_input_guard_refuses_a_path_dependency_outside_the_repository`
  (`CargoGraphError` paths), and
  `test_closure_follows_normal_and_build_dependencies_only`, which asserts
  the closure's package directories, sources and workspace manifests in
  place of the old tautological dev-dependency test.
- `agent-context.py` `consumer_projects` reports a malformed `projects`
  entry through the error list instead of an `assert`. A directory above an
  input now lists that input's consumers, with a fixture on `core/crates`
  (RED: `agent-context omitted the consumers of a crate below the target
  directory`).
- `run_text` has a 30 s timeout, and a timed-out probe records
  `unavailable`.
- The plan's scope line now says the inputs are guarded, not derived.

The 13 commands of the procedure, rerun on the fixed tree, all exit 0:
`test-production-artifacts.py` 46 tests OK, `test-land-unit.sh` 56 OK,
`test-version-contract.py` 22 OK, `test-language-contract.py` 12 OK, and
every guard and fixture script OK.

## Limits

- No production build, verification, deploy or landing ran: there is no Qt,
  CMake shell build, libmpv, Gradle or Android SDK here, and a session
  worktree refuses production runs. The staleness above is proven by
  fingerprint comparison, not by `check` against the author's real manifests.
- The Gradle change was not executed. It must be confirmed by the landing's
  Android build: `buildNative` runs on every `assembleRelease`, and an APK
  built after a change to `magnetita-net` alone carries the new library.
- The guard follows Cargo's declared path dependencies. A build script that
  reads a file outside its package without a path dependency (none was
  found) would still escape it, as would non-Cargo inputs of a CMake or
  Gradle build, which stay hand-declared.
- The toolchain comparison is conservative: every project records all five
  probes, so a CMake or Qt upgrade also stales a Rust-only artifact.

## Follow-up

- MAG-10 (verification of proto, link, mobile and peer) stays with
  `MAG-D1-D`.
- For `AUD-1-F`: choose probes per project, so a Qt or CMake upgrade no
  longer stales pure-Rust artifacts and the APK.
- For `AUD-1-F`: `hashed_by` is a third path matcher next to
  `scripts/landing.py`'s `affected_projects` and `matches_pattern`. Give
  them one owner.

## Landing

- **Base revision:** `1c2e8b36a8255f9521a3840980d4b49ac7f4b5e4`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: the toolchain changed since the build; run build-production.sh; artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: the toolchain changed since the build; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; the toolchain changed since the build; run build-production.sh; artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; the toolchain changed since the build; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style verify: verify-production.sh exit 0, manifest source_fingerprint sha256:a07296319641a7bb878fddba20ba6faa4c0bbda5e9a3377366b2145606354c2d, verification_fingerprint sha256:ee05de94dab1d6a7753ab88fba5a3b73f9a88f292c012ac6821535325ea406ca; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:4ac5523bc556c02252a6b668fe8af67ce2291a6738b3eb92285034765622b5d8, verification_fingerprint sha256:510535d9cb71facb7149368f59c19acd7a2e7c1c3cce5253ded2af1a8d1927cf; siderita build: complete-production.sh exit 0, manifest source_fingerprint sha256:41961220fa5cf9dc774e80fac2444fd1a638d1e05abdada292cc2169eb7f42e3, verification_fingerprint sha256:be64293cd712c6a9caced4863438bdf535e8acf274530f6de64cce0d7612ce02; magnetita build: complete-production.sh exit 0, manifest source_fingerprint sha256:d449f2dac276377685eef183809ffe4d5c0bef23e8261bdfaabd03307c9a5a99, verification_fingerprint sha256:5b71c8e69bbeb1458d5a8645bfeb4005404479189984de7b9cd8f0ec78037da4; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:1a3ab6c6f14e4c17055fc0ce34a28982b54ee41f3862de330573bcaf96b1fead, verification_fingerprint sha256:9dc348d4d675183482dc32a6a9158e272282f3121308f9e69ae0b2a9abbb87ec; grafita build: complete-production.sh exit 0, manifest source_fingerprint sha256:e9efd5f1ca9ed03a3342f931649aad20f9a985c94047e5631aaa38e85a0b8a1f, verification_fingerprint sha256:340a370d179c149466f0baf73732d0b9634480f3b4310de106613d34106cf1ee; fluorita build: complete-production.sh exit 0, manifest source_fingerprint sha256:2d0c3004eb0d47d098bac6fb720efdd84b6ef735a639d80c4f4798307b1bf858, verification_fingerprint sha256:aeece668ed434e27191e0155388e83465c65151c776644b78326895985b0c157; hematita build: complete-production.sh exit 0, manifest source_fingerprint sha256:447b0da876e9635b1c0acc703416147ab76a569d39982d76d9bc4d5149eb9415, verification_fingerprint sha256:c6c09f9eed380893a1955dde8642e51d9e0a9acd88b5bf1f546c6574a6176778
