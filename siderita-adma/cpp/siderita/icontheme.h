#pragma once

#include <QtCore/QString>

// Pin the freedesktop icon theme that QIcon::fromTheme — and therefore every
// QML `IconImage { name: … }` — resolves named icons against. Must be called
// once, after the QGuiApplication exists and before any QML (and thus any icon)
// is loaded. cxx-qt cannot reach QIcon from Rust, so this is a small C++ shim,
// like the clipboard and thumbnail-provider shims.
void siderita_apply_icon_theme(const QString &theme);
