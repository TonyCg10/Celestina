#pragma once

#include <QVariantList>

// Every output of this session, flattened to what a surface can actually read.
//
// It exists because `QScreen` publishes no standalone `width` and `height` to
// QML: it publishes a `geometry` rectangle, and a binding that asks a screen
// for its width gets `undefined` — which arrives in a layout as `NaN` and
// draws three monitors on top of each other rather than failing. Flattening it
// here means the boundary is stated once, in the language the chooser reads,
// instead of being rediscovered by every surface that asks the session what
// screens it has.
//
// One fact, one owner, for the same reason `shellScaleForScreen` is a free
// function beside its own header: the second copy is where the two answers
// start to differ.
QVariantList outputScreenSnapshot();
