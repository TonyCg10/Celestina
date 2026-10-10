use selenita_core::target::{delay_from_seconds, needs_settle, TargetKind};
use selenita_core::Geometry;
use std::time::Duration;

#[test]
fn a_geometry_round_trips_through_grims_form() {
    let geometry = Geometry {
        x: -1920,
        y: 120,
        w: 942,
        h: 1010,
    };
    assert_eq!(geometry.to_string(), "-1920,120 942x1010");
    assert_eq!("-1920,120 942x1010".parse::<Geometry>(), Ok(geometry));
}

#[test]
fn slurps_line_with_its_newline_reads() {
    let geometry: Geometry = "10,20 300x400\n".parse().expect("slurp's output");
    assert_eq!(
        geometry,
        Geometry {
            x: 10,
            y: 20,
            w: 300,
            h: 400
        }
    );
}

#[test]
fn malformed_or_empty_geometries_are_refused() {
    for text in [
        "",
        "10,20",
        "10 20 3x4",
        "a,b 3x4",
        "1,2 0x4",
        "1,2 3x0",
        "1,2 -3x4",
    ] {
        assert!(text.parse::<Geometry>().is_err(), "{text:?}");
    }
}

#[test]
fn the_three_target_words_round_trip_and_nothing_else_reads() {
    for kind in TargetKind::ALL {
        assert_eq!(kind.as_str().parse::<TargetKind>(), Ok(kind));
    }
    assert!("Screen".parse::<TargetKind>().is_err());
    assert!("".parse::<TargetKind>().is_err());
}

#[test]
fn only_the_offered_delays_are_kept() {
    assert_eq!(delay_from_seconds(3), Duration::from_secs(3));
    assert_eq!(delay_from_seconds(10), Duration::from_secs(10));
    assert_eq!(delay_from_seconds(0), Duration::ZERO);
    assert_eq!(delay_from_seconds(7), Duration::ZERO);
    assert_eq!(delay_from_seconds(-3), Duration::ZERO);
}

#[test]
fn the_window_steps_aside_whenever_it_is_shown() {
    for kind in TargetKind::ALL {
        assert!(kind.hides_own_window(true));
        assert!(!kind.hides_own_window(false));
    }
}

#[test]
fn the_settle_waits_only_where_selenita_was() {
    use TargetKind::{Region, Screen, Window};
    assert!(needs_settle(Region, Some("DP-2"), Some("DP-1")));
    assert!(needs_settle(Screen, Some(""), Some("DP-1")));
    assert!(needs_settle(Screen, Some("DP-1"), Some("DP-1")));
    assert!(!needs_settle(Screen, Some("DP-2"), Some("DP-1")));
    assert!(!needs_settle(Window, Some("DP-2"), Some("DP-1")));
    assert!(needs_settle(Window, None, Some("DP-1")));
    assert!(needs_settle(Window, Some("DP-2"), None));
}
