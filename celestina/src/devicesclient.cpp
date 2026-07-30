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
                    if (device.value(QStringLiteral("connected")).toBool()) {
                        connected = true;
                        name = device.value(QStringLiteral("name")).toString();
                        battery = device.value(QStringLiteral("battery"), -1).toInt();
                        charging = device.value(QStringLiteral("charging")).toBool();
                        break;
                    }
                }
                devices.endArray();
            }

            if (connected != m_connected || name != m_name || battery != m_battery
                || charging != m_charging) {
                m_connected = connected;
                m_name = name;
                m_battery = battery;
                m_charging = charging;
                emit changed();
            }

            if (m_reloadPending) {
                m_reloadPending = false;
                reload();
            }
        }
    );
}
