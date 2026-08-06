#pragma once

#include <cxx-qt-lib/qstring.h>
#include <cxx-qt-lib/qstringlist.h>

// Bridge between Siderita's internal clipboard and the desktop's system
// clipboard, using the freedesktop `text/uri-list` plus the widely-honoured
// `x-special/gnome-copied-files` convention (which also carries copy vs cut),
// so copy / cut / paste interoperate with other file managers.
//
// What crosses this seam is a **percent-encoded `file://` URI**, never a path.
// That is what the desktop exchanges, and it is why it can be a QString at all:
// a URI is ASCII, while a Linux filename is a byte string a QString cannot
// hold. Rust owns both halves of the codec (`src/dbus.rs`), so a name that is
// not valid UTF-8 survives the trip to another application and back.

// Publishes `uris` (absolute `file://` URIs, already percent-encoded) to the
// system clipboard as both `text/uri-list` and `x-special/gnome-copied-files`.
// `cut` marks a move.
void siderita_set_clipboard_uris(const QStringList& uris, bool cut);

// The local-file URIs currently on the system clipboard, percent-encoded as
// they were exchanged (empty if there are none, or the clipboard holds non-file
// data).
QStringList siderita_read_clipboard_uris();

// Whether the system clipboard's file list is marked as a cut (move).
bool siderita_clipboard_is_cut();

// Whether the system clipboard currently holds any file URIs.
bool siderita_clipboard_has_uris();

// Clears the system clipboard (used after a cut is consumed).
void siderita_clear_clipboard();
