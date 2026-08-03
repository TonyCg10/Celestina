# ADR 0005: Keep Qt bridge crates as bounded reusable seams

- **Date:** 2026-08-03
- **Status:** accepted

## Context

Most of `celestina-rs` is domain, protocol and testable IO that must remain
independent of Qt. The existing `fluorita-qt` crate contains the narrow C++/Qt
Quick render seam that CXX-Qt cannot express cleanly and shares it between
hosts. By contrast, `siderita-qt` is safe Rust over pure workspace crates: its
historical name describes view-facing contracts, not a Qt dependency. Treating
the whole workspace as Qt-free contradicts the first case; treating every
`*-qt` name as an exception would erase the boundary.

## Decision

Core, ops, engine, network and pure view-contract crates in `celestina-rs`
remain free of Qt and QML regardless of their historical name. `fluorita-qt`,
or a future exception approved through a new ADR, may contain only a stable
C++/Qt/FFI seam when the concrete CXX-Qt limitation is documented and the seam
is reusable or must share Rust-owned lifecycle with its domain producer.

Such a crate contains no QML, screen composition, application workflow or
extractable domain policy. Application-specific QObject state and marshaling
stay in that application's `src/`; manual C++ remains the smallest possible
adapter and names the missing CXX-Qt capability.

## Consequences

- The architecture describes the existing bridge crates without pretending
  they are pure domain.
- Qt dependencies cannot leak into core crates through convenience imports.
- A new Qt-bearing crate needs an explicit consumer, documented CXX-Qt gap, a
  new accepted ADR and architecture tests in the same implementation unit; its
  name alone never grants the exception.
- Hosts retain their own UI state and QML composition even when they share the
  underlying seam.

## Revisit when

CXX-Qt can express the relevant view/render integration directly, a bridge
gains application workflow, or a second host demonstrates that the current API
is not actually the shared intersection. In those cases remove, relocate or
supersede the seam through a new ADR.
