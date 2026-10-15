# Evidence: R7 wallpaper, portal values and Niri colours

- **Date:** 2026-08-04
- **Scope:** R7-A through R7-D of [the R7 plan](../plans/archive/2026-08-04-r7-session-look.md)
- **Environment:** Arch Linux checkout at `5ffd81bc321553a59dfbbc1ab3969cd6177d2625` with the uncommitted R7 batch; Rust/Cargo 1.97.1, GCC 16.1.1, CMake 4.4.2, Qt 6.11.1
- **Artifact:** `celestina/build/production-artifact.toml`, `verified`, built from the declared version 0.5.0 and deployed to `~/.local`

## Procedure

```sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py bump celestina milestone --unit R7-A \
    --summary "Add the session look checkpoint"
(cd celestina-rs && cargo test --locked -p celestina-shell-core)
(cd celestina && cargo clippy --all-targets --locked -- -D warnings)
(cd celestina && cargo test --locked)
celestina/scripts/qmllint-production.sh
ctest --test-dir celestina/build --output-on-failure
celestina/scripts/complete-production.sh
```

The portal backend was also run against the author's live session bus, which is
what surfaced the name collision recorded below.

## Result

| Check | Result |
|---|---|
| `celestina-shell-core` tests | 145 passed, including wallpaper, appearance and the colour generator |
| shell helper tests | 25 passed |
| `notification_server` integration test | passed against a private `dbus-daemon` |
| CTest | 13/13, surface-manager now 20 cases |
| Sealed colour contract | OK, 4 colours |
| Rust format and Clippy (`-D warnings`) | clean in both workspaces |
| QML lint, visual and contrast guards | OK |
| Architecture, documentation and language contracts | OK |
| Version contract | OK; celestina 0.4.0 → 0.5.0 |
| `complete-production.sh` | built once, verified those bytes, deployed to `~/.local`; the session was not activated |

## Observed facts

- An output with no image of its own resolves to a deliberate fallback, never
  to another screen's picture, proved by
  `an_output_never_shows_a_picture_chosen_for_another_screen`. The surface
  falls back the same way when a file fails to decode.
- The wallpaper surface is anchored on all four edges, sits on the background
  layer, reserves nothing (`exclusionZone == -1`) and refuses focus.
- The generated Niri include is byte-identical across runs and names the token
  behind each value.
- `scripts/check-sealed-colours.py` reads the theme and the generator and
  refuses a mismatch. It caught this unit shipping the `surface` colour where
  the compositor fallback belonged, before either was committed.
- On the author's live bus the portal backend claims
  `org.freedesktop.impl.portal.desktop.celestina-shell` and publishes
  `{"scheme":1,"serving":true}`.
- The backend answers two keys and refuses every other with the
  specification's own error, so the portal asks whoever owns them instead.

## Limits

- **A name collision was found and corrected here rather than in the author's
  session.** The backend first claimed
  `org.freedesktop.impl.portal.desktop.celestina`, which Siderita already owns
  for `FileChooser`, and its registration file was first generated as
  `celestina.portal` — the exact path Siderita's installed file occupies.
  Following those instructions would have taken the session's file chooser
  away. The shell now uses its own name and file name, a test asserts the two
  differ, and Siderita's installed file was confirmed intact.
- Nothing generated here was installed. The Niri include and the `.portal`
  file are written under `$XDG_DATA_HOME/celestina/generated/` and referenced
  by the author or not.
- No wallpaper was displayed: the surface description was checked offscreen,
  where LayerShellQt declines to create a layer surface at all. Appearance,
  hotplug on physical monitors, an application actually reading the portal
  values and Niri drawing the generated colours remain `VAL-R7`.

## Follow-up

- `VAL-R7` is deferred until the author runs it against the deployed bundle.
- R6 stays conditional on SHELL-D2, and R8's Polkit and dock slices on SHELL-D3
  and SHELL-D4. The roadmap is idle.
