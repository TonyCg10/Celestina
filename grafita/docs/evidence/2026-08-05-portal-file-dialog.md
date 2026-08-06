# Evidence: 2026-08-05 portal file dialog

- **Date:** 2026-08-05
- **Status:** implementation complete
- **Scope:** grafita — the Qt platform-theme selection that decides whether
  `FileDialog` reaches the session's file-chooser portal
- **Trigger:** without a Qt platform theme, Grafita's `FileDialog` bypassed the
  session's file-chooser portal and drew a separate Qt window
- **Environment:** the author's live session, with its existing desktop handler
  and portal configuration left unchanged; Siderita served the portal route
- **Artifact:** the release build, installed at the author's request

## Procedure

The release build was installed at the author's request and a real open action
was performed, observing which file chooser it reached. No desktop handler or
portal configuration was changed.

## Result

Grafita selects `xdgdesktopportal` only when `QT_QPA_PLATFORMTHEME` is absent,
preserving every explicit environment choice. The observed open action reached
Siderita through the existing portal route.

## Limits

Only one open action in one session was observed. The complete
cross-application daily portal matrix remains deferred to `VAL-SID-02`.
