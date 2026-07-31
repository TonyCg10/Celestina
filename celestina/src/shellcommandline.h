#pragma once

#include <QString>
#include <QStringList>
#include <QVariantMap>

// The parsed form of `celestina msg <verb> [key=value ...]`.
//
// Parsing is deliberately separate from the bus client: it decides nothing
// about which verbs exist — the running shell is the only authority on that —
// and only turns a bounded argument list into the `a{sv}` the `Command` method
// takes. `error` is empty exactly when the line is usable.
struct ShellCommandLine {
    QString verb;
    QVariantMap options;
    QString error;
    // The one reserved read verb: it calls `GetState` instead of `Command`,
    // so a caller can inspect the shell without asking it to do anything.
    bool readsState = false;
};

ShellCommandLine parseShellCommandLine(const QStringList &arguments);
