//! The Audio section's commands and card profiles. Endpoints and streams are
//! `EndpointModel` and `StreamModel`, fed through `AUDIO`.

use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QMap, QMapPair_QString_QVariant, QString, QVariant};

use cuprita_core::audio::Audio;
use cuprita_core::error::AudioError;
use cuprita_core::model::{AudioSnapshot, CardProfile};
use cuprita_core::volume::clamp_volume;

use super::notice::NoticeKind;
use super::{Hub, Report, Worker};
use crate::backend;

/// The trait object the worker drives.
type Backend = dyn Audio + 'static;

/// The audio snapshots the endpoint and stream models subscribe to.
pub static AUDIO: Hub<AudioSnapshot> = Hub::new();

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        // profiles — a list of `{ cardId, id, description, active }`, only for
        // cards with more than one profile
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(QVariant, profiles, READ, NOTIFY)]
        #[qproperty(bool, busy, READ, NOTIFY)]
        // False until the first snapshot arrives; true from then on.
        #[qproperty(bool, loaded, READ, NOTIFY)]
        type AudioController = super::AudioControllerRust;

        #[qsignal]
        fn notice(self: Pin<&mut AudioController>, kind: QString, text: QString);

        #[qinvokable]
        fn set_default(self: Pin<&mut AudioController>, id: u32);
        /// `id` is an endpoint or a stream; `volume` is linear, 0..1.5.
        #[qinvokable]
        fn set_volume(self: Pin<&mut AudioController>, id: u32, volume: f64);
        #[qinvokable]
        fn set_muted(self: Pin<&mut AudioController>, id: u32, muted: bool);
        #[qinvokable]
        fn set_profile(self: Pin<&mut AudioController>, card_id: u32, profile: &QString);
    }

    impl cxx_qt::Threading for AudioController {}
    impl cxx_qt::Initialize for AudioController {}
}

pub struct AudioControllerRust {
    profiles: QVariant,
    busy: bool,
    loaded: bool,
    pending: u32,
    shown_profiles: Vec<CardProfile>,
    worker: Option<Worker<Backend>>,
}

impl Default for AudioControllerRust {
    fn default() -> Self {
        Self {
            // An empty list, not an invalid variant: QML reads `.length`.
            profiles: profile_list(&[]),
            busy: false,
            loaded: false,
            pending: 0,
            shown_profiles: Vec::new(),
            worker: None,
        }
    }
}

impl cxx_qt::Initialize for qobject::AudioController {
    fn initialize(mut self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        // No poll: the backend's watcher asks for every re-read.
        let worker = Worker::start(
            move |refresh| {
                backend::audio(move || {
                    refresh.request();
                })
            },
            None,
            |backend: &mut Backend| backend.snapshot().map_err(|e| e.message_es()),
            move |report| {
                let _ = qt.queue(move |controller: Pin<&mut qobject::AudioController>| {
                    controller.apply(report);
                });
            },
        );
        self.as_mut().rust_mut().worker = worker;
    }
}

/// The profiles worth a choice: those of cards that have more than one.
fn choosable(profiles: &[CardProfile]) -> Vec<CardProfile> {
    profiles
        .iter()
        .filter(|p| profiles.iter().filter(|q| q.card_id == p.card_id).count() > 1)
        .cloned()
        .collect()
}

fn profile_list(profiles: &[CardProfile]) -> QVariant {
    let mut list = QList::<QVariant>::default();
    for p in profiles {
        let mut map = QMap::<QMapPair_QString_QVariant>::default();
        map.insert(QString::from("cardId"), QVariant::from(&p.card_id));
        map.insert(
            QString::from("id"),
            QVariant::from(&QString::from(p.id.as_str())),
        );
        map.insert(
            QString::from("description"),
            QVariant::from(&QString::from(p.description.as_str())),
        );
        map.insert(QString::from("active"), QVariant::from(&p.active));
        list.append(QVariant::from(&map));
    }
    QVariant::from(&list)
}

impl qobject::AudioController {
    fn apply(mut self: Pin<&mut Self>, report: Report<AudioSnapshot>) {
        match report {
            Report::Snapshot(snapshot) => {
                let profiles = choosable(&snapshot.profiles);
                if self.rust().shown_profiles != profiles {
                    self.as_mut().rust_mut().profiles = profile_list(&profiles);
                    self.as_mut().rust_mut().shown_profiles = profiles;
                    self.as_mut().profiles_changed();
                }
                AUDIO.publish(Arc::clone(&snapshot));
                if !self.rust().loaded {
                    // The models queued their copy of this snapshot just now,
                    // on this same thread: flipping `loaded` from a call queued
                    // after theirs means no frame sees it true with lists
                    // still empty.
                    let queued = self.qt_thread().queue(
                        |mut controller: Pin<&mut qobject::AudioController>| {
                            if !controller.rust().loaded {
                                controller.as_mut().rust_mut().loaded = true;
                                controller.as_mut().loaded_changed();
                            }
                        },
                    );
                    if queued.is_err() {
                        eprintln!("Cuprita: the loaded flag could not be queued");
                    }
                }
            }
            Report::Failed(text) => self.as_mut().notice(
                QString::from(NoticeKind::Error.token()),
                QString::from(text.as_str()),
            ),
            Report::Done => {
                let pending = self.rust().pending.saturating_sub(1);
                self.as_mut().rust_mut().pending = pending;
                self.as_mut().set_busy(pending > 0);
            }
        }
    }

    fn set_busy(mut self: Pin<&mut Self>, busy: bool) {
        if self.rust().busy != busy {
            self.as_mut().rust_mut().busy = busy;
            self.busy_changed();
        }
    }

    fn dispatch(
        self: Pin<&mut Self>,
        job: impl FnOnce(&mut Backend) -> Result<(), AudioError> + Send + 'static,
    ) {
        self.dispatch_keyed(None, job);
    }

    /// `key`, when set, lets a later command with the same key replace this
    /// one while both still wait (a slider's writes).
    fn dispatch_keyed(
        mut self: Pin<&mut Self>,
        key: Option<String>,
        job: impl FnOnce(&mut Backend) -> Result<(), AudioError> + Send + 'static,
    ) {
        let job = move |b: &mut Backend| job(b).map_err(|e| e.message_es());
        let queued = self.rust().worker.as_ref().is_some_and(|w| match key {
            Some(key) => w.run_keyed(key, job),
            None => w.run(job),
        });
        if queued {
            let pending = self.rust().pending + 1;
            self.as_mut().rust_mut().pending = pending;
            self.as_mut().set_busy(true);
        } else {
            self.as_mut().notice(
                QString::from(NoticeKind::Error.token()),
                QString::from(AudioError::Unavailable.message_es().as_str()),
            );
        }
    }

    pub fn set_default(self: Pin<&mut Self>, id: u32) {
        self.dispatch(move |b| b.set_default(id));
    }

    pub fn set_volume(self: Pin<&mut Self>, id: u32, volume: f64) {
        // f64 from QML narrowed to the domain's f32, then clamped.
        #[allow(clippy::cast_possible_truncation)]
        let volume = clamp_volume(volume as f32);
        self.dispatch_keyed(Some(format!("volume:{id}")), move |b| {
            b.set_volume(id, volume)
        });
    }

    pub fn set_muted(self: Pin<&mut Self>, id: u32, muted: bool) {
        self.dispatch(move |b| b.set_muted(id, muted));
    }

    pub fn set_profile(self: Pin<&mut Self>, card_id: u32, profile: &QString) {
        let profile = profile.to_string();
        self.dispatch(move |b| b.set_profile(card_id, &profile));
    }
}

#[cfg(test)]
mod tests {
    use super::choosable;
    use cuprita_core::fake::FakeAudio;

    #[test]
    fn a_card_with_two_profiles_is_choosable() {
        let profiles = FakeAudio::scripted().state.profiles;
        assert_eq!(choosable(&profiles).len(), 2);
        assert!(choosable(&profiles[..1]).is_empty());
    }
}
