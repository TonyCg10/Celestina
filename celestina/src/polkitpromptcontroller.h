#pragma once

#include <QHash>
#include <QObject>
#include <QPointer>
#include <QQmlComponent>
#include <QString>

#include "surfacemanager.h"

class PolkitAgent;
class QQmlEngine;
class QWindow;

// Why a request cannot be prompted for, or `None`.
//
// A free function rather than a branch inside the controller, because "may
// this shell ask for a password right now" is the one decision here worth
// testing on its own: a regression that drove the whole controller could pass
// because the component was missing while believing it had proven something
// about the keyboard.
enum class PromptRefusal {
    None,
    // The QML component did not load. Nothing can be shown at all.
    NoComponent,
    // A prompt is already on screen. Two exclusive keyboard grabs on one
    // output is a fight the person watches, not a second prompt.
    AlreadyShowing,
    // No layer shell, so no exclusive grab: a password typed into a surface
    // that does not hold the keyboard can be read by whatever does.
    NoKeyboardGrab,
    // No output to prompt on.
    NoOutput,
};

PromptRefusal promptRefusal(bool componentReady, bool alreadyShowing,
                            LayerShellSupport support, bool hasScreen);
const char *promptRefusalReason(PromptRefusal refusal);

// The surface a person answers an authorization request on, and the rule that
// there is no lesser version of it.
//
// ADR 0005 asks for a dedicated surface holding a keyboard grab. That is not
// decoration: a password typed into a surface that does not hold the keyboard
// can be read by whatever does. So when the grab cannot be taken — no layer
// shell, a compositor that refused the surface, a QML component that failed to
// load — this controller dismisses the request instead of prompting anyway.
// The action then fails exactly as it does on a machine with no graphical
// agent, which is a worse outcome for the person and a safe one.
//
// Everything shown comes from polkitd through `PolkitAgent`: the action id,
// the message, the identity being asked about and PAM's own prompt. Nothing
// here writes a description of what is being authorized, because a shell that
// paraphrased that would be the shell deciding what the person is agreeing to.
class PolkitPromptController final : public QObject
{
    Q_OBJECT

public:
    PolkitPromptController(QQmlEngine *engine, PolkitAgent *agent,
                           QObject *parent = nullptr);

    // False when the component itself failed to load — a broken QML file. The
    // shell then prompts for nothing and every request is dismissed, which is
    // reported rather than discovered.
    bool isEnabled() const { return m_enabled; }

    // Whether a prompt is on screen right now.
    bool isShowing() const { return !m_window.isNull(); }

    // The prompt's answers, called from QML. They carry the cookie implicitly:
    // one prompt is on screen at a time, and it belongs to one request.
    Q_INVOKABLE void respond(QString secret);
    Q_INVOKABLE void dismiss();

private:
    void requested(const QString &cookie, const QString &actionId,
                   const QString &message, const QString &iconName,
                   const QString &identity);
    void finished(const QString &cookie);
    void closeWindow();

    QQmlEngine *m_engine = nullptr;
    PolkitAgent *m_agent = nullptr;
    QQmlComponent m_component;
    bool m_enabled = false;
    QPointer<QWindow> m_window;
    QString m_cookie;
};
