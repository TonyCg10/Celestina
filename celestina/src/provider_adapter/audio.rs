//! What the session is playing through, and whether it is silenced.

use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use celestina_shell_core::audio::{self, AudioLevel};
use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::session::{self, MuteDevice, SessionRequest, Switch};
use celestina_shell_core::snapshot::{Payload, ProviderId};
use serde_json::Value;

use super::tools::{launch, lock_runtime, run_bounded};

/// Volume changes are user-driven and rare, so the panel asks rarely. What
/// matters for how it feels is that the panel re-reads immediately after a
/// change it made itself, which it does.
const INTERVAL: Duration = Duration::from_secs(2);
/// The session's own ceiling: no overdrive, the same `-l 1.0` the keys already
/// pass to `wpctl`.
const VOLUME_CEILING: &str = "1.0";
/// The mixer the session already has. `qpwgraph` is a patchbay, not a mixer.
const EXTERNAL_MIXER: &str = "pavucontrol";
/// wpctl's names for "whatever the session is playing through / listening with".
const SINK: &str = "@DEFAULT_AUDIO_SINK@";
const SOURCE: &str = "@DEFAULT_AUDIO_SOURCE@";
/// Not a session verb: it starts a program instead of changing the session.
const OPEN_MIXER: &str = "open-mixer";

pub const NAME: &str = "audio";

pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>) -> io::Result<()> {
    let Ok(id) = ProviderId::new(NAME) else {
        eprintln!("celestina-provider-adapter: audio: unusable provider name");
        return Ok(());
    };

    lock_runtime(runtime).register(id.clone());
    let runtime = Arc::clone(runtime);
    thread::Builder::new()
        .name(NAME.to_owned())
        .spawn(move || run(&runtime, &id))?;
    Ok(())
}

/// wpctl takes a unit fraction. A whole percent is written out exactly, so
/// nothing this shell asks for passes through a float.
fn unit_fraction(percent: u8) -> String {
    format!("{}.{:02}", percent / 100, percent % 100)
}

fn device(which: MuteDevice) -> &'static str {
    match which {
        MuteDevice::Output => SINK,
        MuteDevice::Input => SOURCE,
    }
}

/// Serves the session's audio verbs, plus the one verb that is not a session
/// verb at all: opening the mixer is starting a program, not asking the
/// session to become something.
pub fn action(
    verb: &str,
    options: &Payload,
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
) -> Result<(), String> {
    if verb == OPEN_MIXER {
        return launch(EXTERNAL_MIXER);
    }

    let request = session::parse_for(NAME, verb, options)?;

    let level_target;
    let args: Vec<&str> = match request {
        SessionRequest::Volume(change) => {
            // The level the device is at is the only thing a step can be
            // relative to, and it is also what the panel will show next.
            let current = level(SINK)
                .ok_or_else(|| "wpctl reports no readable default audio device".to_owned())?;
            level_target = unit_fraction(change.applied_to(current.percent));
            vec!["set-volume", SINK, &level_target, "-l", VOLUME_CEILING]
        }
        SessionRequest::Mute(which, state) => vec![
            "set-mute",
            device(which),
            match state {
                Switch::On => "1",
                Switch::Off => "0",
                Switch::Toggle => "toggle",
            },
        ],
        // `parse_for` already refused everything this provider does not serve.
        _ => return Err(session::unserved_verb(NAME, verb)),
    };

    run_bounded("wpctl", &args).ok_or_else(|| format!("wpctl refused to {verb}"))?;
    // Read back at once rather than leaving the panel a poll behind its own
    // click: what it shows is still a reading, never the change it asked for.
    publish(runtime, id);
    Ok(())
}

/// One device's level, or `None` when wpctl cannot tell us about it.
fn level(device: &str) -> Option<AudioLevel> {
    audio::parse_wpctl_volume(&run_bounded("wpctl", &["get-volume", device])?)
}

/// Publishes the speaker and the microphone as one value, since a panel that
/// showed them a poll apart would be showing two different moments.
fn publish(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    let Some(sink) = level(SINK) else {
        // No readable default device is not a device at zero: the widget goes
        // away rather than claiming silence.
        lock_runtime(runtime).withdraw(id);
        return;
    };

    let mut payload = Payload::new();
    payload.insert("volume".to_owned(), Value::from(sink.percent));
    payload.insert("muted".to_owned(), Value::from(sink.muted));
    // A microphone is only news when it is silenced; the rest of the time the
    // panel says nothing about it rather than carrying a permanent icon.
    if let Some(source) = level(SOURCE) {
        payload.insert("micMuted".to_owned(), Value::from(source.muted));
        payload.insert("micVolume".to_owned(), Value::from(source.percent));
    }

    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: audio: {error}");
    }
}

fn run(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    loop {
        publish(runtime, id);
        thread::sleep(INTERVAL);
    }
}
