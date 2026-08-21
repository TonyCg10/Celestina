#pragma once

#include <QObject>
#include <QPointer>
#include <QQmlComponent>
#include <QElapsedTimer>
#include <QHash>
#include <QPointF>
#include <QRectF>
#include <QSet>
#include <QStringList>
#include <QTimer>
#include <QVariantList>
#include <QVariantMap>

#include <functional>
#include <optional>

#include "osdreadings.h"

class OverlaySurface;
class PanelManager;
class PanelMenuController;
class QQmlEngine;
class QScreen;
class ShellProvidersClient;
class QWindow;

// The shell's on-screen display: what a device is at, shown for a moment
// under the bar and then gone.
//
// It appears at the top right, attached to the bar by the same drop membrane
// the menus use, with the mouth on the panel icon of the reading it shows —
// so a volume change visibly comes out of the volume control. When something
// interactive already occupies that zone it retreats to the bottom-right
// corner instead, and when the level is being changed from inside its own
// open menu it does not appear at all: the menu's slider is already showing
// it.
//
// It is not an overlay in the `OverlayController` sense. Nobody opens it and
// nobody dismisses it: it appears because a provider published a new reading,
// it never takes focus or the keyboard, and it leaves on its own. That is why
// this is a second controller rather than a third component loaded by the
// first, whose whole contract is a keybind-driven, focused, toggled surface.
//
// What is worth showing is `OsdReadings`' decision; this class owns only the
// window's life: create it once, keep it while readings keep arriving, and
// tear it down when they stop.
class OsdController final : public QObject
{
    Q_OBJECT

public:
    OsdController(
        QQmlEngine *engine,
        ShellProvidersClient *providers,
        QObject *parent = nullptr
    );

    // False when the component itself failed to load — a broken QML file. The
    // shell then simply never shows an OSD; nothing else changes.
    bool isEnabled() const { return m_enabled; }
    bool isVisible() const;

    // Wired in after construction, like `PanelManager`'s own setters. A
    // display without them keeps the floating corner it has always had.
    void setPanels(PanelManager *panels);
    void setMenus(PanelMenuController *menus) { m_menus = menus; }
    // The open cards of every other surface on one screen, in output-local
    // shell units: what the display must not paint over. A callback because
    // the owners are five controllers `main()` knows and this class must not.
    void setZoneProbe(std::function<QList<QRectF>(QScreen *)> probe)
    {
        m_zoneProbe = std::move(probe);
    }

    // Where this display's card currently sits, for the toasts' own probe.
    QRectF openCardRectOnOutput(QScreen *screen) const;

    // Which outputs a fullscreen window occupies (SURF-1-C). The resting
    // persistent twins on such an output unmap so the game keeps its direct
    // scanout; a reading raised during the game still opens them — the author
    // wants a volume notch visible there — and the recede's end puts them
    // away again. Wired from the Niri client by `main()`.
    void setFullscreenOutputs(const QStringList &outputs);

public slots:
    // A menu or overlay was just mapped. A display already on screen where
    // that card landed retreats to its fallback corner, carrying its stack,
    // instead of being painted over.
    void retreatIfCovered();

private:
    void providersChanged();
    void show(const OsdReadings::Reading &reading);
    void hide();
    QWindow *createWindow(const QVariantMap &placementProperties);
    void pushReadings(QWindow *window);
    bool resolveAttachment(
        QScreen *screen,
        const QString &kind,
        QRectF *opener,
        QRectF *icon,
        qreal *barHeight
    ) const;
    void updateAttachment(QWindow *window, const QString &kind);
    void ensureSurfaces(QScreen *screen);
    void openTop(QScreen *screen);
    void openFallback(QScreen *screen);
    void switchTo(bool top);
    QWindow *activeWindow() const;
    bool topIntruded() const;
    void beginClose();
    void finishClose();
    // Unmaps the resting twins when their output is fullscreen-occupied.
    // Nothing to do while cards are up or the recede beat is still running.
    void yieldRestingToFullscreen();
    void applyInputMask(QWindow *window);
    void scheduleExpiry();
    void expire();
    QString frontKind() const;

    QQmlComponent m_component;
    QPointer<ShellProvidersClient> m_providers;
    OverlaySurface *m_surface;
    // The display's two persistent surfaces: the attached top-right home and
    // the bottom-right fallback, both premapped and kept alive, because a
    // freshly mapped quiet window takes seconds to its first presented frame
    // — a retreat that remapped arrived invisible. Moving between them is a
    // property push on windows that are already rendering.
    OverlaySurface *m_fallback;
    QRectF m_fallbackCard;
    bool m_activeTop = true;
    QPointer<PanelManager> m_panels;
    QPointer<PanelMenuController> m_menus;
    std::function<QList<QRectF>(QScreen *)> m_zoneProbe;
    // Where the open display's card is, on which screen — recorded at open so
    // the answer never depends on reading a layer surface's geometry back.
    QRectF m_openCard;
    QPointer<QScreen> m_openScreen;
    bool m_openAttached = false;
    // Output-local shell origin of the persistent top carrier. Its cards and
    // every later icon update are translated through this one fixed point.
    QPointF m_attachedCarrierOrigin;
    OsdReadings m_readings;
    // The card file: every reading still worth showing, front first, and the
    // moment each one stops being. One timer serves the earliest deadline; a
    // burst of wheel notches keeps refreshing its own kind's clock, so the
    // display neither flickers per notch nor forgets a neighbour's card.
    QVariantList m_active;
    QHash<QString, qint64> m_deadlines;
    QElapsedTimer m_clock;
    QTimer m_expiryTimer;
    // The exit's own beat. A card leaves by receding — the author's rule
    // (2026-08-13) is that no card may enter while another is still leaving,
    // so a reading that arrives mid-exit waits here and is shown from the
    // start once the departure has finished. Latest wins: the wait holds one
    // reading, never a queue.
    QTimer m_transitionTimer;
    bool m_closing = false;
    std::optional<OsdReadings::Reading> m_pending;
    QSet<QString> m_fullscreenOutputs;
    bool m_enabled;
};
