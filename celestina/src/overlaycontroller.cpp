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

void OverlayController::toggleFrom(QWindow *panel, const QRect &globalOpener)
{
    // The opener is remembered for the next window this controller builds, then
    // spent: a keybind that follows a click must not inherit the click's origin.
    m_opener = globalOpener;
    m_openerPanel = panel;
    toggle();
    m_opener = QRect();
    m_openerPanel = nullptr;
}

QWindow *OverlayController::createWindow()
{
    QVariantMap properties = initialProperties();
    if (!m_opener.isEmpty() && m_openerPanel && m_openerPanel->screen()) {
        const QPoint outputOrigin = m_openerPanel->screen()->geometry().topLeft();
        properties.insert(QStringLiteral("anchoredFromPanel"), true);
        properties.insert(
            QStringLiteral("openerRect"),
            panelPopupOpenerOnOutput(m_opener, outputOrigin)
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
    connect(window, SIGNAL(dismissed()), this, SLOT(close()));
    return window;
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

    if (!m_surface->open(overlay, screen))
        delete overlay;
}

void OverlayController::close()
{
    m_surface->close();
}

void OverlayController::toggle()
{
    if (isOpen())
        close();
    else
        open();
}
