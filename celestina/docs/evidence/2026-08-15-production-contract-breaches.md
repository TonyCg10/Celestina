# Two breaches the release path caught and nothing else could

- **Date:** 2026-08-15
- **Scope:** Celestina unit `R8-P-D`
- **Artifact:** Celestina 0.29.1, `qml/SessionOsd.qml`, `qml/ToastStack.qml`
  and the lock's own QML module
- **Environment:** `celestina/scripts/complete-production.sh` on the author's
  machine, with no development nest running
- **Plan:** [polkit authentication agent](../plans/active/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-R8`

## Procedure

The production pipeline was run to build the shell the author is about to
transition to. Its verification step refused, and the two refusals were fixed
and the contracts re-run.

## Result

    architecture scanner: ERROR: qml/LockScreen.qml: plain QML missing from
                                 celestina/CMakeLists.txt
    ERROR: direct animation curve; use an ease* token
      celestina/qml/SessionOsd.qml:484
      celestina/qml/ToastStack.qml:346, 395, 467, 504

Both are corrected: five `Easing.OutCubic` become `CelestinaTheme.easeStandard`,
which is that exact curve, and `LockScreen.qml` moves from `celestina/qml/` to
`src/lock/`, beside the executable that owns it.

The lock's file was never registrable where it sat. The rule on that directory
is that every file in it belongs to the shell's own QML module, and a
component a *separate process* imports cannot be in that module — so the
scanner was right and the file was in the wrong place, rather than the rule
being too strict.

After both, the architecture, sealed-colour, contrast and QML visual contracts
pass, and the suite stays at 23/23.

## Limits

What this says about the project is worth more than the fix. The last
production build recorded here is 0.6.8; everything since — twenty-three
releases — has been checked only by what runs per commit. These two breaches
survived that long because they are only checked in the release path, and
nothing ran it. Any others of the same kind would have survived too, and the
only evidence that they did not is this one green run.
