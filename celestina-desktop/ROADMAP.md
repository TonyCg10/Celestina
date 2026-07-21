# Celestina Desktop roadmap

## Now — S0: visible and truthful panel

**Outcome:** a clean build launches one correct Niri top panel on every
connected output, showing real local time while Noctalia remains available as
the session fallback, without fake controls or checkout-relative dependencies.

### In

- reproducible Qt/LayerShellQt development environment;
- installed CelestinaStyle consumption;
- explicit per-output map, hotplug, geometry, exclusive-zone and no-focus
  lifecycle;
- a distinct layer-shell namespace for testing beside Noctalia;
- minute-aligned real clock and actionable startup failures;
- one small Rust/QML bridge and executor spike for the next milestone;
- build, startup and resource baselines.

### Out

- live workspace IPC, status tray, multi-output breadth and session packaging;
- launcher, taskbar, lock screen or other shell surfaces;
- replacement of Noctalia-owned idle, Polkit, night-light or greeter sync;
- application workflows from Siderita/editor/viewer/player;
- a generic compositor or plugin framework.

### Work

1. Create the first recoverable Git baseline and record tool versions.
2. Make clean configure/build/lint reproducible.
3. Stage CelestinaStyle and remove the sibling source dependency.
4. Make panel creation per-output, hotplug-safe, visible, fail-fast, uniquely
   namespaced and geometrically correct on Niri without accepting keyboard
   focus.
5. Remove fake workspace controls and tray spacing; update the clock only at
   minute boundaries.
6. Probe the Rust/QML bridge, bounded queue and deterministic shutdown needed
   for S1; keep it separate from the S0 user surface.
7. Test with Noctalia still running but its bar hidden, then record artifact,
   startup, memory, wakeup and relevant GPU baselines.

### Exit

- a clean build and staged Style dependency require no sibling checkout;
- exactly one panel maps on every connected output with correct height,
  exclusive zone, namespace and no focus theft while Noctalia remains running;
- the clock crosses a minute boundary and every visible action is truthful;
- invalid imports/root/layer setup fail visibly and non-zero;
- the bridge probe survives updates, overload and shutdown without stale UI
  state, leaks or cross-thread model mutation.

## Next — S1: daily Niri panel

Keep the per-output lifecycle while observing Niri's event stream through a
Rust adapter and showing real workspaces plus the focused window. Focus requests
show pending/failure/confirmed states; the panel survives IPC loss and Niri
restart without presenting stale state.

## Next — S2: dependable personal session

Install an opt-in Niri startup contract and compose a launcher, notification
daemon, lock/idle tools, portals, keyring, Polkit agent, wallpaper and optional
Xwayland compatibility. Provide dependency diagnostics and verified fallback
keybindings before Noctalia is removed from autostart.

## Later

- Add audio, battery or brightness status when the personal hardware workflow
  proves that the panel needs it.
- Consider tray, launcher or notifications one at a time, based on recurring
  friction with the external component.
- Add a settings UI, richer theming or another shell surface only after file
  configuration and the daily session are stable.

## Non-goals

Do not pursue Noctalia feature parity, support another compositor, build a
plugin framework, own security-sensitive lock/auth flows, plan other Celestina
applications here, fork Niri, overwrite unrelated dotfiles, or add a shell
surface without a demonstrated daily workflow gap.
