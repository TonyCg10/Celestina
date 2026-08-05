#pragma once

#include <QHash>
#include <QObject>
#include <QPointer>
#include <QQmlComponent>
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
    void setNotificationCentre(OverlayController *centre);

private slots:
    // The panel's QML root asks for a context menu at a screen point; the
    // manager knows which window asked and hands both to the menu controller.
    void panelMenuRequested(int globalX, int globalY, const QVariant &workspaces);
    // The notification centre is an overlay the host owns; a panel only asks
    // for it.
    void notificationCentreRequested();
    // A tray item's own menu, asked for from the panel that shows it.
    void trayMenuRequested(
        const QString &service,
        const QString &path,
        int globalX,
        int globalY
    );

private:
    bool ensurePanel(QScreen *screen);
    void removePanel(QScreen *screen);

    QGuiApplication *m_application;
    QQmlComponent m_component;
    QPointer<NiriClient> m_niri;
    QPointer<DevicesClient> m_phone;
    QPointer<ShellProvidersClient> m_providers;
    QPointer<TrayWatcher> m_tray;
    // Disposable until R0-E picks a popup surface: the manager only forwards
    // the panel's request to it and owns no menu state of its own.
    QPointer<PanelMenuController> m_menu;
    QPointer<OverlayController> m_notificationCentre;
    QHash<QScreen *, QPointer<QWindow>> m_panels;
    bool m_reducedMotion;
};
