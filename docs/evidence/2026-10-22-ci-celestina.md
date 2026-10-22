# Evidence: CI-CELESTINA — system libraries for the `celestina` workflow

- **Date:** 2026-10-22
- **Scope:** `.github/workflows/celestina.yml` only: a step that installs
  `libwayland-dev` and `libxkbcommon-dev` before Clippy and the tests. No
  source, manifest or version changed.
- **Environment:** CachyOS, worktree `suite-CI-CELESTINA`, branch
  `unit/suite/CI-CELESTINA`
- **Artifact:** not built; no production entry ran in the session

## Diagnosis

The `celestina` workflow has failed on every push to `main` that touches the
shell since 2026-10-17 (runs 39072814353, 39075099757, 39076192204): the
runner installs no system package and `wayland-sys` panics in its build script
with «The system library `wayland-client` required by crate `wayland-sys` was
not found».

What each crate needs, read from its build script and the resolved features
(`cargo tree --locked -e features -i wayland-sys`):

| Crate | Mechanism | Ubuntu package |
|---|---|---|
| `wayland-sys` 0.31.11 | `pkg-config` probes `wayland-client` and `wayland-egl` (features `client`, `egl`; no `dlopen`; no `cursor`, no `server`) | `libwayland-dev` |
| `smithay-client-toolkit` 0.21.1, `xkbcommon` 0.8.0 | the toolkit's build script probes `xkbcommon.pc`; the crate has no build script and `#[link(name = "xkbcommon")]` | `libxkbcommon-dev` |
| `wayland-protocols*`, `wayland-scanner` | protocol XML bundled in the crates | none |
| `khronos-egl` (`dynamic`), `glow` | EGL loaded at run time; tests do not need it at link time | none |
| `fontdb`, `fontconfig-parser`, `cosmic-text` | pure Rust | none |
| `celestina-fake-system` | needs `dbus-daemon`; a test is skipped when it is missing | none (present on the runner) |
| `cc` (`wayland-backend`) | C compiler | none (present on the runner) |

## Procedure

Run from the worktree root: parse the workflow as YAML, then the architecture,
language and documentation guards.

## Result

| Command | Exit | Output |
|---|---|---|
| YAML parse of `.github/workflows/celestina.yml` (PyYAML) | 0 | OK |
| `bash scripts/check-architecture-contract.sh` | 0 | OK |
| `python3 scripts/check-language-contract.py` | 0 | OK |
| `bash scripts/check-documentation-contract.sh` | 0 | OK |

`act` and `actionlint` are not installed here, so the workflow was not
executed locally.

## Limits

- A review by another model (Opus 5.5) found no further package. Not checked
  on this host: that Ubuntu's `libwayland-dev` ships `wayland-egl.pc` (from
  Debian packaging knowledge) and that the runner has `pkg-config`.

- The real verifier is the `celestina` workflow run on GitHub after the
  landing; this record does not claim it passes.
- If a test dlopens `libEGL` at run time and the runner lacks it, a further
  package (`libegl1`) would be needed; no test was found that does.

## Landing

- **Base revision:** `5f2e04cc758645009e7c253ba882f0f0faaf6933`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
