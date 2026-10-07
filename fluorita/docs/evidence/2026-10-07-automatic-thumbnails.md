# Evidence: 2026-10-07 automatic thumbnails in Fluorita

- **Date:** 2026-10-07
- **Scope:** `FLU-H1-E` — `fluorita`, `celestina-rs/crates/fluorita-qt`, `celestina-rs/crates/fluorita-engine`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with the blur rule for the org.celestina applications
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

Built and checked in the session; the GUI was not launched:

```sh
(cd celestina-rs && cargo test -p fluorita-engine -p fluorita-qt)
(cd fluorita && cargo test)
(cd fluorita && cargo build --release)
(cd siderita && cargo build --release)
sh fluorita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/fluorita
bash scripts/qmllint-cxxqt.sh fluorita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

- **Exit:** every command above exits 0: `fluorita-engine` 177 unit and 11
  integration tests, `fluorita-qt` 7, Fluorita 76; both release builds
  finish; the smoke, the qmllint ratchet (`fluorita` stays at 17), the
  architecture contract, the language contract and the documentation
  contract pass.
- **Observed:** `fluorita-qt` now carries the freedesktop-thumbnail provider
  (`cpp/thumbnailprovider.cpp`, `cpp/fluorita/thumbnailprovider.h`), the
  generic part of Siderita's: cache freshness by mtime, a pool of two to four
  threads with `cancel()`, `QImageReader` with `setScaledSize` and
  `setAutoTransform`, the same accepted formats, owner-only temporary-file
  writes renamed into place, and path-key decoding through the URL Qt hands
  the provider. It refuses a source over 256 MiB or a header over 100
  megapixels before decoding, the budgets of `fluorita/src/image.rs`. Its
  classes stay in an anonymous namespace and its exported functions carry the
  `fluorita_` prefix, so Siderita's own provider still links beside it;
  Siderita can pass a `ThumbnailOwnPicture` hook for embedded pictures and
  launcher icons. Fluorita registers it as `"thumb"` and projects every image
  as `image://thumb/<path key>`; videos and tracks keep their cached
  `file://` poster. The spec's keys are written on the image's own text map:
  through the writer, Qt folded them into one description and split
  `Thumb::URI` at its first colon.
- **Observed:** the poster pass is `fluorita_engine::ArtworkPass`: one job at
  a time, each with its own generation, waited for in 100 ms slices under the
  host's token; an item that does not answer within twice `ARTWORK_TIMEOUT`
  is cancelled and the pass moves on (test
  `a_pass_continues_past_an_item_whose_poll_returns_nothing`). The library
  starts it after the scan's settled publication and after every watched
  change, repeats passes of at most 200 until nothing untried is pending,
  projects between passes, remembers this session's failures by path and
  mtime (`pending_where` filters before the cap), starts the engine only when
  something is pending, and cancels it on rescan and on close.
- **Observed:** the "Generar miniaturas" button, `Ctrl+G`, `Ctrl+Shift+G`, and
  the `artworkPending`, `artworkState`, `artworkDone`, `artworkTotal`,
  `generateArtwork` and `cancelArtwork` wiring are gone.
- **Observed:** the smoke now sets `HOME` to its scratch directory (the library
  seeded its roots from the real `$HOME`) and adds a sixth step: a library
  holding one photo gets its 256 × 128 thumbnail with no engine thread, and
  after a clip is added its poster appears without being asked. The binary
  installed before this unit fails that step.

## Limits

- Not looked at on screen: thumbnails appearing in the grid, and posters
  filling in by batches, are judged from the cache entries and the checks.
- A clip or track that gives nothing is not asked again until Fluorita
  restarts or the file changes.

## Follow-up

- Siderita adopts the shared provider and passes its own hooks in a later unit.

## Landing

- **Base revision:** `f549a18eaf87d5ab7b4fd5c14c119c7c67a42edc`
- **Check:** `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:2643f19d0af9d38100e2b9a8150e2f3ab86904e548c54376c37cbb92f59b932c, verification_fingerprint sha256:29cb77423b778aa144c22a1394a95864e99d4628b8dcd88098401b31914e361d; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:a39ea8c3b2a32923aa32c7bdfa41221656b2140139e06622b1f59de66f7d3d6b, verification_fingerprint sha256:2d917c00d2cfaca0b7473ded4972a53d7bfa79af834e2e9e20e0513a4d6f770b; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:785175c51afdc7969419a59ee6a42af4cc0cebeafb49ec5bc5a80ec2934ea727, verification_fingerprint sha256:4980674b06c28060326b1f3205c9a8c950ea78acc06ad1290abe0221ce87303f; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:54b1080b54a43c19885ec620c6ffe61802a8e9399367b10aeccab3356ce3c209, verification_fingerprint sha256:b087ac68774265ffd3bab9f83edd50495777ee86b4000007ad16c5c5053afb25
- **Deploy:** after the push: fluorita: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh
