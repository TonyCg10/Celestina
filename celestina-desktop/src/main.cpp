#include <cstdlib>

#include <QDebug>
#include <QGuiApplication>
#include <QHash>
#include <QPointer>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QScreen>
#include <QTimer>
#include <QWindow>

#include <LayerShellQt/Window>

namespace {
constexpr int panelHeight = 40;
constexpr auto panelScope = "celestina-desktop-panel";

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

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);
    app.setApplicationName("celestina-desktop");
    app.setApplicationDisplayName("Celestina Desktop");
    app.setDesktopFileName("celestina-desktop");
    app.setOrganizationName("Celestina");
    app.setQuitOnLastWindowClosed(false);

    QQmlEngine engine;
    PanelManager panels(&app, &engine);
    if (!panels.start())
        return EXIT_FAILURE;

    return app.exec();
}
