# Cuprita

Celestina's control centre for the machine's connections: network, Bluetooth
and audio in one window. It replaces nm-applet, Blueman and pavucontrol.

## User contract

- Red: Ethernet and Wi-Fi state, joining and forgetting networks, airplane mode
  and the VPNs NetworkManager already defines.
- Bluetooth: the adapter, discovery, pairing with Cuprita as the pairing agent,
  connecting and forgetting devices.
- Audio: default output and input, volume and mute per device and per
  application, the card profile.
- Cuprita is a window opened from the launcher or a key binding. It has no
  tray icon, no indicator and no daemon.

The sections arrive in the CUP-1 units: Red runs on NetworkManager and
Bluetooth on BlueZ; Audio on PipeWire through WirePlumber. See the
[design](../docs/superpowers/specs/2026-10-08-cuprita-design.md).

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/cuprita-core` | Models, the three backend traits, fakes and pure logic; no Qt. The NetworkManager client (`nm`, feature `nm`) and the BlueZ client with the pairing agent (`bluez`, feature `bluez`) are its D-Bus modules; the audio client (`wpctl`, feature `wpctl`) drives WirePlumber's `wpctl` and PipeWire's `pw-cli` and `pw-mon` |
| `src/` | The CXX-Qt controllers and models, worker threads, the real clients, the adapter of the shared single-instance activation (`celestina_core::activation`) |
| `qml/` | The window, the pill navigation strip and the pages |
| `../celestina-style` | Canonical visual tokens, controls and assets, linked |
| `org.celestina.Cuprita.desktop` | Desktop discovery |

The backends it talks to are NetworkManager and BlueZ over D-Bus, and
PipeWire for audio through WirePlumber's command line.

## Build and use

Cuprita needs Rust and a Qt 6 development environment visible to CXX-Qt. At
run time it needs NetworkManager on the system bus for the Red section and
BlueZ (`bluetoothd`, `org.bluez`) for the Bluetooth section, and `pipewire`
and `wireplumber` in the user session for the Audio section (`wpctl`,
`pw-cli`, and `pw-mon` when present for prompt updates); without one, its
section reports itself unavailable; joining or forgetting a network may ask
polkit for authorisation. Cuprita registers itself as BlueZ's default pairing
agent while it runs. `cargo run -p cuprita-core --example snapshot` (in
`celestina-rs/`) prints what NetworkManager, BlueZ and PipeWire report on this
machine. The canonical production workflow is:

```sh
scripts/build-production.sh
scripts/verify-production.sh
scripts/status-production.sh
scripts/complete-production.sh # canonical agent completion; updates ~/.local
```

After completion, launch `cuprita` or use the desktop entry.

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Local agent delta](AGENTS.md)
