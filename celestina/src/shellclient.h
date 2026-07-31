#pragma once

#include <QStringList>

// Runs `celestina msg <verb> [key=value ...]` against the shell that owns the
// session. The client is transient: it never claims the bus name, never starts
// a shell and answers on stdout while diagnostics go to stderr.
//
// Exits non-zero on a rejected verb, an unreachable or vanishing shell, a
// failed request and a request that never resolves.
int runShellMessage(const QStringList &arguments);
