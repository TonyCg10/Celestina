#pragma once

#include <QObject>
#include <QPointer>
#include <QQmlComponent>
#include <QTimer>

#include "osdreadings.h"

class OverlaySurface;
class QQmlEngine;
class ShellProvidersClient;
class QWindow;

// The shell's on-screen display: what a device is at, shown for a moment in
// the corner and then gone.
//
// It is not an overlay in the `OverlayController` sense. Nobody opens it and
// nobody dismisses it: it appears because a provider published a new reading,
// it never takes focus or the keyboard, and it leaves on its own. That is why
// this is a second controller rather than a third component loaded by the
// first, whose whole contract is a keybind-driven, focused, toggled surface.
//
// What is worth showing is `OsdReadings`' decision; this class owns only the
// window's life: create it once, keep it while readings keep arriving, and
// tear it down when they stop.
class OsdController final : public QObject
{
    Q_OBJECT

public:
    OsdController(
        QQmlEngine *engine,
        ShellProvidersClient *providers,
        QObject *parent = nullptr
    );

    // False when the component itself failed to load — a broken QML file. The
    // shell then simply never shows an OSD; nothing else changes.
    bool isEnabled() const { return m_enabled; }
    bool isVisible() const;

private:
    void providersChanged();
    void show(const OsdReadings::Reading &reading);
    void hide();
    QWindow *createWindow(const OsdReadings::Reading &reading);
    static void applyReading(QWindow *window, const OsdReadings::Reading &reading);

    QQmlComponent m_component;
    QPointer<ShellProvidersClient> m_providers;
    OverlaySurface *m_surface;
    OsdReadings m_readings;
    // Restarted by every new reading, so a burst of wheel notches is one
    // display that stays up rather than one that flickers per notch.
    QTimer m_hideTimer;
    bool m_enabled;
};
