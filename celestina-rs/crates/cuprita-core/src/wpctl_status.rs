//! The pure half of `wpctl status`: the parser over the command's text, with
//! no process and no feature gate, so the shell can read the same output
//! Cuprita reads. The process side (`wpctl`, feature `wpctl`) re-exports these
//! names, so its callers keep their paths.

use std::collections::{HashMap, HashSet};

use crate::model::{AudioEndpoint, AudioStream, EndpointKind};
use crate::volume::clamp_volume;

/// One card listed under `Devices:`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: u32,
    pub name: String,
}

/// What `wpctl status` says about the audio graph: the endpoints (names
/// still to fill from `inspect`), the streams (volumes still to fill from
/// `get-volume`) and the cards.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Status {
    pub endpoints: Vec<AudioEndpoint>,
    pub streams: Vec<AudioStream>,
    pub cards: Vec<Card>,
    /// Each stream's port ids, to follow its links (`pw-link -lI`).
    pub stream_ports: HashMap<u32, Vec<u32>>,
    /// Ids of the streams with at least one port line in state `[active]`,
    /// whether playback or recording (the consumer filters by endpoint).
    pub playing: HashSet<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Other,
    Devices,
    Sinks,
    Sources,
    Streams,
}

/// Strips the tree drawing (`│ ├─ └─`) and returns the rest of the line.
fn untree(line: &str) -> &str {
    line.trim_start_matches([' ', '│', '├', '└', '─'])
}

/// `"46. Name [vol: 0.65 MUTED]"` → `(46, "Name [vol: 0.65 MUTED]")`.
fn numbered(text: &str) -> Option<(u32, &str)> {
    let (id, rest) = text.split_once(". ")?;
    Some((id.trim().parse().ok()?, rest.trim()))
}

/// `"Name   [vol: 0.65 MUTED]"` → `("Name", 0.65, true)`.
fn volume_suffix(text: &str) -> (&str, Option<(f32, bool)>) {
    let Some(at) = text.rfind("[vol:") else {
        return (text.trim(), None);
    };
    let inside = text[at + 5..].trim_end().trim_end_matches(']');
    let mut words = inside.split_whitespace();
    let volume = words.next().and_then(|v| v.parse::<f32>().ok());
    let muted = words.any(|w| w == "MUTED");
    (text[..at].trim(), volume.map(|v| (v, muted)))
}

/// Parses the `Audio` part of `wpctl status`. Endpoint names are left empty
/// and stream volumes at 1.0: the status line does not carry them.
#[must_use]
pub fn parse_status(text: &str) -> Status {
    let mut status = Status::default();
    let mut in_audio = false;
    let mut section = Section::Other;
    // Where the current stream's id ends (ids are right-aligned, so this
    // column, not the indentation, tells a stream from its deeper ports).
    let mut stream_column: Option<usize> = None;
    // Per stream: the target's prefix and whether it is a sink monitor.
    let mut targets: Vec<(String, bool, bool)> = Vec::new();

    for line in text.lines() {
        if !line.starts_with(' ') && !line.is_empty() {
            in_audio = line.trim() == "Audio";
            section = Section::Other;
            continue;
        }
        if !in_audio {
            continue;
        }
        let body = untree(line);
        match body.trim_end() {
            "Devices:" => {
                section = Section::Devices;
                continue;
            }
            "Sinks:" => {
                section = Section::Sinks;
                continue;
            }
            "Sources:" => {
                section = Section::Sources;
                continue;
            }
            "Streams:" => {
                section = Section::Streams;
                continue;
            }
            "Filters:" => {
                section = Section::Other;
                continue;
            }
            _ => {}
        }
        let default = body.starts_with('*');
        let entry = body.trim_start_matches('*').trim_start();
        let Some((id, rest)) = numbered(entry) else {
            continue;
        };
        match section {
            Section::Devices => {
                let name = strip_api_tag(rest);
                status.cards.push(Card {
                    id,
                    name: name.to_owned(),
                });
            }
            Section::Sinks | Section::Sources => {
                let (description, volume) = volume_suffix(rest);
                let (volume, muted) = volume.unwrap_or((1.0, false));
                status.endpoints.push(AudioEndpoint {
                    id,
                    kind: if section == Section::Sinks {
                        EndpointKind::Sink
                    } else {
                        EndpointKind::Source
                    },
                    name: String::new(),
                    description: description.to_owned(),
                    volume: clamp_volume(volume),
                    muted,
                    default,
                });
            }
            Section::Streams => {
                let column = line.find(". ").unwrap_or(0);
                let has_arrow = rest.contains(" > ") || rest.contains(" < ");
                let is_port = has_arrow || stream_column.is_some_and(|c| column > c);
                if is_port {
                    if let Some(stream) = status.streams.last() {
                        status.stream_ports.entry(stream.id).or_default().push(id);
                        // The state follows the port's tab: `…\t[active]`.
                        let active = rest
                            .rsplit_once('\t')
                            .is_some_and(|(_, state)| state.trim() == "[active]");
                        if active {
                            status.playing.insert(stream.id);
                        }
                    }
                }
                if !is_port {
                    stream_column = Some(column);
                    status.streams.push(AudioStream {
                        id,
                        app_name: rest.to_owned(),
                        app_icon: String::new(),
                        volume: 1.0,
                        muted: false,
                        endpoint: 0,
                    });
                    targets.push((String::new(), false, false));
                } else if let Some(target) = targets.last_mut() {
                    // A port: `output_FL > Device:playback_FL [state]`.
                    if target.0.is_empty() {
                        if let Some((arrow, port)) = rest
                            .split_once(" > ")
                            .map(|(_, p)| (true, p))
                            .or_else(|| rest.split_once(" < ").map(|(_, p)| (false, p)))
                        {
                            let port = port.split('\t').next().unwrap_or(port).trim();
                            if let Some((node, port_name)) = port.rsplit_once(':') {
                                *target =
                                    (node.to_owned(), arrow, port_name.starts_with("monitor"));
                            }
                        }
                    }
                }
            }
            Section::Other => {}
        }
    }

    // A first guess at a stream's endpoint, for when its links cannot be
    // read: the sink it plays to (or whose monitor it records), else the
    // source it records from, by the longest description that starts with
    // the port's node nickname. `stream_endpoints` replaces it from the links.
    for (stream, (node, plays, monitor)) in status.streams.iter_mut().zip(targets) {
        if node.is_empty() {
            continue;
        }
        let kind = if plays || monitor {
            EndpointKind::Sink
        } else {
            EndpointKind::Source
        };
        stream.endpoint = status
            .endpoints
            .iter()
            .filter(|e| e.kind == kind && e.description.starts_with(node.as_str()))
            .max_by_key(|e| e.description.len())
            .map_or(0, |e| e.id);
    }
    status
}

/// A card line's name without its trailing API tag (`Brio 100   [alsa]`).
fn strip_api_tag(text: &str) -> &str {
    let text = text.trim();
    if let Some(open) = text.rfind(" [") {
        let tag = &text[open + 2..];
        if let Some(inner) = tag.strip_suffix(']') {
            let api_like = !inner.is_empty()
                && inner
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if api_like {
                return text[..open].trim_end();
            }
        }
    }
    text
}

/// `Volume: 0.65 [MUTED]` from `wpctl get-volume`.
#[must_use]
pub fn parse_volume(text: &str) -> Option<(f32, bool)> {
    let rest = text.trim().strip_prefix("Volume:")?;
    let mut words = rest.split_whitespace();
    let volume = words.next()?.parse::<f32>().ok()?;
    Some((clamp_volume(volume), words.any(|w| w == "[MUTED]")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_names_lose_only_an_api_tag() {
        assert_eq!(strip_api_tag("Brio 100      [alsa]"), "Brio 100");
        assert_eq!(
            strip_api_tag("Card [LG ULTRAGEAR+]"),
            "Card [LG ULTRAGEAR+]"
        );
        assert_eq!(strip_api_tag("Plain"), "Plain");
    }

    #[test]
    fn volume_lines() {
        assert_eq!(parse_volume("Volume: 0.40 [MUTED]\n"), Some((0.4, true)));
        assert_eq!(parse_volume("Volume: 2.00"), Some((1.5, false)));
        assert_eq!(parse_volume("nonsense"), None);
    }

    #[test]
    fn playing_streams_come_from_active_port_lines() {
        let text = r"Audio
 ├─ Sinks:
 │  *   46. USB2.0 Device Analog Stereo       [vol: 0.00 MUTED]
 │     200. Bluetooth Headset                  [vol: 0.50]
 ├─ Sources:
 │  *   34. Brio 100 Mono                       [vol: 0.00 MUTED]
 └─ Streams:
       116. speech-dispatcher-dummy
            117. output_FL       > USB2.0 Device:playback_FL\t[init]
            118. output_FR       > USB2.0 Device:playback_FR\t[init]
       150. Zen
            151. output_FL       > Bluetooth Headset:playback_FL\t[active]
            152. output_FR       > Bluetooth Headset:playback_FR\t[active]
       137. Recorder
            138. input_FL        < Brio 100:capture_FL\t[active]
       160. Idle
"
        .replace("\\t", "\t");
        let status = parse_status(&text);
        assert_eq!(status.playing, HashSet::from([150, 137]));
    }
}
