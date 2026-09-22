//! The shapes a hub publishes to QML.
//!
//! CXX-Qt 0.9 cannot override `QAbstractListModel`'s virtuals from Rust, so
//! both hubs publish index-aligned lists instead of models. These four
//! functions are the whole conversion: a list of strings, a list of numbers,
//! a list of lists of numbers, and the widening `f32` histories need. They
//! live here because the resource hub and the process hub are two callers of
//! one recipe, not two recipes.

use cxx_qt_lib::{QList, QString, QStringList, QVariant};

pub fn strings(values: impl IntoIterator<Item = String>) -> QStringList {
    let mut list = QStringList::default();
    for value in values {
        list.append(QString::from(value.as_str()));
    }
    list
}

#[must_use]
pub fn doubles(values: &[f64]) -> QVariant {
    let mut list = QList::<QVariant>::default();
    for value in values {
        list.append(QVariant::from(value));
    }
    QVariant::from(&list)
}

/// Lists of lists cross as a `QVariant`: it is the one shape both qmllint and
/// the engine resolve (see H1-D), and a page reads them by row index.
#[must_use]
pub fn nested(rows: &[Vec<f64>]) -> QVariant {
    let mut list = QList::<QVariant>::default();
    for row in rows {
        list.append(doubles(row));
    }
    QVariant::from(&list)
}

#[must_use]
pub fn widen(values: &[f32]) -> Vec<f64> {
    values.iter().map(|value| f64::from(*value)).collect()
}
