#include <QtTest>

#include "shellscale.h"

// What one logical pixel is worth on a real monitor.
//
// The shell's tokens are logical pixels and a logical pixel is not a length.
// These cases hold the author's own three outputs as the measurement that
// started this: the same 40-token bar is 12.50 mm on the 27" 1080p panel that
// looks right and 10.94 mm on the 32" 4K panel at scale 1.5, which is what
// the author reported as looking smaller and uncomfortable there.
class ShellScaleTest final : public QObject
{
    Q_OBJECT

private slots:
    void theAuthorsOwnOutputsGetTheFactorTheirSizesAskFor();
    void oneReferenceOutputIsLeftExactlyAsItWas();
    void anUnbelievableReadingChangesNothing();
    void aPlausibleExtremeIsBoundedRatherThanObeyed();
    void factorsSettleOnAStepSoSimilarMonitorsAgree();
    void aNamedNumberWinsOverTheDerivedOne();
};

namespace {
// `physicalDotsPerInch` is the output's logical width over its real width, so
// the compositor's scale is already divided out. These are the three EDIDs the
// author's session reports.
constexpr double hp27Inch1080p = 1920.0 / (600.0 / 25.4);      // 81.3
constexpr double lg24Inch1080p = 1920.0 / (530.0 / 25.4);      // 92.0
constexpr double lg32Inch4kAt150 = 2560.0 / (700.0 / 25.4);    // 92.9
} // namespace

void ShellScaleTest::theAuthorsOwnOutputsGetTheFactorTheirSizesAskFor()
{
    // Both LG panels draw the bar about 13 % smaller than the HP does. The
    // factor has to give that back, and it must not depend on which of them
    // happens to be the primary output.
    QCOMPARE(shellScaleForDensity(lg24Inch1080p), 1.15);
    QCOMPARE(shellScaleForDensity(lg32Inch4kAt150), 1.15);

    // Stated as the thing the author actually sees: the bar's physical height.
    const double referenceMillimetres = 40.0 / (hp27Inch1080p / 25.4);
    for (const double density : {lg24Inch1080p, lg32Inch4kAt150}) {
        const double scaled = 40.0 * shellScaleForDensity(density);
        const double millimetres = scaled / (density / 25.4);
        // Within a third of a millimetre of the output that looks right,
        // which is as close as a 0.05 step allows.
        QVERIFY2(
            qAbs(millimetres - referenceMillimetres) < 0.35,
            qPrintable(QStringLiteral("%1 mm against %2 mm")
                           .arg(millimetres).arg(referenceMillimetres))
        );
    }
}

void ShellScaleTest::oneReferenceOutputIsLeftExactlyAsItWas()
{
    // The tokens were drawn against this density, so the monitor the author
    // is happy with must not move at all when the rule arrives.
    QCOMPARE(shellScaleForDensity(hp27Inch1080p), 1.0);
}

void ShellScaleTest::anUnbelievableReadingChangesNothing()
{
    // Televisions and virtual outputs routinely publish no physical size, and
    // some publish a diagonal of millimetres. None of those is a density to
    // resize a shell from, so the shell keeps the size it has.
    for (const double density : {0.0, -1.0, 5.0, 39.9, 400.1, 10000.0})
        QCOMPARE(shellScaleForDensity(density), 1.0);

    QCOMPARE(shellScaleForDensity(qQNaN()), 1.0);
    QCOMPARE(shellScaleForDensity(qInf()), 1.0);
    // A null screen is the same refusal by another route.
    QCOMPARE(shellScaleForScreen(nullptr), 1.0);
}

void ShellScaleTest::aPlausibleExtremeIsBoundedRatherThanObeyed()
{
    // A believable but very dense or very coarse panel is clamped: past these
    // the shell would eat a small screen or vanish on a coarse one.
    QCOMPARE(shellScaleForDensity(399.0), 1.75);
    QCOMPARE(shellScaleForDensity(41.0), 0.85);
}

void ShellScaleTest::factorsSettleOnAStepSoSimilarMonitorsAgree()
{
    // Two monitors a hair apart in density must not end up fractions of a
    // pixel apart in every derived metric.
    QCOMPARE(shellScaleForDensity(92.0), shellScaleForDensity(93.0));

    // And every factor really is on the step.
    for (const double density : {81.3, 92.0, 96.0, 110.0, 140.0, 200.0}) {
        const double scale = shellScaleForDensity(density);
        QCOMPARE(qRound(scale * 100.0) % 5, 0);
    }
}

void ShellScaleTest::aNamedNumberWinsOverTheDerivedOne()
{
    // Density is the best automatic proxy for how large something looks and
    // not the whole of it: no monitor publishes how far away it is being read
    // from, and some publish a physical size that is simply wrong. An author
    // naming a number is answering a question the EDID cannot.
    QCOMPARE(shellScaleOverride("1.3"), 1.3);
    QCOMPARE(shellScaleOverride(" 1.3 "), 1.3);
    // Named, so taken as named: the step exists to stop two similar monitors
    // disagreeing by a fraction, not to round an instruction.
    QCOMPARE(shellScaleOverride("1.32"), 1.32);
    // Bounded like every derived factor.
    QCOMPARE(shellScaleOverride("9"), 1.75);
    QCOMPARE(shellScaleOverride("0.1"), 0.85);

    // Zero means "nothing was asked for", so the derived factor stands. An
    // unreadable or absurd request must never resize the shell to nothing.
    QCOMPARE(shellScaleOverride(nullptr), 0.0);
    QCOMPARE(shellScaleOverride(""), 0.0);
    QCOMPARE(shellScaleOverride("   "), 0.0);
    QCOMPARE(shellScaleOverride("large"), 0.0);
    QCOMPARE(shellScaleOverride("0"), 0.0);
    QCOMPARE(shellScaleOverride("-2"), 0.0);
}

QTEST_APPLESS_MAIN(ShellScaleTest)
#include "shellscale_test.moc"
