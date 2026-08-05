#pragma once

#include <QObject>
#include <QPointer>
#include <QQmlComponent>
#include <QVariantList>

class OverlaySurface;
class QQmlEngine;
class ShellProvidersClient;
class QWindow;

// The corner where this session's notifications appear.
//
// Nothing here decides how long a toast lives or what it says: the helper's
// server owns every one of those rules and publishes what is currently worth
// showing. This class only follows that list — it maps a surface when the list
// stops being empty, updates it in place while it changes, and tears it down
// when it empties, which is what keeps a burst of notifications from asking the
// compositor for a new surface each time.
//
// It is a second controller rather than a mode of `OsdController` because the
// two answer to different things: a readout is raised by a value changing and
// leaves on a timer this shell owns, while a toast is raised by another
// application and leaves when the server says it has.
class ToastController final : public QObject
{
    Q_OBJECT

public:
    ToastController(
        QQmlEngine *engine,
        ShellProvidersClient *providers,
        QObject *parent = nullptr
    );

    // False when the component itself failed to load — a broken QML file. The
    // shell then shows no toasts; the server and its history are unaffected.
    bool isEnabled() const { return m_enabled; }
    bool isVisible() const;

private:
    void providersChanged();
    void show(const QVariantList &toasts);
    void hide();
    QWindow *createWindow(const QVariantList &toasts);

    QQmlComponent m_component;
    QPointer<ShellProvidersClient> m_providers;
    OverlaySurface *m_surface;
    bool m_enabled;
};
