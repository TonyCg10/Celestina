"""Generate grafita-core's encoding tables from CPython's codecs.

`single` writes `src/encoding/tables.rs`, `multi` writes
`src/encoding/multibyte.rs`. Both refuse to emit a table whose mapping would
not survive the crate's reversibility rules, so a table that reaches the source
tree has already been checked rather than reviewed.
"""
import sys

# (variant, codec, label). Order is the catalogue order.
SPEC = [
    ("Windows1250", "cp1250", "windows-1250"),
    ("Windows1251", "cp1251", "windows-1251"),
    ("Windows1252", "cp1252", "windows-1252"),
    ("Windows1253", "cp1253", "windows-1253"),
    ("Windows1254", "cp1254", "windows-1254"),
    ("Windows1255", "cp1255", "windows-1255"),
    ("Windows1256", "cp1256", "windows-1256"),
    ("Windows1257", "cp1257", "windows-1257"),
    ("Windows1258", "cp1258", "windows-1258"),
    ("Iso8859_1", "latin_1", "ISO-8859-1"),
    ("Iso8859_2", "iso8859_2", "ISO-8859-2"),
    ("Iso8859_3", "iso8859_3", "ISO-8859-3"),
    ("Iso8859_4", "iso8859_4", "ISO-8859-4"),
    ("Iso8859_5", "iso8859_5", "ISO-8859-5"),
    ("Iso8859_6", "iso8859_6", "ISO-8859-6"),
    ("Iso8859_7", "iso8859_7", "ISO-8859-7"),
    ("Iso8859_8", "iso8859_8", "ISO-8859-8"),
    ("Iso8859_9", "iso8859_9", "ISO-8859-9"),
    ("Iso8859_10", "iso8859_10", "ISO-8859-10"),
    ("Iso8859_11", "iso8859_11", "ISO-8859-11"),
    ("Iso8859_13", "iso8859_13", "ISO-8859-13"),
    ("Iso8859_14", "iso8859_14", "ISO-8859-14"),
    ("Iso8859_15", "iso8859_15", "ISO-8859-15"),
    ("Iso8859_16", "iso8859_16", "ISO-8859-16"),
    ("Koi8R", "koi8_r", "KOI8-R"),
    ("Koi8U", "koi8_u", "KOI8-U"),
    ("Cp437", "cp437", "IBM-437"),
    ("Cp850", "cp850", "IBM-850"),
    ("Cp866", "cp866", "IBM-866"),
    ("MacRoman", "mac_roman", "Macintosh"),
]

UNASSIGNED = "'\\0'"


def table_for(codec):
    """The 128 high-half code points, 0 where the standard assigns none."""
    for byte in range(0x80):
        if bytes([byte]).decode(codec) != chr(byte):
            raise SystemExit(f"{codec}: low half is not ASCII at {byte:#04x}")
    points = []
    for byte in range(0x80, 0x100):
        try:
            decoded = bytes([byte]).decode(codec)
        except UnicodeDecodeError:
            # A byte the standard leaves unassigned. The 0x80-0x9F range is the
            # exception: those positions carry the C1 control of the same value,
            # which is what the web platform decodes them as.
            decoded = chr(byte) if byte <= 0x9F else None
        if decoded is None:
            points.append(None)
            continue
        if len(decoded) != 1:
            raise SystemExit(f"{codec}: byte {byte:#04x} is not one character")
        point = ord(decoded)
        if 0xD800 <= point <= 0xDFFF:
            raise SystemExit(f"{codec}: byte {byte:#04x} is a surrogate")
        points.append(point)
    seen = {}
    for offset, point in enumerate(points):
        if point is None:
            continue
        if point in seen:
            raise SystemExit(
                f"{codec}: bytes {seen[point]:#04x} and {0x80 + offset:#04x} share {point:#06x}"
            )
        seen[point] = 0x80 + offset
    return points


def rust_char(point):
    if point is None:
        return "'\\0'"
    return f"'\\u{{{point:04X}}}'"


def rust_rows(points):
    rows = []
    for start in range(0, 128, 6):
        row = ", ".join(rust_char(point) for point in points[start : start + 6])
        rows.append(f"    {row},")
    return "\n".join(rows)


MULTI_SPEC = [
    # (variant, codec, label, exact codec note)
    ("ShiftJis", "cp932", "Shift-JIS", "CP932, the Windows superset in actual use"),
    ("Gbk", "gbk", "GBK", "GBK, which contains GB2312"),
    ("EucKr", "cp949", "EUC-KR", "CP949, the Windows superset of EUC-KR"),
    ("Big5", "big5", "Big5", "Big5"),
]


def multi_tables(codec):
    """Single bytes and two-byte pairs, as code points.

    No catalogued multi-byte encoding uses a byte both as a character and as a
    lead, which the caller checks; that is what lets decoding decide on the
    first byte alone.
    """
    singles = []
    for byte in range(256):
        try:
            decoded = bytes([byte]).decode(codec)
        except UnicodeDecodeError:
            decoded = None
        if decoded is not None and len(decoded) == 1:
            point = ord(decoded)
            if point > 0xFFFF:
                raise SystemExit(f"{codec}: byte {byte:#04x} is outside the BMP")
            singles.append(point)
        else:
            singles.append(0)

    pairs = []
    for lead in range(0x80, 0x100):
        if singles[lead]:
            continue
        for trail in range(256):
            try:
                decoded = bytes([lead, trail]).decode(codec)
            except UnicodeDecodeError:
                continue
            if len(decoded) != 1:
                continue
            point = ord(decoded)
            if point > 0xFFFF:
                raise SystemExit(f"{codec}: {lead:#04x} {trail:#04x} is outside the BMP")
            if 0xD800 <= point <= 0xDFFF:
                raise SystemExit(f"{codec}: {lead:#04x} {trail:#04x} is a surrogate")
            pairs.append(((lead << 8) | trail, point))

    leads = {key >> 8 for key, _point in pairs}
    clash = sorted(lead for lead in leads if singles[lead])
    if clash:
        raise SystemExit(f"{codec}: bytes are both characters and leads: {clash}")
    pairs.sort()
    return singles, pairs


def write_multi():
    out = [
        "//! The multi-byte tables, generated from the standards' own mappings.",
        "//!",
        "//! Written by `tools/generate-encoding-tables.py multi`; do not edit by",
        "//! hand. Unlike the single-byte tables these are not bijective: a",
        "//! character can have two encodings and a byte pair can have no",
        "//! character, so nothing here is safe on its own. What makes them safe",
        "//! is `open_with`, which re-encodes what it decoded and refuses the file",
        "//! unless the bytes come back identical.",
        "//!",
        "//! A single-byte entry of zero means the byte is not a character on its",
        "//! own; it is then a lead byte, or nothing at all. No catalogued",
        "//! encoding uses a byte as both, which the generator checks.",
        "",
    ]
    names = []
    for variant, codec, label, note in MULTI_SPEC:
        singles, pairs = multi_tables(codec)
        const = variant.upper()
        names.append((variant, const, label))
        out.append(f"/// `{label}` single bytes, from CPython's `{codec}` codec ({note}).")
        out.append(f"pub(super) static {const}_SINGLES: [u16; 256] = [")
        for start in range(0, 256, 8):
            row = ", ".join(f"0x{point:04X}" for point in singles[start : start + 8])
            out.append(f"    {row},")
        out.append("];")
        out.append("")
        out.append(f"/// `{label}` byte pairs, sorted by the pair read as a big-endian `u16`.")
        out.append(f"pub(super) static {const}_PAIRS: [(u16, u16); {len(pairs)}] = [")
        for start in range(0, len(pairs), 4):
            row = ", ".join(
                f"(0x{key:04X}, 0x{point:04X})" for key, point in pairs[start : start + 4]
            )
            out.append(f"    {row},")
        out.append("];")
        out.append("")
    out.append("/// The single-byte half of an encoding. Total by construction.")
    out.append("pub(super) fn singles_of(encoding: MultiByte) -> &'static [u16; 256] {")
    out.append("    match encoding {")
    for variant, const, _label in names:
        out.append(f"        MultiByte::{variant} => &{const}_SINGLES,")
    out.append("    }")
    out.append("}")
    out.append("")
    out.append("/// The two-byte half of an encoding, on the same total match.")
    out.append("pub(super) fn pairs_of(encoding: MultiByte) -> &'static [(u16, u16)] {")
    out.append("    match encoding {")
    for variant, const, _label in names:
        out.append(f"        MultiByte::{variant} => &{const}_PAIRS,")
    out.append("    }")
    out.append("}")
    out.append("")
    out.append("/// The name an encoding is known by.")
    out.append("pub(super) fn label_of(encoding: MultiByte) -> &'static str {")
    out.append("    match encoding {")
    for variant, _const, label in names:
        out.append(f'        MultiByte::{variant} => "{label}",')
    out.append("    }")
    out.append("}")
    out.append("")
    out.append("/// Every catalogued multi-byte encoding, in the order it is listed.")
    out.append(f"pub(super) static CATALOGUE: [MultiByte; {len(names)}] = [")
    for variant, _const, _label in names:
        out.append(f"    MultiByte::{variant},")
    out.append("];")
    out.append("")
    out.append("/// A multi-byte encoding the author can name.")
    out.append("///")
    out.append("/// Never concluded from the bytes, and never trusted from the table")
    out.append("/// alone: a document opened as one of these is verified byte for byte")
    out.append("/// before it becomes editable.")
    out.append("#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]")
    out.append("pub enum MultiByte {")
    for variant, _const, label in names:
        out.append(f"    /// `{label}`.")
        out.append(f"    {variant},")
    out.append("}")
    out.append("")
    sys.stdout.write("\n".join(out))


def main():
    out = [
        "//! The single-byte tables, generated from the standards' own mappings.",
        "//!",
        "//! Written by `tools/generate-encoding-tables.py single`; do not edit by hand.",
        "//! Each table holds the 128 characters bytes `0x80..=0xFF` decode to,",
        "//! because every catalogued encoding maps `0x00..=0x7F` to ASCII. A NUL",
        "//! marks a byte the standard leaves unassigned: it has no character at",
        "//! all, so a file containing it is refused rather than decoded into",
        "//! something that would not write back.",
        "",
        "/// A byte the standard assigns no character to.",
        "pub(super) const UNASSIGNED: char = '\\0';",
        "",
    ]
    names = []
    for variant, codec, label in SPEC:
        points = table_for(codec)
        const = variant.upper().replace("__", "_")
        names.append((variant, const, label))
        out.append(f"/// `{label}`, from CPython's `{codec}` codec.")
        out.append(f"pub(super) static {const}: [char; 128] = [")
        out.append(rust_rows(points))
        out.append("];")
        out.append("")
    out.append("/// The table an encoding decodes with. Total by construction: the")
    out.append("/// generator writes this arm and the enum variant together.")
    out.append("pub(super) fn table_of(encoding: SingleByte) -> &'static [char; 128] {")
    out.append("    match encoding {")
    for variant, const, _label in names:
        out.append(f"        SingleByte::{variant} => &{const},")
    out.append("    }")
    out.append("}")
    out.append("")
    out.append("/// The name an encoding is known by, on the same total match.")
    out.append("pub(super) fn label_of(encoding: SingleByte) -> &'static str {")
    out.append("    match encoding {")
    for variant, _const, label in names:
        out.append(f'        SingleByte::{variant} => "{label}",')
    out.append("    }")
    out.append("}")
    out.append("")
    out.append("/// Every catalogued single-byte encoding, in the order it is listed.")
    out.append(f"pub(super) static CATALOGUE: [SingleByte; {len(names)}] = [")
    for variant, _const, _label in names:
        out.append(f"    SingleByte::{variant},")
    out.append("];")
    out.append("")
    out.append("/// A single-byte encoding the author can name.")
    out.append("///")
    out.append("/// These are never concluded from the bytes. Nothing in a file says which")
    out.append("/// of them it is, so one arrives only when a caller names it.")
    out.append("#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]")
    out.append("pub enum SingleByte {")
    for variant, _const, label in names:
        out.append(f"    /// `{label}`.")
        out.append(f"    {variant},")
    out.append("}")
    out.append("")
    sys.stdout.write("\n".join(out))


if len(sys.argv) > 1 and sys.argv[1] == "multi":
    write_multi()
else:
    main()
