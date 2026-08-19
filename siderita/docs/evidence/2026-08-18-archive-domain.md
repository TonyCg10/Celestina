# Evidence: 2026-08-18 the archive domain

- **Date:** 2026-08-18
- **Scope:** `SID-A1-A`; plan
  [archive-compression](../plans/active/2026-08-18-archive-compression.md)
- **Environment:** Arch-derived Linux, `cargo` stable, `unrar` 7.23 and `7-Zip`
  present on the machine. Local time `-0400`, which is what made the zip date
  defect visible at all
- **Artifact:** none built here; the application's own build and deployment are
  `SID-A1-B`'s

## What the domain does

`siderita-archive` identifies a container by its bytes, lists it, extracts it
into a folder and creates a `.zip` or `.tar.gz` from entries on disk. The
filesystem guarantees are `siderita-ops`' own, reused rather than restated:
cancellation, byte and item counts, and the rule that no verb overwrites an
existing entry or leaves a partial result behind.

Zip, tar and tar.gz are decoded in Rust. RAR and 7z are delegated to a tool the
machine already has, because RAR's only decoder ships under a licence a GPL
program may not link and 7z has no mature pure-Rust reader. The format is
offered only when the tool is installed.

## Procedure

Every check below ran against archives produced by other tools, not by this
domain, because a round trip through one implementation proves only that it
agrees with itself.

| Check | Result |
|---|---|
| `cargo test -p siderita-archive` | 22 tests pass |
| `cargo test -p siderita-ops` | 39 tests pass |
| `cargo clippy --workspace --all-targets` | no warnings |
| 12 MB zip from GitHub (`phosphor-icons-2.1.2.zip`) | 66 members extracted, byte-identical to `unzip`, same dates |
| A zip written here, read by `unzip -l`, `unzip -t`, `python zipfile` | no errors; dates match the source files |
| `zip`-written archive extracted here vs by `unzip` | identical trees |
| tar.gz written here, read by `tar tvzf` | correct tree, modes, symlink and dates |
| Encrypted zip, ZipCrypto and AES-256 | asks, refuses a wrong password, opens with the right one |
| Author's `SEXOPHOBIA_….rar` (1.5 GB, encrypted) | 406 members / 1 866 737 733 bytes, exactly what `unrar` lists |
| Author's `Persona 5 Royal_Crack….rar` (43 GB, encrypted headers) | measured in 30 ms (45.26 GiB); extraction cancelled mid-run leaves the destination empty and no orphan process |
| Hand-built zip holding `../../ESCAPADO.txt` | whole extraction refused, destination untouched |

## Result

Every check above passes. The domain reads what other tools write and writes
what other tools read, including dates, modes, symlinks and non-UTF-8 names, and
it refuses — without writing anything — an archive that would escape its
destination.

## Two defects this work fixed in passing

**Zip dates were written in UTC into a field that has no zone.** Every other
tool reads that field as local time, so an archive written here showed up hours
off elsewhere, and one written elsewhere came back shifted. The domain now asks
its caller for the offset in force *at each instant* (`Zone`), and also writes
the exact Unix instant in the `0x5455` extended field, which it prefers on read.
Checked against `unzip -l` in both directions, in winter and summer dates.

**`std::fs::rename` replaces its destination silently.** Every move verb looked
first and renamed second, and documented the window between the two as
survivable for a single-user manager. It stops being survivable the moment two
of this manager's own operations run at once, so `siderita-ops` now reserves the
destination name atomically and renames onto its own reservation. The loser of a
race is told the name is taken instead of overwriting the winner.

## Limits

- A RAR without a password has not been extracted end to end: both of the
  author's RARs are encrypted and no `rar` compressor exists on the machine to
  make one. The delegated success path is covered by 7z.
- Progress from a delegated tool is read from the lines it prints. A member name
  holding two consecutive spaces would cost that member's byte count, never its
  extraction.
