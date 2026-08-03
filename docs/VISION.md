# Celestina vision

Celestina is a coherent, native desktop suite whose applications feel like one
system without becoming one monolith. Each tool owns a clear user task, starts
quickly, integrates with the Wayland desktop and shares visual language and
stable domain contracts with the rest of the suite.

## Product promise

The user should be able to browse, edit, view, play, connect and control local
desktop content through small focused surfaces. Moving between an embedded
action and a full application must preserve intent, data and interaction
semantics.

## Engineering direction

- Pure, testable domain and protocol behaviour lives in Rust crates.
- Qt/CXX-Qt adapters project that behaviour into native application state.
- QML composes accessible, keyboard-complete interfaces from one shared visual
  language.
- C++ is a narrow bridge for Qt capabilities that CXX-Qt cannot express safely
  or adequately.
- Cross-process integration uses explicit, backward-compatible contracts rather
  than QML ids or application internals.
- Applications reuse contracts and assets without importing one another's UI
  trees.

## Product boundaries

Celestina is not a generic application framework, a replacement operating
system or a reason to centralize unrelated state. Shared code must represent a
proven common semantic contract. A focused application may remain independent
even when it shares domain logic, style or activation paths with another.

## Quality bar

The suite values predictable local behaviour, loss-free IO, bounded background
work, keyboard and assistive-technology access, truthful errors and restrained
resource use. Compilation is necessary evidence, but the desktop experience is
accepted only at the layer capable of observing it.

