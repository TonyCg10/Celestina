# Evidence: the catcher that lived in the column

- **Date:** 2026-09-29
- **Scope:** `SID-H1-E` of the
  [SID-H1 plan](../plans/active/2026-09-26-hardening.md): a defect of the
  transient chrome `SID-B1` delivered in `1.6.0`, found by the author on
  `1.7.5` and taken into the active plan the way `SID-H1-C` took the dialog
  and motion fixes. It closes no audit finding.
- **Environment:** session worktree `siderita-SID-H1-E` on branch
  `unit/siderita/SID-H1-E`, base `e459c328` (`origin/main`); Arch-derived
  Linux, Qt 6 `qmltestrunner` on `offscreen`. No production build in the
  worktree, by the landing contract.
- **Artifact:** none from this session; the landing builds, verifies and
  deploys `1.7.6`.

## The defect

The author recorded three things on `1.7.5`: the rings of the operations
dock drew in one place and jumped to another on the first press, from there
they overflowed the content box, the error pill behind them stopped stacking
above and sat under them instead, and pressing anywhere over the folder did
not close an open callout.

They are one thing. `OperationsDock` owns an outside catcher — the item that
turns a press anywhere else into "close the callout" — and reparents it to
`dock.parent` with `anchors.fill: parent`. That was right while the dock sat
in a plain `Item`. `SID-B1-A` moved the dock into the `Column` of
`ActivityStack`, so that parent became a positioner, and a child with `fill`
anchors inside a positioner is one Qt refuses outright. Reproduced in the
test harness, the engine says so itself:

```
QML Column: Cannot specify top, bottom, verticalCenter, fill or centerIn
anchors for items inside Column. Column will not function.
```

"Will not function" is exact: the column stops positioning, so its children
share one `y` (the pill under the rings), the layout it does manage changes
when the catcher becomes visible (the rings move on the first press), and
the catcher covers the column rather than the folder (the press outside
does nothing). The dock's own test never saw it because it instantiates the
dock in a plain `TestCase` item, where `dock.parent` is what the catcher
expects.

## What changed

`OperationsDock` gains `property Item outsideParent: dock.parent`, and the
catcher reparents to that. `ActivityStack` takes it as a `required` property
and hands it to the dock; `FolderBottomStatus`, whose root fills the folder,
passes its own root. The default keeps the dock's direct consumers and its
existing tests as they were; the column never receives an anchored child.
Injection, not a reach through parents: the contract's rule that a
component never climbs to a parent id is what made the reparenting wrong in
the first place.

## Procedure

1. Two tests in `tst_activity_stack.qml`, written before the fix and run
   against the tree as it was: one job and an operation error, open the
   callout by state, and assert the rings and the pill keep their `y`, stay
   stacked, and the column keeps its width; and open the callout by pressing
   the ring, then press far from the column, and assert it closed. Both
   failed with the expected messages, and the run printed the engine's
   warning above.
2. The fix. Both pass, the warning is gone, and every other test still
   passes, including the dock's own `tst_operations_dock`.
3. `scripts/check-language-contract.py`,
   `scripts/check-architecture-contract.sh`,
   `scripts/check-documentation-contract.sh`.

## Result

```
scripts/qml-tests.sh
Totals: 176 passed, 0 failed, 0 skipped, 0 blacklisted
```

`Language contract: OK` · `Architecture contract: OK` · `Documentation
contract: OK`.

One baseline in the first test was read before the pill had measured its
text — a `Text` has no `implicitWidth` until its first render — and compared
unequal to itself for that reason alone; the test now settles for a frame
before reading it. Stub strings in the new tests are English: they are a fake
controller's data, not product copy, and the language guard reads them as
repository text.

## Limits

`qmltestrunner` on `offscreen` proves layout and hit-testing, not appearance
or glass. Whether the rings now sit inside the box on the author's session is
the author's own pass on the landed `1.7.6`. The two other surfaces the
author's screenshot shows — the bottom capsule and the error pill's own
width — were not changed.

## Not covered here

No window was opened on the author's session by an agent.

## Landing

- **Base revision:** `e459c328fc074a4afd688dad9f6e32b0f2838a2a`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:cbe962e1f2f98257fd1befc3d6c1d1a8fe0f4730a1017eb8d41e5db49c5f6507, verification_fingerprint sha256:e6e95238d899be6febc9d7d38a24ef36244cecb65ea292d7dd96c1ac0dbea114
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
