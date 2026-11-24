# Evidence: CEL-2-ICONS — the painter's missing `.desktop` icons

- **Date:** 2026-11-24
- **Scope:** `celestina/crates/celestina-paint/src/{icon,painter}.rs` (a
  file icon key, the bounded icon read and the icon theme's choice, with
  their tests); the producers of `.desktop` icons in `celestina/src`
  (`launcher/view.rs`, `island/pages/activity.rs`, `overview/view.rs`,
  `providers/niri.rs`) and the bubbles' filler (`bubbles/mod.rs`); this
  unit's ledger row. No dependency changed.
- **Environment:** CachyOS, worktree `celestina-CEL-2-ICONS` cut from `main`
  at `44b8cc2a` (celestina 1.14.0) and rebased onto `main` `4144a54e`; toolchains 1.97.1 (pinned) and 1.100.0
  (the runner's stable).
- **Artifact:** none sealed by this record; the landing builds its own.

## What was wrong

The author's journal of the live shell (1.13.x, 2026-11-23/24) warned
`no such icon` from `celestina_paint::painter` for icons the host has:

```text
no such icon icon=Theme("utilities-system-monitor")
no such icon icon=Theme("phone")
no such icon icon=Theme("/home/toni/.local/share/waydroid/data/icons/com.android.vending.png")
no such icon icon=Theme("/home/toni/.local/share/icons/hicolor/512x512/apps/totk.png")
```

`utilities-system-monitor` is the `Icon=` of `amdgpu_top.desktop`, `phone`
of a user `.desktop` file; the Waydroid entries and `totk.desktop` name an
absolute path. Two causes:

1. `theme_path` refused every name with a `/` (`valid_name`), so an absolute
   `Icon=` path, which the Desktop Entry specification allows, never
   resolved.
2. The painter took its theme from `freedesktop_icons::default_theme_gtk()`,
   which asks `gsettings` and answers only when that theme is installed (and
   then with the index's `Name=`, not the directory name). On this host
   gsettings names `WhiteSur-dark`, which is not installed, so the theme was
   `None` and every name was looked up in hicolor alone, which has neither
   `phone` nor `utilities-system-monitor`. The GTK 3 `settings.ini` names
   `Adwaita`, installed, which has both (through `AdwaitaLegacy`).

## What changed

- `IconKey::Theme` stays a theme name only: a name with a `/` resolves to
  nothing, as before, because tray items, notifications and Melibea send
  their strings there over a bus or a socket.
- A new `IconKey::File(PathBuf)` is built only by `IconKey::desktop_icon`
  from a `.desktop` entry's `Icon=`: the launcher's applications and the
  window icons niri's `AppIcons` finds (Activity, the overview). `AppIcons`
  no longer drops an absolute `Icon=`. A file key is drawn only when it is
  absolute, `png` or `svg` (any case) and a regular file.
- `bubbles::fill_icons` drops a path `AppIcons` finds for a Melibea window,
  whose icon is drawn as a theme name: those bubbles keep the initial they
  showed before, with no new warning.
- `rasterise_file` opens the file once, refuses a handle that is not a
  regular file (a symlink to `/proc` or a device), and reads at most
  `MAX_ICON_FILE + 1` bytes instead of trusting the reported length.
- The theme is the first installed directory among the gsettings value, the
  `gtk-icon-theme-name` of `$XDG_CONFIG_HOME/gtk-3.0/settings.ini`, of
  `gtk-4.0/settings.ini`, and `Adwaita`; none installed leaves hicolor.
  "Installed" is a `<name>/index.theme` under `~/.icons`,
  `$XDG_DATA_HOME/icons` or an `$XDG_DATA_DIRS/icons`. The choice is a pure
  function (`choose_theme`) tested over scratch directories; the live
  wrapper runs `gsettings` once, at painter construction, as before, and
  logs the theme and where it came from.

## Procedure

Run in the worktree, `celestina/`:

```text
cargo fmt --all --check                                    -> clean
cargo test --workspace --locked                            -> 49 test binaries, 0 failed
                                                              (celestina-paint lib: 58 passed)
cargo clippy --workspace --all-targets --locked -- -D warnings          (1.97.1) -> clean
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings          -> clean
cargo build --release --locked                             -> built
scripts/smoke.sh --binary <private target>/release/celestina -> smoke: OK, exit 0
```

The nested smoke ran on the release binary of this tree (sha256
`a7f4e0daf5e2edd0f87620bf4585534d872bb723859ca3aef14eace84d669691`), with the
lock pair copied beside it into a target directory of the session's own, so
the fake system bus was built there and no other session's build could
replace the binary mid-run. A first attempt from a plain copy stopped at the
connectivity run with "the binary is not under a debug or release target
directory", the script's own precondition. Lines of the passing run:

```text
smoke: bar on: winit frames=953 rest_frames=0 ... (rest_frames=0)
smoke: glance: frames=1770 rest_frames=1 curl_calls=0
smoke: clipboard: winit frames=2783 rest_frames=0 ... (rest_frames=0, duty switches=2, history changes=6)
smoke: connectivity on the fake bus: winit frames=2038 rest_frames=0 ...
smoke: phone and bubbles on: winit frames=977 rest_frames=0 ...; Melibea subscribed with one window, Magnetita absent
smoke: media phase skipped: .../examples/fake-mpris-player is not built (cargo build --locked --example fake-mpris-player)
smoke: cutover: every duty on then off: winit frames=2638 rest_frames=1 ... (rest_frames=1, at most 13 over 2 surfaces)
smoke: cutover: rss_mib=102.9
smoke: OK
```

The media phase skipped itself because its example was not built in the
private target; this unit touches no media code, and the landing's own
verification runs the smoke again on the tree it commits.

## Result

Every command above passed. New tests in `icon.rs`: `an_absolute_png_or_svg_file_resolves_to_itself`,
`absolute_paths_that_are_not_icon_files_resolve_to_nothing`,
`settings_ini_parsing_trims_quotes_and_skips_comments`,
`the_gsettings_theme_wins_when_it_is_installed`,
`a_missing_gsettings_theme_falls_back_to_the_settings_files`,
`adwaita_is_the_last_resort_and_nothing_installed_means_hicolor`. Five of
the six failed against stubs before the fix.

After the rebase onto `4144a54e`, fmt, the workspace tests (46 test
binaries, 0 failures) and both Clippies passed again on this branch alone.
The nested smoke was rerun on the stacked CEL-2-ICON-READ tree, which holds
this unit unchanged; see that unit's record.

## Review

Reviewed twice by a model other than the implementer, at high reasoning.
The first review found that the first version's absolute-path branch in
`IconKey::Theme` let a tray item's `IconName` (any session-bus peer) name a
file the shell would read, including a symlink to `/proc/self/pagemap` that
the length cap did not bound; the design above replaced it. The second
review found no remote string that reaches `IconKey::File` or a file read:
a window's `app_id` only selects a `.desktop` file inside the applications
directories (`is_desktop_id` refuses a `/`, and the entry is read bounded).

## Limits

The live check is the author's: after the deployment, open the launcher and
see the Waydroid, `totk`, `amdgpu_top` and phone entries with their icons,
and `journalctl --user -t celestina | grep "no such icon"` stay quiet for
them.

Left open, low, same-user only, and older than this unit: opening an icon
file does not use `O_NONBLOCK`, so a FIFO planted in an icon theme or named
by the user's own `.desktop` file still blocks the render thread (the
non-blocking open lives in `celestina-core`, which `celestina-paint` does not
depend on); and an SVG's `<image href>` is read by usvg's default resolver
without a bound. Both need a decision outside this bug.

## Landing

- **Base revision:** `4144a54e7588c7113eb62dd250aa84328fd79955`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:b7a3778cc38ca1cf2e1dfaeef219e45b46810188670a9747e40a9bbec1f3c74a, verification_fingerprint sha256:8667d89778833e7e352026551c22816f05ccd565fad0d5f62cc9af70e220ded6
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
