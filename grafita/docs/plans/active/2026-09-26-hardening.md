# GRA-H1 — Hardening after the monorepo audit

- **Opened:** 2026-09-26
- **Plan ID:** hardening
- **Status:** active
- **Authorization:** after the 2026-09-26 monorepo audit the author asked
  for its whole program to be done; the findings and rulings are in
  [the audit evidence](../../../../docs/evidence/2026-09-26-monorepo-audit.md)
  and the Grafita findings in
  [the Hematita and Grafita record](../../../../docs/evidence/2026-09-26-monorepo-audit-hematita-grafita.md)
- **Scope:** grafita
- **Implementation checkpoint:** GRA-H1
- **Author-validation checkpoint:** none

## Hypothesis

Bounding nesting and decompressed size, and replacing unchecked slices with
typed errors, makes every hostile document the importer receives a refusal
instead of a process abort, in Grafita and in Siderita's embedded preview
alike.

## Tangible outcome

A 20 KB nested PDF, a 1 GiB gzip bomb, a lying ZIP header and a huge `/N` are
each refused with a typed error by `grafita-core`, under tests that also pass
as root; the highlighter no longer does quadratic work on a long line; and
the recent list and preferences stop touching the disk on the GUI thread.

## Scope

- `GRA-H1-A` (P-5) — harden the document importer against hostile input.
- `GRA-H1-B` (P-19) — UTF-16 highlight runs, recent and preference IO off
  the GUI thread, a guarded highlighter target, the shared `file_uri` owner,
  and STATUS truth.

## Exclusions

- GRA-6 (a whole-document round trip per keystroke), a later milestone.
- Production builds and deployment, which run at the author's landing.

## Build order

1. `GRA-H1-A` from `main`.
2. `GRA-H1-B`, stacked on `GRA-H1-A`, after `RS-H1-A` (P-6) landed.

## Implementation exit

Each row's `Automated evidence` names its exit; every unit ends with
Grafita's and Siderita's `scripts/complete-production.sh` at landing, because
Siderita links `grafita-core`. Under ruling R-A2 only Grafita's version
moves.

## Change and commit ledger

Paths are repository-relative. Each row's `Intended change` ends with the
program id and the audit findings it closes.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| GRA-H1-A | `grafita:` | planned | `celestina-rs/crates/grafita-core/src/import/` (`pdf/object.rs`, `pdf/file.rs`, `gzip.rs`, the ZIP-based importers); `celestina-rs/crates/grafita-core/tests/imported.rs`; `celestina-rs/crates/grafita-core/tests/documents.rs` | — | Bound PDF nesting. Replace unchecked slices with typed errors and use `checked_add`. Cap every decoder with `take(limit+1)` and a total across members and filters. Stop trusting `uncompressed_size`/`/N`. Memoise object streams. Add a negative-input table. (P-5: GRA-1, GRA-2, GRA-3, GRA-7) | `cargo test -p grafita-core --offline --no-fail-fast` with fixtures: 20k `[`, non-UTF-8 xref, `startxref` past EOF, `/W` overflow, huge `/N`, 1 GiB gzip → `TooLarge`, lying ZIP header; passes as root. Landing runs Grafita **and** Siderita `complete-production.sh`. | None |
| GRA-H1-B | `grafita:` | planned | `grafita/cpp/highlighter.cpp`; `grafita/cpp/highlighter.h`; the `grafita-core` highlight runs API; `celestina-rs/crates/grafita-core/src/session.rs`; `celestina-rs/crates/grafita-core/src/recent.rs`; `celestina-rs/crates/grafita-core/src/preferences.rs`; `grafita/src/url.rs`; `grafita/STATUS.md` | — | Return UTF-16 runs from Rust in one pass and coalesce the palette rehighlight. Recent-list and `existing()` as worker jobs, and debounced preference writes. `QPointer` target. Adopt `file_uri` (non-UTF-8 byte-exact). STATUS. (P-19: GRA-4, GRA-5, GRA-8, GRA-9; RS-1 (grafita)) | `cargo test -p grafita-core` (recent list via a job; no IO in `receive`); Grafita syntax test on a 5 MB single line within a time bound; Grafita **and** Siderita `complete-production.sh` | None |

This plan records intent; it grants no authority.
