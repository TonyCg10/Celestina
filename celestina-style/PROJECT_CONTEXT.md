# CelestinaStyle

> Current truth verified 2026-07-20. Near-term work lives in
> [ROADMAP.md](ROADMAP.md).

## Purpose

CelestinaStyle is the independent Qt Quick/QML visual library for Celestina
applications. It owns reusable semantic tokens and generic controls, not app
state, Niri integration, dotfiles or application workflows.

Desktop and Siderita are external consumers with independent roadmaps and
release timing.

## Current state

- CelestinaTheme.qml defines palette, typography, spacing, radius, motion and
  glass-related tokens.
- CelestinaButton.qml, CelestinaGlassPanel.qml and
  CelestinaContextMenu.qml are the current finite component set.
- qmldir declares CelestinaStyle 1.0; CMake declares a Qt 6.5 QML module and
  manual install rules.
- There are no tests, clean-prefix fixtures, package metadata, changelog,
  release automation or proven compatibility policy.
- This directory is not a Git repository.
- Desktop consumes the source sibling directly; Siderita creates a build-tree
  symlink. Neither is a valid release contract.

## Intended commands

~~~sh
cmake -S . -B build
cmake --build build
cmake --install build --prefix /path/to/empty-prefix
~~~

These commands are not accepted until clean-prefix fixtures import and run the
installed module without access to the source checkout.

## Decisions and constraints

| Decision | Reason |
|---|---|
| Style is an independent shared-library project | Consumers keep their own implementation and release authority |
| Delivery is an installed/versioned QML module | Source siblings and build symlinks are not public interfaces |
| Style owns reusable presentation only | Domain actions, models and workflows belong to each app |
| Backdrop blur is outside Style | Style may provide tint/glow/tokens; compositor effects belong to the surface owner |
| Accessibility behavior is part of a public control | Screenshots and pointer clicks are insufficient |
| Component growth follows proven reuse | Widget count is not product progress |

## Current blockers

1. The exact installed QML/plugin/type-metadata topology is unproven.
2. The literal 1.0 identity has no compatibility/deprecation promise.
3. Glass APIs and comments incorrectly imply compositor backdrop blur.
4. Inter, JetBrains Mono and icon resources lack packaging/fallback contracts.
5. Keyboard, focus, AT-SPI, reduced motion and high-contrast behavior are
   untested.
6. Repository/release ownership has not been established.

