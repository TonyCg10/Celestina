# Evidence: 2026-08-06 a paste that said nothing, and two labels cut out of a path

- **Date:** 2026-08-06
- **Scope:** `SID-G7-H`; plan
  [shared-reading-surface](../plans/archive/2026-08-04-shared-reading-surface.md);
  two of the low findings of the
  [light monorepo audit](../../../docs/evidence/2026-08-06-light-monorepo-audit.md)
  — a cut pasted into its own folder ending in silence, and `TabStrip.qml` and
  `FolderHeading.qml` still doing string surgery on a display path
- **Environment:** source correction with compilation, lint and unit tests in
  `siderita/`, plus the repository architecture, language and documentation
  guards. No production build, no deployment, no window opened on a live
  session, and nothing from Celestina was built or run — the GPU safety hold was
  respected in full
- **Artifact:** none; no production build ran

## What was wrong

**The paste that said nothing.** `plan_paste` drops a *cut* whose destination is
the folder its entry already occupies, which is correct: moving an entry to
where it is means doing nothing. It dropped it with a bare `continue`, so the
plan carried no memory of it. When such an entry was the only thing on the
clipboard the plan came back empty, `begin_paste` returned on
`plan.sources.is_empty()`, and nothing else happened: no status text, no error,
and the cut ghost still marking entries that were no longer going anywhere. From
the person's side, Ctrl+V did absolutely nothing and the interface agreed to
say nothing about it — indistinguishable from a broken shortcut.

**Two labels cut out of a path.** `TabStrip.qml` and `FolderHeading.qml` each
derived a location label from `currentPath` with `replace(/\/+$/, "")` and
`lastIndexOf("/")`. It was presentation text, so nothing was misaddressed, but
it is the pattern
[ADR 0008](../../../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md)
names as the sign an adapter is not answering a question its QML has — and
`FolderHeading` already had the right call, `displayLocationName(currentPathKey)`,
one branch above the wrong one. The tab chip also missed what that call knows:
a location that is a mounted phone is labelled with the device name, which the
hand-cut version could not produce.

## What changed

- `src/controller/paste.rs` — `PastePlan` gains `same_folder_cuts`, and the
  planner reports each dropped cut into it instead of discarding it. Tests:
  a cut into its own folder is dropped *and* reported; the same entry *copied*
  there is still settled as a duplicate and reported as no such thing; a mixed
  batch reports only the entry it dropped and keeps the rest of the work.
- `src/controller/fileops.rs` — `begin_paste` no longer returns in silence on an
  empty plan. When the whole paste was same-folder cuts it calls the new
  `settle_same_folder_cut`, which clears the internal clipboard and the ghost,
  clears the system clipboard only while it still holds exactly those entries —
  the rule a consumed move already followed, because the system clipboard is
  shared with the rest of the desktop — and sets the status text.
- `src/controller/display.rs` — the wording itself, `same_folder_cut_status`,
  singular and plural, in the module marked `language-contract: product-copy`
  where Siderita's Spanish belongs, with a test pinning both forms.
- `qml/components/folder/FolderHeading.qml`,
  `qml/components/chrome/TabStrip.qml` — both ask
  `controller.displayLocationName(controller.currentPathKey)` for the label. The
  only text either still decides is the one case the adapter has no key for, an
  empty location. The chip touches `phoneRevision` so a device that announces
  its name relabels it.

The re-export list in `src/controller.rs` was deliberately not extended:
`fileops` reaches `display::same_folder_cut_status` through its own module path,
because adding the name to that line makes rustfmt break it across three and the
coordinator's frozen 1106-line baseline may not grow.

## Procedure

```sh
cargo fmt --all --check                                  # in siderita/
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
bash scripts/check-architecture-contract.sh              # from the repository root
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

| Command | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | passes, no diagnostics |
| `cargo test --all-targets --locked` | 109 passed, 0 failed |
| `check-architecture-contract.sh` | Architecture contract: OK |
| `check-language-contract.py` | OK, 157 legacy files ratcheted |
| `check-documentation-contract.sh` | Documentation contract: OK |

`src/controller.rs` is unchanged at 1106 lines, so its frozen architecture row
is untouched. The language baseline is untouched too: the new Spanish is a
string literal in the module that declares its literals to be product copy, and
`src/controller/paste.rs`, which is not such a module, gained none.

The status wording was checked against the defect and not only against the fix:
`plan_paste` reporting the dropped entry is what the emptiness branch reads, so
removing the report makes the paste silent again and the first test fails on the
report rather than on the wording.

## Limits

The tab strip keeps its string surgery, and the qmllint ratchet is why. Asking
the adapter from inside the delegate means reaching the outer `root` id, which
qmllint counts as an unqualified access; resolving the label on the `ListView`
and reading it back through `ListView.view` costs more warnings still. The file
is at its inventoried debt ceiling, so either spelling would have raised it, and
a ratchet is not something to raise for a label. The heading beside it does ask
the adapter, and it cost nothing. Closing the strip properly means giving the
delegate what it needs as a property rather than letting it reach for it, which
is a change to how the strip is fed and not a label fix.



Nothing was tried by hand. What is proven is that the planner reports the entry,
that the controller has a path from that report to a cleared clipboard and a
status line, and that the label functions agree with their inputs. That the
status line is legible where a person is looking when they press Ctrl+V, and
that the two labels read correctly in a real window — including the tab chip for
a mounted phone — belongs to `VAL-SID-06` in
[`../../VALIDATION.md`](../../VALIDATION.md).

The behaviour at the filesystem root changes for the tab chip: it reads `/`
rather than `Inicio`, matching what the heading beside it already showed and
what the adapter answers. This is a visible change and is recorded here rather
than hidden behind QML copy invented to preserve the divergence.

`FolderHeading` still compares `currentPathKey` with `phoneMounts` by trimming
trailing slashes off both. It is a comparison rather than a derivation, and no
path is composed from it, but it is string handling on a key nonetheless. Doing
it properly means the adapter answering which device a location belongs to,
which is a new question with its own decision. It is left open deliberately, not
overlooked.
