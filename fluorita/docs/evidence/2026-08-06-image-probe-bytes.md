# Evidence: 2026-08-06 the image probe addressed by bytes

- **Date:** 2026-08-06
- **Scope:** `F6-D`; plan
  [immersive-content](../plans/archive/2026-08-04-immersive-content.md); closes
  the image limit `F6-C` left open, recorded in
  [the byte-exact path seam evidence](2026-08-06-byte-exact-path-seam.md)
- **Environment:** source correction with compilation, lint and unit tests,
  including tests that create real files and call the C++ seam. No production
  build, no deployment, no window opened on the live session
- **Artifact:** none; no production build ran

## What was wrong

`F6-C` made every path crossing to QML a byte-exact key, but stopped at one C++
seam. `fluorita_probe_image` took a `QString`, and a `QString` cannot name a
file whose name is not valid UTF-8, so the probe was skipped for exactly those
files and `ImageDecision` refused the picture as unreadable on file size alone.
That was recorded as inevitable. It was not: what is inevitable is losing the
byte if the path is *decoded into* a `QString`, and the probe never had to do
that. The same mistake, and the same repair, as Siderita's thumbnail provider in
`SID-G7-E`.

## What changed

- `cpp/fluorita/imageprobe.h`, `cpp/imageprobe.cpp` — the seam takes the ADR
  0008 path key, which is ASCII and therefore survives a `QString` intact, and
  decodes it with `QByteArray::fromPercentEncoding` rather than
  `QUrl::fromPercentEncoding`: the byte-level call, not the one that answers a
  `QString`. The file is opened with `::open` on those bytes and the descriptor
  handed to `QFile`, so `QImageReader` never has to spell the path. A relative
  or empty key is refused rather than resolved against the process's working
  directory, where it would measure some other file.
- `src/player.rs` — `show_image` addresses the probe by key. The comment
  explaining why the probe was skipped is gone with the skipping.

## Procedure

```sh
cargo fmt --all --check                                  # in fluorita/
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
```

## Result

| Command | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | passes, no diagnostics |
| `cargo test --all-targets --locked` | 46 passed, 0 failed |

Three tests were added against the real seam rather than against a mock. Each
writes a 2×2 PNG into a self-removing fixture directory under a name given as
raw bytes, then calls the probe exactly as `show_image` does. The one that
matters, `an_image_whose_name_is_not_utf8_is_measured_on_itself`, first asserts
that the fixture's path has no `str` spelling at all — so the test would be
vacuous if the name were expressible — and then that the probe answers 2×2. A
file that is not an image, an absent file and a relative key all measure
nothing usable; that test also pins the fact that Qt spells an invalid size
−1×−1, which is why the caller gates on a positive pair rather than on zero.

## Limits

No image was opened in a real window during this work: whether such a picture
now displays on the author's session belongs to `VAL-FLU-BYTES` in
[`../../VALIDATION.md`](../../VALIDATION.md), whose expectation this changes
from "refused" to "displayed".

The probe reads the header through `QImageReader`, so a format that cannot
answer before decoding still reports nothing and the budget refuses the file —
unchanged by this unit, and deliberate: guessing a budget is how a hostile
header becomes an allocation.
