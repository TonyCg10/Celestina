#pragma once

#include <QQuickImageProvider>
#include <QSharedPointer>

#include "trayicons.h"

// Serves `image://tray/<key>/<revision>` from what the tray host resolved.
//
// It looks nothing up itself: resolving an icon means asking a theme or reading
// another application's pixels, and both belong to the host that is already
// talking to that application. This only hands over what is already decoded,
// which is why it is safe for Qt to call from its render thread.
class TrayIconProvider final : public QQuickImageProvider
{
public:
    explicit TrayIconProvider(QSharedPointer<TrayIconCache> icons);

    QImage requestImage(const QString &id, QSize *size, const QSize &requested) override;

private:
    QSharedPointer<TrayIconCache> m_icons;
};
