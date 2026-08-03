# Content activation contract

- **Status:** accepted
- **Scope:** Siderita, Grafita and Fluorita

Siderita offers two deliberately different actions over local content. The
gesture communicates whether the user wants a bounded in-place task or the full
owning application.

| Content | `Space` in Siderita | Double-click or `Enter` |
|---|---|---|
| Editable text | Open the embedded Grafita editor | Open standalone Grafita |
| Image, video or audio | Open the minimal embedded Fluorita viewer/player | Open standalone Fluorita on that item |
| Directory | Keep the file-manager interaction | Navigate into the directory |
| Unsupported or unclassified file | Keep bounded Quick Look/fallback behaviour | Delegate to the desktop handler |

## Ownership

Grafita owns document acceptance, editing, encoding and loss-free save rules.
Fluorita owns media classification, library projections, metadata, artwork and
playback sessions. Siderita owns selection, gesture routing, modal lifecycle and
fallback reporting.

Embedded and standalone surfaces share pure domain contracts but keep separate
Qt state and QML composition. Neither application imports the other's UI tree.

## Behavioural invariants

- The same content decision feeds both gestures; a filename or extension alone
  must not create contradictory routes.
- `Space` never becomes an alias for launching the full application.
- Double-click/`Enter` never reuses the embedded modal as the full app.
- Browsing does not construct a decoder or editor session. Work starts only
  after activation and remains bounded and cancellable.
- Failure to start a standalone app falls back truthfully to the desktop handler
  where that is safe; it does not silently report success.
- The embedded Grafita surface is a real simple editor, not the standalone tab
  or project UI.
- Standalone Fluorita retains Gallery for images/video and Music for
  albums/artists/tracks; the embedded surface remains a minimal item viewer or
  player.

Changing this mapping requires an accepted decision and updates to every
consumer with automated evidence in the implementation unit. It also creates or
updates an independent keyboard/real-session author-validation case; that case
does not keep the implementation checkpoint open.
