# Evidence: the shell's desktop entry becomes a registered artifact

- **Date:** 2026-08-05
- **Scope:** `PRD-1-A` of [the desktop entry registration plan](../plans/active/2026-08-05-desktop-entry-registration.md)
- **Environment:** Arch Linux checkout; CMake 4.4.2, Qt 6.11.1
- **Artifact:** none of its own; this unit changes which files the `celestina` manifest seals

## Procedure

```sh
python3 scripts/version_tool.py check
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-staged-units.py
celestina/scripts/complete-production.sh
celestina/scripts/status-production.sh
```

## Result

| Check | Result |
|---|---|
| Registry entry | `celestina/celestina.desktop` added to the `celestina` project's `production_inputs` and `artifact_paths` |
| Manifest | the entry appears under `[[artifacts]]` with its own size and SHA-256 |
| Installed copy | `~/.local/share/applications/celestina.desktop`, digest identical to the source |
| Documentation contract | OK |
| Version contract | OK; no product version moves |

## Observed facts

- Before this unit, `deploy-production.sh` copied the entry while the manifest
  said nothing about it: bytes reaching the author's prefix that verification
  had never sealed. That is what the production-artifact contract forbids, and
  it is why the registration is a delivery rather than a detail.
- The suite roadmap named LNG-1 while its product unit was already committed.
  The paired `LNG-1-B` administrative unit owns that plan's archive transition;
  this unit begins only after it and owns no archive path.

## Limits

- This unit registers; it does not implement. The entry's content, its
  deployment and the installed checks are Celestina's and land under
  `celestina:`.
- Nothing here was validated on a live session: whether the portal stops
  reporting `App info not found for 'celestina'` is `VAL-SHELL-03`, and the
  author has not run it again.

## Delivery order

This unit lands **before** Celestina's `LVR-1-A`. Committed the other way round,
the intermediate revision would have `deploy-production.sh` copying an
unregistered file — the exact defect being corrected.
