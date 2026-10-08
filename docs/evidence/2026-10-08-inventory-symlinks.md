# Evidence: inventory rows of new symbolic links

- **Date:** 2026-10-08
- **Scope:** AUD-1-Q — suite
- **Environment:** CachyOS, Python 3.14, Git 2.x
- **Artifact:** not applicable

## Procedure

Observed during the landing of AUD-1-P (the Cuprita skeleton), on the
canonical checkout:

```sh
python3 scripts/check-staged-units.py
# staged-unit: staged numstat mismatch for cuprita/qml/fonts in
# docs/inventories/2026-09-26-monorepo-hardening/AUD-1-P.numstat.tsv:
# inventory 0/0, Git 1/0
git diff --no-index --numstat /dev/null cuprita/qml/fonts
# error: Could not access 'cuprita/qml/fonts/null'   (exit 1, no row)
git diff --no-index --numstat /dev/null cuprita/qml/fonts.qrc
# 1	0	/dev/null => cuprita/qml/fonts.qrc
```

`cuprita/qml/fonts` is a link to the directory `celestina-style/fonts`; Git
follows it under `--no-index` and compares the directory. The index stores a
link as a one-line blob (its target), so the staged numstat is 1/0.

After the change:

```sh
python3 scripts/test-land-unit.py
```

## Result

- **Exit:** `test-land-unit.py` exits 0 (85 tests). The test
  `test_numstat_rows_of_a_mode_change_and_symlinks` now creates a link to an
  existing directory and expects the row 1/0 with the link target's digest,
  and the Git call count shows the two new links ask Git nothing beyond
  `cat-file -e`.
- **Change:** `scripts/landing.py` `numstat_rows` writes 1/0 for an untracked
  symbolic link without asking Git; tracked paths and ordinary new files are
  unchanged.

## Limits

- Hematita's H1-A inventory recorded its links as 1/0 by hand before the
  landing tool wrote inventories; this is the first skeleton landed through
  the tool.

## Landing

- **Base revision:** `d1804c320a226decc8d14119e21d8e29d5560686`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
