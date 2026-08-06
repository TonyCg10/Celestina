// A freedesktop-thumbnail image provider for Siderita's views.
//
// Registered as "thumb", so a delegate can set `source: "image://thumb/<key>"`
// (the entry's path key, ADR 0008) and get back a small thumbnail of an image
// file — reusing the shared `~/.cache/thumbnails/` cache other managers
// populate, and generating + caching the ones that are missing. The work is
// asynchronous (a thread pool), so scrolling never blocks on a decode.
//
// cxx-qt exposes no image-provider hook, so this is hand-written C++ (like the
// entrymodel), registered onto the engine before the QML loads.
#pragma once

#include <QtCore/QByteArray>
#include <QtCore/QSize>

class QQmlApplicationEngine;

// Adds the "thumb" provider to `engine`. Call once, before loading the QML.
void register_siderita_thumbnail_provider(QQmlApplicationEngine &engine);

// The freedesktop cache key for the file named by `pathBytes`: the canonical
// `file://` URI, spelled exactly as `QUrl::fromLocalFile().toEncoded()` spells
// it, but computed over the raw bytes so a name that is not valid UTF-8 also
// gets a key instead of none.
//
// The preserved set is the one `celestina_core::percent::encode_qt_path`
// documents, and the two spellings must stay identical byte for byte or this
// process stops sharing the desktop's thumbnail cache. That equality is pinned
// by a test in `src/thumbnails.rs`, which is why this is declared here rather
// than kept private to the translation unit.
QByteArray siderita_thumbnail_cache_uri(const QByteArray &pathBytes);

// The pixel size of the image at `pathBytes`, read through the same guards and
// the same byte-exact descriptor the provider decodes through: absolute path,
// regular file, generatable extension, `open` on the raw bytes. Invalid when
// any of those refuses.
//
// This is the seam's testable boundary. The provider's own entry point cannot
// be one: it answers with a QImage, which does not cross the CXX-Qt seam, and
// it writes into the session's shared thumbnail cache, which a test must not
// touch. What this proves instead is the part that used to be broken — that a
// name a QString cannot hold is still found and still decoded.
QSize siderita_thumbnail_source_size(const QByteArray &pathBytes);

// The raw path bytes the provider resolves for a published `key`, reached the
// way Qt reaches them: through a `image://thumb/<key>` URL, whose id Qt derives
// with PrettyDecoded formatting before the provider is ever called.
//
// This exists because the decode alone is not the seam. The regression this
// pins was not a wrong decoder but a wrong assumption about what the id already
// is on arrival: Qt has decoded every escape that spells valid UTF-8, so a name
// with an accent reaches the provider as characters, not as escapes. A test
// that hands the provider a key directly cannot see that, and did not.
QByteArray siderita_thumbnail_resolved_path(const QByteArray &key);
