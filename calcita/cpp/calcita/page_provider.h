// Calcita's page image provider: a document's page as QtPdf renders it with
// its annotations.
//
// QML's `PdfPageImage` renders a page without QtPdf's
// `RenderFlag::Annotations`, and exposes no way to ask for it, so every
// highlight, note and drawing another program put on a page stayed hidden.
// cxx-qt exposes no image-provider hook (`QQuickAsyncImageProvider` must be
// subclassed), so this is hand-written C++; it only renders, and every
// decision about the document stays in Rust or QML.
//
// A page asks for `image://calcitapage/<page>/<key>`: the 0-based page, then
// the document's path key (ADR 0008, the percent-encoded path bytes) handed
// over verbatim. `sourceSize` is honoured; a size of 0 on one side keeps the
// page's proportions.
#pragma once

#include <cstdint>

class QQmlApplicationEngine;

// Adds the provider to `engine` under "calcitapage". Call once, before
// loading the QML.
void calcita_register_page_provider(QQmlApplicationEngine& engine);

// Drops the renders not started, closes the documents the renderer holds on
// its own thread and waits at most `milliseconds` for it. Call once, after
// the event loop has returned.
void calcita_page_provider_shutdown(::std::int32_t milliseconds);
