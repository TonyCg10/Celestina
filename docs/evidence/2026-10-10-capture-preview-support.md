# Evidence: the capture preview's support edit (PRV-1-S)

- **Date:** 2026-10-10
- **Scope:** PRV-1-S of the
  [capture preview plan](../plans/active/2026-10-10-capture-preview.md):
  `docs/projects.toml`
- **Environment:** main at 6358440d (PRV-1-A landed)
- **Artifact:** none; a registry edit, no build output changes

## Change

FLU-P1-A makes Fluorita link `celestina-rs/crates/selenita-core`: after
«Guardar ambas» on a capture, with no Selenita running, Fluorita appends the
copy to Selenita's history through `selenita_core::history::History`. The
architecture contract requires every linked crate among the project's
`production_inputs`, and that file is suite scope, so the entry lands here
before FLU-P1-A. The edit is one path added to Fluorita's
`production_inputs`.

## Procedure

```sh
git cherry-pick 8908c2ee   # the edit as FLU-P1-A's implementer made it
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

## Result

The three contracts pass on this branch. On FLU-P1-A's branch the
architecture contract passes only with this entry present.

## Limits

Until FLU-P1-A lands, the entry names a crate Fluorita does not yet link;
the production-input fingerprint only grows by that crate's files.

## Landing

- **Base revision:** `6358440dcb8fd3304b02f46141edb89b58554e17`
- **Check:** `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:ed56578aaf3f3ba42e7e0914b148526da1255660f6e7f652e9bfb0b010e865db, verification_fingerprint sha256:a4daf41c0e0a7520b5bcfea8fb475a7d3eb313de2c272538e015cd4f828f3e0a
- **Deploy:** after the push: fluorita: deploy-production.sh, status-production.sh
