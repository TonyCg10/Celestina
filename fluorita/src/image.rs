//! Whether an image is safe to show, decided before anything is decoded.
//!
//! Fluorita views photos with what the toolkit already has. That is the
//! contract — the media backend is for media that moves, and starting a decoder
//! for a still would cost a GPU context, a render context and a thread for a
//! picture Qt reads by itself.
//!
//! What the toolkit does *not* do is refuse. A file's header can claim any
//! dimensions it likes, and a 30 000 × 30 000 PNG is a gigabyte of RGBA before
//! anything is drawn, so the budget is checked here first and an image that
//! exceeds it is reported honestly instead of taking the session down.

/// The largest file Fluorita will hand to the toolkit's reader. Generous
/// enough for a raw photograph, bounded enough that a hostile file cannot make
/// the app read a disk into memory.
pub const MAX_BYTES: u64 = 256 * 1024 * 1024;

/// The largest decoded surface, in pixels. At 4 bytes per pixel this is a
/// ~400 MiB allocation, which is already far past any photograph and well past
/// what a window can show.
pub const MAX_PIXELS: u64 = 100_000_000;

/// What the viewer should do with one image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageDecision {
    /// Safe to decode, at its measured size.
    Show { width: u32, height: u32 },
    /// The toolkit cannot read it, or it would not say how big it is.
    Unreadable,
    /// Readable, but past a budget. The message names which one, because
    /// "no se pudo abrir" for a file that is merely enormous is a lie.
    TooLarge { reason: TooLarge },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TooLarge {
    Bytes { actual: u64 },
    Pixels { actual: u64 },
}

impl ImageDecision {
    /// Judges one image from its file size and its probed dimensions.
    ///
    /// `probed` is `None` when the toolkit could not read the header at all.
    #[must_use]
    pub fn judge(bytes: u64, probed: Option<(u32, u32)>) -> Self {
        if bytes > MAX_BYTES {
            return Self::TooLarge {
                reason: TooLarge::Bytes { actual: bytes },
            };
        }
        let Some((width, height)) = probed else {
            return Self::Unreadable;
        };
        if width == 0 || height == 0 {
            return Self::Unreadable;
        }
        let pixels = u64::from(width) * u64::from(height);
        if pixels > MAX_PIXELS {
            return Self::TooLarge {
                reason: TooLarge::Pixels { actual: pixels },
            };
        }
        Self::Show { width, height }
    }

    /// The Spanish sentence the viewer shows, or empty when there is nothing
    /// to say because the image is fine.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Show { .. } => String::new(),
            Self::Unreadable => "No se pudo leer esta imagen".to_owned(),
            Self::TooLarge {
                reason: TooLarge::Bytes { actual },
            } => format!(
                "La imagen ocupa {} MiB y el límite son {} MiB",
                actual / (1024 * 1024),
                MAX_BYTES / (1024 * 1024)
            ),
            Self::TooLarge {
                reason: TooLarge::Pixels { actual },
            } => format!(
                "La imagen tiene {} megapíxeles y el límite son {}",
                actual / 1_000_000,
                MAX_PIXELS / 1_000_000
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageDecision, TooLarge, MAX_BYTES, MAX_PIXELS};

    #[test]
    fn an_ordinary_photograph_is_shown() {
        assert_eq!(
            ImageDecision::judge(8 * 1024 * 1024, Some((6_000, 4_000))),
            ImageDecision::Show {
                width: 6_000,
                height: 4_000
            }
        );
        assert!(ImageDecision::judge(1_000, Some((1, 1)))
            .message()
            .is_empty());
    }

    #[test]
    fn a_file_the_toolkit_cannot_read_says_so() {
        assert_eq!(ImageDecision::judge(1_000, None), ImageDecision::Unreadable);
        // A header that answers with a zero dimension is not a size.
        assert_eq!(
            ImageDecision::judge(1_000, Some((0, 4_000))),
            ImageDecision::Unreadable
        );
    }

    #[test]
    fn the_byte_budget_is_checked_before_the_header_is_trusted() {
        // Deliberately with dimensions that would pass: an enormous file is
        // refused without ever consulting what it claims to be.
        let decision = ImageDecision::judge(MAX_BYTES + 1, Some((10, 10)));
        assert!(matches!(
            decision,
            ImageDecision::TooLarge {
                reason: TooLarge::Bytes { .. }
            }
        ));
        assert!(decision.message().contains("MiB"));
    }

    #[test]
    fn a_small_file_claiming_an_enormous_surface_is_refused() {
        // The decompression bomb: a few kilobytes that would allocate a
        // gigabyte once decoded.
        let decision = ImageDecision::judge(64 * 1024, Some((40_000, 40_000)));
        assert!(matches!(
            decision,
            ImageDecision::TooLarge {
                reason: TooLarge::Pixels { .. }
            }
        ));
        assert!(decision.message().contains("megapíxeles"));
    }

    #[test]
    fn the_budgets_are_the_documented_ones() {
        assert_eq!(MAX_PIXELS, 100_000_000);
        assert_eq!(MAX_BYTES, 256 * 1024 * 1024);
        // Exactly at the limit is allowed; one past it is not.
        assert!(matches!(
            ImageDecision::judge(MAX_BYTES, Some((10_000, 10_000))),
            ImageDecision::Show { .. }
        ));
    }
}
