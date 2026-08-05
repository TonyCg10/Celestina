# Evidence: 2026-08-05 portal picker dialog and parenting

- **Status:** implementation complete
- **Trigger:** the portal picker presented as a second file-manager window and
  ignored the requester's Wayland parent handle
- **Result:** the picker is a compact one-column dialog; the portal retains the
  bounded parent string only for the request lifetime, and a narrow C++ seam
  imports `wayland:` handles through generated `xdg-foreign` marshalling
- **Fallback:** missing scanner, protocol, Wayland surface or valid handle
  leaves a centred unparented dialog and never fails the file request
- **Recorded automated evidence:** release build and link, Rust format and
  Clippy, QML Test 47/47, and an isolated offscreen portal request completed in
  the originating work session
- **Recorded environment evidence:** the session compositor advertised
  `zxdg_importer_v2`; offscreen correctly declined adoption without crashing
- **Deferred evidence:** real requester stacking and lifecycle remain
  `VAL-SID-02` and `VAL-SID-04`
