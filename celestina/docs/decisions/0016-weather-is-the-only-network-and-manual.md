# ADR 0016: The weather is the only network use and its location is manual

- **Date:** 2026-10-17
- **Status:** proposed

## Context

CEL-2-K adds three glance pieces ([design](../../../docs/superpowers/specs/2026-10-17-cel-2-k-glance-design.md)):
a system monitor, the weather and a calendar. The weather is the first thing in
the shell that would leave the machine, and a place name or a coordinate pair
is personal data. The author chose, in brainstorming on 2026-10-17, a manual
location, Open-Meteo through `curl` and no automatic geolocation. The shell
already parses `/proc` and hwmon text nowhere, while `hematita-core` owns those
parsers. The halted shell kept its own copy.

## Decision

1. **Network.** The shell touches the network only for the weather, and only
   when `weather.location` in `shell.json` is present and valid (a latitude in
   -90..=90, a longitude in -180..=180, a label of at most 64 characters).
   Without it no worker fetches, no `curl` starts, and the reading is withdrawn.
   Removing the key stops requests at the next follow of the file.
2. **Location.** It is written by the author and by nothing else: no IP lookup,
   no GeoClue, no timezone guess, no reading of Noctalia's files or key. One
   request carries only the coordinates rounded to two decimals and the fields
   `temperature_2m`, `weather_code` and `is_day`; no label, identifier, key or
   cookie. `curl` runs with `--disable --proto =https --max-filesize --fail
   --silent --max-time`; the reply is bounded to 16 KiB; a reading older than 15
   minutes is absent; a failure waits five minutes. Tests and the smoke never
   reach the network: the fetch is behind a trait, and the smoke puts a fake
   `curl` first on `PATH`.
3. **Structure.** Glance is three pieces, `sysmon`, `weather` and `clock`, and
   two Island pages, Sistema and Reloj, with no duty switch: a piece is on when
   the author places it in a `bar` zone, and its worker runs only then.
4. **Shared parsers.** The system monitor consumes `hematita-core::{cpu, memory,
   sensors}` rather than keeping a second copy; the dependency is registered in
   the manifest, `docs/projects.toml` and the architecture standard, and covers
   those modules only.

## Consequences

- Without a location the shell is as quiet on the network as before.
- A user wanting weather writes coordinates by hand; there is no picker.
- `curl` inherits proxy environment variables and sends its default user agent.
- A change to the parsers in `hematita-core` reaches Celestina, so the
  production inputs of `celestina` include that crate.
- Any other network use needs a new record that supersedes this one.

## Revisit when

A packet capture of a shell started with no location shows a request to the
weather host or any other; or a second feature needs the network; or the author
asks for a location picker or an agenda source.
