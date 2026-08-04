# ADR 0007: Product copy is Spanish; development truth stays English

- **Date:** 2026-08-04
- **Status:** accepted

## Context

[The language standard](../standards/language.md) made English the only
language stored in the repository and listed "canonical UI copy" among the
things it governs. Every other item on that list — rules, documentation, plans,
identifiers, comments, logs, diagnostics, tests, commit subjects — is
development truth, read by whoever works on the suite. UI copy is the one item
on it that is read by the person *using* the products.

The author writes and reads Spanish, is the user of this desktop, and asked for
Spanish in the interface as well as in conversation. The standard as written
forbids that, and it was enforced: implementing Fluorita's source-first library
translated its surface to English because the editing rule requires a
substantially rewritten file to be English throughout. That produced a Spanish
desktop with an English media library, which is not a product decision anyone
made — it is a rule about *development* language being applied to *product*
copy because one line of the standard conflated them.

The alternative is Qt's translation machinery: keep English source strings and
ship a Spanish catalogue. Every user-visible string in the suite already goes
through `qsTr()`, so that door stays open. It is not what this decision
chooses, because there is one locale, one user and no second language asking for
one; adding `lupdate`/`lrelease`, `.ts` sources and a deployment step to reach
the same screen would be infrastructure for a need that does not exist yet.

## Decision

The repository stores two kinds of text and they have different languages.

**Development truth is English**, exactly as before: agent rules, documentation,
roadmaps, plans, decisions, evidence, identifiers, code comments, logs,
diagnostics, script output, test names, protocol tokens and commit subjects.
Nothing in this decision relaxes any of that.

**Product copy is Spanish**: the words a person reads while using Celestina,
Siderita, Magnetita, Grafita or Fluorita. That is the text inside `qsTr()` in
QML, the user-visible strings an adapter hands to a surface, and the
locale-qualified fields of a desktop entry.

The boundary is mechanical, not a matter of taste:

- In QML, only the literal arguments of `qsTr()` are product copy.
- In Rust and C++, product copy lives in a file whose head declares
  `language-contract: product-copy`, and only its string literals are exempt.
  Comments, identifiers and diagnostics in that same file stay English.
- A string that a person never sees is not product copy. State tokens crossing
  the Rust/QML boundary, `eprintln!` diagnostics and error `Display` text read
  by a developer remain English.

Marking a file `product-copy` is a claim that its literals are user-visible. It
does not exempt the file from any other rule, and it may not be used to park
Spanish development prose.

## Consequences

- `scripts/check-language-contract.py` gains the two exemptions above and keeps
  rejecting everything else. Its legacy ratchet keeps falling; rows that existed
  only because a file held Spanish UI strings are removed, because that text is
  no longer debt.
- The reduction it causes cannot be attributed to any source file, which the
  ratchet rule required. That gap is closed the way the architecture ratchet
  already closes its own: a declared migration, needing the scanner and the
  exact evidence field together. Neither half alone moves a row.
- Surfaces already translated to English under the previous rule are translated
  back as their owners are touched. This decision does not open a repo-wide
  rewrite; it removes the reason the next one would go the wrong way.
- Mixed-language screens remain a defect. A surface is Spanish throughout, and
  a half-translated one is finished rather than left.
- If a second locale is ever wanted, `qsTr()` is already in place and the
  answer is a `.ts` catalogue, not a second decision about which language the
  source strings are in.

## Revisit when

Someone other than the author uses these products, a second locale is
requested, or the marker is found holding development prose rather than user
copy — the first two call for real translation catalogues, the third for
tightening the marker rather than removing it.
