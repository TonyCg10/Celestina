# Reusable production artifact contract

## Objective

The agent builds, verifies, and deploys the same profile and bytes the author
runs. When a deployable app bug or milestone closes, the normal author-test
binary already contains the verified bytes; the author does not rebuild.
Standalone verification does not touch the installation or a live session.

`docs/projects.toml` registers each project entry. Every buildable project has
build and verify; deployable projects additionally have deploy and complete.

| Entry | Responsibility | May compile | May install/activate |
|---|---|---:|---:|
| `build-production.sh` | build canonical release artifact and manifest | yes | no |
| `verify-production.sh` | check exact artifact, tests, and safe smoke | must not repeat release build | no |
| `deploy-production.sh` | copy a verified artifact to the normal prefix | no | deploy only |
| `complete-production.sh` | chain build, verify, deploy, and status | once | canonical app exit |
| `activate-production.sh` | start or replace a live surface | no | explicit request only |
| `status-production.sh` | compare manifest, source, and deployed copy | no | no |

`run.sh` preserves its historical human interface, including any old mixture
of build, installation, removal, or activation. It is not canonical agent
evidence. Agents use `complete-production.sh` so the author does not need a
later build or deploy.

A library workspace or shared module does not invent an installation. It uses
`deployable = false`. When it changes in an implementation, complete and deploy
every affected deployable consumer; verifying only the library does not update
the author's binary.

## Single build

- Use the release profile and options for the distributed artifact.
- Reuse `target/` and `build/` between a build and its deploy; do not run
  `clean`. After a landing deployed and checked its artifacts, its `prune`
  step removes the debug profiles as
  [Pruning after deploy](#pruning-after-deploy) says.
- Do not create an agent-only target when the canonical target can be deployed.
- Build all binaries deployed by the project as one unit without starting
  processes or reloading services.
- Write the ignored manifest at the registry-declared path.
- Register each artifact path for one project only. A workspace build that
  also produces a binary a deployable project builds with other Cargo features
  writes to its own target directory, so neither build overwrites the bytes
  the other one recorded; the documentation contract refuses a path that two
  projects register.

The manifest records at least project, profile, artifact paths and digests, Git
revision, dirty state, production and verification fingerprints, relevant
toolchain, UTC time, and the supervised build/verify entrypoints. A changed
source fingerprint invalidates deployment; it never triggers a hidden rebuild.
A change limited to tests, smoke, guards, deploy, activate, status, or shared
helpers invalidates verification and reruns verify without rebuilding release.

The no-argument `build-production.sh` is the canonical user-facing build entry.
It delegates to `scripts/production_artifact.py run-build`, which captures the
production fingerprint and then invokes exactly the registry-declared build
script once in a reserved internal mode. That child runs the real Cargo or CMake
steps without writing a manifest. Only after the child exits zero does the
runner recheck production inputs and artifact stability and write the pending
manifest. There is no public start/record pair and an internal child cannot seal
itself. A nonzero child or changed interval leaves no new build seal.

`scripts/production_artifact.py` computes fingerprints from registered
`production_inputs` and `verification_inputs`. It follows source symlinks so a
shared QML change invalidates consumers, ignores targets/builds/VCS caches, and
never uses mtimes for artifact identity.

Three shared verification inputs are hashed by the project's slice, not whole:
`docs/projects.toml` by the project's own table and `[commit_policy]`,
`scripts/architecture-baseline.tsv` by the rows of paths under the project's
`commit_roots`, and `scripts/qmllint-baseline.tsv` by the project's row. Bytes
that cannot be read as that format count whole. A unit that lowers one
project's ratchet row or edits one project's table therefore re-verifies that
project, not every registered one; the guards themselves still judge the whole
files on every commit and landing.

The verification fingerprint requires `verify_script` and `status_script` for
every project; deployable projects additionally require `deploy_script`,
`complete_script`, and the shared completion orchestrator. A declared activation
entry is required too. Deleting or unregistering any required lifecycle script
invalidates verification and cannot be resealed until the contract is restored.

The canonical no-argument `verify-production.sh` similarly delegates to
`scripts/production_artifact.py run-verification`. The runner validates the
current build, captures a digest over the source fingerprint, complete artifact
set and current verification fingerprint, and invokes exactly the registered
verify script in its reserved internal mode. It clears any prior verification
seal before launching the child, so a failed re-verification cannot leave old
success looking current. It marks the manifest verified only after that child
exits zero and the complete digest is unchanged. A source,
artifact or verification-input change during the child leaves the manifest
unverified. The removed public start/record commands cannot bless a prior or
unrelated execution by supplying descriptive text.

This supervision proves that the registered entrypoint returned success over an
unchanged interval. It does not prove that every command implemented inside that
entrypoint is semantically sufficient; review and fixture coverage still own
that contract.

## Complete production inputs

A fingerprint hashes only what the registry declares, so an input the registry
forgets is a fix that never reaches the installed binary while `check` and
`status` report it current. Three rules close that gap:

- A buildable project declares a non-empty `production_inputs`. The runner
  refuses `run-build` and `check` for one that does not, and the guard below
  reports it.
- A project declares in `cargo_manifests` every Cargo manifest its build
  script compiles: its own `Cargo.toml`, which the guard requires whenever
  the project directory holds one, and any other, such as Magnetita's
  `magnetitad` or the `magnetita-mobile` library the Android APK packs. A
  virtual workspace manifest stands for all of its members.
- `python3 scripts/production_artifact.py check-inputs` asks
  `cargo metadata --no-deps --offline` for each declared manifest's
  workspace, follows every normal and build path dependency (renamed and
  workspace-inherited keys included; development dependencies are not
  linked), and requires the production inputs to hash: every path package
  outside the project directory whole, since its build script, C++ sources
  and included files reach the artifact too; the project's own packages by
  manifest and by the source of each target a release build compiles; the
  workspace manifest each package inherits from; the lockfile of each
  declared manifest's workspace; and the nearest `rust-toolchain.toml` or
  `rust-toolchain` above each declared manifest.
  `scripts/check-architecture-contract.sh` runs it, so the architecture
  guard fails in CI and at landing when a dependency is added without
  registering it.

The closure is guarded, not derived at fingerprint time. The fingerprint stays
a pure function of the declared inputs: it never asks Cargo for the graph, so
two runs over the same bytes always agree and `check`, deploy and status cost
no metadata call, while the landing's affected-project matching and
`agent-context.py` read the same list that the guard proves complete.
Deriving the closure at fingerprint time would give those readers a second,
invisible input set.

The manifest records the probes of the toolchains the project's build uses,
which its registry table declares in `toolchains`, drawn from `rust` (`cargo`
and `rustc`), `cmake`, `cxx` (the C++ compiler), `qt` and `jdk`; a project
that declares none is probed for all but `jdk`, and the production-input guard
refuses an unknown name. A Qt or CMake upgrade therefore stales only the
artifacts built with them, not a pure-Rust library or the APK, and a probe an
older manifest recorded but the project no longer declares is not compared.
rustup chooses the compiler by the directory Cargo runs in,
so the Rust probes run in the directory of each declared Cargo manifest, or
in the project directory when it declares none: Magnetita records its app's
compiler in `magnetita/` and its daemon's, which `celestina-rs/` pins, as
`rustc@magnetita` and `rustc@celestina-rs/crates/magnetitad`. Every probe
has a timeout. `check` compares them: an artifact another compiler or Qt produced is stale
and needs a rebuild, not a verification. The manifest also records one digest
per expanded production and verification input. A failed `check` keeps its
one-line error, which the landing reads, and prints on the following lines
what changed: each new, removed or changed input, up to twenty, and each
toolchain probe with its recorded and current value.

## Exact verification

`verify-production.sh` receives or discovers the canonical manifest and:

1. fails when the manifest is absent, stale, or disagrees with artifact digest;
2. runs required guards, tests, and lint while reusing caches;
3. exercises release binaries directly when a safe mode exists;
4. updates verification evidence in the manifest;
5. writes no XDG prefix, activates no D-Bus/systemd unit, and replaces no
   process.

Test harness compilation is valid. Repeating the distributed release build or
testing only a different binary is not.

## Deploy without rebuilding

`deploy-production.sh` consumes only a current verified manifest. It fails when
relevant input or artifact digest changed. It may accept `--prefix` for explicit
staging, but never runs Cargo or CMake. `status-production.sh [--prefix DIR]`
checks that installed files are byte-for-byte registered and returns nonzero
for missing or different copies.

Desktop databases, icon caches, and D-Bus reload belong to deploy, not build or
verify. Magnetita stops and restarts `magnetitad` only when it was already
active; deploy never enables an inactive service.

## Pruning after deploy

Most of a Cargo build tree is the debug profile that tests, clippy and quick
runs build: tens of gigabytes per application, none of which the installed
copy uses. The release profile is the incremental cache the next production
build reuses: a small change, such as a colour in a QML file, recompiles the
application crate and relinks instead of rebuilding every dependency.
`scripts/build_trees.py prune` therefore removes only the debug profiles:

- the build roots are the first `target` or `build` directory of each
  `artifact_paths` entry and `artifact_manifest` of every project that is not
  halted; a halted project's own tree is never touched;
- in each root it removes every `debug` directory directly under the root or
  one level below it, which covers a target triple such as
  `aarch64-linux-android/debug` and a nested target such as
  `celestina-rs/target/workspace/debug`; it never follows a symbolic link;
- everything else stays, release caches and CMake or Gradle trees included,
  and so does any registered artifact or manifest, so `check`,
  `deploy-production.sh` and `status-production.sh` keep judging the same
  bytes and a verification still finds the generated sources it reads, such
  as the release QML module `qmllint-cxxqt.sh` lints.

`--dry-run` lists what would go and how much it frees without removing
anything. The tool refuses to run in a session worktree, whose builds use the
shared session target instead; `scripts/worktree.sh close` removes that
target when it closes the last session. The landing runs the prune as its
last step; see [the landing contract](landing.md).

## Shell special case

Running the shell activates a real surface, so it has a separate activation
entry. Build produces the Qt host, all required Rust helpers, and the complete
CelestinaStyle module. Verify chains style verification, lints the generated QML
response without a build target that recompiles helpers, and runs an offscreen
host smoke with the built module.

Deploy updates the normal on-disk bundle. Activate is the only entry that starts
or replaces the session, and completion never calls it. The bundle installs the
host, helpers, `libcelestina-style.so`, and `CelestinaStyle/` under
`libexec/celestina`, with a stable `bin/celestina` launcher. Explicit
`CELESTINA_STYLE_PATH`, `CELESTINA_NIRI_ADAPTER_PATH`, and
`CELESTINA_PROVIDER_ADAPTER_PATH` select bundle contents while canonical build
paths remain fallbacks. `activate-production.sh --from-build` can later exercise
the same verified checkout bytes without rebuilding.

## Evidence and hand-off

The ledger records the manifest, `complete-production.sh`, and successful
status, not a copy of the binary. Hand-off names the updated destination and
whether an already-running process/session must restart to load new bytes.
Artifacts and manifests remain ignored by Git.
