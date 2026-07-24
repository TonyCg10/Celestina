#pragma once

#include <cxx-qt-lib/qstring.h>
#include <cxx-qt-lib/qstringlist.h>

// Bridge between Siderita's internal clipboard and the desktop's system
// clipboard, using the freedesktop `text/uri-list` plus the widely-honoured
// `x-special/gnome-copied-files` convention (which also carries copy vs cut),
// so copy / cut / paste interoperate with other file managers.

// Publishes `paths` (absolute local paths) to the system clipboard as both
// `text/uri-list` and `x-special/gnome-copied-files`. `cut` marks a move.
void siderita_set_clipboard_uris(const QStringList& paths, bool cut);

// The local file paths currently on the system clipboard (empty if there are
// none, or the clipboard holds non-file data).
QStringList siderita_read_clipboard_uris();

// Whether the system clipboard's file list is marked as a cut (move).
bool siderita_clipboard_is_cut();

// Whether the system clipboard currently holds any file URIs.
bool siderita_clipboard_has_uris();

// Clears the system clipboard (used after a cut is consumed).
void siderita_clear_clipboard();
