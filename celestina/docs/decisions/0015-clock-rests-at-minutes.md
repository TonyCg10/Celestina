# ADR 0015: The clock rests at minutes; seconds run only while the Island is open

- **Date:** 2026-10-16
- **Status:** accepted

## Context

The capsule's clock is shown at rest, when the frame timer is the only thing
keeping the shell awake, and the rest budget requires zero frames in rest
(VAL-CEL-REST). A seconds display at rest would add a frame
every second for a value nobody reads from the capsule. See the design's §6.5.

## Decision

The Island's capsule keeps showing minutes and keeps its one-frame-a-minute
timer. When an Island page is open, the clock in the page title area shows
`HH:MM:SS` with a one-second timer that exists only while that page is open
and stops when it closes, so rest is unchanged.

## Consequences

- Rest stays at zero frames between minute boundaries, measured by the
  nested smoke.
- The seconds timer is a per-page lifecycle, so it must be torn down with the
  page; a leak would show as frames in rest.
- A surface has one redraw timer, and the seconds' earlier deadline replaces
  the minute's. So every frame asks for the minute deadline again, and a new
  surface (a flip of the `bar` duty) starts both timers afresh. Otherwise the
  capsule's clock would stop after a page closed.

## Revisit when

The author asks for seconds in the capsule, or the rest budget of VAL-CEL-REST
is changed.
