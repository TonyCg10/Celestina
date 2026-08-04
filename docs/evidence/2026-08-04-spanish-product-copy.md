# Evidence: LNG-1 Spanish product copy and its guard migration

- **Date:** 2026-08-04
- **Scope:** LNG-1-A; plan
  [spanish-product-copy](../plans/active/2026-08-04-spanish-product-copy.md)
- **Environment:** guards and fixtures only; no build, artifact or deployment
- **Artifact:** not applicable

## Procedure

```sh
python3 scripts/check-language-contract.py
python3 scripts/test-language-contract.py
bash scripts/test-commit-scope.sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
```

## Result

- **Exit:** 0 for each command.
- `Language contract: OK (160 legacy file(s) ratcheted)`, down from 163 files
  and 1022 suspicious lines to 1003. Ten rows were removed, all
  `celestina/qml`; two fell — `fluorita/qml/components/SeekBar.qml` 6 to 5 and
  `siderita/qml/dialogs/MediaPreview.qml` 21 to 19. No row was added and none
  grew.
- `Ran 9 tests ... OK` for the scanner fixtures, covering both exemptions and
  their refusals: a bare QML literal, a QML comment, a wrapped `qsTr()` call
  whose literal sits on the next line, a marked file's comment, and an
  unmarked file's literal.
- `Commit scope: OK`, including three new fixtures: a declared migration
  retires a row, evidence without a scanner change does not, and a scanner
  change without evidence does not.
- `version-contract: OK (6 owners)`; no declaration moved.

## Resolved debt

- **Resolved language debt:** `scripts/check-language-contract.py`

The reduction was earned by the scanner's new exemptions, not by editing the
files whose rows fell. Ten of the twelve belong to `celestina/qml`, which this
unit does not touch and has no reason to: their Spanish is `qsTr()` product
copy that ADR 0007 now accepts. That is precisely the case the declared
migration exists for, and the guard refuses it without this field.

## Limits

- The scanner is a conservative detector, not a linguistic parser. It sees
  accented characters and a small word list, so unaccented Spanish still
  passes; that limitation predates this unit and is unchanged.
- Marking a file `product-copy` is a claim about its literals that no tool can
  verify. The guard enforces that comments in such a file stay English, which
  is the part it can check.
- No surface was translated here and none was inspected. What a person sees on
  screen is each product's own delivery.

## Follow-up

Fluorita's surface returns to Spanish under `fluorita:`; other products keep
the copy they already ship.
