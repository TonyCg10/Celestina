#include "shellscale.h"

#include <QByteArray>
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

// The author's own answer, when the derived one is wrong for them.
//
// A density is the best automatic proxy for how large something looks, and it
// is not the whole of it: a television at sofa distance and a monitor at
// arm's length can share a density and want very different sizes, and an
// output whose EDID simply lies cannot be argued with. This is also what lets
// an automated run pin the size, so a contract about where a menu lands is not
// quietly rewritten by whatever density the test platform reports.
const char *const scaleOverrideVariable = "CELESTINA_SHELL_SCALE";
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

    // `physicalDotsPerInch` divides the output's logical width by its real
    // width, so the compositor's own scale is already accounted for and only
    // the panel's density is left.
    return screen ? shellScaleForDensity(screen->physicalDotsPerInch()) : 1.0;
}
