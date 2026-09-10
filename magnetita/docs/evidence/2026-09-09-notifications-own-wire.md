# Notifications with actions and replies on the own wire — MAG-P4-B

- **Date:** 2026-09-09
- **Scope:** `MAG-P4-B` of
  [`../plans/archive/2026-09-09-daily-set.md`](../plans/archive/2026-09-09-daily-set.md):
  `celestina-rs/crates/magnetitad/src/link_wire/{mod,notifications}.rs`,
  `notify.rs`, `devices.rs`, `link_commands.rs`,
  `celestina-rs/crates/magnetita-mobile/src/{phone,mobile}.rs`,
  `celestina-rs/crates/magnetita-peer/`, this record
- **Environment:** the workspace's loopback tests; the daemon deployed
  through `magnetita/scripts/complete-production.sh`; the S25U on the own
  wire with the application of `AND-2-B`
- **Artifact:** `magnetitad`, deployed

## Design

- **Phone to desktop:** `NotificationPosted` passes the notifications
  setting and goes to the session's `org.freedesktop.Notifications`
  through a `NotificationServer` trait: the D-Bus one in the daemon, a
  recorder in tests, none without a bus. `notify::post_with` offers the
  phone's buttons as actions keyed by their index and, when the phone
  accepts a reply, KDE's `x-kde-reply-placeholder-text` hint. The bridge
  keeps (device, key) → server id and the reverse, bounded per device,
  so a later post replaces, `NotificationDismissed` closes, and a session
  that ends forgets its keys.
- **Desktop to phone:** three threads listen to the server's
  `ActionInvoked`, `NotificationClosed` and `NotificationReplied`; the
  bridge turns them into `Command::NotificationAction`,
  `NotificationDismiss` (only when the person closed it, reason 2) and
  `NotificationReply` on the device's queue, and the session sends the
  matching envelopes. `Devices1` gains `NotificationAction`,
  `ReplyNotification` and `DismissNotification` for the desktop app.
- **Core:** `send_notification`, `send_notification_gone`; `Event` carries
  the decoded key, action index and reply text; the peer sends
  `--notify APP|TITLE|BODY`.
- The KDE Connect wire ignores the three new commands.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetitad -p magnetita-mobile -p magnetita-peer
cd .. && magnetita/scripts/complete-production.sh
adb shell cmd notification allow_listener org.celestina.magnetita/.notifications.PhoneNotifications
adb shell cmd notification post -S bigtext -t 'Prueba de Magnetita' magtest2 'Hola desde el S25U'
busctl --user call … RecentLog
busctl --user call … DismissNotification ss 4eb6f9984054dd25 <key>
```

## Result

- **Exit:** 0. Daemon: 91 tests. The bridge's tests pin replace, close,
  the bound and the mapping of the three server signals; the loopback
  test posts from a phone endpoint, sees the recorder's post with the
  button and the reply flag and the UI log line, feeds an action and a
  reply through the bridge and receives both on the phone's session, and
  closes the recorder's id when the phone withdraws.
- **On the S25U, 22:16:** with the listener granted, a shell notification
  posted on the phone reached the daemon's UI log as a bell line with the
  app and title (a music player's did too); the phone received
  `notification: dismiss` and the shell notification was gone from the
  phone's dump afterwards.

## Limits

- The app icon the phone sends is not shown yet: the freedesktop
  `image-data` hint wants raw pixels and the daemon does not decode PNG.
  The server shows its generic phone icon.
- Inline replies need a server that emits `NotificationReplied` (KDE's
  extension). The desktop app has `ReplyNotification` on the bus for its
  own field, not yet a screen.
- `VAL-MAG-12` is the author's run with real applications.
