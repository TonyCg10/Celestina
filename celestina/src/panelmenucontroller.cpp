#include "panelmenucontroller.h"

#include <QDebug>
#include <QQmlEngine>
#include <QScreen>
#include <QTimer>
#include <QVariantMap>
#include <QWindow>

#include "niriclient.h"
#include "panelpopupplacement.h"
#include "panelmenusurface.h"

namespace {
// Where the card goes inside a surface that now covers the whole output.
//
// The surface covers the output, so both coordinates come from the real panel
// control. This is what lets a soft menu sit immediately beneath a moved,
// resized or stacked panel instead of inheriting a fixed top edge.
void placeCardOnOutput(QWindow *card, const QPoint &bodyOrigin)
{
    card->setProperty("menuX", bodyOrigin.x());
    card->setProperty("menuY", bodyOrigin.y());
}

void placeCard(QWindow *card, QWindow *panel, const QPoint &globalBodyOrigin)
{
    const QScreen *const screen = panel->screen();
    const QPoint outputOrigin = screen ? screen->geometry().topLeft() : QPoint();
    const QPoint origin = globalBodyOrigin - outputOrigin;

    card->setProperty("menuX", origin.x());
    card->setProperty("menuY", origin.y());
}

QVariantMap menuOutputProperties(QWindow *panel)
{
    const QString output = panel && panel->screen() ? panel->screen()->name() : QString();
    return QVariantMap {
        {QStringLiteral("outputName"), output},
    };
}

void capCardHeightBelowAnchor(
    QWindow *card,
    QWindow *panel,
    const QPoint &globalAnchor
)
{
    const QScreen *const screen = panel ? panel->screen() : nullptr;
    if (!card || !screen)
        return;

    // A foreign application can publish the full bounded DBusMenu inventory.
    // Keep that natural height for Menu's internal ListView, but make the card
    // itself a viewport no taller than the output space below the real request.
    // Using the complete output height would force an overflowing card to y=0
    // when the layer-surface clamp runs, detaching it from its invoking tile.
    // Without a cap, the child surface instead adopts the complete model height,
    // so there is nothing for Qt to scroll and lower actions are off-screen.
    const QRect output = screen->geometry();
    const int outputHeight = qMax(1, output.height());
    const int requestedTop = qBound(
        0,
        globalAnchor.y() - output.top(),
        outputHeight - 1
    );
    const int minimumViewportHeight = qBound(
        1,
        card->property("minimumMenuViewportHeight").toInt(),
        outputHeight
    );
    card->setProperty(
        "maximumContentHeight",
        qBound(
            minimumViewportHeight,
            outputHeight - requestedTop,
            outputHeight
        )
    );
}
} // namespace

QPoint adjacentTrayMenuOrigin(
    const QRect &parentCard,
    const QPoint &requestedAnchor,
    const QSize &childSize,
    const QSize &outputSize,
    int gap
)
{
    const int safeGap = qMax(0, gap);
    const int childWidth = qMax(0, childSize.width());
    const int childHeight = qMax(0, childSize.height());
    const int maximumX = qMax(0, outputSize.width() - childWidth);
    const int maximumY = qMax(0, outputSize.height() - childHeight);

    const int rightOrigin = parentCard.right() + 1 + safeGap;
    const int leftOrigin = parentCard.left() - safeGap - childWidth;
    const int rightRoom = outputSize.width() - rightOrigin;
    const int leftRoom = parentCard.left() - safeGap;

    int requestedX = rightOrigin;
    if (rightRoom < childWidth && leftRoom >= childWidth)
        requestedX = leftOrigin;
    else if (rightRoom < childWidth && leftRoom < childWidth)
        requestedX = leftRoom > rightRoom ? leftOrigin : rightOrigin;

    return QPoint(
        qBound(0, requestedX, maximumX),
        qBound(0, requestedAnchor.y(), maximumY)
    );
}

PanelMenuController::PanelMenuController(
    QQmlEngine *engine,
    NiriClient *niri,
    QObject *parent
)
    : QObject(parent)
    , m_trayComponent(engine)
    , m_trayItemsComponent(engine)
    , m_networkComponent(engine)
    , m_bluetoothComponent(engine)
    , m_performanceComponent(engine)
    , m_captureComponent(engine)
    , m_wallpaperComponent(engine)
    , m_workspaceMapComponent(engine)
    , m_niri(niri)
    , m_surface(new PanelMenuSurface(this))
    , m_trayChildSurface(new PanelMenuSurface(this))
    , m_enabled(enabledByEnvironment())
{
    // A layer-shell dismissal can hide the carrier without first closing its
    // QML Menu. Keep controller identity in sync with both lifecycle paths.
    connect(m_surface, &PanelMenuSurface::dismissed, this, [this]() {
        close();
    });
    connect(m_trayChildSurface, &PanelMenuSurface::dismissed, this, [this]() {
        closeTrayChild(true);
    });

    if (!m_enabled)
        return;

    m_trayComponent.loadFromModule("CelestinaDesktop", "TrayMenu");
    m_trayItemsComponent.loadFromModule("CelestinaDesktop", "TrayItemsMenu");
    if (!m_trayItemsComponent.isReady()) {
        qCritical().noquote()
            << "Celestina could not load the tray items menu:"
            << m_trayItemsComponent.errorString();
    }
    // An indicator menu that will not load leaves its indicator inert rather
    // than opening an empty surface; the panel keeps working either way.
    m_networkComponent.loadFromModule("CelestinaDesktop", "NetworkMenu");
    m_bluetoothComponent.loadFromModule("CelestinaDesktop", "BluetoothMenu");
    m_performanceComponent.loadFromModule("CelestinaDesktop", "PerformanceMenu");
    m_captureComponent.loadFromModule("CelestinaDesktop", "CaptureMenu");
    m_wallpaperComponent.loadFromModule("CelestinaDesktop", "WallpaperMenu");
    for (const QQmlComponent *indicator : {
             &m_networkComponent,
             &m_bluetoothComponent,
             &m_performanceComponent,
             &m_captureComponent,
             &m_wallpaperComponent,
         }) {
        if (!indicator->isReady()) {
            qCritical().noquote()
                << "Celestina could not load an indicator menu:"
                << indicator->errorString();
        }
    }
    // A map that will not load leaves its capsule inert rather than opening an
    // empty surface, exactly as an indicator menu does.
    m_workspaceMapComponent.loadFromModule("CelestinaDesktop", "WorkspaceMap");
    if (!m_workspaceMapComponent.isReady()) {
        qCritical().noquote()
            << "Celestina could not load the workspace map:"
            << m_workspaceMapComponent.errorString();
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

void PanelMenuController::activateWindow(const QString &windowId)
{
    if (m_niri)
        m_niri->requestWindowFocus(windowId);

    // The map has said everything it can say: the answer to this request is the
    // session moving, and it arrives on screen rather than on any control here.
    close();
}

void PanelMenuController::activate(const QString &output, int index)
{
    if (m_niri)
        m_niri->requestWorkspaceFocus(output, index);

    // The answer arrives on the panel's own pill, where the request is
    // visible; the menu has said everything it can say.
    close();
}

void PanelMenuController::openWorkspaceMap(
    QWindow *panel,
    const QPoint &globalAnchor,
    const QVariant &workspaces
)
{
    if (!m_enabled || !panel || !m_niri || !m_workspaceMapComponent.isReady())
        return;

    close();

    QVariantMap initialProperties {
        {QStringLiteral("workspaces"), workspaces},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    initialProperties.insert(menuOutputProperties(panel));
    QObject *rootObject =
        m_workspaceMapComponent.createWithInitialProperties(initialProperties);
    auto *card = qobject_cast<QWindow *>(rootObject);
    if (!card) {
        qCritical().noquote()
            << "Celestina could not create the workspace map:"
            << m_workspaceMapComponent.errorString();
        delete rootObject;
        return;
    }

    // The same two signals the panel menu declares, so the surface that answers
    // a capsule and the one that answers a right click are interchangeable to
    // this controller.
    connect(card, SIGNAL(activated(QString, int)), this, SLOT(activate(QString, int)));
    connect(card, SIGNAL(windowActivated(QString)), this, SLOT(activateWindow(QString)));
    connect(card, SIGNAL(dismissed()), this, SLOT(menuDismissed()));

    placeCard(card, panel, globalAnchor);
    if (!m_surface->open(card, panel))
        delete card;
}

QString indicatorMenuComponent(const QString &kind)
{
    if (kind == QStringLiteral("network"))
        return QStringLiteral("NetworkMenu");
    if (kind == QStringLiteral("bluetooth"))
        return QStringLiteral("BluetoothMenu");
    if (kind == QStringLiteral("performance"))
        return QStringLiteral("PerformanceMenu");
    if (kind == QStringLiteral("capture"))
        return QStringLiteral("CaptureMenu");
    if (kind == QStringLiteral("wallpaper"))
        return QStringLiteral("WallpaperMenu");

    return QString();
}

void PanelMenuController::toggleIndicatorMenu(
    QWindow *panel,
    const QRect &globalOpener,
    const QString &kind,
    QObject *providerSource
)
{
    const bool needsProvider = kind != QStringLiteral("capture");
    if (!m_enabled || !panel || (needsProvider && !providerSource))
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
    const bool sameAgain = (m_openMenuKind == kind);
    close();
    if (sameAgain)
        return;

    QQmlComponent *component = nullptr;
    if (kind == QStringLiteral("network"))
        component = &m_networkComponent;
    else if (kind == QStringLiteral("bluetooth"))
        component = &m_bluetoothComponent;
    else if (kind == QStringLiteral("performance"))
        component = &m_performanceComponent;
    else if (kind == QStringLiteral("capture"))
        component = &m_captureComponent;
    else if (kind == QStringLiteral("wallpaper"))
        component = &m_wallpaperComponent;

    if (!component || !component->isReady())
        return;

    QVariantMap initialProperties {
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    if (needsProvider) {
        initialProperties.insert(
            QStringLiteral("providerSource"),
            QVariant::fromValue(providerSource)
        );
    }
    initialProperties.insert(menuOutputProperties(panel));
    QObject *rootObject = component->createWithInitialProperties(initialProperties);
    auto *window = qobject_cast<QWindow *>(rootObject);
    if (!window) {
        qCritical().noquote()
            << "Celestina could not create the" << kind
            << "menu:" << component->errorString();
        delete rootObject;
        return;
    }

    connect(window, SIGNAL(dismissed()), this, SLOT(menuDismissed()));
    if (kind == QStringLiteral("capture")) {
        connect(
            window,
            SIGNAL(captureRequested()),
            this,
            SLOT(captureScreenshot())
        );
    } else if (kind == QStringLiteral("wallpaper")) {
        connect(
            window,
            SIGNAL(chooseRequested()),
            this,
            SLOT(chooseWallpaperFolder())
        );
    }

    const int contentWidth = window->property("contentWidth").toInt();
    const int anchorGap = window->property("anchorGap").toInt();
    const QScreen *const screen = panel->screen();
    const QPoint outputOrigin = screen ? screen->geometry().topLeft() : QPoint();
    const QRect localOpener = panelPopupOpenerOnOutput(
        globalOpener, outputOrigin
    );
    placeCardOnOutput(
        window,
        panelPopupBodyOrigin(localOpener, contentWidth, anchorGap)
    );
    if (!m_surface->open(window, panel)) {
        delete window;
        return;
    }

    m_openMenuKind = kind;
    m_openIndicatorPanel = panel;
}

void PanelMenuController::captureScreenshot()
{
    // A retiring menu can finish a transition after another contextual menu
    // has replaced it. Only the menu currently carried by the surface may ask
    // Niri for a capture.
    if (sender() != m_surface->window())
        return;

    if (m_niri)
        m_niri->requestScreenshot();
    close();
}

void PanelMenuController::chooseWallpaperFolder()
{
    if (sender() != m_surface->window()
        || m_openMenuKind != QStringLiteral("wallpaper")
        || !m_openIndicatorPanel) {
        return;
    }

    const QPointer<QWindow> panel = m_openIndicatorPanel;
    close();
    QTimer::singleShot(0, panel, [panel]() {
        if (panel)
            QMetaObject::invokeMethod(panel, "openWallpaperFolderPicker");
    });
}

void PanelMenuController::toggleTrayItemsMenu(
    QWindow *panel,
    const QRect &globalOpener,
    QObject *traySource,
    QObject *providerSource
)
{
    constexpr auto trayItemsKind = "tray-items";
    if (!m_enabled || !panel || !traySource || !providerSource
        || !m_trayItemsComponent.isReady()) {
        return;
    }

    const bool sameAgain = (m_openMenuKind == QLatin1String(trayItemsKind));
    close();
    if (sameAgain)
        return;

    QVariantMap initialProperties {
        {QStringLiteral("traySource"), QVariant::fromValue(traySource)},
        {QStringLiteral("providerSource"), QVariant::fromValue(providerSource)},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    initialProperties.insert(menuOutputProperties(panel));
    QObject *rootObject =
        m_trayItemsComponent.createWithInitialProperties(initialProperties);
    auto *window = qobject_cast<QWindow *>(rootObject);
    if (!window) {
        qCritical().noquote()
            << "Celestina could not create the tray items menu:"
            << m_trayItemsComponent.errorString();
        delete rootObject;
        return;
    }

    connect(
        window,
        SIGNAL(activated(QString, QString, int, int)),
        this,
        SLOT(activateTrayItem(QString, QString, int, int))
    );
    connect(
        window,
        SIGNAL(secondaryActivated(QString, QString, int, int)),
        this,
        SLOT(secondaryActivateTrayItem(QString, QString, int, int))
    );
    connect(
        window,
        SIGNAL(itemMenuRequested(QString, QString, int, int)),
        this,
        SLOT(requestTrayItemMenu(QString, QString, int, int))
    );
    connect(window, SIGNAL(dismissed()), this, SLOT(menuDismissed()));

    const int contentWidth = window->property("contentWidth").toInt();
    const int anchorGap = window->property("anchorGap").toInt();
    const QScreen *const screen = panel->screen();
    const QPoint outputOrigin = screen ? screen->geometry().topLeft() : QPoint();
    const QRect localOpener = panelPopupOpenerOnOutput(globalOpener, outputOrigin);
    const QPoint bodyOrigin = panelPopupBodyOrigin(
        localOpener,
        contentWidth,
        anchorGap
    );
    placeCardOnOutput(window, bodyOrigin);
    if (screen) {
        // The tray inventory follows live model and preference snapshots. Its
        // opener-relative top stays fixed; when the rows no longer fit below it,
        // the real Menu scrolls inside the remaining output height instead of
        // moving the whole card over the panel.
        window->setProperty(
            "maximumContentHeight",
            qMax(1, screen->geometry().height() - bodyOrigin.y())
        );
    }
    if (!m_surface->open(window, panel)) {
        delete window;
        return;
    }

    m_openMenuKind = QLatin1String(trayItemsKind);
    m_openPanel = panel;
}

void PanelMenuController::activateTrayItem(
    const QString &service,
    const QString &path,
    int globalX,
    int globalY
)
{
    if (sender() != m_surface->window())
        return;

    emit trayItemActivated(service, path, globalX, globalY);
    close();
}

void PanelMenuController::secondaryActivateTrayItem(
    const QString &service,
    const QString &path,
    int globalX,
    int globalY
)
{
    if (sender() != m_surface->window())
        return;

    emit trayItemSecondaryActivated(service, path, globalX, globalY);
    close();
}

void PanelMenuController::requestTrayItemMenu(
    const QString &service,
    const QString &path,
    int globalX,
    int globalY
)
{
    constexpr auto trayItemsKind = "tray-items";
    QWindow *const parentMenu = m_surface->window();
    if (sender() != parentMenu || !m_openPanel
        || m_openMenuKind != QLatin1String(trayItemsKind)) {
        return;
    }

    // The D-Bus reply carries only the item's live identity, not a request
    // sequence. Coalesce an exact repeated click while that identity is still
    // pending; sending it twice would make the first reply indistinguishable
    // from the second and could consume the newer request.
    if (m_pendingKeepsTrayItems && m_pendingPanel == m_openPanel
        && m_pendingParentMenu == parentMenu
        && m_pendingService == service && m_pendingPath == path) {
        return;
    }

    // The inventory remains mapped. A second request retires only the previous
    // child (or its pending D-Bus answer) and preserves the parent identity.
    closeTrayChild(false);
    beginTrayMenuRequest(
        m_openPanel,
        QPoint(globalX, globalY),
        service,
        path,
        parentMenu
    );
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

    // As above, a peer supplies no request token. Until this exact direct
    // request answers, another click on the same item is the same request, not
    // a distinguishable successor.
    if (!m_pendingKeepsTrayItems && m_pendingPanel == panel
        && m_pendingService == service && m_pendingPath == path) {
        return;
    }

    // A menu requested directly from the bar is a standalone contextual menu:
    // no inventory is its parent, so it keeps the established full-output
    // outside-click surface.
    close();
    beginTrayMenuRequest(panel, globalAnchor, service, path, nullptr);
}

void PanelMenuController::beginTrayMenuRequest(
    QWindow *panel,
    const QPoint &globalAnchor,
    const QString &service,
    const QString &path,
    QWindow *parentMenu
)
{
    m_pendingPanel = panel;
    m_pendingParentMenu = parentMenu;
    m_pendingAnchor = globalAnchor;
    m_pendingService = service;
    m_pendingPath = path;
    m_pendingKeepsTrayItems = parentMenu != nullptr;
    emit trayMenuNeeded(service, path);
}

void PanelMenuController::clearPendingTrayMenu()
{
    m_pendingService.clear();
    m_pendingPath.clear();
    m_pendingPanel = nullptr;
    m_pendingParentMenu = nullptr;
    m_pendingKeepsTrayItems = false;
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

    // The request is spent here. An application may answer the same
    // `AboutToShow` twice, or answer a second time seconds later; without
    // this the guard above lets that through, because it only rejects a
    // *different* item — and a menu the user already dismissed reopens itself
    // at an anchor that has since moved.
    const QPointer<QWindow> panel = m_pendingPanel;
    const QPointer<QWindow> parentMenu = m_pendingParentMenu;
    const bool keepTrayItems = m_pendingKeepsTrayItems;
    const QPoint anchor = m_pendingAnchor;
    clearPendingTrayMenu();

    // An empty layout is still the answer that spends the request. Keeping its
    // target pending would let a duplicate response reopen a menu later.
    if (entries.isEmpty())
        return;

    constexpr auto trayItemsKind = "tray-items";
    if (keepTrayItems
        && (!parentMenu || parentMenu != m_surface->window()
            || m_openMenuKind != QLatin1String(trayItemsKind))) {
        return;
    }

    QVariantMap initialProperties {
        {QStringLiteral("entries"), entries},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    initialProperties.insert(menuOutputProperties(panel));
    QObject *rootObject = m_trayComponent.createWithInitialProperties(initialProperties);
    auto *window = qobject_cast<QWindow *>(rootObject);
    if (!window) {
        qCritical().noquote()
            << "Celestina could not create a tray menu:" << m_trayComponent.errorString();
        delete rootObject;
        return;
    }

    connect(window, SIGNAL(chosen(int)), this, SLOT(trayEntryChosen(int)));
    connect(window, SIGNAL(dismissed()), this, SLOT(menuDismissed()));

    // Both the standalone foreign menu and the child beside the tray
    // inventory use the same bounded viewport. Set it before reading the
    // window size because card-sized layer surfaces adopt that exact measure.
    capCardHeightBelowAnchor(window, panel, anchor);

    PanelMenuSurface::Coverage coverage = PanelMenuSurface::Coverage::Output;
    QPoint childSurfaceOrigin;
    if (keepTrayItems) {
        const QScreen *const screen = panel->screen();
        const QPoint outputOrigin = screen ? screen->geometry().topLeft() : QPoint();
        const QSize outputSize = screen ? screen->geometry().size() : QSize();
        const QRect parentCard(
            parentMenu->property("cardX").toInt(),
            parentMenu->property("cardY").toInt(),
            parentMenu->property("cardWidth").toInt(),
            parentMenu->property("cardHeight").toInt()
        );
        childSurfaceOrigin = adjacentTrayMenuOrigin(
            parentCard,
            anchor - outputOrigin,
            window->size(),
            outputSize,
            window->property("anchorGap").toInt()
        );
        // Card position belongs to the layer surface in this mode. Inside that
        // bounded surface the QML card begins at its own origin.
        placeCardOnOutput(window, QPoint());
        coverage = PanelMenuSurface::Coverage::Card;
    } else {
        placeCard(window, panel, anchor);
    }

    // QML construction is synchronous but may execute arbitrary completion
    // handlers. Revalidate the exact parent immediately before adopting the
    // child so a retired inventory cannot gain a late menu.
    if (keepTrayItems
        && (!parentMenu || parentMenu != m_surface->window()
            || m_openMenuKind != QLatin1String(trayItemsKind))) {
        delete window;
        return;
    }

    if (!m_trayChildSurface->open(window, panel, coverage, childSurfaceOrigin)) {
        delete window;
        return;
    }

    m_openService = service;
    m_openPath = path;
    m_openParentMenu = keepTrayItems ? parentMenu : nullptr;
}

void PanelMenuController::trayEntryChosen(int entryId)
{
    // Nothing is triggered once the child has closed: its identity is cleared,
    // so a late choice reaches no application rather than the wrong one.
    if (sender() != m_trayChildSurface->window()
        || (m_openService.isEmpty() && m_openPath.isEmpty())) {
        return;
    }

    emit trayEntryTriggered(m_openService, m_openPath, entryId);
    closeTrayChild(true);
}

void PanelMenuController::menuDismissed()
{
    if (sender() == m_trayChildSurface->window()) {
        closeTrayChild(true);
        return;
    }

    if (sender() == m_surface->window())
        close();
}

void PanelMenuController::restoreTrayParentFocus(
    const QPointer<QWindow> &parentMenu
)
{
    if (!parentMenu || parentMenu != m_surface->window())
        return;

    // The controller owns the surface state read by the callback. Use it as
    // the timer context so destroying the controller cancels the callback;
    // `parentMenu` remains guarded independently by its QPointer.
    QTimer::singleShot(0, this, [this, parentMenu]() {
        if (!parentMenu || parentMenu != m_surface->window()
            || !parentMenu->isVisible()) {
            return;
        }

        parentMenu->requestActivate();
        QObject *const menu = parentMenu->property("menu").value<QObject *>();
        if (menu) {
            QMetaObject::invokeMethod(
                menu,
                "forceActiveFocus",
                Q_ARG(Qt::FocusReason, Qt::PopupFocusReason)
            );
        }
    });
}

void PanelMenuController::closeTrayChild(bool restoreParentFocus)
{
    const QPointer<QWindow> parentMenu = m_openParentMenu;
    clearPendingTrayMenu();
    m_openService.clear();
    m_openPath.clear();
    m_openParentMenu = nullptr;
    m_trayChildSurface->close();

    if (restoreParentFocus)
        restoreTrayParentFocus(parentMenu);
}

void PanelMenuController::close()
{
    // Whatever was asked for is no longer wanted. A pending target that
    // outlives the menu is what lets a late answer open a surface the user
    // never asked for.
    closeTrayChild(false);
    m_openMenuKind.clear();
    m_openPanel = nullptr;
    m_openIndicatorPanel = nullptr;
    m_surface->close();
}
