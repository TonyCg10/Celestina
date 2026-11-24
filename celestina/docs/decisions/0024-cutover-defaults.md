# ADR 0024: The cutover's defaults: every shipped duty on, the lock off, the bar always drawn

- **Date:** 2026-10-24
- **Status:** accepted
- **Accepted:** by the author on 2026-11-24, with the landing of CEL-2-M

## Context

ADR 0011 made every duty ship `off` while Noctalia held it, and named the
cutover unit, CEL-2-M, as the one that retires those `off` values: "after the
cutover a duty is simply on". The cutover design
(`docs/superpowers/specs/2026-10-23-celestina-cel-2-m-cutover-design.md`, M-3)
planned the flip for `bar`, `idle`, `caffeine`, `nightlight`, `wallpaper`,
`osd`, `media`, `clipboard`, `phone` and `bubbles`, with `lock` and `polkit`
flipping only once their live rows pass, and planned `bar` as a duty whose
default turns on while its `off` keeps the 1.0.0 capsule-only surface. Two
things changed while implementing it. First, a duty whose `off` would mean
"the wide surface with no pieces" is a layout, not a duty, and the program's
design (§6.1) already said the capsule-only surface was "a stage of the
migration, not a second layout to maintain: CEL-2-M removes it". Second, the
review of the implementation (review A, 2026-10-24) showed that with on
defaults an unreadable `shell.json` would start every duty, the clipboard
history among them, against an `off` the author may have written.

## Decision

1. **Defaults.** With no `duties` key, or with a key missing, `bubbles`,
   `caffeine`, `clipboard`, `idle`, `media`, `nightlight`, `osd`, `phone` and
   `wallpaper` are **on**. `lock` stays **off** until the author's
   `VAL-CEL2-LOCK-*` rows pass, and `polkit` stays **off** until
   `VAL-CEL2-POLKIT-LIVE` passes; each flips in a unit of its own that cites
   the passed row.
2. **Lenient reading.** A value that is neither `"on"` nor `"off"` keeps the
   field's default (on) for every duty but the clipboard, which **fails
   closed**: an unknown value reads as off, because a history that records
   what the author copies must never start on a typo.
3. **A protected start is quiet.** When `shell.json` cannot be read (half
   written, too large, a newer schema, not JSON) the shell starts with
   **every duty off** (`Duties::all_off()`), the clipboard duty refuses to
   start while the file is protected, and the first clean read applies the
   real values and lifts the protection. A missing file is not protected: it
   is a fresh install and reads as the defaults.
4. **No `bar` duty.** The bar is always one wide `TopWide` surface per output
   with the `bar` layout table in force; the capsule-only 1.0.0 fallback, its
   reshape path and the `duties.bar` key are removed. An older file's
   `"bar"` inside `duties` is read past as an unknown key and never written
   back. Hiding the bar's pieces is a layout choice (empty zones), not a duty.
5. **Every script that writes a `shell.json`** names every duty explicitly
   (`celestina/tests/duty_files.rs` enforces it against the serialized
   `Duties::default()` keys), so a nest never starts a worker a script did
   not ask for.

## Consequences

- A fresh install or a 1.0.0 file runs the whole shell; the author's
  explicit `"off"` values written during the coexistence steps are honoured.
- ADR 0011's "a missing key means `off`" is superseded by this record for
  the shipped duties; its switching rule and owner detection stay.
- The live rows that told the author to set `duties.bar` to `"on"`
  (`VALIDATION.md`) now say the bar is always drawn.
- The rollback for the bar is an empty layout, not a switch; the rollback
  for every other duty is its `"off"`.
- `lock` and `polkit` keep a documented `off` default in the code until their
  rows pass: two small follow-up units.

## Revisit when

A live row shows a duty that must not default to on, or the author wants a
way to hide the bar that a layout cannot express.
