#pragma once

#include <QObject>
#include <QPointer>
#include <QQmlComponent>
#include <QString>
#include <QVariantMap>

class OverlaySurface;
class QQmlEngine;
class QWindow;
class ShellProvidersClient;

// Opens and closes one keybind-driven overlay — the launcher, the clipboard
// history.
//
// The two are identical in mechanics: one centered on-demand-keyboard surface,
// toggled by a `celestina msg` verb, torn down on its own dismissal. They
// differ only in which QML component they load and what that component does
// with the provider bridge it is handed, so this class owns exactly the shared
// part. Domain logic — searching, launching, selecting a history entry — lives
// entirely in the QML component, which talks to `providerSource` the same way
// every bar widget already does (see `Panel.qml`): nothing here parses a
// provider payload or knows a launcher or a clipboard exists.
class OverlayController final : public QObject
{
    Q_OBJECT

public:
    OverlayController(
        QQmlEngine *engine,
        ShellProvidersClient *providers,
        const QString &qmlComponentName,
        QObject *parent = nullptr
    );

    // Properties this overlay's component needs beyond the provider bridge —
    // the session menu's own request channel, for instance. Set before the
    // overlay is first opened; a component that does not declare one simply
    // never receives it.
    void setExtraProperties(const QVariantMap &properties);

    // False when the component itself failed to load — a broken QML file, not
    // a missing provider. The overlay simply never opens; nothing crashes.
    bool isEnabled() const { return m_enabled; }
    bool isOpen() const;

public slots:
    void open();
    void close();
    void toggle();

private:
    QWindow *createWindow();

    QQmlComponent m_component;
    QPointer<ShellProvidersClient> m_providers;
    QString m_componentName;
    QVariantMap m_extraProperties;
    OverlaySurface *m_surface;
    bool m_enabled;
};
