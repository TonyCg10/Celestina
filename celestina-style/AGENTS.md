# celestina-style — local contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It only adds
constraints for `celestina-style/`; it cannot relax the root or grant authority.

## Required context

- [README.md](README.md), [STATUS.md](STATUS.md), [ROADMAP.md](ROADMAP.md),
  [VALIDATION.md](VALIDATION.md), and [DESIGN.md](DESIGN.md)
- [Architecture](../docs/standards/architecture.md)
- [Rust, C++, Qt, and QML](../docs/standards/rust-cpp-qt-qml.md)
- [Verification](../docs/standards/verification.md)
- [Production artifacts](../docs/contracts/production-artifacts.md)

## Local boundary

- The module owns reusable presentation: tokens, fonts, icons, assets, and
  generic controls. It knows no application controllers, paths, D-Bus, Niri, or
  workflows.
- `CelestinaTheme.qml` is the only source of visual primitives and derivations.
  New roles are semantic, define surface/ink pairs where relevant, and never
  force consumers to derive color, opacity, anatomy, or motion.
- Every public type stays in parity across `qmldir`, `CMakeLists.txt`, QRC, and
  real resources. Search and verify every consumer before changing semantics or
  defaults.
- CXX-Qt applications consume canonical sources through relative links and
  explicit registration. The shell imports the same tree through a build- and
  runtime-supported URI alias. Neither path permits copies.
- An unspecified component stays local until two consumers demonstrate the same
  semantics. Public APIs are narrow, typed, intent-oriented, and never receive
  an application controller.
- Every interactive control covers keyboard, `visualFocus`, Accessible roles
  and actions, enabled/hover/pressed/selected states, and reduced motion. Modal
  layers contain/restore focus and block drag handlers below them.

## Local verification

- `celestina-style/scripts/build-production.sh`
- `celestina-style/scripts/verify-production.sh`
- `celestina-style/scripts/status-production.sh`
- Every affected deployable consumer's registered `complete-production.sh`

The gallery and automatable tests are part of verification. Blur, compositor,
real focus, and AT-SPI belong in `VALIDATION.md`. This module is nondeployable
and has no `deploy-production.sh`. Review its API from the build, QML engine,
and every host, not only from the edited visual file.
