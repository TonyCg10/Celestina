#include "siderita/entrymodel.h"

#include <QtQml/qqml.h>

#include <algorithm>

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
    case SectionRole:
        return entry.section;
    case SizeTextRole:
        return entry.sizeText;
    case DateTextRole:
        return entry.dateText;
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
        {SectionRole, QByteArrayLiteral("section")},
        {SizeTextRole, QByteArrayLiteral("sizeText")},
        {DateTextRole, QByteArrayLiteral("dateText")},
    };
}

namespace {

// Everything a row carries, built once from the parallel columns.
SideritaEntryModel::Row buildRow(const QStringList &names, const QStringList &tokens,
                                 const QStringList &kinds, const QStringList &subtitles,
                                 const QStringList &paths, const QStringList &sections,
                                 const QStringList &sizes, const QStringList &dates, int i)
{
    SideritaEntryModel::Row entry;
    entry.name = names.value(i);
    entry.token = tokens.value(i);
    entry.kind = kinds.value(i);
    entry.subtitle = subtitles.value(i);
    entry.path = paths.value(i);
    entry.section = sections.value(i);
    entry.sizeText = sizes.value(i);
    entry.dateText = dates.value(i);
    entry.isDir = entry.kind == QStringLiteral("directory");
    return entry;
}

bool sameRow(const SideritaEntryModel::Row &left, const SideritaEntryModel::Row &right)
{
    return left.name == right.name && left.token == right.token && left.kind == right.kind
        && left.subtitle == right.subtitle && left.path == right.path
        && left.section == right.section && left.sizeText == right.sizeText
        && left.dateText == right.dateText;
}

} // namespace

// Replaces the list with `next`, telling the view what actually changed.
//
// A reset is the honest answer only when the list is unrecognisable. It is also
// the most expensive one: it drops every delegate, so the view rebuilds all of
// them, re-evaluates their bindings and loses its scroll position and its
// selection. A folder being watched while a download runs in it went through
// that on every tick — measured at 98 ms of CPU for 2 000 entries, and 210 ms
// for 50 000, whether or not anything visible had changed.
//
// So the common shapes of change are recognised and reported as themselves:
//
// - the same rows with some cells edited (a file's size or date moved) becomes
//   `dataChanged` over the runs that differ;
// - a contiguous block appearing (a download finishing, a folder created)
//   becomes an insertion;
// - a contiguous block disappearing becomes a removal.
//
// Anything else — a re-sort, a filter, a different folder — still resets, which
// is correct and rare.
void SideritaEntryModel::setRows(const QStringList &names,
                                 const QStringList &tokens,
                                 const QStringList &kinds,
                                 const QStringList &subtitles,
                                 const QStringList &paths,
                                 const QStringList &sections,
                                 const QStringList &sizes,
                                 const QStringList &dates)
{
    const int count = names.size();
    QVector<Row> next;
    next.reserve(count);
    for (int i = 0; i < count; ++i) {
        next.push_back(buildRow(names, tokens, kinds, subtitles, paths, sections, sizes, dates, i));
    }

    if (next.size() == m_rows.size()) {
        // Same length: if every row still holds the same entry, the change is
        // cell-level and the view keeps everything it had.
        bool sameEntries = true;
        for (int i = 0; i < next.size(); ++i) {
            if (next.at(i).token != m_rows.at(i).token) {
                sameEntries = false;
                break;
            }
        }
        if (sameEntries) {
            int runStart = -1;
            for (int i = 0; i < next.size(); ++i) {
                const bool differs = !sameRow(next.at(i), m_rows.at(i));
                if (differs) {
                    m_rows[i] = next.at(i);
                    if (runStart < 0) {
                        runStart = i;
                    }
                } else if (runStart >= 0) {
                    emit dataChanged(index(runStart), index(i - 1));
                    runStart = -1;
                }
            }
            if (runStart >= 0) {
                emit dataChanged(index(runStart), index(next.size() - 1));
            }
            return;
        }
    }

    // How much of the head and the tail survived. What sits between them is the
    // whole of the change.
    int head = 0;
    const int shortest = std::min(next.size(), m_rows.size());
    while (head < shortest && next.at(head).token == m_rows.at(head).token
           && sameRow(next.at(head), m_rows.at(head))) {
        ++head;
    }
    int tail = 0;
    while (tail < shortest - head
           && next.at(next.size() - 1 - tail).token == m_rows.at(m_rows.size() - 1 - tail).token
           && sameRow(next.at(next.size() - 1 - tail), m_rows.at(m_rows.size() - 1 - tail))) {
        ++tail;
    }

    const int added = next.size() - head - tail;
    const int removed = m_rows.size() - head - tail;
    if (added > 0 && removed == 0) {
        beginInsertRows(QModelIndex(), head, head + added - 1);
        for (int i = 0; i < added; ++i) {
            m_rows.insert(head + i, next.at(head + i));
        }
        endInsertRows();
        return;
    }
    if (removed > 0 && added == 0) {
        beginRemoveRows(QModelIndex(), head, head + removed - 1);
        m_rows.remove(head, removed);
        endRemoveRows();
        return;
    }

    beginResetModel();
    m_rows = next;
    endResetModel();
}

void register_siderita_entry_model()
{
    // A separate module namespace: cxx-qt registers org.celestina.siderita
    // declaratively, and Qt forbids also registering a type into it
    // imperatively ("namespace already used for type registration").
    qmlRegisterType<SideritaEntryModel>("org.celestina.siderita.internal", 1, 0,
                                        "SideritaEntryModel");
}
