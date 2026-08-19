#pragma once

#include <QHash>
#include <QObject>
#include <QPointer>
#include <QQmlComponent>
#include <QRectF>

#include "bubbleanchorsource.h"
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

class PanelManager final : public QObject, public BubbleAnchorSource
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
    // The mapped panel on one output, or null: the quiet surfaces resolve
    // their membrane anchors from the real bar rather than remembering one.
    QWindow *panelWindowFor(QScreen *screen) const
    {
        return m_panels.value(screen).data();
    }

    // M7 — where this output's bubbles currently sit, in the compositor's output-local
    // logical coordinates, or an empty rectangle when that output has no mapped panel.
    //
    // Asked of the live panel rather than remembered, for the same reason `panelWindowFor`
    // exists: a remembered rectangle is a rectangle that can be wrong after a relayout, and
    // a minimize triggered from a keybind has no surface of its own to read.
    QRectF bubbleAnchorFor(const QString &outputName) const override;

    // The session's motion preference, so a surface-less action can honour it too.
    bool reducedMotion() const override { return m_reducedMotion; }

    void setLauncher(OverlayController *launcher);
    void setNotificationCentre(OverlayController *centre);
    void setControlCentre(OverlayController *centre);
    void setClipboard(OverlayController *clipboard);
    void setBubbleSelector(OverlayController *selector);
    void setSessionMenu(OverlayController *menu);

private slots:
    // The panel's QML root asks for a context menu at a screen point; the
    // manager knows which window asked and hands both to the menu controller.
    void workspaceMapRequested(
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor,
        const QVariant &workspaces
    );
    // The compact tray control opens the live item inventory as a contextual
    // menu; an item's own D-Bus menu remains the separate request below.
    void trayDrawerRequested(
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor
    );
    // The notification centre is an overlay the host owns; a panel only asks
    // for it.
    void notificationCentreRequested(
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor
    );
    void launcherRequested(
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor
    );
    // The soft-menu prototype follows this panel control rather than centring
    // itself on whichever output currently holds the pointer.
    void controlCentreRequested(
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor
    );
    void clipboardRequested(
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor
    );
    void bubbleSelectorRequested(
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor
    );
    void sessionMenuRequested(
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor
    );
    // A panel control's contextual menu, asked for from the panel that shows
    // it. The manager knows which window asked and what bridge to hand it.
    void indicatorMenuRequested(
        const QString &kind,
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor
    );
    // A tray item's own menu, asked for from the panel that shows it.
    void trayMenuRequested(
        const QString &service,
        const QString &path,
        const QString &appName,
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor
    );
    // A native folder dialog returns a URL. The manager accepts only a local
    // path; the worker owns every filesystem, scan and image rule after that
    // boundary.
    void wallpaperFolderSelected(const QUrl &source);
    // A press on a bar's background that no control claimed. Whatever
    // contextual surface is up goes away, exactly as it would for a press on
    // the desktop: the bar's strip stays out of those surfaces' input regions
    // so a click on a different opener can swap menus, which leaves this
    // dismissal for the bar itself to report.
    void dismissRequested();

private:
    bool ensurePanel(QScreen *screen);
    void removePanel(QScreen *screen);
public:
    // The panel overlays, all but `keep` retired. The menu controller is
    // untouched: it owns the memory of which indicator is up, which is what
    // lets that indicator's own opener toggle it shut.
    void closeOverlaysExcept(const OverlayController *keep);
    // The same, plus whatever menu the panel has open — every contextual
    // surface this manager can have caused.
    void closeContextualExcept(const OverlayController *keep);

private:
    void togglePanelOverlay(
        OverlayController *controller,
        QWindow *panel,
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor
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
    QPointer<OverlayController> m_bubbleSelector;
    QPointer<OverlayController> m_sessionMenu;
    QHash<QScreen *, QPointer<QWindow>> m_panels;
    // Coalesces a hotplug burst into one request. Owned here because the panel
    // manager is what already observes outputs appearing and disappearing.
    QTimer *m_outputsSettled;
    bool m_reducedMotion;
};
