# Repository language standard

## Two kinds of text

This repository stores development truth and product copy, and they do not
share a language. The split is settled in
[ADR 0007](../decisions/0007-spanish-product-copy.md).

**Development truth is English.** This applies to agent rules, product and
engineering documentation, roadmaps, plans, decisions, evidence, source
identifiers, code comments, logs, diagnostics, script output, tests, fixtures,
protocol tokens, and commit subjects.

**Product copy is Spanish.** This is the text a person reads while using the
products: the literal arguments of `qsTr()` in QML, the user-visible strings an
adapter hands to a surface, and locale-qualified desktop-entry fields. A
surface is Spanish throughout; a half-translated screen is a defect, not a
milestone.

A string nobody sees is not product copy. State tokens crossing the Rust/QML
boundary, `eprintln!` diagnostics and error text read by a developer are
development truth and stay English.

Agents communicate with the author in Spanish unless the author requests
another language. Conversation language never changes the language of
development truth.

## Allowed non-English content

Non-English text is permitted only when its purpose requires it:

- product copy, declared the one mechanical way the guard can check it: the
  literal arguments of `qsTr()` in QML, and the string literals of a Rust or
  C++ file whose head declares `language-contract: product-copy`;
- explicit localization resources under `i18n/`, `l10n/`, `locale/`, or
  `translations/`;
- locale-qualified desktop entries such as `Name[es]=...`;
- fixtures that test Unicode, locale, encoding, or international input and carry
  `language-contract: allow-non-english` near the top;
- immutable historical records created before this standard, until a dedicated
  translation preserves their meaning and references.

The `product-copy` marker exempts string literals and nothing else: comments,
identifiers and diagnostics in a marked file stay English. Marking a file is a
claim that its literals are user-visible, and it may not be used to park
development prose.

Do not use an exception to write operational instructions, comments, or
diagnostics in another language. A localization resource contains translated
product copy, not development truth.

## Editing rule

New and substantially rewritten files are English throughout, product copy
aside. When touching a
legacy mixed-language file, translate the edited coherent section and never add
new non-English development text. A dedicated language migration may translate
whole files mechanically only after checking identifiers, commands, code spans,
links, and behavior-sensitive strings.

Translation must preserve meaning. It must not silently reinterpret a settled
decision, change product behavior, alter a protocol token, or rewrite immutable
delivery evidence. Historical material may receive an English successor or an
explicit translated copy when byte identity matters.

## Enforcement

`scripts/check-language-contract.py` performs two checks:

1. canonical rules and current templates must contain no Spanish-language
   development prose;
2. the repository-wide legacy baseline may only decrease and cannot accept a
   new path.

The baseline is migration debt, not permission to keep mixing languages. It is
lowered whenever a file is translated and removed when no violation remains.
