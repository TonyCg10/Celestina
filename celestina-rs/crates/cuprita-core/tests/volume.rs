use cuprita_core::volume::{clamp_volume, percent, VOLUME_MAX};

#[test]
fn clamps_to_the_range() {
    assert_eq!(clamp_volume(2.0), 1.5);
    assert_eq!(clamp_volume(-1.0), 0.0);
    assert_eq!(clamp_volume(f32::NAN), 0.0);
    assert_eq!(VOLUME_MAX, 1.5);
}

#[test]
fn percent_is_rounded() {
    assert_eq!(percent(0.5), 50);
    assert_eq!(percent(1.5), 150);
    assert_eq!(percent(9.0), 150);
}
