# Evidence: 2026-10-06 the modal fades as one image

- **Date:** 2026-10-06
- **Scope:** `STYLE-G7-T` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.11.2
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys every consumer

## Procedure

The author recorded a modal closing badly: its opaque card went translucent
over the glass while the text on it stayed legible until the end, because
`opacity` on an item tree applies to each descendant on its own. The modal
layer's content host now renders through a `layer` for as long as the host's
opacity is strictly between 0 and 1, so the fade composes the whole dialog
first and then thins it as one image. At rest the layer is off, so a dialog
on screen costs nothing extra.

```sh
bash celestina-style/scripts/check-style-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

## Result

- **Exit:** every suite and guard OK.
- **Observed:** the modal-layer tests (open, close, Escape, the focus
  reason) pass unchanged; the layer is on only during the fade.

## Limits

- The look of the fade is the author's judgement on the session.

## Follow-up

None.

## Landing

- **Base revision:** `5b3d7b5dcbe54311ffda272ac17e787eaef34e6a`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:e0dc48a930a82b5fc3cbcfc1c012ba984b8b3b59e79cad4305da4d5e435e7c72, verification_fingerprint sha256:2f0c459e9bf885a0cab7a7d49101d8e9b15bd203a68e9ab0f2c710eaa509936a
