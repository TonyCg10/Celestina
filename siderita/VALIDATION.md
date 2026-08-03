# Siderita author validation

This manual lane does not contain implementation and does not block
[ROADMAP.md](ROADMAP.md). Each failed row keeps its result and opens a new
corrective implementation unit.

## VAL-SID-01 — Drag comfort and live menu glass

- **Status:** pending
- **Related implementation:** completed CP4
- **Requires:** verified Siderita artifact on the real Niri/Wayland session and
  a disposable source/destination tree
- **Procedure:** spring-open a folder during drag, edge-scroll while dragging,
  reorder sidebar rows, then open/scroll several live-capture menus while
  recording frame timing
- **Pass condition:** every drag reaches only its intended target, reorder
  persists, no covered row receives input and menu blur frame-time p95 is at or
  below the recorded 16.7 ms budget
- **Result:** not run against the current surfaces
- **Evidence:** record fixture, output scale, gestures and timing samples

## VAL-SID-02 — File chooser in daily portal use

- **Status:** pending
- **Related implementation:** completed CP5
- **Requires:** explicitly opted-in portal routing plus one GTK application,
  one Qt application and one browser named in the evidence
- **Procedure:** in each of those three clients exercise open, multi-open, save,
  save-multiple, directory, filters, cancellation and application exit; repeat
  one open request with the opt-in route disabled as the control
- **Pass condition:** requests map once, return the selected URIs or cancellation
  correctly, never strand a backend, and the disabled control remains with its
  pre-existing chooser without changing the other two clients' configuration
- **Result:** not run as the author's sustained default chooser
- **Evidence:** dated application/request list and any failure logs

## VAL-SID-03 — Reduced motion, focus and assistive technology

- **Status:** deferred
- **Related implementation:** completed accessibility foundation plus future
  local follow-up after STYLE-M1
- **Requires:** verified Siderita/CelestinaStyle artifacts, real keyboard and
  AT-SPI stack
- **Procedure:** traverse main view, menus, operation dialogs, picker and both
  embedded surfaces with reduced motion off/on; inspect focus containment,
  restoration, roles, selected/progress/error state and actions
- **Pass condition:** every action remains operable and announced, focus never
  escapes a modal and spatial/scale motion disappears in reduced mode
- **Result:** deferred until STYLE-M1 and the required AT-SPI stack are available
- **Evidence:** dated surface matrix and AT observations

## VAL-SID-04 — Portal transient parenting

- **Status:** deferred
- **Related implementation:** SID-M1
- **Requires:** SID-M1 verified artifact, an opted-in portal requester and real
  Wayland compositor support
- **Procedure:** open pickers from two distinct applications, inspect their
  parent/stacking/minimise lifecycle, then cancel one requester while both exist
- **Pass condition:** each picker belongs to the correct requester, never steals
  another request, closes with its parent and still degrades safely when given
  no usable parent handle
- **Result:** deferred until SID-M1 produces its verified artifact
- **Evidence:** window tree plus dated requester outcomes

## Closed historical observations

`VAL-SID-BASE`, `VAL-SID-GRAFITA` and `VAL-SID-FLUORITA` are preserved in the
[migration evidence](../docs/evidence/2026-08-03-migrated-author-observations.md).
