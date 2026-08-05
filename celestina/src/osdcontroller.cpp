#include "osdcontroller.h"

#include <QCursor>
#include <QDebug>
#include <QGuiApplication>
#include <QQmlEngine>
#include <QScreen>
#include <QVariantMap>
#include <QWindow>

#include "overlaysurface.h"
#include "shellprovidersclient.h"

namespace {
// Long enough to read a number, short enough that it is gone before it is in
// the way. Noctalia's own display used the same order of magnitude.
constexpr int visibleMs = 1800;
const char componentName[] = "SessionOsd";
} // namespace

OsdController::OsdController(
    QQmlEngine *engine,
    ShellProvidersClient *providers,
    QObject *parent
)
    : QObject(parent)
    , m_component(engine)
    , m_providers(providers)
    , m_surface(new OverlaySurface(OverlaySurface::Placement::Readout, this))
    , m_enabled(true)
{
    m_component.loadFromModule("CelestinaDesktop", QLatin1String(componentName));
    if (!m_component.isReady()) {
        qCritical().noquote()
            << "Celestina could not load its on-screen display:"
            << m_component.errorString();
        m_enabled = false;
    }

    m_hideTimer.setSingleShot(true);
    m_hideTimer.setInterval(visibleMs);
    connect(&m_hideTimer, &QTimer::timeout, this, &OsdController::hide);

    if (m_providers) {
        connect(
            m_providers,
            &ShellProvidersClient::changed,
            this,
            &OsdController::providersChanged
        );
    }
}

bool OsdController::isVisible() const
{
    return m_surface->isOpen();
}

void OsdController::providersChanged()
{
    if (!m_providers)
        return;

    if (!m_providers->available()) {
        // Nothing published can be compared with what a dead helper last said,
        // so the next value is a baseline rather than a change to announce.
        m_readings.forget();
        return;
    }

    const std::optional<OsdReadings::Reading> reading =
        m_readings.apply(m_providers->providers());
    if (!reading)
        return;

    show(*reading);
}

void OsdController::applyReading(
    QWindow *window,
    const OsdReadings::Reading &reading
)
{
    window->setProperty("kind", reading.kind);
    window->setProperty("percent", reading.percent);
    window->setProperty("muted", reading.muted);
    window->setProperty("label", reading.label);
}

QWindow *OsdController::createWindow(const OsdReadings::Reading &reading)
{
    const QVariantMap initialProperties {
        {QStringLiteral("kind"), reading.kind},
        {QStringLiteral("percent"), reading.percent},
        {QStringLiteral("muted"), reading.muted},
        {QStringLiteral("label"), reading.label},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    QObject *rootObject = m_component.createWithInitialProperties(initialProperties);
    if (!rootObject) {
        qCritical().noquote()
            << "Celestina could not create its on-screen display:"
            << m_component.errorString();
        return nullptr;
    }

    auto *window = qobject_cast<QWindow *>(rootObject);
    if (!window) {
        qCritical() << "Celestina's on-screen display component is not a window.";
        delete rootObject;
        return nullptr;
    }
    return window;
}

void OsdController::show(const OsdReadings::Reading &reading)
{
    if (!m_enabled)
        return;

    // A display that is already up is updated in place: remapping a surface per
    // wheel notch would make the corner flicker and ask the compositor for a
    // new surface several times a second.
    if (QWindow *const shown = m_surface->window()) {
        applyReading(shown, reading);
        m_hideTimer.start();
        return;
    }

    QWindow *const osd = createWindow(reading);
    if (!osd)
        return;

    // The display follows the pointer's output, like every other surface the
    // shell opens without a click position, and falls back to the primary
    // screen when the pointer sits nowhere a screen claims.
    QScreen *screen = QGuiApplication::screenAt(QCursor::pos());
    if (!screen)
        screen = QGuiApplication::primaryScreen();

    if (!m_surface->open(osd, screen)) {
        delete osd;
        return;
    }

    m_hideTimer.start();
}

void OsdController::hide()
{
    m_hideTimer.stop();
    m_surface->close();
}
