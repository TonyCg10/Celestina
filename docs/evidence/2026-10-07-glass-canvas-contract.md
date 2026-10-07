# Evidence: glass-canvas guard

- **Date:** 2026-10-07
- **Scope:** AUD-1-M — suite
- **Environment:** CachyOS, Python 3.14
- **Artifact:** not applicable

## Procedure

```sh
python3 scripts/test-glass-canvas-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
bash scripts/test-architecture-scanners.sh
```

## Result

- **Exit:** every command above exits 0; the architecture contract prints
  `Glass-canvas contract: OK`.
- **Observed:** the nine fixtures pass. The baseline records today's debt per
  application: `1 fluorita`, `2 grafita`, `2 hematita`, `1 magnetita`,
  `0 siderita`.

## Limits

- Only `Main.qml` is read; secondary windows (Siderita's picker, Magnetita's
  mirror) are judged by hand.

## Follow-up

GRA-H1-C, HEM-H1-C, FLU-H1-C, MAG-D1-F and SID-H1-L lower the rows.

## Landing

- **Base revision:** `5100e2332787531174e639a5a15e9991f0d728a3`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style verify: verify-production.sh exit 0, manifest source_fingerprint sha256:ac43f8a613923423eb865726608098755da4bf7a6e1c13d3b3a5464fc1296ffc, verification_fingerprint sha256:176c1e9ad19ddd409b96c3f065aaa41ee1b548dcdc40350d031e6d0c5141a166; celestina-rs verify: verify-production.sh exit 0, manifest source_fingerprint sha256:5f0b4c070652d1f87bc0b4c162e82c12512cad88ee3c1cefaef59f06a144b122, verification_fingerprint sha256:2d917c00d2cfaca0b7473ded4972a53d7bfa79af834e2e9e20e0513a4d6f770b; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:ac001d0e8f3106a2aa23f2e17212f5992086cab42c577c2efea93d2e085ec85a, verification_fingerprint sha256:4980674b06c28060326b1f3205c9a8c950ea78acc06ad1290abe0221ce87303f; magnetita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:4ece5e76fcfefd5e88c63811f0f413391eabcfc1f0bee89d1fe5ba7740f0635a, verification_fingerprint sha256:b087ac68774265ffd3bab9f83edd50495777ee86b4000007ad16c5c5053afb25; magnetita-android verify: verify-production.sh exit 0, manifest source_fingerprint sha256:5b55c4f62b23679f8584a626b5fece8b9b67c9f8a3e6778efeff4a4895fbb271, verification_fingerprint sha256:021e76858e8af54f9fdcca5a97a08de05069841f210ba933f20c15082f01dc4a; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:fab5d3232ae0bbc63543ca6b6d6826604a98eb841f8d2ab8f833c33560f7a313, verification_fingerprint sha256:f179ed55224b8784ab892f8191d6bad3be00f729a6b55777eba4b299464e1dc0; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:570395f21152473ca1d2233ca3979d31c621363b6e31570c4e61a24df307d682, verification_fingerprint sha256:44635c77cd0da25aaa75633b88750e115b7e40d847e1b4ad9991fd90be26aa3f; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:83831fec0b808447bda890dfdd7c5949bd5fc64798f333bdf25869fc82aeb51f, verification_fingerprint sha256:dbd4492aad28864e25ef839809431476a9737e89760f2e264a6473a7cdbbd9ae
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh
