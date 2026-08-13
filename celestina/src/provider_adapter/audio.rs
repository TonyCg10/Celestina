//! What the session is playing through, and whether it is silenced.
//!
//! language-contract: allow-non-english — the status-parser test vector is a
//! verbatim `wpctl status` capture from the author's session, and wpctl prints
//! device descriptions in the session's own locale ("Estéreo analógico").
//! Surviving localized names is part of what the parser is for, so the vector
//! keeps them. Everything else in this file is English development truth.

use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// The devices the session could be using instead, gathered only when a menu
/// asks. This is deliberately not part of the two-second poll: the poll is the
/// panel's volume reading and already the busiest subprocess in the idle
/// profile (2026-08-12 audit), and a device inventory only matters while the
/// audio menu is open. The menu sends `devices-refresh` when it opens, so the
/// list is exactly as fresh as the moment it became visible, and one more
/// `wpctl status` runs per opening — not per tick, forever.
static INVENTORY: Mutex<Option<DeviceInventory>> = Mutex::new(None);

#[derive(Clone, PartialEq, Debug)]
struct AudioDevice {
    id: u32,
    name: String,
    default: bool,
}

/// One application's audio, as a node this session can move on its own.
#[derive(Clone, PartialEq, Debug)]
struct AudioStream {
    id: u32,
    name: String,
    /// Filled in from the node's own level once the inventory is built, so a
    /// per-application slider shows where that application actually is rather
    /// than where the master is.
    percent: u8,
    muted: bool,
}

#[derive(Clone, PartialEq, Debug, Default)]
struct DeviceInventory {
    outputs: Vec<AudioDevice>,
    inputs: Vec<AudioDevice>,
    /// Applications playing, and applications listening.
    playback: Vec<AudioStream>,
    capture: Vec<AudioStream>,
}

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
/// Also not session verbs. Refreshing reads what exists; setting the default
/// re-routes the session, and both belong to the audio menu rather than to a
/// key. wpctl's own name for the second is `set-default`.
const DEVICES_REFRESH: &str = "devices-refresh";
const SET_DEFAULT: &str = "set-default";
/// One named node's own level and mute, which is how an application gets a
/// slider of its own. The session verbs above always mean the default device;
/// these mean whatever node the menu names, and refuse without one.
const NODE_VOLUME: &str = "node-volume";
const NODE_MUTE_TOGGLE: &str = "node-mute-toggle";

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

    if verb == DEVICES_REFRESH {
        refresh_inventory()?;
        publish(runtime, id);
        return Ok(());
    }

    if verb == NODE_VOLUME || verb == NODE_MUTE_TOGGLE {
        let node = options
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("'{NAME}' needs the node to change"))?
            .to_string();

        if verb == NODE_MUTE_TOGGLE {
            run_bounded("wpctl", &["set-mute", &node, "toggle"])
                .ok_or_else(|| format!("wpctl refused to mute {node}"))?;
        } else {
            // Absolute, not a step: the slider already knows where it was put,
            // and asking for a delta would race the reading it was drawn from.
            let percent = options
                .get("percent")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("'{NAME}' needs the level to set"))?;
            let percent = u8::try_from(percent.min(100)).unwrap_or(100);
            let target = unit_fraction(percent);
            run_bounded(
                "wpctl",
                &["set-volume", &node, &target, "-l", VOLUME_CEILING],
            )
            .ok_or_else(|| format!("wpctl refused to set {node} to {target}"))?;
        }

        // Re-reading the whole inventory is what makes the moved slider show
        // the level the node really took, including a ceiling wpctl applied.
        refresh_inventory()?;
        publish(runtime, id);
        return Ok(());
    }

    if verb == SET_DEFAULT {
        // The device is named by wpctl's own node id, taken from the very list
        // this provider published. A name would be ambiguous the moment two
        // devices share one, which HDMI audio does as a matter of course.
        let node = options
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("'{NAME}' needs the device id to make default"))?;
        let node = node.to_string();
        run_bounded("wpctl", &["set-default", &node])
            .ok_or_else(|| format!("wpctl refused to make {node} the default"))?;
        // The list's `default` marks and the panel's volume both changed
        // meaning; read both back at once so the menu never paints its own
        // click as the answer.
        refresh_inventory()?;
        publish(runtime, id);
        return Ok(());
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

/// Reads `wpctl status` once, then each application's own level, and remembers
/// the result.
///
/// The per-application reads are the reason this is never on the poll: they
/// cost one `wpctl get-volume` per application that has audio open, and they
/// only mean anything while the audio menu is on screen.
fn refresh_inventory() -> Result<(), String> {
    let status = run_bounded("wpctl", &["status"])
        .ok_or_else(|| "wpctl would not describe the session's devices".to_owned())?;
    let mut inventory = parse_wpctl_status(&status);
    for stream in inventory
        .playback
        .iter_mut()
        .chain(inventory.capture.iter_mut())
    {
        // An application whose level cannot be read keeps the zero it was
        // built with rather than borrowing the master's, which would show a
        // slider that lies about what moving it would do.
        if let Some(level) = level(&stream.id.to_string()) {
            stream.percent = level.percent;
            stream.muted = level.muted;
        }
    }
    *INVENTORY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(inventory);
    Ok(())
}

/// The `Sinks:` and `Sources:` sections of `wpctl status`, read as data.
///
/// The output is a box-drawn tree for people, so this reads only the shape
/// that identifies a device row inside the two sections it cares about: an
/// optional `*` for the default, an integer id, a dot, the name, and a
/// bracketed volume this parser ignores because the panel's own reading is the
/// volume of record. Anything shaped differently is someone else's line and is
/// skipped rather than guessed at.
#[derive(Clone, Copy, PartialEq)]
enum Section {
    Outputs,
    Inputs,
    Streams,
}

/// Files the application being described, once its ports have been seen or the
/// next application has started and it turns out to have had none.
fn file_stream(
    inventory: &mut DeviceInventory,
    pending: &mut Option<AudioStream>,
    capture: &mut bool,
) {
    if let Some(stream) = pending.take() {
        if *capture {
            inventory.capture.push(stream);
        } else {
            inventory.playback.push(stream);
        }
    }
    *capture = false;
}

/// One `id. name` row, with the `*` that marks a default and wpctl's own
/// trailing `[vol: …]` marker removed. `None` for anything shaped otherwise,
/// which is someone else's line rather than something to guess at.
fn parse_row(body: &str) -> Option<(bool, u32, &str)> {
    let (default, row) = match body.strip_prefix('*') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, body),
    };
    let (number, name) = row.split_once('.')?;
    let id = number.trim().parse::<u32>().ok()?;
    // A bracket is otherwise part of the name — `PipeWire ALSA [parsecd]` says
    // which application it is, and a generic "cut at the first bracket" turned
    // every ALSA client into the same anonymous row.
    let name = match name.split_once(" [vol:") {
        Some((bare, _)) => bare,
        None => name,
    }
    .trim();
    if name.is_empty() {
        return None;
    }
    Some((default, id, name))
}

fn parse_wpctl_status(status: &str) -> DeviceInventory {
    let mut inventory = DeviceInventory::default();
    let mut in_audio = false;
    let mut section: Option<Section> = None;
    // The application row a port line belongs to: `Streams:` lists an
    // application and then the ports it opened, and only those ports say which
    // direction it is going.
    let mut pending: Option<AudioStream> = None;
    let mut pending_capture = false;

    for line in status.lines() {
        // `Video` has `Sinks:` and `Sources:` sections of its own — a webcam
        // is a video source — so only the blocks under `Audio` are read. The
        // top-level titles are the only lines that start at column zero.
        if !line.starts_with(char::is_whitespace) && !line.is_empty() {
            file_stream(&mut inventory, &mut pending, &mut pending_capture);
            in_audio = line.trim() == "Audio";
            section = None;
            continue;
        }
        if !in_audio {
            continue;
        }

        // The tree characters are decoration; what remains is the row.
        let body = line
            .trim_matches(|c: char| c.is_whitespace() || "│├─└".contains(c))
            .trim();

        if body.starts_with("Sinks:") {
            file_stream(&mut inventory, &mut pending, &mut pending_capture);
            section = Some(Section::Outputs);
            continue;
        }
        if body.starts_with("Sources:") {
            file_stream(&mut inventory, &mut pending, &mut pending_capture);
            section = Some(Section::Inputs);
            continue;
        }
        if body.starts_with("Streams:") {
            file_stream(&mut inventory, &mut pending, &mut pending_capture);
            section = Some(Section::Streams);
            continue;
        }
        // Any other titled block ends the one we were in: `Filters:`,
        // `Devices:` and friends also live under `Audio` and also contain
        // numbered rows that are not devices.
        if body.ends_with(':') {
            file_stream(&mut inventory, &mut pending, &mut pending_capture);
            section = None;
            continue;
        }
        let Some(section) = section else { continue };
        if body.is_empty() {
            continue;
        }

        let Some((default, id, name)) = parse_row(body) else {
            continue;
        };

        match section {
            Section::Outputs | Section::Inputs => {
                let device = AudioDevice {
                    id,
                    name: name.to_owned(),
                    default,
                };
                if section == Section::Outputs {
                    inventory.outputs.push(device);
                } else {
                    inventory.inputs.push(device);
                }
            }
            Section::Streams => {
                // A port row belongs to the application above it and is what
                // says which way that application's audio is going. Ports are
                // named by direction — `output_FL`, `input_FR` — which is the
                // node's own vocabulary rather than an indentation count that
                // a wider terminal could change.
                if name.starts_with("output_") {
                    continue;
                }
                if name.starts_with("input_") {
                    pending_capture = true;
                    continue;
                }
                // Anything else at this level is the next application, so the
                // one being described is complete.
                file_stream(&mut inventory, &mut pending, &mut pending_capture);
                pending = Some(AudioStream {
                    id,
                    name: name.to_owned(),
                    // Filled in per node afterwards; `wpctl status` prints no
                    // level for a stream.
                    percent: 0,
                    muted: false,
                });
            }
        }
    }

    file_stream(&mut inventory, &mut pending, &mut pending_capture);
    inventory
}

/// The inventory as payload values, or `None` while no menu has asked yet.
fn inventory_values() -> Option<(Value, Value, Value, Value)> {
    let held = INVENTORY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let inventory = held.as_ref()?;
    let describe = |devices: &[AudioDevice]| {
        Value::from(
            devices
                .iter()
                .map(|device| {
                    let mut entry = serde_json::Map::new();
                    entry.insert("id".to_owned(), Value::from(device.id));
                    entry.insert("name".to_owned(), Value::from(device.name.clone()));
                    entry.insert("default".to_owned(), Value::from(device.default));
                    Value::from(entry)
                })
                .collect::<Vec<Value>>(),
        )
    };
    let describe_streams = |streams: &[AudioStream]| {
        Value::from(
            streams
                .iter()
                .map(|stream| {
                    let mut entry = serde_json::Map::new();
                    entry.insert("id".to_owned(), Value::from(stream.id));
                    entry.insert("name".to_owned(), Value::from(stream.name.clone()));
                    entry.insert("volume".to_owned(), Value::from(stream.percent));
                    entry.insert("muted".to_owned(), Value::from(stream.muted));
                    Value::from(entry)
                })
                .collect::<Vec<Value>>(),
        )
    };
    Some((
        describe(&inventory.outputs),
        describe(&inventory.inputs),
        describe_streams(&inventory.playback),
        describe_streams(&inventory.capture),
    ))
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
    // Present only once a menu has asked, and kept across the two-second poll
    // so the list does not blink out from under an open menu.
    if let Some((outputs, inputs, playback, capture)) = inventory_values() {
        payload.insert("outputs".to_owned(), outputs);
        payload.insert("inputs".to_owned(), inputs);
        payload.insert("playbackApps".to_owned(), playback);
        payload.insert("captureApps".to_owned(), capture);
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

#[cfg(test)]
mod tests {
    use super::{parse_wpctl_status, AudioDevice};

    /// The shape `wpctl status` prints on the author's own session, with the
    /// details that have already almost caused defects: a `Video` block whose
    /// `Sources:` rows are cameras, sinks whose names carry brackets of their
    /// own, muted markers, and a default on either side.
    const STATUS: &str = "\
PipeWire 'pipewire-0' [1.6.8, toni@cachyos, cookie:905717462]
 └─ Clients:
        32. WirePlumber                         [1.6.8, toni@cachyos, pid:2306]

Audio
 ├─ Devices:
 │      41. USB2.0 Device                       [alsa]
 │
 ├─ Sinks:
 │  *   46. USB2.0 Device Estéreo analógico   [vol: 0.70 MUTED]
 │      63. Navi 48 HDMI/DP Audio Controller Estéreo digital (HDMI) [vol: 1.00]
 │
 ├─ Sources:
 │      45. USB2.0 Device Mono                  [vol: 0.00 MUTED]
 │  *   47. Brio 100 Mono                       [vol: 0.88 MUTED]
 │
 ├─ Filters:
 │      51. loopback                            [Audio/Sink]
 │
 └─ Streams:
        58. Firefox
             65. output_FL       > USB2.0 Device:playback_FL    [active]
        77. HOI4
             81. output_FL       > USB2.0 Device:playback_FL    [active]
             84. output_FR       > USB2.0 Device:playback_FR    [active]
        80. PipeWire ALSA [parsecd]
             85. output_FL       > USB2.0 Device:playback_FL    [active]
             86. output_FR       > USB2.0 Device:playback_FR    [active]
        90. OBS Studio
             91. input_FL        < Brio 100:capture_FL          [active]

Video
 ├─ Sinks:
 │
 ├─ Sources:
 │  *   74. Brio 100 (V4L2)
 │
 └─ Streams:

Settings
 └─ Default Configured Devices:
";

    #[test]
    fn the_audio_sections_are_read_and_the_video_camera_is_not() {
        let inventory = parse_wpctl_status(STATUS);

        assert_eq!(
            inventory.outputs,
            vec![
                AudioDevice {
                    id: 46,
                    name: "USB2.0 Device Estéreo analógico".to_owned(),
                    default: true,
                },
                AudioDevice {
                    id: 63,
                    name: "Navi 48 HDMI/DP Audio Controller Estéreo digital (HDMI)".to_owned(),
                    default: false,
                },
            ]
        );
        assert_eq!(
            inventory.inputs,
            vec![
                AudioDevice {
                    id: 45,
                    name: "USB2.0 Device Mono".to_owned(),
                    default: false,
                },
                AudioDevice {
                    id: 47,
                    name: "Brio 100 Mono".to_owned(),
                    default: true,
                },
            ]
        );
    }

    /// The camera lives under `Video ├─ Sources:` and shares the row shape
    /// exactly, so only the section walk keeps it out. This is the case that
    /// found the defect: the first parser listed the webcam as a microphone.
    #[test]
    fn a_webcam_is_not_a_microphone() {
        let inventory = parse_wpctl_status(STATUS);
        assert!(inventory.inputs.iter().all(|device| device.id != 74));
        assert!(inventory.outputs.iter().all(|device| device.id != 74));
    }

    /// Streams, filters, clients and devices all carry numbered rows too;
    /// none of them is something the session can be switched to.
    /// Applications are read from `Streams:` and filed by the direction of
    /// their own ports, because that is the node's vocabulary. HOI4 plays,
    /// the ALSA client plays, and a recorder that opened `input_` ports is a
    /// capture stream rather than another thing to turn down.
    #[test]
    fn applications_are_read_and_filed_by_the_direction_of_their_ports() {
        let inventory = parse_wpctl_status(STATUS);

        let playing: Vec<(u32, &str)> = inventory
            .playback
            .iter()
            .map(|stream| (stream.id, stream.name.as_str()))
            .collect();
        assert_eq!(
            playing,
            vec![
                (58, "Firefox"),
                (77, "HOI4"),
                (80, "PipeWire ALSA [parsecd]")
            ]
        );

        let listening: Vec<(u32, &str)> = inventory
            .capture
            .iter()
            .map(|stream| (stream.id, stream.name.as_str()))
            .collect();
        assert_eq!(listening, vec![(90, "OBS Studio")]);
    }

    /// A port row carries an id of its own and the same `id. name` shape, so
    /// nothing but the direction prefix keeps it from being listed as an
    /// application in its own right.
    #[test]
    fn a_port_is_not_an_application() {
        let inventory = parse_wpctl_status(STATUS);
        let ids: Vec<u32> = inventory
            .playback
            .iter()
            .chain(&inventory.capture)
            .map(|stream| stream.id)
            .collect();
        for port in [65, 81, 84, 85, 86, 91] {
            assert!(!ids.contains(&port), "{port} is a port, not an application");
        }
    }

    #[test]
    fn only_sinks_and_sources_are_devices() {
        let inventory = parse_wpctl_status(STATUS);
        let ids: Vec<u32> = inventory
            .outputs
            .iter()
            .chain(&inventory.inputs)
            .map(|device| device.id)
            .collect();
        for foreign in [32, 41, 51, 58, 65] {
            assert!(!ids.contains(&foreign), "{foreign} is not a device");
        }
    }

    /// An answer with nothing usable is an empty inventory, never an error:
    /// a session without `PipeWire` simply has no list.
    #[test]
    fn an_alien_answer_is_an_empty_inventory() {
        let inventory = parse_wpctl_status("no such command\n");
        assert!(inventory.outputs.is_empty());
        assert!(inventory.inputs.is_empty());
    }
}
