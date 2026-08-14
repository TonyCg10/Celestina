#include "overlaycontroller.h"

#include "diagnosticjournal.h"
#include "quietplacement.h"

#include <QCursor>
#include <QDebug>
#include <QHash>
#include <QGuiApplication>
#include <QQmlEngine>
#include <QScreen>
#include <QTimer>
#include <QVariantMap>
#include <QWindow>

#include "overlaysurface.h"
#include "panelpopupplacement.h"
#include "shellscale.h"
#include "softclose.h"

QString overlaySourceProperty(const QString &qmlComponentName)
{
    // Every one of these declares `reducedMotion` too; that one is added by the
    // controller because it is a presentation contract rather than a bridge.
    static const QHash<QString, QString> declared {
        {QStringLiteral("LauncherOverlay"), QStringLiteral("providerSource")},
        {QStringLiteral("ClipboardOverlay"), QStringLiteral("providerSource")},
        {QStringLiteral("NotificationCenter"), QStringLiteral("providerSource")},
        {QStringLiteral("ControlCentre"), QStringLiteral("providerSource")},
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
    return quietOpenCardRect(m_surface->window(), screen);
}

bool OverlayController::isOpen() const
{
    return m_surface->isOpen();
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
    if (!m_opener.isEmpty() && m_openerPanel && m_openerPanel->screen()) {
        const QScreen *const screen = m_openerPanel->screen();
        const QPointF outputOrigin = screen->geometry().topLeft();
        // An overlay is drawn at the size its output asks for, exactly as the
        // panel it comes from is; see shellscale.h. Everything below arrives
        // in output pixels and is divided once here, so the QML lays out in
        // the unscaled units its tokens are written in.
        const double shellScale = shellScaleForScreen(screen);
        const auto inShellUnits = [shellScale](const QRectF &rect) {
            return shellScale > 0
                ? QRectF(rect.x() / shellScale, rect.y() / shellScale,
                         rect.width() / shellScale, rect.height() / shellScale)
                : rect;
        };
        const QRectF openerOnOutput =
            panelPopupOpenerOnOutput(m_opener, outputOrigin);
        const QRectF attachmentAnchorOnOutput =
            panelPopupOpenerOnOutput(m_attachmentAnchor, outputOrigin);
        properties.insert(QStringLiteral("anchoredFromPanel"), true);
        properties.insert(
            QStringLiteral("openerRect"), inShellUnits(openerOnOutput));
        properties.insert(
            QStringLiteral("attachmentAnchorRect"),
            inShellUnits(attachmentAnchorOnOutput)
        );
        // The panel is a top-anchored, edge-to-edge surface whose height is
        // exactly the continuous backdrop. Menus attach to that lower edge,
        // not to the variable-height control rectangle inside it.
        properties.insert(
            QStringLiteral("attachmentStartY"),
            shellScale > 0 ? qMax(0, m_openerPanel->height()) / shellScale
                           : qMax(0, m_openerPanel->height())
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

    // QML-declared, so it is reached by name rather than a generated header.
    connect(window, SIGNAL(dismissed()), this, SLOT(overlayDismissed()));
    return window;
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
    // The same closing beat the menus get: fade, then the real close. The
    // lease releases now so the opener's held hover circle lets go with the
    // gesture rather than after the beat.
    if (sender() == m_surface->window()) {
        m_attachmentLease.release();
        QWindow *const window = m_surface->window();
        softCloseWindow(window, [this, window]() {
            if (m_surface->window() == window)
                close();
        });
    }
}

void OverlayController::open()
{
    // A `required property` bound to a destroyed bridge is a component that
    // fails to create, so an overlay whose source is gone does not open at all.
    if (!m_enabled || !m_source || isOpen())
        return;

    // A keybind names no click position; the overlay follows the pointer's
    // output the way a launcher is expected to, and falls back to the primary
    // screen when the pointer sits nowhere a screen claims (a session with no
    // outputs left).
    QScreen *screen = m_openerPanel ? m_openerPanel->screen() : nullptr;
    if (!screen)
        screen = QGuiApplication::screenAt(QCursor::pos());
    if (!screen)
        screen = QGuiApplication::primaryScreen();

    QWindow *const overlay = createWindow();
    if (!overlay)
        return;

    // The output is only known here for a keybind route, which names no click
    // and follows the pointer. `createWindow` already divided any opener
    // geometry by this same factor, because that route has a panel and so
    // resolves to this very screen.
    overlay->setProperty("shellScale", shellScaleForScreen(screen));

    // Everything this covers answers a click by retiring — except the strip
    // the panel reserved, which stays the bar's, so a click on another opener
    // swaps surfaces in one gesture instead of only closing this one. A
    // keybind route names no panel and cannot measure that strip, so it keeps
    // the complete coverage it has always had.
    //
    // The mask is put on *before* the surface is shown, so its very first
    // commit already excludes the strip. Applied after, the author could
    // outrace it: for a few frames the fresh surface took input over the
    // whole output, and a quick click on the next opener was spent dismissing
    // this one instead of reaching the bar — the two-click switch, back
    // again, but only for fast hands. The size connections keep the region
    // true across the configures that follow.
    if (m_openerPanel) {
        const QPointer<QWindow> tracked(overlay);
        const QPointer<QWindow> bar = m_openerPanel;
        const auto apply = [tracked, bar]() {
            if (!tracked || !bar)
                return;
            tracked->setMask(panelPopupInputRegion(
                tracked->width(), tracked->height(), qMax(0, bar->height())));
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

    if (!m_surface->open(overlay, screen)) {
        delete overlay;
        return;
    }

    m_attachmentLease.acquire(m_openerPanel, overlay, m_attachmentAnchor);
    emit contextualSurfaceOpened();
}

void OverlayController::close()
{
    m_surface->close();
    m_attachmentLease.release();
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
    if (isOpen()) {
        m_attachmentLease.release();
        QWindow *const window = m_surface->window();
        softCloseWindow(window, [this, window]() {
            if (m_surface->window() == window)
                close();
        });
    } else {
        open();
    }
}
