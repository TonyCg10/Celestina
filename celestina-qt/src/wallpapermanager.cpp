#include "wallpapermanager.h"

#include <QDebug>
#include <QGuiApplication>
#include <QQmlEngine>
#include <QScreen>
#include <QUrl>
#include <QVariantList>
#include <QVariantMap>
#include <QWindow>

#include "shellprovidersclient.h"
#include "surfacemanager.h"
#include "wallpaperidentity.h"

namespace {
const char componentName[] = "Wallpaper";

void setWallpaperIdentity(
    QWindow *surface,
    const QString &source,
    const QSize &geometry,
    const std::optional<WallpaperIdentityReading> &reading
)
{
    if (!surface)
        return;

    // QUrl owns local-file escaping. In particular, a space or `#` is path
    // data, never an accidental fragment delimiter assembled by QML.
    surface->setProperty("source", source);
    surface->setProperty(
        "sourceUrl",
        source.isEmpty() ? QUrl() : QUrl::fromLocalFile(source)
    );
    // Revision and inventory generation remain part of the QML image request.
    // A changed file at the same path therefore becomes a distinct request.
    surface->setProperty(
        "sourceRevision",
        reading && reading->source == source ? reading->revision : QString()
    );
    surface->setProperty(
        "sourceGeneration",
        QVariant::fromValue<qulonglong>(
            reading && reading->source == source ? reading->generation : 0
        )
    );
    surface->setProperty("sourceWidth", geometry.width());
    surface->setProperty("sourceHeight", geometry.height());
}
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
        {QStringLiteral("sourceUrl"), QUrl()},
        {QStringLiteral("sourceRevision"), QString()},
        {QStringLiteral("sourceGeneration"), QVariant::fromValue<qulonglong>(0)},
        {QStringLiteral("sourceWidth"), screen->geometry().width()},
        {QStringLiteral("sourceHeight"), screen->geometry().height()},
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
    connect(screen, &QScreen::geometryChanged, this, [this] {
        publishOutputs();
        // The old crop no longer describes the image request this surface
        // presents. Clear it while the worker publishes the new identity.
        applyChoices();
    });
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
    QVariantList geometries;
    for (auto entry = m_surfaces.constBegin(); entry != m_surfaces.constEnd(); ++entry) {
        outputs.append(entry.key()->name());
        geometries.append(QVariantMap {
            {QStringLiteral("output"), entry.key()->name()},
            {QStringLiteral("width"), entry.key()->geometry().width()},
            {QStringLiteral("height"), entry.key()->geometry().height()},
        });
    }

    // The helper cannot see the compositor's outputs; it is told, through the
    // one command channel, rather than given a second way to find out.
    m_providers->sendCommand(
        QStringLiteral("wallpaper"),
        QStringLiteral("set-outputs"),
        QVariantMap {
            {QStringLiteral("outputs"), outputs},
            {QStringLiteral("output-geometries"), geometries},
        }
    );
}

void WallpaperManager::applyChoices()
{
    if (!m_providers)
        return;

    // The helper cannot answer a command it never received, and it never
    // received the first one: `start()` publishes the outputs immediately after
    // the client is constructed, and `QProcess::start` is asynchronous, so the
    // helper is still `Starting` and `sendCommand` drops the line and returns 0.
    // Nothing asked again — the outputs are only republished when a screen is
    // added or removed — so a session whose monitors never change showed its
    // fallback for ever. It looked like a decode failure and was a lost request.
    //
    // A provider that has published nothing at all is that state, and it is also
    // what a restarted helper looks like, so the same line repairs both. Once it
    // has published anything, including an output it has no image for, this
    // stops asking.
    if (!m_providers->providers().contains(QStringLiteral("wallpaper"))) {
        for (auto entry = m_surfaces.constBegin(); entry != m_surfaces.constEnd(); ++entry) {
            QWindow *const surface = entry.value();
            QScreen *const screen = entry.key();
            if (!surface || !screen)
                continue;

            setWallpaperIdentity(
                surface,
                surface->property("source").toString(),
                screen->geometry().size(),
                std::nullopt
            );
        }
        publishOutputs();
        return;
    }

    const QVariantMap providers = m_providers->providers();
    const QVariantMap chosen =
        providers.value(QStringLiteral("wallpaper")).toMap();
    for (auto entry = m_surfaces.constBegin(); entry != m_surfaces.constEnd(); ++entry) {
        QWindow *const surface = entry.value();
        QScreen *const screen = entry.key();
        if (!surface || !screen)
            continue;

        // An output the provider says nothing about shows its fallback, which
        // is what an empty path means to the surface. It never inherits
        // another output's picture.
        const QString output = screen->name();
        const QString path = chosen.value(output).toString();
        const QSize geometry = screen->geometry().size();
        std::optional<WallpaperIdentityReading> reading =
            wallpaperIdentityForOutput(providers, output, geometry);
        if (reading && reading->source != path)
            reading.reset();
        setWallpaperIdentity(surface, path, geometry, reading);
    }
}
