# Celestina Rust workspace author validation

This workspace has no independent user-facing surface. Manual checks are owned
by the product that can observe them and do not block this workspace roadmap.

## Validation routing

| Contract | Owning manual queue |
|---|---|
| Siderita file operations, embedded Grafita and embedded Fluorita | [Siderita validation](../siderita/VALIDATION.md) |
| Standalone document interaction and metadata reproduction | [Grafita validation](../grafita/VALIDATION.md) |
| Standalone media, render pacing and playback | [Fluorita validation](../fluorita/VALIDATION.md) |
| KDE Connect, phone hardware, mount and payloads | [Magnetita validation](../magnetita/VALIDATION.md) |

Metadata reproduction is tracked as deferred `VAL-GRA-METADATA` by Grafita, the
product that can observe the save outcome. This workspace does not duplicate
that case or create its own manual lifecycle.
