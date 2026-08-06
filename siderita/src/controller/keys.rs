//! Accepting a path key at an invokable boundary.
//!
//! ADR 0008 makes every path that enters this object a key, and makes a key
//! that is not well formed a typed refusal rather than something to salvage.
//! These are the two shapes that refusal takes on the controller: one entry,
//! or a whole batch. A refusal is reported through `op_error`, the same surface
//! a failed write uses, so a caller that hands over a raw path sees the reason
//! instead of watching the verb do nothing.

use core::pin::Pin;
use std::path::PathBuf;

use cxx_qt_lib::{QString, QStringList};

use super::qobject;
use crate::pathkey::{self, KeyError};

impl qobject::SideritaController {
    /// The path `key` names, or `None` after reporting why it names none.
    pub(crate) fn accept_key(mut self: Pin<&mut Self>, key: &QString) -> Option<PathBuf> {
        match pathkey::decode(key) {
            Ok(path) => Some(path),
            // An empty argument is an interface state, not a fault: a menu can
            // fire with nothing selected, and that has always been a no-op.
            Err(KeyError::Empty) => None,
            Err(error) => {
                self.as_mut().report_key_error(error);
                None
            }
        }
    }

    /// The same, kept as the normalized key string the persisted marks
    /// (favourites, custom icons) are stored under.
    pub(crate) fn accept_mark(self: Pin<&mut Self>, key: &QString) -> Option<String> {
        self.accept_key(key).map(|path| pathkey::encode(&path))
    }

    /// Every path in `keys`, or `None` after reporting the first refusal. A
    /// batch is honoured whole or not at all.
    pub(crate) fn accept_keys(
        mut self: Pin<&mut Self>,
        keys: &QStringList,
    ) -> Option<Vec<PathBuf>> {
        match pathkey::decode_list(keys) {
            Ok(paths) => Some(paths),
            Err(error) => {
                self.as_mut().report_key_error(error);
                None
            }
        }
    }

    fn report_key_error(mut self: Pin<&mut Self>, error: KeyError) {
        let message = format!("{error}");
        self.as_mut().set_op_error(QString::from(message.as_str()));
    }
}
