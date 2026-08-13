#pragma once

#include <QObject>
#include <QString>
#include <QVariantList>

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
    // Every device the daemon reports, each a map with the dict keys the
    // `Devices1` contract states (`id`, `name`, `type`, `connected`,
    // `mounted`, `paired`, `battery`, …). The summary properties above remain
    // the first connected device, which is all the permanent panel shows; the
    // phone menu is what reads this.
    Q_PROPERTY(QVariantList devices READ devices NOTIFY changed)

public:
    explicit DevicesClient(QObject *parent = nullptr);

    bool phoneConnected() const { return m_connected; }
    QString phoneName() const { return m_name; }
    int phoneBattery() const { return m_battery; }
    bool phoneCharging() const { return m_charging; }
    QVariantList devices() const { return m_devices; }

    // The three actions the daemon already serves, fire-and-forget over the
    // bus. Best-effort like everything else here: with no daemon the call
    // vanishes, and the menu's rows are only drawn from a daemon's own list,
    // so there is nothing real to press when there is nobody to answer.
    // Results arrive as the next `Changed` snapshot, never as painted state.
    Q_INVOKABLE void ring(const QString &deviceId);
    Q_INVOKABLE void requestPair(const QString &deviceId);
    Q_INVOKABLE void unpair(const QString &deviceId);

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
    QVariantList m_devices;
    bool m_reloadInFlight = false;
    bool m_reloadPending = false;
    QDBusServiceWatcher *m_serviceWatcher = nullptr;
};
