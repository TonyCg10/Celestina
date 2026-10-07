// Siderita's part of the freedesktop-thumbnail image provider.
//
// The provider itself is the suite's shared one (`fluorita-qt`,
// "fluorita/thumbnailprovider.h"): the "thumb" id, the shared
// `~/.cache/thumbnails/large` cache, the bounded pool, cancellation, freshness
// and the image reader all live there. A delegate still sets
// `source: "image://thumb/<key>"` (the entry's path key, ADR 0008).
//
// What only Siderita knows is the picture a file carries or names inside
// itself — a program's icon, an album cover, a launcher's themed icon — and
// that is all this registers on top, through the shared provider's
// `ThumbnailOwnPicture` hook.
#pragma once

class QQmlApplicationEngine;

// Adds the shared provider to `engine` as "thumb", with Siderita's own-picture
// hook. Call once, before loading the QML.
void register_siderita_thumbnail_provider(QQmlApplicationEngine &engine);
