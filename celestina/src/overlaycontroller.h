#pragma once

#include <QObject>
#include <QPointer>
#include <QQmlComponent>
#include <QPointF>
#include <QRectF>
#include <QString>
#include <QStringList>
#include <QVariantMap>

#include <functional>

#include "panelattachmentlease.h"

class OverlaySurface;
class QQmlEngine;
class QScreen;
class QWindow;

// The bridge property each overlay component declares, or an empty string for a
// component this shell does not have.
//
// One list rather than five call sites, because "which property does this
// component declare" is one fact and a second copy of it is what produced
// `SessionMenu does not have a property called providerSource`. `main()` builds
// every controller through it and the regression compares it against the QML
// files themselves.
QString overlaySourceProperty(const QString &qmlComponentName);

// Opens and closes one overlay — the launcher, clipboard history,
// notification centre, control centre, bubble selector or session menu.
//
// They are identical in mechanics: one on-demand-keyboard surface, centred
// for a `celestina msg`/keybind request or anchored for a panel request, and
// torn down on its own dismissal. They
// differ in which QML component they load and which bridge that component
// reads, so this class owns exactly the shared part. Domain logic — searching,
// launching, selecting a history entry, arming a session verb — lives entirely
// in the QML component, which talks to its bridge the same way every bar widget
// already does (see `Panel.qml`): nothing here parses a provider payload or
// knows a launcher or a clipboard exists.
//
// What it does not do is hand every component the same property set. Qt refuses
// an initial property a component does not declare and says so at runtime,
// which is how a session menu that reads `shellSource` came to log
// `SessionMenu does not have a property called providerSource` on every open.
// The bridge is therefore named by whoever builds the controller, and
// `reducedMotion` is the only property added here — it is a presentation
// contract every one of these surfaces declares.
class OverlayController final : public QObject
{
    Q_OBJECT

public:
    // Where a keybind-opened overlay goes. `QCursor::pos()` is not an answer
    // on Wayland — a layer-shell client cannot ask where the pointer is, so
    // the launcher could open on a blacked-out monitor while the person typed
    // into nothing. The compositor knows the output holding the focused
    // workspace; the host wires that in once for every overlay.
    void setFocusedOutputSource(std::function<QString()> source)
    {
        m_focusedOutput = std::move(source);
    }

    // `source` is the bridge this component reads; which property it arrives
    // as comes from `overlaySourceProperty`. Four of these overlays read the
    // provider bridge as `providerSource` and the session menu reads a request
    // channel as `shellSource`, so the owner supplies the object and the list
    // supplies the name.
    OverlayController(
        QQmlEngine *engine,
        const QString &qmlComponentName,
        QObject *source,
        QObject *parent = nullptr
    );
    ~OverlayController() override;

    // Exactly what this controller would hand its component, and nothing else.
    // Exposed so a regression can compare it against what the QML file
    // declares, before a session is the thing that finds out.
    QVariantMap initialProperties() const;

    // False when the component itself failed to load — a broken QML file, not
    // a missing source. The overlay simply never opens; nothing crashes.
    bool isEnabled() const { return m_enabled; }
    bool isOpen() const;

    // The open overlay's card on one output, in output-local shell units, or
    // an empty rectangle. The quiet surfaces ask before landing at the top
    // right.
    QRectF openCardRectOnOutput(QScreen *screen) const;

    // A fullscreen window took these outputs (SURF-1-C): a carrier parked on
    // one of them unmaps so the game keeps its direct scanout. An overlay
    // that is actually open is left alone — it is being used, and its own
    // close parks or unmaps as ever. Wired from the Niri client by `main()`.
    void yieldParkedCarrier(const QStringList &fullscreenOutputs);

public slots:
    void open();
    void close();
    void toggle();
    // The same overlay opened from a panel control rather than a keybind. The
    // opener's rectangle travels with it so the surface can grow out of the
    // control instead of appearing beside it; an empty rectangle means there was
    // no control, which is what a keybind is.
    // Toggle for a surface with no pointer origin, told directly which monitor's bubbles
    // it is about.
    void toggleWithBubbleAnchor(const QString &output, const QRectF &anchor);
    void toggleFrom(
        QWindow *panel,
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor
    );

signals:
    // This overlay has swapped the first frame after publishing painted glass.
    // Consumers may now retire the contextual surface it replaces without
    // exposing the desktop between the old and new cards.
    void contextualSurfaceOpened();

private slots:
    // A retired QML window may finish its close transition after a successor
    // is mapped. Only the window still adopted by the surface may close it.
    void overlayDismissed();
    // `glassRegions` is a QML alias, so its notify signal is connected by name.
    // A non-empty publication arms readiness; only its following swapped frame
    // may publish the edge to the rest of the shell.
    void overlayGlassRegionsChanged();

private:
    QWindow *createWindow();
    // The per-route geometry and anchors the next window needs beyond the
    // constant initial properties: the panel opener, the attachment anchor,
    // the bubble anchor. One computation for both lifecycles — created fresh
    // it merges into the initial properties, reused it is applied over the
    // reset defaults.
    QVariantMap routeProperties() const;
    // Resets a parked carrier's route to this open's own: defaults first, so
    // a keybind that follows a panel click does not inherit the click's
    // origin, then whatever `routeProperties` names.
    void applyRouteProperties(QWindow *window) const;
    // Brings a parked carrier's content back for the next open: the fields'
    // completed retirement is cleared, the faded content paints again, and
    // the readiness trackers forget the previous open so the reveal gate can
    // be earned again on this same window.
    void reviveWindowForReuse(QWindow *window);
    void revealPresentedWindow(QWindow *window);
    // The hard edge is private: only a completed soft retirement or object
    // teardown may destroy the carrier immediately. The generation is the
    // guard the window pointer used to be: before carrier reuse, a reopen
    // built a new window and a stale beat's pointer no longer matched — now
    // every open resumes the same window, so the beat names which open it
    // belongs to and a reopen invalidates it by advancing the count.
    void closeNow(
        QWindow *expectedWindow = nullptr,
        quint64 expectedGeneration = 0
    );

    // Where the surface should grow from, while a panel-opened toggle is in
    // flight. Empty for a keybind, which has no origin on screen.
    QRectF m_opener;
    QRectF m_attachmentAnchor;
    QPointer<QWindow> m_openerPanel;
    QString m_bubbleAnchorOutput;
    QRectF m_bubbleAnchor;
    PanelAttachmentLease m_attachmentLease;
    // Physical output-local origin of the mapped carrier. Zero for keybind
    // and floating routes; a panel-attached overlay retains the panel's lower
    // seam here so occupancy answers can translate its local card back once.
    QPointF m_openCarrierOriginOnOutput;
    QPointer<QWindow> m_revealIssuedWindow;
    QPointer<QWindow> m_glassAwaitingFrameWindow;
    QPointer<QWindow> m_readyWindow;

    QQmlComponent m_component;
    std::function<QString()> m_focusedOutput;
    QString m_componentName;
    QString m_sourceProperty;
    // Guarded rather than owned: the bridge outlives no overlay in practice,
    // but an overlay that opened after its source died would bind `undefined`
    // into a `required property`.
    QPointer<QObject> m_source;
    OverlaySurface *m_surface;
    // Which open the surface is currently showing. Advanced by every open —
    // fresh map and resume alike — so a close beat scheduled against an
    // earlier open can never park the one the person just made.
    quint64 m_openGeneration = 0;
    bool m_enabled;
};
