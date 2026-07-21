# CelestinaStyle roadmap

## Now — STYLE-0: relocatable QML module

**Outcome:** an empty staged prefix contains a complete CelestinaStyle module
that clean fixtures can import and operate without the source checkout.

### In

- inventory of the four current QML files and their public surface;
- one coherent Qt-supported module/install topology;
- clean-prefix theme and interactive-component fixtures;
- actionable missing/corrupt module failures;
- relocation, removal and artifact/load baselines;
- concise consumer instructions with no sibling paths.

### Out

- new widget families, theme editor or application-specific controls;
- full API stability/accessibility certification;
- Niri rules, Wayland protocols or consumer repository migrations;
- release publication or Git initialization without separate authorization.

### Work

1. Inventory public QML types, imports, properties, assets and generated files.
2. Configure the current project and choose the smallest complete install
   topology for runtime and QML tooling.
3. Add one theme fixture and one interactive-control fixture using only the
   staged prefix; move the prefix and repeat.
4. Prove missing/corrupt dependency errors and uninstall ownership.
5. Record artifact/load cost and handoff instructions for external consumers.

### Exit

- clean configure/build/install produce an accounted module tree;
- fixtures resolve only the staged module with the source checkout hidden;
- moving the prefix works after declaring its new import root;
- missing artifacts fail visibly instead of falling back to a sibling/global
  copy;
- install/remove touches only owned files and resource baselines are recorded.

## Next — STYLE-1: stable accessible design contract

Define compatibility/deprecation, truthful glass APIs, font/icon fallbacks,
keyboard/focus/AT-SPI and reduced-motion behavior for the current finite set.
Desktop and Siderita then accept the same installed release independently.

## Later

Add components or toolkit-neutral assets only after at least two real consumers
demonstrate reusable demand.

## Non-goals

Do not become an application framework, global configuration daemon, complete
Qt Controls replacement, compositor integration layer or owner of consumer
layouts and domain state.
