# Magnetita Android — design

- **Status:** accepted direction, 2026-09-09
- **Language:** Samsung One UI on the phone it was made for, in Samsung blue
- **Lineage:** the author's MilaHub application — its token philosophy and
  its One UI components, not its screens

## Direction

Magnetita on the phone is a control surface you glance at: is the desktop
here, what does it want, one press to answer. The language is One UI as
Samsung ships it, which is what the S25U's owner already has in hand:
pure black, one accent, big rounded groups, a large title that collapses
into a toolbar, floating pills for navigation, sheets that rise from the
bottom with generous radii and no handle. MilaHub proved that language in
pink and mint; Magnetita takes the same rules in blue.

Five rules, in the order they win:

1. **Black is the canvas.** Background is `#000000`; surfaces are
   near-black tinted toward the accent, never grey. Depth comes from tint
   and blur, not from shadows or borders.
2. **One accent, one semantic.** The accent is Samsung blue; it marks what
   is interactive and what is the desktop. The one semantic colour is a
   cool mint for *link alive*; red is reserved for a refusal.
3. **Type carries the hierarchy.** A 32 sp medium title with tight tracking
   collapses to a 20 sp bold toolbar title; body is 16/24. No icon labels
   in caps.
4. **Shapes say what a thing is.** 26 dp for grouped cards and dialogs,
   32 dp for sheets, 18 dp for buttons, pills only for switches, chips and
   the floating navigation strip.
5. **Motion is one clock per gesture.** A sheet rises once; the navigation
   pill shrinks to 0.97 while a sheet is up; nothing bounces.

## Tokens

Dark is the primary theme; light is designed, not derived.

| Token | Dark | Light | Role |
|---|---|---|---|
| `background` | `#000000` | `#F6F8FC` | the canvas |
| `surface` | `#06080C` | `#FFFFFF` | cards, sheets, dialogs |
| `surfaceVariant` | `#0F141C` | `#E7EEF9` | inner containers |
| `onSurface` | `#FFFFFF` | `#161A21` | text |
| `onSurfaceVariant` | `#A9B4C4` | `#4B5563` | secondary text |
| `primary` | `#4D9FFF` | `#0F6BD8` | the accent, the desktop |
| `onPrimary` | `#001A3D` | `#FFFFFF` | text on the accent |
| `secondary` | `#9CC7FF` | `#3A7FD9` | quieter accent, links, chips |
| `tertiary` | `#7CD9C0` | `#1E9C78` | link alive |
| `error` | `#FF8A8A` | `#C62828` | a refusal |
| `outlineVariant` | `#1A2230` | `#D5DEEB` | hairlines inside groups |

The glow behind a page is `primary` at 35 % alpha in a radial gradient from
the top-left, the way MilaHub glows pink; it is the only decorative colour.
Frosted glass (`haze`) is reserved for the floating navigation pill and a
sheet's scrim.

## Components

- **Header** — collapsing large title (32 sp → 20 sp), subtitle in the
  accent, actions on the right, a black gradient under the collapsed
  toolbar so it stays legible over content.
- **Group** — a 26 dp card in `surface` holding rows separated by
  `outlineVariant` hairlines; the device card, the settings groups.
- **Row** — icon left, title and one-line detail, control right (switch,
  chevron, value). 56 dp tall.
- **Pill navigation** — floating, blurred, three destinations at most:
  Dispositivo, Acciones, Ajustes.
- **Sheet** — 32 dp radius, `background` colour, 12 dp inset from the
  edges, no drag handle, a title row and its content; used for pairing and
  confirmations.
- **State chip** — a pill in `tertiary` for *conectado*, `surfaceVariant`
  for *buscando*, `error` for *rechazado*.

## Copy

Spanish, short, in the second person only when it asks the person to do
something (the pairing hint tells the person to scan the desktop's code).
States are nouns (connected, looking for the desktop). No exclamation marks.
