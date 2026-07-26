#include "devicesclient.h"

#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusMessage>
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
    reload();
}

void DevicesClient::reload()
{
    bool connected = false;
    QString name;
    int battery = -1;
    bool charging = false;

    QDBusMessage call = QDBusMessage::createMethodCall(
        QString::fromLatin1(service),
        QString::fromLatin1(path),
        QString::fromLatin1(iface),
        QStringLiteral("ListDevices"));
    const QDBusMessage reply = QDBusConnection::sessionBus().call(call);

    // The first connected device is the phone we surface. A non-reply (no daemon,
    // or any bus error) just leaves the defaults — the indicator stays hidden.
    if (reply.type() == QDBusMessage::ReplyMessage && !reply.arguments().isEmpty()) {
        const QDBusArgument devices =
            reply.arguments().constFirst().value<QDBusArgument>();
        devices.beginArray();
        while (!devices.atEnd()) {
            QVariantMap device;
            devices >> device;
            if (device.value(QStringLiteral("connected")).toBool()) {
                connected = true;
                name = device.value(QStringLiteral("name")).toString();
                battery = device.value(QStringLiteral("battery")).toInt();
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
}
