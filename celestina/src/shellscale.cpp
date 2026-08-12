#include "shellscale.h"

#include <QScreen>
#include <QtGlobal>

#include <cmath>

namespace {
// The density the shell's tokens were drawn against: the author's 27" 1080p
// panel, 600 mm wide, which is the output they describe as correctly sized.
// Every other output is expressed relative to it, so that monitor keeps its
// present appearance exactly and only the ones that differ move.
constexpr double referenceDotsPerInch = 81.3;

// Below the lower bound a shell would shrink past legibility on a very coarse
// output; above the upper one it would eat a small screen. Both are refusals
// to act on an implausible reading, not opinions about taste.
constexpr double minimumScale = 0.85;
constexpr double maximumScale = 1.75;

// Sizes settle on a step rather than on whatever a millimetre reading happens
// to produce, so two similar monitors do not end up a third of a pixel apart
// and every derived metric stays reproducible.
constexpr double scaleStep = 0.05;

// A physical size no real desktop monitor has. Televisions and virtual
// outputs frequently report zero, and some report a diagonal of a few
// millimetres; neither is a density this may divide by.
constexpr double minimumSensibleDotsPerInch = 40.0;
constexpr double maximumSensibleDotsPerInch = 400.0;
} // namespace

double shellScaleForDensity(double density)
{
    if (!std::isfinite(density)
        || density < minimumSensibleDotsPerInch
        || density > maximumSensibleDotsPerInch) {
        // Nothing reliable was published, so the shell keeps the size it has
        // rather than resizing itself from a number it cannot believe.
        return 1.0;
    }

    const double exact = density / referenceDotsPerInch;
    const double stepped = std::round(exact / scaleStep) * scaleStep;
    return qBound(minimumScale, stepped, maximumScale);
}

double shellScaleForScreen(const QScreen *screen)
{
    // `physicalDotsPerInch` divides the output's logical width by its real
    // width, so the compositor's own scale is already accounted for and only
    // the panel's density is left.
    return screen ? shellScaleForDensity(screen->physicalDotsPerInch()) : 1.0;
}
