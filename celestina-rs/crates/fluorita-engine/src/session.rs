//! One playback session, translated into reports the core will believe.
//!
//! The whole point of this module is that it never asserts anything the backend
//! did not say. A `Play` request sets a property and returns; the session only
//! reports `Playing` when the backend's own `pause` property changes. A seek is
//! reported as completed when the backend restarts playback at the new
//! position, not when the command is accepted.
//!
//! Every report carries the generation the session was opened with, so a report
//! that arrives after the user selected something else is rejected upstream by
//! `fluorita-core` instead of overwriting newer state.

use std::time::Duration;

use celestina_core::Generation;
use fluorita_core::{EngineReport, PlaybackRequest, PlaybackState, ReportKind};
use libmpv2::events::{Event, PropertyData};
use libmpv2::{mpv_end_file_reason, EndFileReason, Format, Mpv};

use crate::backend::{AudioOutput, EngineSession, RenderHandle, SessionRequest, VideoOutput};
use crate::error::{EngineError, EngineResult};
use crate::instance::Instance;
use crate::source::SourceHandle;

/// mpv expresses volume as a percentage; the suite uses `0.0..=1.0`.
const VOLUME_SCALE: f64 = 100.0;

pub struct MpvSession {
    instance: Instance,
    client: Mpv,
    generation: Generation,
    /// The source, kept for the whole session: `start` loads from it, and a
    /// `fd://` source is only valid while its file stays open.
    source: SourceHandle,
    seek_in_flight: bool,
    closed: bool,
    started: bool,
}

impl MpvSession {
    pub fn open(request: SessionRequest) -> EngineResult<Self> {
        let handle = SourceHandle::open(&request.source)?;
        let video_output = if request.hardware_decoding {
            "auto-safe"
        } else {
            "no"
        };
        let mut options: Vec<(&str, &str)> = vec![
            // `libmpv` is the render-API output: the backend decodes and hands
            // frames to whatever surface drives its render context, and draws
            // no window of its own. Without a surface asking for it, a session
            // decodes blind — which is what audio and every test want, and the
            // only mode that works where there is no GPU context at all.
            (
                "vo",
                match request.video_output {
                    VideoOutput::Embedded => "libmpv",
                    VideoOutput::None => "null",
                },
            ),
            ("hwdec", video_output),
            ("pause", if request.start_paused { "yes" } else { "no" }),
            ("keep-open", "yes"),
        ];
        // Silence is an explicit driver; *sound* is the backend's own probe.
        // There is no driver called "auto": naming one leaves the session with
        // no audio at all, and a cover-art-only file then reaches end of file
        // immediately and sits paused at zero — which is exactly how this was
        // found.
        if matches!(request.audio_output, AudioOutput::Silent) {
            options.push(("ao", "null"));
        }
        let instance = Instance::new(&options)?;
        let client = instance.client()?;

        for (name, format) in [
            ("time-pos", Format::Double),
            ("duration", Format::Double),
            ("pause", Format::Flag),
            ("volume", Format::Double),
        ] {
            client
                .observe_property(name, format, 0)
                .map_err(|source| EngineError::Backend {
                    operation: "observe a playback property",
                    source,
                })?;
        }

        if let Some(level) = request.initial_volume {
            instance.set("volume", &format!("{}", level * VOLUME_SCALE))?;
        }

        Ok(Self {
            instance,
            client,
            generation: request.generation,
            source: handle,
            seek_in_flight: false,
            closed: false,
            started: false,
        })
    }

    fn translate(
        instance: &Instance,
        seek_in_flight: &mut bool,
        event: &Event<'_>,
    ) -> Option<ReportKind> {
        match event {
            Event::PropertyChange { name, change, .. } => translate_property(name, change),
            // A seek is only complete when the backend restarts playback at the
            // new position; reporting it earlier would move the playhead in the
            // UI before the decoder agreed.
            Event::PlaybackRestart if *seek_in_flight => {
                *seek_in_flight = false;
                Some(ReportKind::SeekCompleted(position_of(instance)))
            }
            Event::Seek => {
                *seek_in_flight = true;
                None
            }
            Event::EndFile(reason) => Some(end_of_file(*reason)),
            Event::Shutdown => Some(ReportKind::Failed(
                "el motor multimedia se cerró".to_owned(),
            )),
            _ => None,
        }
    }
}

/// The last position the backend reported, never a requested one.
fn position_of(instance: &Instance) -> Duration {
    instance
        .optional_f64("time-pos")
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map_or(Duration::ZERO, Duration::from_secs_f64)
}

fn translate_property(name: &str, change: &PropertyData<'_>) -> Option<ReportKind> {
    {
        match (name, change) {
            ("time-pos", PropertyData::Double(value)) if value.is_finite() && *value >= 0.0 => {
                Some(ReportKind::Position(Duration::from_secs_f64(*value)))
            }
            ("duration", PropertyData::Double(value)) if value.is_finite() && *value > 0.0 => {
                Some(ReportKind::Duration(Duration::from_secs_f64(*value)))
            }
            ("pause", PropertyData::Flag(paused)) => Some(ReportKind::State(if *paused {
                PlaybackState::Paused
            } else {
                PlaybackState::Playing
            })),
            ("volume", PropertyData::Double(value)) if value.is_finite() => {
                Some(ReportKind::Volume((*value / VOLUME_SCALE).clamp(0.0, 1.0)))
            }
            _ => None,
        }
    }
}

/// mpv distinguishes "the file ended" from "the file broke"; so must the UI.
///
/// `EndFileReason` is the C enum's integer, not a Rust enum, so this compares
/// against the binding's constants instead of matching variants — and an
/// unknown future reason is reported as a failure rather than silently read as
/// a clean end.
fn end_of_file(reason: EndFileReason) -> ReportKind {
    if reason == mpv_end_file_reason::Eof {
        ReportKind::State(PlaybackState::Ended)
    } else if reason == mpv_end_file_reason::Stop || reason == mpv_end_file_reason::Quit {
        ReportKind::State(PlaybackState::Idle)
    } else if reason == mpv_end_file_reason::Redirect {
        ReportKind::Failed("el archivo redirige a otra fuente, que esta sesión no sigue".to_owned())
    } else {
        ReportKind::Failed("no se pudo reproducir este archivo".to_owned())
    }
}

impl EngineSession for MpvSession {
    fn generation(&self) -> Generation {
        self.generation
    }

    fn start(&mut self) -> EngineResult<()> {
        if self.closed {
            return Err(EngineError::WorkerStopped);
        }
        if self.started {
            return Ok(());
        }
        self.started = true;
        self.instance.load(&self.source)
    }

    fn render_handle(&self) -> Option<RenderHandle> {
        // A closed session reports nothing: a surface that kept rendering into
        // a context being torn down is the one crash this seam can cause.
        (!self.closed).then(|| self.instance.render_handle())
    }

    fn request(&mut self, request: PlaybackRequest) -> EngineResult<()> {
        if self.closed {
            return Err(EngineError::WorkerStopped);
        }
        match request {
            PlaybackRequest::Play => self.instance.set("pause", "no"),
            PlaybackRequest::Pause => self.instance.set("pause", "yes"),
            PlaybackRequest::Stop => self.instance.command("stop", &[]),
            PlaybackRequest::Seek(target) => {
                self.seek_in_flight = true;
                self.instance.command(
                    "seek",
                    &[&format!("{}", target.as_secs_f64()), "absolute", "exact"],
                )
            }
            PlaybackRequest::SetVolume(level) => self.instance.set(
                "volume",
                &format!("{}", level.clamp(0.0, 1.0) * VOLUME_SCALE),
            ),
        }
    }

    fn poll(&mut self, timeout: Duration) -> Option<EngineReport> {
        if self.closed {
            return None;
        }
        let Self {
            instance,
            client,
            generation,
            seek_in_flight,
            ..
        } = self;
        let event = client.wait_event(timeout.as_secs_f64())?;
        let kind = match event {
            Ok(event) => Self::translate(instance, seek_in_flight, &event)?,
            Err(error) => ReportKind::Failed(format!("el motor multimedia falló: {error}")),
        };
        Some(EngineReport {
            generation: *generation,
            kind,
        })
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        // Best effort: the instance is dropped either way, and a backend that
        // already died must not turn closing into an error the host handles.
        let _ = self.instance.command("stop", &[]);
    }
}

impl Drop for MpvSession {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::{end_of_file, VOLUME_SCALE};
    use fluorita_core::{PlaybackState, ReportKind};
    use libmpv2::mpv_end_file_reason;

    #[test]
    fn the_end_of_a_file_is_not_the_same_as_a_broken_file() {
        assert_eq!(
            end_of_file(mpv_end_file_reason::Eof),
            ReportKind::State(PlaybackState::Ended)
        );
        assert!(matches!(
            end_of_file(mpv_end_file_reason::Error),
            ReportKind::Failed(_)
        ));
        assert_eq!(
            end_of_file(mpv_end_file_reason::Stop),
            ReportKind::State(PlaybackState::Idle)
        );
    }

    #[test]
    fn volume_is_scaled_between_the_two_conventions() {
        assert!((0.8_f64 * VOLUME_SCALE - 80.0).abs() < f64::EPSILON);
        assert!((80.0 / VOLUME_SCALE - 0.8).abs() < f64::EPSILON);
    }
}
