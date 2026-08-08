#pragma once

#include <QObject>
#include <QPoint>
#include <QPointer>
#include <QQmlComponent>
#include <QVariant>
#include <QWindow>

class NiriClient;
class PanelMenuSurface;
class QQmlEngine;

// Opens the panel's context menu: the shell's second real surface consumer.
//
// On by default since the author accepted it on the real session (2026-07-30).
// `CELESTINA_PANEL_MENU=0` turns it off — a way back that costs nothing if it
// ever misbehaves on a session. A menu that cannot load leaves the panel
// without one rather than opening an empty surface.
// Which QML component draws an indicator's menu, or an empty string for a kind
// this shell does not have.
//
// A free function for the same reason `overlaySourceProperty` is one: "which
// component answers this name" is one fact, and a second copy of it is what
// produced a surface bound to a property it never declared.
QString indicatorMenuComponent(const QString &kind);

// Convert a top-panel control's horizontal global anchor into the menu
// surface's local coordinates. The compositor places the surface below all
// exclusive zones, so its local top — not an unknowable global Y — is the
// vertical anchor. Kept pure so every menu consumer shares a testable rule.
QPoint panelMenuOrigin(
    const QPoint &globalAnchor,
    const QPoint &outputOrigin,
    int shadowMargin
);

class PanelMenuController final : public QObject
{
    Q_OBJECT

public:
    PanelMenuController(QQmlEngine *engine, NiriClient *niri, QObject *parent = nullptr);

    bool isEnabled() const { return m_enabled; }

    // True unless the environment explicitly turns the menu off.
    static bool enabledByEnvironment();

public slots:
    // `workspaces` is the panel's own list for that output, as shown.
    void open(
        QWindow *panel,
        const QPoint &globalAnchor,
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
    // A connectivity indicator's own menu. `kind` is `network` or `bluetooth`;
    // anything else opens nothing. The same surface carries it, so opening one
    // closes whatever panel menu was up — two of these can never coexist.
    //
    // Asking for the kind that is already open closes it instead. The click
    // that reaches this while a menu is up is already the surface's to answer,
    // but a host that decided otherwise would leave a menu that only closed on
    // the second click, which is the defect this shell has had once already.
    void toggleIndicatorMenu(
        QWindow *panel,
        const QPoint &globalAnchor,
        const QString &kind,
        QObject *providerSource
    );
    void close();

    // Which indicator menu is on screen, or an empty string. Exposed so a
    // regression can read what a second request did rather than inferring it.
    QString openIndicator() const { return m_openIndicator; }

signals:
    // "Ask this item for its menu", and "this entry was chosen".
    void trayMenuNeeded(const QString &service, const QString &path);
    void trayEntryTriggered(const QString &service, const QString &path, int entryId);

public slots:
    void trayMenuReady(
        const QString &service,
        const QString &path,
        const QVariantList &entries
    );

private slots:
    void trayEntryChosen(int entryId);
    // A retiring window may emit its QML close signal after its successor is
    // already mapped. Only the currently adopted window may close the surface.
    void menuDismissed();
    // A menu item is the same request a click on the strip makes.
    void activate(const QString &output, int index);

private:
    QWindow *createMenuWindow(const QVariant &workspaces);

    QQmlComponent m_component;
    QQmlComponent m_trayComponent;
    QQmlComponent m_networkComponent;
    QQmlComponent m_bluetoothComponent;
    QPointer<NiriClient> m_niri;
    // The menu that was asked for and has not answered yet: where to put it,
    // and whose it is.
    QPointer<QWindow> m_pendingPanel;
    QPoint m_pendingAnchor;
    QString m_pendingService;
    QString m_pendingPath;
    // Whose menu is on screen right now, which is not the same question: the
    // request is answered once and forgotten, while the open menu still has to
    // know where to send an entry the user chooses.
    QString m_openService;
    QString m_openPath;
    // Which indicator menu is up, so asking for the same one again closes it.
    QString m_openIndicator;
    PanelMenuSurface *m_surface;
    bool m_enabled;
};
