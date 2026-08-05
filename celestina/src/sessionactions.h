#pragma once

#include <QHash>
#include <QObject>
#include <QPointer>
#include <QString>

class ShellService;

// The session menu's one way to ask for something.
//
// It exists so a QML surface never touches the exported D-Bus object: that
// object's slots are the session's public interface, and a window holding it
// could call anything on it. This exposes exactly one verb-shaped request and
// reports exactly the outcomes that request produced, keyed by verb because
// that is what the menu is about.
class SessionActions final : public QObject
{
    Q_OBJECT

public:
    explicit SessionActions(ShellService *shell, QObject *parent = nullptr);

    // Asks the shell for one session verb. A verb that could not even be sent
    // reports `failed` at once rather than leaving the menu waiting.
    Q_INVOKABLE void send(const QString &verb);

signals:
    void commandOutcome(const QString &verb, const QString &state, const QString &reason);

private:
    QPointer<ShellService> m_shell;
    // Which verb each in-flight request was, so an outcome can name it.
    QHash<qulonglong, QString> m_pending;
};
