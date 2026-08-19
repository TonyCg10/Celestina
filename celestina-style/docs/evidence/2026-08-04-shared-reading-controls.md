# Evidence — STYLE-G7 shared reading controls

- **Date:** 2026-08-04
- **Scope:** CelestinaStyle `STYLE-G7-A`; plan
  [shared-reading-controls](../plans/active/2026-08-04-shared-reading-controls.md)
- **Environment:** Linux 7.1.5-1-cachyos, Qt 6, offscreen Qt platform for the
  gallery smoke; the author's checkout carried the two consuming units
  (Grafita `G7-A` and Siderita `SID-G7-A`) uncommitted while this ran, since
  they are delivered by the two commits that follow this one
- **Artifact:** `celestina-style/build/libcelestina-style.so` and
  `celestina-style/build/CelestinaStyle`, sealed in
  `celestina-style/build/production-artifact.toml`
- **Related author validation:** `VAL-STYLE-04` (pending, not claimed here)

## Procedure

Suite guards over the whole checkout:

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/version_tool.py check
```

Canonical module build and verification of those same bytes:

```sh
bash celestina-style/scripts/build-production.sh
bash celestina-style/scripts/verify-production.sh
```

## Result

- **Exit:** 0 for every command above.
- **Observed:**

```text
Contrast contract: OK
QML visual contract: OK
Architecture contract: OK
version-contract: OK (6 owners)
manifest: celestina-style/build/production-artifact.toml (verified)
```

Inside the verification entry: the style and contrast contracts, the module
`qmllint` target (informational `ComponentBehavior` notes only, no error), the
`celestina-style-modal-focus` CTest (1/1 passed) and the offscreen gallery smoke
(8 s live over the compiled module).

### Component creation record

- **Canonical owner:** `celestina-style`, the module the architecture table
  names for reusable tokens and controls.
- **Equivalent recipes searched:** `rg` over `qml/` in every application for an
  existing gutter or scroll-position control before either was written; both
  first existed only inside Grafita's window, which is what the sharing contract
  requires while a control has one consumer.
- **Old path removed:** neither consumer keeps a copy. Grafita and Siderita
  reach both types through the canonical symlink path their `build.rs` already
  registers for every other shared type; there is no second implementation.
- **Dependency direction:** the module depends on nothing but `CelestinaTheme`;
  both consumers depend on the module. No application type is referenced.
- **Boundary evidence:** the architecture guard's raw-control scanner accepts
  `CelestinaScrollBar` because it is built from QtQuick primitives rather than
  re-skinning `QtQuick.Controls.ScrollBar`, and the same guard is what retires
  the two `QuickLookView.qml` rows in the Siderita unit that follows.

## Limits

- A build and a lint prove registration and construction. They do not prove the
  bar's contrast, the numerals' legibility, or the gutter's alignment at the
  author's display scale; that is `VAL-STYLE-04`.
- The gallery smoke exercises the module, not either consumer's composition.
  Both consumers' surface behaviour is recorded in their own evidence.
- No compositor, hardware or AT-SPI result is claimed.

## Follow-up

`VAL-STYLE-04`, and the two consuming units
[`G7-A`](../../../grafita/docs/plans/active/2026-08-04-g7-reading-comfort.md)
and
[`SID-G7-A`](../../../siderita/docs/plans/archive/2026-08-04-shared-reading-surface.md).
