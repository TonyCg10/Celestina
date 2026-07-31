#include "panelmanager.h"

#include <QDebug>
#include <QGuiApplication>
#include <QQmlEngine>
#include <QPoint>
#include <QRect>
#include <QScreen>
#include <QSize>
#include <QTimer>
#include <QVariantMap>
#include <QWindow>

#include "devicesclient.h"
#include "niriclient.h"
#include "panelblurcontroller.h"
#include "panelmenucontroller.h"
#include "shellprovidersclient.h"
#include "surfacemanager.h"

namespace {
constexpr int panelHeight = 40;
constexpr auto panelScope = "celestina-panel";

LayerSurfaceSpec panelSpec(QScreen *screen)
{
    auto anchors = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    anchors |= LayerShellQt::Window::AnchorLeft;
    anchors |= LayerShellQt::Window::AnchorRight;

    LayerSurfaceSpec spec;
    spec.scope = QString::fromLatin1(panelScope);
    spec.screen = screen;
    spec.anchors = anchors;
    // Full width, fixed height: the compositor owns the axis the panel spans.
    spec.desiredSize = QSize(0, panelHeight);
    spec.exclusiveZone = panelHeight;
    spec.layer = LayerShellQt::Window::LayerTop;
    spec.keyboard = LayerShellQt::Window::KeyboardInteractivityNone;
    spec.activateOnShow = false;
    // The manager owns dismissal and screen removal. Keeping LayerShellQt from
    // closing the QWindow avoids a closed-but-still-tracked panel.
    spec.closeOnDismissed = false;
    spec.acceptsFocus = false;
    return spec;
}
}

PanelManager::PanelManager(
    QGuiApplication *application,
    QQmlEngine *engine,
    NiriClient *niri,
    DevicesClient *phone,
    ShellProvidersClient *providers,
    PanelMenuController *menu,
    bool reducedMotion
)
    : QObject(application)
    , m_application(application)
    , m_component(engine)
    , m_niri(niri)
    , m_phone(phone)
    , m_providers(providers)
    , m_menu(menu)
    , m_reducedMotion(reducedMotion)
{
    m_component.loadFromModule("CelestinaDesktop", "Panel");
}

PanelManager::~PanelManager()
{
    const auto panels = m_panels.values();
    m_panels.clear();

    for (const auto &panel : panels) {
        if (panel)
            delete panel.data();
    }
}

bool PanelManager::start()
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

bool PanelManager::ensurePanel(QScreen *screen)
{
    if (!screen)
        return false;

    const auto existingPanel = m_panels.value(screen);
    if (existingPanel)
        return true;

    m_panels.remove(screen);

    const QVariantMap initialProperties {
        {QStringLiteral("outputName"), screen->name()},
        {QStringLiteral("reducedMotion"), m_reducedMotion},
        {QStringLiteral("niriProvider"), QVariant::fromValue(m_niri.data())},
        {QStringLiteral("phoneProvider"), QVariant::fromValue(m_phone.data())},
        {QStringLiteral("providerSource"), QVariant::fromValue(m_providers.data())},
    };
    QObject *rootObject = m_component.createWithInitialProperties(
        initialProperties
    );
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

    // The panel window asks; the host decides whether any surface answers.
    // The signal is QML-declared, so it is connected by name.
    if (m_menu) {
        connect(
            window,
            SIGNAL(contextMenuRequested(int, int, QVariant)),
            this,
            SLOT(panelMenuRequested(int, int, QVariant))
        );
    }

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

    if (!mapLayerSurface(window, panelSpec(screen))) {
        qCritical() << "Celestina could not create a layer-shell surface for"
                    << screen->name();
        m_panels.remove(screen);
        delete window;
        return false;
    }

    // The controller owns retries, geometry changes and the QML fallback
    // state. It is parented to the window, so no callback survives removal.
    auto *blurController = new PanelBlurController(window, window);
    blurController->start();

    qInfo() << "Celestina panel mapped on output" << screen->name()
            << "geometry" << screen->geometry()
            << "scale" << screen->devicePixelRatio();
    return true;
}

void PanelManager::panelMenuRequested(
    int globalX,
    int globalY,
    const QVariant &workspaces
)
{
    auto *panel = qobject_cast<QWindow *>(sender());
    if (!panel || !m_menu)
        return;

    m_menu->open(panel, QPoint(globalX, globalY), workspaces);
}

void PanelManager::removePanel(QScreen *screen)
{
    const QPointer<QWindow> window = m_panels.take(screen);
    if (!window)
        return;

    qInfo() << "Celestina panel removed from output" << screen->name();
    window->hide();
    window->deleteLater();
}
