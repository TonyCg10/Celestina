# CelestinaStyle status

- **Updated:** 2026-08-12
- **Implementation:** the shared source contract has a 1.5.0 milestone
  prototype with the two reading controls, demonstrated shell glyphs, additive
  `GlassSurface.ExternalBackdrop` mode, compatible opt-in `ContentSurface` and
  `ContextualVeil` roles, and one opt-in vector silhouette for an
  external-backdrop material; `STYLE-G7-J` remains `active` until its immutable
  delivery record and commit exist, `STYLE-G7` stays active and
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
  `ContentSurface` applies strength to the complete decorative stack;
  `ContextualVeil` attenuates tint and noise while suppressing outline and
  lit-edge layers entirely. Both roles are shadowless, never capture another
  Wayland client and preserve one compositor region for the complete menu.
- The 1.5.1 delivery adds one glyph, `minus`, and its catalogue entry. The
  catalogue carried `plus` and not its pair, so a consumer building a stepper
  had a raise with no lower — which is what the shell's per-monitor brightness
  and per-application volume rows needed. The icon is the vendored Lucide
  shape at the catalogue's own 2.5 stroke width, and the case that covers it
  names the two together, because a catalogue that can only raise leaves every
  consumer of it unable to lower anything. Recorded under `STYLE-G7-L`.
- The 1.5.0 milestone corrects the shared icon vocabulary in two ways, neither
  of them a size change. `CelestinaIcon` asked for its vendored SVG at the
  item's logical size; `IconImage` renders that SVG once at exactly the size
  requested, so on any output above 1.0 scale the compositor received a pixmap
  smaller than the physical area it filled — the pixelation the author saw on a
  fractionally scaled monitor and never on an integer-scaled one. The requested
  size now follows the screen's real device pixel ratio. Separately, the
  catalogue's stroke width rises from 2 to 2.5 across all 96 icons, because the
  author asked for thicker glyphs at unchanged dimensions: this is weight, not
  scale, and it reaches every consumer in the suite because the catalogue is
  shared. Recorded under `STYLE-G7-K`.
- The 1.4.0 milestone prototype leaves that rounded default unchanged and adds
  `GlassSurface.silhouettePath` only for an explicitly shaped external
  backdrop. The path paints the same semantic tint and edge vocabulary without
  a rectangular shaped-surface shadow, while the optional
  `silhouetteEdgePath` can omit an edge that must remain visually open. The
  semantic `ContextualVeil` role suppresses both outline and lit edge on
  rounded and shaped paths without adding a mutable content-surface switch. The
  current Celestina composition reuses the existing `ContextualVeil` role for
  one continuous panel backdrop and keeps its inset `ContentSurface` capsules
  on the ordinary rounded path at all times. Only the contextual membrane uses
  the generic shaped path, and it uses `ContextualVeil` alone. Style retains
  only proportional vertical travel; Celestina owns the droplet membrane: one
  narrow icon-proportional mouth clinging to the bar seam with a meniscus,
  centred on the clicked glyph and clamped inside the body's flat top span, a
  hanging neck that tension only thins, and a concave swell landing tangent
  on the body's top edge inside its ordinary rounded corners.
  Celestina also owns the panel region, derived polygon, live anchor tracking,
  placement and opener policy. None of those ratios, curve controls or tension
  calculations adds a Style token or API. The shell likewise owns the tokened
  state that keeps only the opener's ordinary hover feedback visible while its
  surface is open; this introduces no Style property or component contract.
  There is no dense bridge or cross-window material transition. The veil
  suppresses rounded and shaped outline/lit-edge layers so neither body leaves
  an apparent edge halo; no compositor or shell layout policy enters this
  module.
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
The preceding droplet-pill, fixed-anatomy, narrow proportional-connector,
whole-capsule/dense-bridge, glyph-mouth, body-wide icon-scaled and fluid
body-proportional-waist experiments
remain as explicitly superseded prototype evidence. The glyph-mouth revision's
focused consumer CTest 4/4 and QuickTest 208/208 do not verify either later
geometry. The body-wide icon-scaled revision's focal QuickTest 210/210,
canonical Style verification with 29 production-common fixtures and CTest 1/1,
and registered Celestina completion with CTest 17/17 verify that superseded
revision only. The fluid body-proportional-waist revision verified cleanly at
211/211 but the author rejected its live hourglass read on 2026-08-11; it
does not verify the current droplet mouth, neck and tangent landing. For the
current revision, canonical Style verification passes
production-common 29/29, the architecture, contrast and QML visual guards,
`qmllint` with only the pre-existing `CelestinaLineGutter` warnings, CTest 1/1
and the eight-second smoke. Registered Celestina completion passes the full
Rust suites, CTest 17/17 and its eight-second release smoke, then reports the
deployed artifacts current and verified without activating a session.
Author-visible consumer validation remains pending. The exact compatibility
and evidence boundary is recorded in
[edge-attached glass silhouette](docs/evidence/2026-08-11-edge-attached-glass-silhouette.md).
Compositor and AT results belong only to [VALIDATION.md](VALIDATION.md).

## Records

- [Visual contract](DESIGN.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Registry entry](../docs/projects.toml)
