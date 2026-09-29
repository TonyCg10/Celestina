# Magnetita Android author validation

This manual lane requires the real phone. It does not contain implementation
and does not block [ROADMAP.md](ROADMAP.md). The first pairing on the S25U
is `VAL-MAG-11` in Magnetita's [VALIDATION.md](../magnetita/VALIDATION.md);
entries specific to the application are added here as its screens land.

## VAL-AND-1 — Pairing consent and the desktop's input gates on the phone

- **Status:** pending
- **Related implementation:** `AND-6-D`
- **Requires:** the release build with `AND-6-D` on the S25U, the deployed
  daemon, the desktop's pairing QR, "Control de pantalla" allowed, a folder
  shared in the "Archivos" row and the phone's mount on the desktop
- **Procedure:** open a `magnetita://pair` link naming the desktop from
  another app (for example `adb shell am start -d '<link>'` or a note);
  press Home while the consent shows and reopen the app; open it again and
  leave the consent unanswered for two minutes; open it again and, while it
  shows, open a second link naming another id; dismiss the message; open
  the first link again and confirm; scan the desktop's QR and cancel, then scan and confirm; open a
  link naming `8.8.8.8:1760` and one naming the phone's own Wi-Fi address;
  with no mirror running, press keys and Back/Home/Recents on the
  desktop's mirror controls; run `rmdir` on a non-empty folder of the
  mount, then on an empty one
- **Pass condition:** the link opens the consent screen with the desktop's
  id, fingerprint and address and pairs only on "Emparejar"; Home drops the
  offer; after two minutes the consent turns into the expiry message; the
  second link turns the consent into the message that both links were
  discarded, which stays until dismissed; the scan shows the same screen; the public and own addresses show
  their refusals; keys and global actions do nothing without a stream; the
  non-empty folder and its files remain on the phone (the desktop reports
  an error, `ENOTEMPTY` once `MAG-D1-D` lands) and the empty one is removed
- **Result:** not run
- **Evidence:** pending; the automated half is
  [the pairing consent record](docs/evidence/2026-09-26-pairing-consent.md)
