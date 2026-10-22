# ADR 0018: Connectivity state is read, and the shell sends only a closed list of actions

- **Date:** 2026-10-17
- **Status:** proposed

## Context

The shell shows the state of NetworkManager, BlueZ, UPower and
power-profiles-daemon. Editing that state belongs to `nm-applet`, `blueman`
and the power tools. The pieces need a few actions (join a saved network,
switch Wi-Fi, connect a paired device, change the profile), and the author's
rule is that a surface never offers an action the shell cannot check.

The status stays `proposed` until the author sets it: this directory's
convention names no case where a session accepts a record, and the session
that wrote it did not ask the author to accept it.

## Decision

- The shell reads NetworkManager and BlueZ through `cuprita-core` (features
  `nm` and `bluez` only), and UPower and power-profiles-daemon through its own
  `zbus` power worker (`connectivity/power.rs`: `Properties.GetAll`, sent with
  `NoAutoStart`, so reading never asks the bus to start a service). It
  publishes bounded, shell-owned state types. Pieces never see cuprita's
  model.
- The shell sends only the actions of `celestina_shell::connectivity::Action`:
  `Refresh`, `SetWifiEnabled`, `JoinSaved`, `Leave`, `SetBluetoothPowered`,
  `ConnectDevice`, `DisconnectDevice`, `SetProfile`, `CycleProfile` and
  `OpenEditor`. Each is validated against the last confirmed inventory before
  any call, and a refusal is typed (`UnknownId`, `KindNotAllowed`,
  `NeedsSecretClass`, `NotActive`, `NotPaired`, `NoSuchProfile` and others).
- Creating, editing, forgetting or exporting a profile, airplane mode, VPN
  activation, scans and discovery, pairing, trusting, removing devices,
  registering a BlueZ agent and holding a power profile have no variant.
- A request is confirmed by a later observation, not by its reply. A row
  stays pending for at most 20 s, then shows the failure by kind.

## Consequences

- `cuprita-core` has no Wi-Fi hardware kill switch: `wifi_hardware_blocked`
  is always false and the `HardwareBlocked` refusal never fires. A NetworkManager
  refusal to switch Wi-Fi shows as a timeout.
- VPN rows, enterprise and WEP networks are shown but not clickable; their
  row says "configured in nm-applet".
- BlueZ's device icon is coarse in cuprita, so every input device shows the
  pointer glyph.
- The lanes and the power worker read on their own threads; a blocked client
  call is left behind at shutdown after one shared 2 s bound for the join of
  all workers.

## Revisit when

The author asks for an action outside the list (airplane mode, VPN, pairing),
which needs its own decision and its own validation, or cuprita gains a
hardware-block reading.
