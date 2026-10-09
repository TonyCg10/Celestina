//! language-contract: product-copy
//! The appearance section: reduced motion and the text size, the one place in
//! the suite where the shared appearance file is edited.
//!
//! Every read and write runs on this section's worker, never on the Qt
//! thread: a change reads the file as stored (without the environment
//! override, `celestina_settings::load_stored`), sets the one value and saves both
//! (`celestina_settings::save`), and the watcher re-reads after any write,
//! from here or from elsewhere. The window itself follows the file through
//! `CupritaController`, so a change here previews in Cuprita as in every
//! other open window. While `CELESTINA_REDUCED_MOTION` is set, reduced motion
//! is forced on and the page shows the switch disabled.

use std::pin::Pin;

use celestina_settings::{Appearance, SettingsError, TextScale};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use super::notice::NoticeKind;
use super::{Report, Worker};
use crate::backend::{self, AppearanceStore};

/// The trait object the worker drives.
type Backend = dyn AppearanceStore + 'static;

/// The notice when the appearance could not be saved.
const NOT_SAVED: &str = "No se pudo guardar la apariencia";

/// What the notice says when the file cannot be written.
fn message_es(error: &SettingsError) -> String {
    match error {
        SettingsError::NoConfigHome => {
            "No hay carpeta de configuración donde guardar la apariencia".to_owned()
        }
        _ => NOT_SAVED.to_owned(),
    }
}

/// Reads the stored appearance (never the environment's forced reduced
/// motion), changes it with `change` and saves it.
fn update(
    change: impl FnOnce(&mut Appearance) + Send + 'static,
) -> impl FnOnce(&mut Backend) -> Result<(), String> + Send + 'static {
    move |store: &mut Backend| {
        let mut value = store.read_stored();
        change(&mut value);
        store.write(&value).map_err(|error| {
            // The detail goes to the log; the notice stays a sentence.
            eprintln!("Cuprita: the appearance was not saved: {error}");
            message_es(&error)
        })
    }
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, reduced_motion, READ, NOTIFY)]
        /// `compact`, `normal`, `large` or `larger`.
        #[qproperty(QString, text_scale, READ, NOTIFY)]
        /// `CELESTINA_REDUCED_MOTION` is set: reduced motion stays on.
        #[qproperty(bool, forced_by_environment, READ, CONSTANT)]
        // False until the first reading arrives; true from then on.
        #[qproperty(bool, loaded, READ, NOTIFY)]
        type AppearanceController = super::AppearanceControllerRust;

        #[qsignal]
        fn notice(self: Pin<&mut AppearanceController>, kind: QString, text: QString);

        #[qinvokable]
        fn set_reduced_motion(self: Pin<&mut AppearanceController>, on: bool);
        #[qinvokable]
        fn set_text_scale(self: Pin<&mut AppearanceController>, name: &QString);
    }

    impl cxx_qt::Threading for AppearanceController {}
    impl cxx_qt::Initialize for AppearanceController {}
}

pub struct AppearanceControllerRust {
    reduced_motion: bool,
    text_scale: QString,
    forced_by_environment: bool,
    loaded: bool,
    worker: Option<Worker<Backend>>,
}

impl Default for AppearanceControllerRust {
    fn default() -> Self {
        Self {
            reduced_motion: false,
            text_scale: QString::from(TextScale::Normal.as_str()),
            // An environment read, not IO.
            forced_by_environment: celestina_settings::env_forces_reduced_motion(),
            loaded: false,
            worker: None,
        }
    }
}

impl cxx_qt::Initialize for qobject::AppearanceController {
    fn initialize(mut self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        // No poll: the file's watcher asks for every re-read.
        let worker = Worker::start(
            move |refresh| {
                backend::appearance(move || {
                    refresh.request();
                })
            },
            None,
            |store: &mut Backend| Ok(store.read()),
            move |report| {
                let _ = qt.queue(move |controller: Pin<&mut qobject::AppearanceController>| {
                    controller.apply(report);
                });
            },
        );
        self.as_mut().rust_mut().worker = worker;
    }
}

impl qobject::AppearanceController {
    fn apply(mut self: Pin<&mut Self>, report: Report<Appearance>) {
        match report {
            Report::Snapshot(value) => {
                if self.rust().reduced_motion != value.reduced_motion {
                    self.as_mut().rust_mut().reduced_motion = value.reduced_motion;
                    self.as_mut().reduced_motion_changed();
                }
                let scale = QString::from(value.text_scale.as_str());
                if self.rust().text_scale != scale {
                    self.as_mut().rust_mut().text_scale = scale;
                    self.as_mut().text_scale_changed();
                }
                if !self.rust().loaded {
                    self.as_mut().rust_mut().loaded = true;
                    self.as_mut().loaded_changed();
                }
            }
            Report::Failed(text) => self.as_mut().notice(
                QString::from(NoticeKind::Error.token()),
                QString::from(text.as_str()),
            ),
            Report::Done => {}
        }
    }

    fn dispatch(mut self: Pin<&mut Self>, change: impl FnOnce(&mut Appearance) + Send + 'static) {
        let queued = self
            .rust()
            .worker
            .as_ref()
            .is_some_and(|worker| worker.run(update(change)));
        if !queued {
            self.as_mut().notice(
                QString::from(NoticeKind::Error.token()),
                QString::from(NOT_SAVED),
            );
        }
    }

    pub fn set_reduced_motion(self: Pin<&mut Self>, on: bool) {
        // Forced on by the environment: the file's value would not show.
        if self.rust().forced_by_environment {
            return;
        }
        self.dispatch(move |value| value.reduced_motion = on);
    }

    pub fn set_text_scale(self: Pin<&mut Self>, name: &QString) {
        let Some(scale) = TextScale::from_name(&name.to_string()) else {
            eprintln!("Cuprita: unknown text scale {name}");
            return;
        };
        self.dispatch(move |value| value.text_scale = scale);
    }
}

#[cfg(test)]
mod tests {
    use celestina_settings::{Appearance, TextScale};

    use super::update;
    use crate::backend::AppearanceStore;

    struct Memory {
        value: Appearance,
        /// Stands for `CELESTINA_REDUCED_MOTION`: `read` reports reduced
        /// motion on, `read_stored` the file's value.
        forced: bool,
    }

    impl AppearanceStore for Memory {
        fn read(&mut self) -> Appearance {
            let mut value = self.value;
            if self.forced {
                value.reduced_motion = true;
            }
            value
        }

        fn read_stored(&mut self) -> Appearance {
            self.value
        }

        fn write(&mut self, value: &Appearance) -> Result<(), celestina_settings::SettingsError> {
            self.value = *value;
            Ok(())
        }
    }

    #[test]
    fn a_change_keeps_the_other_value() {
        let mut store = Memory {
            value: Appearance {
                reduced_motion: true,
                text_scale: TextScale::Normal,
            },
            forced: false,
        };
        update(|value| value.text_scale = TextScale::Large)(&mut store).expect("saved");
        assert_eq!(
            store.value,
            Appearance {
                reduced_motion: true,
                text_scale: TextScale::Large,
            }
        );
        update(|value| value.reduced_motion = false)(&mut store).expect("saved");
        assert_eq!(store.value.text_scale, TextScale::Large);
        assert!(!store.value.reduced_motion);
    }

    #[test]
    fn a_forced_reduced_motion_is_never_saved() {
        let mut store = Memory {
            value: Appearance {
                reduced_motion: false,
                text_scale: TextScale::Normal,
            },
            forced: true,
        };
        assert!(store.read().reduced_motion);
        update(|value| value.text_scale = TextScale::Large)(&mut store).expect("saved");
        assert_eq!(
            store.value,
            Appearance {
                reduced_motion: false,
                text_scale: TextScale::Large,
            }
        );
    }
}
