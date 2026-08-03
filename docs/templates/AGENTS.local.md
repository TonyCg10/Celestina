# PROJECT — local contract

This file inherits the root `AGENTS.md` in full. It only adds constraints for
`PROJECT/`; it cannot relax the root or grant authority.

## Required context

- `PROJECT/README.md`
- `PROJECT/STATUS.md`
- `PROJECT/ROADMAP.md`
- `PROJECT/VALIDATION.md`
- additional contracts or decisions: `PATH`

## Local boundary

- Project responsibility and exclusions.
- Owners of Rust, C++, Qt adaptation, and QML.
- Risks or invariants not already defined by the shared standard.

## Local verification

- `PROJECT/scripts/build-production.sh`
- `PROJECT/scripts/verify-production.sh`
- `PROJECT/scripts/complete-production.sh` for deployable bug and milestone exits
- strictly local additional tests

Act with expert Rust/C++/Qt/QML judgment. Verify does not install or activate;
complete updates the normal test binary without activating a live surface.
