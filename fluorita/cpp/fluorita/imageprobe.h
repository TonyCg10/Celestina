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
//
// `key` is the path key of ADR 0008 — percent-encoded path bytes, and therefore
// ASCII, which is the only spelling a `QString` carries without loss. It is
// decoded to raw bytes here and the file is opened by descriptor, because every
// Qt file API takes a `QString` and a `QString` cannot name a file whose name is
// not valid UTF-8. Passing the path itself would measure the header of whatever
// file the lossy spelling happened to hit.
QSize fluorita_probe_image(const QString &key);
