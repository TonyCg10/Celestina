#include <QtTest>

#include "trayicons.h"

class TrayIconsTest final : public QObject
{
    Q_OBJECT

private slots:
    void readsTheSessionsIconThemeName();
    void refusesTrayPixmapsItCannotTrust();
    void picksTheSizeClosestToWhatIsDrawn();
    void convertsFromTheSpecificationsByteOrder();
};

void TrayIconsTest::readsTheSessionsIconThemeName()
{
    // The shape this session's GTK settings actually have.
    QCOMPARE(
        parseGtkIconThemeName(QStringLiteral("[Settings]\ngtk-icon-theme-name=Adwaita\n")),
        QStringLiteral("Adwaita")
    );
    QCOMPARE(
        parseGtkIconThemeName(QStringLiteral("gtk-icon-theme-name = Papirus-Dark  \n")),
        QStringLiteral("Papirus-Dark")
    );

    // A commented-out setting is not one, and a theme name is a directory name
    // rather than a path to follow.
    QVERIFY(parseGtkIconThemeName(QStringLiteral("#gtk-icon-theme-name=Adwaita\n")).isEmpty());
    QVERIFY(parseGtkIconThemeName(QStringLiteral("gtk-icon-theme-name=../../etc\n")).isEmpty());
    QVERIFY(parseGtkIconThemeName(QStringLiteral("gtk-icon-theme-name=\n")).isEmpty());
    QVERIFY(parseGtkIconThemeName(QString()).isEmpty());
}

void TrayIconsTest::refusesTrayPixmapsItCannotTrust()
{
    // Sizes that disagree with the byte count, and one large enough to be a
    // memory claim rather than an icon.
    QVERIFY(bestTrayPixmap({TrayPixmap {2, 2, QByteArray(4, '\0')}}, 22).isNull());
    QVERIFY(bestTrayPixmap({TrayPixmap {0, 0, QByteArray()}}, 22).isNull());
    QVERIFY(bestTrayPixmap({TrayPixmap {-1, 4, QByteArray(16, '\0')}}, 22).isNull());
    QVERIFY(bestTrayPixmap({TrayPixmap {4096, 4096, QByteArray()}}, 22).isNull());
    QVERIFY(bestTrayPixmap({}, 22).isNull());
}

void TrayIconsTest::picksTheSizeClosestToWhatIsDrawn()
{
    const auto pixmap = [](int size) {
        return TrayPixmap {size, size, QByteArray(size * size * 4, '\0')};
    };

    // The smallest that still covers the drawn size.
    QCOMPARE(bestTrayPixmap({pixmap(16), pixmap(24), pixmap(64)}, 22).width(), 24);
    // Nothing covers it: the largest there is, because growing a small icon
    // looks worse than shrinking a large one.
    QCOMPARE(bestTrayPixmap({pixmap(8), pixmap(16)}, 22).width(), 16);
    QCOMPARE(bestTrayPixmap({pixmap(22)}, 22).width(), 22);
}

void TrayIconsTest::convertsFromTheSpecificationsByteOrder()
{
    // One opaque red pixel, most significant byte first, as the specification
    // requires and as this machine does not read.
    QByteArray argb;
    argb.append(char(0xff)).append(char(0xff)).append(char(0x00)).append(char(0x00));

    const QImage image = bestTrayPixmap({TrayPixmap {1, 1, argb}}, 22);

    QCOMPARE(image.size(), QSize(1, 1));
    QCOMPARE(image.pixelColor(0, 0), QColor(255, 0, 0, 255));
}

QTEST_GUILESS_MAIN(TrayIconsTest)

#include "trayicons_test.moc"
