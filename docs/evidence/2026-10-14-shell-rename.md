# Evidence: the halted shell moves to celestina-qt and the Rust shell is registered

- **Date:** 2026-10-15
- **Scope:** CEL-1-A — suite
- **Environment:** CachyOS, Python 3.14.8, Git, Rust 1.97.1 (pinned by
  `celestina/rust-toolchain.toml`); the worktree of `unit/suite/CEL-1-A`,
  stacked on `unit/suite/CEL-1-A1` at `1d5a7ccb`
- **Artifact:** `celestina/target/release/celestina` (the 0.1.0 stub), built
  in the worktree, not installed

## Procedure

The CEL-1-A edit list of the plan (Task 5), as the A1 rehearsal corrected it
([the rehearsal record](2026-10-14-rename-rules.md)). The rehearsal record's
script rebuilt the staged rename in this worktree from its first `git fetch`
on (the clone and hook configuration steps do not apply to a worktree): the
fetched `rehearsal/cel-1-a` tip is `3313d261`, every step exits 0, and the
patch-id comparison proves the staged change equal to the rehearsal commit's.
Then the corrections the rehearsal commit does not carry:

- `.github/workflows/celestina.yml`, new, for the Rust shell: `name:
  celestina`, paths `celestina/**`, `celestina-rs/crates/celestina-shell/**`,
  `celestina-rs/crates/celestina-tokens/**` and the workflow itself; one job
  running `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
  --locked -- -D warnings` and `cargo test --workspace --locked` in
  `celestina/` on stable. The old workflow moved to `celestina-qt.yml` in the
  rehearsal steps.
- `.github/workflows/README.md`: the CI map names both workflows.
- `AGENTS.md` "Halted projects": the salvage paragraph as the plan words it,
  including "(there is no `bounded.rs`)".
- This record, and the CEL-1-A row of
  [the RSH-1 plan](../plans/active/2026-10-14-rust-shell.md), now `active`.

Everything staged with `git add -A`. The move is byte for byte: the tree of
`celestina-qt/` in the index equals HEAD's `celestina/` tree
(`d8e790de0248c4e7c53ccdb707df8a53e1ef7657`, 484 files).

The guards, from the worktree's root over that index; HEAD holds the A1
rules, which are the rules `main` will hold once A0 and A1 land:

| Command | Exit |
|---|---|
| `python3 scripts/check-staged-units.py --root .` | 0 |
| `git diff --cached --name-only --no-renames \| python3 scripts/commit_scope.py --check 'suite-maintenance: Move the halted shell to celestina-qt and register the Rust shell'` | 0 |
| `python3 scripts/version_tool.py check` | 0 |
| `bash scripts/check-architecture-contract.sh` | 0 |
| `python3 scripts/check-language-contract.py` | 0 |
| `bash scripts/check-documentation-contract.sh` | 0 |
| `bash scripts/test-commit-scope.sh` | 0 |
| `sh scripts/test-documentation-contract.sh` | 0 |
| `python3 scripts/test-land-unit.py` | 0 |
| `bash scripts/test-architecture-scanners.sh` | 0 |
| `sh scripts/test-staged-units.sh` | 0 |
| `python3 scripts/test-version-contract.py` | 0 |
| `cd celestina && cargo build --release --locked && ./target/release/celestina --check` | 127 as written in the worktree; 0 with `CARGO_TARGET_DIR=target` (see below) |
| `celestina` without arguments | 2 (`the shell is not delivered yet`) |
| `cd celestina && cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` (the new workflow's steps, `RUSTUP_TOOLCHAIN=stable`, Rust 1.98.1) | 0, 0, 0 |

The build itself exits 0; `./target/release/celestina` is absent in a
worktree because `scripts/worktree.sh` writes `.cargo/config.toml` with the
session's shared `target-dir`, where the stub's `--check` also exits 0. With
that override removed (`CARGO_TARGET_DIR=target`) the command exits 0 as
written; the canonical checkout the landing builds in has no such override,
so `artifact_paths` (`celestina/target/release/celestina`) holds there.

The landing tool has no dry run, so the landing evidence the design's §9 asks
for is: (a) the rehearsal record linked above, where the rename was committed
through the hooks in a scratch clone and its stub built, verified and
deployed into a scratch prefix; (b) the hook chain
(`pre-commit`, `commit-msg`) that the author's own commit runs over this exact
index; and (c) `python3 scripts/test-land-unit.py` on the branch, above.

## Result

- **Exit:** every guard above exits 0; the build-and-check exits 0 once
  the worktree's shared target directory is accounted for.
- **Observed:** the staged-units guard accepts the 63 inventories and 21
  plans as moved with their renamed owner (`staged-unit: 63 inventory file(s)
  and 21 plan(s) moved with their renamed owner`); the version contract
  reports `OK (12 owners)`, `celestina-qt` carrying the rows named
  `celestina` through `history_owner` and `celestina` its new `0.1.0
  baseline` row; the language guard reports 131 ratcheted files.

## Limits

- Nothing is installed or activated; the stub binary only answers
  `--check`. The landing builds, verifies and deploys it to
  `$prefix/bin/celestina`; activation refuses until CEL-1-B.
- The new `celestina.yml` workflow has not run in CI; its three cargo
  commands pass locally under the pinned toolchain.

## Follow-up

CEL-1-B in [the CEL-1 plan](../../celestina/docs/plans/archive/2026-10-14-cel-1-island.md).

## Correction (2026-10-15)

The new `celestina` is registered `versioned = false` and no history row is appended: the landing's `unbump` step returns `docs/version-history.tsv` to the fork point's bytes, so a baseline row cannot ride a landed unit. The author adopts the 0.1.0 version by hand after this unit lands (registry `version_source` + mirror and the baseline row in one `suite-maintenance` commit on `main`, as Cuprita's registration did), before CEL-1-B lands as a `celestina-milestone`.

## Landing

- **Base revision:** `866d343d257642abfb727fa60cd3fb95075661a5`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: missing celestina/target/production-artifact.toml; run celestina/scripts/build-production.sh first; `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check calcita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check selenita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:82c748ef089c27cba37543221c5ee5230c7a2310822e67a23886dc66800fc187, verification_fingerprint sha256:f02e935d007fcd5e973750f786ec0dc8d05bb1e50b7cc6e6efcf160d3f411532; celestina-style verify: verify-production.sh exit 0, manifest source_fingerprint sha256:469c2a323a83a4dffc40a6ca0fe6c75e2e4ee133fb090a09888b5ac8450d1027, verification_fingerprint sha256:3a7c5e8c00becb9c41ccf06a4a987e3348a5e3c17c93829072a94a55d0245749; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:84bee6074e1cf63b966fcd2dc9e826c43ea1f341449c77a1bf95319331dac8c9, verification_fingerprint sha256:1eeb39e1e6a148222f900ae15d15b48894f9053f3528563c9cd6fe1b770f0918; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:834b4cf8733cf1ce1be1f92e276f465d3b24b69f55267cb8ddc22613e752a373, verification_fingerprint sha256:2bdc6ed978d5d975888c7d44b3442f906663e963c7259c28d1f210afc3c101db; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:e4e877575323af62fef45a254c488366a291129fca5f762c8077e4e062025069, verification_fingerprint sha256:a6ef2396671ef9eed2a381eb7939a80b3673eb3c048a7585396420711b73900e; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:9c519febe914826c7c733aeae945b64580bea32d052a618a456a69ce57ecccf9, verification_fingerprint sha256:e035e55f4399bb5a1ae35a1f469ab19a470e26fd27cb1e71d2b5278c26da2002; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:c29f2ffbbe469ccc859b9225714586465c3c1f42363089e5c240e384120ebe8b, verification_fingerprint sha256:f8d57c78ef0aceadc7622511fc27343a3a170fab636c01df5c05fccb8bd685a1; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:91fd5979c57823c96f5ddf5c3fc7a10d135dd7408780074e9e45129bcd365316, verification_fingerprint sha256:25ee28d4c4f21095eb20452c7309ace0033716023d32c21325e09f684ee856bb; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:12e531e8f7da12bb275da059e4bcc61603788fdc22cee3b249fa71ba230eb490, verification_fingerprint sha256:f3a13d03abdd8aed1f0353da78199f14bad3ab7929202b9fdb3ee2c5dfc62388; cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:00be137c06f1282e264ce7072fa50f0af20b6c61fefc04f564a990e6b04e69fa, verification_fingerprint sha256:270a0aaf5825e3f6e1a9666c6dff0e54f75ca3c4cf035830bef8d1f9be6813a5; calcita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:0f65e46e69e0c73992b6d70b10100b5799775d46429c9c5820d6993fc1f10f68, verification_fingerprint sha256:7cdb32ad5aa5d987317e3efa40ae8a00f20988bede3649ce60b26882f914eec0; selenita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:e713f203aab38a299e5973d9f455c68c15e5eb90e8ef71f1d93bb61c58f8ef76, verification_fingerprint sha256:b75e6fb082e8cb1a51f25d5ebd7235d04ad9e95e9fe3467f82905ba1a3e48362
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh; calcita: deploy-production.sh, status-production.sh; selenita: deploy-production.sh, status-production.sh
