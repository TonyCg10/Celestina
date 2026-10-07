// The freedesktop-thumbnail image provider, shared by the hosts that show
// media in a grid.
//
// A host registers it under "thumb", so a delegate can set
// `source: "image://thumb/<key>"` — the item's path key (ADR 0008),
// percent-encoded path bytes — and get back a thumbnail of an image file. It
// reuses the shared `$XDG_CACHE_HOME/thumbnails/large` cache every desktop
// file manager populates, and generates and caches the entries that are
// missing or older than their source. The work runs on a small pool of its
// own, so scrolling never blocks on a decode and a request the view no longer
// needs is taken back out of the queue.
//
// Images only, and only through Qt's image reader: the media backend is never
// started here. Video posters and embedded covers are produced by the engine's
// artwork pass and found in the same cache.
//
// cxx-qt exposes no image-provider hook, so this is hand-written C++. It
// lives in this crate because two hosts need the same one: Fluorita registers
// it as it is; Siderita can pass a hook (`ThumbnailOwnPicture`) for the
// pictures a file carries or names inside itself, which only it knows how to
// read.
#pragma once

#include <QtCore/QByteArray>
#include <QtCore/QSize>
#include <QtGui/QImage>

#include <cstdint>
#include <functional>

class QQmlApplicationEngine;

namespace fluorita {

// The longest edge of a generated thumbnail: the spec's "large" size.
inline constexpr int kThumbnailEdge = 256;

// What a host hook answers for one file: a picture, and whether it may be
// written into the shared cache. A picture that belongs to something other
// than the file — an icon theme's icon a launcher names, say — must not be,
// because the cache entry would outlive a change of theme.
struct OwnPicture
{
    QImage image;
    bool cacheable = false;
};

// The seam a host fills to answer before the image reader. Called on a pool
// thread with the raw path bytes, after the cache missed. An empty image means
// "nothing of mine", and the image reader is tried next.
//
// Held by value in every request, never by reference to the provider, so a
// decode still running after the engine went away has nothing to dangle.
using ThumbnailOwnPicture = std::function<OwnPicture(const QByteArray &pathBytes)>;

} // namespace fluorita

// Adds the provider to `engine` under "thumb", with no host hook. Call once,
// before loading the QML.
void register_fluorita_thumbnail_provider(QQmlApplicationEngine &engine);

// The same, with a hook consulted after the cache and before the image reader.
void register_fluorita_thumbnail_provider_with(QQmlApplicationEngine &engine,
                                               fluorita::ThumbnailOwnPicture ownPicture);

// Drains the provider's pool on quit: drops the requests not started and waits
// at most `milliseconds` for the running ones. Call once, after the event loop
// has returned.
void fluorita_thumbnail_shutdown(::std::int32_t milliseconds);

// The freedesktop cache key for the file named by `pathBytes`: the canonical
// `file://` URI, spelled exactly as `QUrl::fromLocalFile().toEncoded()` spells
// it, but computed over the raw bytes so a name that is not valid UTF-8 also
// gets a key. The preserved set is `celestina_core::percent::encode_qt_path`'s,
// and the two must agree byte for byte or the process stops sharing the
// desktop's cache.
QByteArray fluorita_thumbnail_cache_uri(const QByteArray &pathBytes);

// The raw path bytes the provider resolves for a published `key`, reached the
// way Qt reaches them: through an `image://thumb/<key>` URL, whose id Qt
// derives with PrettyDecoded formatting before the provider is ever called.
QByteArray fluorita_thumbnail_resolved_path(const QByteArray &key);

// The size of the thumbnail the provider would generate for the file at
// `pathBytes` — decoded through the same guards, budgets and reader — without
// reading or writing the shared cache. Invalid when any guard refuses.
//
// The testable boundary: the provider's entry point answers with a QImage,
// which does not cross the CXX-Qt seam, and writes into the session's shared
// cache, which a test must not touch.
QSize fluorita_thumbnail_generated_size(const QByteArray &pathBytes);
