# Evidence: support edits for Selenita's screenshots unit

- **Date:** 2026-10-09
- **Scope:** EXT-1-S — suite
- **Environment:** CachyOS, Python 3.14, the workspace toolchain
- **Artifact:** not applicable

## Procedure

```sh
(cd celestina-rs && cargo test -p celestina-core --features activation)
python3 scripts/test-activation-contract.py
python3 scripts/test-production-artifacts.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

- **Exit:** every command above exits 0 (recorded in the landing log).
- **Change:** `Owner::connection()` exposes the owning connection so
  Selenita serves `org.celestina.Selenita1` (`Capture`, `ToggleRecording`)
  beside `Application1`; the scanner allowlists that literal by (file,
  literal) with its reason; Selenita's registry entry lists `siderita-ops`
  among its production inputs.

## Limits

- Nothing runs live here; SEL-1-A carries the behaviour and its evidence.

## Landing

- **Base revision:** `202aeb38eb21c4aa51d11e90ab57e848bf1acdeb`
- **Check:** `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check calcita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check selenita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:8e326b914a26f3830e478317733a5231e3818b0fce7f475c50dec8e27eb61456, verification_fingerprint sha256:8ee4fe63f26fd21a617f5ed5f78f4b75317c545b8a26b5df99e3cdc261d5e026; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:780f99bba772ed034896fd1f9ab445d8660206f4db119206ffa5907965aff476, verification_fingerprint sha256:4e4c9f4bbfe41cd41690fa242748446d59112ca070feb53af29f48f8dc373f81; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:f2b7d9be7408a7ec1fd3bb6ef5eef64e4cbd443e804d63497b95b88d8d16966f, verification_fingerprint sha256:147871ad792a5b53ee0f7c665ce91ce25872de9ae2ac451bedc6bdde13f290e9; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:0681fca2daf8a49173a9c8f432b215c24970fb93a8ee6c77bd83d447b5259a31, verification_fingerprint sha256:ad5a9a7d84e572a6f89188ddfa0b1d5d3c6e1d3fb9d892a8930df0847d5ffcff; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d2f6ba4193c2634b4c42eaac00204d931cdb19efc566b081729c101677747139, verification_fingerprint sha256:a0ae233ecf3c7e8e31ae8a81422875600a4fc516e5f7cf743920aa144f202ad8; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:83f4cd4e1a3ede9dea7b68830c479f2a15ef3f1d1f49fdfb16ae36b4c8ff41f5, verification_fingerprint sha256:bb4e7203c261ba3c77e61be97c966f04e13b0db0546428e91cc64671d6a8e1ea; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:b4c950c5dbc9d4064d34c8d3e5df701e6e782cc12c49a69fb5010fe8d3cfe23d, verification_fingerprint sha256:daee35677d75e773025c27c23abffae482c9721a36f86ccaaab3fc9aaa047985; cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:cfef9938afe6b810e842ea06a919e69bb78b4e8e07a9be92ccdc77a8bf0b1930, verification_fingerprint sha256:b2cdbda340eec5b42466a3720d8925e7541a52675bd29e80cf4a2414dacd6606; calcita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:05f9a857a97fc0f6438397f5b38c03302dc16065c72c54d82a7ff3f707ceaa92, verification_fingerprint sha256:0f5538769d977295b37aab7ecd9a0aaf54febbe0d41a948e32589c2b5c53621e; selenita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d863646612bde73a0d35a6ec9cbfa0c8ac38e6f533acafa4ccd373c6fd778c43, verification_fingerprint sha256:1c618e0a171bd2d0ccda1ad2e22b75288201ad50025b73afae1ac9befa33d903
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh; calcita: deploy-production.sh, status-production.sh; selenita: deploy-production.sh, status-production.sh
