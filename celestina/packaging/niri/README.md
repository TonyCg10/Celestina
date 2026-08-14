# The compositor patch Celestina's dense glass wants

Celestina's material carries two blur strengths in one card: the colourless
veil keeps the session's ordinary slight sample, and the dark content sections
summarize the colours behind them at a much stronger one. A compositor grants
one blur strength per surface, so the shell asks for the second one by keeping
invisible companion surfaces under the dark sections — and those companions
need a strength of their own.

`per-layer-blur-strength.patch` adds `passes` and `offset` to niri's
`background-effect` rule, beside the `noise` and `saturation` overrides
upstream already carries there. It completes an existing pattern rather than
introducing a mechanism: about fifty lines across two files, no shader and no
render-pipeline change, because niri already blurs per surface.

## Why the shell does not simply stack more companions

Stacking works and needs no patch — several companions compose several samples
— but blur radius grows with the square root of the sample count, so matching
`passes 4, offset 6` from a `passes 2, offset 2` profile needs roughly
twenty-five stacked surfaces. Measured, not estimated: three companions left
the wallpaper's houses and paths plainly legible through the cards.

## Building it

    scripts/build-patched-niri.sh

It clones niri, applies this patch, builds, and installs the result to
`~/.local/lib/celestina/niri`. Nothing outside that directory is touched: the
distribution's own `/usr/bin/niri` stays exactly as it was, and the login
session keeps using it until someone deliberately changes that.

`dev-session.sh` picks the patched binary up automatically when it is there,
and falls back to the session's niri when it is not.

## Running the live session on it

This is the author's decision, not the script's, because it replaces the
compositor of a daily driver. The systemd user unit is what chooses the
binary:

    systemctl --user edit niri.service

with

    [Service]
    ExecStart=
    ExecStart=%h/.local/lib/celestina/niri --session

The patched binary is a fork: `pacman -Syu` will keep updating `/usr/bin/niri`
and will never update this one. Re-run the build script after a niri release,
or drop the override and go back to stock at any time — the shell degrades to
one blur strength and nothing else changes.

## Sending it upstream

The patch is worth proposing to niri: the override it adds is the one a
compositing shell needs to give a material more than one blur strength, and
the same rule already accepts two other per-surface overrides. Until then it
lives here, versioned with the shell that wants it.
