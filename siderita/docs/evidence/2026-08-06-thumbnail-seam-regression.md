# Evidence: 2026-08-06 the thumbnail seam, tested where it broke

- **Date:** 2026-08-06
- **Scope:** `SID-G7-F`; plan
  [shared-reading-surface](../plans/active/2026-08-04-shared-reading-surface.md);
  finding `C1` of the
  [light monorepo audit](../../../docs/evidence/2026-08-06-light-monorepo-audit.md),
  a regression `SID-G7-E` introduced the same day
- **Environment:** source correction with compilation, lint and unit tests,
  including a test that drives the provider through the same URL a delegate
  writes. No production build, no deployment, no window opened on a live session
- **Artifact:** none; no production build ran

## What was wrong

`SID-G7-E` made the thumbnail provider decode at the byte level, which was
right, and switched the conversion to `id.toLatin1()`, which was not. Qt does
not pass the published key through to a provider: it derives the id with
`url.toString(RemoveScheme | RemoveAuthority).mid(1)`, and that formatting has
already turned every escape spelling valid UTF-8 back into its character. So an
accented name arrives decoded, and `toLatin1` flattened each of its characters
to a single byte where the file on disk holds two. The escape Qt cannot decode —
the not-valid-UTF-8 case the change existed for — was unaffected either way.

The result was a fix for the rare name that broke the ordinary one: every entry
whose name carries an accent or a tilde failed its `stat`, fell back to a generic
glyph, and stopped reading or writing the shared thumbnail cache.

The tests stayed green throughout, and that is the more useful defect. They call
the C++ helpers with raw path bytes, which enters the provider *after* the seam
where the assumption lived. They proved the function and never touched the path
that feeds it.

## What changed

- `cpp/thumbnailprovider.cpp` — the conversion is `id.toUtf8()`, correct for a
  decoded character and for a surviving escape alike. The comment now states
  what the id already is on arrival, since that, not the decoder, was the thing
  nobody had checked.
- `cpp/thumbnailprovider.cpp`, `cpp/siderita/thumbnailprovider.h` — the decode
  is named (`pathBytesForId`) and the seam is exposed as
  `siderita_thumbnail_resolved_path`, which builds the `image://thumb/<key>` URL
  a delegate writes and derives the id exactly as Qt does. The test now runs the
  code the provider runs, reached the way the provider is reached.
- `src/thumbnails.rs` — binds it, and pins the round trip for an ordinary name,
  an accented one, a folder and file that are both accented, an astral
  character, a name with spaces and parentheses, and a name that is not valid
  UTF-8.

## Procedure

```sh
cargo fmt --all --check                                  # in siderita/
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

## Result

| Command | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | passes, no diagnostics |
| `cargo test --all-targets --locked` | 94 passed, 0 failed |
| `check-architecture-contract.sh` | Architecture contract: OK |
| `check-language-contract.py` | OK, 158 legacy files ratcheted |

The new test was checked against the defect rather than only against the fix:
with `toLatin1()` restored it fails on the accented fixture, and with
`toUtf8()` it passes. A test that stays green either way is what allowed this
regression to ship, so the failing run is the part of this record that matters.

## Limits

No thumbnail was drawn in a real window. What is proven is that the provider
resolves the right bytes for a key delivered through Qt's own URL handling; that
the grid then shows the picture belongs to `VAL-SID-06` in
[`../../VALIDATION.md`](../../VALIDATION.md), whose procedure already walks a
non-UTF-8 name through the interface and now has an ordinary accented name worth
walking beside it.

The cache-key divergence for a name containing a semicolon stands, unchanged and
not ours: GLib escapes it where Qt does not.
