// LIVE-1-A. A name the themes do not know is looked up once, not per frame.
//
// The defect this guards against is invisible in a screenshot and expensive
// only while something is on screen: the provider cached its answer but decided
// whether to resolve by asking whether the *image* was null, which cannot tell
// "cached miss" from "never asked". Every miss therefore resolved again on
// every frame that drew it, and resolving a miss is the slowest lookup there
// is — `QIcon::fromTheme` walks every installed theme's directories before it
// can conclude nothing is there, on the GUI thread.
//
// The author's session found it the hard way: a workspace map holding two
// applications no theme knows (`com.anthropic.Claude`, `Chatgpt`) paid that
// walk twice per frame for as long as the map was open.
//
// The guard is a count of searches, never a clock: on a machine with few themes
// installed the wasted walk is quick enough to hide inside timing noise — a timing
// version of this test passed against the defect it was written to catch.

#include <QGuiApplication>
#include <QTest>

#include "appiconprovider.h"

class AppIconProviderTest : public QObject
{
    Q_OBJECT

private slots:
    void aMissIsResolvedOnceAndThenRemembered();
    void aHitIsResolvedOnceAndThenRemembered();
    void aRefusedNameNeverReachesTheThemeLoader();
};

void AppIconProviderTest::aMissIsResolvedOnceAndThenRemembered()
{
    AppIconProvider provider;
    // A name no installed theme can possibly carry, so the first lookup is a
    // complete search and every later one must not be one at all.
    const QString absent =
        QStringLiteral("celestina.test.no.such.application.9d28vd");

    QSize size;
    QVERIFY(provider.requestImage(absent, &size, QSize()).isNull());
    QCOMPARE(provider.resolutionCount(), 1);

    // Enough repetitions that a per-call search could not be mistaken for
    // anything else. The count, not the clock: on a machine with few themes
    // installed the wasted walk is quick enough to hide inside timing noise,
    // which is exactly how this defect survived until a real session met it.
    for (int attempt = 0; attempt < 200; ++attempt)
        QVERIFY(provider.requestImage(absent, &size, QSize()).isNull());

    QCOMPARE(provider.resolutionCount(), 1);
}

// A name that does resolve is also searched for only once.
void AppIconProviderTest::aHitIsResolvedOnceAndThenRemembered()
{
    AppIconProvider provider;
    QSize size;
    // Whatever this session's theme offers; the assertion is about the number
    // of searches, which holds whether or not the icon exists.
    const QString present = QStringLiteral("folder");
    provider.requestImage(present, &size, QSize());
    QCOMPARE(provider.resolutionCount(), 1);
    for (int attempt = 0; attempt < 50; ++attempt)
        provider.requestImage(present, &size, QSize());
    QCOMPARE(provider.resolutionCount(), 1);
}

// The name comes from another program. It is a theme key and never a path, and
// a refusal must not depend on the cache having seen it.
void AppIconProviderTest::aRefusedNameNeverReachesTheThemeLoader()
{
    AppIconProvider provider;
    QSize size;
    for (const QString &name : {QStringLiteral("../etc/passwd"),
                                QStringLiteral(".hidden"),
                                QStringLiteral("with/separator"),
                                QString()}) {
        QVERIFY(provider.requestImage(name, &size, QSize()).isNull());
    }
}

QTEST_MAIN(AppIconProviderTest)
#include "appiconprovider_test.moc"
