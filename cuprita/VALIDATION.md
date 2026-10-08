# Author validation — Cuprita

This queue contains no implementation work and never blocks `ROADMAP.md`.

## VAL-C — Join and forget a Wi-Fi network

- **Status:** pending
- **Related implementation:** CUP-1-C
- **Requires:** the deployed Cuprita on the real session
- **Procedure:** open Cuprita on Red; join a protected Wi-Fi network that has
  no saved profile with a wrong password first, then with the right one
  (polkit may prompt); disconnect and reconnect it; forget it with
  «Olvidar»; switch Wi-Fi off and on
- **Pass condition:** the wrong password shows a notice, the row reads «Error
  al conectar» and nothing is saved; the right one is typed once and the row
  reads «Conectado»; after forgetting, the row loses «Olvidar»; the Wi-Fi rows
  leave with the switch and return with it
- **Result:** not run
- **Evidence:** none

## VAL-D — Pair and unpair a Bluetooth device

- **Status:** pending
- **Related implementation:** CUP-1-D
- **Requires:** the deployed Cuprita and a device to pair
- **Procedure:** open Cuprita on Bluetooth with Blueman's applet closed;
  press «Buscar», pair the headphones or the phone with its «+» (confirm the
  passkey with «Coincide» or type the PIN in Cuprita's dialog), connect it,
  then choose «Olvidar» in its menu
- **Pass condition:** the pairing dialog is Cuprita's, the device pairs and
  reads «Conectado», and after «Olvidar» it leaves the list (or returns as
  «No emparejado» while searching)
- **Result:** not run
- **Evidence:** none

## VAL-E — Switch audio output and mute an application

- **Status:** pending
- **Related implementation:** CUP-1-E
- **Requires:** the deployed Cuprita on the real session
- **Procedure:** open Cuprita on Audio; choose the HDMI output in the
  «Salida» selector and then the first output again; while two applications
  play, mute one with its row's button and unmute it; in a sound card's
  «Perfil» card choose another profile and then the first one
- **Pass condition:** sound follows the selected output, only the muted
  application goes silent, and the card plays again on its first profile
- **Result:** not run
- **Evidence:** none
