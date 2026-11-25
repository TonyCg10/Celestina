# Evidence: archiving the STYLE-G7 plan (ARC-2-A)

- **Date:** 2026-11-25
- **Scope:** light unit; `ARC-2-A`: the STYLE-G7 plan moved to `celestina-style/docs/plans/archive/` with `Closed`, `Successor` and `Status`; its rows `STYLE-G7-K` and `STYLE-G7-J` moved verbatim to "Rows closed outside the ledger"; `celestina-style`'s roadmap idle, status and active-plans index; Markdown links to the moved plan retargeted; the ARC-2 plan, opened and closed here; the root roadmap and status
- **Environment:** worktree `suite-ARC-2-A` on `fe854019`
- **Artifact:** not applicable (no production input changes)

## Procedure

```sh
git merge-base --is-ancestor 1e30380a HEAD
grep -n 'devicePixelRatio\|sourceSize' celestina-style/CelestinaIcon.qml
grep -ho 'stroke-width="[0-9.]*"' celestina-style/icons/*.svg | sort | uniq -c
rg -l 'silhouettePath|ContextualVeil' --glob '*.qml' --glob '!celestina-style/**' .
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-staged-units.py --root . docs/inventories/2026-11-25-style-g7-archive/ARC-2-A.numstat.tsv
```

## Result

- **Exit:** `1e30380a` is an ancestor of `main`; `CelestinaIcon.qml` sizes its source by `Screen.devicePixelRatio`; all 113 icons use `stroke-width="2.5"`; no project outside `celestina-style` uses the silhouette API or `ContextualVeil`; the four guards exit 0
- **Review:** 1 round (Sonnet): `STYLE-G7-K` confirmed in the code and `STYLE-G7-J` without a consumer; fixed: `celestina-style/VALIDATION.md` still called `STYLE-G7-J` active, and the plan now explains the ids `STYLE-G7-I`, `-L` and `-M` that never had a row; deferred: none

## Limits

- `STYLE-G7-K` was not re-verified on a scaled output; the claim rests on the code and on its 2026-08-12 commit.
- Sealed by hand with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and `sealed_diffstat`. The commit must reach `main` by fast-forward with parent `fe854019`.

## Follow-up

Whether to remove the unused `GlassSurface` silhouette API and `ContextualVeil` role.
