# CelestinaStyle status

- **Updated:** 2026-08-03
- **Implementation:** the shared 1.0 source contract is live; `STYLE-M1` is
  planned and has no active execution plan
- **Author validation:** previous real-session review exists, with focused
  follow-ups pending in [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- `qmldir` and `CMakeLists.txt` expose the same canonical type set, including
  `CelestinaSlider`, `CelestinaInputShield` and the filled folder/file icons.
- Siderita, Magnetita, Grafita and Fluorita consume canonical files through
  relative links. The shell imports the same source module in its panel,
  chooser and newer overlay/provider surfaces.
- The declared compiled-module floor is Qt 6.9. `DESIGN.md` also records APIs
  observed on the author's newer Qt environment; those observations do not
  silently raise the minimum.
- Tokens, glass, semantic surfaces, controls, Lucide icons, Inter Variable,
  content icons and host-controlled reduced motion are implemented.
- The installed/versioned module is not current delivery architecture. It
  remains gated on a consumer outside this repository.

## Planned implementation debt

- Define and enforce the compatibility/deprecation policy for the 1.0 public
  QML surface.
- Finish the finite legacy-motion audit and make missing reduced-motion routes
  reproducibly detectable.
- Complete the written font fallback and mono-face contract.
- Keep in-scene glass, compositor glass and translucent fallback terminology
  unambiguous in public APIs and documentation.

Further controls are demand-driven and are not an open widget backlog.

## Evidence boundary

The S1-S6 migration, component follow-ups and recorded captures through
2026-08-03 are preserved in the
[historical roadmap](docs/history/roadmap-through-2026-08-03.md). On 2026-08-03
the canonical compiled module passed its contract guards, `all_qmllint`, CTest
1/1 and an eight-second compiled-module smoke; all registered consumers were
also verified through their own production entries. Exact commands and limits
are in the suite [evidence](../docs/evidence/2026-08-03-repository-governance.md).
Compositor and AT results belong only to [VALIDATION.md](VALIDATION.md).

## Records

- [Visual contract](DESIGN.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Registry entry](../docs/projects.toml)
