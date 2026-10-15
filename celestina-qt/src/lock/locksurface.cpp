#include "locksurface.h"

#include <QtWaylandClient/private/qwaylanddisplay_p.h>
#include <QtWaylandClient/private/qwaylandscreen_p.h>

#include <QDebug>

#include <wayland-client-core.h>
#include <QGuiApplication>

// The manager, bound once from the registry. It exists only to hand out the
// one lock; nothing else in this program keeps a reference to it.
class SessionLockManager : public QtWayland::ext_session_lock_manager_v1
{
public:
    using QtWayland::ext_session_lock_manager_v1::ext_session_lock_manager_v1;
};

namespace {
SessionLockManager *g_manager = nullptr;
}

SessionLock *SessionLockIntegration::s_lock = nullptr;

SessionLock::SessionLock(::ext_session_lock_v1 *lock, QObject *parent)
    : QObject(parent)
    , QtWayland::ext_session_lock_v1(lock)
    , m_session(new LockSession(this))
{
    // The facade holds the one unlocking call. Its own guards decide whether
    // this ever runs; the protocol object only supplies the act.
    m_session->m_unlock = [this]() { unlock_and_destroy(); };
}

void SessionLock::ext_session_lock_v1_locked()
{
    m_session->markConfirmed();
}

void SessionLock::ext_session_lock_v1_finished()
{
    m_session->markFinished();
}

SessionLockSurface::SessionLockSurface(
    SessionLock *lock,
    QtWaylandClient::QWaylandWindow *window
)
    : QtWaylandClient::QWaylandShellSurface(window)
    , QtWayland::ext_session_lock_surface_v1(
          lock->get_lock_surface(window->wlSurface(),
                                 window->waylandScreen()->output()))
    , m_window(window)
{
    // The protocol is explicit: "Committing the surface before acking the
    // first configure is a protocol error", and it promises the compositor
    // sends that configure immediately on binding. Qt does not know that — it
    // commits the surface as soon as this constructor returns, to apply the
    // viewport and transform it just set — so the configure has to be
    // collected and acknowledged here, before control goes back to it.
    //
    // On a queue of this surface's own, not the display's. Qt's round trip
    // dispatches the main queue, and running that from inside window creation
    // re-enters Qt's own machinery and never returns — measured, on the
    // nested session. A private queue waits for exactly the events of this
    // one proxy: everything else the socket delivers meanwhile stays queued
    // for Qt to dispatch when it is ready.
    struct wl_display *const display = m_window->display()->wl_display();
    struct wl_event_queue *const queue = wl_display_create_queue(display);
    auto *const proxy = reinterpret_cast<struct wl_proxy *>(object());
    wl_proxy_set_queue(proxy, queue);
    // Bounded, because a compositor that never configures must not hang the
    // lock: the surface simply stays blank and the compositor covers that
    // output with its own colour, which is still locked.
    for (int attempt = 0; attempt < 8 && !m_configured; ++attempt) {
        if (wl_display_roundtrip_queue(display, queue) < 0)
            break;
    }
    // Back to Qt's queue for every later configure — an output that changes
    // size while locked is Qt's business, not this constructor's.
    wl_proxy_set_queue(proxy, nullptr);
    wl_event_queue_destroy(queue);

    if (!m_configured) {
        // The compositor did not configure a lock surface it had just been
        // asked for. Nothing can be drawn on this output without committing
        // illegally, so the surface stays blank and the compositor renders
        // its own solid colour there — which is still a locked session.
        qCritical("celestina-lock: no configure for a lock surface");
    }
}

SessionLockSurface::~SessionLockSurface()
{
    destroy();
}

void SessionLockSurface::ext_session_lock_surface_v1_configure(
    uint32_t serial,
    uint32_t width,
    uint32_t height
)
{
    // The compositor decides this surface's size, and a lock surface that
    // commits a buffer of any other size is killed for it. Acknowledge the
    // serial, take the size it named, and let Qt apply it on its own thread
    // rather than resizing from under the renderer here.
    ack_configure(serial);
    m_configured = true;

    const QSize size(static_cast<int>(width), static_cast<int>(height));
    if (!size.isEmpty())
        m_window->resizeFromApplyConfigure(size);
    m_window->applyConfigureWhenPossible();
    // Exposed only now, and explicitly. Until the first configure is
    // acknowledged this surface may not carry a buffer — the compositor kills
    // a lock client that commits one early — and Qt has no other signal that
    // the wait is over.
    m_window->updateExposure();
}

bool SessionLockIntegration::initialize(
    QtWaylandClient::QWaylandDisplay *display
)
{
    // Nothing is bound here: the display is only remembered, and the manager
    // is bound when the first surface actually asks for it. Binding during
    // the display's own construction is not known to be harmful — it was
    // suspected of hanging Mesa's EGL and that was disproved by loading this
    // integration with an empty `initialize`, which hung just the same — but
    // binding late is the smaller claim on Qt's startup either way, and it
    // keeps this integration inert for a process that never locks.
    m_display = display;
    return true;
}

QtWaylandClient::QWaylandShellSurface *
SessionLockIntegration::createShellSurface(
    QtWaylandClient::QWaylandWindow *window
)
{
    if (!s_lock) {
        for (const auto &global : m_display->globals()) {
            if (global.interface
                != QLatin1String("ext_session_lock_manager_v1")) {
                continue;
            }
            g_manager = new SessionLockManager();
            g_manager->init(m_display->wl_registry(),
                            static_cast<int>(global.id), 1);
            break;
        }
        if (!g_manager) {
            // Without the protocol there is no lock to be had. Refusing here
            // is what makes the caller report "could not lock" instead of
            // putting up a window that merely looks like one.
            qCritical("celestina-lock: this compositor has no "
                      "ext-session-lock-v1");
            return nullptr;
        }
        s_lock = new SessionLock(g_manager->lock());
    }
    return new SessionLockSurface(s_lock, window);
}


