#pragma once

#include <QObject>
#include <QString>

class QDBusServiceWatcher;

// The phone, as Magnetita reports it, for the panel to draw.
//
// Reads `org.celestina.Devices1` — the same small session-bus contract Siderita
// consumes — and exposes the first connected device as a handful of bindable
// properties. Best-effort by design: no daemon on the bus is simply "no phone",
// never an error the panel has to show. The daemon's `Changed` signal drives the
// live refresh, and the match is registered even before Magnetita is up, so the
// indicator lights the moment the daemon appears. Reads are asynchronous and
// burst-coalesced so a missing or slow session bus never stalls the panel.
// This remains manual C++ because cxx-qt-lib does not wrap QtDBus's
// QDBusPendingCallWatcher/QDBusArgument boundary for the existing CMake host.
class DevicesClient final : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool phoneConnected READ phoneConnected NOTIFY changed)
    Q_PROPERTY(QString phoneName READ phoneName NOTIFY changed)
    Q_PROPERTY(int phoneBattery READ phoneBattery NOTIFY changed)
    Q_PROPERTY(bool phoneCharging READ phoneCharging NOTIFY changed)

public:
    explicit DevicesClient(QObject *parent = nullptr);

    bool phoneConnected() const { return m_connected; }
    QString phoneName() const { return m_name; }
    int phoneBattery() const { return m_battery; }
    bool phoneCharging() const { return m_charging; }

signals:
    void changed();

private slots:
    // Re-reads ListDevices and emits `changed` only when something actually moved.
    void reload();

private:
    bool m_connected = false;
    QString m_name;
    int m_battery = -1;
    bool m_charging = false;
    bool m_reloadInFlight = false;
    bool m_reloadPending = false;
    QDBusServiceWatcher *m_serviceWatcher = nullptr;
};
