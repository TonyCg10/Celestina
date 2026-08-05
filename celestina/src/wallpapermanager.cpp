#include "wallpapermanager.h"

#include <QDebug>
#include <QGuiApplication>
#include <QQmlEngine>
#include <QScreen>
#include <QVariantMap>
#include <QWindow>

#include "shellprovidersclient.h"
#include "surfacemanager.h"

namespace {
const char componentName[] = "Wallpaper";
} // namespace

LayerSurfaceSpec wallpaperSurfaceSpec(QScreen *screen)
{
    LayerSurfaceSpec spec;
    spec.scope = QStringLiteral("celestina-wallpaper");
    spec.screen = screen;
    // Anchored on all four edges: the compositor sizes it to the output, so a
    // resolution change is the compositor's business rather than a geometry
    // this shell has to recompute.
    auto anchors = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    anchors |= LayerShellQt::Window::AnchorBottom;
    anchors |= LayerShellQt::Window::AnchorLeft;
    anchors |= LayerShellQt::Window::AnchorRight;
    spec.anchors = anchors;
    spec.layer = LayerShellQt::Window::LayerBackground;
    // A wallpaper reserves nothing: it is what everything else sits on.
    spec.exclusiveZone = -1;
    spec.keyboard = LayerShellQt::Window::KeyboardInteractivityNone;
    spec.activateOnShow = false;
    // The compositor never dismisses a background surface; if it did, the
    // window must stay tracked so the screen is not left with a hole.
    spec.closeOnDismissed = false;
    spec.acceptsFocus = false;
    return spec;
}

WallpaperManager::WallpaperManager(
    QGuiApplication *application,
    QQmlEngine *engine,
    ShellProvidersClient *providers,
    bool reducedMotion,
    QObject *parent
)
    : QObject(parent)
    , m_component(engine)
    , m_providers(providers)
    , m_reducedMotion(reducedMotion)
    , m_enabled(true)
{
    m_component.loadFromModule("CelestinaDesktop", QLatin1String(componentName));
    if (!m_component.isReady()) {
        qCritical().noquote()
            << "Celestina could not load its wallpaper surface:"
            << m_component.errorString();
        m_enabled = false;
    }

    if (application) {
        connect(application, &QGuiApplication::screenAdded, this, [this](QScreen *screen) {
            addScreen(screen);
            publishOutputs();
        });
        connect(application, &QGuiApplication::screenRemoved, this, [this](QScreen *screen) {
            removeScreen(screen);
            publishOutputs();
        });
    }

    if (m_providers) {
        connect(
            m_providers,
            &ShellProvidersClient::changed,
            this,
            &WallpaperManager::applyChoices
        );
    }
}

WallpaperManager::~WallpaperManager()
{
    for (QWindow *const surface : std::as_const(m_surfaces)) {
        if (surface) {
            surface->hide();
            surface->deleteLater();
        }
    }
    m_surfaces.clear();
}

bool WallpaperManager::start()
{
    if (!m_enabled)
        return false;

    for (QScreen *const screen : QGuiApplication::screens())
        addScreen(screen);

    publishOutputs();
    applyChoices();
    return !m_surfaces.isEmpty();
}

void WallpaperManager::addScreen(QScreen *screen)
{
    if (!m_enabled || !screen || m_surfaces.contains(screen))
        return;

    const QVariantMap initialProperties {
        {QStringLiteral("source"), QString()},
        {QStringLiteral("outputName"), screen->name()},
        {QStringLiteral("reducedMotion"), m_reducedMotion},
    };
    QObject *rootObject = m_component.createWithInitialProperties(initialProperties);
    auto *window = qobject_cast<QWindow *>(rootObject);
    if (!window) {
        qCritical().noquote()
            << "Celestina could not create a wallpaper surface:"
            << m_component.errorString();
        delete rootObject;
        return;
    }

    window->setScreen(screen);
    if (!mapLayerSurface(window, wallpaperSurfaceSpec(screen))) {
        qWarning() << "Celestina could not map a wallpaper on" << screen->name();
        delete window;
        return;
    }

    m_surfaces.insert(screen, window);
}

void WallpaperManager::removeScreen(QScreen *screen)
{
    QWindow *const surface = m_surfaces.take(screen);
    if (!surface)
        return;

    surface->hide();
    surface->deleteLater();
}

void WallpaperManager::publishOutputs()
{
    if (!m_providers)
        return;

    QVariantList outputs;
    for (auto entry = m_surfaces.constBegin(); entry != m_surfaces.constEnd(); ++entry)
        outputs.append(entry.key()->name());

    // The helper cannot see the compositor's outputs; it is told, through the
    // one command channel, rather than given a second way to find out.
    m_providers->sendCommand(
        QStringLiteral("wallpaper"),
        QStringLiteral("set-outputs"),
        QVariantMap {{QStringLiteral("outputs"), outputs}}
    );
}

void WallpaperManager::applyChoices()
{
    if (!m_providers)
        return;

    const QVariantMap chosen =
        m_providers->providers().value(QStringLiteral("wallpaper")).toMap();
    for (auto entry = m_surfaces.constBegin(); entry != m_surfaces.constEnd(); ++entry) {
        QWindow *const surface = entry.value();
        if (!surface)
            continue;

        // An output the provider says nothing about shows its fallback, which
        // is what an empty path means to the surface. It never inherits
        // another output's picture.
        const QString path = chosen.value(entry.key()->name()).toString();
        surface->setProperty("source", path);
    }
}
