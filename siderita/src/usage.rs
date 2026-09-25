//! The occupation of a folder for the properties dialog and the quick look,
//! as a QML hub.
//!
//! The walk, the tree, the mount boundaries and the projection are
//! `hematita_core::usage`'s; the rules about which scan is current are
//! [`crate::usage_session`]'s. This file runs the scan on its own thread and
//! marshals the current folder to Qt, nothing else.
//!
//! - **One scan at a time.** `open` on another folder cancels the running
//!   walk through its token; `close` cancels it too, so no thread keeps
//!   reading the disk after the modal is gone. The walk checks the token
//!   every few hundred entries.
//! - **Only the current result lands.** Progress and the tree carry the
//!   generation they were asked under, and the session drops a stale one.
//! - **Drilling costs no IO.** `enter` and `up` reproject the tree in memory.
//!
//! The scan thread is detached, not joined: joining would block the Qt
//! thread until the walk reached its next cancellation check. It owns its
//! root path and a `CxxQtThread` whose `queue` drops the result once the hub
//! is gone. `/proc/self/mountinfo` is read on that thread too.
//!
//! Every path crossing this bridge is a path key (ADR 0008), encoded and
//! decoded by [`crate::pathkey`]. Siderita never links `usage::remove`: its
//! own trash and delete stay its only removals.

use std::collections::HashSet;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QString, QStringList, QVariant};
use hematita_core::usage::mounts::{mount_targets, parse_mountinfo};
use hematita_core::usage::tree::Tree;
use hematita_core::usage::walk::{scan, Progress, ScanError};

use crate::usage_session::UsageSession;

/// The least time between two progress publications of one scan.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

const MOUNTINFO: &str = "/proc/self/mountinfo";

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;

        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        // mode       — idle | scanning | analysed | failed
        // failure    — why a scan failed, as a token: root | not-a-folder |
        //              too-many | thread; the page words it
        // root, currentPath — path keys of the scanned folder and of the one
        //              shown inside it
        // crumbs     — folder names from the scanned root to the current one
        // row*       — the current folder's children, index-aligned, biggest
        //              first; id -1 is the merged remainder
        // tiles      — the treemap, `[id, x, y, w, h]` per tile in 0..1
        // revision   — set last, after every other property of a publication
        #[qobject]
        #[qml_element]
        #[qproperty(QString, owner)]
        #[qproperty(QString, mode)]
        #[qproperty(QString, failure)]
        #[qproperty(QString, root)]
        #[qproperty(QString, current_path)]
        #[qproperty(QStringList, crumbs)]
        #[qproperty(f64, progress_entries)]
        #[qproperty(f64, progress_bytes)]
        #[qproperty(f64, total_bytes)]
        #[qproperty(f64, files_below)]
        #[qproperty(f64, folders_below)]
        #[qproperty(f64, unreadable_below)]
        #[qproperty(f64, other_devices)]
        #[qproperty(QVariant, row_ids)]
        #[qproperty(QStringList, row_names)]
        #[qproperty(QStringList, row_kinds)]
        #[qproperty(QVariant, row_allocated)]
        #[qproperty(QVariant, row_shares)]
        #[qproperty(QVariant, row_files)]
        #[qproperty(QVariant, row_unreadable)]
        #[qproperty(QVariant, row_merged)]
        #[qproperty(QVariant, tiles)]
        #[qproperty(i32, revision)]
        type SideritaUsage = super::SideritaUsageRust;

        /// Scans the folder the path key `path` names for the surface
        /// `owner`, unless it is the one already scanned or being scanned.
        /// `owner` takes the hub over. An empty key closes for `owner`.
        #[qinvokable]
        fn open(self: Pin<&mut SideritaUsage>, path: &QString, owner: &QString);

        /// Cancels the scan and forgets the tree when `owner` opened the hub
        /// last; otherwise nothing. Idempotent: the modals call it on every
        /// dismissal and on every step to an entry that is not a folder.
        #[qinvokable]
        fn close(self: Pin<&mut SideritaUsage>, owner: &QString);

        /// Shows the child folder `id` of the current folder; false for a
        /// file, the remainder or another device's mount.
        #[qinvokable]
        fn enter(self: Pin<&mut SideritaUsage>, id: i32) -> bool;

        /// Shows the current folder's parent; false at the scanned root.
        #[qinvokable]
        fn up(self: Pin<&mut SideritaUsage>) -> bool;

        /// Shows the crumb at `depth` (0 is the scanned root) in one step.
        #[qinvokable]
        fn up_to(self: Pin<&mut SideritaUsage>, depth: i32) -> bool;

        /// The path key of node `id`, or empty when it is not in the tree.
        #[qinvokable]
        fn path_of(self: &SideritaUsage, id: i32) -> QString;

        /// Hands the folder the path key `path` names to Hematita's storage
        /// section through its desktop entry, which applies `%f`. False when
        /// the launch could not even start; the reason goes to the log and
        /// the page words the failure.
        #[qinvokable]
        fn open_in_hematita(self: &SideritaUsage, path: &QString) -> bool;
    }

    impl cxx_qt::Threading for SideritaUsage {}
}

pub struct SideritaUsageRust {
    owner: QString,
    mode: QString,
    failure: QString,
    root: QString,
    current_path: QString,
    crumbs: QStringList,
    progress_entries: f64,
    progress_bytes: f64,
    total_bytes: f64,
    files_below: f64,
    folders_below: f64,
    unreadable_below: f64,
    other_devices: f64,
    row_ids: QVariant,
    row_names: QStringList,
    row_kinds: QStringList,
    row_allocated: QVariant,
    row_shares: QVariant,
    row_files: QVariant,
    row_unreadable: QVariant,
    row_merged: QVariant,
    tiles: QVariant,
    revision: i32,

    session: UsageSession,
}

impl Default for SideritaUsageRust {
    fn default() -> Self {
        Self {
            owner: QString::default(),
            mode: QString::from("idle"),
            failure: QString::default(),
            root: QString::default(),
            current_path: QString::default(),
            crumbs: QStringList::default(),
            progress_entries: 0.0,
            progress_bytes: 0.0,
            total_bytes: 0.0,
            files_below: 0.0,
            folders_below: 0.0,
            unreadable_below: 0.0,
            other_devices: 0.0,
            row_ids: doubles(&[]),
            row_names: QStringList::default(),
            row_kinds: QStringList::default(),
            row_allocated: doubles(&[]),
            row_shares: doubles(&[]),
            row_files: doubles(&[]),
            row_unreadable: doubles(&[]),
            row_merged: doubles(&[]),
            tiles: doubles(&[]),
            revision: 0,
            session: UsageSession::default(),
        }
    }
}

impl qobject::SideritaUsage {
    pub fn open(mut self: Pin<&mut Self>, path: &QString, owner: &QString) {
        if path.is_empty() {
            self.close(owner);
            return;
        }
        let owner_text = owner.to_string();
        let root = match crate::pathkey::decode(path) {
            Ok(root) => root,
            Err(error) => {
                eprintln!("siderita-usage: refused a path key: {error}");
                self.as_mut().rust_mut().session.close();
                self.as_mut().rust_mut().session.owner = owner_text;
                self.as_mut().set_owner(owner.clone());
                self.fail("root");
                return;
            }
        };
        self.as_mut().set_owner(owner.clone());
        let Some((generation, token)) = self
            .as_mut()
            .rust_mut()
            .session
            .begin(root.clone(), &owner_text)
        else {
            return;
        };
        self.as_mut().set_mode(QString::from("scanning"));
        self.as_mut().set_failure(QString::default());
        self.as_mut().set_progress_entries(0.0);
        self.as_mut().set_progress_bytes(0.0);
        self.as_mut().publish();

        let qt = self.qt_thread();
        let spawned = std::thread::Builder::new()
            .name("siderita-usage".to_owned())
            .spawn(move || {
                let boundaries = mount_boundaries();
                let mut last: Option<Instant> = None;
                let mut report = |progress: Progress| {
                    let now = Instant::now();
                    if !due(last, now, PROGRESS_INTERVAL) {
                        return;
                    }
                    last = Some(now);
                    let (files, bytes) = (progress.files, progress.bytes);
                    let _ = qt.queue(move |hub: Pin<&mut qobject::SideritaUsage>| {
                        hub.apply_progress(generation, files, bytes);
                    });
                };
                let result = scan(&root, &boundaries, &token, &mut report);
                let _ = qt.queue(move |hub: Pin<&mut qobject::SideritaUsage>| {
                    hub.apply_tree(generation, result);
                });
            });
        if let Err(error) = spawned {
            eprintln!("siderita-usage: cannot start the scan thread: {error}");
            self.as_mut().rust_mut().session.close();
            self.as_mut().rust_mut().session.owner = owner_text;
            self.fail("thread");
        }
    }

    pub fn close(mut self: Pin<&mut Self>, owner: &QString) {
        if !self
            .as_mut()
            .rust_mut()
            .session
            .close_by(&owner.to_string())
        {
            return;
        }
        self.as_mut().set_owner(QString::default());
        self.as_mut().set_mode(QString::from("idle"));
        self.as_mut().set_failure(QString::default());
        self.as_mut().set_progress_entries(0.0);
        self.as_mut().set_progress_bytes(0.0);
        self.publish();
    }

    pub fn enter(mut self: Pin<&mut Self>, id: i32) -> bool {
        let moved = self.as_mut().rust_mut().session.enter(id);
        if moved {
            self.publish();
        }
        moved
    }

    pub fn up(mut self: Pin<&mut Self>) -> bool {
        let moved = self.as_mut().rust_mut().session.up();
        if moved {
            self.publish();
        }
        moved
    }

    pub fn up_to(mut self: Pin<&mut Self>, depth: i32) -> bool {
        let Ok(depth) = usize::try_from(depth) else {
            return false;
        };
        let moved = self.as_mut().rust_mut().session.up_to(depth);
        if moved {
            self.publish();
        }
        moved
    }

    pub fn path_of(&self, id: i32) -> QString {
        self.rust()
            .session
            .path_of(id)
            .map(|path| crate::pathkey::publish(&path))
            .unwrap_or_default()
    }

    pub fn open_in_hematita(&self, path: &QString) -> bool {
        let launched = crate::pathkey::decode(path)
            .map_err(|error| error.to_string())
            .and_then(|path| crate::apps::launch_with("org.celestina.Hematita", &path));
        if let Err(error) = &launched {
            eprintln!("siderita-usage: cannot open Hematita: {error}");
        }
        launched.is_ok()
    }

    /// Progress of the scan asked under `generation`, when it is current.
    fn apply_progress(mut self: Pin<&mut Self>, generation: u64, files: u64, bytes: u64) {
        if generation != self.rust().session.generation {
            return;
        }
        self.as_mut().set_progress_entries(files as f64);
        self.as_mut().set_progress_bytes(bytes as f64);
    }

    /// The end of the scan asked under `generation`, when it is current.
    fn apply_tree(mut self: Pin<&mut Self>, generation: u64, result: Result<Tree, ScanError>) {
        match result {
            Ok(tree) => {
                if self.as_mut().rust_mut().session.accept(generation, tree) {
                    self.as_mut().set_mode(QString::from("analysed"));
                    self.publish();
                }
            }
            // A close or a newer open cancelled it and already published.
            Err(ScanError::Cancelled) => {}
            Err(error) => {
                if self.as_mut().rust_mut().session.failed(generation) {
                    eprintln!("siderita-usage: {error}");
                    let token = match error {
                        ScanError::NotADirectory { .. } => "not-a-folder",
                        ScanError::TooManyEntries { .. } => "too-many",
                        ScanError::Root { .. } | ScanError::Cancelled => "root",
                    };
                    self.fail(token);
                }
            }
        }
    }

    fn fail(mut self: Pin<&mut Self>, token: &str) {
        self.as_mut().set_mode(QString::from("failed"));
        self.as_mut().set_failure(QString::from(token));
        self.publish();
    }

    /// Every property from the session's projection, `revision` last.
    fn publish(mut self: Pin<&mut Self>) {
        let projection = self.rust().session.project();
        let key = |path: &PathBuf| {
            if path.as_os_str().is_empty() {
                QString::default()
            } else {
                crate::pathkey::publish(path)
            }
        };
        self.as_mut().set_root(key(&projection.root_path));
        self.as_mut()
            .set_current_path(key(&projection.current_path));
        self.as_mut().set_crumbs(strings(projection.crumbs));
        self.as_mut().set_total_bytes(projection.total_bytes);
        self.as_mut().set_files_below(projection.files_below);
        self.as_mut().set_folders_below(projection.folders_below);
        self.as_mut()
            .set_unreadable_below(projection.unreadable_below);
        self.as_mut().set_other_devices(projection.other_devices);
        self.as_mut().set_row_ids(doubles(&projection.row_ids));
        self.as_mut().set_row_names(strings(projection.row_names));
        self.as_mut().set_row_kinds(strings(projection.row_kinds));
        self.as_mut()
            .set_row_allocated(doubles(&projection.row_allocated));
        self.as_mut()
            .set_row_shares(doubles(&projection.row_shares));
        self.as_mut().set_row_files(doubles(&projection.row_files));
        self.as_mut()
            .set_row_unreadable(doubles(&projection.row_unreadable));
        self.as_mut()
            .set_row_merged(doubles(&projection.row_merged));
        self.as_mut().set_tiles(doubles(&projection.rects));
        let revision = self.rust().revision.wrapping_add(1);
        self.set_revision(revision);
    }
}

/// The mount targets the walk stops at. Without the table it still stops at
/// other devices; only same-device mounts would go unrecognised.
fn mount_boundaries() -> HashSet<PathBuf> {
    std::fs::read_to_string(MOUNTINFO)
        .map(|text| mount_targets(&parse_mountinfo(&text)))
        .unwrap_or_default()
}

/// Whether a progress report is due, `interval` after the last one sent.
fn due(last: Option<Instant>, now: Instant, interval: Duration) -> bool {
    last.is_none_or(|last| now.duration_since(last) >= interval)
}

fn strings(values: Vec<String>) -> QStringList {
    let mut list = QStringList::default();
    for value in values {
        list.append(QString::from(value.as_str()));
    }
    list
}

fn doubles(values: &[f64]) -> QVariant {
    let mut list = QList::<QVariant>::default();
    for value in values {
        list.append(QVariant::from(value));
    }
    QVariant::from(&list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_report_is_due_and_the_next_waits_the_interval() {
        let start = Instant::now();
        assert!(due(None, start, PROGRESS_INTERVAL));
        assert!(!due(
            Some(start),
            start + Duration::from_millis(100),
            PROGRESS_INTERVAL
        ));
        assert!(due(
            Some(start),
            start + PROGRESS_INTERVAL,
            PROGRESS_INTERVAL
        ));
    }
}
