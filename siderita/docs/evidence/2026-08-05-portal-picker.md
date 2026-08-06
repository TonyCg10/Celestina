# Evidence: 2026-08-05 portal picker dialog and parenting

- **Date:** 2026-08-05
- **Status:** implementation complete
- **Scope:** siderita — the portal file-picker presentation and the adoption of
  the requester's Wayland parent handle
- **Trigger:** the portal picker presented as a second file-manager window and
  ignored the requester's Wayland parent handle
- **Environment:** the originating work session; the session compositor
  advertised `zxdg_importer_v2`, and the offscreen run correctly declined
  adoption without crashing
- **Artifact:** the release build produced by the recorded build-and-link check
  in the originating work session; no deployment is recorded here

## Procedure

The recorded checks were the release build and link, Rust format and Clippy,
QML Test 47/47, and an isolated offscreen portal request completed in the
originating work session.

## Result

The picker is a compact one-column dialog; the portal retains the bounded
parent string only for the request lifetime, and a narrow C++ seam imports
`wayland:` handles through generated `xdg-foreign` marshalling.

A missing scanner, protocol, Wayland surface or valid handle leaves a centred
unparented dialog and never fails the file request.

## Limits

Real requester stacking and lifecycle were not exercised here: the offscreen
run declined adoption, and those paths remain deferred to `VAL-SID-02` and
`VAL-SID-04`.
