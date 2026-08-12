#include <QtTest>

#include "shellscale.h"

#include <cmath>

// How large the shell draws itself on a real monitor.
//
// These cases hold the author's own three outputs and their own judgement on
// each, because that judgement is the specification. Two of those monitors
// differ by 1.6 dpi and want visibly different sizes, which is the measurement
// that decided this file corrects by physical size rather than by density.
class ShellScaleTest final : public QObject
{
    Q_OBJECT

private slots:
    void theAuthorsOwnMonitorsGetTheSizeTheyAskedFor();
    void densityCannotSeparateTwoOfThemAndSizeCan();
    void oneReferenceMonitorIsLeftExactlyAsItWas();
    void aSmallerMonitorIsNeverShrunk();
    void anUnbelievableReadingChangesNothing();
    void aSizeQtInventedIsNotAMeasurement();
    void aPlausibleExtremeIsBoundedRatherThanObeyed();
    void factorsSettleOnAStepSoSimilarMonitorsAgree();
    void aNamedNumberWinsOverTheDerivedOne();
};

namespace {
// The three EDIDs the author's session reports, with the density Qt derives
// from each. Diagonals: 27.2", 24.0" and 31.5".
constexpr double hp27Width = 600.0, hp27Height = 340.0, hp27Dpi = 80.98;
constexpr double lg24Width = 530.0, lg24Height = 300.0, lg24Dpi = 91.73;
constexpr double lg32Width = 700.0, lg32Height = 390.0, lg32Dpi = 93.34;

double diagonal(double width, double height)
{
    return std::hypot(width, height);
}
} // namespace

void ShellScaleTest::theAuthorsOwnMonitorsGetTheSizeTheyAskedFor()
{
    // Each of these is a decision the author made looking at the real panel,
    // not a number derived and then accepted.
    QCOMPARE(shellScaleForOutput(diagonal(hp27Width, hp27Height), hp27Dpi), 1.0);
    QCOMPARE(shellScaleForOutput(diagonal(lg24Width, lg24Height), lg24Dpi), 1.0);
    QCOMPARE(shellScaleForOutput(diagonal(lg32Width, lg32Height), lg32Dpi), 1.15);
}

void ShellScaleTest::densityCannotSeparateTwoOfThemAndSizeCan()
{
    // The measurement that decided the model. The 24" and the 32" are 1.6 dpi
    // apart — a difference no rule could act on — and the author wants 1.00 and
    // 1.15. Their diagonals are 24.0" and 31.5", which is a real difference.
    QVERIFY(std::abs(lg24Dpi - lg32Dpi) < 2.0);
    QVERIFY(diagonal(lg32Width, lg32Height)
            > diagonal(lg24Width, lg24Height) * 1.25);
    QVERIFY(shellScaleForOutput(diagonal(lg32Width, lg32Height), lg32Dpi)
            > shellScaleForOutput(diagonal(lg24Width, lg24Height), lg24Dpi));
}

void ShellScaleTest::oneReferenceMonitorIsLeftExactlyAsItWas()
{
    // The monitor the sizes were drawn against must not move at all, or every
    // surface the author already approved changes underneath them.
    QCOMPARE(shellScaleForOutput(diagonal(hp27Width, hp27Height), hp27Dpi), 1.0);
}

void ShellScaleTest::aSmallerMonitorIsNeverShrunk()
{
    // By size alone the 24" resolves to 0.88. The author asked for 1.00: a
    // smaller screen is not read from proportionally closer, because a desk
    // has a front edge.
    const double bySizeAlone = diagonal(lg24Width, lg24Height)
                               / diagonal(hp27Width, hp27Height);
    QVERIFY(bySizeAlone < 0.9);
    QCOMPARE(shellScaleForOutput(diagonal(lg24Width, lg24Height), lg24Dpi), 1.0);
    // And nothing smaller than the reference exists at all.
    QCOMPARE(shellScaleForOutput(300.0, 120.0), 1.0);
}

void ShellScaleTest::anUnbelievableReadingChangesNothing()
{
    // Televisions and virtual outputs routinely publish zero, and some publish
    // a diagonal of millimetres. Neither is a size this may divide by.
    QCOMPARE(shellScaleForOutput(0.0, 96.5), 1.0);
    QCOMPARE(shellScaleForOutput(-500.0, 96.5), 1.0);
    QCOMPARE(shellScaleForOutput(5.0, 96.5), 1.0);
    QCOMPARE(shellScaleForOutput(std::nan(""), 96.5), 1.0);
    // A wall-sized panel is refused rather than obeyed.
    QCOMPARE(shellScaleForOutput(3000.0, 96.5), 1.0);
}

void ShellScaleTest::aSizeQtInventedIsNotAMeasurement()
{
    // When a compositor publishes no physical size, Qt fills one in, and what
    // comes back describes Qt rather than the monitor. The fabricated size is
    // perfectly plausible — a range check cannot catch it — but the density it
    // was computed backwards from is exact. A nested Niri produced 481.6 x
    // 253.5 mm for its 1896-pixel output, which is 100.00 dpi to the last
    // digit, and the shell drew itself a quarter larger than the session
    // beside it.
    QCOMPARE(shellScaleForOutput(diagonal(481.6, 253.5), 100.0), 1.0);
    QCOMPARE(shellScaleForOutput(diagonal(700.0, 390.0), 96.0), 1.0);
    // Exact by construction, so only a hair's width counts as fabricated: a
    // real monitor landing near one keeps its own reading.
    QCOMPARE(shellScaleForOutput(diagonal(lg32Width, lg32Height), 100.5), 1.15);
}

void ShellScaleTest::aPlausibleExtremeIsBoundedRatherThanObeyed()
{
    // A 55" panel at the far end of the believable range would ask for more
    // than twice the reference; it is capped instead.
    QCOMPARE(shellScaleForOutput(diagonal(1210.0, 680.0), 60.0), 1.75);
}

void ShellScaleTest::factorsSettleOnAStepSoSimilarMonitorsAgree()
{
    // Two panels of nearly the same size must not end up a fraction apart in
    // every metric derived from this.
    for (double width = 560.0; width <= 900.0; width += 3.0) {
        const double scale = shellScaleForOutput(diagonal(width, width * 0.5625),
                                                 85.0);
        QCOMPARE(qRound(scale * 100.0) % 5, 0);
    }
}

void ShellScaleTest::aNamedNumberWinsOverTheDerivedOne()
{
    // Physical size is the best automatic proxy and not the whole of it: no
    // monitor publishes how far away it is being read from.
    QCOMPARE(shellScaleOverride("1.3"), 1.3);
    QCOMPARE(shellScaleOverride(" 1.3 "), 1.3);
    // Named, so taken as named: the step exists to stop two similar monitors
    // disagreeing by a fraction, not to round an instruction.
    QCOMPARE(shellScaleOverride("1.32"), 1.32);
    QCOMPARE(shellScaleOverride("9"), 1.75);
    QCOMPARE(shellScaleOverride("0.1"), 1.0);

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
