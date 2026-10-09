use cuprita_core::model::*;
use cuprita_core::order::{order_devices, order_networks, signal_bars};

fn net(name: &str, state: NetworkState, known: bool, signal: Option<u8>) -> Network {
    Network {
        id: name.into(),
        kind: NetworkKind::Wifi,
        name: name.into(),
        state,
        signal,
        security: Security::Psk,
        known,
    }
}

#[test]
fn connected_then_known_then_signal() {
    let mut v = vec![
        net("weak-known", NetworkState::Disconnected, true, Some(20)),
        net(
            "strong-unknown",
            NetworkState::Disconnected,
            false,
            Some(90),
        ),
        net("home", NetworkState::Connected, true, Some(60)),
        net("mid-unknown", NetworkState::Disconnected, false, Some(50)),
    ];
    order_networks(&mut v);
    let names: Vec<_> = v.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(
        names,
        ["home", "weak-known", "strong-unknown", "mid-unknown"]
    );
}

#[test]
fn bars_thresholds() {
    assert_eq!(signal_bars(0), 0);
    assert_eq!(signal_bars(29), 1);
    assert_eq!(signal_bars(54), 2);
    assert_eq!(signal_bars(79), 3);
    assert_eq!(signal_bars(100), 4);
}

#[test]
fn security_keys_are_tokens() {
    use cuprita_core::order::security_label_key;
    assert_eq!(security_label_key(Security::Open), "open");
    assert_eq!(security_label_key(Security::Psk), "psk");
    assert_eq!(security_label_key(Security::Enterprise), "enterprise");
    assert_eq!(security_label_key(Security::Wep), "wep");
}

fn device(name: &str, paired: bool, connected: bool) -> BluetoothDevice {
    BluetoothDevice {
        address: name.into(),
        name: name.into(),
        kind: DeviceKind::Other,
        paired,
        connected,
        trusted: paired,
        battery: None,
    }
}

#[test]
fn devices_connected_then_paired_then_by_name() {
    let mut v = vec![
        device("zebra", false, false),
        device("Mouse", true, false),
        device("phone", true, true),
        device("apple", false, false),
        device("keyboard", true, false),
    ];
    order_devices(&mut v);
    let names: Vec<_> = v.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["phone", "keyboard", "Mouse", "apple", "zebra"]);
}
