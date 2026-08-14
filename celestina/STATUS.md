# Celestina status

- **Updated:** 2026-08-13
- **Implementation:** R0-R5, R7, R8's departure slice, LVR-1 through LVR-3, the
  static hardening previously drafted as `AUD-1`, `UX-1` and `WSG-1` are
  complete
- **Design direction:** `PANEL-1` is active for the author-selected borderless
  glass bar: one nearly transparent, shadowless `ContextualVeil` reaches
  edge-to-edge with no outer margin and owns one real compositor-blur region.
  Ordinary rounded `ContentSurface` capsules remain inset inside it with fixed
  light/white foregrounds and no compositor region of their own. A
  panel-opened primary carrier starts a droplet-shaped `ContextualVeil`
  membrane at the bar's lower edge. One narrow icon-proportional mouth clings
  to the seam with a horizontal-tangent meniscus, centred beneath the exact
  glyph and clamped inside the body's flat top span. It narrows to a neck just
  below the bar and swells concavely until it lands tangent on the body's top
  edge, which keeps its ordinary rounded corners outside the swell. Its
  tension uses travel, icon/body reference scales and horizontal displacement
  and only thins the neck. The
  clicked control remains the independent placement and interaction opener and
  keeps its ordinary hover circle while its own surface remains open. Neither
  its `PanelPill` nor any `ContentSurface` changes shape, geometry or material,
  and no dense bridge or cross-window fill transition is painted. The glyph
  anchor is tracked live to reposition the waist. Routes without a real panel
  opener and glyph anchor remain floating. Every shell surface uses that same
  veil-over-section anatomy, the on-screen display and the notification toasts
  included — both quiet surfaces now hang from the bar at the top right with
  the same drop membrane, from the panel icon of what they report, retreat to
  their own corners when the zone is taken, and the display stays silent while
  its own menu is open: no surface paints
  its own opaque plate, and none captures a scene it is not part of. The rest of `UX-2`
  remains planned under the still-open `SHELL-D5` discussion
- **Author validation:** the author closed the LVR-3 phase on 2026-08-07 after
  first-generation media, the four-item tray, Bluetooth state retention,
  output-triggered DDC rediscovery, outside-click dismissal and a clean
  Noctalia to Celestina to Noctalia lifecycle all passed. Wi-Fi remained visible
  throughout the exercised session; deliberate offline testing was unsafe in
  the live network layout and remains deferred rather than inferred. See
  [VALIDATION.md](VALIDATION.md)
- **Live migration:** Noctalia remains the rollback and must not be removed.
  `scripts/handover-status.sh` reports the unrecorded responsibilities, and the
  removal tool refuses while any are unbuilt, unrecorded or failed
- **GPU safety hold: ended 2026-08-07.** `VAL-GPU-01` passed: a long
  Noctalia-only observation and two controlled handovers, one without DDC and
  one with DDC, hotplug, brightness and media, produced no PCIe device loss.
  That is strong negative reproduction evidence, not proof, so every DDC
  invariant stands — one owning worker, global serialization, coalesced
  operations, a bounded timeout, deterministic cancellation, a killed and
  reaped child, and no frequent polling. Builds, tests and deployment run
  again; Noctalia still owns the session and Celestina is not activated. See
  the
  [system audit](docs/evidence/2026-08-05-gpu-loss-system-audit.md) and
  [lifecycle record](docs/evidence/2026-08-05-ddc-process-lifecycle.md).

## Current checkout truth

- **Current milestone prototype — `PANEL-1-S`.** The quiet surfaces are made
  of the shell's glass and hang from the bar. The on-screen display and the
  toast stack appear at the top right, attached by the same drop membrane as
  the menus, the mouth on the panel icon of what they report; each yields the
  zone to anything interactive already there — the display to the bottom-right
  corner, the toasts to the bottom centre — and each counts the other. A level
  changed from inside its own open menu raises no display, and an open
  notification centre keeps the corner quiet the same way. The display is a
  file of live cards, each kind on its own clock, the cards behind peeking out
  and hover raising one; it lives on two persistent surfaces so a menu opening
  over it moves the file at once instead of remapping.

  Its compositor blur is live: what broke it was never the effect but three
  lifecycle defects — premapping at boot poisoning the whole overlay layer,
  the effect withdraw gated on an `isExposed()` flag that flaps on idle
  Wayland windows, and dying delegates never republishing their glass — each
  isolated on the nested session and fixed on 2026-08-13. A quiet window
  carries a one-pixel heartbeat because a Wayland surface that commits nothing
  loses its frame callbacks, and layer surfaces now follow their window's size
  so a growing stack cannot overflow the screen. The author exercised the
  suppression, the fallbacks and the blur lifecycle live during the work; the
  remaining live checks stay in [VALIDATION.md](VALIDATION.md).

- **Previous milestone prototype — `PANEL-1-R`.** Every panel reading now opens
  something. The clock opens a calendar, the phone reading opens Magnetita's
  device list with ring/pair/unpair, brightness opens a card with one slider
  per monitor that speaks DDC, and audio opens a card with the output, the
  input and one slider per application making or taking sound — the provider
  learned to read `wpctl status`'s `Streams:` section and to move a named node,
  and it does so once per opening rather than on its two-second poll, because
  the 2026-08-12 audit measured that poll as the busiest subprocess in the
  shell.

  Four surfaces of hand-rolled card scaffolding collapsed into one `SoftCard`:
  header, dense body section, dismissal, and a height that is *measured* like
  `AnchoredMenu`'s rather than summed from constants. The constants were the
  defect — three cards, three different arithmetics, all smaller than what they
  drew, which is why rows fell off the bottom. Removing them exposed the real
  cause underneath: `AnchoredCard` bound its window size to its content, so a
  height that settled after first layout re-fired the binding and shrank the
  surface under its own placement clamp. The size is a request made once, and
  re-made only when the host changes the viewport cap or the side attachment.

  Five separate places mixed the shell's unscaled units with real output
  pixels, all invisible at factor 1: the attachment lease's live anchor
  refresh, the tray child's parent card, its viewport cap, its membrane gap,
  and the tray menu's reparented heading. Each is converted once now, and the
  child menu moved onto the same output-covering carrier every other menu
  uses — a card-sized surface was the structural reason its sideways push
  could never read as one piece, since the compositor's glass fills such a
  surface edge to edge and leaves the card no canvas to travel across.

  The hierarchy is icon-first from here (author's standing decision,
  2026-08-13): secondary actions are compact icons where the action applies,
  every opener carries the same capsule behind it inset from its reading pill,
  and the speaker-and-microphone pair reads as one capsule twice as long. The
  tray menu's "Acciones" label is gone; its header already says what the list
  is. CTest 18/18, provider tests 115/115.

- **Previous milestone prototype — `PANEL-1-Q`.** Building and deploying now
  refuse to run while a Celestina shell is executing the files they would
  rewrite. A third GPU loss was recorded on 2026-08-12 with the same shape as
  the first two: a development nest was live, `deploy-production.sh` replaced
  the installed bundle underneath it, its helper channel broke, the host
  restarted the provider adapter seven times inside 1.5 seconds, and the seven
  resulting `ddcutil detect` children contended for one I²C bus
  (`Max wait time 0 milliseconds exceeded after 2 flock() calls`) six seconds
  before `amdgpu: device lost from bus!`. `PANEL-1-M`'s smoke gate held — the
  smoke runs in a scratch state home and none of those seven journals were
  there — so the exposure it did not cover was build and deploy rewriting the
  files a real session already had open, which until now was governed only by
  the author and the assistant remembering. `session-interlock.sh` reads
  `/proc/PID/exe` rather than command lines, so an unrelated process that merely
  mentions the build tree cannot abort a release, and it counts a binary that
  has already been replaced (`" (deleted)"`) as live, which is precisely the
  dangerous state. Four cases are exercised: nothing running, a live host, a
  host whose binary was already swapped underneath it, and release resuming
  once the session is closed. The same hard reboot left eleven empty objects in
  the Git store — all unreachable, no committed history lost — repaired before
  this delivery.

- **Previous milestone prototype — `PANEL-1-P`.** The falling drop now keeps
  its blur for the whole fall instead of only after landing: the compositor
  region was debounced to the settle timer, which only ever described the
  landed shape, so `SoftMenuField` republishes it synchronously on every
  `attachmentProgress` frame while the fall is active, deduplicated in
  `PanelBlurController` so a static menu costs nothing extra. A follow-on
  performance audit measured the provider adapter writing ~290 KB/s to the
  SSD at idle — 126× its own journal file's growth — because every routine
  poll subprocess (`wpctl`, `nmcli`, `bluetoothctl`, `powerprofilesctl`,
  `ip`) logged at the `Critical` level reserved for GPU-loss forensics, which
  flushes and fsyncs per line. The level now follows what the child can
  reach: `ddcutil` and every anomaly (a failed spawn, a timeout, a broken
  wait, a kill-and-reap, a failed exit) keep `Critical`; an ordinary
  spawn/started/exit of anything else drops to `Info` and still gets
  recorded, just without the disk cost. Measured after the change: 0 B/s of
  `write_bytes` over 45 seconds where there had been 289,724 B/s, with 594
  info and 23 critical lines recorded in that window — nothing stopped being
  logged. The audit also chased and closed a false lead in its own first
  pass — the provider's 143 MiB RSS was suspected as a wallpaper-decode leak,
  disproved by the journal recording no wallpaper event at all in that run,
  and confirmed stable by direct measurement (4 KB drift across 100 seconds
  and 250 subprocess spawns) — recorded as sizing, not a defect, with the
  true allocation site left unidentified rather than guessed at. See the
  [performance audit](docs/evidence/2026-08-12-shell-performance-audit.md).

- **Previous milestone prototype — `PANEL-1-O`.** The per-output factor is
  derived from a monitor's physical diagonal rather than its density, after
  the author checked `PANEL-1-N`'s density model live and it was wrong on
  their own hardware. Their 24" and 32" panels are 1.6 dpi apart —
  indistinguishable by density — and they confirmed 1.00 on the first and 1.15
  on the second; their diagonals differ enough to separate them. The factor
  floors at 1.0 rather than shrinking a smaller monitor, because the author's
  24" resolves to 0.88 by size alone and they asked for 1.00: a smaller screen
  is not read from proportionally closer. Qt's fabricated densities (96 and
  100, produced when a compositor publishes no physical size) are refused to a
  hair's width, which is what a nested Niri without a physical size had been
  triggering — the shell measured 100.00 dpi exactly and drew itself a quarter
  larger than the session beside it, reproduced live and traced to the nest
  rather than assumed. `CELESTINA_SHELL_SCALE` is unchanged. The complete
  CTest suite passes 18/18, with cases holding the author's three monitors and
  their own judgement on each as the specification.

- **Current milestone prototype — `PANEL-1-M` and `PANEL-1-N`.** The canonical
  verification workflow no longer reaches the graphics card. `complete-production.sh`
  ends in a smoke that starts the real host with the real provider adapter, and
  that adapter probed DDC on the same I²C buses the running desktop uses; two
  GPU losses on 2026-08-12 had concurrent `ddcutil` children on one bus
  immediately before the machine went down. `CELESTINA_DDC` gates it and the
  smoke sets it to `0`: the helper still starts, registers and publishes as it
  does in a session, and opens no bus. Verified on the real binary — the
  journal records `ddc.disabled`, no `ddc.start`, `ddc.detected` or `ddc.end`,
  and the smoke runs with no `ddcutil` line at all.
  Contextual surfaces are now sized per output too, which `PANEL-1-L` had
  explicitly left undone: every menu and overlay scales its own scene by the
  same factor as the panel, and the opener, icon anchor and attachment seam
  they are handed are divided by it once in the two controllers. That also
  fixes a defect `PANEL-1-L` shipped: blur regions were published from a mapped
  origin and an unmapped size, so on a 1.15 output the panel asked to blur a
  region a third narrower than the bar it painted; both collectors now derive
  the rectangle from two mapped corners. `CELESTINA_SHELL_SCALE` lets the
  author name the factor when a density cannot answer for them, and pins it in
  the test environment — the offscreen platform reports a density of its own,
  which had been silently rewriting geometry contracts stated in output pixels.
  The complete CTest suite passes 18/18, including one case that exercises a
  real 1.15 factor end to end. The author-run visual pass remains pending.

- **Current milestone prototype — `PANEL-1-K` and `PANEL-1-L`.** The bar's own
  reading capsules now reach the screen's top edge instead of floating inside
  the veil: each keeps its rounded bottom and loses the gap above it, and the
  centred clock alone is held by an elastic skin that is widest where the edge
  grips it and draws in over the whole side. Flanked capsules keep straight
  sides so no neighbour is overlapped and no gap on the bar was widened. Three
  rejected silhouettes — squared, a shallow outward lip, and pinched narrower
  at the edge — are recorded in the geometry so they are not tried again.
  What the shell draws is now the same physical size on every output. One
  bounded, stepped factor per screen comes from that output's real density
  and is applied as a scene scale, so every layout number inside — the
  40-pixel bar, the capsules at y=5, the attachment seam — is unchanged and
  only the last step to real pixels differs. The density the tokens were drawn
  against maps to 1.0, so the author's 27" panel does not move; both LG panels
  resolve to 1.15. An output publishing no believable physical size stays at
  1.0. Raster fidelity is corrected at its source: the tray host rasterizes a
  foreign icon at 64 pixels rather than 18, and every raster consumer asks for
  its source at the density it will be drawn at. Glyph strokes and the bar's
  reading weights thicken without any size token changing. `PerformanceMenu`,
  `NetworkMenu` and `BluetoothMenu` no longer rebuild their complete row list
  on every provider tick — the defect that left `Rendimiento` permanently
  clipped — and `PerformanceMenu`'s readings are now the way into the system
  monitor. The complete CTest suite passes 18/18. The scene scale reaches the
  panel only; contextual surfaces still draw unscaled, and the author-run
  visual pass remains pending in
  [per-output sizing and raster fidelity](docs/evidence/2026-08-12-output-sizing-and-raster-fidelity.md).

- **Current milestone prototype — `PANEL-1-J`.** The settled droplet gains its
  opening motion and nothing else. `membraneOutline` takes one bounded
  progress input, so every frame of an opening attached surface is a real
  droplet outline rather than a scale or fade of the settled one, in both
  orientations. Below 1 the body opens out of its own mouth in span and extent
  together while the neck thins under flight tension; at exactly 1 the result
  is the settled geometry byte for byte, so the motion cannot move where a
  surface ends up. Above 1 is the elastic recoil: the membrane hauls the
  settled body 14 pixels back toward its seam and thins its neck before
  letting it settle, bounded in pixels and against the travel so it can never
  swing a tall surface or pull a body into its own mouth. The mouth stays
  welded to the seam and the neck keeps a hard floor at every frame, so the
  drop is always under tension and never pinches off. The carried content
  rides inside the drop: the geometry publishes the momentary body as a
  frame-space rectangle, and the content is translated and clipped to it
  rather than scaled, so rows emerge from the seam with the glass and nothing
  is reflowed. Reduced motion resolves the settled geometry immediately and
  starts no animation. Placement, the attachment lease, compositor-region
  publication, focus, Escape, outside-click and every floating route are
  untouched. The focused selection passes 4/4, the offscreen QuickTest runner
  226/226, and registered production completion passes with CTest 17/17 and
  the release smoke before deploying the verified bundle to `~/.local` without
  session activation. Two earlier curves and one monotone author-rejected
  revision are recorded as superseded in the
  [droplet fall evidence](docs/evidence/2026-08-12-droplet-tension-fall.md).
  The author-run nested-Niri pass remains pending.

- **Settled shape the motion above opens — `PANEL-1-I`.** Celestina 0.12.0 with
  CelestinaStyle 1.4.0 replaces the top-edge droplet experiment with one
  marginless `ContextualVeil` backdrop across the complete 40-pixel panel. The
  panel publishes one finite compositor-blur region. Its information groups are
  ordinary rounded `ContentSurface` capsules at output-local y=5 with height
  30; they paint no compositor region of their own. A primary menu or overlay
  opened from the panel receives the clicked control for placement, its exact
  glyph as a separate attachment anchor and
  `attachmentStartY == barHeight`. The overlay's elastic membrane is solely
  `ContextualVeil`, shaped as one drop falling out of the bar: a narrow
  icon-proportional mouth clings to the seam with a horizontal-tangent
  meniscus, centred beneath the glyph and clamped inside the body's flat top
  span, narrows to its neck just below the bar and swells concavely until it
  lands tangent on the body's top edge. The body keeps its ordinary rounded
  top corners outside the swell, so no body-wide edge and no waist between two
  wide ends can read as an hourglass. Vertical travel, icon/body reference
  scales and horizontal displacement determine its tension, which only thins
  the neck. A tokened tracker
  republishes the live glyph rectangle when tray/provider layout changes, and
  the invoking control retains its ordinary hover circle only while its own
  surface owns that lease.
  A foreign tray child menu born from a row of the mapped inventory now uses
  the same droplet sideways: its card-sized surface sits flush against the
  parent card, the membrane strip inside that window is its horizontal
  travel, and the mouth clings to the edge facing the parent at the invoking
  tile's height — toward whichever side the child was born on. Point-only
  routes still float. The foreign menu's own viewport is also contained: the
  header card and section label are pinned beside the viewport instead of
  scrolling with it, the raised top padding plus a clipped ListView keep
  scrolled rows strictly inside the dark body section, and the separate
  scroll bar is removed in favour of direct wheel/keyboard/drag scrolling.
  `PanelPill` and `ContentSurface` remain ordinary rounded surfaces throughout;
  no capsule opens, stretches or contributes a dense bridge. The painted path
  and finite compositor polygon share the same geometry; the panel remains one
  disjoint y=0..39 blur region and the menu starts at y=40. The veil paints no
  outline, lit edge or apparent halo.
  Command and keybind routes remain floating rounded surfaces. The workspace
  map is now a real panel-attached droplet: each workspace dot and the
  collapsed monitor dot publish the semantic attachment-source contract, the
  right click transports their control and dot rectangles, and the map hangs
  from the bar beneath the exact invoking dot with the live lease. The
  collapsed monitor group itself dropped its bordered, numbered capsule and
  is one dot, larger than the workspace dots beside it; its count stays in
  the accessible name. Provider, focus, Escape,
  outside-click, destructive-confirmation, reduced-motion and parent/child menu
  contracts are unchanged. The immediately preceding whole-capsule revision
  passed QML lint, the focused selection 4/4 with 208/208 QuickTest cases, the
  architecture and Style checks, and Celestina's registered completion with
  CTest 17/17 before deployment without activation. That evidence is
  superseded for attachment geometry. The later glyph-mouth revision passed
  its focused CTest selection 4/4, offscreen QuickTest 208/208, architecture
  and canonical production checks, then deployed without activation; it too is
  now explicit superseded evidence. The first body-wide revision also remains
  superseded: its icon-scaled 9..11-pixel waist read as a straight hourglass in
  the author-provided screenshot. The fluid body-proportional-waist revision
  that followed verified cleanly but the author rejected its live read as a
  strange hourglass on 2026-08-11: any waist between two body-wide edges keeps
  the hourglass identity. The current droplet revision passes the focused
  selection 4/4 and complete offscreen QuickTest runner 211/211, and its
  registered production completion passes CTest 17/17 with the eight-second
  release smoke before deploying the verified bundle to `~/.local` without
  activating a session. The author-run nested-Niri visual matrix remains
  pending.
  `PANEL-1-I`'s bytes shipped in `a97eb55` as celestina 0.12.0; its row stays
  `active` because no immutable inventory was taken at that delivery and one
  cannot be written afterwards. `STYLE-G7-J` remains `active` likewise.

- **Current design iteration — `PANEL-1-J`.** The author accepted the settled
  droplet and asked for its motion. An attached surface is now born as a drop
  at its own seam and falls into place: the same `membraneOutline` takes one
  bounded progress value, so every frame is a real droplet outline rather
  than a scale or fade of the settled one. The body's lateral span and its
  extent from the seam open together out of the mouth, and flight tension
  peaks mid-fall to thin the neck before it relaxes on landing. Two
  invariants hold at every sampled frame: the mouth is settled geometry and
  is never scaled, so the seam contact never moves, and the neck keeps a hard
  floor measured against its resting width, so the drop is always under
  tension and never pinches off. Progress 1 returns exactly the geometry
  `PANEL-1-I` verified, so the motion cannot change where a surface ends up.
  The carried content now sits in one layer above the glass instead of beside
  it and reveals as the body arrives, so no row is painted outside the drop
  carrying it. The fall is two tokened halves — an accelerating release then
  a decelerating settle — and reduced motion resolves the settled geometry
  with no animation at all. Placement, the attachment lease,
  compositor-region publication, focus, Escape, outside-click and every
  floating route are untouched. Focused selection 4/4, offscreen QuickTest
  223/223, registered production completion with CTest 17/17 and the release
  smoke, deployed to `~/.local` and reporting current without activation. The
  author-run nested-Niri pass is pending and `PANEL-1-J` is not committed.
  The preceding droplet verification is preserved as superseded evidence and
  does not verify this composition. The 768p regression fixture keeps a
  732-pixel Control Centre attached at y=72 and its blur polygon disjoint from
  the panel; the last 36 pixels are clipped instead of moving the card over the
  bar. Reachable low-height overflow remains outside this prototype.

- **Delivered prototype snapshot — `PANEL-1-B`.** Celestina 0.11.0 records the
  cumulative uncommitted B-H sequence as one milestone delivery. It includes
  the contextual menu hierarchy, panel grouping, tray and wallpaper tools,
  non-xray compositor profile, canonical shared glass and the final fixed
  light/white foreground over dense dark content cards and panel capsules. The
  retracted contrast analysis, appearance publication, host adapter and QML
  polarity inputs are absent rather than dormant. The wallpaper gallery,
  per-output selection, same-path image reload and atomic import remain. The
  canonical production exit passed and deployed the verified bundle without
  activation; a nested-only restart confirmed the fixed-white instance while
  leaving host Niri and Noctalia intact. The earlier B-H labels below preserve
  prototype chronology, not separate published versions. `PANEL-1` remains
  active; `PANEL-1-I` is the current author-selected prototype above.

- **Delivered in celestina 0.10.0 — `PANEL-1-A`.** The first borderless-glass
  panel baseline removes the hard full-width plate, keeps its soft wallpaper
  shadow, and places each content group on a finite compositor-blur capsule.
  Workspaces retain per-monitor grouping as positional state marks; status and
  action readings use the canonical 1.2.0 glyph catalogue; CPU and memory keep
  their percentages; the phone omits its device name; and the existing control
  centre, clipboard, notifications and session surfaces have permanent panel
  entry points. Tray registration is reconciled after startup and its wrapper
  follows the item model, so four published items remain visible after a host
  restart. The canonical production exit built, verified and deployed 0.10.0
  without activating the session. `PANEL-1-B` now applies the bounded Velo
  candidate to every existing interactive shell menu: network, Bluetooth and
  tray preserve their real Qt `Menu` lifecycle, while workspace map, control
  centre, clipboard, notification centre, session and launcher preserve their
  custom overlay focus and dismissal models. Every surface publishes one light
  compositor-glass body and uses denser tint-only sections rather than a stack
  of resting row pills. A bounded worker now samples each output's exact ready
  wallpaper and selects a surface-local light or dark foreground by WCAG
  contrast without changing the global theme. The host admits that result only
  when output, source, file revision, inventory generation and crop geometry
  match the exact image request that reached `Image.Ready`; all exposed text
  foregrounds stay on the two measured candidates. Loading, stale, failed and
  no-blur paths keep the established light-ink/dark-fallback pair. Panel-opened
  cards follow the real opener; launcher stays centred. Session confirmation
  text can no longer resize its full-output input surface and jump the power
  menu over the bar. Automated construction, geometry, blur-region,
  interaction and contrast checks pass. An agent-run nested scale-1 comparison
  switched the panel and an open control centre from light ink on the dark
  requested wallpaper to dark ink on the previous bright wallpaper, then back
  again without restart; the requested wallpaper was restored. The same unit
  presents capture as `Caja de herramientas`, adds a folder-backed wallpaper
  gallery beside it, removes the tray's visible count, and persists bounded
  pin/hide choices. The corrective C prototype replaces the rejected expanding
  hidden list with one fixed four-column icon grid and adjacent visible/hidden
  selectors. It reserves three rows in either mode, scrolls overflow,
  never paints producer names, and restores focus only after the exact durable
  key and requested mode are confirmed; unrelated tray activity cannot consume
  that pending transition. Pinned items render beside the compact opener. The
  opener and inventory heading use the semantic system-tray glyph. Exact icon
  names resolve through the Qt theme and its installed GTK fallback chain, so
  Solaar's published `battery-good` resolves without an application heuristic;
  applications such as Slack that publish only a pixmap retain that artwork.
  Every unresolved foreign icon stays a fixed-size glyph rather than producer
  text. A foreign tray menu uses a child surface so the inventory stays mapped.
  The D prototype replaces the two painted selector labels with accessible
  eye and eye-off glyphs, grows only the application artwork inside each fixed
  grid tile from 19 to 23 pixels, and suppresses every shell hover tooltip at
  its local button boundary without removing hover feedback or assistive names.
  Foreign D-Bus menus are now capped to the logical output space remaining
  below their real request before the carrier adopts its size, so an overflowing
  real Qt Menu stays anchored, exposes a draggable scroll route and keeps every
  action reachable by arrow key. Escape still closes only that child while the
  tray inventory remains mapped.
  Wallpaper selection persists a user-chosen folder, rejects a scan beyond the
  512-entry safety bound instead of publishing a partial catalogue, and exposes
  every accepted image through deterministic pages of at most 64 thumbnails.
  The menu reports total and page navigation rather than a terminal limited-
  gallery label, and a catalogue/id click changes only the invoking output; no
  source path is embedded in the panel. The canonical production exit built,
  verified and deployed its candidate without host-session activation. The
  E prototype identifies Niri 26.04's automatic xray policy as the reason those
  finite blur regions showed the wallpaper even above an application. The
  registered nested profile now sets `xray false` only for the panel, primary
  menu, tray-child menu and interactive overlay namespaces; live-session
  documentation exposes the identical block as a manual opt-in. The first
  live-reload comparison changed an over-application
  sample from wallpaper-like `srgb(42,47,43)` to `srgb(24,106,116)`, but a
  later reconstruction returned to the wallpaper and that sequence was not
  accepted as durable evidence. The final controlled cycle started nested Niri
  PID 1102853 from one stable exact rule, opened a uniform teal application and
  crossed its edge with the launcher. In
  `/tmp/celestina-non-xray-clean-start.png`, pixel `(186,291)` in the glass
  above the application is `srgb(31,106,115)`, close to the uncovered
  `(16,56)` reference `srgb(0,91,102)`, while `(686,291)` in the same glass
  above the wallpaper is `srgb(33,39,33)`. Closing and reopening the
  launcher preserved those values in
  `/tmp/celestina-non-xray-clean-reopen.png`. A first Celestina-only restart
  produced PID 1106789 and adapters 1107007/1107009 and preserved them in
  `/tmp/celestina-non-xray-after-celestina-restart.png`. A second registered
  Celestina-only restart, without a Niri reload, produced PID 1110628 and
  adapters 1110890/1110891. In
  `/tmp/celestina-blur-control-launcher-clean-after-restart.png`, those same
  three coordinates retained the same three values. After the canonical
  production exit, the final nested-only restart loaded the verified bytes as
  current PID 1127567 with adapters 1127828/1127829. The same values remain at
  the same coordinates in
  `/tmp/celestina-blur-control-launcher-production-final.png`. This live colour
  split was sampled on the launcher namespace `celestina-overlay`; the other
  exact namespaces share the validated matcher but were not separately sampled.
  Nested Niri stayed PID 1102853 and host Niri PID 1224 plus Noctalia PID 1276
  remained intact. The author's live Niri configuration was not edited. The
  earlier `/tmp/celestina-contextual-tools-live.png` records the recapped
  panel. The author-owned all-menu, scale-2, multi-output, native folder chooser
  and real tray gesture review is still pending. That includes visual
  acceptance of the eye controls and larger artwork, hover confirmation across
  the shell, and a
  real wheel/drag pass through an overflowing foreign menu. Non-xray blur is
  more expensive and experimental in Niri 26.04; motion/drag behavior and text
  contrast over arbitrary application content remain author-run checks because
  Wayland does not expose those pixels to Celestina's wallpaper-derived ink
  analysis. The F prototype removes the last shell-local menu
  material recipe: both the very light outer veil and every denser content
  section now use CelestinaStyle's `GlassSurface.ExternalBackdrop`.
  `CompositorGlassRegion` remains only the shell-owned KWindowEffects geometry
  and fallback adapter, so each menu still publishes exactly one blur region
  and no QML capture attempts to read another Wayland client. The final
  canonical exit passed and deployed without host-session activation. A
  nested-only restart replaced the old shell with PID 1224284 and adapters
  1224469/1224470 on `wayland-2`; nested Niri PID 1144687, host Niri PID 1224
  and Noctalia PID 1276 remained intact. Opening Control Centre through the
  session command confirmed one 30-fragment compositor shape and emitted no QML
  construction or binding error. Visual acceptance of the material remains in
  the author-owned matrix. The G prototype narrows the
  reference-backed treatment to the two requested information surfaces:
  contextual content cards and panel capsules now share CelestinaStyle's
  dense matte `ContentSurface`, while the contextual carrier uses the much
  lighter `ContextualVeil`. Both roles have zero elevation, use no QML capture
  and leave the menu at one compositor region. The content material polarity
  follows the same measured foreground decision, so the established bright and
  dark wallpaper pairing is preserved. Other suite glass keeps its compatible
  default material. The canonical production exit passed all Rust unit and
  integration suites, QML lint and QuickTests, CTest 17/17 and the release
  smoke, then deployed without host-session activation. A nested-only restart
  replaced PID 1224284 with PID 1336218 and adapters 1336400/1336401 on the
  unchanged `wayland-2` nest; nested Niri 1144687, host Niri 1224 and Noctalia
  1276 remained intact. The live bright-backdrop capture confirms dark ink over
  matching dense light cards/capsules and a nearly transparent outer field.
  Toasts, OSD, output
  sharing and new clock/weather behavior remain outside the Velo redesign. The
  rejected earlier fields remain recorded, and this is not yet an accepted
  suite-wide UX-2 language. `VAL-PANEL-1` is partial rather than passed at both
  scales.

- **Delivered in celestina 0.8.0 — `DIAG-1`.** Every Celestina process now writes a
  structured, bounded, always-on JSONL journal under
  `$XDG_STATE_HOME/celestina/diagnostics/`, correlated by one `run_id` the host
  generates and exports before it spawns either helper. It records classes of
  event, technical identities and timings — never clipboard content,
  notification bodies, media metadata, window titles, launched commands or
  secrets. `scripts/diagnostic-report.sh` collects it read-only after a reset.
  The format, privacy rules and the limits of what it can prove are in
  [docs/diagnostics.md](docs/diagnostics.md). The canonical production exit
  built, verified and deployed 0.8.0 without activating the live session.
  `VAL-DIAG-1` is the next author-owned check.


- Delivered in celestina 0.8.0: `WSG-1`. A workspace now carries the monitor it
  belongs to, not only the one it is on. Niri publishes the second and never the
  first, so the home is remembered from a frame that could see more than one
  output — or declared by the author in the shell's settings — and a frame that
  cannot tell a displaced workspace from a native one teaches nothing. A strip
  carrying more than one monitor's workspaces opens the group holding the focus
  and shows every other as one capsule naming its monitor, its count and its
  urgency; a capsule click is an ordinary focus request and the group opens
  because the focus arrived. A strip of one group renders exactly as it did
  before. A missing, corrupt, oversized or future-schema memory file degrades to
  that same flat strip rather than failing. The canonical production exit built,
  verified and deployed 0.8.0 without activating the session. `VAL-WSG-1` — the
  live capsule, its assistive route and the moment the memory is first taught —
  is not run.


- Delivered in celestina 0.7.0: `UX-1`. Network and Bluetooth retain truthful
  panel summaries while each now opens its own dismissible menu. Saved network
  profiles and known Bluetooth devices use provider-owned stable identities;
  actions remain pending after tool acceptance and settle only from a later
  observation. The durable host ledger survives menu destruction, keeps failed
  targets visible even if their row disappears and distinguishes these
  confirmed actions from the control centre's immediate requests. The canonical
  production exit built, verified and deployed 0.7.0. `VAL-UX-1` passed in the
  live Niri session on 2026-08-08. Follow-up work remains for opener-relative
  a deliberate shell-wide visual-usability pass. The menu surface now follows
  the compositor's real exclusive-zone placement and keeps the invoking
  control's horizontal anchor. Directly changing from one open menu to another
  still required two clicks in the last live observation and is not claimed as
  fixed. A clock/date calendar-and-weather menu with location management is a
  separate product extension beyond UX-1.

- Delivered together in celestina 0.6.8: `LVR-3-G`, in the same atomic batch as
  `LVR-3-F`.
  A network probe that saw nothing can no longer retire a link at any repetition
  count — `UNREADABLE_HOLD` is removed rather than raised — and only a poll that
  positively found no default route can, twice over. A route naming a device the
  device list cannot explain is unreadable, not offline, which is what a Wi-Fi
  card re-associating looks like.

  The tray had a real defect, found by walking the whole D-Bus path against a
  private bus rather than by reasoning about the parts. A registry read rebuilt
  the registration list wholesale from the snapshot its reply carried, so an
  application that registered while that read was in flight was removed by an
  answer composed before it existed — and no second registration signal was
  ever coming for it. The new `celestina-tray-watcher` integration test
  reproduced the live symptom on its first run, publishing two of four with
  Slack and Solaar missing. A registry read is now a reconciliation against the
  registrations known when it was sent, and all four are published.

  The model, the open drawer and the 1920-pixel flank layout were also checked
  and hold. The folded drawer additionally now shows how many items are behind
  its chevron, which it never did. Both live cases stay failed until the author
  reruns them.

- Delivered in celestina 0.6.8: `LVR-3-F`, the
  first unit of this plan with executable evidence. Four readings stopped
  treating one unlucky observation as the truth: Bluetooth publishes the
  adapter's own state so a powered radio with nothing on it stays visible; the
  network holds its last confirmed link across a bounded run of unreadable
  polls without raising the shared 750 ms tool deadline; each overlay receives
  only the properties it declares, so the session menu no longer logs a runtime
  property error; and output hotplug asks the single DDC worker for one
  coalesced rediscovery instead of waiting out the 300-second refresh. Beyond
  those four: every transient surface — the five focused overlays, the panel's
  context menu and a tray item's menu — now covers its own output, so a click
  outside a card is the surface's to answer and the panel button that opened an
  overlay is behind it rather than in front of it; a tray item that registers
  and then fails to describe itself is retried, logged and shown under the name
  it registered with instead of being dropped silently; and media is driven by
  MPRIS owner and property signals over `zbus`, with `playerctl` gone from this
  shell entirely and only a one-second progress tick and a thirty-second
  bounded reconciliation left. The
  canonical production exit ran and deployed the verified bytes to
  `~/.local`; the session was not replaced. Recorded in
  [one poll is not the truth](docs/evidence/2026-08-07-one-poll-is-not-the-truth.md).

- Delivered in celestina 0.6.7: `LVR-3-E`. The helper target gathers its sources
  at configure time rather than naming ten of nineteen by hand. Later canonical
  exits compiled and exercised that gathered target after the hold ended.

- Delivered in celestina 0.6.6: `LVR-3-D`. A snapshot the host would discard is
  now skipped rather than treated as the end of the Niri session, so an
  oversized frame no longer costs a reconnect loop against the compositor. It
  is included in the verified 0.6.8 bundle.

- Delivered in celestina 0.6.5: `LVR-3-C`, repairing two defects `LVR-3-B`
  introduced into the helper-restart path. The escalation timer now names the
  instance it was armed against, so it cannot kill the replacement that started
  inside the grace window; and the restart delay is decided by the handler that
  knows how the helper exited, so the spacing an unclean exit earns is applied
  rather than lost to a race. Later canonical exits and controlled shutdowns
  exercised the complete lifecycle after the hold ended.

- A C++20/Qt 6.9+ host maps one top layer-shell panel per output and owns the
  `org.celestina.Shell1` session interface.
- Rust helpers reduce Niri state and carry the aggregate providers through the
  pure `celestina-shell-core` contracts.
- Media is read from the session bus rather than asked for: the helper follows
  `org.mpris.MediaPlayer2.*` owners appearing and disappearing and whatever a
  player says at its own object path. Nothing is spawned for it, and the only
  clock left advances a playing track's progress between two things the player
  said.
- The panel contains workspace/window, system, media, audio, DDC and tray
  paths. Workspace, audio, microphone, DDC, CPU/RAM and tray paths passed the
  follow-up. Version 0.6.2 gives the first helper generation a bounded fast
  MPRIS discovery window, and 0.6.8 replaces polling with MPRIS signals. The
  final full-shell rerun passed without replacing the helper.
- The launcher and clipboard-history overlays are implemented and use the same
  surface and command contracts.
- Typed volume, mute and brightness session verbs enter through
  `org.celestina.Shell1`, reach their provider and are answered `pending` and
  then `confirmed` or `failed` from a later reading, never from acceptance.
- A corner on-screen display shows volume, microphone and per-monitor
  brightness. It is raised by what a provider published, never by a request, so
  a key that changed nothing raises nothing; it never takes focus or the
  keyboard.
- Night light and the idle inhibitor are held states. Version 0.6.2 handles the
  host's ordinary Unix termination signal, releases both held children before
  exit and gives stdin-driven shutdown enough time to drain. A process
  regression proves an active fake inhibitor is gone before the helper exits;
  the repeated live lifecycle rerun remains author validation.
- `displays-off` is composed through Niri, whose own answer is the outcome.
- `lock` and `lock-and-suspend` are refused: no locker provider exists while
  SHELL-D1 is open, and a shell that cannot lock says so instead of reporting
  success. The provider seam is the refusal site.
- The optional Niri bindings, the tool each verb needs and the rollback are in
  [README.md](README.md). Nothing applies them: the shell never edits a Niri
  configuration, and deleting the block is the whole rollback.
- No task document authorizes changing Niri configuration, installing a locker,
  activating the shell or stopping Noctalia.

- A control centre — `celestina msg control-centre-toggle` — changes volume,
  mute, night light, caffeine, do-not-disturb and the power profile through the
  verbs that already existed, and shows each request as pending, confirmed or
  failed beside the provider's own reading. Network and Bluetooth are read-only
  there: this shell is not a manager for either.
- The session menu — `celestina msg session-menu-toggle` — asks twice before
  ending anything: log out through the compositor, restart and power off
  through logind, and suspend refused while no locker exists.
- The control centre also carries a month calendar, computed rather than
  fetched, and a weather reading that is absent rather than stale. No location
  means no weather and no request: this shell does not look up where somebody
  is.
- Choices survive a restart: they are written durably before anything publishes
  them, and night light, caffeine and do-not-disturb are restored at startup.
- Toasts appear in the top-right corner and never take focus; the notification
  centre — the panel's unread indicator, or
  `celestina msg notifications-toggle` — is the keyboard path to every action a
  toast offers. The on-screen display moved low and centred so a volume key
  cannot paint over a notification.
- The aggregate helper can be the session's `org.freedesktop.Notifications`
  server, but claims the name only when it is free. The follow-up proved
  takeover, replacement, close, action, DND, history, unread state and rollback
  without unrelated providers disappearing. Version 0.6.2 moves Escape to a
  window-level shortcut, and the offscreen focus regression passes; the live
  focused-button rerun remains author validation.

- The shell draws the session's wallpaper itself: one background surface per
  output, sized by the compositor, reserving nothing. An output with no image
  of its own paints a deliberate fallback rather than another screen's picture
  or a black rectangle, and a file that fails to decode falls back the same
  way. Images live in `$XDG_DATA_HOME/celestina/wallpapers`, named for the
  output (`DP-1.png`) or `default.*`.
- Physical removal and reconnection of `DP-2` changed only that output's panel,
  wallpaper and workspaces. The appearance backend and its public portal route
  returned the sealed dark-scheme/accent values through a tested rollback. The
  live Niri preference file needed an explicit
  `Settings=celestina-shell` selection. The README now records descriptor
  installation, selection, broker restart and exact rollback while preserving
  Siderita's FileChooser backend.

## Durable boundaries

`celestina-shell-core` owns pure protocol and policy. Rust helpers own bounded
non-Qt IO. C++ owns Qt, D-Bus and layer-surface adaptation that CXX-Qt cannot
express cleanly. QML owns presentation only. See [AGENTS.md](AGENTS.md) and the
suite [architecture standard](../docs/standards/architecture.md).

## Evidence boundary

The preceding whole-capsule, glyph-mouth and both body-wide-edge revisions
retain their architecture, focused, canonical and deployment results only as
explicit superseded history. They do not verify the current droplet membrane:
its narrow glyph-centred mouth on the bar seam, meniscus and tangent body
landing, restored rounded body-top corners or persistent opener circle. The
current focused selection passes 4/4 and its offscreen QuickTest runner passes
211/211. Registered production completion passes, including CTest 17/17 and
the eight-second release smoke. The verified `~/.local` deployment reports
current; no session was activated. Only the nested-Niri perceptual boundary remains pending and is
recorded in the
[edge-attached shell evidence](docs/evidence/2026-08-11-edge-attached-shell-prototype.md).
Automated production and offscreen evidence does not replace `VAL-PANEL-1`.

The canonical release bundle was built and verified on 2026-08-03: Rust tests,
direct QML lint, CTest 11/11, an eight-second offscreen smoke of the release host
with the compiled style module, and dynamic-library checks passed. Exact
artifacts, commands and limits are recorded in the suite
[evidence](../docs/evidence/2026-08-03-repository-governance.md). This does not
replace any real Niri check in [VALIDATION.md](VALIDATION.md).

R3 ran its registered exit end to end on 2026-08-04: `complete-production.sh`
built the 0.2.0 bundle once, verified those exact bytes — Rust checks, QML lint,
CTest 13/13 and an eight-second offscreen smoke — and deployed them to the
author's normal test prefix under `~/.local`. The live session was not replaced
and no service, package manager or configuration was touched. The record is
[the R3 completion evidence](docs/evidence/2026-08-04-r3-completion.md).

The deployed 0.6.0 bundle was verified again and activated by the author on
2026-08-04. CTest 13/13 and the release smoke passed, and normal panel,
workspace, audio, DDC and session-hold paths worked. The run stopped when the
first live notification invalidated the complete provider frame; it also found
missing browser media, an undismissable clipboard empty state, English product
copy and startup accessibility/application-id diagnostics. The exact stop
point, causes and unrun checks are in the
[live validation evidence](docs/evidence/2026-08-04-live-validation-failures.md).

The author exercised the corrected 0.6.1 checkout across the full handover on
2026-08-05, including watcher/notification rollback, helper and held-child
failure, settings persistence, portal integration and physical output hotplug.
The exact pass/fail matrix and external session changes are in the
[follow-up evidence](docs/evidence/2026-08-05-live-validation-follow-up.md).
Afterward `complete-production.sh` rebuilt, verified and deployed the same
source without activating Celestina; the live session remained on Noctalia.

Celestina 0.6.2 closes the bounded LVR-2 implementation checkpoint. The
canonical production exit passed all Rust tests, QML lint, CTest 13/13 and the
eight-second release smoke, then deployed the verified bundle without
activating it. The three corrected live cases remain recorded as failed until
the author reruns them against 0.6.2.

## Records

- Completed plan:
  [UX-1 network and Bluetooth indicator menus](docs/plans/archive/2026-08-07-network-bluetooth-indicator-menus.md)
- Celestina 0.6.8 is built, verified and deployed by the canonical production
  exit: 181 shell-core tests, 46 helper unit tests and six tests across three
  integration binaries, Clippy and `cargo fmt` clean, QML lint, CTest 15/15 and the
  eight-second offscreen release smoke. The author then completed the controlled
  live rerun and restored Noctalia, which still owns the session.
- Current checkpoint: `PANEL-1`, now prototyping one edge-to-edge
  `ContextualVeil` panel with a single finite blur region, ordinary inset
  rounded capsules, and a droplet contextual membrane that starts at
  `barHeight` as one narrow glyph-centred mouth, narrows to its neck just
  below the bar and swells tangent onto the menu body's flat top edge inside
  its ordinary rounded corners. The clicked control remains the body placement authority,
  keeps its hover circle while its own menu is open, and never changes its
  capsule or any dense content card. The membrane is only `ContextualVeil`, with no dense
  bridge, and live glyph tracking keeps its waist aligned through tray/provider
  layout changes. The
  carrier exposes no outline, lit edge or apparent halo. Command/keybind,
  workspace and foreign child surfaces remain floating. `UX-2` beyond this
  bounded prototype, conditional lock and Polkit work remain outside it (no
  dock is planned: ADR 0003)
- Last completed plan: [LVR-3 late provider insertion](docs/plans/archive/2026-08-05-late-provider-insertion.md)
- The milestone before it: [R8 Noctalia departure](docs/plans/archive/2026-08-04-r8-noctalia-departure.md)
- Open product questions: [discussion queue](docs/discussions/README.md)
- Accepted product decisions: [decision index](docs/decisions/README.md)
- Completed detailed roadmap: [history through 2026-08-03](docs/history/roadmap-through-2026-08-03.md)
- Original phase work orders: [Noctalia replacement history](docs/history/noctalia-replacement-through-2026-08-03.md)
