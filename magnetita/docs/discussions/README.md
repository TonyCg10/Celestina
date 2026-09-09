# Magnetita discussions

Questions raised by
[ADR 0001](../decisions/0001-own-protocol-and-android-app.md). All five were
concluded by the author on 2026-09-04 and applied to the ADR and the
roadmap; the `MAG-P0` spikes now verify the choices and feed the ADR's
*Revisit when* rather than gate them. Rejected arguments are preserved.

| Discussion | Status | Question |
|---|---|---|
| [Transport](2026-09-04-transport-quic-or-tcp.md) | applied | QUIC, or TCP + TLS with a multiplexer? |
| [Pairing](2026-09-04-pairing-qr-or-code.md) | applied | QR code or typed code as the primary pairing path? |
| [Mirror capture](2026-09-04-mirror-capture-path.md) | applied | `MediaProjection` in the own app, or keep `scrcpy` as the engine? |
| [Remote input](2026-09-04-remote-input-on-wayland.md) | applied | `uinput` or the RemoteDesktop portal for trackpad and keyboard? |
| [KDE Connect wire](2026-09-04-kde-connect-compatibility.md) | applied | Retire the KDE Connect wire after parity, or keep both? |
