# Evidence: ADR 0009 amended for the video trim (PRV-1-T)

- **Date:** 2026-10-10
- **Scope:** PRV-1-T of the
  [capture preview plan](../plans/active/2026-10-10-capture-preview.md):
  `docs/decisions/0009-editing-without-an-encoder.md`
- **Environment:** main with FLU-P1-A (Fluorita 1.6.0) and SEL-2-A (Selenita
  1.1.0) landed
- **Artifact:** none; a decision record, no build output changes

## Change

FLU-P1-B trims a video's duration frame-accurately, which needs a
re-encode. The spec (§2, §6) fixed that as the one exception to "editing
without an encoder": the operation is raster-class (it produces a new file
and the interface says so) and runs `/usr/bin/ffmpeg`, from the package
libmpv already requires, as a child process that is never linked. Pictures
keep every rule of the ADR. The decision record is suite scope, so the
amendment lands here, ahead of FLU-P1-B.

## Procedure

```sh
git cherry-pick b61a4845   # the amendment as FLU-P1-B's implementer wrote it
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

## Result

Both contracts pass; FLU-P1-B's task review read the amendment's text.

## Limits

Until FLU-P1-B lands, the ADR describes an operation the deployed Fluorita
does not offer yet.

## Landing

- **Base revision:** `4c03e5ce07cc0d2ade92a229ff129bbbf6cd5ec8`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
