#pragma once

#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusContext>
#include <QDBusMessage>
#include <QHash>
#include <QList>
#include <QMap>
#include <QObject>
#include <QString>
#include <QVariantMap>

class PolkitConversation;
class QDBusServiceWatcher;

// One identity polkitd will accept an answer from, exactly as it sent it.
// `kind` is "unix-user" or "unix-group"; `details` carries the uid or gid.
// Kept as its own type rather than flattened into a name, because the shell
// must offer what polkitd offered and inventing a display name here would be
// this shell deciding who may authorize something.
struct PolkitIdentity {
    QString kind;
    QVariantMap details;
};
Q_DECLARE_METATYPE(PolkitIdentity)
Q_DECLARE_METATYPE(QList<PolkitIdentity>)

QDBusArgument &operator<<(QDBusArgument &argument, const PolkitIdentity &value);
const QDBusArgument &operator>>(const QDBusArgument &argument,
                                PolkitIdentity &value);

// This session's authentication agent: the object polkitd calls when something
// needs authorization, and the registration that makes it callable.
//
// ADR 0005 bounds what this may do. It shows what polkitd sent, collects one
// answer, and hands that answer to `PolkitConversation`, which hands it to the
// system helper. No branch here decides an authorization, and none can: the
// helper answers polkitd directly, so this object never carries a verdict that
// polkitd would trust.
//
// It registers on the system bus for this session's own subject. A registered
// agent is a session-wide singleton — polkitd refuses a second one — so
// failing to register is reported rather than retried into a fight with
// whatever already holds it.
class PolkitAgent final : public QObject, protected QDBusContext
{
    Q_OBJECT
    // QtDBus takes the exported interface name from this class info, and the
    // wire names of the methods from their C++ names — which is why they are
    // capitalized against this project's usual style. These two are polkit's
    // names, not this project's, and may not be renamed.
    Q_CLASSINFO("D-Bus Interface",
                "org.freedesktop.PolicyKit1.AuthenticationAgent")

public:
    enum class Attachment {
        // polkitd accepted this agent for this session.
        Registered,
        // polkitd refused, is not there, or another agent holds the session.
        Refused,
        // No system bus at all: the shell runs, authorization prompts do not.
        NoBus,
    };

    explicit PolkitAgent(QObject *parent = nullptr);
    ~PolkitAgent() override;

    // Exports this object and registers it for `sessionId`. Registration is
    // repeated whenever polkitd reappears on the bus, so a restarted polkitd
    // finds this session served again without the shell restarting.
    Attachment attach(const QDBusConnection &bus, const QString &sessionId);

    // How many requests are being answered right now. polkitd may ask more
    // than once at a time and each request is its own conversation.
    int pendingCount() const { return int(m_pending.size()); }

    // The path this object is exported at. Fixed, because it is half of what
    // registration told polkitd.
    static QString objectPath();

public slots:
    // polkit's own interface. `details` is what the action's policy file
    // declares, and is passed through untouched.
    void BeginAuthentication(const QString &actionId, const QString &message,
                             const QString &iconName,
                             const QMap<QString, QString> &details,
                             const QString &cookie,
                             const QList<PolkitIdentity> &identities);
    void CancelAuthentication(const QString &cookie);

public:
    // The prompt's half of the seam. `respond` answers the request polkitd
    // asked about; `dismiss` abandons it, which is a refusal to answer rather
    // than a wrong answer and is reported to polkitd as a cancellation.
    void respond(const QString &cookie, QString secret);
    void dismiss(const QString &cookie);

signals:
    // A request arrived and needs a person. The surface that shows it is
    // `R8-P-C`; this signal is the whole of what it is told, and every string
    // in it came from polkitd.
    void authenticationRequested(const QString &cookie, const QString &actionId,
                                 const QString &message,
                                 const QString &iconName,
                                 const QString &identity);
    // What PAM asked for this request, relayed exactly.
    void secretRequested(const QString &cookie, const QString &prompt);
    void visibleRequested(const QString &cookie, const QString &prompt);
    void informed(const QString &cookie, const QString &text);
    void problemReported(const QString &cookie, const QString &text);
    // The request is over. `authorized` is true only when the helper said so.
    void authenticationFinished(const QString &cookie, bool authorized);

    void registeredChanged(bool registered);

private:
    struct Request {
        // The call itself, not a reply built from it: a reply and an error
        // are both made from the original message, and only one of them is
        // ever sent.
        QDBusMessage call;
        PolkitConversation *conversation = nullptr;
    };

    bool registerWithAuthority();
    void refuse(const QString &name, const QString &text);
    void finish(const QString &cookie, bool authorized, const QString &error);

    QDBusConnection m_bus;
    QDBusServiceWatcher *m_watcher = nullptr;
    QString m_sessionId;
    QString m_user;
    bool m_registered = false;
    QHash<QString, Request> m_pending;
};
