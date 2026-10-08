//! One row per audio endpoint of one kind: the page creates one model for the
//! outputs (`kind: "sink"`, the default) and one for the inputs
//! (`kind: "source"`). Roles: `id, kind, name, description, volume, percent,
//! muted, isDefault`.

use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

use cuprita_core::model::{AudioEndpoint, AudioSnapshot, EndpointKind};
use cuprita_core::volume::percent;

use super::audio::AUDIO;
use crate::models::{count, reconcile, role_names, row_of, Keyed, RowSink, FIRST_ROLE};

const ROLES: &[&str] = &[
    "id",
    "kind",
    "name",
    "description",
    "volume",
    "percent",
    "muted",
    "isDefault",
];

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!(<QtCore/QAbstractListModel>);
        type QAbstractListModel;

        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(i32, count, READ, NOTIFY)]
        #[qproperty(QString, kind)]
        // The default endpoint of this kind, so the card needs no row walk:
        // its id (-1 when none), description, volume percentage and mute.
        #[qproperty(i32, default_id, cxx_name = "defaultId", READ, NOTIFY)]
        #[qproperty(
            QString,
            default_description,
            cxx_name = "defaultDescription",
            READ,
            NOTIFY
        )]
        #[qproperty(i32, default_percent, cxx_name = "defaultPercent", READ, NOTIFY)]
        #[qproperty(bool, default_muted, cxx_name = "defaultMuted", READ, NOTIFY)]
        type EndpointModel = super::EndpointModelRust;

        #[cxx_override]
        fn data(self: &EndpointModel, index: &QModelIndex, role: i32) -> QVariant;
        #[cxx_override]
        fn row_count(self: &EndpointModel, parent: &QModelIndex) -> i32;
        #[cxx_override]
        fn role_names(self: &EndpointModel) -> QHash_i32_QByteArray;
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows(
            self: Pin<&mut EndpointModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut EndpointModel>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        unsafe fn begin_remove_rows(
            self: Pin<&mut EndpointModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        unsafe fn end_remove_rows(self: Pin<&mut EndpointModel>);
        #[inherit]
        #[cxx_name = "beginMoveRows"]
        unsafe fn begin_move_rows(
            self: Pin<&mut EndpointModel>,
            source_parent: &QModelIndex,
            source_first: i32,
            source_last: i32,
            destination_parent: &QModelIndex,
            destination_child: i32,
        ) -> bool;
        #[inherit]
        #[cxx_name = "endMoveRows"]
        unsafe fn end_move_rows(self: Pin<&mut EndpointModel>);
        #[inherit]
        fn index(self: &EndpointModel, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
        #[inherit]
        #[qsignal]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut EndpointModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QList_i32,
        );
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qlist.h");
        type QList_i32 = cxx_qt_lib::QList<i32>;
    }

    impl cxx_qt::Threading for EndpointModel {}
    impl cxx_qt::Initialize for EndpointModel {}
}

impl Keyed for AudioEndpoint {
    fn key(&self) -> String {
        self.id.to_string()
    }
}

fn kind_token(kind: EndpointKind) -> &'static str {
    match kind {
        EndpointKind::Sink => "sink",
        EndpointKind::Source => "source",
    }
}

pub struct EndpointModelRust {
    count: i32,
    rows: Vec<AudioEndpoint>,
    /// Which endpoints this model lists. Set once, when QML creates it: the
    /// first snapshot is queued, so it arrives after the property is set.
    kind: QString,
    default_id: i32,
    default_description: QString,
    default_percent: i32,
    default_muted: bool,
}

impl Default for EndpointModelRust {
    fn default() -> Self {
        Self {
            count: 0,
            rows: Vec::new(),
            kind: QString::from("sink"),
            default_id: -1,
            default_description: QString::default(),
            default_percent: 0,
            default_muted: false,
        }
    }
}

impl cxx_qt::Initialize for qobject::EndpointModel {
    fn initialize(self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        AUDIO.subscribe(move |snapshot: Arc<AudioSnapshot>| {
            qt.queue(move |model: Pin<&mut qobject::EndpointModel>| {
                model.apply(&snapshot);
            })
            .is_ok()
        });
    }
}

struct Sink<'a>(Pin<&'a mut qobject::EndpointModel>);

impl RowSink<AudioEndpoint> for Sink<'_> {
    fn rows(&mut self) -> &mut Vec<AudioEndpoint> {
        &mut self.0.as_mut().rust_mut().get_mut().rows
    }
    fn begin_insert(&mut self, at: usize) {
        let at = count(at);
        // SAFETY: paired with `end_insert` by `reconcile`, on the Qt thread.
        unsafe {
            self.0
                .as_mut()
                .begin_insert_rows(&QModelIndex::default(), at, at);
        }
    }
    fn end_insert(&mut self) {
        // SAFETY: closes the `begin_insert` above.
        unsafe { self.0.as_mut().end_insert_rows() }
    }
    fn begin_remove(&mut self, at: usize) {
        let at = count(at);
        // SAFETY: paired with `end_remove` by `reconcile`, on the Qt thread.
        unsafe {
            self.0
                .as_mut()
                .begin_remove_rows(&QModelIndex::default(), at, at);
        }
    }
    fn end_remove(&mut self) {
        // SAFETY: closes the `begin_remove` above.
        unsafe { self.0.as_mut().end_remove_rows() }
    }
    fn begin_move(&mut self, from: usize, to: usize) {
        let (from, to) = (count(from), count(to));
        let root = QModelIndex::default();
        // SAFETY: paired with `end_move` by `reconcile`, on the Qt thread. An
        // upward move to `to` is always a valid destination, so Qt accepts it.
        unsafe {
            self.0
                .as_mut()
                .begin_move_rows(&root, from, from, &root, to);
        }
    }
    fn end_move(&mut self) {
        // SAFETY: closes the `begin_move` above.
        unsafe { self.0.as_mut().end_move_rows() }
    }
    fn changed(&mut self, at: usize) {
        let index = self.0.index(count(at), 0, &QModelIndex::default());
        self.0
            .as_mut()
            .data_changed(&index, &index, &cxx_qt_lib::QList::default());
    }
}

impl qobject::EndpointModel {
    fn apply(mut self: Pin<&mut Self>, snapshot: &AudioSnapshot) {
        let rows: Vec<AudioEndpoint> = snapshot
            .endpoints
            .iter()
            .filter(|e| kind_token(e.kind) == self.rust().kind.to_string())
            .cloned()
            .collect();
        let chosen = rows.iter().find(|e| e.default).cloned();
        reconcile(&mut Sink(self.as_mut()), rows);
        self.as_mut().show_default(chosen.as_ref());
        let rows = count(self.rust().rows.len());
        if self.rust().count != rows {
            self.as_mut().rust_mut().count = rows;
            self.count_changed();
        }
    }

    fn show_default(mut self: Pin<&mut Self>, chosen: Option<&AudioEndpoint>) {
        let id = chosen.map_or(-1, |e| i32::try_from(e.id).unwrap_or(-1));
        if self.rust().default_id != id {
            self.as_mut().rust_mut().default_id = id;
            self.as_mut().default_id_changed();
        }
        let description = QString::from(chosen.map_or("", |e| e.description.as_str()));
        if self.rust().default_description != description {
            self.as_mut().rust_mut().default_description = description;
            self.as_mut().default_description_changed();
        }
        let shown = chosen.map_or(0, |e| i32::from(percent(e.volume)));
        if self.rust().default_percent != shown {
            self.as_mut().rust_mut().default_percent = shown;
            self.as_mut().default_percent_changed();
        }
        let muted = chosen.is_some_and(|e| e.muted);
        if self.rust().default_muted != muted {
            self.as_mut().rust_mut().default_muted = muted;
            self.as_mut().default_muted_changed();
        }
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(r) = row_of(&self.rust().rows, index) else {
            return QVariant::default();
        };
        match role - FIRST_ROLE {
            0 => QVariant::from(&r.id),
            1 => QVariant::from(&QString::from(kind_token(r.kind))),
            2 => QVariant::from(&QString::from(r.name.as_str())),
            3 => QVariant::from(&QString::from(r.description.as_str())),
            4 => QVariant::from(&f64::from(r.volume)),
            5 => QVariant::from(&i32::from(percent(r.volume))),
            6 => QVariant::from(&r.muted),
            7 => QVariant::from(&r.default),
            _ => QVariant::default(),
        }
    }

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        count(self.rust().rows.len())
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        role_names(ROLES)
    }
}
