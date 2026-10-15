#include "lockuncover.h"

#include <QTimer>

LockUncover::LockUncover(int ceilingMs, QObject *parent)
    : QObject(parent)
    , m_ceilingMs(ceilingMs < 0 ? 0 : ceilingMs)
{
}

void LockUncover::begin()
{
    if (m_begun)
        return;
    m_begun = true;

    emit retreat();

    // Started before anything can respond to `retreat`, and never restarted.
    // A handler that throws, hangs or does nothing at all cannot affect this:
    // by the time any of them runs, the only thing that uncovers this session
    // is already counting down.
    QTimer::singleShot(m_ceilingMs, this, [this]() { emit uncover(); });
}
