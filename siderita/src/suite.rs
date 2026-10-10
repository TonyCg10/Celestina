//! The «Abrir en» section of an entry's menu: which of the suite's
//! applications apply to the acting entries, opening them there, and sending
//! files to a phone through Magnetita.
//!
//! A narrow object rather than more of the controller: the controller is a
//! ratcheted legacy coordinator, and this needs none of its state. The menu
//! hands it each acting entry's key and whether the folder model says it is a
//! folder; media is decided by name. Nothing here touches the disk on the Qt
//! thread: the installed applications were looked up on a worker at start
//! ([`crate::apps::probe_installed`]), opening goes through
//! [`crate::apps::open_in`]'s worker, and a send through the device model's.

use std::pin::Pin;

use cxx_qt::Threading;
use cxx_qt_lib::{QString, QStringList};

use crate::apps::{self, ItemKind, Target};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type SideritaSuite = super::SideritaSuiteRust;

        /// The targets that apply to the entries `keys` name, `folders`
        /// parallel to them ("1" for a folder): `grafita`, `fluorita`,
        /// `calcita`, `hematita` when installed, and `phone` when every entry
        /// is a file; the menu turns `phone` into one entry per connected
        /// phone.
        #[qinvokable]
        fn targets(self: &SideritaSuite, keys: &QStringList, folders: &QStringList) -> QStringList;

        /// Opens the entries `keys` name in the application `target`.
        #[qinvokable]
        fn open(self: Pin<&mut SideritaSuite>, target: &QString, keys: &QStringList);

        /// Sends the files `keys` name to the phone with id `device`.
        #[qinvokable]
        fn send(self: &SideritaSuite, device: &QString, keys: &QStringList);

        /// An application could not be started; the message is worded for
        /// the person.
        #[qsignal]
        fn failed(self: Pin<&mut SideritaSuite>, message: QString);
    }

    impl cxx_qt::Threading for SideritaSuite {}
}

#[derive(Default)]
pub struct SideritaSuiteRust;

/// Each entry's kind: a folder when the folder model says so — which counts a
/// symlink to a folder, as activating it navigates — media or a PDF by name,
/// any other file (a broken symlink among them) otherwise.
fn item_kinds(paths: &[std::path::PathBuf], folders: &[bool]) -> Vec<ItemKind> {
    paths
        .iter()
        .zip(folders)
        .map(|(path, folder)| {
            if *folder {
                ItemKind::Folder
            } else if fluorita_core::MediaKind::classify_path(path).is_some() {
                ItemKind::Media
            } else if is_pdf_name(path) {
                ItemKind::Pdf
            } else {
                ItemKind::File
            }
        })
        .collect()
}

/// Whether `path` is named as a PDF: the extension `pdf` in any case, as
/// Calcita itself admits a path.
fn is_pdf_name(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
}

impl qobject::SideritaSuite {
    pub fn targets(&self, keys: &QStringList, folders: &QStringList) -> QStringList {
        let Ok(paths) = crate::pathkey::decode_list(keys) else {
            return QStringList::default();
        };
        let folders: Vec<bool> = folders.iter().map(|flag| flag.to_string() == "1").collect();
        if folders.len() != paths.len() {
            return QStringList::default();
        }
        let kinds = item_kinds(&paths, &folders);
        apps::only_installed(apps::suite_targets(&kinds))
            .into_iter()
            .map(|target| QString::from(target.key()))
            .collect()
    }

    pub fn open(self: Pin<&mut Self>, target: &QString, keys: &QStringList) {
        let Some(target) = Target::application(&target.to_string()) else {
            return;
        };
        let Some(name) = target.activation_name() else {
            return;
        };
        let Ok(paths) = crate::pathkey::decode_list(keys) else {
            return;
        };
        if paths.is_empty() {
            return;
        }
        let qt = self.qt_thread();
        apps::open_in(
            name,
            paths,
            move |paths| {
                use crate::controller::shell::spawn_detached_all;
                match target {
                    // Hematita's program is reached by its desktop id, as
                    // `gtk-launch` finds it; it looks at one folder.
                    Target::Hematita => paths
                        .first()
                        .map_or(Ok(()), |path| apps::launch_with(name.0, path)),
                    Target::Grafita => spawn_detached_all("grafita", paths),
                    Target::Fluorita => spawn_detached_all("fluorita", paths),
                    Target::Calcita => spawn_detached_all("calcita", paths),
                    // Not an application: `Target::application` never names it.
                    Target::Phone => Ok(()),
                }
            },
            move |outcome| {
                // A refusal by a running instance is already on stderr; only
                // a failed start reaches the window.
                let Err(apps::OpenFailure::Spawn(message)) = outcome else {
                    return;
                };
                let _ = qt.queue(move |suite: Pin<&mut qobject::SideritaSuite>| {
                    suite.failed(QString::from(message.as_str()));
                });
            },
        );
    }

    pub fn send(&self, device: &QString, keys: &QStringList) {
        let device = device.to_string();
        if device.is_empty() {
            return;
        }
        let Ok(paths) = crate::pathkey::decode_list(keys) else {
            return;
        };
        // One `SendFileUri` per file on the device model's worker: the path
        // leaves as its bytes, never as display text (ADR 0008).
        for path in paths {
            crate::devicemodel::send_file(&device, path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{item_kinds, ItemKind};
    use std::path::PathBuf;

    /// The folder flag as the folder model sets it: whether activating the
    /// entry enters a directory, following a link
    /// (`EntryRow::targets_directory`); `Path::is_dir` follows links too.
    fn model_flag(path: &std::path::Path) -> bool {
        path.is_dir()
    }

    #[test]
    fn a_symlink_to_a_folder_is_a_folder_and_a_broken_one_a_file() {
        let dir = std::env::temp_dir().join(format!("siderita-suite-links-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("real")).unwrap();
        let to_folder = dir.join("to-folder");
        let broken = dir.join("broken");
        std::os::unix::fs::symlink(dir.join("real"), &to_folder).unwrap();
        std::os::unix::fs::symlink(dir.join("nowhere"), &broken).unwrap();
        let paths: Vec<PathBuf> = vec![to_folder, broken];
        let folders: Vec<bool> = paths.iter().map(|path| model_flag(path)).collect();
        let kinds = item_kinds(&paths, &folders);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(kinds, vec![ItemKind::Folder, ItemKind::File]);
    }

    #[test]
    fn media_and_pdfs_are_decided_by_name() {
        let paths = vec![
            PathBuf::from("/x/photo.png"),
            PathBuf::from("/x/notes"),
            PathBuf::from("/x/informe.pdf"),
            PathBuf::from("/x/INFORME.PDF"),
            PathBuf::from("/x/pdf"),
        ];
        assert_eq!(
            item_kinds(&paths, &[false; 5]),
            vec![
                ItemKind::Media,
                ItemKind::File,
                ItemKind::Pdf,
                ItemKind::Pdf,
                ItemKind::File
            ]
        );
    }
}
