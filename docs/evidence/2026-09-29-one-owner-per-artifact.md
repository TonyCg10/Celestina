# Evidence: one project per registered artifact path

- **Date:** 2026-09-29
- **Scope:** `AUD-1-G` of the
  [monorepo hardening plan](../plans/archive/2026-09-26-monorepo-hardening.md):
  `celestina-rs/scripts/build-production.sh`, the `celestina-rs` entry of
  `docs/projects.toml`, the registry check in
  `scripts/documentation_contract.py`, its fixture test and
  [the production artifact contract](../contracts/production-artifacts.md)
- **Environment:** session worktree `unit/suite/AUD-1-G` on the author's
  machine (Linux 7.2.8); Python 3.14.7, Git 2.55.0. No production entry ran
  in the session; the landing builds and verifies `celestina-rs`
- **Artifact:** `celestina-rs/target/workspace/release/magnetitad`, which the
  landing's `celestina-rs` build records

## Defect

`celestina-rs` and `magnetita` both registered
`celestina-rs/target/release/magnetitad`. The workspace build
(`cargo build --workspace`) unifies features across every member, and
Magnetita's (`cargo build -p magnetitad`) does not, so the two builds link
different bytes into the same file. In the landings of `MAG-D1-D` and
`MAG-D1-E` on 2026-09-29, the `celestina-rs` verification ran after
Magnetita's build and overwrote the binary, and the deploy stopped with
`artifact digest or set does not match the recorded build`; Magnetita had to
be rebuilt by hand before `land-unit.py --continue` could deploy it.
Afterwards `celestina-rs`'s own manifest no longer matched either.

## Change

- The workspace build writes to `celestina-rs/target/workspace`, and
  `celestina-rs` registers `celestina-rs/target/workspace/release/magnetitad`.
  Magnetita keeps `celestina-rs/target/release/magnetitad`, which its deploy
  and status entries read.
- The documentation contract refuses an artifact path that two projects
  register, naming the path and both projects. A halted project is left out:
  no landing builds it, so it overwrites nothing. On `origin/main` before this
  unit the check reports the `magnetitad` path; the halted shell's
  registration of the two `celestina-style` artifacts is not reported.

## Procedure

```sh
sh scripts/test-documentation-contract.sh
sh scripts/check-documentation-contract.sh
python3 scripts/documentation_contract.py --root <canonical checkout on origin/main> --quiet
bash scripts/test-architecture-scanners.sh
sh scripts/test-production-artifacts.sh
python3 scripts/test-production-artifacts.py
sh scripts/test-production-common.sh
python3 scripts/test-land-unit.py
sh scripts/test-staged-units.sh
bash scripts/test-commit-scope.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py check
```

The new fixture case was first run against `origin/main`'s
`documentation_contract.py` (RED), then against this unit's (GREEN).

## Result

- **RED:** `test-documentation-contract.sh` failed with
  `the registry accepted one artifact path registered by two projects`.
- **GREEN:** every command above exited 0 on the final tree, except the run
  against `origin/main`, which reported, as intended,
  `artifact path celestina-rs/target/release/magnetitad is registered by both
  \`celestina-rs\` and \`magnetita\``.

## Limits

- The first workspace build in the new target directory compiles the whole
  workspace once; later builds reuse it.

## Landing

- **Base revision:** `4b380772de3b24305bedc2bbc21169beef815a17`
- **Check:** `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; missing production artifact: celestina-rs/target/workspace/release/magnetitad; artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again
- **Build:** celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:3b304f586f6100a053b1496a37e69ea0ece8f0783977303ad156831bb349d2b8, verification_fingerprint sha256:bd742464af5db29674e2acba6ab943e8cf0bf4b701a476608dc514b7d75280ba
