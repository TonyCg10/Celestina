#include "overlaycontroller.h"

#include "diagnosticjournal.h"
#include "quietplacement.h"

#include <QCursor>
#include <QDebug>
#include <QHash>
#include <QGuiApplication>
#include <QQuickItem>
#include <QQuickWindow>
#include <QQmlEngine>
#include <QScreen>
#include <QTimer>
#include <QVariantMap>
#include <QWindow>

#include "overlaysurface.h"
#include "panelpopupplacement.h"
#include "shellscale.h"
#include "softclose.h"

namespace {
QPointF panelCarrierOriginOnOutput(QWindow *panel)
{
    return panel ? QPointF(0, qMax(0, panel->height())) : QPointF();
}
} // namespace

QString overlaySourceProperty(const QString &qmlComponentName)
{
    // Every one of these declares `reducedMotion` too; that one is added by the
    // controller because it is a presentation contract rather than a bridge.
    static const QHash<QString, QString> declared {
        {QStringLiteral("LauncherOverlay"), QStringLiteral("providerSource")},
        {QStringLiteral("ClipboardOverlay"), QStringLiteral("providerSource")},
        {QStringLiteral("NotificationCenter"), QStringLiteral("providerSource")},
        {QStringLiteral("ControlCentre"), QStringLiteral("providerSource")},
        {QStringLiteral("BubbleSelector"), QStringLiteral("providerSource")},
        // The one overlay that reads no provider: it asks the session to end.
        {QStringLiteral("SessionMenu"), QStringLiteral("shellSource")},
    };
    return declared.value(qmlComponentName);
}

OverlayController::OverlayController(
    QQmlEngine *engine,
    const QString &qmlComponentName,
    QObject *source,
    QObject *parent
)
    : QObject(parent)
    , m_component(engine)
    , m_componentName(qmlComponentName)
    , m_sourceProperty(overlaySourceProperty(qmlComponentName))
    , m_source(source)
    , m_surface(new OverlaySurface(
          OverlaySurface::Placement::Centered,
          QStringLiteral("celestina-overlay"),
          this
      ))
    , m_enabled(true)
{
    connect(m_surface, &OverlaySurface::dismissed, this, [this]() {
        m_attachmentLease.release();
        m_openCarrierOriginOnOutput = QPointF();
        m_revealIssuedWindow.clear();
        m_glassAwaitingFrameWindow.clear();
        m_readyWindow.clear();
    });

    if (m_sourceProperty.isEmpty()) {
        qCritical() << "Celestina has no overlay named" << m_componentName;
        m_enabled = false;
        return;
    }

    m_component.loadFromModule("CelestinaDesktop", m_componentName);
    if (!m_component.isReady()) {
        qCritical().noquote() << "Celestina could not load its" << m_componentName
                               << "overlay:" << m_component.errorString();
        m_enabled = false;
    }
}

OverlayController::~OverlayController()
{
    closeNow();
}

QVariantMap OverlayController::initialProperties() const
{
    QVariantMap properties {
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    if (!m_sourceProperty.isEmpty())
        properties.insert(m_sourceProperty, QVariant::fromValue(m_source.data()));

    return properties;
}

QRectF OverlayController::openCardRectOnOutput(QScreen *screen) const
{
    QWindow *const window = m_surface->window();
    QRectF card = quietOpenCardRect(window, screen);
    if (card.isEmpty() || !window)
        return QRectF();

    const double scale = window->property("shellScale").toDouble();
    const qreal divisor = scale > 0.0 ? scale : 1.0;
    return card.translated(
        m_openCarrierOriginOnOutput.x() / divisor,
        m_openCarrierOriginOnOutput.y() / divisor
    );
}

bool OverlayController::isOpen() const
{
    return m_surface->isOpen();
}

void OverlayController::toggleWithBubbleAnchor(const QString &output, const QRectF &anchor)
{
    // A keybind has no pointer origin, so there is no panel to read geometry from. The
    // anchor is handed straight in instead: the caller already resolved which monitor's
    // bubbles this surface is about, and that is the whole of what it needs.
    m_bubbleAnchorOutput = output;
    m_bubbleAnchor = anchor;
    toggle();
    m_bubbleAnchorOutput.clear();
    m_bubbleAnchor = QRectF();
}

void OverlayController::toggleFrom(
    QWindow *panel,
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor
)
{
    // The opener is remembered for the next window this controller builds, then
    // spent: a keybind that follows a click must not inherit the click's origin.
    m_opener = globalOpener;
    m_attachmentAnchor = globalAttachmentAnchor;
    m_openerPanel = panel;
    toggle();
    m_opener = QRectF();
    m_attachmentAnchor = QRectF();
    m_openerPanel = nullptr;
}

QWindow *OverlayController::createWindow()
{
    QVariantMap properties = initialProperties();
    // M7 — where this output's bubbles sit. Outside the opener block on purpose: a bubble
    // anchor depends only on which panel the surface belongs to, never on where a pointer
    // was, so a selector opened from a keybind must get one too. Unlike the geometry below
    // it is not translated into the carrier — it stays in the compositor's output-local
    // logical space, because it is going to Niri rather than into this surface's layout.
    if (m_componentName == QStringLiteral("BubbleSelector")) {
        QRectF rect = m_bubbleAnchor;
        QString output = m_bubbleAnchorOutput;
        // Opened from the panel itself: ask that live panel where its slot is now.
        if (rect.isEmpty() && m_openerPanel) {
            QVariant answer;
            if (QMetaObject::invokeMethod(
                    m_openerPanel,
                    "bubbleAnchorRect",
                    Q_RETURN_ARG(QVariant, answer)
                )
                && answer.canConvert<QRectF>()) {
                rect = answer.toRectF();
                output = m_openerPanel->property("outputName").toString();
            }
        }
        if (!output.isEmpty() && rect.isValid() && rect.width() > 0 && rect.height() > 0) {
            properties.insert(QStringLiteral("bubbleAnchorRect"), rect);
            properties.insert(QStringLiteral("bubbleAnchorOutput"), output);
        }
    }
    if (!m_opener.isEmpty() && m_openerPanel && m_openerPanel->screen()) {
        const QScreen *const screen = m_openerPanel->screen();
        const QPointF outputOrigin(screen->geometry().topLeft());
        const QPointF carrierOrigin =
            panelCarrierOriginOnOutput(m_openerPanel);
        // An overlay is drawn at the size its output asks for, exactly as the
        // panel it comes from is; see shellscale.h. Everything below arrives
        // in output pixels, is translated into the physically inset carrier,
        // and is divided once here, so QML lays out in the unscaled local units
        // its tokens are written in.
        const double shellScale = shellScaleForScreen(screen);
        properties.insert(QStringLiteral("anchoredFromPanel"), true);
        properties.insert(
            QStringLiteral("openerRect"),
            panelAttachmentRectOnCarrier(
                m_opener, outputOrigin, carrierOrigin, shellScale)
        );
        properties.insert(
            QStringLiteral("attachmentAnchorRect"),
            panelAttachmentRectOnCarrier(
                m_attachmentAnchor,
                outputOrigin,
                carrierOrigin,
                shellScale
            )
        );
        // The QWindow begins at the panel's physical lower edge. That edge is
        // the attachment seam, so it is exactly zero in the local scene.
        properties.insert(
            QStringLiteral("attachmentStartY"),
            0
        );
    }

    QObject *rootObject = m_component.createWithInitialProperties(properties);
    if (!rootObject) {
        qCritical().noquote() << "Celestina could not create its" << m_componentName
                               << "overlay:" << m_component.errorString();
        return nullptr;
    }

    auto *window = qobject_cast<QWindow *>(rootObject);
    if (!window) {
        qCritical() << "Celestina's" << m_componentName << "overlay component is not a window.";
        delete rootObject;
        return nullptr;
    }

    // QML-declared, so these are reached by name rather than a generated
    // header. `glassRegions` becomes non-empty only after the common field has
    // non-zero painted opacity and has published the matching blur geometry.
    connect(window, SIGNAL(dismissed()), this, SLOT(overlayDismissed()));
    connect(
        window,
        SIGNAL(glassRegionsChanged()),
        this,
        SLOT(overlayGlassRegionsChanged())
    );

    auto *const quickWindow = qobject_cast<QQuickWindow *>(window);
    if (!quickWindow) {
        qCritical() << "Celestina's" << m_componentName
                    << "overlay component is not a Qt Quick window.";
        delete window;
        return nullptr;
    }

    // Mapping is one controller concern, not five component-local recipes.
    // A swapped frame while Qt still reports the window unexposed may be a
    // bootstrap buffer the compositor will discard. The first swap after
    // exposure is the presentation gate; `revealNow()` consumes that gate
    // directly instead of waiting for a second frame inside SoftMenuField.
    const QPointer<QWindow> tracked(window);
    connect(
        quickWindow,
        &QQuickWindow::frameSwapped,
        this,
        [this, tracked]() {
            if (!tracked || m_surface->window() != tracked
                || tracked->property("celestinaRetiring").toBool()) {
                return;
            }
            if (!tracked->isExposed()) {
                tracked->requestUpdate();
                return;
            }

            // A non-empty glass publication describes the next render, not
            // the frame whose swap first opened the reveal gate. Retire the
            // predecessor only after that painted buffer itself has swapped.
            if (m_glassAwaitingFrameWindow == tracked) {
                m_glassAwaitingFrameWindow.clear();
                if (m_readyWindow != tracked) {
                    m_readyWindow = tracked;
                    emit contextualSurfaceOpened();
                }
                return;
            }

            if (m_revealIssuedWindow != tracked)
                revealPresentedWindow(tracked);
        }
    );
    return window;
}

void OverlayController::revealPresentedWindow(QWindow *window)
{
    if (!window || m_surface->window() != window
        || m_revealIssuedWindow == window
        || window->property("celestinaRetiring").toBool()) {
        return;
    }

    m_revealIssuedWindow = window;
    const auto fields = window->findChildren<QQuickItem *>(
        QStringLiteral("celestina-soft-menu-field"));
    if (fields.isEmpty()) {
        qCritical() << "Celestina's" << m_componentName
                    << "overlay has no shared presentation field.";
        return;
    }
    for (QQuickItem *const field : fields)
        QMetaObject::invokeMethod(field, "revealNow");
}

void OverlayController::overlayGlassRegionsChanged()
{
    auto *const window = qobject_cast<QWindow *>(sender());
    if (!window || m_surface->window() != window || m_readyWindow == window
        || window->property("celestinaRetiring").toBool()) {
        return;
    }

    if (window->property("glassRegions").toList().isEmpty()) {
        if (m_glassAwaitingFrameWindow == window)
            m_glassAwaitingFrameWindow.clear();
        return;
    }

    // Publishing the region mutates scene state before Qt Quick has swapped
    // the buffer that paints it. Arm exactly one following frame and make sure
    // even a reduced-motion scene schedules that frame.
    m_glassAwaitingFrameWindow = window;
    window->requestUpdate();
}

void OverlayController::overlayDismissed()
{
    DiagnosticJournal::instance().record(
        DiagnosticJournal::Record(
            DiagnosticJournal::Level::Info,
            QStringLiteral("ctx.overlay_dismissed"))
            .text(QStringLiteral("overlay"), m_componentName)
            .flag(QStringLiteral("is_current"),
                  sender() == m_surface->window())
    );
    // Every public retirement takes the same idempotent soft edge. The lease
    // releases there immediately while the private hard edge waits for the
    // animation to complete.
    if (sender() == m_surface->window())
        close();
}

void OverlayController::open()
{
    // A `required property` bound to a destroyed bridge is a component that
    // fails to create, so an overlay whose source is gone does not open at all.
    if (!m_enabled || !m_source || isOpen())
        return;

    // A keybind names no click position; the overlay follows the output the
    // compositor says is focused. The pointer's position was the old answer
    // and is not one on Wayland — Qt reports a stale or zero point there, and
    // the launcher opened on whatever output owned the origin, including a
    // blacked-out one.
    QScreen *screen = m_openerPanel ? m_openerPanel->screen() : nullptr;
    if (!screen && m_focusedOutput) {
        const QString name = m_focusedOutput();
        const auto screens = QGuiApplication::screens();
        for (QScreen *const candidate : screens) {
            if (!name.isEmpty() && candidate->name() == name) {
                screen = candidate;
                break;
            }
        }
    }
    if (!screen)
        screen = QGuiApplication::primaryScreen();

    const bool attachedFromPanel = !m_opener.isEmpty()
        && m_openerPanel && m_openerPanel->screen();
    const QPointF carrierOrigin = attachedFromPanel
        ? panelCarrierOriginOnOutput(m_openerPanel) : QPointF();

    QWindow *const overlay = createWindow();
    if (!overlay)
        return;

    // The output is only known here for a keybind route, which names no click
    // and follows the pointer. `createWindow` already divided any opener
    // geometry by this same factor, because that route has a panel and so
    // resolves to this very screen.
    overlay->setProperty("shellScale", shellScaleForScreen(screen));

    // Everything this covers answers a click by retiring. On a panel route the
    // carrier itself begins below the bar, so its complete local region is the
    // outside-dismiss barrier and the panel remains physically outside it. A
    // keybind route starts at zero and keeps complete-output coverage.
    //
    // The mask is put on *before* the surface is shown, so its very first
    // commit already owns the complete below-panel carrier. Applied after,
    // the author could outrace it: for a few frames the fresh surface took an
    // incomplete region, and a quick outside click reached whatever was
    // behind it. The size connections keep the region true across the
    // configures that follow.
    if (attachedFromPanel) {
        const QPointer<QWindow> tracked(overlay);
        const QPointer<QWindow> bar = m_openerPanel;
        const auto apply = [tracked, bar]() {
            if (!tracked || !bar)
                return;
            tracked->setMask(panelPopupInputRegion(
                tracked->width(), tracked->height(), 0));
        };
        apply();
        connect(overlay, &QWindow::widthChanged, overlay, apply);
        connect(overlay, &QWindow::heightChanged, overlay, apply);
        // A mask set before the platform surface exists can be lost with it,
        // and a window whose QML size already matches the output sees no
        // size change on configure — the one reapply hook this had. Measured
        // on the nested session: the control centre kept whole-output input
        // and its card swallowed bar clicks in silence. Reapplying after the
        // map, and once more after the first commits, closes every path.
        QTimer::singleShot(0, overlay, apply);
        QTimer::singleShot(120, overlay, apply);
    }

    if (!m_surface->open(
            overlay,
            screen,
            OverlaySurface::Placement::Centered,
            qRound(carrierOrigin.y())
        )) {
        delete overlay;
        return;
    }

    m_openCarrierOriginOnOutput = carrierOrigin;
    m_attachmentLease.acquire(
        m_openerPanel,
        overlay,
        m_attachmentAnchor,
        carrierOrigin
    );
}

void OverlayController::close()
{
    m_attachmentLease.release();
    QWindow *const window = m_surface->window();
    if (!window) {
        m_openCarrierOriginOnOutput = QPointF();
        m_revealIssuedWindow.clear();
        m_glassAwaitingFrameWindow.clear();
        m_readyWindow.clear();
        return;
    }

    const QPointer<OverlayController> self(this);
    const QPointer<QWindow> tracked(window);
    softCloseWindow(window, [self, tracked]() {
        if (self && tracked)
            self->closeNow(tracked);
    });
}

void OverlayController::closeNow(QWindow *expectedWindow)
{
    if (expectedWindow && m_surface->window() != expectedWindow)
        return;

    m_surface->close();
    m_attachmentLease.release();
    m_openCarrierOriginOnOutput = QPointF();
    m_revealIssuedWindow.clear();
    m_glassAwaitingFrameWindow.clear();
    m_readyWindow.clear();
}

void OverlayController::toggle()
{
    // A toggle that finds its overlay already fading is a request to have it
    // gone, not to reopen it mid-beat.

    // One bounded line per gesture: which overlay was asked, and whether the
    // ask found it open. The nested session's console is unreachable, and the
    // author's two-click reports live or die on this ordering.
    DiagnosticJournal::instance().record(
        DiagnosticJournal::Record(
            DiagnosticJournal::Level::Info,
            QStringLiteral("ctx.toggle"))
            .text(QStringLiteral("overlay"), m_componentName)
            .flag(QStringLiteral("was_open"), isOpen())
    );
    if (isOpen())
        close();
    else
        open();
}
