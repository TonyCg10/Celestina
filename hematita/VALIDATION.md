# Author validation — Hematita

This queue contains no implementation work and never blocks `ROADMAP.md`.

## VAL-H1 — Live CPU and memory on the real session

- **Status:** pending
- **Related implementation:** H1
- **Requires:** the deployed Hematita on the real session
- **Procedure:** launch `hematita`; watch the CPU graph for a minute while
  compiling something; switch to Memory with the pointer and with the arrow
  keys from the strip; read the strip with the screen reader; check the
  monitor's own CPU in another tool while idle
- **Pass condition:** the graph advances once per second without stutter;
  values match another monitor within a few percent; the strip is reachable by
  Tab and walkable by arrows; the screen reader names each section and its
  checked state; Hematita idles under 1 % CPU
- **Result:** not run
- **Evidence:** none
