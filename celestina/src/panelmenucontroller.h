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
    void close();

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
    // A menu item is the same request a click on the strip makes.
    void activate(const QString &output, int index);

private:
    QWindow *createMenuWindow(const QVariant &workspaces);

    QQmlComponent m_component;
    QQmlComponent m_trayComponent;
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
    PanelMenuSurface *m_surface;
    bool m_enabled;
};
