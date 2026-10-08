# Evidence: qmllint ratchet order

- **Date:** 2026-10-08
- **Scope:** AUD-1-N — suite
- **Environment:** CachyOS, Python 3.14, Siderita release build in the shared
  Cargo target
- **Artifact:** not applicable

## Procedure

Before the change, measured on main with the batched qmllint invocation:

- natural (directory) order: Siderita 239 warnings;
- `sort -z` order: 241; `Main.qml` listed last: 241.
- Grafita 45, Fluorita 17, Hematita 0 and Magnetita 4 are the same in both
  orders.

The two extra lines are a second "Warnings occurred while importing module
org.celestina.siderita.internal" and "Failed to import" pair reported at
`PickerWindow.qml:6`, which appears when `qml/Main.qml` is listed late.

After the change:

```sh
bash scripts/qmllint-cxxqt.sh siderita
bash scripts/test-qmllint-target.sh
bash scripts/test-production-common.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/test-land-unit.py
```

## Result

- **Exit:** every command above exits 0; `qmllint-cxxqt.sh siderita` prints
  `OK — org.celestina.siderita (241 non-fatal baseline warning(s))` and
  `test-land-unit.py` runs 85 tests, OK.
- **Observed:** `scripts/qmllint-cxxqt.sh` sorts the sources, so the count is
  the same in every checkout; Siderita's baseline row is 241.

## Limits

- The row rises from 239 to 241 only because the measurement is corrected; no
  warning was added. The other rows are unchanged.

## Follow-up

None.

## Landing

- **Base revision:** `99b3548449d91abfbb6bcd514592c28052690baa`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style verify: verify-production.sh exit 0, manifest source_fingerprint sha256:015e346ae3fb0bd76fbd62fb574fd802a2f78838705edea103a298ccc79d5c2f, verification_fingerprint sha256:111530a20c0605619386c7fdc3f58a517782b7478bc62c9de0ff978a39bcf099; celestina-rs verify: verify-production.sh exit 0, manifest source_fingerprint sha256:16db1ea3d96e80baf1b0f5b00ff6b5a79b97db5db9a45153c381dc8589eeb719, verification_fingerprint sha256:8fd5a49ee8cc68c2c0f74939378558e019cd0e87d87439b3aed706adde683ce7; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:73ac983bb8fec3b7e2acbf4d80dd1a4fb1d5a07bd3ceaa8b4fd9108e0f816a02, verification_fingerprint sha256:2c8cddd7532ded597f8615d43d791c8a10bfea2b70f672710786eaed5e5a84c4; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:e9c249e49d5113bc1f427e0a8cdf3ba28839828e931e411d3f838fc222f2f443, verification_fingerprint sha256:c10c6af8c5a4a538a84a19ef49366d7580c6ae9a7b512e7074ae85b53a2c0342; magnetita-android verify: verify-production.sh exit 0, manifest source_fingerprint sha256:5b55c4f62b23679f8584a626b5fece8b9b67c9f8a3e6778efeff4a4895fbb271, verification_fingerprint sha256:d81df3c0c20ebb8e52b0689e9d322fad5b24909d06e0fe05e764434b43431433; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:63d6a3db0801758772eb3be80ebbd542f7417a5ba25a5d621a4dffec841c7a37, verification_fingerprint sha256:389c6f3460d7cd112e7fbceb833307658fe97b67758cdd07732e1ecdc79df34c; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:80f3a2387c171041204449005e07537488e401c578eec6bea4ebb35ef92a8b2e, verification_fingerprint sha256:2be186f523e9cd63510c679c6108d43138a49c5e37d3f2fccf104e6325c8375f; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d026c6c329f696185a5e7578ce74ab430075a2cd0a9ac3a492bc99df1af9ac01, verification_fingerprint sha256:da9db5b50b74b4d8e33d67d0a11af97cd3eada9220601ed755e4d8afaa60cefe
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh
