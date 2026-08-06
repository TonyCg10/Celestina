//! Finding entries: the in-folder name filter / query (which just reprojects
//! the current snapshot) and the bounded recursive filename search, which walks
//! on a worker thread and paints its results through the same model + roles the
//! folder view uses, so hits look and behave like ordinary rows.

use core::pin::Pin;
use std::path::Path;

use celestina_core::CancellationToken;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};

use super::qobject;
use super::search_hit_parent;

impl qobject::SideritaController {
    pub fn apply_query(mut self: Pin<&mut Self>, query: &QString) {
        if self.query() == query {
            return;
        }

        self.as_mut().set_query(query.clone());
        self.as_mut().rust_mut().get_mut().options.query = query.to_string();
        self.as_mut().reproject();
    }

    pub fn apply_name_filters(mut self: Pin<&mut Self>, patterns: &QStringList) {
        let patterns: Vec<String> = patterns
            .iter()
            .map(ToString::to_string)
            .filter(|pattern| !pattern.is_empty())
            .collect();
        if self.rust().options.name_filters == patterns {
            return;
        }
        self.as_mut().rust_mut().get_mut().options.name_filters = patterns;
        self.as_mut().reproject();
    }

    /// Runs a bounded recursive filename search of the current folder on a worker
    /// thread and shows the results overlay. Truthful about scope: the summary
    /// reports the match cap and whether the walk was cut short.
    pub fn search_recursive(mut self: Pin<&mut Self>, query: &QString) {
        let query = query.to_string();
        if query.trim().is_empty() {
            return;
        }
        let Some(root) = self.rust().history.current().map(Path::to_path_buf) else {
            return;
        };

        if let Some(token) = self.as_mut().rust_mut().get_mut().search_cancel.take() {
            token.cancel();
        }
        let token = CancellationToken::new();
        self.as_mut().rust_mut().get_mut().search_cancel = Some(token.clone());
        self.as_mut()
            .set_search_query(QString::from(query.as_str()));
        // `search_active` only flips once results land and replace the folder
        // rows — during the walk the folder view stays live and interactive.
        self.as_mut().set_search_running(true);
        self.as_mut().set_search_summary(QString::from("Buscando…"));

        const LIMIT: usize = 500;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let outcome = crate::search::search(&root, &query, LIMIT, &token);
            if token.is_cancelled() && outcome.hits.is_empty() {
                // A search superseded before it found anything: drop it.
                return;
            }
            let _ = qt.queue(move |controller: Pin<&mut qobject::SideritaController>| {
                controller.publish_search(outcome);
            });
        });
    }

    /// Publishes a finished (or cancelled) search onto the Qt thread.
    fn publish_search(mut self: Pin<&mut Self>, outcome: crate::search::SearchOutcome) {
        let current = self.rust().history.current().map(Path::to_path_buf);
        let in_current = |hit: &crate::search::SearchHit| current.as_deref() == hit.path.parent();

        // Group the hits: those in the searched folder first, then everything
        // deeper — each group A→Z — so the two sections read contiguously.
        let mut hits = outcome.hits;
        hits.sort_by(|a, b| {
            in_current(b)
                .cmp(&in_current(a))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let summary = if outcome.cancelled {
            format!(
                "{} coincidencias · búsqueda detenida ({} carpetas)",
                hits.len(),
                outcome.dirs_scanned
            )
        } else if outcome.truncated {
            format!(
                "{}+ coincidencias · detenida en el límite ({} carpetas)",
                hits.len(),
                outcome.dirs_scanned
            )
        } else {
            format!(
                "{} coincidencias · {} carpetas exploradas",
                hits.len(),
                outcome.dirs_scanned
            )
        };

        // Parallel role columns so the hits ride the *same* model + roles the
        // folder view uses — the list/grid then render and behave identically
        // (single-click selects, double-click opens, keyboard, selection). The
        // token is the hit index, the subtitle its containing folder, and the
        // section the header the list groups it under.
        let names: QStringList = hits
            .iter()
            .map(|h| QString::from(h.name.as_str()))
            .collect();
        // Identity, not text (ADR 0008): a hit is opened, revealed and trashed
        // through the key published here.
        let paths: QStringList = hits
            .iter()
            .map(|h| crate::pathkey::publish(&h.path))
            .collect();
        let kinds: QStringList = hits
            .iter()
            .map(|h| QString::from(if h.is_dir { "directory" } else { "file" }))
            .collect();
        let tokens: QStringList = (0..hits.len())
            .map(|i| QString::from(i.to_string().as_str()))
            .collect();
        let subtitles: QStringList = hits
            .iter()
            .map(|h| QString::from(search_hit_parent(&h.path).as_str()))
            .collect();
        let sections: QStringList = hits
            .iter()
            .map(|h| {
                QString::from(if in_current(h) {
                    "En esta carpeta"
                } else {
                    "En subcarpetas"
                })
            })
            .collect();
        // Search always renders as the sectioned list, never the details
        // columns, so the size/date columns are left blank for hits.
        let blank: QStringList = hits.iter().map(|_| QString::default()).collect();

        self.as_mut().rust_mut().get_mut().search_hits = hits;
        self.as_mut()
            .set_search_summary(QString::from(summary.as_str()));
        self.as_mut().set_search_running(false);
        self.as_mut().set_search_active(true);
        // A fresh result set drops any selection carried over from the folder.
        self.as_mut().set_selected_token(QString::default());
        self.as_mut().set_entry_names(names.clone());
        self.as_mut().rows_ready(
            names,
            tokens,
            kinds,
            subtitles,
            paths,
            sections,
            blank.clone(),
            blank,
        );
    }

    pub fn cancel_search(mut self: Pin<&mut Self>) {
        if let Some(token) = self.as_mut().rust_mut().get_mut().search_cancel.take() {
            token.cancel();
        }
    }

    /// Leaves search without touching the view — the caller repaints (a folder
    /// reproject, or a navigation scan) once it has decided what to show next.
    pub(crate) fn exit_search(mut self: Pin<&mut Self>) {
        self.as_mut().cancel_search();
        self.as_mut().rust_mut().get_mut().search_hits.clear();
        self.as_mut().set_search_running(false);
        self.as_mut().set_search_active(false);
    }

    /// Cancels search and returns the content box to the current folder's rows.
    pub fn close_search(mut self: Pin<&mut Self>) {
        self.as_mut().exit_search();
        self.as_mut().reproject();
    }
}
