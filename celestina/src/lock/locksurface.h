#pragma once

#include <QtWaylandClient/private/qwaylandshellintegration_p.h>
#include <QtWaylandClient/private/qwaylandshellsurface_p.h>
#include <QtWaylandClient/private/qwaylandwindow_p.h>

#include <QHash>
#include <QObject>
#include <QPointer>
#include <QScreen>

#include "qwayland-ext-session-lock-v1.h"

#include "locksession.h"

// The Wayland half of a session lock: the protocol object that makes the
// compositor stop showing this session, and the per-output surfaces that are
// all it will show instead.
//
// ADR 0004 is why this is here rather than a layer surface pretending to be a
// lock. `ext-session-lock-v1` moves the guarantee out of this program: once
// the compositor has sent `locked`, the session stays locked whatever happens
// to this process. A layer surface with a keyboard grab looks the same and
// guarantees nothing — kill the client and the desktop is simply there.
//
// Qt allows one shell integration per process, and the shell's own surfaces
// are layer surfaces, which is the mechanical reason the lock is its own
// program. The isolation it buys is the reason it would have been worth doing
// anyway.

// The lock itself: acquired once, and released from exactly one place.
class SessionLock final : public QObject,
                          public QtWayland::ext_session_lock_v1
{
    Q_OBJECT

public:
    SessionLock(::ext_session_lock_v1 *lock, QObject *parent = nullptr);

    // The facade everything above the protocol talks to. Owned by this.
    LockSession *session() const { return m_session; }

protected:
    void ext_session_lock_v1_locked() override;
    void ext_session_lock_v1_finished() override;

private:
    LockSession *m_session;
};

// One output's cover. The compositor configures its exact size; anything this
// draws before acknowledging that size is a protocol error.
class SessionLockSurface final : public QtWaylandClient::QWaylandShellSurface,
                                 public QtWayland::ext_session_lock_surface_v1
{
    Q_OBJECT

public:
    SessionLockSurface(
        SessionLock *lock,
        QtWaylandClient::QWaylandWindow *window
    );
    ~SessionLockSurface() override;

    bool isExposed() const override { return m_configured; }

protected:
    void ext_session_lock_surface_v1_configure(
        uint32_t serial,
        uint32_t width,
        uint32_t height
    ) override;

private:
    QtWaylandClient::QWaylandWindow *m_window;
    bool m_configured = false;
};

// Qt asks this to give every window its shell role. It answers with lock
// surfaces and nothing else: a window that appears in this process is by
// definition part of the cover.
class SessionLockIntegration final
    : public QtWaylandClient::QWaylandShellIntegration
{
public:
    bool initialize(QtWaylandClient::QWaylandDisplay *display) override;
    QtWaylandClient::QWaylandShellSurface *createShellSurface(
        QtWaylandClient::QWaylandWindow *window
    ) override;

private:
    QtWaylandClient::QWaylandDisplay *m_display = nullptr;
    static SessionLock *s_lock;
};
