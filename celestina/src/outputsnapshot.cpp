#include "outputsnapshot.h"

#include <QGuiApplication>
#include <QRect>
#include <QScreen>
#include <QVariantMap>

QVariantList outputScreenSnapshot()
{
    QVariantList screens;
    for (QScreen *screen : QGuiApplication::screens()) {
        const QRect geometry = screen->geometry();
        screens.append(QVariantMap {
            {QStringLiteral("name"), screen->name()},
            {QStringLiteral("width"), geometry.width()},
            {QStringLiteral("height"), geometry.height()},
            {QStringLiteral("devicePixelRatio"), screen->devicePixelRatio()},
        });
    }
    return screens;
}
