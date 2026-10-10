use selenita_core::niri::{self, NiriError};
use selenita_core::Geometry;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;

const FOCUSED: &str = include_str!("fixtures/niri-focused-window.json");
const OUTPUTS: &str = include_str!("fixtures/niri-outputs.json");
const WINDOWS: &str = include_str!("fixtures/niri-windows.json");
const WORKSPACES: &str = include_str!("fixtures/niri-workspaces.json");

#[test]
fn the_focused_window_reads_from_niris_reply() {
    let window = niri::parse_focused_window(FOCUSED).expect("the fixture");
    assert_eq!(window.id, 2);
    assert_eq!(window.app_id, "com.anthropic.Claude");
    assert_eq!(window.title, "A window title");
    assert_eq!(window.size, (942, 1010));
    // A tiled window: niri gives no position.
    assert_eq!(window.position, None);
    assert_eq!(
        window.geometry(),
        Geometry {
            x: 0,
            y: 0,
            w: 942,
            h: 1010
        }
    );
}

#[test]
fn a_floating_window_carries_its_position_and_offset() {
    let json = FOCUSED
        .replace(
            "\"tile_pos_in_workspace_view\": null",
            "\"tile_pos_in_workspace_view\": [100.4, 50.0]",
        )
        .replace(
            "\"window_offset_in_tile\": [\n      0.0,\n      0.0\n    ]",
            "\"window_offset_in_tile\": [2.0, 3.0]",
        );
    let window = niri::parse_focused_window(&json).expect("floating");
    assert_eq!(window.position, Some((102, 53)));
}

#[test]
fn no_focused_window_and_garbage_are_typed() {
    assert_eq!(
        niri::parse_focused_window("null"),
        Err(NiriError::NoFocusedWindow)
    );
    assert!(matches!(
        niri::parse_focused_window("{"),
        Err(NiriError::Protocol(_))
    ));
    assert!(matches!(
        niri::parse_focused_window("{\"id\":1}"),
        Err(NiriError::Protocol(_))
    ));
}

#[test]
fn the_outputs_read_left_to_right() {
    let outputs = niri::parse_outputs(OUTPUTS).expect("the fixture");
    let names: Vec<_> = outputs.iter().map(|output| output.name.as_str()).collect();
    assert_eq!(names, vec!["HDMI-A-1", "DP-1", "DP-2"]);
    assert_eq!(
        outputs[1].logical,
        Geometry {
            x: 1920,
            y: 0,
            w: 2560,
            h: 1440
        }
    );
}

#[test]
fn a_disabled_output_is_left_out() {
    let json = r#"{"X-1":{"name":"X-1","logical":null},"Y-1":{"name":"Y-1","logical":{"x":0,"y":0,"width":10,"height":10}}}"#;
    let outputs = niri::parse_outputs(json).expect("outputs");
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].name, "Y-1");
}

#[test]
fn replies_unwrap_or_say_why_not() {
    assert_eq!(
        niri::unwrap_reply(r#"{"Ok":{"Outputs":{}}}"#, "Outputs").expect("ok"),
        serde_json::json!({})
    );
    assert_eq!(
        niri::unwrap_reply(r#"{"Err":"nope"}"#, "Outputs"),
        Err(NiriError::Refused("nope".to_owned()))
    );
    assert!(matches!(
        niri::unwrap_reply(r#"{"Ok":{"Workspaces":[]}}"#, "Outputs"),
        Err(NiriError::Protocol(_))
    ));
}

#[test]
fn the_client_speaks_one_line_each_way() {
    let dir = std::env::temp_dir().join(format!("selenita-niri-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    let socket = dir.join("niri.sock");
    let listener = UnixListener::bind(&socket).expect("listen");
    let compact: serde_json::Value = serde_json::from_str(FOCUSED).expect("fixture");
    let reply = format!("{{\"Ok\":{{\"FocusedWindow\":{compact}}}}}\n");
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        let mut request = String::new();
        BufReader::new(&stream)
            .read_line(&mut request)
            .expect("request");
        (&stream).write_all(reply.as_bytes()).expect("reply");
        request
    });
    let window = niri::focused_window(&socket).expect("the fake niri");
    assert_eq!(server.join().expect("server"), "\"FocusedWindow\"\n");
    assert_eq!(window.app_id, "com.anthropic.Claude");
    assert!(matches!(
        niri::outputs(&dir.join("absent.sock")),
        Err(NiriError::Io(_))
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_window_meant_is_the_latest_focused_that_is_not_selenita() {
    let windows = niri::parse_windows(WINDOWS).expect("the fixture");
    assert_eq!(windows.len(), 2);
    // With the focused window standing in for Selenita, the one before it.
    let meant = niri::pick_window(&windows, "com.anthropic.Claude").expect("another");
    assert_eq!(meant.id, 45);
    // With Selenita absent, the focused window.
    assert_eq!(
        niri::pick_window(&windows, "org.celestina.Selenita").map(|w| w.id),
        Some(2)
    );
    assert_eq!(
        niri::pick_window(&windows[..1], "com.anthropic.Claude"),
        None
    );
}

#[test]
fn a_window_finds_its_output_through_its_workspace() {
    let windows = niri::parse_windows(WINDOWS).expect("windows");
    let workspaces = niri::parse_workspaces(WORKSPACES).expect("workspaces");
    assert!(!workspaces.is_empty());
    let expected = workspaces
        .iter()
        .find(|workspace| Some(workspace.id) == windows[0].workspace_id)
        .and_then(|workspace| workspace.output.clone());
    assert!(expected.is_some());
    assert_eq!(
        niri::output_of(&windows[0], &workspaces).map(str::to_owned),
        expected
    );
}
