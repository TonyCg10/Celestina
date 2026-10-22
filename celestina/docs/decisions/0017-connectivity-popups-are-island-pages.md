# ADR 0017: A connectivity piece opens an Island page, not a popup surface

- **Date:** 2026-10-17
- **Status:** proposed

## Context

CEL-2-F adds four pieces to the bar: network, Bluetooth, battery and
power profile. Each shows state and offers a short list of actions. The bar
takes no keyboard yet, and the piece contract requires every action to be
reachable from a page or a verb. A popup card beside the bar would be a second
path with its own grab, dismissal and focus policy, and no keyboard. The
Island already has tabs, Escape, click-outside, reduced motion and focus.
The plan of CEL-2-F (decision D1) asked the author to confirm this before the
Island integration. The author approved D1 in chat on 2026-10-17; the session
recorded that here, so the approval is not the author's own edit of the
repository. The status stays `proposed` until the author sets it: this
directory's convention names no case where a session accepts a record.

## Decision

The popup of a connectivity piece is an Island page, not an `xdg_popup`:

- `network` opens the `network` page;
- `bluetooth` opens the `bluetooth` page;
- `battery` and `power-profile` open the `power` page.

The power-profile piece also cycles the profile on a middle click, through
the same action route as the page's rows.

## Consequences

- The Island grows instead of a small card hanging under its icon. The tab
  strip has six pages in the default body; the pills now size to their
  titles, which is a change of the tab strip shared with the older pages.
- The state, the workers and the validation do not depend on the surface, so
  a card popup can replace the page later inside `pieces/` and `island/`.
- No new popup shell, no second dismissal policy.

## Revisit when

The author asks for a card under the piece, or the six-tab strip cannot be
read at the author's text scale (measured at about 12 px over the body at the
"Larger" scale, with the titles clipped slightly).
