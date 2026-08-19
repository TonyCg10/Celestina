# Evidence: 2026-08-19 what a folder shows

- **Date:** 2026-08-19
- **Scope:** `SID-A2-B`; plan
  [after-the-archive-verbs](../plans/archive/2026-08-19-after-the-archive-verbs.md);
  domain evidence
  [volume Trash and embedded images](2026-08-19-volume-trash-and-embedded-images.md)
- **Environment:** Arch-derived Linux, Qt 6.11.1. The author ran the deployed
  binary on the live session; everything else is offscreen renders
- **Artifact:** `siderita/target/release/siderita`, built, verified and deployed
  by `scripts/complete-production.sh`

## What changed

A folder now tells its files apart: a family per extension, a page per language
where the icon family draws one (Rust, TypeScript, TSX, Vue, HTML, CSS, SQL,
SVG, and the document kinds), and a tint where it does not (Python, Go, C, C++,
Java, JSON, YAML, TOML). A file that carries its own picture shows it, through
the thumbnail provider that already caches and decodes off the UI thread.

The sidebar marks one row instead of two, remembers which sections are folded,
and the unsaved-changes question has a surface of its own. The content box takes
a corner concentric with the window, and changing route settles the new listing
with a fade and a 1.5% scale instead of moving it eight pixels up.

## Procedure

| Check | Result |
|---|---|
| `cargo test` | 115 tests pass |
| `scripts/qml-tests.sh` | 71 tests pass |
| `scripts/smoke.sh` | binary alive 8 s, no QML errors |
| `scripts/complete-production.sh` | built, verified and deployed the same bytes |
| Repository guards | language, architecture, style, qmllint and contrast contracts pass |
| Offscreen renders | icon families and tints; the guard dialog with a short and a wrapped name; the reveal at three points; concentric vs copied vs card radius |
| Live run over a folder holding a `.exe` and an `.epub` | both thumbnails generated and cached in `~/.cache/thumbnails/large/` |

## Result

Everything above passes and the deployed binary is the one those bytes were
verified as.

## Limits

- Colour is applied only where several languages share one page. Tinting
  everything would leave colour meaning nothing.
- Phosphor draws no page for Python, C, C++, Markdown or JSON. Those keep the
  code page and their tint; borrowing a glyph from a second family would put a
  foreign shape in a set that reads as one.
- `VAL-SID-08` — the author's own pass on the live session, including a delete
  and a restore on a removable volume.
