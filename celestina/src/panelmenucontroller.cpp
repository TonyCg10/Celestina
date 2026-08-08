#include "panelmenucontroller.h"

#include <QDebug>
#include <QQmlEngine>
#include <QScreen>
#include <QVariantMap>
#include <QWindow>

#include "niriclient.h"
#include "panelmenusurface.h"

namespace {
// Where the card goes inside a surface that now covers the whole output.
//
// The click supplies the control's horizontal global anchor. Vertically, a
// layer-shell client cannot know where the compositor finally placed a stacked
// panel. The menu surface therefore respects exclusive zones and its own top
// edge is already the panel's real bottom edge. The card is inset inside that
// surface by the room its shadow needs.
void placeCard(QWindow *card, QWindow *panel, const QPoint &globalAnchor)
{
    const int inset = card->property("shadowMargin").toInt();
    const QScreen *const screen = panel->screen();
    const QPoint outputOrigin = screen ? screen->geometry().topLeft() : QPoint();
    const QPoint origin = panelMenuOrigin(globalAnchor, outputOrigin, inset);

    card->setProperty("menuX", origin.x());
    card->setProperty("menuY", origin.y());
}
} // namespace

QPoint panelMenuOrigin(
    const QPoint &globalAnchor,
    const QPoint &outputOrigin,
    int shadowMargin
)
{
    return QPoint(
        globalAnchor.x() - outputOrigin.x() - shadowMargin,
        -shadowMargin
    );
}

PanelMenuController::PanelMenuController(
    QQmlEngine *engine,
    NiriClient *niri,
    QObject *parent
)
    : QObject(parent)
    , m_component(engine)
    , m_trayComponent(engine)
    , m_networkComponent(engine)
    , m_bluetoothComponent(engine)
    , m_niri(niri)
    , m_surface(new PanelMenuSurface(this))
    , m_enabled(enabledByEnvironment())
{
    if (!m_enabled)
        return;

    m_trayComponent.loadFromModule("CelestinaDesktop", "TrayMenu");
    // An indicator menu that will not load leaves its indicator inert rather
    // than opening an empty surface; the panel keeps working either way.
    m_networkComponent.loadFromModule("CelestinaDesktop", "NetworkMenu");
    m_bluetoothComponent.loadFromModule("CelestinaDesktop", "BluetoothMenu");
    for (const QQmlComponent *indicator : {&m_networkComponent, &m_bluetoothComponent}) {
        if (!indicator->isReady()) {
            qCritical().noquote()
                << "Celestina could not load an indicator menu:"
                << indicator->errorString();
        }
    }
    m_component.loadFromModule("CelestinaDesktop", "PanelMenu");
    if (!m_component.isReady()) {
        qCritical().noquote()
            << "Celestina could not load the panel menu:"
            << m_component.errorString();
        m_enabled = false;
    }
}

bool PanelMenuController::enabledByEnvironment()
{
    const QByteArray requested = qgetenv("CELESTINA_PANEL_MENU").trimmed().toLower();
    if (requested.isEmpty() || requested == "1" || requested == "true")
        return true;
    if (requested == "0" || requested == "false")
        return false;

    // An unreadable value is not a request to remove a working menu.
    qWarning().noquote()
        << "Celestina ignored an unreadable CELESTINA_PANEL_MENU:" << requested;
    return true;
}

QWindow *PanelMenuController::createMenuWindow(const QVariant &workspaces)
{
    const QVariantMap initialProperties {
        {QStringLiteral("workspaces"), workspaces},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    QObject *rootObject =
        m_component.createWithInitialProperties(initialProperties);
    if (!rootObject) {
        qCritical().noquote()
            << "Celestina could not create the panel menu:"
            << m_component.errorString();
        return nullptr;
    }

    auto *window = qobject_cast<QWindow *>(rootObject);
    if (!window) {
        qCritical() << "Celestina's panel menu component is not a window.";
        delete rootObject;
        return nullptr;
    }

    // The menu's signals are QML-declared, so they are reached by name; the
    // controller — not the delegate — is what talks to the provider.
    connect(window, SIGNAL(activated(QString, int)), this, SLOT(activate(QString, int)));
    connect(window, SIGNAL(dismissed()), this, SLOT(menuDismissed()));
    return window;
}

void PanelMenuController::activate(const QString &output, int index)
{
    if (m_niri)
        m_niri->requestWorkspaceFocus(output, index);

    // The answer arrives on the panel's own pill, where the request is
    // visible; the menu has said everything it can say.
    close();
}

void PanelMenuController::open(
    QWindow *panel,
    const QPoint &globalAnchor,
    const QVariant &workspaces
)
{
    if (!m_enabled || !panel || !m_niri)
        return;

    close();

    QWindow *const menu = createMenuWindow(workspaces);
    if (!menu)
        return;

    placeCard(menu, panel, globalAnchor);
    if (!m_surface->open(menu, panel))
        delete menu;
}

QString indicatorMenuComponent(const QString &kind)
{
    if (kind == QStringLiteral("network"))
        return QStringLiteral("NetworkMenu");
    if (kind == QStringLiteral("bluetooth"))
        return QStringLiteral("BluetoothMenu");

    return QString();
}

void PanelMenuController::toggleIndicatorMenu(
    QWindow *panel,
    const QPoint &globalAnchor,
    const QString &kind,
    QObject *providerSource
)
{
    if (!m_enabled || !panel || !providerSource)
        return;
    if (indicatorMenuComponent(kind).isEmpty()) {
        qWarning() << "Celestina has no indicator menu named" << kind;
        return;
    }

    // The same indicator again is a request to put it away. In a live session
    // the click never gets this far — the open surface covers the output and
    // answers it first — but a host that reopened here would resurrect the
    // defect where the first click did nothing visible and only the second
    // closed the menu.
    const bool sameAgain = (m_openIndicator == kind);
    close();
    if (sameAgain)
        return;

    QQmlComponent &component = kind == QStringLiteral("network")
        ? m_networkComponent
        : m_bluetoothComponent;
    if (!component.isReady())
        return;

    QObject *rootObject = component.createWithInitialProperties(QVariantMap {
        {QStringLiteral("providerSource"), QVariant::fromValue(providerSource)},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    });
    auto *window = qobject_cast<QWindow *>(rootObject);
    if (!window) {
        qCritical().noquote()
            << "Celestina could not create the" << kind
            << "menu:" << component.errorString();
        delete rootObject;
        return;
    }

    connect(window, SIGNAL(dismissed()), this, SLOT(menuDismissed()));

    placeCard(window, panel, globalAnchor);
    if (!m_surface->open(window, panel)) {
        delete window;
        return;
    }

    m_openIndicator = kind;
}

void PanelMenuController::requestTrayMenu(
    QWindow *panel,
    const QPoint &globalAnchor,
    const QString &service,
    const QString &path
)
{
    if (!m_enabled || !panel)
        return;

    close();
    m_pendingPanel = panel;
    m_pendingAnchor = globalAnchor;
    m_pendingService = service;
    m_pendingPath = path;
    emit trayMenuNeeded(service, path);
}

void PanelMenuController::trayMenuReady(
    const QString &service,
    const QString &path,
    const QVariantList &entries
)
{
    // An answer to a menu nobody is waiting for — a second right-click, or one
    // that arrived after the panel moved on — is not a menu to open.
    if (service != m_pendingService || path != m_pendingPath || !m_pendingPanel)
        return;
    if (entries.isEmpty())
        return;

    // The request is spent here. An application may answer the same
    // `AboutToShow` twice, or answer a second time seconds later; without
    // this the guard above lets that through, because it only rejects a
    // *different* item — and a menu the user already dismissed reopens itself
    // at an anchor that has since moved.
    QWindow *const panel = m_pendingPanel;
    const QPoint anchor = m_pendingAnchor;
    m_pendingService.clear();
    m_pendingPath.clear();
    m_pendingPanel = nullptr;

    QObject *rootObject = m_trayComponent.createWithInitialProperties(QVariantMap {
        {QStringLiteral("entries"), entries},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    });
    auto *window = qobject_cast<QWindow *>(rootObject);
    if (!window) {
        qCritical().noquote()
            << "Celestina could not create a tray menu:" << m_trayComponent.errorString();
        delete rootObject;
        return;
    }

    connect(window, SIGNAL(chosen(int)), this, SLOT(trayEntryChosen(int)));
    connect(window, SIGNAL(dismissed()), this, SLOT(menuDismissed()));

    placeCard(window, panel, anchor);
    if (!m_surface->open(window, panel)) {
        delete window;
        return;
    }

    m_openService = service;
    m_openPath = path;
}

void PanelMenuController::trayEntryChosen(int entryId)
{
    // Nothing is triggered once the menu has closed: `close()` forgets whose it
    // was, so a late choice reaches no application rather than the wrong one.
    if (m_openService.isEmpty() && m_openPath.isEmpty())
        return;

    emit trayEntryTriggered(m_openService, m_openPath, entryId);
    close();
}

void PanelMenuController::menuDismissed()
{
    if (sender() != m_surface->window())
        return;

    close();
}

void PanelMenuController::close()
{
    // Whatever was asked for is no longer wanted. A pending target that
    // outlives the menu is what lets a late answer open a surface the user
    // never asked for.
    m_pendingService.clear();
    m_pendingPath.clear();
    m_pendingPanel = nullptr;
    m_openService.clear();
    m_openPath.clear();
    m_openIndicator.clear();
    m_surface->close();
}
