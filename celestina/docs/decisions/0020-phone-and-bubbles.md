# ADR 0020: Phone and minimised windows: v1 by default, a guard on the socket, one selector

- **Date:** 2026-10-17
- **Status:** accepted

## Context

CEL-2-J takes over two duties Noctalia holds for the author: the phone piece
(battery, connection and media of the author's phone, read from Magnetita) and
the bubbles of minimised windows (read from Melibea's socket, with the
`Mod+K` selector). Both depend on services that are not this shell's: Melibea
may be older than this shell, the socket may be absent, and a Magnetita bus
name may come and go. The plan
([CEL-2-J](../evidence/2026-10-17-phone-and-bubbles-plan.md))
fixes three decisions, recorded here as they ended.

## Decision

**D1 — Melibea protocol: v1 by default, v2 per action, degrade to v1.**
`subscribe`, `list`, plain `restore`, `close` and `minimize` speak v1, so a
v1-only Melibea keeps serving the shell. A request that carries a transition
hint (an anchored motion from the bubble's rectangle, or `disabled` under
reduced motion) speaks v2. A daemon that answers `incompatible_version` makes
the worker remember v1-only for the life of that connection and resend the
same action as v1 without the hint. A transition is a presentation hint only,
so degrading never changes the state asked for. A reconnection forgets the
memory. The anchor travels whenever the bubble's rectangle is usable and the
shell does not know the window to be on another output. A minimised window
is usually not in niri's model, so the ordinary case is an unknown output and
the anchor is sent, for Melibea or niri to degrade if the output is not the
window's. When the model does name the window's output and it is not the
bubble's, no anchor is sent (`disabled` under reduced motion, otherwise no
hint).

**D2 — The guard when `melibea.service` is absent.** The shell never starts,
stops or probes the unit. It reads one observable: `$MELIBEA_SOCKET` (an
absolute path), else `$XDG_RUNTIME_DIR/melibea.sock`. The path must exist, be
a socket (a symbolic link is always refused, never followed), belong to the shell's uid
and grant nothing to group or others. Anything else is `Absent` or `Refused`.
`Absent` is a normal state: the bubbles piece draws nothing, the selector shows
one note row, the duty stays on, and reconnection backs off from 0.5 s to 30 s.
A connection lost mid-run withdraws the bubbles and follows the same backoff.
Magnetita follows the same shape: a missing bus name is `Absent`, and its
`Changed` and owner-change signals bring it back live.

**D3 — The selector keyboard model.** `Mod+K` runs `celestina bubbles select`,
which reuses the launcher's overlay (`celestina-overlay`, exclusive keyboard,
one card, a field, up to eight rows) with a bubbles source. Typing filters by
title, then app id. `Up`/`Down` and `Tab`/`Shift+Tab` move the selection with
wrap. `Enter` or a click restores the selected window and closes. `Delete`
asks the window to close (`close_requested`) while the field is empty or its
caret is at the end with nothing selected (where it would do nothing in the
field); anywhere else it edits the text. The row leaves only when Melibea's
next revision removes it, and a second `Delete` on a window whose close is
still pending asks for nothing. `Escape`, a press outside the card and the
loss of keyboard focus close without acting. A second `celestina bubbles
select` closes an open selector. Focus starts in the field with the first row
selected, so `Mod+K, Enter` restores the most recently minimised window. There
are no digit shortcuts, no multi-select and no minimise from the selector; the
verb `celestina bubbles minimize` minimises the focused window for the
author's own key binding.

## Consequences

- A Melibea older than this shell still works; the only cost is that bubble
  transitions are not anchored until it is upgraded.
- The shell cannot start, stop or repair Melibea or Magnetita. A missing one is
  a state, shown as nothing, and it never blocks the duty or the loop.
- The selector is a second use of the launcher's overlay, so the overlay's
  focus and close rules are shared; no second surface was added.
- Bounds are deliberate and hostile-input-first: with more than 64 minimised
  windows, or a snapshot over 64 KiB, the link is `Unavailable` in a loop with
  the ordinary backoff and the bubbles stay withdrawn. Showing a truncated
  list would present a projection Melibea never made.
- A connection attempt has no timeout of its own beyond the socket being a
  local Unix socket whose peer is Melibea; reads and writes after it are
  bounded. A Melibea that accepts and never answers is detected by the read
  deadlines, not at `connect`.
- The duty-off to duty-on path no longer joins the old worker on the loop: the
  new worker starts at once, the old one shuts down on its own thread within
  `JOIN_WAIT`, and the run number keeps its last news out of the new state.
- `Ring` is wired in the worker (only for a device it listed) but has no
  caller: it is a future capability with no verb. Magnetita is never
  activated by the shell: the worker asks `NameHasOwner` before any call, and
  the repository ships no D-Bus activation file for `org.celestina.Devices1`
  (only the systemd user unit `magnetita/magnetitad.service`).
- The Phone page has no tab of its own, ever. With the nine tabs of main plus
  a tenth the row does not fit (`no_tab_label_is_wider_than_its_pill` failed
  with the duty on), and the author decided the page opens from the phone
  piece (a click) and from `island open phone`, as the clock and glance
  pages do where no tab applies. `island open phone` is accepted only while
  `duties.phone` is on (with it off it is a no-op with a journal line), and
  turning the duty off while the Phone page is open moves the Island to
  Activity. With the Phone page open no tab is selected; the row is the nine
  tabs, unselected. `page_of` of the Phone page's would-be key still reads
  back without panicking.
- The worker's socket checks are a guard against accidents by the same user,
  not a defence against an adversary who can replace the socket between the
  check and the connection. The residual window is recorded in the evidence.

## Revisit when

Melibea changes its protocol incompatibly (a new version is needed in the
shared contract, not in this shell), the author asks for minimising from the
selector or for digit shortcuts, or a duty is needed that the socket cannot
express.
