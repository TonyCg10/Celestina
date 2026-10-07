# Evidence: 2026-10-07 the bottom menus and the shared thumbnail provider in Siderita

- **Date:** 2026-10-07
- **Scope:** `SID-H1-M` — `siderita`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with the blur rule for the org.celestina applications
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

```sh
(cd siderita && cargo build --release)
(cd siderita && cargo test)
sh siderita/scripts/qml-tests.sh
sh siderita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/siderita
bash scripts/qmllint-cxxqt.sh siderita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

`tst_bottom_capsule.qml` gained
`test_view_sort_menu_opens_above_its_button_without_covering_it` and
`test_view_sort_sections_keep_their_place_above_their_rows`. The smoke gained
a third step: the scratch folder it opens holds one 600x300 PNG, and the
launch must leave a 256x128 entry for it in the scratch freedesktop cache,
under the MD5 of its `file://` URI and with a `Thumb::URI` text chunk equal to
that URI. As a control, the same smoke was run against the binary installed
before this change (`~/.local/bin/siderita`).

## Result

- **Exit:** the release build, `cargo test` (144 passed, 1 ignored), the smoke
  run, qmllint (239 baseline warnings, unchanged), the architecture contract,
  the language contract and the documentation contract exited 0; the QML tests
  ended with 174 passed, 0 failed. The control smoke against the installed
  binary exited 1.
- **Observed:** the view-and-sort menu and the picker's filter menu open with
  `GlassContextMenu.popupBeside(button, true)`; offscreen the menu ends above
  the middle icon, and its three headers (`GlassMenuSection`) stand at rows 0,
  4 and 9 of 11. The new smoke step found the 256x128 entry with its
  `Thumb::URI`; the installed binary's entry carried only a key `Thumb`, the
  defect the shared provider fixed. The five `thumbnails::tests` pass against
  the shared provider's helpers.

## Limits

- The menu placement and the thumbnails in the grid are judged by eye in the
  author's session.
- The shared provider adds budgets Siderita's own did not have (sources over
  256 MiB, headers over 100 MP or with no readable size, cache entries over
  8 MiB are refused).

## Follow-up

- None.

## Landing

- **Base revision:** `6b117d6a36eb6579fb1f4f38a1320357bd2173f5`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:1dd88aa4371fb58c50f330fabaf9ac5a66efe0a5baad27a0eaebd0c6848c2002, verification_fingerprint sha256:a5548e3778becf871df1926e6b2636faa252ed530a14fb78fdb06ec6f8e4845f
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
