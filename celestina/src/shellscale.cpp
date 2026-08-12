#include "shellscale.h"

#include <QByteArray>
#include <QScreen>
#include <QSizeF>
#include <QtGlobal>

#include <cmath>

namespace {
// The monitor the shell's sizes were drawn against: the author's 27" panel,
// 600 x 340 mm, which they describe as correctly sized. Every other output is
// expressed relative to it, so that one keeps its present appearance exactly
// and only the monitors that differ move.
const double referenceDiagonalMillimetres = std::hypot(600.0, 340.0);

// Never smaller than the reference. Scaling down with size was tried against
// the author's own monitors and rejected: their 24" panel resolves to 0.88 by
// size alone and they asked for 1.00. A smaller screen is not read from
// proportionally closer, because a desk has a front edge.
constexpr double minimumScale = 1.0;
// Above this a shell would eat the screen it is meant to stay out of. It is a
// refusal to act on an extreme reading, not an opinion about taste.
constexpr double maximumScale = 1.75;

// Sizes settle on a step rather than on whatever a millimetre reading happens
// to produce, so two similar monitors do not end up a third of a pixel apart
// and every derived metric stays reproducible.
constexpr double scaleStep = 0.05;

// A diagonal no desktop monitor has. Televisions and virtual outputs
// frequently report zero, and some report a few millimetres.
constexpr double minimumSensibleDiagonal = 250.0;   // about 10"
constexpr double maximumSensibleDiagonal = 1600.0;  // about 63"

// The densities Qt invents when the compositor publishes no physical size.
//
// This is not a range check — the size Qt fabricates is perfectly plausible,
// which is exactly why the first version of this file walked into it. A nested
// Niri publishes no size for its `winit` output, Qt filled in 481.6 x 253.5 mm
// for a 1896-pixel-wide screen, and that is 100.00 dpi to the last digit
// because it was computed backwards from 100. The shell then "measured" a
// factor and drew itself a quarter larger than the session beside it.
//
// A fabricated density is exact by construction; a real EDID reports whole
// millimetres and essentially never lands on one of these to within a hair.
// Treating them as no reading at all costs a genuine 96 or 100 dpi monitor its
// adjustment, which is the safe direction: it keeps the size it already had
// rather than being resized from a number nobody measured.
constexpr double qtFallbackDotsPerInch[] = {96.0, 100.0};
constexpr double fabricatedDensityEpsilon = 0.01;

bool looksFabricated(double density)
{
    if (!std::isfinite(density))
        return true;

    for (const double fallback : qtFallbackDotsPerInch) {
        if (std::abs(density - fallback) < fabricatedDensityEpsilon)
            return true;
    }
    return false;
}

// The author's own answer, when the derived one is wrong for them.
//
// Physical size is the best automatic proxy for how large something should
// look, and it is not the whole of it: a television at sofa distance and a
// monitor at arm's length can share a diagonal and want very different sizes.
// This is also what lets an automated run pin the size, so a contract about
// where a menu lands is not quietly rewritten by whatever the test platform
// reports.
const char *const scaleOverrideVariable = "CELESTINA_SHELL_SCALE";
} // namespace

double shellScaleForOutput(double diagonalMillimetres, double dotsPerInch)
{
    if (!std::isfinite(diagonalMillimetres)
        || diagonalMillimetres < minimumSensibleDiagonal
        || diagonalMillimetres > maximumSensibleDiagonal
        || looksFabricated(dotsPerInch)) {
        // Nothing reliable was published, so the shell keeps the size it has
        // rather than resizing itself from a number it cannot believe.
        return 1.0;
    }

    const double exact = diagonalMillimetres / referenceDiagonalMillimetres;
    const double stepped = std::round(exact / scaleStep) * scaleStep;
    return qBound(minimumScale, stepped, maximumScale);
}

double shellScaleOverride(const char *requested)
{
    if (!requested)
        return 0.0;

    bool readable = false;
    const double asked = QByteArray(requested).trimmed().toDouble(&readable);
    if (!readable || !std::isfinite(asked) || asked <= 0.0) {
        // An unreadable request is not an instruction to resize the shell to
        // nothing; the derived factor stands.
        return 0.0;
    }

    // Bounded like every derived factor, and deliberately not stepped: an
    // author naming a number means that number.
    return qBound(minimumScale, asked, maximumScale);
}

double shellScaleForScreen(const QScreen *screen)
{
    const double overridden =
        shellScaleOverride(qgetenv(scaleOverrideVariable).constData());
    if (overridden > 0.0)
        return overridden;

    if (!screen)
        return 1.0;

    const QSizeF size = screen->physicalSize();
    return shellScaleForOutput(
        std::hypot(size.width(), size.height()),
        screen->physicalDotsPerInch()
    );
}
