#pragma once

#include <QList>
#include <QString>
#include <QVariantMap>

// A tray is a handful of applications. Anything past this is a misbehaving
// watcher, not a session with that many controls.
//
// It is published here because the published list is not the only thing a
// foreign process can grow: a host that accepts registrations, signal
// subscriptions or cached icons beyond what it could ever show is growing on
// another application's word. Everything keyed by an item obeys this same
// number, so there is one bound to reason about rather than several.
constexpr qsizetype maxTrayItems = 64;
// The longest registration string, object path or theme path accepted from
// another process. A bus name with a path appended is short; anything longer
// is a client spending the panel's memory rather than naming an item.
constexpr qsizetype maxTrayPathLength = 512;

// What a StatusNotifierItem is, once the panel has finished distrusting it.
//
// Tray items are other applications' controls, published by whatever toolkit
// they happen to use, and the ones on this session already disagree with each
// other: one omits `ItemIsMenu` entirely, another's `IconName` fails to read at
// all and offers only raw pixels, a third has an empty title. So nothing here
// requires a field to exist — it decides what the panel can show given what
// arrived, and says plainly when that is nothing.
//
// Pure on purpose: the D-Bus conversation lives in `TrayWatcher`, and every
// rule about what an item *means* is testable against a plain map.
struct TrayItem {
    // The bus name and object path that answer for this item, together its
    // identity: an application may publish more than one.
    QString service;
    QString path;

    QString id;
    // A restart-stable fingerprint of the application's real `Id`. Empty when
    // the peer never supplied an Id: the synthesized display fallback is not a
    // durable identity and must not become a preference. Live activation
    // continues to use service/path, never this fingerprint.
    QString preferenceKey;
    // User-facing application name: declared `Title`, or a bounded name
    // derived from the technical Id/tooltip contract — never empty for a
    // peer that supplied any usable identity.
    QString title;
    // "active", "passive" or "attention". A drawer shows all three; where each
    // belongs is the panel's business, not this file's.
    QString status;
    // A themed icon name from the application's own icon theme, empty when it
    // named none.
    QString iconName;
    // An icon theme directory the application ships with itself. Absolute or
    // ignored: a relative path from another process names nothing this panel
    // can resolve.
    QString iconThemePath;
    // Whether the item published raw pixels. The panel needs this because an
    // item may have those and no name at all.
    bool hasPixmap = false;
    // The DBusMenu object path, empty when the item published none.
    QString menuPath;

    bool operator==(const TrayItem &other) const;
};

// Produces the opaque preference identity exported to QML. The SNI `Id` is the
// protocol field required to remain consistent between sessions; live D-Bus
// service names and object paths are deliberately absent. Empty input means
// there is no stable preference identity to promise.
QString trayPreferenceKey(const QString &id);

// Chooses the user-facing application name without changing the SNI Id used
// for durable preference identity. A declared Title wins. Technical
// Chromium/Electron status-icon suffixes are removed; only a generic runtime
// Id lets the tooltip supply product identity, because app-specific peers may
// use that tooltip for transient state instead.
QString trayDisplayName(
    const QString &id,
    const QString &declaredTitle,
    const QString &toolTipTitle
);

// Splits a watcher's registration string, which is a bus name with the object
// path appended — `:1.19/org/ayatana/NotificationItem/nm_applet`. An entry with
// no path names the specification's default one.
//
// Returns false for anything that is not a usable registration.
bool parseTrayRegistration(const QString &entry, QString *service, QString *path);

// What the panel can show for an item that registered and then never described
// itself.
//
// `GetAll` is a call to another application's process and it can fail: the
// object may not be exported yet, the interface may be the wrong one, the peer
// may refuse. Dropping the item there is what makes a registration the watcher
// lists into a control nobody can see, permanently and without a word — so an
// item that never answered is shown with the only name this shell has for it,
// which is the one it registered under.
TrayItem unreadTrayItem(const QString &service, const QString &path);

// Reads what `org.freedesktop.DBus.Properties.GetAll` answered for one item.
// Absent keys are normal — `GetAll` omits a property whose getter failed — so
// this fills in what it can and never fails for a missing field.
TrayItem readTrayItem(
    const QString &service,
    const QString &path,
    const QVariantMap &properties
);

// The items the panel is showing, in the order the watcher reported them.
//
// It is a value, not a live object: the watcher hands over a whole list and
// this says whether what QML reads has changed.
class TrayItems
{
public:
    // Returns whether the published list changed.
    bool replace(const QList<TrayItem> &items);
    bool clear();

    QList<TrayItem> items() const { return m_items; }
    // One map per item, ready for QML.
    QVariantList toVariantList() const;
    bool isEmpty() const { return m_items.isEmpty(); }

private:
    QList<TrayItem> m_items;
};
