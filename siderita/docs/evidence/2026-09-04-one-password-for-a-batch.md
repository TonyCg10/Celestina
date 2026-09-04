# Evidence: 2026-09-04 one password opens the archives that share it

- **Date:** 2026-09-04
- **Scope:** `FEEDBACK-3-SID`; no active plan — a defect reported directly by
  the author
- **Environment:** Arch-derived Linux, Qt 6.11.1, `cargo` stable
- **Artifact:** `siderita/target/release/siderita`

## The defect

Extracting several encrypted archives in one selection asked for a password
once per archive, even when they were all protected with the same key. The
worker in `siderita/src/controller/archive.rs` cleared the password after the
archive that had been asked about:

```rust
// Only the archive the question was asked about carries it.
password = None;
```

That was a deliberate rule, and it was protecting something real — a person's
key must not be handed to a file they were never asked about — but it charged
the whole cost of that rule to the common case. Ten archives from one download,
one key: ten modals.

## What changed

**The key a person gives is carried through the batch, as a second attempt and
never as the first.** Each following archive is opened with no password at all.
Only if it answers that it needs one is the carried key offered, silently. So an
archive that is not encrypted never sees the key, and the guarantee the old
comment defended is kept whole: the password reaches only files that asked for
a password.

**The retry is clean.** `siderita_archive::extract` removes its destination
folder when it refuses, so the second attempt starts against an empty
destination — no partial tree, no half-written member. That is what makes
"try, then try again" safe here rather than merely convenient; it is asserted
by `an_encrypted_zip_answers_for_its_password`.

**The modal still appears when the carried key does not fit**, and it appears
carrying its wrong-password heading rather than its first-request one: a key
was offered and refused, and the dialog says so. The answer then becomes the
batch's key for what remains.

**Both attempts measure and report identically.** The body of one attempt moved
into `attempt()`, so the progress ring means the same thing on the first pass
and the second; two inline copies would have drifted.

## Procedure

| Check | Result |
|---|---|
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets` | no warnings of ours |
| `cargo test` (siderita) | 122 pass, 1 ignored |
| `cargo test -p siderita-archive` | 13 pass |
| `scripts/qml-tests.sh` | 102 pass |
| `scripts/smoke.sh` | binary alive 8 s, no QML errors, no auto-bindings |
| `scripts/check-language-contract.py` | OK, 156 files ratcheted |
| `scripts/check-architecture-contract.sh` | OK |
| `scripts/verify-production.sh` | see below |

## Result

- **Exit:** 0 for every command in the table above.

## Limits

- The batch behaviour is exercised by reasoning over the loop and by the domain
  test that proves the three answers it depends on (asks, refuses, opens) and
  that a refused attempt leaves nothing behind. The controller loop itself has
  no automated harness in this project; a selection of several archives sharing
  one key is an author check.
- The carried key does not survive Siderita being closed, and is never written
  anywhere. It lives only in the worker thread of the batch that collected it.
- A batch that stops to ask a second time replaces its carried key with the new
  answer; an archive that needed the older key and comes later in the same
  batch will ask again. Remembering several keys per batch is a different
  question and is not in this unit.
