# ADR 0008: A path crossing the Qt seam is percent-encoded; display text is separate

- **Date:** 2026-08-06
- **Status:** accepted

## Context

The suite's Rust cores handle filenames as bytes, deliberately and with tests
for it. `siderita-core` keys entries on an `EntryId` built from `OsString`,
`siderita-ops` validates a rename by bytes, `fluorita-core` stores catalogue
paths percent-encoded, and the engine addresses files by descriptor. A Linux
filename is a byte string that is not required to be UTF-8, and the domain
layers treat it that way.

Every one of those guarantees is discarded at the Qt boundary. Both
applications publish a path to QML with `to_string_lossy`, which replaces each
invalid byte with U+FFFD, and every verb rebuilds a `PathBuf` from the `QString`
that comes back. The round trip is not reversible: the path that returns names
a file that does not exist. A file whose name is not valid UTF-8 therefore
appears in the listing, with a replacement character where the byte was, and
cannot be opened, renamed, copied, trashed or described — Siderita reports
`SourceMissing`, Fluorita reports that the item is no longer in the library.
The suite audit recorded this as `SID-A2` and `FLU-M1`.

The two projects have the same defect at the same boundary for the same reason,
so they need one rule rather than two repairs. Non-UTF-8 names are not
hypothetical here: files arrive from other systems, from archives, and from
software that wrote a locale-encoded name years ago, and a file manager that
cannot act on what it displays is failing at its primary job.

## Decision

Two distinct representations cross the seam, and the difference between them is
part of every adapter's published contract.

**A path key** is the byte-exact identity of a file:
`celestina_core::percent::encode(percent::path_bytes(path))`. It is opaque
ASCII, it is what every invokable that acts on a file accepts, and it is what a
model publishes for identity, activation, drag and drop. Rust decodes it with
`percent::decode_strict` and `percent::path_from_bytes`; a value that is not
well-formed is refused with a typed error rather than salvaged, because a
malformed key did not come from us.

**Display text** is what a person reads: the existing lossy conversion, with
U+FFFD standing in for bytes no font can show. It is published under its own
property names, it is never an argument to anything, and it never returns to
Rust as a path.

QML consequently stops composing paths. It does not concatenate a directory and
a name, strip a `file://` prefix, or percent-decode: a surface that needs a URL
asks the adapter for one, and a surface that needs a child path asks for it.
Where a path leaves the process entirely — a drag to another application, a URI
on the clipboard, the document portal — the existing spec-specific encoders keep
their own rules, which is why `percent::encode_qt_path` stays exactly what it
is: the thumbnail cache key that must match Qt's own spelling byte for byte.

## Alternatives considered

**Row tokens for everything.** Siderita already publishes a stable
dev+inode+name token beside each row, and the audit's first suggestion was to
route the verbs through it. It is the right mechanism for what it does — keeping
a selection across a rescan — but it only answers for a row that is in the
current view. A breadcrumb segment, a bookmarked place, a path from the command
line, a dropped URI and a file the watcher has already removed are all paths
without a row, and each would need a second mechanism. Two mechanisms for one
question is what the reuse rule exists to prevent.

**Refuse non-UTF-8 names.** Honest, and much smaller: list them, mark them
unusable, explain why. Rejected because the file is genuinely there and the
domain layer can genuinely act on it; the limitation would exist only to spare
the adapter.

**`QByteArray` across the bridge.** Type-correct and lossless, and CXX-Qt can
carry it. Rejected because QML has no useful handling for byte arrays: every
model role, every signal argument and every JavaScript comparison would need a
conversion the percent key gives for free, and the key is already the idiom at
the suite's other boundaries — D-Bus, the portal, the catalogue and the
thumbnail cache all speak it.

## Consequences

The adapters grow an encode at publication and a decode at entry, and their
tests gain a non-UTF-8 fixture that must survive the whole round trip. A QML
surface that used to do string surgery on a path asks the adapter instead, which
is the direction the architecture rules already point: QML presents state and
does not decide domain policy.

Any invokable taking a path is now taking a key, so a caller that passes a raw
path gets a typed refusal instead of silently acting on the wrong file. That is
a visible behaviour change for anything outside the two applications that calls
these objects — nothing does today, and the D-Bus surfaces, which do have
external callers, keep their own documented encodings.

## Revisit when

A third application needs the same seam — the key helper is a two-function
composition living in each adapter today, and a second consumer is the moment
to lift it into `celestina-core` beside the codec it composes rather than let a
third copy appear. Also revisit if CXX-Qt and QML grow a comfortable byte-array
path, which would make the encoding unnecessary rather than merely convenient,
or if a surface is found doing string surgery on a key, which means the adapter
is not yet answering a question the QML has.
