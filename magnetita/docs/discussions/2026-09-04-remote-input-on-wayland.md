# Remote input on the desktop: `uinput` or the RemoteDesktop portal

- **Opened:** 2026-09-04
- **Status:** applied
- **Question:** how does the daemon turn the phone's trackpad and keyboard
  events into pointer and key input on the author's Wayland session?

## Context

The trackpad and keyboard capability is on the author's list. On Wayland no
client may inject input into another; the two honest routes are a virtual
device the kernel presents to the compositor (`uinput`), or the compositor's
own RemoteDesktop portal with `libei`. The author's compositor is niri. The
choice blocks `MAG-P5` and decides whether deployment needs a udev rule.

## Strongest case

`uinput` works on every compositor today: the daemon creates one virtual
pointer and one virtual keyboard through `/dev/uinput`, and niri treats them
as real devices. It is one small safe crate (`evdev`/`uinput` bindings, or
`rustix` ioctls) in the daemon's existing bounded style, and the devices are
created and destroyed with the session, so nothing lingers. Absolute pointer
motion, relative motion, scroll and key codes are all native.

## Counter-case

`/dev/uinput` is root-owned; the author's user needs a udev rule granting the
`input` group (or a dedicated group) write access, which is a host change
outside the repository and must be recorded in `HOST-HYGIENE.md` and done by
the author. A virtual keyboard also has to deal with keymaps: the phone sends
text, the daemon must map it to key codes for the session's layout, and
anything outside the layout needs a Unicode composition path. The portal
route needs no privilege and hands the keymap problem to the compositor,
but niri would have to implement the RemoteDesktop portal with `libei`, which
must be verified on the installed version rather than assumed.

## Alternatives

- `xdg-desktop-portal` RemoteDesktop + `libei` (`reis` crate). Preferred
  whenever the author's compositor supports it; unprivileged and keymap-aware.
- niri's own IPC, if it exposes input injection. Compositor-specific; would
  need its own decision.
- `ydotool`-style external daemon. A subprocess with root, which this suite
  refuses on principle.

## Falsifiers and evidence needed

`MAG-P0-D`: a udev rule on the author's host grants `/dev/uinput`; the daemon
moves the pointer and types a sentence into the development nest, never the
live session, following the recorded rule against blind input injection.
In the same spike, `busctl --user introspect
org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop` on the
author's session shows whether a RemoteDesktop implementation is present; if
niri provides one, the portal path is tried against the nest and wins if it
works.

## Conclusion

**`uinput`**, concluded by the author on 2026-09-04, because it works on the
installed compositor today and fits the daemon's owned-device discipline.
The udev rule is a recorded host change in `HOST-HYGIENE.md`, applied by the
author. The RemoteDesktop portal with `libei` is the named successor: the
ADR's *Revisit when* moves input to the portal as soon as niri exposes it,
and `MAG-P0-D` records whether it already does. Applied in
[ADR 0001](../decisions/0001-own-protocol-and-android-app.md) §10 and
`MAG-P5-B`. Verified on 2026-09-07: `/dev/uinput` is already granted to
the author by Solaar's `uaccess` rule, a virtual pointer and keyboard were
created and destroyed as the user, and niri 26.04 ships no RemoteDesktop
backend (only Mutter's declares it) — see
[the availability record](../evidence/2026-09-07-uinput-and-portal.md). The
nest injection run is still owed.
