# Evidence: 2026-08-05 portal file dialog

- **Status:** implementation complete
- **Trigger:** without a Qt platform theme, Grafita's `FileDialog` bypassed the
  session's file-chooser portal and drew a separate Qt window
- **Result:** Grafita selects `xdgdesktopportal` only when
  `QT_QPA_PLATFORMTHEME` is absent, preserving every explicit environment
  choice
- **Observed evidence:** the release build was installed at the author's
  request and a real open action reached Siderita through the existing portal
  route; no desktop handler or portal configuration was changed
- **Deferred evidence:** the complete cross-application daily portal matrix
  remains `VAL-SID-02`
