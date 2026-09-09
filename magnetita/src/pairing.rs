//! The desktop half of QR pairing: the daemon arms a window and returns the
//! `magnetita://pair` text; the app draws it as a QR and knows when the
//! phone it was meant for has arrived.
//!
//! The QR is projected as rows of `#` and `.`, one character per module, so
//! the surface draws it with plain rectangles and depends on no image
//! plugin. The daemon's window is two minutes; the surface counts it down.

use std::collections::BTreeSet;

use qrcode::{EcLevel, QrCode};

/// The seconds the daemon keeps one QR valid.
pub const WINDOW_SECONDS: i32 = 120;

/// The QR of `uri` as rows of `#` (dark) and `.` (light), quiet zone
/// excluded; the surface adds its own margin.
pub fn matrix(uri: &str) -> Result<String, String> {
    let code = QrCode::with_error_correction_level(uri, EcLevel::M)
        .map_err(|error| format!("QR: {error}"))?;
    let width = code.width();
    let cells = code.to_colors();
    let rows: Vec<String> = cells
        .chunks(width)
        .map(|row| {
            row.iter()
                .map(|cell| {
                    if *cell == qrcode::Color::Dark {
                        '#'
                    } else {
                        '.'
                    }
                })
                .collect()
        })
        .collect();
    Ok(rows.join("\n"))
}

/// The paired device ids seen when the window opened; the first paired id
/// outside that set is the phone the QR admitted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    known: BTreeSet<String>,
}

impl Watch {
    pub fn open<'a>(paired_ids: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            known: paired_ids.into_iter().map(str::to_owned).collect(),
        }
    }

    /// The name of a newly paired device, if one appeared.
    pub fn arrived<'a>(
        &self,
        paired: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Option<String> {
        paired
            .into_iter()
            .find(|(id, _)| !self.known.contains(*id))
            .map(|(_, name)| name.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_matrix_is_square_with_the_finder_pattern_in_the_corner() {
        let text =
            matrix("magnetita://pair?v=1&id=abc&fp=00&secret=11&addr=10.0.0.1:1760").unwrap();
        let rows: Vec<&str> = text.lines().collect();
        assert!(rows.len() >= 21);
        assert!(rows.iter().all(|row| row.len() == rows.len()));
        assert!(rows[0].starts_with("#######."));
        assert!(matrix("").is_ok());
    }

    #[test]
    fn the_watch_names_only_a_device_that_was_not_paired_before() {
        let watch = Watch::open(["old"]);
        assert_eq!(watch.arrived([("old", "Old phone")]), None);
        assert_eq!(
            watch.arrived([("old", "Old phone"), ("new", "SM-S938U")]),
            Some("SM-S938U".into())
        );
    }
}
