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
// A page asks for `image://calcitapage/<page>/<revision>/<key>`: the 0-based
// page, the revision of what the window shows (see src/pages.rs), then the
// document's path key (ADR 0008, the percent-encoded path bytes) handed over
// verbatim. `sourceSize` is honoured; a size of 0 on one side keeps the
// page's proportions. A copy registered for the key and revision (the
// document an edit is drawn over) is rendered instead of the file.
#pragma once

#include <cstdint>

#include "rust/cxx.h"

class QQmlApplicationEngine;
class QString;

// Adds the provider to `engine` under "calcitapage". Call once, before
// loading the QML.
void calcita_register_page_provider(QQmlApplicationEngine& engine);

// Registers `bytes`, a whole PDF, as what the pages of `key` at `revision`
// render from. Callable from any thread; the bytes are copied.
void calcita_page_provider_show_copy(const QString& key, ::std::int32_t revision,
                                     ::rust::Slice<const ::std::uint8_t> bytes);

// Forgets the copy of `key` at `revision`. Callable from any thread.
void calcita_page_provider_drop_copy(const QString& key, ::std::int32_t revision);

// Drops the renders not started, closes the documents the renderer holds on
// its own thread and waits at most `milliseconds` for it. Call once, after
// the event loop has returned.
void calcita_page_provider_shutdown(::std::int32_t milliseconds);
