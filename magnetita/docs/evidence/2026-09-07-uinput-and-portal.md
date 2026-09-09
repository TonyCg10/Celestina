# `uinput` access and the RemoteDesktop portal on the author's host — MAG-P0-D

- **Date:** 2026-09-07
- **Scope:** `MAG-P0-D` of
  [`../plans/archive/2026-09-07-own-protocol-spikes.md`](../plans/archive/2026-09-07-own-protocol-spikes.md);
  verifies the [input discussion](../discussions/2026-09-04-remote-input-on-wayland.md)
- **Environment:** CachyOS, Linux 7.2.3, niri 26.04 (`8ed0da4`) as the live
  session, `xdg-desktop-portal` with the author's `portals.conf`; scratch
  crate `uinput-probe` on `evdev 0.12.2`
- **Artifact:** not applicable

## Procedure

```sh
getfacl -p /dev/uinput
grep -rh uinput /usr/lib/udev/rules.d /etc/udev/rules.d
busctl --user introspect org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop | grep RemoteDesktop
grep Interfaces /usr/share/xdg-desktop-portal/portals/gnome.portal /usr/share/xdg-desktop-portal/portals/wlr.portal
cat ~/.config/xdg-desktop-portal/*.conf
./target/release/uinput-probe        # create + destroy, no event emitted
```

## Result

- **Exit:** 0 for the probe
- **Observed:**
  - `/dev/uinput` is `root:root 0660` with an ACL `user:toni:rw-`. The ACL
    comes from `TAG+="uaccess"` in
    `/usr/lib/udev/rules.d/42-logitech-unify-permissions.rules` (Solaar's
    rule), which logind applies to the seat's active user. No udev rule of
    the suite's own is needed today.
  - The probe created a virtual pointer (`/dev/input/event22`) and a
    virtual keyboard (`/dev/input/event23`) as this user, printed their
    nodes, emitted nothing and destroyed both; no residual device in
    `/proc/bus/input/devices`.
  - The portal frontend exposes `org.freedesktop.portal.RemoteDesktop`, but
    the only backend declaring the implementation interface is
    `gnome.portal` (Mutter's), which cannot serve under niri; `wlr.portal`
    declares only `ScreenCast` and `Screenshot`, and the author's config
    routes those to `wlr`. niri 26.04 provides no RemoteDesktop backend.
    The discussion's falsifier — a working RemoteDesktop implementation on
    this session — is not met, so `uinput` stands.

## Limits

- The access depends on a rule shipped by Solaar and on being the seat's
  active user. If Solaar is removed, `/dev/uinput` reverts to root-only and
  the suite needs its own `uaccess` rule; `HOST-HYGIENE.md` should record
  that dependency when `MAG-P5-B` lands. A daemon started outside the seat
  session (for example over SSH) would not get the ACL.
- The injection half — pointer motion and a typed sentence observed in the
  development nest — was not run: no nest was running, and the recorded
  rule forbids injecting into the live session. The probe's `--type` mode
  exists for that run.

## Follow-up

Run `uinput-probe --type` against a running nest and append the
observation; the input discussion cites this record for availability.
