#pragma once

#include <cxx-qt-lib/qstring.h>

// Puts `text` on the system clipboard as plain text. Called on the Qt thread
// (QClipboard belongs to it); no file or bus IO of Calcita's own.
void calcita_set_clipboard_text(const QString& text);
