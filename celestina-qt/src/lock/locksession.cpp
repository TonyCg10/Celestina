#include "locksession.h"

namespace {
LockSession *g_session = nullptr;
}

LockSession::LockSession(QObject *parent)
    : QObject(parent)
{
    g_session = this;
}

LockSession *LockSession::instance()
{
    return g_session;
}

void LockSession::release()
{
    if (!m_confirmed || m_released || !m_unlock)
        return;
    m_released = true;
    m_unlock();
}

void LockSession::markConfirmed()
{
    m_confirmed = true;
    emit confirmed();
}

void LockSession::markFinished()
{
    // Already gone as far as the compositor is concerned: releasing now would
    // be a use-after-destroy, so the release path is closed rather than left
    // to a later caller's good judgement.
    m_confirmed = false;
    m_released = true;
    emit finished();
}
