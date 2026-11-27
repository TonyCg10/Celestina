# CEL-I-015 Implementation Plan: a better launcher search, ranked by use

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The launcher finds applications by their localized names and keywords, by words in any order, with one typo, and by Spanish category words, and orders them by how often and how recently they were launched.

**Architecture:**
- `celestina-core`'s `.desktop` reader keeps the localized values it drops today.
- A new pure matcher in `celestina-shell` prepares each entry once per index and scores queries against it.
- A new use record (`usage.json`) replaces `recent.json`'s writer and seeds itself from it.
- The launcher's state shows localized display names.

**Tech Stack:** Rust; `celestina-core`, `celestina-shell`, `celestina/src/launcher`; `serde_json` (already a dependency of the shell).

**Spec:** `docs/superpowers/specs/2026-11-26-celestina-launcher-design.md` §5.6 and §9 calls 1 and 8.

**Branch:** `unit/celestina/CEL-I-015` from `main`. Kind `milestone`.

**Two landings.** `celestina-shell` is one of `celestina`'s commit roots, but `celestina-core` is not: it belongs to `celestina-rs` and has its own component prefix, `celestina-core` (`docs/projects.toml`). `CONTRIBUTING.md` says component prefixes are for atomic code commits that close no ledger unit. So:
- Task 1 lands first, on its own branch, as one `celestina-core-milestone:` commit, by whatever path the author authorises for a component commit. Ask before starting: `land-unit.py` closes ledger units, and this commit closes none.
- Tasks 2–6 then land as the `CEL-I-015` unit on a `main` that already carries Task 1.

## Global Constraints

- English everywhere except `celestina/src/copy.rs` (the head says `language-contract: product-copy`). The Spanish words of the group labels and aliases are the preview's: `TABS[].lbl` and `const ALIAS` in `/home/toni/CODIGO/previews/2026-11-26-launcher/index.html`. Copy them into `copy.rs` with their accents (for example the IDE alias's word for code carries an acute accent).
- No new dependency.
- Existing `DesktopEntry` fields keep their meaning; Siderita and every other caller compile and behave unchanged.
- The `Mod+K` selector keeps `celestina_shell::launcher::score`; `/` switches are unchanged.
- `MAX_ROWS` (8) still caps the launcher's rows in this unit; `CEL-I-017` lifts it.
- Live-session safety and checks as in `celestina/AGENTS.md`; nests only.

## Review Focus

1. A `.desktop` file with hundreds of `Name[xx]` lines: the reader keeps at most `MAX_LOCALIZED_PER_KEY` (64) per key and the scan stays bounded.
2. `LANG=C`, `LANG=POSIX`, an empty `LANG`, or `es_ES.UTF-8@euro`: no panic; C/POSIX/empty mean no locale; the encoding is dropped and the modifier kept.
3. A `usage.json` that is not JSON, holds a negative or float count, a 10 000-byte id, or 5 000 ids: it reads as empty or is trimmed, logs once, and never panics.
4. A query of only spaces or punctuation shows the empty-query list, not "no results".
5. Two launches in the same second keep both counts (no lost update when `record` is called twice before a write completes; the write is whole and atomic).

---

### Task 1: localized values in the `.desktop` reader

**Files:**
- Modify: `celestina-rs/crates/celestina-core/src/desktop_entry.rs` (struct, `parse_with`, new `Locale`, tests)

**Interfaces:**
- Produces:
  - `pub struct Locale { pub lang: String, pub country: Option<String>, pub modifier: Option<String> }`
  - `impl Locale { pub fn parse(value: &str) -> Option<Locale>; pub fn from_vars(get: impl Fn(&str) -> Option<String>) -> Option<Locale>; pub fn from_env() -> Option<Locale> }`. `from_vars` reads `LC_ALL`, then `LC_MESSAGES`, then `LANG`, taking the first non-empty one. `parse` drops `.encoding` and returns `None` for `C`, `POSIX` and empty values.
  - Fields on `DesktopEntry`: `pub name_l10n: Vec<(String, String)>`, `pub generic_name_l10n: Vec<(String, String)>`, `pub comment_l10n: Vec<(String, String)>`, `pub keywords_l10n: Vec<(String, Vec<String>)>`. Each holds (locale tag as written, value).
  - `pub const MAX_LOCALIZED_PER_KEY: usize = 64;`
  - `impl DesktopEntry { pub fn localized_name(&self, locale: &Locale) -> Option<&str>; pub fn localized_generic_name(&self, locale: &Locale) -> Option<&str>; pub fn localized_keywords(&self, locale: &Locale) -> Option<&[String]> }`. Each returns the first match in the specification's order: `lang_COUNTRY@MODIFIER`, `lang_COUNTRY`, `lang@MODIFIER`, `lang`. `None` means the caller uses the unlocalized field.

- [ ] **Step 1: Write the failing tests:**
  - the existing fixture at line ~723 (it carries `Name[es]`, `GenericName` and `Keywords`) now yields `name_l10n == [("es", <its value>)]`;
  - `Locale::parse("es_ES.UTF-8@euro")` gives lang `es`, country `ES`, modifier `euro`;
  - `Locale::parse("C")` is `None`;
  - with both `Name[es_ES]` and `Name[es]` present and locale `es_ES`, `localized_name` gives the `es_ES` value; with locale `es_MX` it gives the `es` one;
  - `from_vars` prefers `LC_ALL` over `LANG` and skips an empty `LC_ALL`;
  - a body with 100 `Name[xx]` lines keeps 64;
  - localized values are unescaped like the rest under `read`.
  
  Fixtures stay ASCII English or Unicode escapes, as the file already does.
- [ ] **Step 2: Run** `cargo test -p celestina-core --locked desktop_entry` from `celestina-rs/`. Expected: FAIL.
- [ ] **Step 3: Implement.** In `parse_with`, split a key of the form `Key[tag]` with `split_once('[')` and a trailing `]`. For the four keys, push `(tag, values.string(value))` (or `values.list` for Keywords) while the key's vec is shorter than the bound. Update the doc comment on `name` that says localized variants are ignored.
- [ ] **Step 4: Run** `cargo test -p celestina-core --locked` and `cargo clippy -p celestina-core --all-targets --locked -- -D warnings`. Expected: PASS.
- [ ] **Step 5: Commit** `Add the localized Name, GenericName, Comment and Keywords values to the desktop entry reader`

### Task 2: the matcher

**Files:**
- Create: `celestina-rs/crates/celestina-shell/src/launcher_search.rs` (with tests)
- Modify: `celestina-rs/crates/celestina-shell/src/lib.rs` (`pub mod launcher_search;`)

**Interfaces:**
- Consumes: `celestina_shell::launcher::fold` (reuse; do not write a second fold), Task 1's `Locale` and accessors.
- Produces:
  - `pub struct Prepared { pub id: String, pub display: String, pub detail: String, /* private folded texts */ }`
  - `impl Prepared { pub fn new(entry: &DesktopEntry, locale: Option<&Locale>, aliases: &[&str]) -> Prepared }`. `display` is the localized `Name` or `Name`; `detail` is the localized `GenericName` or `GenericName`. Folded name words come from both names; secondary words come from both generic names, both keyword lists and `aliases`; plus the folded `Exec`.
  - `pub fn search(entries: &[Prepared], query: &str, use_score: &dyn Fn(&str) -> u64) -> Vec<usize>`: indices into `entries`, best first. An empty or all-separator query returns an empty vec, and the caller shows its empty-query list.
  - `pub const MAX_TERMS: usize = 8; pub const MAX_TERM_CHARS: usize = 64;`
  - `fn within_one_edit(word: &str, term: &str) -> bool` (private; tested). Optimal string alignment distance ≤ 1 on chars, checked against the word's prefixes of the term's length −1, +0 and +1 and the whole word.

Scoring, verbatim from the spec:
- **Terms.** The query is folded and split on whitespace and on characters that are neither alphanumeric nor `+`. At most 8 terms, each cut to 64 chars.
- **Term score**, the best tier that matches:
  - 60: the term equals a folded name;
  - 50: a name starts with it;
  - 40: a name word starts with it;
  - 30: a secondary word starts with it;
  - 20: a name word is within one edit, for terms of 4 chars or more;
  - 15: a secondary word is within one edit, same length rule;
  - 10: the term is a subsequence of a name;
  - 5: the term is a subsequence of a folded secondary text or `Exec`;
  - no tier: the term misses.
- **Entry.** An entry matches when every term scores. Its score is `sum / terms` (integer division is fine), forced to 60 when the folded whole query equals the folded `display`.
- **Order.** Score desc, then `use_score(id)` desc, then folded `display` asc.

- [ ] **Step 1: Write the failing tests:**
  - tiers: `Krita` for `krtia` (20); a keyword word start (30) above a name subsequence (10);
  - order freedom: a two-word query matches a `GenericName` with the words reversed;
  - an alias passed in `aliases` matches;
  - use order breaks ties;
  - name order breaks equal use;
  - bounds: a 10 000-char query and a 50-term query return without panic, using at most 8 terms;
  - the whole-name rule;
  - `within_one_edit` cases: insertion, deletion, substitution, swap, two edits false;
  - empty and punctuation-only queries.
- [ ] **Step 2: Run** `cargo test -p celestina-shell --locked launcher_search`. Expected: FAIL.
- [ ] **Step 3: Implement** as specified. Keep `launcher.rs` untouched.
- [ ] **Step 4: Run** `cargo test -p celestina-shell --locked` and clippy. Expected: PASS.
- [ ] **Step 5: Commit** `Add the launcher's word, typo and alias matcher`

### Task 3: the use record

**Files:**
- Create: `celestina/src/launcher/usage.rs`
- Modify: `celestina/src/launcher/recent.rs` (drop `record` and the writer; keep `load_from`/`parse` and `ids()` as the seeder; update its head comment)
- Modify: `celestina/src/launcher/mod.rs` (`pub mod usage;`)

**Interfaces:**
- Produces:
  - `pub struct Launch { pub count: u32, pub last: u64 }` (`last` in Unix seconds)
  - `pub struct Usage { path: Option<PathBuf>, apps: BTreeMap<String, Launch> }`
  - `impl Usage { pub fn load() -> Usage; pub fn load_from(path: Option<PathBuf>, seed: Option<PathBuf>) -> Usage; pub fn record(&mut self, id: &str, now: u64); pub fn score(&self, id: &str, now: u64) -> u64; pub fn scores(&self, now: u64) -> HashMap<String, u64> }`
  - Constants: `pub const MAX_APPS: usize = 200;` and `const MAX_FILE_BYTES: u64 = 64 * 1024;`
- The file is `$XDG_CONFIG_HOME/celestina/usage.json`: `{"version":1,"apps":{"<id>":{"count":N,"last":S}}}`.
- Read and write bounded and atomically as `recent.rs` does (`celestina_core::atomic_file::read_bounded` and `replace_private`); validate ids as `recent.rs` does (at most `MAX_ID_BYTES`).
- Past `MAX_APPS`, drop the lowest scores.
- The score is `count × weight(now − last)`. The weight is:

  | Age of the last launch | Weight |
  |---|---|
  | under 4 days | 100 |
  | under 14 days | 70 |
  | under 31 days | 50 |
  | under 90 days | 30 |
  | 90 days or more | 10 |

  A future `last` counts as age 0. `count` saturates.
- **Seed.** When the `usage.json` path holds no file and `recent.json` reads, the i-th recent id (0 is the newest) gets `count 1` and `last = now − 60 × i`. The seeded record is written once. `recent.json` is never written again and is left in place.

- [ ] **Step 1: Write the failing tests** (temp dirs as `recent.rs`'s tests do):
  - a round trip;
  - the seed order;
  - each weight bucket's boundary;
  - eviction at 201;
  - Review Focus 3's malformed inputs;
  - a future timestamp.
- [ ] **Step 2: Run** `cargo test -p celestina --locked launcher::usage`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `Add the launcher's use record, seeded once from the recent launches`

### Task 4: groups and aliases (product side)

**Files:**
- Create: `celestina/src/launcher/groups.rs`
- Modify: `celestina/src/copy.rs` (new constants), `celestina/src/launcher/mod.rs`

**Interfaces:**
- Produces:
  - `pub enum Group { Internet, Multimedia, Games, Development, Office, System, Utilities }` with `pub const ALL: [Group; 7]` in tab order (Internet, Multimedia, Games, Development, Office, System, Utilities), `pub fn label(self) -> &'static str` and `pub fn icon(self) -> &'static str` (`globe`, `film`, `gamepad-2`, `file-code`, `file-text`, `settings`, `toolbox`).
  - `pub fn group_of(categories: &[String]) -> Group`, which assigns by the spec's priority order (§5.2):
    1. Games: `Game`;
    2. Development: `Development`;
    3. Multimedia: `AudioVideo`, `Audio`, `Video`, `Graphics`;
    4. Internet: `Network`;
    5. Office: `Office`, `Education`, `Science`;
    6. System: `System`, `Settings`;
    7. Utilities: everything else.
  - `pub fn aliases(categories: &[String]) -> Vec<&'static str>`: the group's label plus one constant per category in the preview's `ALIAS` table (keys `WebBrowser`, `TerminalEmulator`, `TextEditor`, `FileManager`, `Player`, `IDE`, `InstantMessaging`, `Monitor`, `Emulator`, `AudioVideoEditing`, `RasterGraphics`, `2DGraphics`).
  - Copy constants `LAUNCHER_GROUP_INTERNET` … `LAUNCHER_GROUP_UTILITIES` and `LAUNCHER_ALIAS_WEB_BROWSER` … in `copy.rs`.

- [ ] **Step 1: Write the failing tests:**
  - `Utility;Game;Graphics` → Games;
  - `Utility;Development` → Development;
  - `X-WayDroid-App` → Utilities;
  - `Network;WebBrowser` gives aliases holding the Internet label and the web-browser alias.
- [ ] **Step 2: Run them to verify they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them to verify they pass.**
- [ ] **Step 5: Commit** `Add the launcher's application groups and Spanish category aliases`

### Task 5: wire the state, the view and the app

**Files:**
- Modify: `celestina/src/launcher/state.rs`, `view.rs`, `mod.rs`, `celestina/src/app.rs` (~line 479 field, ~1572 open, ~1695 launch), `celestina/src/main.rs` (~line 182)

**Interfaces:**
- Consumes: Tasks 1–4.
- Produces:
  - `Row::App(Box<AppRow>)` with `pub struct AppRow { pub entry: DesktopEntry, pub name: String, pub detail: String }`. The view shows `name` and `detail`, and `LauncherAction::Launch` still carries the `DesktopEntry`.
  - `LauncherState::new(scores: HashMap<String, u64>, locale: Option<Locale>)`. The state keeps `prepared: Vec<Prepared>`, rebuilt when the index's length or ids differ from the last build. Each entry is prepared with `groups::aliases(&entry.categories)`.
  - Empty query: every entry ordered by use score desc, then folded display name, capped at `MAX_ROWS`.
  - `Launcher::new(surface, output, theme, reduced, scores, locale, index)`.
  - `App` replaces `recent: RecentLaunches` with `usage: Usage`. On a launch it calls `usage.record(&entry.id, now_unix)`; on an open it passes `usage.scores(now_unix)` and `Locale::from_env()`.

- [ ] **Step 1: Update the tests in `state.rs`:**
  - `recent_use_lifts_an_entry` becomes a use-score test with the same intent;
  - `ranking_orders_prefix_then_word_start_then_substring` keeps its order expectations under the new tiers (exact 60, prefix 50, word 40, subsequence 10);
  - add a localized-name test (an entry with `name_l10n = [("es", "X")]` and locale `es` displays `X`).
  
  Update `view.rs` tests for `AppRow`.
- [ ] **Step 2: Run** `cargo test -p celestina --locked launcher::` and see the failures.
- [ ] **Step 3: Implement** the wiring. Keep the selector's path (`selector.rs`) on the old `score`.
- [ ] **Step 4: Run** all three workspace checks from `celestina/`. Expected: PASS.
- [ ] **Step 5: Commit** `Replace the launcher's ranking with the word, typo and alias matcher ranked by use`

### Task 6: evidence in a nest

- [ ] Private target dir, one nest as in `celestina/AGENTS.md`.
  - Copy the author's index fixture (`previews/2026-11-26-launcher/apps.json` lists the ids) only as a list of ids to check: do not copy his `.desktop` files into the repo.
  - In the nest, use a throwaway `XDG_DATA_HOME` with three hand-written `.desktop` files that carry `Name[es]`, `Keywords[es]` and `Categories=Network;WebBrowser;`, plus `LANG=es_ES.UTF-8`.
  - Take `grim` shots of these queries: the web-browser alias, a typo, two words reversed.
- [ ] Run the launcher's smoke phases (`smoke.sh --list`, then `--only`).
- [ ] Write the evidence with the screenshots.
