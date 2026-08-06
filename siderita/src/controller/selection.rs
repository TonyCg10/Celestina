//! Acting on the current entry: single-click selection, double-click activation
//! (navigate into a folder, open a file, reveal a starred file), the read-only
//! accessors the QML calls per row (token / detail / path / kind / index) and a
//! text preview, plus the properties panel (metadata inline, a folder's
//! recursive size computed on a worker thread). Search hits and trashed entries
//! are read from their own lists so every row lookup takes the same path.

use core::pin::Pin;
use std::path::Path;

use celestina_core::CancellationToken;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use siderita_qt::RowKind;

use super::qobject;
use super::{kind_key, kind_label, search_hit_parent, PendingNav};
use crate::pathkey;

impl qobject::SideritaController {
    pub fn select_token(mut self: Pin<&mut Self>, token: &QString) {
        // The selected item's name and detail are shown in the sidebar info box
        // (driven by selected_token), so selecting no longer writes the status
        // line. A search hit's token is its index — accepted as-is if in range.
        if self.rust().virtual_rows() {
            if self.rust().search_hit(token).is_some() {
                self.as_mut().set_selected_token(token.clone());
            }
            return;
        }
        let selected = self
            .rust()
            .row_by_token(token)
            .map(|row| row.token().to_string());
        if let Some(token) = selected {
            self.as_mut()
                .set_selected_token(QString::from(token.as_str()));
        }
    }

    pub fn activate_token(mut self: Pin<&mut Self>, token: &QString) {
        // In Trash, activating an entry does nothing — restore / delete are the
        // actions, offered by the context menu; nothing is "opened" from Trash.
        if self.rust().trash_active {
            return;
        }
        // A search hit acts exactly like a folder entry: a folder navigates in
        // (leaving search), a file opens in its default app (search stays up so
        // more hits can be opened).
        if self.rust().search_active {
            let Some((path, is_dir, name)) = self
                .rust()
                .search_hit(token)
                .map(|hit| (hit.path.clone(), hit.is_dir, hit.name.clone()))
            else {
                return;
            };
            if is_dir {
                self.as_mut().exit_search();
                self.as_mut().request_nav_scan(PendingNav::To(path));
            } else {
                self.as_mut().set_selected_token(token.clone());
                self.as_mut().open_in_default_app(&path, &name);
            }
            return;
        }

        let selected = self.rust().row_by_token(token).map(|row| {
            (
                row.path().to_path_buf(),
                row.targets_directory(),
                row.display_name().to_owned(),
            )
        });

        let Some((path, enters_directory, name)) = selected else {
            return;
        };

        // A symlink to a folder is browsed rather than handed to `xdg-open`:
        // the row still labels it a link, but a linked home folder opens where
        // a folder would.
        if enters_directory {
            self.as_mut().request_nav_scan(PendingNav::To(path));
        } else {
            self.as_mut().select_token(token);
            self.as_mut().open_in_default_app(&path, &name);
        }
    }

    pub fn entry_token(&self, index: i32) -> QString {
        if self.rust().virtual_rows() {
            let count = self.rust().search_hits.len() as i32;
            return if index >= 0 && index < count {
                QString::from(index.to_string().as_str())
            } else {
                QString::default()
            };
        }
        self.rust()
            .row(index)
            .map(|row| QString::from(row.token().to_string().as_str()))
            .unwrap_or_default()
    }

    pub fn entry_detail(&self, index: i32) -> QString {
        // A search hit's detail is where it lives — its containing folder.
        if self.rust().search_active {
            return usize::try_from(index)
                .ok()
                .and_then(|i| self.rust().search_hits.get(i))
                .map(|hit| QString::from(search_hit_parent(&hit.path).as_str()))
                .unwrap_or_default();
        }
        // A trashed entry's detail is where it came from and when it was deleted.
        if self.rust().trash_active {
            return usize::try_from(index)
                .ok()
                .and_then(|i| self.rust().trash_entries.get(i))
                .map(|e| {
                    let origin = e.original.to_string_lossy();
                    let date = crate::format::trash_date(&e.deletion_date);
                    QString::from(
                        if date.is_empty() {
                            origin.into_owned()
                        } else {
                            format!("{origin} · {date}")
                        }
                        .as_str(),
                    )
                })
                .unwrap_or_default();
        }
        let Some(row) = self.rust().row(index) else {
            return QString::default();
        };
        let kind = kind_label(row.kind());
        let date = row
            .modified()
            .map(crate::format::system_time)
            .unwrap_or_default();
        // Folders show kind + date (their entry size is not meaningful); files
        // show kind · size · date.
        let detail = if row.kind() == RowKind::Directory {
            if date.is_empty() {
                kind.to_owned()
            } else {
                format!("{kind} · {date}")
            }
        } else {
            let size = crate::format::size(row.size());
            if date.is_empty() {
                format!("{kind} · {size}")
            } else {
                format!("{kind} · {size} · {date}")
            }
        };
        QString::from(detail.as_str())
    }

    pub fn entry_info(&self, index: i32) -> QStringList {
        let Some(index) = usize::try_from(index).ok() else {
            return QStringList::default();
        };

        // Virtual locations do not carry an EntryRow, but their selected path is
        // still enough for the same compact type/size/date summary. Trash uses
        // its deletion timestamp because that is the meaningful date there.
        if self.rust().trash_active {
            let Some(entry) = self.rust().trash_entries.get(index) else {
                return QStringList::default();
            };
            return path_info_lines(
                &entry.trashed,
                entry.trashed.is_dir(),
                Some(crate::format::trash_date_short(&entry.deletion_date)),
            );
        }
        if self.rust().search_active || self.rust().recent_active {
            let Some(hit) = self.rust().search_hits.get(index) else {
                return QStringList::default();
            };
            return path_info_lines(&hit.path, hit.is_dir, None);
        }

        let Some(row) = i32::try_from(index)
            .ok()
            .and_then(|row_index| self.rust().row(row_index))
        else {
            return QStringList::default();
        };
        let mut lines = vec![QString::from(kind_label(row.kind()))];
        if row.kind() != RowKind::Directory {
            lines.push(QString::from(crate::format::size(row.size()).as_str()));
        }
        if let Some(date) = row.modified().map(crate::format::system_time_short) {
            if !date.is_empty() {
                lines.push(QString::from(date.as_str()));
            }
        }
        lines.into_iter().collect()
    }

    pub fn index_for_token(&self, token: &QString) -> i32 {
        if self.rust().virtual_rows() {
            return token
                .to_string()
                .parse::<usize>()
                .ok()
                .filter(|&i| i < self.rust().search_hits.len())
                .and_then(|i| i32::try_from(i).ok())
                .unwrap_or(-1);
        }
        let Ok(token) = token.to_string().parse::<u64>() else {
            return -1;
        };
        self.rust()
            .view
            .as_ref()
            .and_then(|view| {
                view.rows()
                    .iter()
                    .position(|row| row.token().value() == token)
            })
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1)
    }

    /// The path key (ADR 0008) of the row at `index` — its byte-exact
    /// identity, and the argument every verb on this object expects. The name a
    /// person reads is `entry_names[index]`; the two are not interchangeable.
    pub fn entry_path(&self, index: i32) -> QString {
        if self.rust().virtual_rows() {
            return usize::try_from(index)
                .ok()
                .and_then(|i| self.rust().search_hits.get(i))
                .map(|hit| pathkey::publish(&hit.path))
                .unwrap_or_default();
        }
        self.rust()
            .row(index)
            .map(|row| pathkey::publish(row.path()))
            .unwrap_or_default()
    }

    /// The entry `key` names as a `file://` URI — for the `text/uri-list` a
    /// drag hands to another application, and for any surface that has to load
    /// the file through a URL.
    ///
    /// Composed here rather than in QML: `encodeURI` leaves `#` and `?` raw, so
    /// dragging `informe#3.pdf` handed the receiving application a URI that
    /// ended at the `#`, and `encodeURIComponent` per segment cannot spell a
    /// byte that is not valid UTF-8 at all. The rule is the portal's rule, and
    /// it has one owner.
    pub fn path_uri(&self, key: &QString) -> QString {
        pathkey::decode(key)
            .map(|path| QString::from(crate::dbus::path_to_uri(&path).as_str()))
            .unwrap_or_default()
    }

    /// Whether the name `key` spells is taken. One `lstat`, so it is safe to
    /// ask from the Qt thread; a dangling symlink still occupies the name and
    /// answers `true`.
    pub fn path_exists(&self, key: &QString) -> bool {
        pathkey::decode(key).is_ok_and(|path| std::fs::symlink_metadata(path).is_ok())
    }

    /// Whether activating the row at `index` enters a directory — true for a
    /// folder and for a symlink that resolves to one. The activation host asks
    /// this instead of comparing `entry_kind` to "directory", so a linked folder
    /// is never sent through the content classifier on its way to being opened.
    pub fn entry_targets_directory(&self, index: i32) -> bool {
        if self.rust().virtual_rows() {
            return usize::try_from(index)
                .ok()
                .and_then(|i| self.rust().search_hits.get(i))
                .is_some_and(|hit| hit.is_dir);
        }
        self.rust()
            .row(index)
            .is_some_and(siderita_qt::EntryRow::targets_directory)
    }

    pub fn entry_kind(&self, index: i32) -> QString {
        if self.rust().virtual_rows() {
            return usize::try_from(index)
                .ok()
                .and_then(|i| self.rust().search_hits.get(i))
                .map(|hit| QString::from(if hit.is_dir { "directory" } else { "file" }))
                .unwrap_or_default();
        }
        self.rust()
            .row(index)
            .map(|row| QString::from(kind_key(row.kind())))
            .unwrap_or_default()
    }

    /// Opens the folder holding `path` and selects that entry once it lands —
    /// how a starred *file* reveals itself from the sidebar, instead of the
    /// sidebar quietly launching an application.
    pub fn reveal_path(mut self: Pin<&mut Self>, key: &QString) {
        let Some(path) = self.as_mut().accept_key(key) else {
            return;
        };
        let Some(parent) = path.parent().map(Path::to_path_buf) else {
            return;
        };
        self.as_mut().rust_mut().get_mut().pending_select_path = Some(path);
        self.as_mut().request_nav_scan(PendingNav::To(parent));
    }

    /// A lossy, capped sample for the read-only quick-look pane.
    ///
    /// This does not decide whether anything is editable and must never be
    /// asked to: `grafita-core` classifies content by bytes and encoding on a
    /// worker, and its answer is what routes `Space` to the editor. What
    /// reaches quick-look has already been refused as editable, so this only
    /// has to render something legible from it.
    pub fn preview_text(&self, key: &QString) -> QString {
        // Cap the read: a preview only needs the first screenful or two, and this
        // runs on the GUI thread (the user pressed space), so it must stay cheap.
        const MAX_BYTES: usize = 128 * 1024;
        let Ok(path) = pathkey::decode(key) else {
            return QString::default();
        };
        let Ok(file) = std::fs::File::open(&path) else {
            return QString::default();
        };
        use std::io::Read;
        let mut buf = Vec::new();
        if file.take(MAX_BYTES as u64).read_to_end(&mut buf).is_err() {
            return QString::default();
        }
        // A NUL byte in the sample is the cheap, reliable "this is binary" tell —
        // real text files don't carry them, most binaries do within 128 KiB.
        if buf.contains(&0) {
            return QString::default();
        }
        // Lossy so one stray non-UTF-8 byte shows a � rather than blanking the
        // whole preview; genuinely binary content was already rejected above.
        QString::from(String::from_utf8_lossy(&buf).as_ref())
    }

    /// Opens the properties panel for `path`: the metadata is gathered inline
    /// (fast), and a folder's recursive size is computed on a worker thread so a
    /// deep tree never blocks the UI.
    pub fn open_properties(mut self: Pin<&mut Self>, key: &QString) {
        let Some(path) = self.as_mut().accept_key(key) else {
            return;
        };

        // Cancel any directory-size walk still running from a previous open.
        if let Some(token) = self.as_mut().rust_mut().get_mut().prop_size_cancel.take() {
            token.cancel();
        }

        let props = crate::properties::gather(&path);
        self.as_mut()
            .set_prop_name(QString::from(props.name.as_str()));
        self.as_mut()
            .set_prop_path(QString::from(props.path.as_str()));
        self.as_mut()
            .set_prop_kind(QString::from(props.kind.as_str()));
        self.as_mut()
            .set_prop_mime(QString::from(props.mime.as_str()));
        self.as_mut()
            .set_prop_permissions(QString::from(props.permissions.as_str()));
        self.as_mut()
            .set_prop_owner(QString::from(props.owner.as_str()));
        self.as_mut()
            .set_prop_modified(QString::from(props.modified.as_str()));
        self.as_mut()
            .set_prop_accessed(QString::from(props.accessed.as_str()));
        self.as_mut().set_prop_symlink(QString::from(
            props.symlink_target.unwrap_or_default().as_str(),
        ));
        self.as_mut().set_prop_is_dir(props.is_dir);

        match props.size {
            Some(size) => self
                .as_mut()
                .set_prop_size(QString::from(crate::format::size_full(size).as_str())),
            None => {
                self.as_mut().set_prop_size(QString::from("Calculando…"));
                let token = CancellationToken::new();
                self.as_mut().rust_mut().get_mut().prop_size_cancel = Some(token.clone());
                let qt = self.qt_thread();
                let dir = path.clone();
                let dir_key = props.path.clone();
                std::thread::spawn(move || {
                    let size = crate::properties::directory_size(&dir, &token);
                    if token.is_cancelled() {
                        return;
                    }
                    let text = crate::format::size_full(size);
                    let _ = qt.queue(
                        move |mut controller: Pin<&mut qobject::SideritaController>| {
                            // Ignore if the panel has since moved to another entry.
                            if controller.rust().prop_path.to_string() == dir_key {
                                controller
                                    .as_mut()
                                    .set_prop_size(QString::from(text.as_str()));
                            }
                        },
                    );
                });
            }
        }

        self.as_mut().set_properties_pending(true);
    }

    pub fn close_properties(mut self: Pin<&mut Self>) {
        if let Some(token) = self.as_mut().rust_mut().get_mut().prop_size_cancel.take() {
            token.cancel();
        }
        self.as_mut().set_properties_pending(false);
    }
}

fn path_info_lines(path: &Path, is_dir: bool, date: Option<String>) -> QStringList {
    let mut lines = vec![QString::from(if is_dir { "Carpeta" } else { "Archivo" })];
    let metadata = std::fs::metadata(path).ok();
    if !is_dir {
        if let Some(size) = metadata.as_ref().map(std::fs::Metadata::len) {
            lines.push(QString::from(crate::format::size(size).as_str()));
        }
    }
    let date = date.unwrap_or_else(|| {
        metadata
            .and_then(|value| value.modified().ok())
            .map(crate::format::system_time_short)
            .unwrap_or_default()
    });
    if !date.is_empty() {
        lines.push(QString::from(date.as_str()));
    }
    lines.into_iter().collect()
}
