#pragma once

#include <QDBusContext>
#include <QDBusError>
#include <QHash>
#include <QObject>
#include <QPointer>
#include <QString>
#include <QTimer>
#include <QElapsedTimer>
#include <QVariantMap>

#include "sessionrequests.h"

class NiriClient;
class LockController;
class OverlayController;
class ShellProvidersClient;
class BubbleAnchorSource;
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

    // The session name is claimed before the adapter is constructed. A host
    // that must defer therefore starts no helper process and performs no
    // provider IO; the accepted host wires Niri immediately afterwards.
    void setNiriClient(NiriClient *niri);

    // Exports the object first and claims the name second, so a client that
    // sees the name always finds the object behind it.
    Attachment attach(const QDBusConnection &bus);

    // Wired in after construction, once main() has built the overlay
    // controllers — a shell that starts without one still owns the bus name
    // and serves every other verb; that overlay's toggle just errors.
    void setLauncherController(OverlayController *controller);
    void setClipboardController(OverlayController *controller);
    void setNotificationCentreController(OverlayController *controller);
    void setControlCentreController(OverlayController *controller);
    void setBubbleSelectorController(OverlayController *controller);
    void setSessionMenuController(OverlayController *controller);
    // The session lock. Without one the lock and suspend verbs keep the
    // fail-closed refusal they have always had.
    void setLockController(LockController *controller);
    // The bridge every session verb that changes a device travels over. A
    // shell without it still owns the bus name and serves every other verb;
    // those verbs then fail visibly instead of pretending to have worked.
    void setProvidersClient(ShellProvidersClient *providers);
    // Wired in after construction, like the overlay controllers. Only the anchor for a
    // minimize needs it; a shell without one still minimizes, with Niri's ordinary motion.
    void setBubbleAnchorSource(BubbleAnchorSource *anchors);

private:
    qulonglong toggleBubbleSelector(const QString &verb);
    QVariantMap minimizeTransition() const;
    qulonglong minimizeWindow(const QVariantMap &options);
    void reportProviderAction(
        qulonglong helperRequestId,
        const QString &state,
        const QString &reason
    );

public:

    // The longest a request may stay pending before the shell reports a
    // failure. A monitor over DDC is what makes it seconds rather than
    // milliseconds; a client waiting on a result outlasts this rather than
    // guessing its own bound.
    static int maxRequestLifetimeMs();

    static QString serviceName();
    static QString objectPath();
    static QString interfaceName();

    // Why the last refused request was refused, cleared as it is read.
    //
    // A bus caller learns this from the error reply QtDBus sends it. An
    // in-process caller has no such reply — there is no message behind its
    // call — so it collects the same sentence here instead of inventing its
    // own. This is deliberately an ordinary method: a slot or a signal would
    // be exported and would widen the session interface.
    QString takeRefusalReason();

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
    // A compositor action Niri answered itself, reported once against the
    // request the session bus knows about.
    void reportAction(
        qulonglong niriRequestId,
        const QString &state,
        const QString &reason
    );

private:
    qulonglong focusWorkspace(const QVariantMap &options);
    // Asks the compositor to blank the outputs. There is no matching verb to
    // wake them: any input does, and that is the compositor's business.
    qulonglong powerOffMonitors();
    // Ends the session through the compositor, which owns it.
    qulonglong logOut();
    // Asks logind to reboot or power off. `method` is its own method name; the
    // outcome is whatever logind answers, never an assumption.
    qulonglong askLogind(const QString &verb, const QString &method);
    // Reports one finished request on the bus.
    void reportOutcome(
        qulonglong requestId,
        const QString &verb,
        const QString &state,
        const QString &reason
    );
    // Forwards one session verb to its provider and starts waiting for the
    // reading that would prove it happened.
    qulonglong requestSession(
        const QString &verb,
        const QVariantMap &options,
        const SessionRequests::Expectation &expectation
    );
    // Emits one `CommandResult` per transition the table recorded, and keeps
    // the expiry tick running only while something is still in flight.
    void reportSessionOutcomes();
    // A toggle is a local UI action with no compositor round trip to wait on,
    // unlike `focusWorkspace`: it resolves the moment it runs, so it needs no
    // entry in `m_focusRequests`.
    qulonglong toggleOverlay(OverlayController *controller, const QString &verb);
    qulonglong lockSession(const QString &verb);
    // Refuses the request being served and answers whoever asked.
    //
    // `QDBusContext` carries a call context only while QtDBus is dispatching a
    // real message; `sendErrorReply` dereferences that context unconditionally,
    // so calling it for an in-process caller crashes the shell. Every refusal
    // goes through here: the reply is sent only when there is a message to
    // reply to, and the reason is always retained for `takeRefusalReason`.
    // Returning `0` is what a refused `Command` answers.
    qulonglong refuse(QDBusError::ErrorType type, const QString &message);

    QPointer<NiriClient> m_niri;
    QPointer<OverlayController> m_launcher;
    QPointer<OverlayController> m_clipboard;
    QPointer<OverlayController> m_notificationCentre;
    QPointer<OverlayController> m_controlCentre;
    QPointer<OverlayController> m_bubbleSelector;
    QPointer<OverlayController> m_sessionMenu;
    QPointer<LockController> m_lock;
    QPointer<ShellProvidersClient> m_providers;
    // Not a QPointer: the interface is not a QObject. Its implementation outlives this
    // service, both being owned by main() for the session.
    BubbleAnchorSource *m_anchors = nullptr;
    QTimer m_stateTimer;
    // A pending session verb must not wait forever for a device that will
    // never answer, so the table is swept while anything is in flight.
    QTimer m_sessionTimer;
    // Monotonic and independent of the wall clock, which a session may change.
    QElapsedTimer m_clock;
    SessionRequests m_sessionRequests;
    // The bus sees this service's own request ids, never another component's
    // counter; the map is bounded by the Niri client's own request table.
    QHash<qulonglong, qulonglong> m_focusRequests;
    // The same, for compositor actions whose outcome Niri reports itself.
    QHash<qulonglong, qulonglong> m_actionRequests;
    // Which verb each of those was, so the outcome says what it answered.
    QHash<qulonglong, QString> m_actionVerbs;
    // Provider-backed verbs, kept apart from `m_actionRequests` on purpose: the compositor
    // adapter and the provider helper number their requests independently, both from one, so
    // one map would answer the wrong caller as soon as the two counters met.
    QHash<qulonglong, qulonglong> m_providerRequests;
    QHash<qulonglong, QString> m_providerVerbs;
    // The last refusal's sentence, held only until the caller that caused it
    // reads it. One request is served at a time, so one slot is enough.
    QString m_refusalReason;
    qulonglong m_lastRequestId = 0;
};
