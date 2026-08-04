# Evidence: GOV-2 guard and contract alignment

- **Date:** 2026-08-03
- **Scope:** GOV-2-A against
  `a019c1363435c26f3a50e09ad5b0d33a9e4c27df`; the audit inherited the complete
  GOV-2 worktree and reviewed it as one cross-cutting unit
- **Environment:** Linux 7.1.5-1-cachyos, Python 3, Git worktree on `main`,
  hooks enabled through `core.hooksPath=.githooks`
- **Artifact:** the seven registered build scripts changed to delegate execution
  and sealing to the artifact runner, so their existing manifests are
  intentionally stale. No release artifact was rebuilt or deployed. Before
  that final production-input
  change, all seven manifests had been reverified and their installed state was
  inspected separately.

## Procedure

The final guard and fixture pass was:

```sh
ARCHITECTURE_COMPARE_REF=HEAD bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
LANGUAGE_COMPARE_REF=HEAD python3 scripts/check-language-contract.py
bash scripts/test-architecture-scanners.sh
bash scripts/test-documentation-contract.sh
bash scripts/test-commit-scope.sh
bash scripts/test-staged-units.sh
python3 scripts/test-version-contract.py
python3 scripts/version_tool.py check
python3 scripts/version_tool.py show
python3 scripts/audit-version-commits.py
bash scripts/test-production-artifacts.sh
bash scripts/test-production-common.sh
git diff --check
```

The final artifact state was also inspected without building, verifying,
deploying or activating any project:

```sh
for project in celestina-style celestina-rs celestina siderita magnetita grafita fluorita; do
    "$project/scripts/status-production.sh"
done
```

All seven status entries returned 1 with both the production and verification
fingerprints stale, which is the expected result after changing their registered
production inputs.

The production-artifact fixtures contain twenty-seven positive and negative cases.
They prove that all seven build/verify entry pairs delegate to the runner, that
the removed start/record commands cannot seal directly, and that an internal
entrypoint cannot seal itself. They reject missing lifecycle scripts, nonzero
registered children, source changes during a build, and source, artifact or
verification-input changes during a verification child. A failed re-verification
also clears the prior seal.
The commit-scope fixture uses isolated Git repositories and real staged blobs;
it covers exact architecture and language reductions, mismatched values,
missing source changes, foreign prefixes, HEAD/INDEX authority intersection,
committed-rule interpretation of staged data, ignored staged `SystemExit(0)` and
`os._exit(0)` code, canonical versus lookalike evidence roots, merge
ratchet/source restrictions, full debt removal, and deletion of a source with
durable architecture-resolution evidence. Subject fixtures reject Spanish,
mixed-language, gerund and noun openings while an explicit scope-only mode
replays inherited history. Staged-unit fixtures prove that neither an unstaged
nor staged registry can hide an invalid inventory, and conflicting HEAD/INDEX
layout ownership fails. The architecture fixture also uses an isolated Git
history to prove that the real history command accepts suite and matching-owner
evidence while rejecting nested lookalikes.

The version fixture contains twenty cases. It proves strict Cargo, Cargo.lock
and CMake source parsing; exact PATCH, MINOR and MAJOR transitions; unchanged
maintenance; append-only history; product and multi-product suite ownership;
component rejection; baseline adoption; committed-HEAD hook loading; wrapper
and revert behavior; synchronized source/mirror updates; and a temporary Git
history where the legacy adoption commit and a typed maintenance commit pass
while a later untyped commit fails. The worktree history audit reports its
truthful pre-adoption state until this unit lands; after adoption it audits every
reachable non-merge commit against its first parent.

Before runner supervision changed registered production inputs,
every registered artifact was reverified without invoking a release build or
deployment:

```sh
celestina-style/scripts/verify-production.sh
celestina-rs/scripts/verify-production.sh
celestina/scripts/verify-production.sh
siderita/scripts/verify-production.sh
magnetita/scripts/verify-production.sh
grafita/scripts/verify-production.sh
fluorita/scripts/verify-production.sh
```

Those runs are historical evidence for the preceding verification-input state,
not a current-manifest claim. `celestina-rs` and Magnetita were verified outside
the filesystem sandbox so their loopback-network tests could open local sockets.
The initial sandboxed
`celestina-rs` run rejected seven such tests with `PermissionDenied`; the same
suite passed without the sandbox restriction.

## Result

- Every final guard and fixture suite above exited 0. The seven artifact
  verifications also exited 0 before the later build-script edit intentionally
  invalidated their manifests. The guard chain reports `Contrast contract: OK`,
  `QML visual contract: OK` and `Architecture contract: OK`.
- The version contract reports six aligned product owners: Celestina `0.1.0`,
  CelestinaStyle `1.0.0`, Siderita `1.0.1`, and Magnetita, Grafita and Fluorita
  `1.0.0`. These are adoption baselines read from registered declarations, not
  reconstructed historical milestone counts. The two CMake product versions
  were normalized from equivalent two-component spellings to strict `X.Y.Z`;
  QML import/API `VERSION 1.0` declarations remain separate and unchanged.
- Final subjects use the registered base scope plus `bug`, `milestone`,
  `release` or `maintenance`. The first three require the exact SemVer
  transition and matching append-only row in the same staged unit;
  `maintenance` forbids both. The ledger continues to record only the base
  scope. The adoption commit itself must use legacy `suite:` because HEAD owns
  interpretation; all later normal commits require the typed form, and CI
  replays them even when hooks were disabled.
- The language guard reports 178 ratcheted files, down from 196 at `HEAD`.
  `scripts/language-baseline.tsv` removes eighteen rows, adds none and raises
  none, so both the worktree guard and the history comparison accept it.
- Canonical rules, changed guard diagnostics and changed production entry
  points are English. Existing Spanish UI text and 178 explicitly ratcheted
  legacy paths remain outside this translation unit.
- `agent-context.py` now fails closed on missing registered context, reports
  shared rules plus lexical and resolved owners, and preserves both owners when
  a symlink crosses a project boundary. Positive, omission and escaping-symlink
  fixtures pass.
- Shared architecture and language ratchets are in every project/component
  scope. Normal commit authority is the intersection of HEAD and INDEX. Python
  rules committed in HEAD interpret both registry revisions and INDEX
  source/baseline data, so staged or unstaged rule modules cannot execute or
  authorize their own broader prefix or paths. Delivery discovery uses the
  conservative union of both registry layouts and rejects ownership conflicts.
  Removing a `lines` row requires a changed or deleted source and a durable
  evidence record in the primary owner's canonical evidence directory;
  component-local lookalike directories are rejected. Merge commits cannot
  change ratchets and their staged guarded sources must still equal the INDEX
  baselines. Normal, revert and fixup subjects require a recognized English
  imperative and conservative non-English screening; historical replay remains
  deliberately scope-only. Semantics-changing scanners therefore use a
  compatible two-commit migration: dormant implementation first, activation
  and measurement adjustment after it becomes HEAD.
- Deployable artifacts fingerprint both the suite and project completion
  orchestrators. Every project also requires its registered verify and status
  entries; deployable projects require deploy, and any declared activation is
  fail-closed. Changing, deleting or unregistering a required entry invalidates
  verification and cannot be resealed. Nondeployable libraries do not gain a
  completion dependency.
- `production_artifact.py` captures the initial state and invokes exactly the
  registered build or verify script in a reserved internal mode. It writes a
  build seal only after that child exits zero with unchanged production inputs,
  and a verification seal only when source, artifact and verification state are
  unchanged. Public start/record commands no longer exist, internal entrypoints
  cannot seal themselves, and all twenty-seven artifact fixtures pass. This
  proves successful execution of the registered entrypoint, not that its
  internal command list is semantically complete.
- Project exits now distinguish implementation closure from author validation.
  A deployable bug or milestone closes through `complete-production.sh`, which
  builds once, verifies those exact bytes, deploys the normal author-test
  destination and reports status. Shell activation remains a separate author
  action. Shared-library work completes its affected deployable consumers.

The installed-state audit before the runner edit was intentionally read-only:

| Project | Artifact state before runner edit | Installed author-test destination |
|---|---|---|
| `celestina-style` | current and verified | not deployable |
| `celestina-rs` | current and verified | not deployable |
| `celestina` | current and verified | missing: shell, both adapters, style library/module and launcher |
| `siderita` | current and verified | different from the verified artifact |
| `magnetita` | current and verified | both client and daemon differ from the verified artifact |
| `grafita` | current and verified | different from the verified artifact |
| `fluorita` | current and verified | different from the verified artifact |

The five deployable status commands therefore exited 1, as designed. The later
runner edit changed every registered build script, so all seven manifests
now additionally fail their production fingerprint until the next real release
build. This is not treated as a current installed-binary claim and no hidden
rebuild repairs it.

## Limits

- No release build, deployment, shell activation, user-service restart, XDG
  installation, commit or push was performed. Earlier verification compiled
  test/debug targets and ran offscreen smoke where the project defines it; it
  did not replace any installed author-test binary.
- Strict normalization of the Celestina and CelestinaStyle CMake product
  declarations is governance adoption, not a new product release. It remains a
  changed production input and is deliberately not hidden behind an unrecorded
  rebuild.
- Build, verification and deployment do not yet hold a suite-wide lock. Deploy
  atomically replaces one file or tree at a time, not a whole multi-artifact
  bundle, so concurrent mutation or a mid-deploy failure can leave a partial
  installation. The completion workflow fails at that point and does not claim
  success; it may stop before reaching status. No transactional rollback claim
  is made.
- Installed status compares only registered artifact mappings. Desktop files,
  generated icons, portal descriptors, service templates and permission bits
  are not all represented, so a successful status is an exact-content claim for
  the mapped artifacts, not a complete audit of every installed resource.
- Shared-module consumer fan-out remains a mandatory agent rule rather than an
  executable registry relation. This unit does not prove proportional completion
  of every deployable consumer after a shared crate or style change.
- Passing compilation and smoke do not prove real Wayland interaction,
  appearance, portal behaviour, hardware integration or assistive-technology
  behaviour. GOV-2 carries no author-validation checkpoint because it makes no
  such claim.
- The language detector was not hardened. Its current measure still ratchets
  178 legacy paths, and success does not mean every historical diagnostic,
  source string or Spanish UI label was translated.
- The GitHub workflows exercise contracts, the Rust workspace and shell Rust
  helpers. They do not compile the shell Qt/C++ host, the CXX-Qt applications
  or the style module; those build matrices remain local verification, as the
  CI map now states.
- Git hooks are repository-integrity controls for normal agent and author
  workflows, not an adversarial sandbox or a replacement for protected remote
  review.

## Follow-up

The single unit in
[the archived GOV-2 plan](../plans/archive/2026-08-03-guard-contract-alignment.md)
is closed by one inventory against the direct parent of its landing commit. The
excluded policy decisions and production-hardening fronts remain separate
future work; they are not hidden unfinished milestones inside GOV-2-A.
