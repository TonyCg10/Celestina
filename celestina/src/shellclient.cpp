#include "shellclient.h"

#include <cstdio>
#include <cstdlib>

#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusReply>
#include <QDBusServiceWatcher>
#include <QEventLoop>
#include <QJsonDocument>
#include <QTextStream>
#include <QTimer>
#include <QVariantMap>

#include "shellcommandline.h"
#include "shellservice.h"

namespace {
// The shell owns how long a request may stay pending; this only has to outlast
// it, so the two numbers cannot drift apart. A client still exits the moment a
// terminal state arrives — this bound is reached only when the shell itself
// stops answering.
const int resultTimeoutMs = ShellService::maxRequestLifetimeMs() + 2000;

void reportError(const QString &message)
{
    QTextStream(stderr) << "celestina msg: " << message << '\n';
}

// D-Bus hands nested containers back as a lazily typed argument. Unwrapping
// them generically keeps this client free of any knowledge about which keys
// the shell's state carries.
QVariant demarshal(const QVariant &value)
{
    if (value.metaType() != QMetaType::fromType<QDBusArgument>())
        return value;

    const QDBusArgument argument = value.value<QDBusArgument>();
    switch (argument.currentType()) {
    case QDBusArgument::ArrayType: {
        QVariantList items;
        argument.beginArray();
        while (!argument.atEnd()) {
            QVariant item;
            argument >> item;
            items.append(demarshal(item));
        }
        argument.endArray();
        return items;
    }
    case QDBusArgument::MapType: {
        QVariantMap entries;
        argument.beginMap();
        while (!argument.atEnd()) {
            argument.beginMapEntry();
            QString key;
            QVariant entry;
            argument >> key >> entry;
            entries.insert(key, demarshal(entry));
            argument.endMapEntry();
        }
        argument.endMap();
        return entries;
    }
    default:
        return value;
    }
}

QVariantMap demarshalMap(const QVariantMap &map)
{
    QVariantMap result;
    for (auto entry = map.constBegin(); entry != map.constEnd(); ++entry)
        result.insert(entry.key(), demarshal(entry.value()));
    return result;
}

// Waits for the transitions of one request. It is connected to the signal
// *before* the request is made — otherwise the bus would never route a result
// that the shell resolves immediately — so it buffers whatever arrives until
// it learns which id is its own. A result for any other id belongs to another
// caller and is dropped.
class CommandWatcher final : public QObject
{
    Q_OBJECT

public:
    void setRequestId(qulonglong requestId)
    {
        m_requestId = requestId;
        m_known = true;

        const QList<QPair<qulonglong, QString>> buffered = m_buffered;
        m_buffered.clear();
        for (const auto &result : buffered)
            apply(result.first, result.second);
    }

    int run()
    {
        if (m_resolved)
            return m_status;

        QTimer::singleShot(resultTimeoutMs, &m_loop, [this] {
            reportError(QStringLiteral("the shell did not resolve the request"));
            resolve(EXIT_FAILURE);
        });
        return m_loop.exec();
    }

    void abort(const QString &reason)
    {
        reportError(reason);
        resolve(EXIT_FAILURE);
    }

public slots:
    void commandResult(
        qulonglong requestId,
        const QString &state,
        const QVariantMap &details
    )
    {
        Q_UNUSED(details)
        apply(requestId, state);
    }

private:
    void apply(qulonglong requestId, const QString &state)
    {
        if (!m_known) {
            m_buffered.append({requestId, state});
            return;
        }
        if (requestId != m_requestId || m_resolved)
            return;

        if (state == QStringLiteral("pending")) {
            reportError(QStringLiteral("pending"));
            return;
        }

        QTextStream(stdout) << state << '\n';
        resolve(
            state == QStringLiteral("confirmed") ? EXIT_SUCCESS : EXIT_FAILURE
        );
    }

    void resolve(int status)
    {
        if (m_resolved)
            return;

        m_resolved = true;
        m_status = status;
        m_loop.exit(status);
    }

    QEventLoop m_loop;
    QList<QPair<qulonglong, QString>> m_buffered;
    qulonglong m_requestId = 0;
    int m_status = EXIT_FAILURE;
    bool m_known = false;
    bool m_resolved = false;
};

int printState(QDBusInterface &shell)
{
    const QDBusReply<QVariantMap> reply = shell.call(QStringLiteral("GetState"));
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return EXIT_FAILURE;
    }

    const QJsonDocument document =
        QJsonDocument::fromVariant(demarshalMap(reply.value()));
    QTextStream(stdout) << document.toJson(QJsonDocument::Indented);
    return EXIT_SUCCESS;
}
} // namespace

int runShellMessage(const QStringList &arguments)
{
    const ShellCommandLine line = parseShellCommandLine(arguments);
    if (!line.error.isEmpty()) {
        reportError(line.error);
        return EXIT_FAILURE;
    }

    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        reportError(QStringLiteral("no session bus: %1").arg(bus.lastError().message()));
        return EXIT_FAILURE;
    }

    QDBusInterface shell(
        ShellService::serviceName(),
        ShellService::objectPath(),
        ShellService::interfaceName(),
        bus
    );
    if (!shell.isValid()) {
        reportError(QStringLiteral("no Celestina shell owns this session (%1)")
                        .arg(shell.lastError().message()));
        return EXIT_FAILURE;
    }

    if (line.readsState)
        return printState(shell);

    // Listening before asking: a request the shell resolves immediately must
    // not answer into a match rule that does not exist yet.
    CommandWatcher watcher;
    if (!bus.connect(
            ShellService::serviceName(),
            ShellService::objectPath(),
            ShellService::interfaceName(),
            QStringLiteral("CommandResult"),
            &watcher,
            SLOT(commandResult(qulonglong, QString, QVariantMap))
        )) {
        reportError(QStringLiteral("cannot listen for the request's result"));
        return EXIT_FAILURE;
    }

    QDBusServiceWatcher busWatcher(
        ShellService::serviceName(),
        bus,
        QDBusServiceWatcher::WatchForUnregistration
    );
    QObject::connect(
        &busWatcher,
        &QDBusServiceWatcher::serviceUnregistered,
        &watcher,
        [&watcher](const QString &) {
            watcher.abort(QStringLiteral("the shell left the bus"));
        }
    );

    const QDBusReply<qulonglong> reply = shell.call(
        QStringLiteral("Command"),
        line.verb,
        line.options
    );
    if (!reply.isValid()) {
        reportError(reply.error().message());
        return EXIT_FAILURE;
    }

    watcher.setRequestId(reply.value());
    return watcher.run();
}

#include "shellclient.moc"
