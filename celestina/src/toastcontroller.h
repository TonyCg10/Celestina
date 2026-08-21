#pragma once

#include <QObject>
#include <QPointer>
#include <QQmlComponent>
#include <QRectF>
#include <QStringList>
#include <QTimer>
#include <QVariantList>
#include <QVariantMap>

#include <functional>

class OverlaySurface;
class PanelManager;
class QQmlEngine;
class QScreen;
class ShellProvidersClient;
class QWindow;

// Where this session's notifications appear: at the top right, attached to
// the bar by the same drop membrane the menus use, with the mouth on the
// panel's own notification bell. When something interactive already occupies
// that zone the stack retreats to the bottom centre — deliberately not the
// same corner the on-screen display retreats to, so the two fallbacks cannot
// paint over each other.
//
// Nothing here decides how long a toast lives or what it says: the helper's
// server owns every one of those rules and publishes what is currently worth
// showing. This class only follows that list — it maps a surface when the list
// stops being empty, updates it in place while it changes, and tears it down
// when it empties, which is what keeps a burst of notifications from asking the
// compositor for a new surface each time.
//
// It is a second controller rather than a mode of `OsdController` because the
// two answer to different things: a readout is raised by a value changing and
// leaves on a timer this shell owns, while a toast is raised by another
// application and leaves when the server says it has.
class ToastController final : public QObject
{
    Q_OBJECT

public:
    ToastController(
        QQmlEngine *engine,
        ShellProvidersClient *providers,
        QObject *parent = nullptr
    );

    // False when the component itself failed to load — a broken QML file. The
    // shell then shows no toasts; the server and its history are unaffected.
    bool isEnabled() const { return m_enabled; }
    bool isVisible() const;

    // Wired in after construction. A stack without them keeps the floating
    // corner it has always had.
    void setPanels(PanelManager *panels) { m_panels = panels; }
    void setZoneProbe(std::function<QList<QRectF>(QScreen *)> probe)
    {
        m_zoneProbe = std::move(probe);
    }
    // Whether the notification centre is on screen. A toast raised while the
    // whole list is already open would announce what is being looked at —
    // the same rule that keeps a level's display quiet while its own menu
    // is up.
    void setCentreProbe(std::function<bool()> probe)
    {
        m_centreProbe = std::move(probe);
    }

    // Where the open stack's cards sit, for the display's own probe.
    QRectF openCardRectOnOutput(QScreen *screen) const;

    // A fullscreen window took these outputs (SURF-1-C): a carrier parked on
    // one of them unmaps so the game keeps its direct scanout. A stack that
    // is actually showing toasts is left alone. Wired from the Niri client
    // by `main()`.
    void yieldParkedCarrier(const QStringList &fullscreenOutputs);

private slots:
    // QML owns the presentation clock. Only the currently adopted stack may
    // close its carrier when that clock reaches the end of departure.
    void toastDepartureFinished();

private:
    void providersChanged();
    void show(const QVariantList &toasts, const QVariantList &actions);
    void hide();
    QWindow *createWindow(
        const QVariantList &toasts,
        const QVariantList &actions,
        const QVariantMap &placementProperties
    );
    void applyInputMask(QWindow *window);

    QQmlComponent m_component;
    QPointer<ShellProvidersClient> m_providers;
    OverlaySurface *m_surface;
    QPointer<PanelManager> m_panels;
    std::function<QList<QRectF>(QScreen *)> m_zoneProbe;
    std::function<bool()> m_centreProbe;
    QRectF m_openCard;
    QPointer<QScreen> m_openScreen;
    bool m_openAttached = false;
    // A watchdog, not the presentation clock: QML emits `departureFinished`
    // after its own exit beat. This closes only if a broken or unpresented
    // scene never delivers that edge; reentry stops it and reclaims the block.
    QTimer m_closeTimer;
    bool m_enabled;
};
