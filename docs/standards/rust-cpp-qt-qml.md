# Rust, C++, Qt, and QML engineering standard

## Required expertise

Agents work as experts in Rust, modern C++, Qt 6, QML, and CXX-Qt. A valid
change accounts for ownership, lifetimes, pinning, thread affinity, QObject
lifecycle, signals/models, FFI, QML registration, build integration, minimum Qt
version, error paths, and every consumer. Inspect primary documentation when an
API or version is uncertain.

## Rust and CXX-Qt

- Modules own one named, testable responsibility.
- Bridges define the FFI contract and delegate; business logic does not live in
  `#[cxx_qt::bridge]`.
- Expose narrow typed values. Do not leak generic maps or internal IDs when a
  stable type exists.
- `unsafe` requires prior approval, isolation, a documented invariant, and a
  safe boundary.
- Production paths do not use `unwrap`, `expect`, or `panic!` except for a
  locally demonstrated invariant. Never add `#[allow]` to hide debt.
- Errors preserve context and source. Background work is bounded, cancellable,
  and joined deterministically.
- Every domain behavior arrives with tests. Property/model semantics also need
  adapter-level tests when Rust tests alone cannot observe them.

## Manual C++

- Manual C++ exists only for a named CXX-Qt gap, such as an unsupported Qt API,
  rendering seam, platform integration, or ownership shape.
- Use RAII and explicit ownership. QObjects are created, used, and destroyed on
  the correct thread.
- Never retain a borrowed Rust reference beyond the call that provided it.
- Blocking D-Bus, filesystem, process, and network work never runs on the GUI
  thread.
- Convert errors at the boundary without discarding context or terminating the
  process.
- C++ adapts; it does not duplicate pure Rust policy.

## Qt models, signals, and threads

- A model mutation uses the matching begin/end protocol and consistent roles.
- Signals describe confirmed state transitions. Requested state is not exposed
  as confirmed before its producer acknowledges it.
- Cross-thread delivery uses queued Qt mechanisms or an explicit safe seam.
- Async results carry generation/revision/identity and stale results are
  discarded.
- Bursty sources are bounded or coalesced and shutdown prevents late callbacks.
- D-Bus failures degrade best-effort functionality instead of freezing or
  crashing the application.

## QML

- Hosts coordinate; coherent regions become components with typed properties,
  required inputs, narrow functions, and signals.
- A child does not reach parent IDs. Rename ambiguous properties instead of
  writing `x: x`.
- Delegates contain presentation and local interaction, not IO or domain rules.
- Register every QML file in `build.rs`, CMake, QRC, or `qmldir` as required.
- Prefer shared controls and semantic tokens. Do not hard-code color, control
  anatomy, opacity, or motion.
- Avoid `property var` when a narrower contract exists; document the expected
  interface when it is unavoidable.

## Accessibility and motion

- Every action works by keyboard and assistive technology, not only pointer.
- Use Qt controls for semantics when suitable. Custom `Item`/`MouseArea`
  controls expose equivalent Accessible role, name, state, and action.
- Visible focus uses `visualFocus`; dialogs contain focus, disable lower
  actions, and restore focus on close.
- Lists, tabs, selection, progress, errors, and toggles expose semantic state.
- New or changed motion honors `CelestinaTheme.reducedMotion`; spatial or scale
  motion becomes instant or disabled.
- Contrast reaches 4.5:1 for normal text and 3:1 for large text in every state.

## Toolchain and dependencies

The declared minimum Qt version covers the newest API used or supplies a
fallback. Every dependency has a local justification. Heavy runtimes,
frameworks, or duplicate async executors require author approval and measured
need.
