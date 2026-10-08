//! One row per application playing or recording.
//! Roles: `id, appName, appIcon, volume, percent, muted`.

use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

use cuprita_core::model::{AudioSnapshot, AudioStream};
use cuprita_core::volume::percent;

use super::audio::AUDIO;
use crate::models::{count, reconcile, role_names, row_of, Keyed, RowSink, FIRST_ROLE};

const ROLES: &[&str] = &["id", "appName", "appIcon", "volume", "percent", "muted"];

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!(<QtCore/QAbstractListModel>);
        type QAbstractListModel;

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
        type StreamModel = super::StreamModelRust;

        #[cxx_override]
        fn data(self: &StreamModel, index: &QModelIndex, role: i32) -> QVariant;
        #[cxx_override]
        fn row_count(self: &StreamModel, parent: &QModelIndex) -> i32;
        #[cxx_override]
        fn role_names(self: &StreamModel) -> QHash_i32_QByteArray;
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows(
            self: Pin<&mut StreamModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut StreamModel>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        unsafe fn begin_remove_rows(
            self: Pin<&mut StreamModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        unsafe fn end_remove_rows(self: Pin<&mut StreamModel>);
        #[inherit]
        #[cxx_name = "beginMoveRows"]
        unsafe fn begin_move_rows(
            self: Pin<&mut StreamModel>,
            source_parent: &QModelIndex,
            source_first: i32,
            source_last: i32,
            destination_parent: &QModelIndex,
            destination_child: i32,
        ) -> bool;
        #[inherit]
        #[cxx_name = "endMoveRows"]
        unsafe fn end_move_rows(self: Pin<&mut StreamModel>);
        #[inherit]
        fn index(self: &StreamModel, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
        #[inherit]
        #[qsignal]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut StreamModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QList_i32,
        );
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qlist.h");
        type QList_i32 = cxx_qt_lib::QList<i32>;
    }

    impl cxx_qt::Threading for StreamModel {}
    impl cxx_qt::Initialize for StreamModel {}
}

impl Keyed for AudioStream {
    fn key(&self) -> String {
        self.id.to_string()
    }
}

#[derive(Default)]
pub struct StreamModelRust {
    count: i32,
    rows: Vec<AudioStream>,
}

impl cxx_qt::Initialize for qobject::StreamModel {
    fn initialize(self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        AUDIO.subscribe(move |snapshot: Arc<AudioSnapshot>| {
            qt.queue(move |model: Pin<&mut qobject::StreamModel>| {
                model.apply(&snapshot);
            })
            .is_ok()
        });
    }
}

struct Sink<'a>(Pin<&'a mut qobject::StreamModel>);

impl RowSink<AudioStream> for Sink<'_> {
    fn rows(&mut self) -> &mut Vec<AudioStream> {
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

impl qobject::StreamModel {
    fn apply(mut self: Pin<&mut Self>, snapshot: &AudioSnapshot) {
        let rows = snapshot.streams.clone();
        reconcile(&mut Sink(self.as_mut()), rows);
        let rows = count(self.rust().rows.len());
        if self.rust().count != rows {
            self.as_mut().rust_mut().count = rows;
            self.count_changed();
        }
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(r) = row_of(&self.rust().rows, index) else {
            return QVariant::default();
        };
        match role - FIRST_ROLE {
            0 => QVariant::from(&r.id),
            1 => QVariant::from(&QString::from(r.app_name.as_str())),
            2 => QVariant::from(&QString::from(r.app_icon.as_str())),
            3 => QVariant::from(&f64::from(r.volume)),
            4 => QVariant::from(&i32::from(percent(r.volume))),
            5 => QVariant::from(&r.muted),
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
