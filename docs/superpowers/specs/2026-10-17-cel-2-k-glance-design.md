# CEL-2-K — Glance: system monitor, weather and calendar — design

- **Date:** 2026-10-17
- **Status:** drafted from the author's answers in brainstorming (three
  decisions, §2); awaiting the author's review of this document
- **Unit:** `CEL-2-K` of plan `cel-2-parity`, project `celestina`
- **Builds on:** [the parity program](2026-10-16-celestina-parity-program-design.md)
  (§5 row K, §7 "K"), the piece contract of CEL-2-A, and the live-follow of
  `shell.json` that CEL-2-B and CEL-2-E share

## 1. Goal

Three pieces of "see the machine and the day at a glance", each a `Piece` of the
table, plus one Island page:

| Piece | In the bar | Page |
|---|---|---|
| `sysmon` | CPU and memory percentages | its own page: the numbers, a short history and the temperature |
| `weather` | the current temperature (whole °C) | opens the clock page |
| `clock` | the short date ("vie 17") | the **Reloj** page: the month calendar and the weather reading |

There is no duty switch (program §5 row K): a piece appears when the author
places it in `shell.json`'s `bar` zones. The weather is the one thing in the
shell that touches the network, so §3 records the decision that bounds it.

Out of scope: events, reminders, alarms, a world clock, timers (the program
excludes the productivity group); a weather forecast beyond the current
reading; a location picker UI; any geolocation.

## 2. Decisions the author made in brainstorming (2026-10-17)

| Decision | Choice | Alternatives considered |
|---|---|---|
| Location | **Manual only.** The author writes latitude, longitude and a label in `shell.json`; without them there is no request and no piece content. The shell never works out where the person is | a verb that geolocates by IP once; automatic geolocation like Noctalia's |
| Source and default | **Open-Meteo, off unless a location is set.** No key, no account, a GET through `curl` on its own worker, as the halted shell did | a second explicit `weather.enabled` switch; no weather in this unit |
| Structure | **Three pieces, one Reloj page**: `sysmon`, `weather`, `clock` | one `glance` piece and page |

Decided without asking (conventional defaults, reviewable here):

- **Calendar source:** computed locally by arithmetic (the salvaged
  `celestina-shell::calendar`, Monday-first), with no events and no network.
  An agenda from a local `.ics` file is a later, separate decision.
- **System reading:** reuse `hematita-core` (`cpu`, `memory`, `sensors`),
  which already parses `/proc/stat`, `/proc/meminfo` and hwmon, instead of
  copying the halted `sysmon.rs` core (§5).

## 3. The weather and the network (decision record)

The shell does not call the network until a decision is recorded; this is it.

1. The weather worker makes **no request unless `weather.location` is present
   and valid** in `shell.json`. Removing the key stops requests at the next
   follow of the file and withdraws the reading.
2. There is **no geolocation of any kind**: no IP lookup, no GeoClue, no
   timezone-based guess. Coordinates come only from the file.
3. One request carries exactly the rounded coordinates (two decimals, about
   1 km) and the three fields asked for (`temperature_2m`, `weather_code`,
   `is_day`). No identifier, no place name, no key, no cookie, no history.
   `curl` is run with `--fail --silent --max-time` and a bounded output read.
4. The reply is bounded (16 KiB) and checked before it is believed; a reading
   older than 15 minutes is **absent**, not shown stale. A failure waits five
   minutes before asking again.
5. Tests and the smoke **never reach the network**: the fetch is behind a
   `Fetch` trait; tests inject a fake, and the nested smoke runs with the
   location unset, or with a fetcher override that serves a canned body from a
   file named by an environment variable that exists only in test builds of the
   harness (§7).
6. The author's third-party key in Noctalia's settings is never read; the
   shell reads no Noctalia file.

The falsifier: if a packet capture of a shell started with no location shows a
request to the weather host, this decision is violated.

## 4. Architecture

```text
celestina-rs/crates/celestina-shell/src/
  weather.rs     salvage by copy of celestina-shell-core::weather (request_url,
                 read, next_step, still_worth_showing) and Location from its settings
  calendar.rs    already in the crate (salvaged earlier): Month, weekday, rows
  settings.rs    + `weather: { latitude, longitude, label }` (optional)
  glance.rs      NEW (small, pure): Load thresholds (80/90), the short-date and
                 month-title formatting inputs (names come from copy.rs)
celestina/src/providers/
  sysmon.rs      worker: /proc sampler every 2 s through hematita-core, publishes
                 SysReading { cpu, ram, temperature, history }; withdraws on error
  weather.rs     worker: the Fetch trait, the curl fetcher, the decision loop;
                 publishes WeatherReading or withdraws
celestina/src/pieces/
  sysmon.rs weather.rs clock.rs   three Piece impls (pure, stateless)
celestina/src/island/pages/
  clock.rs sysmon.rs               the two PageViews
```

- **Providers** follow the CEL-1 worker shape (named thread behind the
  `Worker` guard, a typed `Message`, bounded channels, joined shutdown). The
  sysmon worker runs only while a *placed piece* needs it and the bar duty
  is on (it is spawned when `sysmon` is in a zone, stopped when removed); an
  open page alone does not start it. Reason: a page that could start a worker
  would make an open page a second owner of the worker's lifecycle and would
  keep `/proc` sampling alive for a surface that is not part of the bar's
  rest budget. Instead the System page says honestly that it has no data and
  asks for the piece to be placed, and the Reloj page says the date comes
  from the bar. An unplaced piece costs no wakeup. The weather worker runs only with a valid
  location and a placed `weather` or `clock` piece.
- **Slices:** `BarCx` and `PageCx` gain one read-only slice each for the
  sysmon reading, the weather reading and today's date (year, month, day,
  weekday from the same wall clock `island/clock.rs` already reads). A piece
  reads only its slice.
- **Rest cost:** the bar already redraws once a minute. The sysmon reading
  would change every two seconds, which would break "rest is one frame a
  minute" (ADR 0015). So **the sysmon bar widget redraws at most once every
  ten seconds** (the worker publishes every two seconds, the loop coalesces
  into the widget's display on a ten-second clock) and the page, while open,
  uses the same one-second tick as the page clock. The nested smoke asserts
  zero frames in a ten-second rest window with sysmon placed, then at most
  one.
- **Calendar page state:** the displayed month (relative offset from the
  current month, reset when the page closes) is Island page state; Previous,
  Next and "today" are keyed buttons routed like the Tray page's rows. The
  today cell is highlighted; weeks start on Monday; nothing is written to
  disk.
- **Failure modes:** unreadable `/proc` or hwmon withdraws the sysmon reading
  (the widget shows nothing, not zero); a failed or absent weather reading
  makes the `weather` piece absent (a zone with no content draws no capsule)
  and the Reloj page shows only the calendar; the clock piece never fails.

## 5. Reuse and salvage

- **System reading:** `hematita-core::{cpu, memory, sensors, history::Ring}`
  are the monorepo's one owner of the parsing. Celestina becomes a narrow
  consumer of them, as Siderita is of `hematita-core::usage`;
  `docs/standards/architecture.md` lists this consumer in the same unit. The
  halted `sysmon.rs` core and `provider_adapter/sysmon.rs` are consulted as
  reference only and are not copied; the 80/90 thresholds are policy and are
  re-stated in `glance.rs`.
- **Weather:** `celestina-shell-core/src/weather.rs` is copied into
  `celestina-shell` (the file is already pure and tested) with its source path
  and commit in the head comment; `provider_adapter/weather.rs` is reference
  for the worker. The root `AGENTS.md` salvage list names these for CEL-2-K
  (added by the salvage session before the unit's first commit).
- **Calendar:** `calendar.rs` is already in `celestina-shell`; nothing is
  copied.

## 6. Piece details

- **`sysmon`** — widget: `CPU 23% · RAM 41%` in tabular figures; the colour
  follows `Load` (normal, elevated, critical) through theme roles. Click opens
  the page. The page lists CPU and memory (used/total), the hottest CPU
  temperature when hwmon provides one, and a 60-sample sparkline per series
  (the `Ring`); a row opens Mission Center, the monitor the session already
  has, through the launcher's spawn path (no new process code).
- **`weather`** — widget: `12°` and the condition icon (day/night variant);
  absent while there is no fresh reading. Click opens the Reloj page.
- **`clock`** — widget: the short date; click opens Reloj. The Island's own
  capsule clock is unchanged.
- **Reloj page** — title row with the seconds clock (CEL-2-A), then the month
  calendar with Previous, Next and today, then the weather reading (temperature,
  condition word, place label, "updated hh:mm") when present. Spanish copy in
  `copy.rs` (weekday and month names, condition words from the Open-Meteo
  codes, labels).
- All pieces are reachable without a pointer through their page and the
  `island open clock` / `island open sysmon` verbs (the bar takes no keyboard
  yet; ADR 0007).

## 7. Verification

- **Pure tests:** the salvaged weather rules (URL carries only rounded
  coordinates, bounded reply, freshness, retry); the calendar month rows;
  settings round trip for `weather` including invalid latitude, a hostile
  label length and a wrong type; the `Fetch` fake driving the decision loop
  without a clock; `Load` thresholds; the display cadence of the widget.
- **Characterisation first:** the three CEL-2-A pieces' views and the default
  zones are snapshotted before the table grows and asserted unchanged.
- **Providers:** the sysmon worker is tested against a fixture directory (a
  fake `/proc` root and hwmon root passed in, never the real ones); the weather
  worker against the fake `Fetch`.
- **Network guard test:** a test that starts the weather provider with no
  location, runs it for several ticks with a fetcher that panics if called,
  and asserts silence (the §3 rule 1 falsifier in code).
- **Nested smoke:** the nest with sysmon and clock placed and no location;
  with a canned weather body from a fixture file; metrics: a rest window with
  at most one frame in ten seconds; the process makes no outbound connection
  (checked with `ss` on the process, no `curl` child spawned).
- **Guards:** `cargo fmt --check`, `cargo clippy --all-targets -D warnings`,
  `cargo test`, `celestina --check`, `smoke.sh`, then
  `bash scripts/check-architecture-contract.sh`, the language and
  documentation contracts.
- **Live rows (author):** `VAL-CEL2-SYSMON` (the numbers agree with a monitor,
  the colour changes under load), `VAL-CEL2-WEATHER` (with a location set the
  temperature appears, with it removed the piece disappears and `ss` shows no
  connection), `VAL-CEL2-CALENDAR` (month navigation, today highlight, the
  seconds clock on the page).

## 8. Risks

| Risk | Mitigation |
|---|---|
| The two-second sysmon sample breaks the rest-at-one-frame rule | the widget redraws on a ten-second coalesced clock; the smoke measures it |
| A weather request leaks when no location is set | the worker does not start without a location; the network guard test and the smoke check |
| A new cross-project dependency on `hematita-core` | narrow consumer, listed in the architecture standard; the modules used are pure parsers with no UI |
| The locale-specific names (months, weekdays) drift from a library | they are product copy in `copy.rs`, checked by the language contract |
| Reading hwmon is blocking IO | on the worker, bounded to a few files, never on the loop |

## 9. Decisions recorded here (to become `celestina/docs/decisions/`)

- **The shell touches the network only for the weather, only with a manual
  location, and never to find out where the person is** (§3).
- **Glance is three pieces and one page, with no duty switch** (§2, §4).
- **Celestina consumes `hematita-core`'s system parsers** rather than keeping
  a second copy (§5).
