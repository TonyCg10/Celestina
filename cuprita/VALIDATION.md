# Author validation — Cuprita

This queue contains no implementation work and never blocks `ROADMAP.md`.

## VAL-C — Join and forget a Wi-Fi network

- **Status:** pending
- **Related implementation:** CUP-1-C
- **Requires:** the deployed Cuprita on the real session
- **Procedure:** open Cuprita, join a Wi-Fi network that asks for a password,
  check the connection, then forget it
- **Pass condition:** the network joins with the password typed once, shows as
  connected, and disappears from the saved list after forgetting it
- **Result:** not run
- **Evidence:** none

## VAL-D — Pair and unpair a Bluetooth device

- **Status:** pending
- **Related implementation:** CUP-1-D
- **Requires:** the deployed Cuprita and a device to pair
- **Procedure:** search, pair the headphones or the phone through Cuprita's
  dialog, connect, then forget the device
- **Pass condition:** the device pairs with the code entered in Cuprita and
  connects, and forgetting it removes it
- **Result:** not run
- **Evidence:** none

## VAL-E — Switch audio output and mute an application

- **Status:** pending
- **Related implementation:** CUP-1-E
- **Requires:** the deployed Cuprita on the real session
- **Procedure:** switch the output to HDMI and back, then mute one application
- **Pass condition:** sound follows the selected output, and only the muted
  application goes silent
- **Result:** not run
- **Evidence:** none
