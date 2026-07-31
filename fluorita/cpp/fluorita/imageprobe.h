// Reading an image's shape without decoding it.
//
// Hand-written C++ because cxx-qt-lib exposes no `QImageReader`, and because
// this must not become a second decoder: Qt already reads every format the
// suite claims to support, and Siderita's thumbnailer already relies on the
// same reader. Fluorita only needs the size *before* deciding whether the file
// is safe to decode at all, which the reader answers from the header alone.
#pragma once

#include <cxx-qt-lib/qsize.h>
#include <cxx-qt-lib/qstring.h>

// The image's pixel dimensions, or an empty size when the file is not a
// readable image. Never decodes the pixels.
QSize fluorita_probe_image(const QString &path);
