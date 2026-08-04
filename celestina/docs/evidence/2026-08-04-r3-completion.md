# Evidence: the R3 registered production exit

- **Date:** 2026-08-04
- **Scope:** R3-Z of [the R3 plan](../plans/archive/2026-08-03-r3-session-verbs.md)
- **Environment:** Arch Linux checkout at `6e48ec8f9dd45eb24c14e6c325f8381f8a5e3848`; Rust/Cargo 1.97.1, GCC 16.1.1, CMake 4.4.2, Qt 6.11.1
- **Artifact:** `celestina/build/production-artifact.toml`, now `verified`, built from the declared version 0.2.0

## Procedure

The delivery itself landed earlier as `4fdcb35`. This unit is the exit that
commit could not run at the time, because the suite-wide architecture guard was
red on another project's uncommitted change. That change has since landed, so
the registered entry was run whole:

```sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
celestina/scripts/complete-production.sh
```

## Result

| Step | Result |
|---|---|
| Architecture, documentation and language contracts | OK; the language baseline holds 160 ratcheted legacy files |
| `complete-production.sh` build | one release build of the 0.2.0 bundle, caches reused |
| `complete-production.sh` verify | CTest 13/13, QML lint, Rust checks and an eight-second offscreen smoke against those exact bytes |
| Manifest | `celestina/build/production-artifact.toml` moved to `verified` |
| Deploy | bundle copied to `~/.local`; `celestina`, both helpers, `libcelestina-style.so`, the `CelestinaStyle` module and the `celestina` entry point all report installed |
| Status | `artifact: celestina current and verified` |

## Observed facts

- The deployed bytes are the verified bytes: deploy compiled nothing and the
  status check compares digests rather than timestamps.
- The session was not activated. No live process was replaced, no service was
  enabled, no package manager or system prefix was touched and no Niri
  configuration was read or written.

## Limits

- This is the automated exit only. It proves the bundle builds, passes its own
  checks, starts offscreen and installs — nothing about Wayland geometry, the
  OSD on a real compositor, gamma actually warming, an idle inhibitor actually
  holding logind, monitors blanking, DDC hardware or AT-SPI.
- `VAL-R3` therefore remains deferred and is the author's to run against the
  now-deployed bundle. R3's implementation closes on this record; a failed
  manual check opens a new corrective unit rather than reopening the
  checkpoint.

## Follow-up

- Run `VAL-R3` when the author chooses to, and record the observations in
  [VALIDATION.md](../../VALIDATION.md).
- The next implementation checkpoint (R4, notifications) starts with its own
  plan; the roadmap is idle until one is opened.
