# Evidence: repository governance and reusable production artifacts

- **Date:** 2026-08-03
- **Scope:** GOV-1 across the suite and all seven registered projects
- **Environment:** Arch Linux checkout at `0bdec97f382327b5a116462ce9e4910a36a00803` with uncommitted GOV-1 changes; Rust/Cargo 1.97.1, GCC 16.1.1, CMake 4.4.2 and Qt 6.11.1
- **Artifact:** seven `production-artifact.toml` manifests; immutable fingerprints and artifact digests are recorded below

## Procedure

The canonical production entries registered in `docs/projects.toml` were run
with these exact commands:

```sh
celestina/scripts/build-production.sh
celestina-style/scripts/build-production.sh
celestina-rs/scripts/build-production.sh
siderita/scripts/build-production.sh
magnetita/scripts/build-production.sh
grafita/scripts/build-production.sh
fluorita/scripts/build-production.sh

celestina/scripts/verify-production.sh
celestina-style/scripts/verify-production.sh
celestina-rs/scripts/verify-production.sh
siderita/scripts/verify-production.sh
magnetita/scripts/verify-production.sh
grafita/scripts/verify-production.sh
fluorita/scripts/verify-production.sh

for project in celestina celestina-style celestina-rs siderita magnetita grafita fluorita; do
    python3 scripts/production_artifact.py check "$project" --require-verified
done
```

The installed-state comparison was run read-only with:

```sh
for project in celestina celestina-style celestina-rs siderita magnetita grafita fluorita; do
    "$project/scripts/status-production.sh" || true
done
```

The repository contracts were then checked independently:

```sh
bash scripts/test-production-artifacts.sh
bash scripts/test-documentation-contract.sh
bash scripts/check-documentation-contract.sh
bash scripts/test-commit-scope.sh
bash scripts/test-staged-units.sh
bash scripts/test-architecture-scanners.sh
bash scripts/check-architecture-contract.sh
find . -path './.git' -prune -o -path './.claude/worktrees' -prune -o -type f -name '*.sh' -print0 | xargs -0 -n1 sh -n
git diff --check
```

## Artifact identities

| Project | Built UTC | Verified UTC | Source fingerprint | Verification fingerprint |
|---|---|---|---|---|
| celestina | 2026-08-03T18:32:07Z | 2026-08-03T18:45:21Z | `sha256:08ee185e3dd41fdf7c6855ff86fdf7d151cd1b715534eea2d5f21932f28fd955` | `sha256:38333fbaaf75e347ca9c60ae8441c28d97c062750bde39e571f68eb7b314e5aa` |
| celestina-style | 2026-08-03T18:32:05Z | 2026-08-03T18:45:09Z | `sha256:12d1acc8c85975671f13fee277ada5ebca1b56ff411bff9666785e5f5ef9f33a` | `sha256:a7307be781e4db646bd9027f642ed36b7514f2515ca43772b2222a5c50fe415a` |
| celestina-rs | 2026-08-03T18:40:10Z | 2026-08-03T18:43:17Z | `sha256:660f6abcc80bcab34487bdee8b85de187a46b71d800fa56bfd780d64b315fd48` | `sha256:cd441f0ee95306727da14deb7f97bfd3710bcd93ff5814e9f43e8a02c4c5467e` |
| siderita | 2026-08-03T18:40:43Z | 2026-08-03T18:47:03Z | `sha256:b7915813b94e7d226cb82b60c92043471653b57a82bc7813e603771c42da218e` | `sha256:8fe32a1660fa79835030d2ddd23eeb33ffaac772c31b6284a08da210e3dfec1a` |
| magnetita | 2026-08-03T18:41:02Z | 2026-08-03T18:41:41Z | `sha256:6a58b5f8018329309364fee712a40313d3a8bfd125630b6be5126607068defd5` | `sha256:a279f32e7de3f1680dcfe8eaa11a1c1e319a9125d408bf65f62703eb4a18f7f2` |
| grafita | 2026-08-03T18:33:36Z | 2026-08-03T18:42:12Z | `sha256:1d58aa004fb88919f06cf133978a5ab0e7184e52f3f952c8319202ae625ce5ad` | `sha256:a4506a74ae88c578284daec9cc414413b769bfa9babec3b05574caa943e6241e` |
| fluorita | 2026-08-03T18:33:54Z | 2026-08-03T18:42:56Z | `sha256:fb4e94d9963e1017600cc60d21978a5a93c247f94226b0cda6d55f657f19eabf` | `sha256:6de829f4a149800020dc8f80bd942425c1d6d6c07e86b6b2bec79ff2061efef0` |

Artifact digests:

- `celestina/build/celestina`: `sha256:ea12967bf3b04bf4794a96b8208af63b97c4f44d52fc119c50f1e8d5b391f40d`
- `celestina/build/rust-target/release/celestina-niri-adapter`: `sha256:13f57bff551b76942a6743db687b6578baf8be1515b930bff6cd9e332898bdc9`
- `celestina/build/rust-target/release/celestina-provider-adapter`: `sha256:668001bab1a14c1afae0f6e0103c67bb95952d0b8013776fb14a111eee16135d`
- `celestina-style/build/libcelestina-style.so`: `sha256:a6930a77c3cfc7c2b2523692ec27f1bae2a530c14f272c03bfcdc15756a742dc`
- `celestina-style/build/CelestinaStyle`: `sha256:1c112d5374313be67f766cd0fbbf07f6e0715ef84a1789b98d2f90e6e1dca020`
- `celestina/scripts/celestina-launcher.sh`: `sha256:1a12d71f80ff8ce1b71f5fd92051256a262bdf81f2a22d305330293b233ca3d2`
- `celestina-rs/target/release/magnetitad`: `sha256:2dc2ecfa8536c4af58bdef5af67156e58ec8e998b8161613e6083b3ef52db6e3`
- `siderita/target/release/siderita`: `sha256:ad39fd8cd2da156ab7463b92846efe812cf442d68b36039908895cbe441276aa`
- `magnetita/target/release/magnetita`: `sha256:f8177943197420cc813f9cd3c4cd79c7f1a2a56cbc93a510a2dfd3c1bea92e20`
- `grafita/target/release/grafita`: `sha256:64707bf8fba8bcc2aa4bb999541ab1bf928e95ec1f33227dbc08e3daf8d2d0b7`
- `fluorita/target/release/fluorita`: `sha256:76ea57057f0c4b04e8bcd6f6b60fac9841f456e1aee23f360809ab0ea350b237`

The exact GOV-1 checkout delta is preserved as a no-rename per-path numstat and
content-hash [inventory](../inventories/2026-08-03-repository-governance/GOV-1.numstat.tsv).
Only the inventory's own row uses the `self` marker because hashing that row
would be recursive. The archived plan row records its final SHA-256 normally.

## Result

- **Exit:** all build, verify, fixture, guard, syntax and diff checks exited 0.
- **Observed:** each verify entry completed before sealing its manifest; the
  final `--require-verified` pass accepted all seven source fingerprints,
  artifact digests and verification fingerprints.
- `celestina-style` was built at 18:32 UTC and verified at 18:45 UTC. Its
  artifact contains `libcelestina-style.so` and the complete compiled
  `CelestinaStyle/` module.
- `celestina` was built at 18:32 UTC and verified at 18:45 UTC. Its manifest
  seals the Qt host, both Rust helpers, style backing library, compiled style
  module and production launcher. The final CTest pass was 11/11; direct
  `qmllint`, offscreen host/module smoke and dynamic-library checks passed.
- `celestina-rs`, Siderita, Magnetita, Grafita and Fluorita were built and
  verified between 18:33 and 18:47 UTC through their registered Rust, CXX-Qt,
  QML and smoke matrices.
- The first sandboxed Rust-workspace pass could not open seven loopback sockets
  (`EPERM`). The same locked test matrix was rerun with approved unrestricted
  execution and passed; this was an environment restriction, not a product
  assertion suppressed from the result.
- Siderita's `qmllint` completed with 326 non-fatal warnings from the existing
  baseline. The warnings were not converted into a false clean-lint claim.
- Production-contract fixtures passed 6 Python cases plus the shell common
  helper cases, including stale fingerprints, atomic replacement, outer-trap
  preservation and interrupted tree-swap restoration.
- Documentation/context, commit-scope, exact staged-unit and architecture
  positive/negative fixtures passed. The delivery matrix covers compatible
  units sharing a plan; exact paths, numstat and hashes; partial staging with
  titled Markdown links; orphan/non-`done` inventories; owner and subject-prefix
  mismatches; incompatible multiowner batches; delivery attempted inside a
  merge; immutable inventory endpoints; and an `active` -> `archive` transition
  with a distinct administrative unit.
  The root `CLAUDE.md` was removed; its remaining basename is only a negative
  fixture. Registered `.claude/worktrees/` was not modified.
- The installed-state audit was read-only. No Celestina shell bundle is present
  under `~/.local`; the installed Siderita, Magnetita, `magnetitad`, Grafita and
  Fluorita bytes differ from the newly verified artifacts. Shared style and
  Rust workspace are intentionally nondeployable. The five deployable status
  commands returned 1 as designed for absent/different bytes; the two
  nondeployable status commands returned 0.

## Limits

- Nothing was installed, deployed, activated, restarted or written into the
  live XDG/session paths during this governance-only migration. The resulting
  contract requires future app bug fixes and milestones to run their registered
  `complete-production.sh`: build once, verify, deploy to the normal author-test
  prefix and confirm the installed bytes.
- Offscreen smoke proves startup and QML loading, not real Niri layer-shell,
  blur, focus, output hotplug, hardware, phone behavior or AT-SPI.
- The shell smoke enters `--pick-output`, so it exercises
  `CELESTINA_STYLE_PATH` but returns before constructing `NiriClient` and
  `ShellProvidersClient`. Their helper-path overrides are compiled and match the
  launcher/activation contract, but do not yet have a direct runtime test.

## Refactor and reuse contract refinement

- Replaced the universal 800-line acceptance boundary with semantic extraction
  criteria: one canonical owner, a named responsibility, narrow typed API,
  acyclic dependency direction, removal/delegation of the old path and boundary
  tests.
- `scripts/architecture-baseline.tsv` now acts only as a non-growth ratchet for
  specifically recorded legacy coordinators. New source files are not accepted
  or rejected by line count alone.
- `bash scripts/test-architecture-scanners.sh` and
  `bash scripts/check-architecture-contract.sh` passed after the refinement.

## Repository language contract

- Rewrote the root and all seven local `AGENTS.md` contracts, governance,
  engineering standards, production contract, contribution guide, current
  templates, suite validation, and cross-suite CI documentation in English.
- Added `docs/standards/language.md` as the single policy source: repository
  content is English while agents communicate with the author in Spanish.
- Added `scripts/check-language-contract.py` and hermetic fixtures. Canonical
  sources have zero detected Spanish prose; 196 legacy code/UI files are held by
  an exact non-growth baseline that must decrease and cannot accept new paths.
- Explicit localization resources, locale-qualified desktop entries, and marked
  international-input fixtures are the only current-content exceptions.
- The existing build/target caches were reused and not cleaned. Their size and
  cleanup policy were not changed by GOV-1.
- No commit or push was performed.

## Follow-up

`VAL-GOV-1` remains pending in the independent author-validation lane. Product
manual checks remain in each project's `VALIDATION.md`; they do not reopen
GOV-1.
