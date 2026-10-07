# Evidence: 2026-10-06 the hover ladder

- **Date:** 2026-10-06
- **Scope:** `STYLE-G7-S` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.11.2, offscreen QPA for tests
- **Artifact:** not applicable in the session; the landing builds and verifies the registered module and its consumers

## Procedure

The author recorded the properties dialog on the session: "Abrir en
Hematita" (a tonal button) was invisible at rest and turned into a grey
block under the pointer, and "Ir a la carpeta" (a ghost button) showed no
hover at all. `STYLE-G7-Q` had turned `controlFill`, the neutral plate a
tonal button rests on and a ghost lifts to, into the Haze tint of the pills,
and the 5 % white hover washes were a 1.1:1 lift on the new `#0b0c10` card.

```sh
python3 celestina-style/scripts/check-contrast-contract.py
cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

- **Exit:** `Contrast contract: OK`; ctest `100% tests passed out of 1`
  with the two new cases
  `test_a_tonal_button_lifts_from_its_plate_under_the_pointer` and
  `test_a_ghost_button_lifts_from_clear_under_the_pointer` green; every
  guard OK.
- **Observed:** tokens `controlFill #1affffff` (plate), `controlHover
  #29ffffff` (lift), `surfaceHover #1affffff` (row and ghost lift),
  `pillFill #b3050608` (a pill's Haze tint, consumed by `CelestinaCapsule`
  and, in `SID-H1-H`, Siderita's pills). `CelestinaButton`'s neutral roles
  use exactly those: Ghost clear → `surfaceHover` → `pressedWash`, Tonal
  `controlFill` → `controlHover` → `pressedWash`. The contrast guard gained
  the ladder check: with the previous tokens it reported
  `ghost hover/card: 1.11:1; minimum 1.2:1`, which is the defect.

## Limits

- Application-local hover fills that bypass the shared button
  (`FloatingButton`, `NavItem`, `ProcessHeader`, `SidebarRow`) read the same
  tokens and therefore lift the same way, but they are not covered by the
  button tests; a shared guard over consumer QML for a hover that reads a
  non-lift token is a later unit.
- The perceptual size of the 10 %/16 % steps is the author's judgement on
  the session.

## Follow-up

None.

## Landing

- **Base revision:** `4aea52e5a06ff23f55499488dffaaf5688463421`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; the toolchain changed since the build; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:4cdfba87f5d7f574e4146a177cd5f316e9f11fd41eeb3ad54bf133d79de22174, verification_fingerprint sha256:0b16d50a81d126668589ef215f16baddc57dd0ba1079a4bd57e299966a78bb73; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5f0b4c070652d1f87bc0b4c162e82c12512cad88ee3c1cefaef59f06a144b122, verification_fingerprint sha256:c66a64437f3d4b424125e0b6c869c06efd5484c1623913df66877412a3588354; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:58526b4922f2fd01872c5a2d1eac8d2e5dbbc816f01e4a5518b00dbca7f7bdf9, verification_fingerprint sha256:8356ac53b9d4bff20b242ee8ebddd18bac6ad58f543e3a7ff20179aa7773baa2; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:ba0cd82aa97c1566754059f3dbb378b88d38bb9ce77b6b80cea4e60e4bb97870, verification_fingerprint sha256:5a601fa080643798e0fae51e70c4cc7052af3ed5f0de35a354fb8af7e8b22677; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5b55c4f62b23679f8584a626b5fece8b9b67c9f8a3e6778efeff4a4895fbb271, verification_fingerprint sha256:cd0199993b46b76ccd879f3aadfbc5c9cbe7d3c65c058c0bb05192d10bf03391; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:46ae0f5c0b1f9c381b78c99b4b82d2c9b552a53f6a337ed74625d8b5c684f968, verification_fingerprint sha256:f1012d4a1c73c9031d89cd8001c60ad8dfe48cbbc204df3def679fe699f5aae5; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:055617dcb4067ebb1eead44bb1b8f955c11d91dfd040a7684d36bab0c17ec523, verification_fingerprint sha256:696ff6bcad89dae3b6ebd8a653c9c9244a2002eb356f680be2adb05becaf0ca5; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:7f969a728b7e302a4c9d0bd9863744cd6164498b9675c6380e549cd176347df7, verification_fingerprint sha256:2c86cceb0866b86c1708a6c87a06a213e3c4b0ee89d48293c40e745e19dd4991
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh
