#include "trayiconprovider.h"

#include <QUrl>

TrayIconProvider::TrayIconProvider(QSharedPointer<TrayIconCache> icons)
    : QQuickImageProvider(QQuickImageProvider::Image)
    , m_icons(std::move(icons))
{
}

QImage TrayIconProvider::requestImage(
    const QString &id,
    QSize *size,
    const QSize &requested
)
{
    // `<key>/<revision>`: the revision only exists to make QML ask again when
    // an application changes its icon without changing its identity.
    const QString key = QUrl::fromPercentEncoding(id.section(u'/', 0, 0).toUtf8());
    QImage image = m_icons->take(key);
    if (image.isNull())
        return image;

    if (requested.isValid() && !requested.isEmpty()) {
        image = image.scaled(
            requested,
            Qt::KeepAspectRatio,
            Qt::SmoothTransformation
        );
    }
    if (size)
        *size = image.size();
    return image;
}
