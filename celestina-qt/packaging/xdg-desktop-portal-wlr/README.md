# The screen-sharing backend patch the session needs

The session shares a screen through `xdg-desktop-portal-wlr`: Celestina owns
only the "which screen?" dialog (`celestina --pick-output`), and the backend
captures the output and hands it to PipeWire. niri gives that backend
wlr-screencopy and nothing newer.

Release 0.8.x of the backend drives the PipeWire graph itself
(`PW_STREAM_FLAG_DRIVER`), so a stream advances only when the backend asks for
the next cycle with `pw_stream_trigger_process`. Only its ext-image-copy-capture
path asks; the wlr-screencopy path never does. On this compositor every shared
screen therefore delivered the one frame primed at stream start and froze, in
every application, with no error anywhere.

`trigger-process-after-screencopy.patch` carries upstream's fix
([c0255d7b](https://github.com/emersion/xdg-desktop-portal-wlr/commit/c0255d7b),
2026-08-11, in no release as of 0.8.4) onto the released tag: each place the
screencopy path hands a buffer back to PipeWire now also asks for the next
cycle. It is a backport, not Celestina's own change, and it exists only until
a release contains the commit.

## Building it

    scripts/build-patched-xdpw.sh

It fetches the tag matching the installed package, applies the patch, builds
with meson (privately installed into `~/.cache/celestina/xdpw-tools` when the
host has none) and installs the binary to `~/.local/libexec`. Nothing outside
that directory is touched: the distribution's own
`/usr/lib/xdg-desktop-portal-wlr` stays as it was.

## Running the session on it

    scripts/build-patched-xdpw.sh --install-override

writes a systemd user drop-in for `xdg-desktop-portal-wlr.service` that points
its `ExecStart` at the built binary, reloads and restarts the service. An
active screen share is interrupted by that restart; nothing else in the session
is. The backend keeps reading the session's own
`~/.config/xdg-desktop-portal-wlr/config`, chooser included.

## Keeping it working

`pacman -Syu` updates the package and never this binary. When a release lands
with the commit above, delete
`~/.config/systemd/user/xdg-desktop-portal-wlr.service.d/override.conf`,
`systemctl --user daemon-reload` and restart the service. The build script
refuses when the patch no longer applies, which is the signal to check.
