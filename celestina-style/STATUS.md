# CelestinaStyle status

- **Updated:** 2026-08-11
- **Implementation:** the shared source contract is delivered at 1.3.0 with the
  two reading controls, the demonstrated shell glyphs, one additive
  `GlassSurface.ExternalBackdrop` mode and compatible opt-in `ContentSurface`
  and `ContextualVeil` roles; `STYLE-G7-F` records that cumulative prototype
  snapshot, `STYLE-G7` remains active for the next demonstrated design need and
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
- The verified 1.3.0 extension adds only `toolbox`, `pin`, `eye`, `eye-off` and
  `system-tray`. `toolbox` maps to Lucide's literal tool case rather than its
  briefcase; `system-tray` maps to the bounded inbox anatomy instead of the
  launcher grid or generic application window. All tray and wallpaper behavior
  remains local to Celestina.
- The same 1.3.0 delivery keeps `InSceneCapture` as the compatible default and adds
  an explicit external-backdrop path. That path never activates
  `ShaderEffectSource`; it renders the same tint, noise, outline, lit edge and
  readable fallback above a compositor effect owned by the consuming host.
- The same 1.3.0 delivery leaves the default material pixel-compatible. Celestina
  alone opts its information-bearing menu cards and panel capsules into one
  `ContentSurface`, while its contextual carrier uses `ContextualVeil`.
  Strength applies to tint, noise, outline and lit edge together; both roles
  are shadowless, never capture another Wayland client and preserve one
  compositor region for the complete menu.
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
The external-backdrop prototype passed the same production guard, lint, QtTest
and gallery smoke on 2026-08-11; exact commands and limits are in the
[external-backdrop evidence](docs/evidence/2026-08-11-external-backdrop-glass.md).
The final 1.3.0 snapshot passed the same canonical production workflow and its
focused semantic-role construction tests; exact results are recorded in
[semantic glass material roles](docs/evidence/2026-08-11-semantic-glass-material-roles.md).
Compositor and AT results belong only to [VALIDATION.md](VALIDATION.md).

## Records

- [Visual contract](DESIGN.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Registry entry](../docs/projects.toml)
