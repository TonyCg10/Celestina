//! Creating a backend instance the suite can trust.
//!
//! Every instance starts from the same deliberately boring baseline: no user
//! config, no scripts, no on-screen controller, no key bindings, no terminal.
//! Fluorita draws its own interface, so anything the backend would draw or
//! bind is a defect, and reading `~/.config/mpv` would make one machine behave
//! unlike another.

use std::path::Path;
use std::time::{Duration, Instant};

use libmpv2::events::Event;
use libmpv2::{EndFileReason, Mpv};

use crate::backend::RenderHandle;
use crate::error::{EngineError, EngineResult};
use crate::source::SourceHandle;

/// Options every instance gets, whatever it is for. Their absence is a
/// determinism or safety hole, so a rejection aborts creating the instance
/// entirely — see [`Instance::new`].
const REQUIRED_BASELINE: &[(&str, &str)] = &[
    // Determinism: the engine must not inherit the user's mpv configuration.
    ("config", "no"),
    ("load-scripts", "no"),
    ("osc", "no"),
    ("osd-level", "0"),
    ("input-default-bindings", "no"),
    ("input-vo-keyboard", "no"),
    ("terminal", "no"),
    // Hostile input: never follow a playlist or a network reference out of the
    // file the user asked for.
    ("load-unsafe-playlists", "no"),
    ("ytdl", "no"),
];

/// Fluorita owns every pixel of chrome and every key. `load-scripts` above
/// only covers the user's own scripts; mpv also ships built-in Lua — an
/// on-screen console, a stats overlay, a track selector — which would draw
/// over the app and cost a thread each. This list turns each one off.
///
/// Applied best-effort, unlike the required baseline: these seven names are
/// newer mpv properties (absent from mpv 0.37, the version Ubuntu 24.04 ships
/// and what CI's runner installs from apt). One of these being unknown to an
/// older `libmpv` must degrade — this app's chrome might get an overlay drawn
/// on it by mpv's own script, same as a D-Bus failure degrades a service
/// rather than blocking it — never take down instance creation, and thus every
/// kind of playback, over a single missing hardening toggle.
const HARDENING: &[(&str, &str)] = &[
    ("load-console", "no"),
    ("load-context-menu", "no"),
    ("load-stats-overlay", "no"),
    ("load-auto-profiles", "no"),
    ("load-select", "no"),
    ("load-positioning", "no"),
    ("load-commands", "no"),
];

pub struct Instance {
    mpv: Mpv,
}

impl std::fmt::Debug for Instance {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The backend handle has no printable state; naming the type is enough
        // for a test assertion and avoids pretending there is more.
        formatter.write_str("Instance(<mpv handle>)")
    }
}

impl Instance {
    /// Builds an instance with the baseline plus `options`.
    pub fn new(options: &[(&str, &str)]) -> EngineResult<Self> {
        let required: Vec<(String, String)> = REQUIRED_BASELINE
            .iter()
            .chain(options.iter())
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();

        let mpv = Mpv::with_initializer(|init| {
            for (name, value) in &required {
                init.set_property(name, value.as_str())?;
            }
            // Best-effort: an unknown property here must not fail the whole
            // instance, only skip that one hardening toggle.
            for (name, value) in HARDENING {
                let _ = init.set_property(name, *value);
            }
            Ok(())
        })
        .map_err(|source| EngineError::BackendUnavailable { source })?;

        Ok(Self { mpv })
    }

    /// A second handle onto the same core, for receiving events while the first
    /// one issues commands.
    pub fn client(&self) -> EngineResult<Mpv> {
        let client = self
            .mpv
            .create_client(None)
            .map_err(|source| EngineError::Backend {
                operation: "create an event client",
                source,
            })?;
        client
            .disable_deprecated_events()
            .map_err(|source| EngineError::Backend {
                operation: "disable deprecated events",
                source,
            })?;
        Ok(client)
    }

    /// Starts loading `handle`, replacing whatever was playing.
    pub fn load(&self, handle: &SourceHandle) -> EngineResult<()> {
        self.mpv
            .command("loadfile", &[handle.url(), "replace"])
            .map_err(|source| EngineError::Backend {
                operation: "load the file",
                source,
            })
    }

    pub fn command(&self, name: &'static str, args: &[&str]) -> EngineResult<()> {
        self.mpv
            .command(name, args)
            .map_err(|source| EngineError::Backend {
                operation: name,
                source,
            })
    }

    pub fn set(&self, name: &'static str, value: &str) -> EngineResult<()> {
        self.mpv
            .set_property(name, value)
            .map_err(|source| EngineError::Backend {
                operation: name,
                source,
            })
    }

    /// The backend handle as an opaque address, for the one consumer that must
    /// have it: a host surface creating libmpv's render context on its own GPU
    /// thread. Taking the address is inert here — nothing in this crate may
    /// dereference it, and `forbid(unsafe_code)` guarantees it cannot.
    pub fn render_handle(&self) -> RenderHandle {
        RenderHandle::from_address(self.mpv.ctx.as_ptr() as usize)
    }

    /// A property that may legitimately be unavailable — most of them are,
    /// before the file is loaded or when the format simply has no such value.
    pub fn optional_f64(&self, name: &str) -> Option<f64> {
        self.mpv.get_property::<f64>(name).ok()
    }

    pub fn optional_i64(&self, name: &str) -> Option<i64> {
        self.mpv.get_property::<i64>(name).ok()
    }

    pub fn optional_bool(&self, name: &str) -> Option<bool> {
        self.mpv.get_property::<bool>(name).ok()
    }

    pub fn optional_string(&self, name: &str) -> Option<String> {
        self.mpv
            .get_property::<String>(name)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }
}

/// Why [`wait_for_load`] stopped waiting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadOutcome {
    Loaded,
    /// The backend gave up on the file; the reason is the backend's own.
    Ended(EndFileReason),
}

/// Waits until the file is loaded, the backend gives up, or the budget runs
/// out. Cancellation is checked between events so a queued job dies promptly.
pub fn wait_for_load(
    client: &Mpv,
    deadline: Duration,
    cancellation: &celestina_core::CancellationToken,
    path: &Path,
) -> EngineResult<LoadOutcome> {
    let started = Instant::now();
    while started.elapsed() < deadline {
        if cancellation.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        let remaining = deadline.saturating_sub(started.elapsed());
        match client.wait_event(remaining.as_secs_f64().min(0.25)) {
            Some(Ok(Event::FileLoaded)) => return Ok(LoadOutcome::Loaded),
            Some(Ok(Event::EndFile(reason))) => return Ok(LoadOutcome::Ended(reason)),
            Some(Ok(Event::Shutdown)) => {
                return Err(EngineError::Undecodable {
                    path: path.to_path_buf(),
                    detail: "the backend shut down while loading".to_owned(),
                })
            }
            Some(Err(source)) => {
                return Err(EngineError::Backend {
                    operation: "wait for the file to load",
                    source,
                })
            }
            _ => {}
        }
    }
    Err(EngineError::TimedOut {
        operation: "load",
        after: deadline,
    })
}

#[cfg(test)]
mod tests {
    use super::{Instance, HARDENING, REQUIRED_BASELINE};

    #[test]
    fn the_required_baseline_refuses_user_configuration_and_chrome() {
        // A regression here would mean the engine draws mpv's OSC over
        // Fluorita's own controls, or behaves differently on another machine.
        for required in [
            ("config", "no"),
            ("osc", "no"),
            ("input-default-bindings", "no"),
            ("load-scripts", "no"),
            ("ytdl", "no"),
        ] {
            assert!(
                REQUIRED_BASELINE.contains(&required),
                "missing required baseline option: {required:?}"
            );
        }
    }

    #[test]
    fn hardening_options_are_the_ones_absent_from_older_mpv() {
        // These are what mpv 0.37 (Ubuntu 24.04's apt package, which is what
        // CI's `ubuntu-latest` runner installs) does not know. Instance
        // creation must survive their absence rather than fail entirely.
        for toggle in [
            "load-console",
            "load-context-menu",
            "load-stats-overlay",
            "load-auto-profiles",
            "load-select",
            "load-positioning",
            "load-commands",
        ] {
            assert!(
                HARDENING.iter().any(|(name, _)| *name == toggle),
                "missing hardening toggle: {toggle}"
            );
            assert!(
                !REQUIRED_BASELINE.iter().any(|(name, _)| *name == toggle),
                "{toggle} must be best-effort, not required — an older mpv \
                 rejecting it must not fail every instance"
            );
        }
    }

    #[test]
    fn an_instance_starts_and_answers_properties() {
        let instance = Instance::new(&[("vo", "null"), ("ao", "null")])
            .expect("libmpv must be present to run the engine tests");

        // Nothing is loaded, so playback properties are legitimately absent.
        assert_eq!(instance.optional_f64("duration"), None);
        assert!(instance.client().is_ok());
    }

    #[test]
    fn a_rejected_required_option_is_a_typed_backend_failure() {
        // Passed as a per-call option, not a hardening toggle: this path must
        // still fail loudly, unlike the best-effort one above.
        let error = Instance::new(&[("no-such-option-at-all", "1")])
            .expect_err("mpv rejects unknown options");

        assert!(error.to_string().contains("backend"));
    }
}
