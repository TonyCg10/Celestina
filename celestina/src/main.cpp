#include <cstdio>
#include <cstdlib>

#include <QDebug>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QHash>
#include <QPointer>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQmlContext>
#include <QScreen>
#include <QStandardPaths>
#include <QTimer>
#include <QWindow>

#include <KWindowEffects>
#include <LayerShellQt/Window>

#include "devicesclient.h"

namespace {
constexpr int panelHeight = 40;
constexpr auto panelScope = "celestina-panel";

class PanelManager final : public QObject
{
public:
    PanelManager(QGuiApplication *application, QQmlEngine *engine)
        : QObject(application)
        , m_application(application)
        , m_component(engine)
    {
        m_component.loadFromModule("CelestinaDesktop", "Panel");
    }

    ~PanelManager() override
    {
        const auto panels = m_panels.values();
        m_panels.clear();

        for (const auto &panel : panels) {
            if (panel)
                delete panel.data();
        }
    }

    bool start()
    {
        if (!m_component.isReady()) {
            qCritical().noquote()
                << "Celestina could not load the panel component:"
                << m_component.errorString();
            return false;
        }

        QObject::connect(
            m_application,
            &QGuiApplication::screenAdded,
            this,
            [this](QScreen *screen) {
                const QPointer<QScreen> pendingScreen(screen);

                QTimer::singleShot(0, this, [this, pendingScreen] {
                    if (!pendingScreen
                        || !QGuiApplication::screens().contains(pendingScreen.data())) {
                        return;
                    }

                    if (!ensurePanel(pendingScreen.data())) {
                        qWarning() << "Celestina kept existing panels after failing "
                                      "to map a newly added output.";
                    }
                });
            }
        );

        QObject::connect(
            m_application,
            &QGuiApplication::screenRemoved,
            this,
            [this](QScreen *screen) { removePanel(screen); }
        );

        const auto screens = QGuiApplication::screens();
        if (screens.isEmpty())
            qInfo() << "Celestina is waiting for an output.";

        for (QScreen *screen : screens) {
            if (!ensurePanel(screen))
                return false;
        }

        return true;
    }

private:
    bool ensurePanel(QScreen *screen)
    {
        if (!screen)
            return false;

        const auto existingPanel = m_panels.value(screen);
        if (existingPanel)
            return true;

        m_panels.remove(screen);

        QObject *rootObject = m_component.create();
        if (!rootObject) {
            qCritical().noquote()
                << "Celestina could not create a panel for output"
                << screen->name() << m_component.errorString();
            return false;
        }

        auto *window = qobject_cast<QWindow *>(rootObject);
        if (!window) {
            qCritical() << "Celestina's panel component is not a window.";
            delete rootObject;
            return false;
        }

        window->setObjectName(
            QStringLiteral("celestina-panel-%1").arg(screen->name())
        );
        window->setScreen(screen);
        window->setFlag(Qt::FramelessWindowHint);
        window->setFlag(Qt::WindowDoesNotAcceptFocus);
        window->setHeight(panelHeight);

        auto *layerWindow = LayerShellQt::Window::get(window);
        if (!layerWindow) {
            qCritical() << "Celestina could not create a layer-shell surface for"
                        << screen->name();
            delete window;
            return false;
        }

        layerWindow->setScreen(screen);
        layerWindow->setScope(QString::fromLatin1(panelScope));

        auto anchors = LayerShellQt::Window::Anchors(
            LayerShellQt::Window::AnchorTop
        );
        anchors |= LayerShellQt::Window::AnchorLeft;
        anchors |= LayerShellQt::Window::AnchorRight;
        layerWindow->setAnchors(anchors);

        layerWindow->setDesiredSize(QSize(0, panelHeight));
        layerWindow->setExclusiveZone(panelHeight);
        layerWindow->setLayer(LayerShellQt::Window::LayerTop);
        layerWindow->setKeyboardInteractivity(
            LayerShellQt::Window::KeyboardInteractivityNone
        );
        layerWindow->setActivateOnShow(false);
        // The manager owns dismissal and screen removal. Keeping LayerShellQt
        // from closing the QWindow avoids a closed-but-still-tracked panel.
        layerWindow->setCloseOnDismissed(false);

        m_panels.insert(screen, window);
        QObject::connect(
            window,
            &QObject::destroyed,
            this,
            [this, screen, window] {
                auto panel = m_panels.find(screen);
                if (panel != m_panels.end()
                    && (panel.value().isNull() || panel.value().data() == window)) {
                    m_panels.erase(panel);
                }
            }
        );

        // Mapping last ensures the output and all layer-shell properties are
        // fixed before the compositor creates the surface.
        window->show();

        // The panel's glass: ask the compositor (niri's ext-background-effect)
        // to blur the wallpaper behind the translucent panel. Best-effort — a
        // compositor that does not implement it simply leaves the panel a plain
        // translucent tint, no worse than before.
        KWindowEffects::enableBlurBehind(window, true);

        qInfo() << "Celestina panel mapped on output" << screen->name()
                << "geometry" << screen->geometry()
                << "scale" << screen->devicePixelRatio();
        return true;
    }

    void removePanel(QScreen *screen)
    {
        const QPointer<QWindow> window = m_panels.take(screen);
        if (!window)
            return;

        qInfo() << "Celestina panel removed from output" << screen->name();
        window->hide();
        window->deleteLater();
    }

    QGuiApplication *m_application;
    QQmlComponent m_component;
    QHash<QScreen *, QPointer<QWindow>> m_panels;
};
}

// The session's "which screen do I share?" dialog.
//
// xdg-desktop-portal-wlr brings no dialog of its own: it runs a command and
// keeps whatever output name that command prints. Hosting that chooser here —
// rather than in a loose `qml` runtime — buys two things the session actually
// needs: a real stdout to answer on, and a stable Wayland app_id (`celestina`),
// which is what a window rule has to match on to float it instead of tiling it.
//
// It is only the chooser, not the portal. Serving ScreenCast itself waits for
// the Niri adapter (CP1) — capture belongs to whoever knows the outputs and the
// windows.
int runOutputChooser(QGuiApplication &app, QQmlEngine &engine)
{
    QQmlComponent component(&engine);
    component.loadFromModule("CelestinaDesktop", "OutputChooser");
    if (component.isError()) {
        for (const auto &error : component.errors())
            qWarning() << "celestina --pick-output:" << error.toString();
        return EXIT_FAILURE;
    }

    QScopedPointer<QObject> chooser(component.create());
    if (chooser.isNull()) {
        qWarning() << "celestina --pick-output: the chooser did not load";
        return EXIT_FAILURE;
    }

    int status = EXIT_FAILURE;
    // The window answers by setting `chosen` (or `cancelled`); xdpw's simple
    // chooser protocol expects `Monitor: <output name>` on stdout.
    QObject::connect(chooser.data(), SIGNAL(chosenChanged()), &app, SLOT(quit()));
    QObject::connect(chooser.data(), SIGNAL(cancelledChanged()), &app, SLOT(quit()));
    app.setQuitOnLastWindowClosed(true);
    app.exec();

    const QString chosen = chooser->property("chosen").toString();
    if (!chosen.isEmpty()) {
        std::fputs("Monitor: ", stdout);
        std::fputs(qPrintable(chosen), stdout);
        std::fputc('\n', stdout);
        std::fflush(stdout);
        status = EXIT_SUCCESS;
    }
    // Cancelling exits non-zero *without* a name: for the backend that reads as
    // "the user does not want to share", which is not the same as a failure.
    return status;
}

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);
    app.setApplicationName("celestina");
    app.setApplicationDisplayName("Celestina Desktop");
    app.setDesktopFileName("celestina");
    app.setOrganizationName("Celestina");
    app.setQuitOnLastWindowClosed(false);

    QQmlEngine engine;

    // Make CelestinaStyle importable from source. The style tree's directory is
    // named `celestina-style`, but its module URI is `CelestinaStyle`, so expose
    // it under that name via a runtime symlink and add the import path. Self-
    // provisioning here means the panel and the chooser both resolve the style
    // without a wrapper pre-setting QML_IMPORT_PATH.
    {
        QString runtime =
            QStandardPaths::writableLocation(QStandardPaths::RuntimeLocation);
        if (runtime.isEmpty())
            runtime = QDir::tempPath();
        const QString importRoot =
            QDir(runtime).filePath(QStringLiteral("celestina-shell-import"));
        QDir().mkpath(importRoot);
        const QString styleLink =
            importRoot + QStringLiteral("/CelestinaStyle");
        // Always relink — a stale link (an older checkout, another build) would
        // silently feed the shell the wrong CelestinaStyle. QFile::remove clears
        // the symlink itself, not its target, and is a no-op when absent.
        QFile::remove(styleLink);
        QFile::link(QStringLiteral(CELESTINA_STYLE_DIR), styleLink);
        engine.addImportPath(importRoot);
    }

    if (app.arguments().contains(QStringLiteral("--pick-output")))
        return runOutputChooser(app, engine);

    // The phone the panel draws. Parented to the app so it outlives the engine's
    // teardown; exposed to every panel through the shared root context.
    auto *phone = new DevicesClient(&app);
    engine.rootContext()->setContextProperty(QStringLiteral("Phone"), phone);

    PanelManager panels(&app, &engine);
    if (!panels.start())
        return EXIT_FAILURE;

    return app.exec();
}
