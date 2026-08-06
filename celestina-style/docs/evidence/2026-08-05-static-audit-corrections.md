# Evidence: static-audit corrections to the style module

- **Date:** 2026-08-05
- **Scope:** `STYLE-G7-C` of
  [the shared reading controls plan](../plans/active/2026-08-04-shared-reading-controls.md).
  The findings come from the suite's
  [static audit](../../../docs/evidence/2026-08-05-static-suite-audit.md):
  `STY-M1`, `STY-M2`, `STY-M3`, `STY-M4`, `STY-B1`, `STY-B2`, `STY-B3` and
  `STY-B4`. `STY-B5` is explicitly out of scope and is recorded as pending below
- **Environment:** Arch Linux, Qt 6.11.1, `qmltestrunner` and `qmllint` from
  `/usr/lib/qt6/bin`, `qml6` 6.11.1. Offscreen QPA throughout; no real Wayland
  session, no compositor, no assistive-technology stack
- **Artifact:** none. The registered `build-production.sh`,
  `verify-production.sh` and `status-production.sh` were **not** run, by the
  author's instruction for this unit, so no production seal was produced or
  invalidated. Everything below was executed against the canonical **source**
  tree through a private import root

## Procedure

The module was imported from source the way `qmldir` promises it can be — a
private directory holding a `CelestinaStyle` entry pointed at the source tree —
so every check below observes the same bytes that were edited, with no build
step in between:

```sh
qmltestrunner -input celestina-style/tests            # whole suite
qmltestrunner -input celestina-style/tests/tst_scrollbar.qml
qmltestrunner -input celestina-style/tests/tst_iconcatalog.qml
qmllint -I <import-root> CelestinaScrollBar.qml CelestinaIcons.qml \
    CelestinaIconButton.qml CelestinaButton.qml CelestinaIconShapes.qml \
    tests/tst_scrollbar.qml tests/tst_iconcatalog.qml
qml6 gallery/Gallery.qml                              # offscreen, 8 s
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash -n celestina-style/scripts/sync-phosphor-shapes.sh
sh -n celestina-style/gallery/run.sh
```

Each of the two behavioural corrections was additionally run against a
**negative fixture**: a throwaway copy of the module with the pre-correction
expression restored, so the new tests are shown to fail on the old code rather
than merely to pass on the new.

## STY-M1 — the handle position disagreed with the drag

`CelestinaScrollBar.qml`. `handleOffset` was
`clamp(scrolledFraction * trackLength)`, while `scrollToHandle` converts through
`handleTravel` and `contentTravel`. The two only agree while
`handleLength == shownFraction * trackLength`, and the `minimumHandle` clamp
breaks that equality in exactly the long documents the clamp exists for.

`handleOffset` now derives from the same pair of distances the drag uses, guarded
against `contentTravel <= 0`:

```qml
readonly property real contentOffset: root.horizontal
    ? root.surface.contentX : root.surface.contentY

readonly property real handleOffset: root.contentTravel <= 0
    ? 0
    : Math.max(0, Math.min(root.handleTravel,
                           root.contentOffset / root.contentTravel * root.handleTravel))
```

`scrolledFraction` was removed with it. It was the wrong mapping's only input,
nothing in the repository read it (`rg` over every project found no consumer),
and leaving it in place would leave the discarded conversion available to the
next reader.

Measured on the fixture the new test uses — an 800 px track over a document with
a 0.01 visible ratio, so `handleLength` clamps to `minimumHandle` 24 and
`handleTravel` is 776:

| Position | Old `handleOffset` | Corrected |
|---|---|---|
| Top | 0 | 0 |
| Half the document | 396 | 388 |
| 99% of the travel | 776 — already at the end | 768.2 |
| End | 776 | 776 |

So the handle saturated once the document passed roughly 98% and sat pinned
while the last screens still scrolled, and everywhere else it ran ahead of the
position it reports. During a drag that error is what separates the handle from
the pointer, because `onPressed` measures the grab against `handleOffset` while
`scrollToHandle` writes back through the other conversion.

The audit's own illustration — "the handle reaches the bottom with 22% of the
document left" — is not reproduced at these numbers: with `minimumHandle` at 24
logical px the pinned tail is about 2% of an 800 px track. The size of the error
scales with `minimumHandle / trackLength`, so a short track shows a much larger
tail (a 200 px track pins the last ~11%). The defect is exactly as described;
only the quoted magnitude is track-dependent, and it is recorded here as
measured rather than as quoted.

Regression: `tests/tst_scrollbar.qml`, six cases, including that the clamp is
actually active in the fixture (otherwise the other cases prove nothing), that
position inverts the drag over seven offsets, and that an unscrollable surface
yields no offset and no movement.

## STY-M2 — the sync script truncated its own output before downloading

`scripts/sync-phosphor-shapes.sh`. The assembling block redirected onto
`CelestinaIconShapes.qml`, so the redirection truncated the module's source the
instant the block opened — before the first of thirteen network downloads. It is
a brace group, not a subshell, so the `exit 1` on a failed download terminates
the script outright and leaves a half-written singleton that does not parse,
taking the whole module import down until someone runs `git checkout`.

The block now writes `"$work_dir/CelestinaIconShapes.qml"`, under the existing
cleanup trap. Afterwards the generated shape count is compared with
`${#shapes[@]}`, and only on a match is the file installed beside the target and
renamed into place. The trap also removes the `.tmp` staging path.

**Not executed.** The script performs thirteen network downloads and rewrites a
tracked source file; running it was outside this unit. It was syntax-checked
(`bash -n`, clean) and read line by line. The count regex
`^        "[^"]+": \[$` was checked against the exact `printf` that emits those
lines. That the corrected script produces a byte-identical catalogue is
**unproven** and belongs to whoever next regenerates the shapes.

## STY-M3 — the gallery's import root was a predictable path in `/tmp`

`gallery/run.sh`. With `XDG_RUNTIME_DIR` unset — ssh, cron, containers — the
import root fell back to `/tmp/celestina-style-gallery`, a fixed name in a
world-writable directory. The import root is where the QML engine resolves the
module it is about to execute, so a local user who pre-creates that directory
and supplies their own `CelestinaStyle` entry has the runtime load their code
with the author's privileges.

It now uses `mktemp -d` under `${TMPDIR:-/tmp}` with a cleanup trap on `EXIT`,
`INT`, `TERM` and `HUP`, and a plain `ln -s` into the fresh directory rather than
`ln -sfn` over whatever was there. The final `exec` was dropped: the shell has to
outlive the runtime for the trap to fire, and the script's exit status is still
the runtime's.

**Not executed**, by instruction. `sh -n` and `bash -n` are clean. The
equivalent import-root arrangement it now builds was exercised directly by every
`qmltestrunner` and `qml6` invocation in this record, which used a private
`mktemp`-style directory containing a `CelestinaStyle` symlink and resolved the
module correctly.

## STY-M4 — Phosphor's licence was named but not shipped

`CelestinaIconShapes.qml` redistributes geometry derived from Phosphor Icons,
which is MIT and requires the notice to travel with it. The generated header
pointed at `icons/LICENSE`, and `icons/` contained only `LICENSE-lucide.txt`.

Added `icons/LICENSE-phosphor.txt` with the MIT text and the upstream copyright
line, and corrected the reference in both the generated header and the generator
that emits it.

The copyright line was read from
`https://raw.githubusercontent.com/phosphor-icons/core/v2.0.8/LICENSE` — the
exact tag this catalogue is pinned to — and is `Copyright (c) 2023 Phosphor
Icons`. That was the only network access in this unit, and nothing was
downloaded to disk.

**Not registered in CMake or the QRC, deliberately.** `LICENSE-lucide.txt` and
`fonts/LICENSE.txt` are not registered either — no licence in this module is —
so registering only the new one would break the parity the local contract asks
for rather than establish it. Whether the module should ship its notices inside
the compiled resource is a real question and it is a separate one; it is
recorded as pending below.

## STY-B1 — untrusted names were looked up through the prototype chain

`CelestinaIcons.qml`. `aliases[requested]` and `available[candidate]` read names
that a consumer supplies and that, through `~/.config/siderita/icons.conf`,
ultimately come from a hand-editable file. On an ordinary object literal
`"toString"` and `"constructor"` resolve up the prototype chain to inherited
functions instead of to `undefined`.

Both tables are now built with `Object.assign(Object.create(null), { … })`, so a
subscript answers only with a catalogue entry. The guard is on the table rather
than on each reader, which also covers any future consumer of the two public
properties — `resolve` is not their only reader.

Honest severity: the observable outcome was already correct. `=== true` refused
the inherited function and `Function.prototype.length` is 0, so `candidate.length
> 0` refused it a second time; the negative fixture confirms this — with plain
literals, `test_a_prototype_name_falls_back_like_any_unknown_name` still passes
and only the direct table lookup fails. What was wrong was a lookup succeeding
against something that is not an icon, and two accidental properties of
`Function` being what stopped it.

Regression: `tests/tst_iconcatalog.qml`, four cases covering seven prototype
names, the ordinary catalogue answers, and that both tables remain enumerable.

## STY-B2 — an icon-only button could be announced with no name

`CelestinaIconButton.qml` set `Accessible.name: helpText`, and `helpText`
defaults to `""` while `iconName` is `required`. An icon-only button carries no
text, so a consumer that forgets the tooltip ships a control that assistive
technology cannot name at all, and nothing in the type demands it.

It now degrades: `helpText.length > 0 ? helpText : iconName`.

Degrade rather than require, and the reason is measured rather than aesthetic:
16 of the 29 `CelestinaIconButton` instantiations in the repository set no
`helpText` (Siderita 9, Fluorita 3, Grafita 2, and the remainder). Making it
`required` would refuse to construct the majority of the suite's icon buttons,
across five projects this unit may not touch. It is also the degradation
Siderita's `FloatingButton` already applies with `text`. `iconName` is an
English catalogue key and therefore a diagnosable placeholder, not product copy —
it keeps the control operable and makes the missing label audible instead of
silent.

## STY-B3 — the disabled-primary tokens were adopted, not retired

`CelestinaTheme.qml` defines `accentDisabledFill` and `accentDisabledInk` and
documents them as "a primary action that remains identifiable while disabled",
and no component of the module read either. `CelestinaButton` painted every
disabled role identically, so a disabled primary "Guardar" and a disabled tonal
"Cancelar" were the same rectangle.

**Adopted, not retired.** Retirement was never actually available: `rg` over the
repository shows `siderita/qml/components/chrome/FloatingButton.qml` consumes
both tokens today, for exactly this state. They are unused *within the module*,
which is not the same as dead, and removing them would break a consumer this unit
may not edit.

`CelestinaButton`'s disabled background is now `accentDisabledFill` for the
`Primary` role and `controlFill` for every other role.

Only the fill. The measurements, computed with the same compositing and WCAG
arithmetic that `scripts/check-contrast-contract.py` uses, over the four surfaces
a button sits on:

| Disabled primary label | canvas | card | tonal | elevated |
|---|---|---|---|---|
| `accentDisabledInk` over `accentDisabledFill` | 3.50 | 3.07 | 2.89 | 2.64 |
| `accentDisabledInk`, fill at the existing `disabledOpacity` | 3.84 | 3.45 | 3.26 | 2.97 |
| `textMuted`, fill at the existing `disabledOpacity` *(adopted)* | 7.27 | 6.21 | 5.73 | 5.06 |

The contract owes normal text 4.5:1 **in every state**, and `accentDisabledInk`
does not reach it at body size against its own fill, so the label stays on
`textMuted`. Dropping the blanket `disabledOpacity` for this role was also tried
on paper and rejected: it makes the wash more obviously blue but takes the label
to 4.33:1 on `elevated`, below the floor.

The visible result is therefore modest by construction — on canvas the disabled
primary renders `#0b1421` against the tonal `#0c0d0f`, a blue cast rather than a
different value. Whether that reads as "still the primary action" at the author's
display is perception, not arithmetic, and is queued as `VAL-STYLE-05`.

`scripts/check-contrast-contract.py` was run and passes; note that it checks no
disabled pair at all, so the table above is this record's own measurement and not
a guard result. Adding disabled pairs to that script is a contract change with
its own consequences and is recorded as pending.

Not fixed: a disabled destructive still loses its red. The theme has no
`dangerDisabled` pair, and inventing one is a palette decision under the sealed
colour contract, not a bug fix.

## STY-B4 — already covered; no change made

The audit records that `.gitignore` does not cover `__pycache__/`. It does. Root
`.gitignore:23` carries `**/__pycache__/`, committed, and it is effective:

```sh
git check-ignore -v celestina-style/scripts/__pycache__/
# .gitignore:23:**/__pycache__/  celestina-style/scripts/__pycache__/
git status --short --untracked-files=all | grep -i pycache   # no output
```

The directory exists and is untracked *because it is ignored*, which is the
intended state. Adding a duplicate row to `celestina-style/.gitignore` would be
noise, so nothing was changed. The finding is stale.

## Result

| Check | Result |
|---|---|
| `qmltestrunner -input celestina-style/tests` (offscreen, source module) | 36 passed, 0 failed, 0 skipped |
| `tst_scrollbar.qml` against the corrected source | 8 passed, 0 failed |
| `tst_scrollbar.qml` against the pre-correction mapping (negative fixture) | 5 passed, **3 failed** — end-of-document, mid-document and drift-under-drag |
| `tst_iconcatalog.qml` against the corrected source | 6 passed, 0 failed |
| `tst_iconcatalog.qml` against plain object literals (negative fixture) | 5 passed, **1 failed** — the table answered `toString` with a function |
| `qmllint` over the five changed and two new QML files | clean, exit 0 |
| `qml6 gallery/Gallery.qml` offscreen for 8 s | constructed and stayed up; empty stdout and stderr |
| `bash scripts/check-architecture-contract.sh` | style contract OK, sealed colours OK, contrast OK; the only errors name `celestina-rs`, `siderita` and `celestina` sources, all pre-existing |
| `python3 scripts/check-language-contract.py` | no `celestina-style` row; the legacy Spanish counts of the two ratcheted files this unit edits are unchanged |
| `bash -n` on the sync script, `sh -n` and `bash -n` on `gallery/run.sh` | clean |

## Limits

- **No production artifact was built, verified, deployed or sealed.** The
  registered scripts were not run, by instruction. Everything here observes the
  source tree, so the compiled module's own `all_qmllint` and its CTest run are
  **not** evidence of this unit; the two new tests are auto-discovered through
  `QUICK_TEST_SOURCE_DIR` and will be part of that run when it happens, but they
  have not been run through it.
- **Two scripts were corrected and not executed**, and the reasons differ:
  `sync-phosphor-shapes.sh` downloads and rewrites a tracked source, and
  `gallery/run.sh` was excluded by instruction. Their corrections are read and
  syntax-checked only.
- Offscreen QPA proves construction and layout arithmetic. It proves nothing
  about blur, real focus, motion perception, AT-SPI, or how the scroll bar feels
  under a pointer — the scroll bar's own perceptual check is `VAL-STYLE-04` and
  the disabled-primary appearance is the new `VAL-STYLE-05`.
- The contrast table for the disabled primary is this record's arithmetic. The
  repository's contrast guard checks no disabled pair, so nothing enforces it.
- The suite guards were run against a worktree that other work is changing
  concurrently: two consecutive `check-architecture-contract.sh` runs minutes
  apart reported different line counts for `celestina-rs/crates/magnetitad` and
  `siderita/src/controller.rs`, and the language scanner's failing set changed
  between runs. Those failures are not this unit's and no attempt was made to
  fix or work around them; only the absence of a `celestina-style` row in either
  is claimed.

## Pending, deliberately not done

- **`STY-B5` — `CelestinaLineGutter.qml` reindexes the whole document on every
  keystroke.** Real, and untouched here. Its fix is an incremental or deferred
  reindex, which is a redesign with its own measurement, its own fixture and its
  own before/after numbers against Grafita's declared 64 MiB ceiling. It is the
  same class of work as, and shares a boundary with, Grafita's `GRA-M6`, which
  the suite audit also deferred for this reason.
- A disabled `Destructive` role that keeps its red needs a `dangerDisabled`
  surface/ink pair under the sealed colour contract.
- Licence notices are not registered in the compiled module's resources — for
  Lucide, Inter or Phosphor. If the module is ever installed outside this tree,
  that is the moment it matters.
- The contrast guard covers no disabled pair.
