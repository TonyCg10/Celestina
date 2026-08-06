#include "panelmenucontroller.h"

#include <QDebug>
#include <QQmlEngine>
#include <QVariantMap>
#include <QWindow>

#include "niriclient.h"
#include "panelmenusurface.h"

PanelMenuController::PanelMenuController(
    QQmlEngine *engine,
    NiriClient *niri,
    QObject *parent
)
    : QObject(parent)
    , m_component(engine)
    , m_trayComponent(engine)
    , m_niri(niri)
    , m_surface(new PanelMenuSurface(this))
    , m_enabled(enabledByEnvironment())
{
    if (!m_enabled)
        return;

    m_trayComponent.loadFromModule("CelestinaDesktop", "TrayMenu");
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
    connect(window, SIGNAL(dismissed()), this, SLOT(close()));
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

    // The card is inset inside its surface by the room its shadow needs, and
    // the menu belongs under the panel rather than under the cursor: the click
    // chooses the column, the panel's own edge chooses the line.
    const int inset = menu->property("shadowMargin").toInt();
    const QPoint anchor(
        globalAnchor.x() - inset,
        panel->geometry().bottom() + 1 - inset
    );

    if (!m_surface->open(menu, panel, anchor))
        delete menu;
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
    connect(window, SIGNAL(dismissed()), this, SLOT(close()));

    const int inset = window->property("shadowMargin").toInt();
    const QPoint placement(
        anchor.x() - inset,
        panel->geometry().bottom() + 1 - inset
    );
    if (!m_surface->open(window, panel, placement)) {
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
    m_surface->close();
}
