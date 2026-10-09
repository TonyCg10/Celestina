//! Calcita's side of the suite's appearance file.
//!
//! `celestina_settings` owns the file (`~/.config/celestina/appearance.toml`),
//! its watcher, the follower thread and the `CELESTINA_REDUCED_MOTION`
//! override. This module only reads the first value for the controller and
//! starts the follower that hands every later value to it.

use celestina_settings::{Appearance, Follower};

/// The two values the window binds into `CelestinaAppearance`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Values {
    pub reduced_motion: bool,
    pub text_scale: f64,
}

impl From<Appearance> for Values {
    fn from(value: Appearance) -> Self {
        Self {
            reduced_motion: value.reduced_motion,
            text_scale: value.text_scale.factor(),
        }
    }
}

/// The appearance as stored now: a bounded read of a two-line file, done
/// before the window's first frame so the theme starts right.
pub fn initial() -> Values {
    celestina_settings::load().into()
}

/// Starts following the file; `deliver` runs on the follower thread and must
/// queue to Qt itself. Dropping the follower stops it.
pub fn follow(deliver: impl Fn(Values) + Clone + Send + 'static) -> Follower {
    celestina_settings::follow(move |value: Appearance| deliver(value.into()))
}

#[cfg(test)]
mod tests {
    use super::Values;
    use celestina_settings::{Appearance, TextScale};

    #[test]
    fn the_text_scale_crosses_as_its_factor() {
        let values = Values::from(Appearance {
            reduced_motion: true,
            text_scale: TextScale::Large,
        });
        assert!(values.reduced_motion);
        assert_eq!(values.text_scale, TextScale::Large.factor());
    }
}
