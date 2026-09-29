# Imported documents

Grafita has two kinds of document and they never share a save path. This
contract states the second one. The first — a text file whose bytes Grafita
reproduces exactly — is the [content activation](../../../docs/contracts/content-activation.md) and
loss-free save behaviour the editor started with, and nothing here changes it.

## The two kinds

| | Native document | Imported document |
|---|---|---|
| What it is | a text file | a container somebody else wrote: `.docx` today, more as checkpoints land |
| What is edited | the bytes | the text inside the structure |
| Save contract | untouched content reproduces the original bytes | every part the author did not edit is written back as the bytes it already was: styles, images, metadata, parts Grafita does not understand |
| Refusal | a save that cannot reproduce the bytes is refused | a save that cannot write a part back unchanged is refused |
| How it is decided | the bytes are text in a reversible encoding | the bytes are a container holding a part this crate reads |

An imported document is never presented as a native one. A host shows which
container it came out of, and offers no encoding choice: the encoding of a
projection is not something anybody picks.

## What an imported document does not do

It never creates structure. No styles, no tables, no page layout, no fonts, no
paragraphs. It carries text in and out of a structure that already exists.

Two consequences, both refusals rather than approximations:

- Text inserted inside an existing run inherits that run's formatting. That is
  the whole formatting rule.
- Adding or removing a paragraph is refused, because a paragraph is structure.
  The refusal happens before anything is written and the file on disk is
  untouched.

## How the promise is kept

The container is parsed and rebuilt rather than repacked. Each member the
author did not edit is copied as the exact bytes it occupies — local header,
extra field and compressed data alike, in the order it appeared — so nothing is
recompressed to be written back unchanged. Only a replaced member is written
afresh, keeping the name, timestamp and compression method it had.

The text part is located by byte offset rather than parsed into a tree. A tree
would have to be serialised again, and no serialiser reproduces someone else's
whitespace, attribute order and namespace prefixes; offsets leave every byte
between the text spans exactly where it was.

Writing goes through the same save the native document uses: a sibling
temporary, reproduced metadata, synchronisation, identity revalidation and an
atomic rename. An imported document is not a second way to write a file, only a
second way to produce its bytes.

## What an imported document refuses to unpack

The ceiling a host puts on a document (`Limits::max_bytes`, 64 MiB by default)
is a ceiling on the document, not only on the file. A container's content
counts against it as it is unpacked: every ZIP member a document reads and
every filter of a PDF stream draw on the same budget, and each decoder is read
only one byte past what is left. A container that would pass the ceiling is
refused as too large before the excess is held. A size a header declares bounds
the read and is checked against the result, but it never sizes an allocation.

A PDF's structure is read with the same suspicion, because Grafita and
Siderita's preview both run this reader in-process under `panic = "abort"`.
Objects nested past 64 levels, an offset, length or field width the file
cannot hold, and a count its own data cannot contain are refusals; a page or
field tree that names a node twice is walked once. The hostile cases, each a
refusal that must arrive promptly, are the tests in
`celestina-rs/crates/grafita-core/tests/hostile.rs`.

## What every container checkpoint must prove

Open a real document, save it, and compare the result with the input. A
checkpoint whose formats do not all pass that round trip does not close. The
one thing this contract cannot promise is the author's judgement: it guarantees
that nothing they did not touch has changed, not that what they typed is what
they meant.
