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
    void close();

private slots:
    // A menu item is the same request a click on the strip makes.
    void activate(const QString &output, int index);

private:
    QWindow *createMenuWindow(const QVariant &workspaces);

    QQmlComponent m_component;
    QPointer<NiriClient> m_niri;
    PanelMenuSurface *m_surface;
    bool m_enabled;
};
