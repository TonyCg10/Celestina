#pragma once

#include <QHash>
#include <QObject>
#include <QPointer>
#include <QQmlComponent>
#include <QUrl>
#include <QVariant>
#include <QWindow>

class DevicesClient;
class NiriClient;
class PanelMenuController;
class ShellProvidersClient;
class TrayWatcher;
class QGuiApplication;
class QQmlEngine;
class QScreen;
class QTimer;

// Owns the per-output panel lifecycle: one layer-shell surface per QScreen,
// created on start and on hotplug, torn down when its output disappears.
//
// This remains manual C++ because LayerShellQt's surface configuration is not
// reachable through CXX-Qt. The manager is deliberately limited to window
// creation, layer-shell configuration and screen bookkeeping; Niri state,
// blur policy and presentation live in their own owners.
class OverlayController;

class PanelManager final : public QObject
{
    Q_OBJECT

public:
    PanelManager(
        QGuiApplication *application,
        QQmlEngine *engine,
        NiriClient *niri,
        DevicesClient *phone,
        ShellProvidersClient *providers,
        TrayWatcher *tray,
        PanelMenuController *menu,
        bool reducedMotion
    );

    ~PanelManager() override;

    bool start();

    // Wired in after construction, once main() has built the overlay. A shell
    // without it keeps every panel and only that request goes unanswered.
    void setLauncher(OverlayController *launcher);
    void setNotificationCentre(OverlayController *centre);
    void setControlCentre(OverlayController *centre);
    void setClipboard(OverlayController *clipboard);
    void setSessionMenu(OverlayController *menu);

private slots:
    // The panel's QML root asks for a context menu at a screen point; the
    // manager knows which window asked and hands both to the menu controller.
    void workspaceMapRequested(int globalX, int globalY, const QVariant &workspaces);
    // The compact tray control opens the live item inventory as a contextual
    // menu; an item's own D-Bus menu remains the separate request below.
    void trayDrawerRequested(
        int globalX,
        int globalY,
        int openerWidth,
        int openerHeight
    );
    // The notification centre is an overlay the host owns; a panel only asks
    // for it.
    void notificationCentreRequested(
        int globalX,
        int globalY,
        int openerWidth,
        int openerHeight
    );
    void launcherRequested(
        int globalX,
        int globalY,
        int openerWidth,
        int openerHeight
    );
    // The soft-menu prototype follows this panel control rather than centring
    // itself on whichever output currently holds the pointer.
    void controlCentreRequested(
        int globalX,
        int globalY,
        int openerWidth,
        int openerHeight
    );
    void clipboardRequested(
        int globalX,
        int globalY,
        int openerWidth,
        int openerHeight
    );
    void sessionMenuRequested(
        int globalX,
        int globalY,
        int openerWidth,
        int openerHeight
    );
    // A panel control's contextual menu, asked for from the panel that shows
    // it. The manager knows which window asked and what bridge to hand it.
    void indicatorMenuRequested(
        const QString &kind,
        int globalX,
        int globalY,
        int openerWidth,
        int openerHeight
    );
    // A tray item's own menu, asked for from the panel that shows it.
    void trayMenuRequested(
        const QString &service,
        const QString &path,
        int globalX,
        int globalY
    );
    // A native folder dialog returns a URL. The manager accepts only a local
    // path; the worker owns every filesystem, scan and image rule after that
    // boundary.
    void wallpaperFolderSelected(const QUrl &source);

private:
    bool ensurePanel(QScreen *screen);
    void removePanel(QScreen *screen);
    void togglePanelOverlay(
        OverlayController *controller,
        QWindow *panel,
        int globalX,
        int globalY,
        int openerWidth,
        int openerHeight
    );
    // The set of outputs changed. Brightness lives behind DDC, which is a
    // one-at-a-time conversation with a monitor, so this never asks twice for
    // one burst: it restarts a short timer and the provider hears once.
    void outputsChanged();

    QGuiApplication *m_application;
    QQmlComponent m_component;
    QPointer<NiriClient> m_niri;
    QPointer<DevicesClient> m_phone;
    QPointer<ShellProvidersClient> m_providers;
    QPointer<TrayWatcher> m_tray;
    // Disposable until R0-E picks a popup surface: the manager only forwards
    // the panel's request to it and owns no menu state of its own.
    QPointer<PanelMenuController> m_menu;
    QPointer<OverlayController> m_launcher;
    QPointer<OverlayController> m_notificationCentre;
    QPointer<OverlayController> m_controlCentre;
    QPointer<OverlayController> m_clipboard;
    QPointer<OverlayController> m_sessionMenu;
    QHash<QScreen *, QPointer<QWindow>> m_panels;
    // Coalesces a hotplug burst into one request. Owned here because the panel
    // manager is what already observes outputs appearing and disappearing.
    QTimer *m_outputsSettled;
    bool m_reducedMotion;
};
