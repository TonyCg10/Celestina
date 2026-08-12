#include "overlaycontroller.h"

#include <QCursor>
#include <QDebug>
#include <QHash>
#include <QGuiApplication>
#include <QQmlEngine>
#include <QScreen>
#include <QVariantMap>
#include <QWindow>

#include "overlaysurface.h"
#include "panelpopupplacement.h"
#include "shellscale.h"

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
    , m_surface(new OverlaySurface(OverlaySurface::Placement::Centered, this))
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
    if (sender() == m_surface->window())
        close();
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

    if (!m_surface->open(overlay, screen)) {
        delete overlay;
        return;
    }

    m_attachmentLease.acquire(m_openerPanel, overlay, m_attachmentAnchor);
}

void OverlayController::close()
{
    m_surface->close();
    m_attachmentLease.release();
}

void OverlayController::toggle()
{
    if (isOpen())
        close();
    else
        open();
}
