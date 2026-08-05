// Makes the portal's picker a child of the window that asked for it.
//
// The portal hands a backend a `parent_window` string: on Wayland,
// `wayland:<handle>`, where the handle was produced by the asking application
// exporting its toplevel through `xdg-foreign`. Importing that handle and
// pointing it at this window's surface is what tells the compositor the two
// belong together — so the dialog stacks over its application, follows it, and
// is not just another window that happens to be on top.
//
// It is C++ and not Rust because both ends of the operation are Qt internals:
// the `wl_display` the application is already connected to and the `wl_surface`
// behind a `QWindow`. Reaching them from Rust would mean unsafe FFI into Qt,
// which this workspace does not do.
#pragma once

#include <QtCore/QObject>
#include <QtCore/QString>

struct wl_registry;
struct zxdg_importer_v2;
struct zxdg_imported_v2;

class QWindow;

class SideritaWindowParent : public QObject
{
    Q_OBJECT

public:
    explicit SideritaWindowParent(QObject *parent = nullptr);
    ~SideritaWindowParent() override;

    // Adopts `window` into the toplevel named by `parentWindow`, which is the
    // portal's own string (`wayland:<handle>`; anything else, including the
    // empty string an application sends when it has no window to point at, is
    // ignored). Returns whether the compositor accepted the relationship, so a
    // caller can say honestly that it did not happen.
    //
    // Safe to call before the window has a surface: it asks the window to
    // create one first, and answers false rather than crashing on a platform
    // that has no Wayland at all (offscreen, X11).
    Q_INVOKABLE bool adopt(QWindow *window, const QString &parentWindow);

private:
    // Held for as long as this object lives: destroying the import is what
    // ends the relationship, so it must outlive the call that made it.
    zxdg_importer_v2 *m_importer = nullptr;
    zxdg_imported_v2 *m_imported = nullptr;
};

void register_siderita_window_parent();
