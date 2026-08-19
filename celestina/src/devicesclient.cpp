#include "devicesclient.h"

#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusServiceWatcher>
#include <QVariantMap>

namespace {
constexpr auto service = "org.celestina.Magnetita";
constexpr auto path = "/org/celestina/Devices1";
constexpr auto iface = "org.celestina.Devices1";
// The wireless screen mirror is a sibling contract under the same bus name,
// not part of the device contract: it rides on Android debugging rather than
// KDE Connect, and only the daemon knows where the phone currently answers.
constexpr auto mirrorPath = "/org/celestina/Mirror1";
constexpr auto mirrorIface = "org.celestina.Mirror1";
} // namespace

DevicesClient::DevicesClient(QObject *parent)
    : QObject(parent)
{
    // Live refresh. Registering the match before the daemon is up costs nothing
    // and means the panel lights the instant Magnetita starts emitting.
    QDBusConnection::sessionBus().connect(
        QString::fromLatin1(service),
        QString::fromLatin1(path),
        QString::fromLatin1(iface),
        QStringLiteral("Changed"),
        this,
        SLOT(reload()));
    m_serviceWatcher = new QDBusServiceWatcher(
        QString::fromLatin1(service),
        QDBusConnection::sessionBus(),
        QDBusServiceWatcher::WatchForOwnerChange,
        this);
    connect(
        m_serviceWatcher,
        &QDBusServiceWatcher::serviceOwnerChanged,
        this,
        [this](const QString &, const QString &, const QString &) { reload(); });
    reload();
}

void DevicesClient::reload()
{
    if (m_reloadInFlight) {
        m_reloadPending = true;
        return;
    }
    m_reloadInFlight = true;

    QDBusConnection bus = QDBusConnection::sessionBus();
    QDBusMessage call = QDBusMessage::createMethodCall(
        QString::fromLatin1(service),
        QString::fromLatin1(path),
        QString::fromLatin1(iface),
        QStringLiteral("ListDevices"));
    auto *watcher = new QDBusPendingCallWatcher(bus.asyncCall(call), this);
    connect(
        watcher,
        &QDBusPendingCallWatcher::finished,
        this,
        [this](QDBusPendingCallWatcher *finished) {
            const QDBusMessage reply = finished->reply();
            finished->deleteLater();
            m_reloadInFlight = false;

            bool connected = false;
            QString name;
            int battery = -1;
            bool charging = false;
            QVariantList snapshot;

            // A non-reply (no daemon, timeout or malformed value) is the empty
            // snapshot. The asynchronous call never stalls the panel thread.
            if (reply.type() == QDBusMessage::ReplyMessage
                && !reply.arguments().isEmpty()
                && reply.arguments().constFirst().canConvert<QDBusArgument>()) {
                const QDBusArgument devices =
                    reply.arguments().constFirst().value<QDBusArgument>();
                devices.beginArray();
                while (!devices.atEnd()) {
                    QVariantMap device;
                    devices >> device;
                    // The complete list is kept for the phone menu; the
                    // summary stays the first connected device, which is all
                    // the permanent panel reading shows.
                    snapshot.append(device);
                    if (!connected
                        && device.value(QStringLiteral("connected")).toBool()) {
                        connected = true;
                        name = device.value(QStringLiteral("name")).toString();
                        battery = device.value(QStringLiteral("battery"), -1).toInt();
                        charging = device.value(QStringLiteral("charging")).toBool();
                    }
                }
                devices.endArray();
            }

            if (connected != m_connected || name != m_name || battery != m_battery
                || charging != m_charging || snapshot != m_devices) {
                m_connected = connected;
                m_name = name;
                m_battery = battery;
                m_charging = charging;
                m_devices = snapshot;
                emit changed();
            }

            if (m_reloadPending) {
                m_reloadPending = false;
                reload();
            }
        }
    );
}

// One fire-and-forget action, addressed to a device the daemon itself listed.
// No watcher and no result handling on purpose: what happened comes back as
// the next `Changed` snapshot, which is the only truth the panel ever paints.
static void callDeviceAction(const char *method, const QString &deviceId)
{
    QDBusMessage call = QDBusMessage::createMethodCall(
        QString::fromLatin1(service),
        QString::fromLatin1(path),
        QString::fromLatin1(iface),
        QString::fromLatin1(method));
    call << deviceId;
    QDBusConnection::sessionBus().asyncCall(call);
}

void DevicesClient::ring(const QString &deviceId)
{
    callDeviceAction("Ring", deviceId);
}

void DevicesClient::requestPair(const QString &deviceId)
{
    callDeviceAction("RequestPair", deviceId);
}

void DevicesClient::unpair(const QString &deviceId)
{
    callDeviceAction("Unpair", deviceId);
}

void DevicesClient::mirror()
{
    // No device id: the mirror is not per-device. It rides on Android's own
    // wireless debugging, which the daemon discovers on the LAN, and the phone
    // that answers there is not necessarily the one this row was drawn from.
    //
    // Start only. The daemon ignores a request for a mirror that is already
    // running, so a second press cannot tear down a window the author is
    // looking at, and stopping stays where the state to reason about lives —
    // the Magnetita application, or the scrcpy window's own close button.
    QDBusMessage call = QDBusMessage::createMethodCall(
        QString::fromLatin1(service),
        QString::fromLatin1(mirrorPath),
        QString::fromLatin1(mirrorIface),
        QStringLiteral("Start"));
    QDBusConnection::sessionBus().asyncCall(call);
}
