use calcita_core::zoom::{zoom_in, zoom_out, ZoomMode, LADDER};

#[test]
fn zoom_in_takes_the_next_rung() {
    assert_eq!(zoom_in(1.0), 1.1);
    assert_eq!(zoom_in(0.5), 0.67);
}

#[test]
fn zoom_in_stops_at_the_top() {
    assert_eq!(zoom_in(4.0), 4.0);
    assert_eq!(zoom_in(9.0), 4.0);
}

#[test]
fn zoom_out_stops_at_the_bottom() {
    assert_eq!(zoom_out(0.5), 0.5);
    assert_eq!(zoom_out(0.1), 0.5);
}

#[test]
fn a_factor_between_rungs_moves_to_the_neighbouring_rung() {
    assert_eq!(zoom_in(1.03), 1.1);
    assert_eq!(zoom_out(1.03), 1.0);
}

#[test]
fn the_ladder_has_twelve_ascending_rungs() {
    assert_eq!(LADDER.len(), 12);
    assert!(LADDER.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn the_mode_round_trips_through_its_word() {
    for mode in [ZoomMode::FitWidth, ZoomMode::FitPage, ZoomMode::Free(1.25)] {
        assert_eq!(ZoomMode::parse(&mode.to_word()), Some(mode));
    }
    assert_eq!(ZoomMode::parse("free:abc"), None);
}
