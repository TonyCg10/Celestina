#include "traymenu.h"

namespace {
// A menu is a handful of choices. These bounds are what keep a tree from
// another process from being walked as far as it claims to go.
constexpr int maxDepth = 3;
constexpr qsizetype maxEntriesPerLevel = 64;
constexpr qsizetype maxLabelLength = 128;

bool isTrue(const QVariantMap &properties, const QString &key, bool fallback)
{
    const QVariant value = properties.value(key);
    return value.isValid() ? value.toBool() : fallback;
}

void appendEntries(const TrayMenuNode &node, int depth, QVariantList *entries)
{
    if (depth > maxDepth || entries->size() >= maxEntriesPerLevel)
        return;

    qsizetype taken = 0;
    for (const TrayMenuNode &child : node.children) {
        if (taken >= maxEntriesPerLevel || entries->size() >= maxEntriesPerLevel)
            return;
        // `visible: false` is the application saying not to show this at all.
        if (!isTrue(child.properties, QStringLiteral("visible"), true))
            continue;

        ++taken;
        const QString type =
            child.properties.value(QStringLiteral("type")).toString();
        const bool separator = type == QStringLiteral("separator");
        const QString label =
            trayMenuLabel(child.properties.value(QStringLiteral("label")).toString());
        // A submenu is announced by how the application says to display its
        // children, not by whether any arrived: an application may fill it only
        // when asked.
        const bool submenu =
            child.properties.value(QStringLiteral("children-display")).toString()
                == QStringLiteral("submenu");

        entries->append(QVariantMap {
            {QStringLiteral("id"), child.id},
            {QStringLiteral("label"), label},
            {QStringLiteral("separator"), separator},
            // Disabled entries stay: this session's menus use them as headings,
            // and dropping them would hide what the rest of the menu is about.
            {QStringLiteral("enabled"),
             !separator && isTrue(child.properties, QStringLiteral("enabled"), true)},
            {QStringLiteral("iconName"),
             child.properties.value(QStringLiteral("icon-name")).toString()},
            {QStringLiteral("toggleType"),
             child.properties.value(QStringLiteral("toggle-type")).toString()},
            // -1 is the specification's "indeterminate", which is not "off".
            {QStringLiteral("toggleState"),
             child.properties.contains(QStringLiteral("toggle-state"))
                 ? child.properties.value(QStringLiteral("toggle-state")).toInt()
                 : -1},
            {QStringLiteral("submenu"), submenu},
            {QStringLiteral("depth"), depth},
        });

        if (submenu)
            appendEntries(child, depth + 1, entries);
    }
}
} // namespace

QString trayMenuLabel(const QString &raw)
{
    QString label;
    label.reserve(raw.size());
    for (qsizetype at = 0; at < raw.size(); ++at) {
        if (raw.at(at) != u'_') {
            label.append(raw.at(at));
            continue;
        }
        // A doubled underscore is the literal one the application meant.
        if (at + 1 < raw.size() && raw.at(at + 1) == u'_') {
            label.append(u'_');
            ++at;
        }
    }
    return label.left(maxLabelLength);
}

QVariantList buildTrayMenu(const TrayMenuNode &root)
{
    QVariantList entries;
    appendEntries(root, 0, &entries);
    return entries;
}
