# Evidence: migrated author observations through 2026-08-03

- **Date:** 2026-08-03
- **Scope:** closed real-session observations migrated from the seven project roadmaps
- **Environment:** historical Niri/Wayland desktop sessions and, for Magnetita, the author's Galaxy S25 Ultra over the real LAN
- **Artifact:** historical project artifacts; exact binary digests were not retained before the governance contract

## Procedure

This record transcribes only author observations already dated in the former
project roadmaps. No check was rerun during migration. Each item retains the
interaction exercised and its explicit coverage limit; absent commands,
captures or binary digests are reported as unavailable rather than inferred.

## Result

### VAL-SHELL-R0-BASE — passed 2026-07-31

The author exercised panel geometry and focus, menu keyboard dismissal,
workspace confirmation and helper recovery in the Niri session. It did not
cover compositor restart, forced failure or resource measurement.

### VAL-SHELL-R2-BASE — passed 2026-08-02

The author exercised launcher search/launch/Escape and clipboard
select/persist/clear. It did not cover a screen reader or a sensitive clipboard
client.

### VAL-STYLE-BASE — passed 2026-07-27 through 2026-07-28

The S1-S6 gallery and affected application composition were reviewed in a real
session. This predates the later tint, content-icon and full reduced-motion
audit.

### VAL-STYLE-BLUR — passed 2026-07-29

A finite shell panel region rendered blurred while adjacent wallpaper remained
sharp. This proves the mechanism, not the latest tint values.

### VAL-SID-BASE — passed 2026-07-25 through 2026-07-31

Core file operations, portal activation, removable media and major visual
composition were exercised on real Wayland. The pending matrices in
`siderita/VALIDATION.md` were outside that pass.

### VAL-SID-GRAFITA — passed 2026-07-31

Embedded edit/save/guarded-close/focus and standalone activation were driven
with real keyboard and mouse. IME and full AT-SPI were outside the pass.

### VAL-SID-FLUORITA — passed 2026-08-03

Embedded image/video/audio playback, seek/volume and standalone activation were
exercised in the real session. Full AT-SPI and reduced-motion coverage remained
separate.

### VAL-MAG-1.0 — passed 2026-07-26

Pair/reconnect, mount, battery, notifications, file sharing, ring, clipboard,
MPRIS and settings were exercised with the Galaxy S25 Ultra. This predates the
2026-07-29 hardening and is not evidence for those corrected paths.

### VAL-GRA-EMBEDDED — passed 2026-07-31

Space activation, editing, save, undo/redo, guarded close, keyboard focus
containment and restoration worked in Siderita with real keyboard and mouse.
IME and AT-SPI were not covered.

### VAL-GRA-STANDALONE — passed 2026-07-31

Standalone typing, shortcuts, close lifecycle, open action and the icon at
panel sizes worked in the real session. No dedicated glass pass was retained.

### VAL-GRA-COMFORT — passed 2026-07-31 through 2026-08-01

Find, go-to-line, syntax highlighting, tabs and tab reordering were used in the
real application. Arbitrary huge-file performance was not established.

### VAL-FLU-PLAYBACK — passed 2026-07-31 through 2026-08-03

Image, audio and correctly oriented video worked in standalone Fluorita and
Siderita's embedded surface. This is not an exhaustive codec/display matrix.

### VAL-FLU-INPUT — passed 2026-08-03

Real keyboard navigation, activation, rapid seek and volume interaction worked
in both hosts without observed hangs. Full AT-SPI/reduced-motion was not
requested.

### VAL-FLU-PRESENT — passed 2026-07-31 through 2026-08-03

The recorded 4K60 sample reported zero dropped/delayed frames and the author
observed no tearing. The backend could not report display FPS or vsync jitter.

### VAL-FLU-LIFECYCLE — passed 2026-08-03

Window close during playback left no process and repeated open/seek/close
automation found no growing resource leak. It did not simulate an untouched
multi-hour playback session.

### VAL-FLU-MPRIS — passed 2026-07-31

Identity, metadata, play/pause, seek and volume were exercised over the real
session bus. Shell presentation was a separate conditional feature.

## Limits

- This is a migration record, not newly executed validation.
- Exact revisions, artifact hashes, raw logs and most captures were not retained
  under the old roadmap system; the observations must not be widened beyond
  their text.
- New or corrected behavior requires a fresh `VAL-*` case and dated evidence.

## Follow-up

Open manual checks remain only in the corresponding project `VALIDATION.md`.
