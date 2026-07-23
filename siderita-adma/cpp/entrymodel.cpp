#include "siderita/entrymodel.h"

#include <QtQml/qqml.h>

SideritaEntryModel::SideritaEntryModel(QObject *parent)
    : QAbstractListModel(parent)
{
}

int SideritaEntryModel::rowCount(const QModelIndex &parent) const
{
    // A flat list has no rows under a valid parent.
    return parent.isValid() ? 0 : m_rows.size();
}

QVariant SideritaEntryModel::data(const QModelIndex &index, int role) const
{
    const int row = index.row();
    if (!index.isValid() || row < 0 || row >= m_rows.size()) {
        return QVariant();
    }
    const Row &entry = m_rows.at(row);
    switch (role) {
    case NameRole:
        return entry.name;
    case TokenRole:
        return entry.token;
    case KindRole:
        return entry.kind;
    case SubtitleRole:
        return entry.subtitle;
    case PathRole:
        return entry.path;
    case IsDirRole:
        return entry.isDir;
    default:
        return QVariant();
    }
}

QHash<int, QByteArray> SideritaEntryModel::roleNames() const
{
    return {
        {NameRole, QByteArrayLiteral("name")},
        {TokenRole, QByteArrayLiteral("token")},
        {KindRole, QByteArrayLiteral("kind")},
        {SubtitleRole, QByteArrayLiteral("subtitle")},
        {PathRole, QByteArrayLiteral("path")},
        {IsDirRole, QByteArrayLiteral("isDirectory")},
    };
}

void SideritaEntryModel::setRows(const QStringList &names,
                                 const QStringList &tokens,
                                 const QStringList &kinds,
                                 const QStringList &subtitles,
                                 const QStringList &paths)
{
    beginResetModel();
    m_rows.clear();
    const int count = names.size();
    m_rows.reserve(count);
    for (int i = 0; i < count; ++i) {
        Row entry;
        entry.name = names.value(i);
        entry.token = tokens.value(i);
        entry.kind = kinds.value(i);
        entry.subtitle = subtitles.value(i);
        entry.path = paths.value(i);
        entry.isDir = entry.kind == QStringLiteral("directory");
        m_rows.push_back(entry);
    }
    endResetModel();
}

void register_siderita_entry_model()
{
    qmlRegisterType<SideritaEntryModel>("org.celestina.siderita", 1, 0,
                                        "SideritaEntryModel");
}
