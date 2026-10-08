use cuprita_core::audio::Audio;
use cuprita_core::bluetooth::Bluetooth;
use cuprita_core::fake::{ids, FakeAudio, FakeBluetooth, FakeNetwork};
use cuprita_core::model::{NetworkKind, NetworkState};
use cuprita_core::network::Network;

#[test]
fn the_scripted_network_starts_wired_and_ordered() {
    let snapshot = FakeNetwork::scripted().snapshot().unwrap();
    assert_eq!(snapshot.networks.len(), 4);
    assert_eq!(snapshot.networks[0].kind, NetworkKind::Ethernet);
    assert_eq!(snapshot.networks[0].state, NetworkState::Connected);
}

#[test]
fn connecting_the_open_cafe_connects_it() {
    let mut network = FakeNetwork::scripted();
    network.connect("open-cafe", None).unwrap();
    let snapshot = network.snapshot().unwrap();
    let cafe = snapshot
        .networks
        .iter()
        .find(|n| n.id == "open-cafe")
        .unwrap();
    assert_eq!(cafe.state, NetworkState::Connected);
}

#[test]
fn wifi_off_hides_the_wifi_rows() {
    let mut network = FakeNetwork::scripted();
    network.set_wifi_enabled(false).unwrap();
    let snapshot = network.snapshot().unwrap();
    assert!(!snapshot.wifi_enabled);
    assert!(snapshot
        .networks
        .iter()
        .all(|n| n.kind != NetworkKind::Wifi));
}

#[test]
fn pairing_the_phone_pairs_it() {
    let mut bluetooth = FakeBluetooth::scripted();
    bluetooth.pair("AA:BB:CC:00:00:02").unwrap();
    let snapshot = bluetooth.snapshot().unwrap();
    assert!(
        snapshot
            .devices
            .iter()
            .find(|d| d.address == "AA:BB:CC:00:00:02")
            .unwrap()
            .paired
    );
}

#[test]
fn the_default_sink_moves() {
    let mut audio = FakeAudio::scripted();
    audio.set_default(ids::SINK_B).unwrap();
    let snapshot = audio.snapshot().unwrap();
    let default = snapshot
        .endpoints
        .iter()
        .find(|e| e.default && e.id != ids::SOURCE)
        .unwrap();
    assert_eq!(default.id, ids::SINK_B);
    assert!(
        snapshot
            .endpoints
            .iter()
            .find(|e| e.id == ids::SOURCE)
            .unwrap()
            .default
    );
}

#[test]
fn volume_and_profile_commands_are_observed() {
    let mut audio = FakeAudio::scripted();
    audio.set_volume(ids::STREAM_MUSIC, 0.25).unwrap();
    audio.set_muted(ids::SINK_A, true).unwrap();
    audio.set_profile(ids::CARD, "output:hdmi-stereo").unwrap();
    let snapshot = audio.snapshot().unwrap();
    assert_eq!(
        snapshot
            .streams
            .iter()
            .find(|s| s.id == ids::STREAM_MUSIC)
            .unwrap()
            .volume,
        0.25
    );
    assert!(
        snapshot
            .endpoints
            .iter()
            .find(|e| e.id == ids::SINK_A)
            .unwrap()
            .muted
    );
    assert!(
        snapshot
            .profiles
            .iter()
            .find(|p| p.id == "output:hdmi-stereo")
            .unwrap()
            .active
    );
}
