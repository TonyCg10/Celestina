//! Parser tests over captured `wpctl` and `pw-cli` output; one feeds
//! Spanish descriptions on purpose (non-ASCII input).
//!
//! language-contract: allow-non-english

#![cfg(feature = "wpctl")]

use cuprita_core::model::EndpointKind;
use std::collections::HashMap;

use cuprita_core::wpctl::{
    card_profiles, parse_profiles, parse_status, stream_endpoints, ProfileEntry,
};

const STATUS: &str = include_str!("fixtures/wpctl-status.txt");
const ENUM_PROFILE: &str = include_str!("fixtures/pw-cli-enum-profile.txt");
const PROFILE: &str = include_str!("fixtures/pw-cli-profile.txt");

#[test]
fn status_lists_sinks_sources_and_defaults() {
    let status = parse_status(STATUS);
    let sinks: Vec<_> = status
        .endpoints
        .iter()
        .filter(|e| e.kind == EndpointKind::Sink)
        .collect();
    assert_eq!(sinks.len(), 2);
    assert_eq!(sinks[0].id, 46);
    assert!(sinks[0].default && sinks[0].muted);
    assert_eq!(sinks[0].description, "USB2.0 Device Analog Stereo");
    assert_eq!(sinks[1].id, 92);
    assert!(!sinks[1].default && !sinks[1].muted);
    assert!((sinks[1].volume - 0.75).abs() < 1e-6);
    // A description with brackets of its own keeps them.
    assert_eq!(
        sinks[1].description,
        "Navi 48 HDMI/DP Audio Controller Digital Stereo (HDMI) [LG ULTRAGEAR+]"
    );
    let sources: Vec<_> = status
        .endpoints
        .iter()
        .filter(|e| e.kind == EndpointKind::Source)
        .collect();
    assert_eq!(sources.len(), 2);
    assert!(sources[0].default && !sources[1].default);
}

#[test]
fn status_lists_streams_and_their_endpoints() {
    let status = parse_status(STATUS);
    assert_eq!(status.streams.len(), 2);
    let speech = &status.streams[0];
    assert_eq!(
        (speech.id, speech.app_name.as_str()),
        (116, "speech-dispatcher-dummy")
    );
    assert_eq!(speech.endpoint, 46);
    // Recording a sink's monitor counts as that sink, not the source.
    let spectrum = &status.streams[1];
    assert_eq!(
        (spectrum.id, spectrum.app_name.as_str()),
        (137, "Noctalia Spectrum")
    );
    assert_eq!(spectrum.endpoint, 46);
}

#[test]
fn status_lists_cards_and_skips_video() {
    let status = parse_status(STATUS);
    let ids: Vec<u32> = status.cards.iter().map(|c| c.id).collect();
    assert_eq!(ids, [55, 56, 57, 58]);
    assert_eq!(status.cards[2].name, "USB2.0 Device");
    assert!(status.endpoints.iter().all(|e| e.id != 87));
}

#[test]
fn profiles_mark_the_active_one_and_drop_off() {
    let all = parse_profiles(ENUM_PROFILE);
    assert_eq!(all[0].name, "off");
    assert_eq!(all[1].index, 1);
    let active = parse_profiles(PROFILE);
    assert_eq!(active.len(), 1);
    let profiles = card_profiles(57, &all, &active);
    assert_eq!(profiles.len(), all.len() - 1);
    let on: Vec<_> = profiles.iter().filter(|p| p.active).collect();
    assert_eq!(on.len(), 1);
    assert_eq!(on[0].id, "output:analog-stereo+input:mono-fallback");
    assert_eq!(on[0].description, "Analog Stereo Output + Mono Input");
    assert!(profiles.iter().all(|p| p.card_id == 57));
}

fn status_of(audio: &str) -> String {
    format!("PipeWire 'pipewire-0' [1.6.9]\n └─ Clients:\n        32. WirePlumber\n\nAudio\n{audio}\nVideo\n ├─ Sinks:\n │  *   87. Camera   [vol: 1.00]\n")
}

#[test]
fn an_empty_audio_section_reads_as_nothing() {
    let status = parse_status(&status_of(
        " ├─ Devices:\n │  \n ├─ Sinks:\n │  \n ├─ Sources:\n │  \n ├─ Filters:\n │  \n └─ Streams:\n",
    ));
    assert!(status.endpoints.is_empty() && status.streams.is_empty() && status.cards.is_empty());
}

#[test]
fn non_ascii_descriptions_and_a_muted_source() {
    let status = parse_status(&status_of(
        " ├─ Sources:\n │  *   34. Micrófono «estudio» ñ   [vol: 0.40 MUTED]\n │      45. Línea   [vol: 1.20]\n └─ Streams:\n",
    ));
    assert_eq!(status.endpoints.len(), 2);
    let mic = &status.endpoints[0];
    assert_eq!(mic.description, "Micrófono «estudio» ñ");
    assert!(mic.muted && mic.default && mic.kind == EndpointKind::Source);
    assert!((mic.volume - 0.4).abs() < 1e-6);
    assert!(!status.endpoints[1].muted);
}

#[test]
fn streams_with_ids_of_every_width_and_order() {
    let status = parse_status(&status_of(
        " ├─ Sinks:\n │  *   46. Speakers Analog   [vol: 0.50]\n │  \n └─ Streams:\n\
         \x20        9. Tiny\n\
         \x20            10. output_FL       > Speakers:playback_FL\t[active]\n\
         \x20     1203. Wide\n\
         \x20             8. output_FL       > Speakers:playback_FL\t[active]\n\
         \x20          1204. monitor_FL     \n\
         \x20      140. Lost\n\
         \x20           141. output_FL       > Nowhere:playback_FL\t[init]\n\
         \x20       77. Silent\n",
    ));
    let ids: Vec<u32> = status.streams.iter().map(|s| s.id).collect();
    assert_eq!(ids, [9, 1203, 140, 77]);
    assert_eq!(status.stream_ports.get(&9), Some(&vec![10]));
    assert_eq!(status.stream_ports.get(&1203), Some(&vec![8, 1204]));
    assert_eq!(status.streams[0].endpoint, 46);
    // A port naming no endpoint, or no port at all: no endpoint.
    assert_eq!(status.streams[2].endpoint, 0);
    assert_eq!(status.streams[3].endpoint, 0);
}

#[test]
fn links_beat_the_nickname_guess() {
    let mut status = parse_status(STATUS);
    for endpoint in &mut status.endpoints {
        endpoint.name = format!("node-{}", endpoint.id);
    }
    // The speech stream's port 117 is linked to the HDMI sink's node.
    let mut links = HashMap::new();
    links.insert(117, vec!["node-92".to_owned()]);
    stream_endpoints(
        &mut status.streams,
        &status.stream_ports,
        &status.endpoints,
        &links,
    );
    assert_eq!(status.streams[0].endpoint, 92);
    // No link: the guess stands.
    assert_eq!(status.streams[1].endpoint, 46);
}

#[test]
fn an_unavailable_profile_is_offered_only_when_active() {
    let entry = |index, name: &str, available| ProfileEntry {
        index,
        available,
        name: name.to_owned(),
        description: name.to_owned(),
    };
    let all = [
        entry(1, "a", true),
        entry(2, "b", false),
        entry(3, "c", false),
    ];
    let names = |active: &[ProfileEntry]| -> Vec<String> {
        card_profiles(5, &all, active)
            .into_iter()
            .map(|p| p.id)
            .collect()
    };
    assert_eq!(names(&[entry(1, "a", true)]), ["a"]);
    assert_eq!(names(&[entry(3, "c", false)]), ["a", "c"]);
    // The fixture's card: every profile there is available or unknown.
    assert!(parse_profiles(ENUM_PROFILE).iter().all(|p| p.available));
}
