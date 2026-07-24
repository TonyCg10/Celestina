// A freedesktop-thumbnail image provider for Siderita's views.
//
// Registered as "thumb", so a delegate can set `source: "image://thumb/<path>"`
// (the path percent-encoded) and get back a small thumbnail of an image file —
// reusing the shared `~/.cache/thumbnails/` cache other managers populate, and
// generating + caching the ones that are missing. The work is asynchronous (a
// thread pool), so scrolling never blocks on a decode.
//
// cxx-qt exposes no image-provider hook, so this is hand-written C++ (like the
// entrymodel), registered onto the engine before the QML loads.
#pragma once

class QQmlApplicationEngine;

// Adds the "thumb" provider to `engine`. Call once, before loading the QML.
void register_siderita_thumbnail_provider(QQmlApplicationEngine &engine);
