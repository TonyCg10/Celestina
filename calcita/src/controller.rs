// language-contract: product-copy
//! The window-wide controller, a QML singleton.
//!
//! It carries the appearance to every window, owns the set of open documents
//! (one window each, keyed by the path's pathkey), admits what `Open`, a drop
//! or the file chooser asks to open, and keeps the recent documents. The
//! per-window reading state lives in `CalcitaDocument`. The singleton lives as
//! long as the engine, so its follower and its store worker live until the
//! process exits.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::mpsc;
use std::time::SystemTime;

use calcita_core::reading::Reading;
use calcita_core::recent::{Recent, RecentStore};
use calcita_core::zoom::ZoomMode;
use celestina_core::{file_uri, pathkey};
use celestina_settings::Follower;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};

use crate::appearance::{self, Values};

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
        #[qml_singleton]
        #[qproperty(
            bool,
            appearance_reduced_motion,
            cxx_name = "appearanceReducedMotion",
            READ,
            NOTIFY
        )]
        #[qproperty(
            f64,
            appearance_text_scale,
            cxx_name = "appearanceTextScale",
            READ,
            NOTIFY
        )]
        #[qproperty(bool, smoke_report, cxx_name = "smokeReport", READ, CONSTANT)]
        #[qproperty(QStringList, documents, READ, NOTIFY)]
        #[qproperty(QStringList, recents, READ, NOTIFY)]
        #[qproperty(QStringList, recent_names, cxx_name = "recentNames", READ, NOTIFY)]
        type CalcitaController = super::CalcitaControllerRust;

        /// A document was admitted: the window opens one for `key`.
        #[qsignal]
        fn open_document(self: Pin<&mut CalcitaController>, key: QString);

        /// `key` is already open: its window comes forward.
        #[qsignal]
        fn raise_document(self: Pin<&mut CalcitaController>, key: QString);

        /// Something to tell the person: `kind` is `error` or `info`;
        /// `origin` names the window that asked (a document's key, `main`),
        /// or is empty when no window did and the front one shows it.
        #[qsignal]
        fn notice(self: Pin<&mut CalcitaController>, kind: QString, text: QString, origin: QString);

        /// Opens the document `key` names (a pathkey), asked by `origin`.
        #[qinvokable]
        fn open_path(self: Pin<&mut CalcitaController>, key: &QString, origin: &QString);

        /// Opens each `file://` URI of a drop or of the file chooser, asked
        /// by `origin`; anything else is refused with a notice to it.
        #[qinvokable]
        fn open_dropped(self: Pin<&mut CalcitaController>, uris: &QStringList, origin: &QString);

        /// The window of `key` closed.
        #[qinvokable]
        fn close_document(self: Pin<&mut CalcitaController>, key: &QString);

        /// Remembers where `key` is being read; saved on the store's worker.
        #[qinvokable]
        fn remember(self: Pin<&mut CalcitaController>, key: &QString, page: i32, zoom: &QString);

        /// The page `key` was left at, 1 when it is not remembered.
        #[qinvokable]
        fn restored_page(self: &CalcitaController, key: &QString) -> i32;

        /// The zoom word `key` was left at (`width`, `page`, `free:<f>`).
        #[qinvokable]
        fn restored_zoom(self: &CalcitaController, key: &QString) -> QString;
    }

    impl cxx_qt::Threading for CalcitaController {}
    impl cxx_qt::Initialize for CalcitaController {}
}

/// `smokeReport` is `CALCITA_SMOKE_REPORT` set: `scripts/smoke.sh` asks the
/// window to print what it shows once it is up.
pub struct CalcitaControllerRust {
    appearance_reduced_motion: bool,
    appearance_text_scale: f64,
    smoke_report: bool,
    documents: QStringList,
    recents: QStringList,
    recent_names: QStringList,
    store: RecentStore,
    /// The stored list has been read: until then a save would overwrite it
    /// with only this session's documents.
    store_ready: bool,
    /// Held for the singleton's life; dropping it stops the follower.
    follower: Option<Follower>,
    /// The store worker's queue: each message is a whole store to write.
    saver: Option<mpsc::Sender<RecentStore>>,
}

impl Default for CalcitaControllerRust {
    fn default() -> Self {
        let initial = appearance::initial();
        Self {
            appearance_reduced_motion: initial.reduced_motion,
            appearance_text_scale: initial.text_scale,
            smoke_report: std::env::var_os("CALCITA_SMOKE_REPORT").is_some(),
            documents: QStringList::default(),
            recents: QStringList::default(),
            recent_names: QStringList::default(),
            store: RecentStore::default_file()
                .map(RecentStore::at)
                .unwrap_or_default(),
            store_ready: false,
            follower: None,
            saver: None,
        }
    }
}

impl cxx_qt::Initialize for qobject::CalcitaController {
    fn initialize(mut self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        let follower = appearance::follow(move |values| {
            let _ = qt.queue(move |controller: Pin<&mut qobject::CalcitaController>| {
                controller.apply_appearance(values);
            });
        });
        self.as_mut().rust_mut().follower = Some(follower);
        let qt = self.qt_thread();
        self.as_mut().rust_mut().saver = Some(start_store_worker(move |store| {
            let _ = qt.queue(move |controller: Pin<&mut qobject::CalcitaController>| {
                controller.apply_store(store);
            });
        }));
    }
}

/// The store worker: reads the store once, hands it to `loaded`, then writes
/// every store it is sent, the latest one winning when several wait.
fn start_store_worker(
    loaded: impl FnOnce(RecentStore) + Send + 'static,
) -> mpsc::Sender<RecentStore> {
    let (sender, receiver) = mpsc::channel::<RecentStore>();
    let spawned = std::thread::Builder::new()
        .name("calcita-recents".to_owned())
        .spawn(move || {
            match RecentStore::load() {
                Ok(store) => loaded(store),
                Err(error) => eprintln!("calcita: {error}"),
            }
            while let Ok(mut store) = receiver.recv() {
                while let Ok(newer) = receiver.try_recv() {
                    store = newer;
                }
                if let Err(error) = store.save() {
                    eprintln!("calcita: {error}");
                }
            }
        });
    if let Err(error) = spawned {
        eprintln!("calcita: the recent store worker did not start: {error}");
    }
    sender
}

/// Why a request to open names no document Calcita reads.
#[derive(Debug, PartialEq)]
pub enum Refusal {
    /// Not a local file path (a malformed key, a remote URI).
    NotAFile,
    /// A file whose name does not end in `.pdf`.
    NotAPdf,
}

impl Refusal {
    pub fn message_es(&self) -> &'static str {
        match self {
            Self::NotAFile => "Solo se pueden abrir archivos de este equipo.",
            Self::NotAPdf => "Calcita solo abre documentos PDF.",
        }
    }
}

/// One request to open, before the file is looked at.
pub enum Request {
    Key(String),
    Uri(String),
}

/// The path `request` names when it is one Calcita opens, judged by its
/// name and by whether it is a folder (`is_dir`, a stat the worker makes).
/// Reading the file belongs to QtPdf, whose failure the window shows.
pub fn admit(request: &Request, is_dir: impl Fn(&Path) -> bool) -> Result<PathBuf, Refusal> {
    let path = match request {
        Request::Key(key) => pathkey::decode(key).map_err(|_| Refusal::NotAFile)?,
        Request::Uri(uri) => file_uri::to_path(uri).map_err(|_| Refusal::NotAFile)?,
    };
    let is_pdf = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"));
    if is_pdf && !is_dir(&path) {
        Ok(path)
    } else {
        Err(Refusal::NotAPdf)
    }
}

/// The name a window and the recents show for `path`.
pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.to_string_lossy(), |name| name.to_string_lossy())
        .into_owned()
}

impl qobject::CalcitaController {
    fn apply_appearance(mut self: Pin<&mut Self>, values: Values) {
        if self.rust().appearance_reduced_motion != values.reduced_motion {
            self.as_mut().rust_mut().appearance_reduced_motion = values.reduced_motion;
            self.as_mut().appearance_reduced_motion_changed();
        }
        if self.rust().appearance_text_scale != values.text_scale {
            self.as_mut().rust_mut().appearance_text_scale = values.text_scale;
            self.as_mut().appearance_text_scale_changed();
        }
    }

    /// The loaded store arrives; entries remembered since then stay first.
    fn apply_store(mut self: Pin<&mut Self>, loaded: RecentStore) {
        let mut merged = loaded;
        let touched = !self.rust().store.entries().is_empty();
        for entry in self.rust().store.entries().iter().rev() {
            merged.touch(entry.clone());
        }
        self.as_mut().rust_mut().store = merged;
        self.as_mut().rust_mut().store_ready = true;
        if touched {
            self.as_mut().send_store();
        }
        self.as_mut().publish_recents();
    }

    fn send_store(self: Pin<&mut Self>) {
        if let Some(saver) = self.rust().saver.as_ref() {
            let _ = saver.send(self.rust().store.clone());
        }
    }

    fn publish_recents(mut self: Pin<&mut Self>) {
        let entries = self.rust().store.entries();
        let keys: QStringList = entries
            .iter()
            .map(|entry| QString::from(pathkey::encode(&entry.path).as_str()))
            .collect();
        let names: QStringList = entries
            .iter()
            .map(|entry| QString::from(display_name(&entry.path).as_str()))
            .collect();
        self.as_mut().rust_mut().recents = keys;
        self.as_mut().rust_mut().recent_names = names;
        self.as_mut().recents_changed();
        self.as_mut().recent_names_changed();
    }

    fn refuse(self: Pin<&mut Self>, refusal: &Refusal, origin: QString) {
        self.notice(
            QString::from("error"),
            QString::from(refusal.message_es()),
            origin,
        );
    }

    /// Judges `requests` on a worker (the folder check is a stat) and
    /// answers on the Qt thread, in order.
    fn judge(self: Pin<&mut Self>, requests: Vec<Request>, origin: &QString) {
        let qt = self.qt_thread();
        let origin = origin.to_string();
        let spawned = std::thread::Builder::new()
            .name("calcita-admit".to_owned())
            .spawn(move || {
                let verdicts: Vec<Result<PathBuf, Refusal>> = requests
                    .iter()
                    .map(|request| admit(request, Path::is_dir))
                    .collect();
                let _ = qt.queue(
                    move |mut controller: Pin<&mut qobject::CalcitaController>| {
                        for verdict in verdicts {
                            match verdict {
                                Ok(path) => controller.as_mut().admit(&path),
                                Err(refusal) => controller
                                    .as_mut()
                                    .refuse(&refusal, QString::from(origin.as_str())),
                            }
                        }
                    },
                );
            });
        if let Err(error) = spawned {
            eprintln!("calcita: the admission worker did not start: {error}");
        }
    }

    fn admit(mut self: Pin<&mut Self>, path: &Path) {
        let key = QString::from(pathkey::encode(path).as_str());
        if self.rust().documents.iter().any(|open| *open == key) {
            self.raise_document(key);
            return;
        }
        self.as_mut().rust_mut().documents.append(key.clone());
        self.as_mut().documents_changed();
        self.open_document(key);
    }

    pub fn open_path(self: Pin<&mut Self>, key: &QString, origin: &QString) {
        self.judge(vec![Request::Key(key.to_string())], origin);
    }

    pub fn open_dropped(self: Pin<&mut Self>, uris: &QStringList, origin: &QString) {
        let requests = uris
            .iter()
            .map(|uri| Request::Uri(uri.to_string()))
            .collect();
        self.judge(requests, origin);
    }

    pub fn close_document(mut self: Pin<&mut Self>, key: &QString) {
        let index = self.rust().documents.iter().position(|open| open == key);
        if let Some(index) = index.and_then(|index| isize::try_from(index).ok()) {
            self.as_mut().rust_mut().documents.remove(index);
            self.as_mut().documents_changed();
        }
    }

    pub fn remember(mut self: Pin<&mut Self>, key: &QString, page: i32, zoom: &QString) {
        let Ok(path) = pathkey::decode(&key.to_string()) else {
            return;
        };
        let reading = Reading {
            page: u32::try_from(page).unwrap_or(1).max(1),
            zoom: ZoomMode::parse(&zoom.to_string()).unwrap_or(ZoomMode::FitWidth),
        };
        self.as_mut().rust_mut().store.touch(Recent {
            path,
            page: reading.page,
            zoom: reading.zoom,
            opened_at: SystemTime::now(),
        });
        if self.rust().store_ready {
            self.as_mut().send_store();
        }
        self.publish_recents();
    }

    fn reading_for(&self, key: &QString) -> Reading {
        pathkey::decode(&key.to_string())
            .ok()
            .and_then(|path| self.rust().store.reading_for(&path))
            .unwrap_or_default()
    }

    pub fn restored_page(&self, key: &QString) -> i32 {
        i32::try_from(self.reading_for(key).page).unwrap_or(1)
    }

    pub fn restored_zoom(&self, key: &QString) -> QString {
        QString::from(self.reading_for(key).zoom.to_word().as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{admit, display_name, Refusal, Request};
    use std::path::{Path, PathBuf};

    fn no_dir(_: &Path) -> bool {
        false
    }

    #[test]
    fn a_pdf_key_is_admitted_byte_exact() {
        use std::os::unix::ffi::OsStrExt;
        let expected = PathBuf::from(std::ffi::OsStr::from_bytes(b"/tmp/informe\xff.PDF"));
        let request = Request::Key("/tmp/informe%FF.PDF".to_owned());
        assert_eq!(admit(&request, no_dir), Ok(expected));
    }

    #[test]
    fn a_dropped_non_pdf_is_refused() {
        let uri = |text: &str| Request::Uri(text.to_owned());
        assert_eq!(
            admit(&uri("file:///tmp/foto.png"), no_dir),
            Err(Refusal::NotAPdf)
        );
        assert_eq!(
            admit(&uri("https://example.org/a.pdf"), no_dir),
            Err(Refusal::NotAFile)
        );
        assert_eq!(
            admit(&uri("file:///tmp/a%20b.pdf"), no_dir),
            Ok(PathBuf::from("/tmp/a b.pdf"))
        );
    }

    #[test]
    fn a_folder_named_like_a_pdf_is_refused() {
        let request = Request::Key("/tmp/carpeta.pdf".to_owned());
        assert_eq!(admit(&request, |_| true), Err(Refusal::NotAPdf));
    }

    #[test]
    fn the_name_is_the_last_component() {
        assert_eq!(display_name(Path::new("/tmp/informe.pdf")), "informe.pdf");
    }
}
