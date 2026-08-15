#include "polkitagent.h"

#include "polkitconversation.h"

#include <QDBusConnectionInterface>
#include <QDBusMetaType>
#include <QDBusReply>
#include <QDBusServiceWatcher>
#include <QLocale>
#include <QLoggingCategory>

#include <pwd.h>
#include <unistd.h>

namespace {

// polkit's own names. None of these are this project's to choose.
const QString authorityService =
    QStringLiteral("org.freedesktop.PolicyKit1");
const QString authorityPath =
    QStringLiteral("/org/freedesktop/PolicyKit1/Authority");
const QString authorityInterface =
    QStringLiteral("org.freedesktop.PolicyKit1.Authority");
const QString cancelledError =
    QStringLiteral("org.freedesktop.PolicyKit1.Error.Cancelled");

// The account whose passphrase the helper will be asked about. polkitd sends
// identities as uids; the helper wants a name, and `/etc/passwd` is where that
// mapping lives rather than anywhere this shell could invent it.
QString nameForUid(uint uid)
{
    const struct passwd *entry = ::getpwuid(uid);
    if (!entry || !entry->pw_name)
        return QString();
    return QString::fromLocal8Bit(entry->pw_name);
}

// Which identity to offer. The person sitting here is the one who can answer,
// so their own account is preferred when polkitd offers it; otherwise the
// first `unix-user` polkitd named is used, unchanged. Group identities are
// skipped rather than guessed at — asking "which member of this group are
// you" is a question this unit does not have a surface for.
QString chooseIdentity(const QList<PolkitIdentity> &identities)
{
    const uint self = uint(::getuid());
    QString fallback;
    for (const PolkitIdentity &identity : identities) {
        if (identity.kind != QLatin1String("unix-user"))
            continue;
        const QVariant uid = identity.details.value(QStringLiteral("uid"));
        if (!uid.isValid())
            continue;
        const QString name = nameForUid(uid.toUInt());
        if (name.isEmpty())
            continue;
        if (uid.toUInt() == self)
            return name;
        if (fallback.isEmpty())
            fallback = name;
    }
    return fallback;
}

} // namespace

QDBusArgument &operator<<(QDBusArgument &argument, const PolkitIdentity &value)
{
    argument.beginStructure();
    argument << value.kind << value.details;
    argument.endStructure();
    return argument;
}

const QDBusArgument &operator>>(const QDBusArgument &argument,
                                PolkitIdentity &value)
{
    argument.beginStructure();
    argument >> value.kind >> value.details;
    argument.endStructure();
    return argument;
}

QString PolkitAgent::objectPath()
{
    return QStringLiteral("/org/celestina/PolkitAgent1");
}

PolkitAgent::PolkitAgent(QObject *parent)
    : QObject(parent)
    , m_bus(QDBusConnection::systemBus())
{
    qDBusRegisterMetaType<PolkitIdentity>();
    qDBusRegisterMetaType<QList<PolkitIdentity>>();
    qDBusRegisterMetaType<QMap<QString, QString>>();
    m_user = nameForUid(uint(::getuid()));
}

PolkitAgent::~PolkitAgent()
{
    // Every request still open is abandoned, and abandoned means cancelled:
    // a shell going away has not authorized anything.
    const QList<QString> cookies = m_pending.keys();
    for (const QString &cookie : cookies)
        finish(cookie, false, cancelledError);
}

PolkitAgent::Attachment PolkitAgent::attach(const QDBusConnection &bus,
                                            const QString &sessionId)
{
    m_bus = bus;
    m_sessionId = sessionId;

    if (!m_bus.isConnected()) {
        qWarning().noquote()
            << "Celestina found no bus for its polkit agent; authorization "
               "prompts are unavailable:"
            << m_bus.lastError().message();
        return Attachment::NoBus;
    }
    if (m_sessionId.isEmpty() || m_user.isEmpty()) {
        qWarning().noquote()
            << "Celestina could not name its own session or user; the polkit "
               "agent is not registered.";
        return Attachment::Refused;
    }

    if (!m_bus.registerObject(objectPath(), this,
                              QDBusConnection::ExportAllSlots)) {
        qWarning().noquote()
            << "Celestina could not export its polkit agent:"
            << m_bus.lastError().message();
        return Attachment::NoBus;
    }

    // polkitd restarting is ordinary — a package upgrade does it. The agent
    // re-registers when it comes back rather than leaving the session without
    // one until the shell is restarted.
    if (!m_watcher) {
        m_watcher = new QDBusServiceWatcher(
            authorityService, m_bus,
            QDBusServiceWatcher::WatchForOwnerChange, this);
        connect(m_watcher, &QDBusServiceWatcher::serviceOwnerChanged, this,
                [this](const QString &, const QString &, const QString &owner) {
                    if (owner.isEmpty()) {
                        // polkitd went away and took the registration with
                        // it. Nothing is pretended about the interval.
                        m_registered = false;
                        emit registeredChanged(false);
                        return;
                    }
                    if (registerWithAuthority())
                        emit registeredChanged(true);
                });
    }

    if (!registerWithAuthority())
        return Attachment::Refused;
    emit registeredChanged(true);
    return Attachment::Registered;
}

bool PolkitAgent::registerWithAuthority()
{
    // The subject is this session, in polkit's own `(sa{sv})` shape — the
    // same wire type an identity has, which is why it is built from that
    // struct rather than a second one that would marshal identically.
    PolkitIdentity subject;
    subject.kind = QStringLiteral("unix-session");
    subject.details.insert(QStringLiteral("session-id"), m_sessionId);

    QDBusMessage call = QDBusMessage::createMethodCall(
        authorityService, authorityPath, authorityInterface,
        QStringLiteral("RegisterAuthenticationAgent"));
    call << QVariant::fromValue(subject)
         << QLocale::system().name()
         << objectPath();

    const QDBusMessage answer = m_bus.call(call, QDBus::Block, 5000);
    m_registered = answer.type() == QDBusMessage::ReplyMessage;
    if (!m_registered) {
        qWarning().noquote()
            << "Celestina's polkit agent was not registered:"
            << answer.errorMessage();
    }
    return m_registered;
}

// A refusal that survives being made in-process. `sendErrorReply` writes into
// the call this object is answering, and outside a D-Bus call there is none —
// it dereferences a connection that is not there and takes the shell down with
// it. The same crash was found in the session verbs' `Suspend`, which is why
// every refusal here goes through one place that checks first.
void PolkitAgent::refuse(const QString &name, const QString &text)
{
    if (calledFromDBus())
        sendErrorReply(name, text);
}

void PolkitAgent::BeginAuthentication(const QString &actionId,
                                      const QString &message,
                                      const QString &iconName,
                                      const QMap<QString, QString> &details,
                                      const QString &cookie,
                                      const QList<PolkitIdentity> &identities)
{
    Q_UNUSED(details)

    // This object sits on a bus every process on the machine can reach, and a
    // prompt asking for a password is exactly what somebody would want to
    // forge. Only the process that owns polkit's own name may ask for one. A
    // forged request could never produce an authorization — the cookie would
    // not be one polkitd issued — but it could produce a convincing prompt,
    // and that is the attack worth refusing.
    if (calledFromDBus()) {
        const QString owner =
            m_bus.interface()
                ? m_bus.interface()->serviceOwner(authorityService).value()
                : QString();
        if (owner.isEmpty() || this->message().service() != owner) {
            refuse(QDBusError::errorString(QDBusError::AccessDenied),
                   QStringLiteral("Only polkit may ask for authentication."));
            return;
        }
    }

    if (cookie.isEmpty() || m_pending.contains(cookie)) {
        refuse(cancelledError,
               QStringLiteral("That request is already being answered."));
        return;
    }

    const QString identity = chooseIdentity(identities);
    if (identity.isEmpty()) {
        // Nobody this shell can ask. Cancelled rather than denied: the person
        // was never given a chance to answer, and pretending otherwise would
        // put a failure in polkitd's log that nobody caused.
        refuse(cancelledError,
               QStringLiteral("No identity this session can ask."));
        return;
    }

    Request request;
    if (calledFromDBus()) {
        // The reply waits for the person. polkit's own agents hold it open the
        // same way — a method that returned immediately would be telling
        // polkitd the prompt was over before it was shown.
        setDelayedReply(true);
        request.call = this->message();
    }

    auto *conversation = new PolkitConversation(this);
    request.conversation = conversation;
    m_pending.insert(cookie, request);

    connect(conversation, &PolkitConversation::secretRequested, this,
            [this, cookie](const QString &prompt) {
                emit secretRequested(cookie, prompt);
            });
    connect(conversation, &PolkitConversation::visibleRequested, this,
            [this, cookie](const QString &prompt) {
                emit visibleRequested(cookie, prompt);
            });
    connect(conversation, &PolkitConversation::informed, this,
            [this, cookie](const QString &text) { emit informed(cookie, text); });
    connect(conversation, &PolkitConversation::problemReported, this,
            [this, cookie](const QString &text) {
                emit problemReported(cookie, text);
            });
    connect(conversation, &PolkitConversation::answered, this,
            [this, cookie](PolkitConversation::Verdict verdict) {
                const bool authorized =
                    verdict == PolkitConversation::Verdict::Authenticated;
                // A verdict that is not an authorization still ends the
                // request normally: polkitd already knows what the helper
                // decided, and an error here would describe the prompt as
                // broken rather than the answer as wrong.
                finish(cookie, authorized, QString());
            });

    emit authenticationRequested(cookie, actionId, message, iconName, identity);
    conversation->start(identity, cookie);
}

void PolkitAgent::CancelAuthentication(const QString &cookie)
{
    if (!m_pending.contains(cookie))
        return;
    finish(cookie, false, cancelledError);
}

void PolkitAgent::respond(const QString &cookie, QString secret)
{
    const auto it = m_pending.constFind(cookie);
    if (it == m_pending.cend()) {
        // Nothing is waiting for this. The answer is wiped rather than kept
        // for a request that might arrive later — there is no such thing as a
        // pre-filled response here.
        auto *at = reinterpret_cast<volatile char16_t *>(secret.data());
        for (qsizetype index = 0; index < secret.size(); ++index)
            at[index] = u'\0';
        secret.clear();
        return;
    }
    it->conversation->respond(std::move(secret));
}

void PolkitAgent::dismiss(const QString &cookie)
{
    if (!m_pending.contains(cookie))
        return;
    finish(cookie, false, cancelledError);
}

void PolkitAgent::finish(const QString &cookie, bool authorized,
                         const QString &error)
{
    const auto it = m_pending.find(cookie);
    if (it == m_pending.end())
        return;

    Request request = *it;
    m_pending.erase(it);

    if (request.conversation) {
        request.conversation->disconnect(this);
        request.conversation->cancel();
        request.conversation->deleteLater();
    }

    if (request.call.type() == QDBusMessage::MethodCallMessage) {
        if (error.isEmpty()) {
            m_bus.send(request.call.createReply());
        } else {
            m_bus.send(request.call.createErrorReply(
                error, QStringLiteral("The prompt was dismissed.")));
        }
    }

    emit authenticationFinished(cookie, authorized);
}
