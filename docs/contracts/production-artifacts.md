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
- Reuse `target/` and `build/`; do not run `clean` by default.
- Do not create an agent-only target when the canonical target can be deployed.
- Build all binaries deployed by the project as one unit without starting
  processes or reloading services.
- Write the ignored manifest at the registry-declared path.

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
