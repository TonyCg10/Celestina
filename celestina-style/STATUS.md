# CelestinaStyle status

- **Updated:** 2026-08-08
- **Implementation:** the shared source contract is live at 1.2.0 with the two
  reading controls and the panel's finite status-glyph set published;
  `STYLE-G7` is the active checkpoint and
  `STYLE-M1` remains planned with no execution plan
- **Author validation:** previous real-session review exists, with focused
  follow-ups pending in [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- `CelestinaScrollBar` and `CelestinaLineGutter` are published: a scroll
  position built from primitives so its whole anatomy comes from tokens, and
  line numbers that build only the delegates a viewport shows. Grafita's window
  and Siderita's two text surfaces are their consumers.
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
- The semantic Lucide catalogue includes the finite network, Bluetooth,
  resource, notification, power-profile, brightness and session glyphs used by
  Celestina's accepted panel baseline. The shell still owns their state,
  accessibility copy and interaction.
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
