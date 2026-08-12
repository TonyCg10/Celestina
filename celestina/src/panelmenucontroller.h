#pragma once

#include <QObject>
#include <QPoint>
#include <QRect>
#include <QRectF>
#include <QPointer>
#include <QQmlComponent>
#include <QSize>
#include <QVariant>
#include <QWindow>

#include "panelattachmentlease.h"

class NiriClient;
class PanelMenuSurface;
class QQmlEngine;

// Opens the panel's context menu: the shell's second real surface consumer.
//
// On by default since the author accepted it on the real session (2026-07-30).
// `CELESTINA_PANEL_MENU=0` turns it off — a way back that costs nothing if it
// ever misbehaves on a session. A menu that cannot load leaves the panel
// without one rather than opening an empty surface.
// Which QML component draws a panel-owned contextual menu, or an empty string
// for a kind this shell does not have.
//
// A free function for the same reason `overlaySourceProperty` is one: "which
// component answers this name" is one fact, and a second copy of it is what
// produced a surface bound to a property it never declared.
QString indicatorMenuComponent(const QString &kind);

// Place a foreign tray menu beside the shell-owned inventory. The requested
// anchor supplies the row's vertical origin; horizontal placement prefers the
// side with enough room and then clamps the complete child to the output.
QPoint adjacentTrayMenuOrigin(
    const QRect &parentCard,
    const QPoint &requestedAnchor,
    const QSize &childSize,
    const QSize &outputSize,
    int gap
);

class PanelMenuController final : public QObject
{
    Q_OBJECT

public:
    PanelMenuController(
        QQmlEngine *engine,
        NiriClient *niri,
        QObject *parent = nullptr
    );

    bool isEnabled() const { return m_enabled; }

    // True unless the environment explicitly turns the menu off.
    static bool enabledByEnvironment();

public slots:
    // `workspaces` is the panel's own list for that output, as shown.
    // The map a collapsed capsule opens: the same recipe as the panel menu,
    // with a board of window tiles instead of a list of rows.
    void openWorkspaceMap(
        QWindow *panel,
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor,
        const QVariant &workspaces
    );
    // A tray menu is a conversation with the application that owns it. The
    // controller asks and draws; who holds that conversation is the host's
    // wiring, so nothing here knows the tray host exists.
    void requestTrayMenu(
        QWindow *panel,
        const QPoint &globalAnchor,
        const QString &service,
        const QString &path
    );
    // A panel control's own contextual menu. The same surface carries network,
    // Bluetooth, performance, toolbox and wallpaper actions, so opening one
    // closes whatever panel menu was up — two of these can never coexist.
    //
    // Asking for the kind that is already open closes it instead. The click
    // that reaches this while a menu is up is already the surface's to answer,
    // but a host that decided otherwise would leave a menu that only closed on
    // the second click, which is the defect this shell has had once already.
    void toggleIndicatorMenu(
        QWindow *panel,
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor,
        const QString &kind,
        QObject *providerSource
    );
    // The shell-owned tray inventory. It is distinct from `requestTrayMenu`,
    // which asks one foreign application for its own D-Bus menu.
    void toggleTrayItemsMenu(
        QWindow *panel,
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor,
        QObject *traySource,
        QObject *providerSource
    );
    void close();

    // Which indicator menu is on screen, or an empty string. Exposed so a
    // regression can read what a second request did rather than inferring it.
    QString openIndicator() const { return m_openMenuKind; }

signals:
    // "Ask this item for its menu", and "this entry was chosen".
    void trayMenuNeeded(const QString &service, const QString &path);
    void trayEntryTriggered(const QString &service, const QString &path, int entryId);
    void trayItemActivated(
        const QString &service,
        const QString &path,
        int globalX,
        int globalY
    );
    void trayItemSecondaryActivated(
        const QString &service,
        const QString &path,
        int globalX,
        int globalY
    );

public slots:
    void trayMenuReady(
        const QString &service,
        const QString &path,
        const QVariantList &entries
    );

private slots:
    void trayEntryChosen(int entryId);
    void activateTrayItem(
        const QString &service,
        const QString &path,
        int globalX,
        int globalY
    );
    void secondaryActivateTrayItem(
        const QString &service,
        const QString &path,
        int globalX,
        int globalY
    );
    void requestTrayItemMenu(
        const QString &service,
        const QString &path,
        int globalX,
        int globalY,
        int globalWidth,
        int globalHeight
    );
    // A retiring window may emit its QML close signal after its successor is
    // already mapped. Only the currently adopted window may close the surface.
    void menuDismissed();
    // Capture stays a Niri-owned operation. The contextual row reports the
    // choice and this host seam sends the same request the former direct panel
    // button sent.
    void captureScreenshot();
    // The transient wallpaper gallery asks the permanent panel to open its
    // native folder chooser. The menu is closed first so destroying its
    // carrier cannot also destroy the dialog that will answer later.
    void chooseWallpaperFolder();
    // A menu item is the same request a click on the strip makes.
    void activate(const QString &output, int index);
    void activateWindow(const QString &windowId);

private:
    void beginTrayMenuRequest(
        QWindow *panel,
        const QRect &globalAnchor,
        const QString &service,
        const QString &path,
        QWindow *parentMenu
    );
    void clearPendingTrayMenu();
    void closeTrayChild(bool restoreParentFocus);
    void restoreTrayParentFocus(const QPointer<QWindow> &parentMenu);

    QQmlComponent m_trayComponent;
    QQmlComponent m_trayItemsComponent;
    QQmlComponent m_networkComponent;
    QQmlComponent m_bluetoothComponent;
    QQmlComponent m_performanceComponent;
    QQmlComponent m_captureComponent;
    QQmlComponent m_wallpaperComponent;
    QQmlComponent m_workspaceMapComponent;
    QPointer<NiriClient> m_niri;
    // The menu that was asked for and has not answered yet: where to put it,
    // and whose it is.
    QPointer<QWindow> m_pendingPanel;
    QPointer<QWindow> m_pendingParentMenu;
    // The invoking control's complete global rectangle. A point-only route
    // arrives as a zero-sized rectangle and never attaches a membrane.
    QRect m_pendingAnchor;
    QString m_pendingService;
    QString m_pendingPath;
    bool m_pendingKeepsTrayItems = false;
    // Whose menu is on screen right now, which is not the same question: the
    // request is answered once and forgotten, while the open menu still has to
    // know where to send an entry the user chooses.
    QString m_openService;
    QString m_openPath;
    QPointer<QWindow> m_openParentMenu;
    // The panel-owned menu kind that is up, so asking for the same one again
    // closes it. A foreign tray item's D-Bus menu is identified separately by
    // service/path above.
    QString m_openMenuKind;
    // The panel that opened the tray inventory. Retained only while that menu
    // is current so a right click can place the chosen item's own asynchronously
    // loaded D-Bus menu beside it on the same output.
    QPointer<QWindow> m_openPanel;
    // The panel behind the current first-party indicator menu. Kept separate
    // from the tray inventory owner because only the wallpaper gallery needs
    // to call back into its permanent folder chooser.
    QPointer<QWindow> m_openIndicatorPanel;
    PanelAttachmentLease m_attachmentLease;
    PanelMenuSurface *m_surface;
    PanelMenuSurface *m_trayChildSurface;
    bool m_enabled;
};
