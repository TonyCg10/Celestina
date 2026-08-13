#include "panelmanager.h"

#include "shellscale.h"

#include "diagnosticjournal.h"
#include "overlaycontroller.h"

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
#include "traywatcher.h"
#include "surfacemanager.h"

namespace {
constexpr int panelHeight = 40;
constexpr auto panelScope = "celestina-panel";
// How long the outputs must stay still before the DDC worker is told they
// changed. Enabling one monitor produces several `QScreen` events in a row and
// a person plugging in two produces more; every one of them costs the same
// single rediscovery, so they are worth waiting out. It is short against the
// five-minute refresh this exists to avoid waiting for, and long against a
// burst.
constexpr int outputsSettleMs = 1500;

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
    // The height is asked for in real output pixels, which is the bar's 40
    // design units times this output's factor. Asking for the unscaled number
    // had the compositor size the strip 40 px tall under a scene drawing 46:
    // the bottom of every welded capsule was clipped off, and the reserved
    // exclusive strip was short by the same amount, so the shell looked like a
    // veil that refused to wrap its own pills.
    const int scaledPanelHeight =
        qRound(panelHeight * shellScaleForScreen(screen));
    spec.desiredSize = QSize(0, scaledPanelHeight);
    spec.exclusiveZone = scaledPanelHeight;
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
    TrayWatcher *tray,
    PanelMenuController *menu,
    bool reducedMotion
)
    : QObject(application)
    , m_application(application)
    , m_component(engine)
    , m_niri(niri)
    , m_phone(phone)
    , m_providers(providers)
    , m_tray(tray)
    , m_menu(menu)
    , m_outputsSettled(new QTimer(this))
    , m_reducedMotion(reducedMotion)
{
    m_component.loadFromModule("CelestinaDesktop", "Panel");

    m_outputsSettled->setSingleShot(true);
    m_outputsSettled->setInterval(outputsSettleMs);
    connect(m_outputsSettled, &QTimer::timeout, this, [this] {
        if (!m_providers)
            return;

        // One request, whatever the burst was. The provider owns what it costs
        // and when it runs; this only says that looking again is worth it.
        m_providers->sendCommand(
            QStringLiteral("brightness"),
            QStringLiteral("outputs-changed")
        );
    });
}

void PanelManager::outputsChanged()
{
    m_outputsSettled->start();
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

void PanelManager::setNotificationCentre(OverlayController *centre)
{
    m_notificationCentre = centre;
}

void PanelManager::setLauncher(OverlayController *launcher)
{
    m_launcher = launcher;
}

void PanelManager::setControlCentre(OverlayController *centre)
{
    m_controlCentre = centre;
}

void PanelManager::setClipboard(OverlayController *clipboard)
{
    m_clipboard = clipboard;
}

void PanelManager::setSessionMenu(OverlayController *menu)
{
    m_sessionMenu = menu;
}

void PanelManager::togglePanelOverlay(
    OverlayController *controller,
    QWindow *panel,
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor
)
{
    if (!controller || !panel)
        return;

    controller->toggleFrom(panel, globalOpener, globalAttachmentAnchor);
}

void PanelManager::notificationCentreRequested(
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor
)
{
    togglePanelOverlay(
        m_notificationCentre,
        qobject_cast<QWindow *>(sender()),
        globalOpener,
        globalAttachmentAnchor
    );
}

void PanelManager::launcherRequested(
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor
)
{
    togglePanelOverlay(
        m_launcher,
        qobject_cast<QWindow *>(sender()),
        globalOpener,
        globalAttachmentAnchor
    );
}

void PanelManager::controlCentreRequested(
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor
)
{
    togglePanelOverlay(
        m_controlCentre,
        qobject_cast<QWindow *>(sender()),
        globalOpener,
        globalAttachmentAnchor
    );
}

void PanelManager::clipboardRequested(
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor
)
{
    togglePanelOverlay(
        m_clipboard,
        qobject_cast<QWindow *>(sender()),
        globalOpener,
        globalAttachmentAnchor
    );
}

void PanelManager::sessionMenuRequested(
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor
)
{
    togglePanelOverlay(
        m_sessionMenu,
        qobject_cast<QWindow *>(sender()),
        globalOpener,
        globalAttachmentAnchor
    );
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

                // A monitor that just arrived may have a brightness the worker
                // has not found; it is on a five-minute clock otherwise.
                outputsChanged();
            });
        }
    );

    QObject::connect(
        m_application,
        &QGuiApplication::screenRemoved,
        this,
        [this](QScreen *screen) {
            removePanel(screen);
            // A monitor leaving renumbers what `ddcutil` found as much as one
            // arriving does.
            outputsChanged();
        }
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
        // How much larger this output needs the shell drawn so it measures the
        // same as on every other one; see shellscale.h.
        {QStringLiteral("shellScale"), shellScaleForScreen(screen)},
        {QStringLiteral("niriProvider"), QVariant::fromValue(m_niri.data())},
        {QStringLiteral("phoneProvider"), QVariant::fromValue(m_phone.data())},
        {QStringLiteral("providerSource"), QVariant::fromValue(m_providers.data())},
        {QStringLiteral("traySource"), QVariant::fromValue(m_tray.data())},
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
            SIGNAL(workspaceMapRequested(QRectF, QRectF, QVariant)),
            this,
            SLOT(workspaceMapRequested(QRectF, QRectF, QVariant))
        );
        connect(
            window,
            SIGNAL(trayMenuRequested(QString, QString, int, int)),
            this,
            SLOT(trayMenuRequested(QString, QString, int, int))
        );
        connect(
            window,
            SIGNAL(trayDrawerRequested(QRectF, QRectF)),
            this,
            SLOT(trayDrawerRequested(QRectF, QRectF))
        );
        connect(
            window,
            SIGNAL(indicatorMenuRequested(QString, QRectF, QRectF)),
            this,
            SLOT(indicatorMenuRequested(QString, QRectF, QRectF))
        );
    }

    connect(
        window,
        SIGNAL(launcherRequested(QRectF, QRectF)),
        this,
        SLOT(launcherRequested(QRectF, QRectF))
    );
    connect(
        window,
        SIGNAL(notificationCentreRequested(QRectF, QRectF)),
        this,
        SLOT(notificationCentreRequested(QRectF, QRectF))
    );
    connect(
        window,
        SIGNAL(controlCentreRequested(QRectF, QRectF)),
        this,
        SLOT(controlCentreRequested(QRectF, QRectF))
    );
    connect(
        window,
        SIGNAL(clipboardRequested(QRectF, QRectF)),
        this,
        SLOT(clipboardRequested(QRectF, QRectF))
    );
    connect(
        window,
        SIGNAL(sessionMenuRequested(QRectF, QRectF)),
        this,
        SLOT(sessionMenuRequested(QRectF, QRectF))
    );
    connect(
        window,
        SIGNAL(wallpaperFolderSelected(QUrl)),
        this,
        SLOT(wallpaperFolderSelected(QUrl))
    );

    // The tray crosses C++, Qt's property notifier and a QML layout before it
    // becomes pixels. Record the last seam as well as the D-Bus seam so a
    // future disappearance says whether the model failed to arrive or merely
    // became zero-sized/hidden in presentation. Counts and geometry are
    // technical state; no application titles enter the diagnostic journal.
    if (m_tray) {
        const QString panelOutput = screen->name();
        const auto recordTrayPresentation = [window, panelOutput]() {
            QObject *drawer = window->findChild<QObject *>(
                QStringLiteral("celestina-tray-drawer")
            );
            auto record = CELESTINA_JOURNAL(Debug, "tray.presentation")
                              .text(QStringLiteral("output"), panelOutput)
                              .flag(QStringLiteral("drawer_found"), drawer != nullptr);
            if (drawer) {
                record.number(
                    QStringLiteral("item_count"),
                    drawer->property("items").toList().size()
                );
                record.flag(QStringLiteral("visible"), drawer->property("visible").toBool());
                record.number(QStringLiteral("width"), drawer->property("width").toLongLong());
                record.number(
                    QStringLiteral("implicit_width"),
                    drawer->property("implicitWidth").toLongLong()
                );
            }
            DiagnosticJournal::instance().record(record);
        };
        connect(m_tray, &TrayWatcher::changed, window, [window, recordTrayPresentation]() {
            QTimer::singleShot(0, window, recordTrayPresentation);
        });
        QTimer::singleShot(0, window, recordTrayPresentation);
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

void PanelManager::trayDrawerRequested(
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor
)
{
    auto *panel = qobject_cast<QWindow *>(sender());
    if (!panel || !m_menu || !m_tray)
        return;

    m_menu->toggleTrayItemsMenu(
        panel,
        globalOpener,
        globalAttachmentAnchor,
        m_tray,
        m_providers
    );
}

void PanelManager::wallpaperFolderSelected(const QUrl &source)
{
    auto *panel = qobject_cast<QWindow *>(sender());
    if (!panel || !m_providers || !source.isLocalFile())
        return;

    const QString path = source.toLocalFile();
    if (path.isEmpty())
        return;

    m_providers->requests()->send(
        QStringLiteral("wallpaper-gallery"),
        QStringLiteral("set-folder"),
        QVariantMap {
            {QStringLiteral("source"), path},
        },
        QStringLiteral("folder"),
        RequestLedger::ImmediatePolicy
    );
}

void PanelManager::workspaceMapRequested(
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor,
    const QVariant &workspaces
)
{
    auto *panel = qobject_cast<QWindow *>(sender());
    if (!panel || !m_menu)
        return;

    m_menu->openWorkspaceMap(
        panel, globalOpener, globalAttachmentAnchor, workspaces);
}

void PanelManager::indicatorMenuRequested(
    const QString &kind,
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor
)
{
    auto *panel = qobject_cast<QWindow *>(sender());
    if (!panel || !m_menu || !m_providers)
        return;

    // The phone menu's provider is Magnetita's D-Bus client, not the aggregate
    // helper bridge: its list, and the three actions on a row, live there.
    QObject *source = m_providers;
    if (kind == QStringLiteral("phone"))
        source = m_phone;

    m_menu->toggleIndicatorMenu(
        panel,
        globalOpener,
        globalAttachmentAnchor,
        kind,
        source
    );
}

void PanelManager::trayMenuRequested(
    const QString &service,
    const QString &path,
    int globalX,
    int globalY
)
{
    auto *panel = qobject_cast<QWindow *>(sender());
    if (!panel || !m_menu)
        return;

    m_menu->requestTrayMenu(panel, QPoint(globalX, globalY), service, path);
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
