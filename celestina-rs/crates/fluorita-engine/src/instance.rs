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

/// Options every instance gets, whatever it is for.
const BASELINE: &[(&str, &str)] = &[
    // Determinism: the engine must not inherit the user's mpv configuration.
    ("config", "no"),
    ("load-scripts", "no"),
    // Fluorita owns every pixel of chrome and every key. `load-scripts` only
    // covers the user's own; mpv also ships built-in Lua — an on-screen
    // console, a stats overlay, a track selector — which draw over the app and
    // cost a thread each. They are all off.
    ("osc", "no"),
    ("load-console", "no"),
    ("load-context-menu", "no"),
    ("load-stats-overlay", "no"),
    ("load-auto-profiles", "no"),
    ("load-select", "no"),
    ("load-positioning", "no"),
    ("load-commands", "no"),
    ("osd-level", "0"),
    ("input-default-bindings", "no"),
    ("input-vo-keyboard", "no"),
    ("terminal", "no"),
    // Hostile input: never follow a playlist or a network reference out of the
    // file the user asked for.
    ("load-unsafe-playlists", "no"),
    ("ytdl", "no"),
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
        let owned: Vec<(String, String)> = BASELINE
            .iter()
            .chain(options.iter())
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();

        let mpv = Mpv::with_initializer(|init| {
            for (name, value) in &owned {
                init.set_property(name, value.as_str())?;
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
    use super::{Instance, BASELINE};

    #[test]
    fn the_baseline_refuses_user_configuration_and_chrome() {
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
                BASELINE.contains(&required),
                "missing baseline option: {required:?}"
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
    fn a_rejected_option_is_a_typed_backend_failure() {
        let error = Instance::new(&[("no-such-option-at-all", "1")])
            .expect_err("mpv rejects unknown options");

        assert!(error.to_string().contains("backend"));
    }
}
