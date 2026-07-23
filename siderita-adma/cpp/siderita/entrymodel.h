// A native role-based list model for Siderita's file view, replacing the
// QStringList-of-names + per-delegate invokables + viewRevision workaround.
//
// cxx-qt 0.9 cannot override QAbstractListModel's virtuals from Rust, so the
// model is hand-written Qt/C++: the controller pushes the projected rows in as
// parallel string lists (setRows), and the list/grid delegates read them as
// roles (name/token/kind/subtitle/path/isDirectory) with proper model-reset
// signals — so the view updates without re-querying every cell on every change.
#pragma once

#include <QtCore/QAbstractListModel>
#include <QtCore/QHash>
#include <QtCore/QStringList>
#include <QtCore/QVector>

class SideritaEntryModel : public QAbstractListModel
{
    Q_OBJECT

public:
    enum Roles {
        NameRole = Qt::UserRole + 1,
        TokenRole,
        KindRole,
        SubtitleRole,
        PathRole,
        IsDirRole,
    };

    explicit SideritaEntryModel(QObject *parent = nullptr);

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    // Replaces the whole list from the controller's projected view.
    Q_INVOKABLE void setRows(const QStringList &names,
                             const QStringList &tokens,
                             const QStringList &kinds,
                             const QStringList &subtitles,
                             const QStringList &paths);

private:
    struct Row {
        QString name;
        QString token;
        QString kind;
        QString subtitle;
        QString path;
        bool isDir;
    };
    QVector<Row> m_rows;
};

// Registers the type under the org.celestina.siderita QML module; call once
// before loading the QML.
void register_siderita_entry_model();
