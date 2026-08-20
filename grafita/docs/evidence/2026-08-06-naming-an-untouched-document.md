# Evidence: 2026-08-06 naming a document nobody has typed into

- **Date:** 2026-08-06
- **Scope:** `G7-D`; plan
  [g7-reading-comfort](../plans/archive/2026-08-04-g7-reading-comfort.md); a low
  finding of the
  [light monorepo audit](../../../docs/evidence/2026-08-06-light-monorepo-audit.md)
- **Environment:** source correction with compilation, lint and unit tests. No
  production build, no deployment, no window opened on a live session
- **Artifact:** none; no production build ran

## What was wrong

`G7-C` added a clean-document guard to `save()` so pressing the shortcut twice
would not rewrite an unchanged file. It was placed before the check for whether
the document has a file at all, which made it answer a question it was never
about: a new document that nobody had typed into yet is clean, so the shortcut
returned nothing — no write, and no request for a destination either. There is
no separate "save as" verb; every path to naming a document goes through
`save()`. So an untouched new document could not be given a name at all.

The audit called this a behaviour change that might well be intended but was
unrecorded. It was not intended. The clean check exists so an unchanged *file*
is not rewritten; a document with no file has nothing to rewrite, and the check
does not apply to it.

## What changed

- `grafita-core/src/session.rs`, `save` — the destination question is asked
  first, and the clean guard applies only once a file is known to exist. The
  comment states the ordering as the rule it is, because the two lines read
  interchangeably and are not.

## Procedure

```sh
cargo test -p grafita-core                               # in celestina-rs/
cargo fmt --all --check
cargo clippy -p grafita-core --all-targets --locked -- -D warnings
```

## Result

| Command | Result |
|---|---|
| `cargo test -p grafita-core` | 83 + 23 + 28 pass, 0 fail |
| `cargo fmt --all --check` | clean |
| `cargo clippy -p grafita-core --all-targets` | passes, no diagnostics |

The added test opens a new document, asserts it starts clean — so it cannot pass
by accident on a dirty one — and then asserts the save shortcut answers
`DestinationNeeded` rather than nothing.

## Limits

No dialogue was opened. That the chooser really appears for an untouched new
document on a real session belongs with `VAL-GRA-SAVEAS` in
[`../../VALIDATION.md`](../../VALIDATION.md), which already walks the save-as
path.

The double-save protection this reorders is unaffected: a document that has a
file and no changes still writes nothing, which was the whole point of the guard
that `G7-C` added.
