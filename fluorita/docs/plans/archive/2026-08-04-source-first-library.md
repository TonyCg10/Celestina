# Source-first library and direct activation

- **Opened:** 2026-08-04
- **Plan ID:** source-first-library
- **Status:** done
- **Scope:** fluorita
- **Implementation checkpoint:** F5
- **Author-validation checkpoint:** VAL-FLU-SOURCES
- **Closed:** 2026-08-04
- **Successor:** F6, [immersive-content](../active/2026-08-04-immersive-content.md)

## Hypothesis

The library can be navigated by configured source without adding a domain
concept, because `fluorita-core` already models the mapped roots and every
catalogue record already carries the source that owns it; what is missing is
persistence of the user's choices, that column in the projection and a surface
that uses it.

## Tangible outcome

Launching Fluorita shows a sidebar of the mapped folders and a button that opens
the desktop folder chooser; the chosen folder persists across restarts and can
be removed again. Selecting a folder shows the supported content inside it —
the Gallery grid for a source contributing images or video, the Music
projection for one contributing audio. A single click opens the item.

## Scope

- User-owned, persistent media sources with identity stable across runs, seeded
  on first run from the existing XDG media directories.
- Per-source Gallery and Music projections, and the resolution from a source's
  contributed kinds to the projection that renders it.
- A bounded desktop file-chooser portal client for choosing a folder, off the
  GUI thread.
- The source sidebar, its adaptive content panel and the removal of the two
  kind tabs.
- Single-click activation in both content projections.

## Exclusions

- Subfolder navigation inside a source. The sidebar lists mapped roots; a tree
  needs its own accepted slice and ADR 0006 names it as a revisit condition.
- A cross-source view. ADR 0006 states it would be an explicit entry beside the
  sources, not a restored kind tab, and it is not requested.
- Any embedded Siderita surface change. It stays a minimal viewer/player with
  no library, sources or settings.
- Per-source kind editing. A chosen folder contributes the kinds its content
  supports under the existing seeding rule; a per-source kind editor is not
  requested.
- Scan, watch, metadata, artwork, trailer and playback behaviour, and the
  `Space` versus double-click contract between hosts.
- Promoting the sidebar row to `celestina-style`. One consumer does not prove
  shared semantics; the standard keeps a new visual control local until a
  second consumer does.

## Build order

1. **FLU-5-A — persisted user-owned sources.** `SourceSet` gains removal and
   identity that survives a restart, `SourceScope` scopes both catalogue
   projections to one root, and `Catalogue::retain_configured` is the single
   answer to whether a record still belongs to a configured root.
   `fluorita-engine`
   gains a source store beside the catalogue store: same header-versioned,
   percent-encoded, bounded, atomically replaced format, first-run seeding from
   the XDG media directories, and a corrupt or unreadable file falling back to
   that seed rather than to an empty library. Tests land in the same unit.
2. **FLU-5-B — the adapter.** The library QObject publishes the source rows and
   the selected source, projects Gallery and Music for that source only, and
   exposes adding and removing a folder. Choosing one is a bounded portal
   request on a worker; the GUI thread never blocks and a portal that is absent
   or refused reports a stated failure. Adding or removing re-enters the
   existing scan path rather than adding a second one.
3. **FLU-5-C — the surface.** A sidebar component lists the sources and its
   button opens the chooser; the content panel resolves to the Gallery grid or
   the Music projection from the selected source; the two kind-tab buttons and
   the `StackLayout` they drove are removed. Both projections activate on a
   single click while keyboard activation keeps working. New QML files are
   registered in `build.rs`.
4. **FLU-5-D — the documents and the delivery.** README, STATUS, VALIDATION and
   this ledger record the delivered surface and the exit that proved it, the
   library lane's language-baseline rows come down with the translations that
   earned them, and the milestone version transition lands with its history
   row.

The projection a source resolves to is decided by what the selected folder
actually holds rather than by its declared kinds. A folder holding both
pictures and music would otherwise have half its contents hidden behind a
control nobody would think to look for, and a `SourceView` type computing that
from `KindSet` would have been a second, weaker answer beside the projections
themselves.

## Implementation exit

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
cargo fmt --check            # fluorita and the changed workspace crates
cargo clippy -- -D warnings  # same
cargo test                   # fluorita-core, fluorita-engine, fluorita
bash fluorita/scripts/complete-production.sh
bash siderita/scripts/complete-production.sh
```

`fluorita-core` and `fluorita-engine` change, so Siderita's embedded surface
consumes the same bytes and completes too; verifying only Fluorita would leave
the author's installed file manager stale. QML registration, `qmllint` and the
offscreen smoke run inside the Fluorita verification entry. A build proves
compilation and a smoke proves startup; neither proves the real compositor,
pointer interaction or assistive technology, and no such claim is made from
them.

The persisted source store is a new on-disk format. Its exit includes a
round-trip test over a non-UTF-8 root name and a test that an unreadable or
unrecognised file seeds instead of emptying the library.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| FLU-5-E | `fluorita:` | done | [inventory](../../inventories/2026-08-04-source-first-library/FLU-5-E.numstat.tsv) | 6 files, +184/-162 | Archive this plan and return the Fluorita roadmap to idle now that its unit is delivered | [evidence](../../evidence/2026-08-04-source-first-library.md) | None |
| FLU-5-D | `fluorita:` | done | [inventory](../../inventories/2026-08-04-source-first-library/FLU-5-D.numstat.tsv) | 30 files, +2477/-315 | Navigate the library by mapped folder: persistent user-owned sources, source-scoped projections, the sidebar and its adaptive panel, single-click activation, and the documents and version transition that carry them | [evidence](../../evidence/2026-08-04-source-first-library.md) | `VAL-FLU-SOURCES` |

The four steps of the build order are review boundaries, not separate
deliveries: none of them is independently useful and none was ever going to
be committed alone, so they close as the single atomic unit the version
transition already names.

## Where the verified bytes were built

`scripts/check-architecture-contract.sh` runs over the whole checkout and is
chained by both completion commands. It fails on the author's uncommitted
Grafita work — `grafita/qml/components/EditorScrollBar.qml` rebuilds a Qt
`ScrollBar` outside the baseline — which is unrelated to this unit and was left
untouched. On the author's instruction both products were therefore built,
verified and deployed from a detached worktree at `HEAD` carrying only this
unit's changes, with `docs/version-history.tsv` and
`scripts/language-baseline.tsv` rebuilt from `HEAD` plus this unit's lines
alone. Both completions exited zero and reported current, verified, installed
artifacts. The detail and its consequences are in the
[evidence](../../evidence/2026-08-04-source-first-library.md).

The author resolved that Grafita violation during the same session, so both
completions were then run again from the main checkout and passed there. The
manifests and seals live in the author's checkout and the temporary worktree was
removed.

All four units form one atomic batch. The batch delivers a completed feature
checkpoint, so it closes as `fluorita-milestone` with the exact MINOR
transition and one appended version-history row before the canonical production
build. The ledger keeps the base authority `fluorita:`; the suffix is commit
intent, not a second scope.

This plan depends on suite checkpoint ACT-1
([plan](../../../../docs/plans/archive/2026-08-04-source-first-library-navigation.md)),
which amends the activation contract this surface would otherwise contradict.
That unit is documentation only and lands under `suite:`; it is never folded
into this batch.
