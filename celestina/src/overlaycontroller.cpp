#include "overlaycontroller.h"

#include <QCursor>
#include <QDebug>
#include <QGuiApplication>
#include <QQmlEngine>
#include <QScreen>
#include <QVariantMap>
#include <QWindow>

#include "overlaysurface.h"
#include "shellprovidersclient.h"

OverlayController::OverlayController(
    QQmlEngine *engine,
    ShellProvidersClient *providers,
    const QString &qmlComponentName,
    QObject *parent
)
    : QObject(parent)
    , m_component(engine)
    , m_providers(providers)
    , m_componentName(qmlComponentName)
    , m_surface(new OverlaySurface(this))
    , m_enabled(true)
{
    m_component.loadFromModule("CelestinaDesktop", m_componentName);
    if (!m_component.isReady()) {
        qCritical().noquote() << "Celestina could not load its" << m_componentName
                               << "overlay:" << m_component.errorString();
        m_enabled = false;
    }
}

bool OverlayController::isOpen() const
{
    return m_surface->isOpen();
}

QWindow *OverlayController::createWindow()
{
    const QVariantMap initialProperties {
        {QStringLiteral("providerSource"), QVariant::fromValue(m_providers.data())},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    QObject *rootObject = m_component.createWithInitialProperties(initialProperties);
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
    if (!m_enabled || !m_providers || isOpen())
        return;

    QWindow *const overlay = createWindow();
    if (!overlay)
        return;

    // A keybind names no click position; the overlay follows the pointer's
    // output the way a launcher is expected to, and falls back to the primary
    // screen when the pointer sits nowhere a screen claims (a session with no
    // outputs left).
    QScreen *screen = QGuiApplication::screenAt(QCursor::pos());
    if (!screen)
        screen = QGuiApplication::primaryScreen();

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
