#pragma once

#include <QDBusContext>
#include <QHash>
#include <QObject>
#include <QPointer>
#include <QTimer>
#include <QVariantMap>

class NiriClient;
class QDBusConnection;

// The session's one way in.
//
// Panel mode owns `org.celestina.Shell` on the session bus and exports this
// object at `/org/celestina/Shell1`. The name is what makes panel mode
// single-instance: a second host finds it taken and defers instead of mapping
// a second set of panels. Transient clients — `celestina msg`, `--pick-output`
// — never claim it.
//
// The interface is versioned in its name *and* in every payload's `version`
// key. Published members are kept: a later version adds keys to the `a{sv}`
// maps rather than changing what an existing consumer reads, and a new
// interface version never entitles a second host to the name.
//
// Every later keybind routes through `Command`. Nothing invents a second
// channel, and the adapter's own stdin pipe is not one — it is internal to the
// Niri client.
class ShellService final : public QObject, protected QDBusContext
{
    Q_OBJECT
    // QtDBus takes the exported interface name from this class info, and the
    // wire names of the methods and signals from their C++ names — which is
    // why they are capitalized against this project's usual style.
    Q_CLASSINFO("D-Bus Interface", "org.celestina.Shell1")

public:
    enum class Attachment {
        // The shell owns the name and answers on it.
        Owned,
        // Another panel-mode host is already running this session.
        NameTaken,
        // No session bus: the panel runs, the channel does not.
        NoBus,
    };

    explicit ShellService(NiriClient *niri, QObject *parent = nullptr);

    // Exports the object first and claims the name second, so a client that
    // sees the name always finds the object behind it.
    Attachment attach(const QDBusConnection &bus);

    static QString serviceName();
    static QString objectPath();
    static QString interfaceName();

public slots:
    // The shell's current state, always carrying `version`.
    QVariantMap GetState();
    // Asks the shell to do something. Returns the id of a request that is now
    // pending; every later transition arrives as `CommandResult`. A verb the
    // shell does not serve, or options it cannot use, produce an error reply
    // instead of a silent no-op.
    qulonglong Command(const QString &verb, const QVariantMap &options);

signals:
    void Changed(const QVariantMap &state);
    void CommandResult(
        qulonglong requestId,
        const QString &state,
        const QVariantMap &details
    );

private slots:
    void publishState();
    void reportFocusRequest(qulonglong niriRequestId, const QString &state);

private:
    qulonglong focusWorkspace(const QVariantMap &options);

    QPointer<NiriClient> m_niri;
    QTimer m_stateTimer;
    // The bus sees this service's own request ids, never another component's
    // counter; the map is bounded by the Niri client's own request table.
    QHash<qulonglong, qulonglong> m_focusRequests;
    qulonglong m_lastRequestId = 0;
};
