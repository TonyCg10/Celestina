# Repository language standard

## Canonical language

English is the only working language stored in this repository. This applies to
agent rules, product and engineering documentation, roadmaps, plans, decisions,
evidence, source identifiers, code comments, logs, diagnostics, script output,
tests, fixtures, commit subjects, and canonical UI copy.

Agents communicate with the author in Spanish unless the author requests
another language. Conversation language never changes repository language.

## Allowed non-English content

Non-English text is permitted only when its purpose requires it:

- explicit localization resources under `i18n/`, `l10n/`, `locale/`, or
  `translations/`;
- locale-qualified desktop entries such as `Name[es]=...`;
- fixtures that test Unicode, locale, encoding, or international input and carry
  `language-contract: allow-non-english` near the top;
- immutable historical records created before this standard, until a dedicated
  translation preserves their meaning and references.

Do not use an exception to write operational instructions, comments, or
diagnostics in another language. A localization resource contains translated
product copy, not development truth.

## Editing rule

New and substantially rewritten files are English throughout. When touching a
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
