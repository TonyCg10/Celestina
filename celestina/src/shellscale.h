#pragma once

class QScreen;

// How much larger the shell must draw itself on one output so that what it
// draws is the same physical size everywhere.
//
// The shell's tokens are logical pixels, and a logical pixel is not a fixed
// length: it is whatever the output's density and the compositor's scale make
// it. On the author's session the same 40-token bar measures 12.50 mm on a 27"
// 1080p panel and 10.94 mm on a 32" 4K panel at scale 1.5 — 13 % smaller on the
// larger monitor, which is also the one viewed from further away. That is the
// defect this corrects, and it is arithmetic rather than taste.
//
// `QScreen::physicalDotsPerInch()` is already the number needed: it divides the
// output's logical width by its real width, so the compositor's scale is
// accounted for and only the panel's own density is left. Dividing that by one
// reference density gives the factor directly.
//
// A missing or absurd EDID physical size is common enough that it cannot be
// trusted blindly; an output that reports one is left at 1.0 rather than
// scaled by a guess.
//
// The arithmetic is separate from the screen it is usually read from, because
// the arithmetic is the decision — the reference density, the bounds and what
// counts as an unbelievable reading — and a QScreen cannot be constructed to
// state it in a test.
double shellScaleForDensity(double dotsPerInch);
// The author's own answer, read from `CELESTINA_SHELL_SCALE`, or zero when
// nothing readable was asked for. Density is the best automatic proxy for how
// large something looks and not the whole of it — viewing distance is not
// published by any monitor — so a named number wins over a derived one.
double shellScaleOverride(const char *requested);
double shellScaleForScreen(const QScreen *screen);
