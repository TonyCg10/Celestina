#pragma once

class QScreen;

// How much larger the shell must draw itself on one output.
//
// The shell's tokens are logical pixels, and a logical pixel is not a fixed
// length: it is whatever the output's density and the compositor's scale make
// it. On the author's session the same 40-token bar measured 12.50 mm on a 27"
// panel and 10.94 mm on a 32" one — 13 % smaller on the larger monitor. That is
// the defect this corrects, and it is arithmetic rather than taste.
//
// What it corrects *by* is the monitor's physical size, not its density. That
// is the author's own judgement on three real outputs, and density cannot
// express it: their 24" and 32" panels differ by 1.6 dpi — indistinguishable —
// and want visibly different sizes, because the 32" is read from further away.
// Size is the proxy for that distance; no monitor publishes the distance
// itself. A larger screen sits further back and needs the shell drawn larger to
// subtend the same angle.
//
// It never shrinks below the reference. A smaller monitor is not read from
// proportionally closer — a desk has a front edge — so scaling down with size
// would make a 24" panel's shell too small. The author confirmed exactly that:
// a 24" resolved to 0.88 by size alone and they asked for 1.00.
//
// A missing or absurd physical size is common enough that it cannot be trusted
// blindly, and Qt fabricates one when the compositor publishes none, so an
// output that reports nothing believable is left at 1.0 rather than scaled by
// a guess.
//
// The arithmetic is separate from the screen it is usually read from, because
// the arithmetic is the decision — the reference monitor, the bounds and what
// counts as an unbelievable reading — and a QScreen cannot be constructed to
// state it in a test.
double shellScaleForOutput(double diagonalMillimetres, double dotsPerInch);
double shellScaleForScreen(const QScreen *screen);

// The author's own answer, read from `CELESTINA_SHELL_SCALE`, or zero when
// nothing readable was asked for. Physical size is the best automatic proxy for
// how large something should look and not the whole of it — viewing distance is
// not published by any monitor — so a named number wins over a derived one.
double shellScaleOverride(const char *requested);
