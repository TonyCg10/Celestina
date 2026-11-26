# Celestina status

- **Updated:** 2026-11-25
- **Version:** the one `celestina/Cargo.toml` carries, set by each landing
- **Implementation:** CEL-1 (1.0.0) and every unit of the parity program
  CEL-2 are landed on `main`; CEL-2-M (the cutover) closed the program after
  the author switched the session over on 2026-11-24 (Noctalia is no longer
  started), and its plan was archived on 2026-11-25. One process draws a full-width bar per output with the Island
  in its centre zone and the pieces of the `bar` table; it serves the
  launcher, notifications and the tray, and performs the duties of
  `shell.json`: idle, caffeine, night light, wallpaper, OSD, media, clipboard
  history, phone, bubbles, the session lock and the polkit agent, plus the
  desktop overview and the shortcut sheet. The halted Qt/QML shell was
  removed from the tree on 2026-10-24
  ([ADR 0023](docs/decisions/0023-qt-shell-retired.md)).
- **Landed units (version each landing set):** CEL-2-E 1.1.0 (wallpaper),
  CEL-2-B 1.2.0 (energy), CEL-2-A 1.3.0 (bar and pieces), CEL-2-K 1.4.0
  (glance), CEL-2-G 1.5.0 and CEL-2-G-SYNC 1.5.1 (clipboard), CEL-2-F 1.6.0
  (connectivity), CEL-2-C 1.7.0 (OSD), CEL-2-J 1.8.0 (phone and bubbles),
  CEL-2-D 1.9.0 (media), CEL-2-H-a 1.10.0 (lock core), CEL-2-L 1.11.0 and
  CEL-2-L-CLIPPY 1.11.1 (overview and shortcuts), CEL-2-H-b 1.12.0 (lock
  backdrop and clock), CEL-2-I 1.13.0 (polkit agent), CEL-2-C-DDC 1.13.1 (the
  placed brightness piece asks the DDC worker for its detection and reads),
  CEL-2-B-GAMMA 1.13.2 (the night light paced by the compositor, and back
  after a lost connection), CEL-2-C-KEYS 1.13.3 (the Volume and Brightness
  pieces key their nodes from their own ids), CEL-2-B-CAFFEINE 1.13.4
  (caffeine inhibits idling only: its logind lock is `idle`, a suspend asked
  for by hand goes through, and switching it on withdraws an idle suspend in
  flight;
  [ADR 0025](docs/decisions/0025-caffeine-inhibits-idle-only.md)) and
  CEL-2-I-CANCEL 1.13.5 (the polkit agent's helper conversation is lazy:
  nothing connects until the password is submitted, so a cancelled prompt
  never reaches PAM and cannot count as a failed login in `pam_faillock`;
  the amendment of
  [ADR 0021](docs/decisions/0021-polkit-agent.md)). Each
  unit's evidence is linked from
  [the plan's ledger](docs/plans/archive/2026-10-16-cel-2-parity.md).
- **CEL-2-M (landed as 1.14.0):** the cutover's code:
  every shipped duty defaults to on, with `lock` and `polkit` off
  (CEL-2-M-LOCK-POLKIT, below, turns those two on); the clipboard reads an
  unknown value as off; a `shell.json`
  that cannot be read starts the shell with every duty off; the `bar` duty and
  the capsule-only surface are gone, the bar is always drawn
  ([ADR 0024](docs/decisions/0024-cutover-defaults.md)); the smoke runs the
  cutover configuration in a nest and every script that writes a `shell.json`
  names every duty; `scripts/cutover-config-probe.sh` checks a copy of the
  niri configuration. Reviewed twice by a model other than the implementer.
  With these defaults and CEL-2-C-DDC, a
  start that chooses nothing has the OSD duty on and the `brightness` piece
  placed, so it runs one `ddcutil` detection and one read per monitor;
  `scripts/dev-session.sh` therefore starts every nest with
  `CELESTINA_DDC=0` and `CELESTINA_BACKLIGHT=0` (the second keeps the nested
  shell off the host's backlight) unless the caller set that variable. The
  night-light duty is on by default and holds no gamma until
  `energy.night_light` is switched on.
  [The handover note](docs/handover.md) holds the switch-over and the way
  back.
- **CEL-2-M-LOCK-POLKIT (landed as 1.15.0):** `lock` and
  `polkit` default to on, both in one unit, after the author reported
  `VAL-CEL2-LOCK-PAM`, `-LIVE`, `-RECOVERY`, `-SUSPEND` and
  `VAL-CEL2-POLKIT-LIVE` as passed on 1.13.5 on 2026-11-24. Both fail closed:
  a value that is neither `"on"` nor `"off"` reads as off. An explicit `"off"`
  and a protected start are unchanged. The lock's behaviour is unchanged, so
  the PAM service must be installed before the first start
  ([ADR 0026](docs/decisions/0026-lock-and-polkit-on-by-default.md)). The five
  other lock rows stay pending and do not gate the default.
- **Focus:** the session runs Celestina since the author's switch-over of
  2026-11-24 ([evidence](docs/evidence/2026-11-23-cutover.md), "The author's
  switch-over"); both default flips above have landed, and what is left is
  the rows the author has not run. The shell is at 1.15.4.
- **In progress:** `CEL-D-002` to `CEL-D-005`, entries of the standing
  [CEL-TRACKS](docs/plans/active/2026-11-25-cel-tracks.md) plan: the capsule
  sheets, where a bar capsule grows into the menu of the piece pressed
  ([design](../docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md),
  [plan](../docs/superpowers/plans/2026-11-25-celestina-capsule-sheets-plan.md)).
- **Author validation:** the author ran most rows live on 1.13.0 to 1.13.5
  between 2026-11-23 and 2026-11-24 and reported them through the
  coordinating session; the cutover's evidence lists what was reported to
  pass and what was not run. No row of [VALIDATION.md](VALIDATION.md) has
  been closed in the file: its statuses are the author's to write, and no
  agent writes them. No agent has run anything in the author's session. The
  author's live use produced the five defect reports that CEL-2-C-DDC,
  CEL-2-B-GAMMA, CEL-2-C-KEYS, CEL-2-B-CAFFEINE and CEL-2-I-CANCEL fixed.
  Not run: `VAL-CEL2-CLIPBOARD-SECRET` with a password manager.
- **After CEL-2-M-LOCK-POLKIT:** the lock's residual
  (a pre-exec window of microseconds before the signal mask, accepted by the
  author on 2026-10-23, ADR 0022 A5) is a unit of its own.
- **Blockers:** none.
