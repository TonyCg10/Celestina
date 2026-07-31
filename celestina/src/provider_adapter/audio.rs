//! What the session is playing through, and whether it is silenced.

use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use celestina_shell_core::audio::{self, AudioLevel};
use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::snapshot::{Payload, ProviderId};
use serde_json::Value;

use super::tools::{launch, lock_runtime, run_bounded};

/// Volume changes are user-driven and rare, so the panel asks rarely. What
/// matters for how it feels is that the panel re-reads immediately after a
/// change it made itself, which it does.
const INTERVAL: Duration = Duration::from_secs(2);
/// The session's own step and ceiling: `config.kdl` raises volume with
/// `wpctl set-volume @DEFAULT_AUDIO_SINK@ 0.1+ -l 1.0`, and Noctalia's bar
/// scrolled in 5 % steps. The keys keep their step; the panel keeps the bar's.
const VOLUME_UP: &str = "0.05+";
const VOLUME_DOWN: &str = "0.05-";
const VOLUME_CEILING: &str = "1.0";
/// The mixer the session already has. `qpwgraph` is a patchbay, not a mixer.
const EXTERNAL_MIXER: &str = "pavucontrol";
/// wpctl's names for "whatever the session is playing through / listening with".
const SINK: &str = "@DEFAULT_AUDIO_SINK@";
const SOURCE: &str = "@DEFAULT_AUDIO_SOURCE@";

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

pub fn action(verb: &str, runtime: &Mutex<ProviderRuntime>, id: &ProviderId) -> Result<(), String> {
    let args: Vec<&str> = match verb {
        // The session's own ceiling: no overdrive, the same `-l 1.0` the keys
        // already pass.
        "louder" => vec!["set-volume", SINK, VOLUME_UP, "-l", VOLUME_CEILING],
        "quieter" => vec!["set-volume", SINK, VOLUME_DOWN],
        "toggle-mute" => vec!["set-mute", SINK, "toggle"],
        "toggle-mic-mute" => vec!["set-mute", SOURCE, "toggle"],
        "open-mixer" => return launch(EXTERNAL_MIXER),
        _ => return Err(format!("'{NAME}' does not serve the verb '{verb}'")),
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
