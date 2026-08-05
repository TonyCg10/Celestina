#include "sessionactions.h"

#include "shellservice.h"

SessionActions::SessionActions(ShellService *shell, QObject *parent)
    : QObject(parent)
    , m_shell(shell)
{
    if (!m_shell)
        return;

    connect(
        m_shell,
        &ShellService::CommandResult,
        this,
        [this](qulonglong requestId, const QString &state, const QVariantMap &details) {
            const auto pending = m_pending.constFind(requestId);
            if (pending == m_pending.constEnd())
                return;

            emit commandOutcome(
                pending.value(),
                state,
                details.value(QStringLiteral("reason")).toString()
            );
            // "pending" is the only state a request leaves behind.
            if (state != QStringLiteral("pending"))
                m_pending.remove(requestId);
        }
    );
}

void SessionActions::send(const QString &verb)
{
    if (!m_shell) {
        emit commandOutcome(
            verb,
            QStringLiteral("failed"),
            tr("this shell has no session channel")
        );
        return;
    }

    const qulonglong requestId = m_shell->Command(verb, QVariantMap());
    if (requestId == 0) {
        // The shell refused or could not send it. `Command` reports the reason
        // on the bus; in-process the menu is told the request did not happen.
        emit commandOutcome(verb, QStringLiteral("failed"), QString());
        return;
    }
    m_pending.insert(requestId, verb);
}
