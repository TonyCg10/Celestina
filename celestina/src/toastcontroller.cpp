#include "toastcontroller.h"

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
const char componentName[] = "ToastStack";
} // namespace

ToastController::ToastController(
    QQmlEngine *engine,
    ShellProvidersClient *providers,
    QObject *parent
)
    : QObject(parent)
    , m_component(engine)
    , m_providers(providers)
    , m_surface(new OverlaySurface(OverlaySurface::Placement::Corner, this))
    , m_enabled(true)
{
    m_component.loadFromModule("CelestinaDesktop", QLatin1String(componentName));
    if (!m_component.isReady()) {
        qCritical().noquote()
            << "Celestina could not load its toast stack:"
            << m_component.errorString();
        m_enabled = false;
    }

    if (m_providers) {
        connect(
            m_providers,
            &ShellProvidersClient::changed,
            this,
            &ToastController::providersChanged
        );
    }
}

bool ToastController::isVisible() const
{
    return m_surface->isOpen();
}

void ToastController::providersChanged()
{
    if (!m_providers)
        return;

    // A helper that went away is showing nothing, whatever was on screen when
    // it left. The server is the only thing that knows what is still live.
    const QVariantMap published =
        m_providers->available()
        ? m_providers->providers().value(QStringLiteral("notifications")).toMap()
        : QVariantMap();
    const QVariantList toasts = published.value(QStringLiteral("toasts")).toList();

    if (toasts.isEmpty()) {
        hide();
        return;
    }
    // The actions travel beside the notifications, not inside them; the surface
    // joins them by id.
    show(toasts, published.value(QStringLiteral("actions")).toList());
}

QWindow *ToastController::createWindow(
    const QVariantList &toasts,
    const QVariantList &actions
)
{
    const QVariantMap initialProperties {
        {QStringLiteral("toasts"), toasts},
        {QStringLiteral("actions"), actions},
        {QStringLiteral("providerSource"), QVariant::fromValue(m_providers.data())},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    QObject *rootObject = m_component.createWithInitialProperties(initialProperties);
    if (!rootObject) {
        qCritical().noquote()
            << "Celestina could not create its toast stack:"
            << m_component.errorString();
        return nullptr;
    }

    auto *window = qobject_cast<QWindow *>(rootObject);
    if (!window) {
        qCritical() << "Celestina's toast stack component is not a window.";
        delete rootObject;
        return nullptr;
    }
    return window;
}

void ToastController::show(const QVariantList &toasts, const QVariantList &actions)
{
    if (!m_enabled)
        return;

    // Already up: the list is replaced in place. Remapping the surface for
    // every arrival would make the corner flicker and would lose whatever the
    // person was reading.
    if (QWindow *const shown = m_surface->window()) {
        shown->setProperty("toasts", toasts);
        shown->setProperty("actions", actions);
        return;
    }

    QWindow *const stack = createWindow(toasts, actions);
    if (!stack)
        return;

    // The corner of the output the pointer is on, like every other surface the
    // shell opens without a click position.
    QScreen *screen = QGuiApplication::screenAt(QCursor::pos());
    if (!screen)
        screen = QGuiApplication::primaryScreen();

    if (!m_surface->open(stack, screen))
        delete stack;
}

void ToastController::hide()
{
    m_surface->close();
}
