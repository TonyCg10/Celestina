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
    , m_niri(niri)
    , m_surface(new PanelMenuSurface(this))
    , m_enabled(enabledByEnvironment())
{
    if (!m_enabled)
        return;

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

void PanelMenuController::close()
{
    m_surface->close();
}
