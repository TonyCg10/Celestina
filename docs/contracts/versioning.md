# Product version and commit-kind contract

Celestina uses one typed commit subject and one SemVer transition for each
product delivery. The rule exists so a completed bug or milestone cannot land
while the executable still advertises an old placeholder version.

## Canonical state

`docs/projects.toml` registers how to read each product version. The declared
number remains in the product's Cargo or CMake manifest; the registry stores the
location and format, never a copied current number. Mirrors such as
`Cargo.lock` must agree with the canonical declaration.

`docs/version-history.tsv` is the append-only release ledger. Its first row per
product is a verified adoption baseline, not a reconstruction of unrecorded
history. Do not infer a fictional `0.5.4` from old milestones. Later rows are
added by the commit that changes the declared version and contain:

```text
owner<TAB>version<TAB>kind<TAB>unit<TAB>summary
```

Git supplies the author, date and commit identity. Earlier rows are immutable;
never edit, reorder or delete them.

## Subject and SemVer mapping

Final normal commits use:

```text
<registered-base-prefix>-<kind>: <English imperative>
```

| Kind | Exact transition | Example from `0.5.4` |
|---|---|---|
| `bug` | increment PATCH | `0.5.5` |
| `milestone` | increment MINOR and reset PATCH | `0.6.0` |
| `release` | increment MAJOR and reset MINOR/PATCH | `1.0.0` |
| `maintenance` | no version or history change | `0.5.4` |

For example:

```text
siderita-bug: Fix picker synchronization
siderita-milestone: Add the indexed search checkpoint
celestina-release: Publish the first stable shell contract
suite-maintenance: Align repository governance
```

The ledger and inventory keep the registered base authority, such as
`siderita:`. The `-bug` suffix is commit intent, not a second scope. A future
project such as Calcita must first register the base prefix and its version
source; its milestone subject would then be `calcita-milestone:`.

`maintenance` covers documentation, tests, refactors and tooling that do not
deliver new product behavior or a product defect correction. It is not a way to
avoid a required bump. A bug fix uses `bug` even when the patch is small; a
completed feature checkpoint uses `milestone`; `release` is reserved for an
intentional major compatibility/lifecycle boundary.

## Agent workflow

1. Implement and verify the ledger unit before choosing its final subject.
2. Choose the kind from the delivered outcome, not from the size of the diff.
3. For `bug`, `milestone` or `release`, update the version before the canonical
   production build. Use the helper so every registered declaration and the
   history row move together:

   ```sh
   python3 scripts/version_tool.py bump siderita bug \
     --unit SID-8-A \
     --summary "Fix picker synchronization"
   ```

4. Run `python3 scripts/version_tool.py check`, then the project's normal
   verification and `complete-production.sh`. The built and deployed binary
   must therefore contain the new version; a parallel build is not sufficient.
5. Include the version declarations, mirrors, history row, code, plan,
   inventory and evidence in the same commit. The summary argument must equal
   the imperative text after the subject colon.

`python3 scripts/version_tool.py show` prints the registered current
versions without maintaining another snapshot.
`python3 scripts/audit-version-commits.py` replays all non-merge commits after
adoption against their first parent; CI therefore catches deliveries made with
local hooks disabled.

## Ownership and cross-suite changes

The six top-level products currently versioned are Celestina, CelestinaStyle,
Siderita, Magnetita, Grafita and Fluorita. The virtual `celestina-rs` workspace
and component prefixes have no aggregate product version. A component-only
maintenance commit may use its component prefix. A component bug delivered by
one product uses that product's primary `-bug` prefix and bumps that product.

A genuine `suite-bug`, `suite-milestone` or `suite-release` may bump one or more
registered products when one atomic shared change affects them. Every changed
product must make the same kind of exact transition and append its own history
row. Unrelated product deliveries remain separate commits.

Crate package versions are not silently promoted to public release promises.
Independent crate release policy requires a separate accepted decision because
one crate can appear in several application lockfiles.

## Git edge cases

- `fixup!`, `squash!` and `amend!` commits are temporary and must not add
  another version or history row. Squash them before delivery.
- Do not directly amend a delivered typed commit: relative to its own HEAD the
  required transition has already happened. Use a fixup and squash it before
  publishing.
- Use `git revert --no-commit` for a product delivery, then create a new
  `<product>-bug:` commit with the next PATCH and a new history row. Version
  history never moves backward.
- Merge commits do not close delivery units. Versioned changes arrive in their
  ordinary typed parent commits.
- The governance commit that first adopts this contract uses the legacy
  `suite:` form because committed HEAD rules interpret staged data. Typed
  subjects become mandatory immediately after that migration lands.

When the author explicitly requests a tag, use
`<project>-v<version>` (for example, `siderita-v1.0.2`). The append-only ledger
is authoritative even when no tag is requested.
