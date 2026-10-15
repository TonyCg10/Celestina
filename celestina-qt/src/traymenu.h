#pragma once

#include <QList>
#include <QString>
#include <QVariantList>
#include <QVariantMap>

// What another application's menu means, once the panel has read it.
//
// `com.canonical.dbusmenu` describes a tree of nodes with open-ended
// properties, and the menus on this session use most of them: labels carrying
// GTK mnemonics, separators, entries deliberately disabled to act as headings,
// nested submenus, and icons named or shipped as bytes. None of it is
// guaranteed to be there.
//
// The D-Bus conversation is `TrayWatcher`'s; this is only the reading, so every
// rule about what an entry *is* can be tested against a plain tree.

/// One node exactly as the wire described it, before any rule is applied.
struct TrayMenuNode {
    int id = 0;
    QVariantMap properties;
    QList<TrayMenuNode> children;
};

/// Turns a menu tree into the flat, bounded list QML draws.
///
/// Depth and breadth are capped: a menu is a handful of choices, and a tree
/// from another process is not something to walk as far as it says. Entries an
/// application marked invisible are dropped; entries it disabled are kept,
/// because this session uses those as headings and dropping them would hide
/// what the rest of the menu is about.
QVariantList buildTrayMenu(const TrayMenuNode &root);

/// Strips GTK mnemonics: `_Desactivar` is shown as `Desactivar`, and a doubled
/// `__` is the literal underscore the application meant.
QString trayMenuLabel(const QString &raw);
