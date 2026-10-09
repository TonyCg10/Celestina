//! How large the pages are drawn: fitted to the window or a free factor that
//! walks a fixed ladder.

/// The factors zoom in and zoom out step through, ascending.
pub const LADDER: [f32; 12] = [
    0.5, 0.67, 0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 3.0, 4.0,
];

/// Below this distance two factors are the same rung: a factor read back
/// from the view carries rounding.
const SAME_RUNG: f32 = 0.005;

/// The zoom a window reads with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ZoomMode {
    /// The page's width fills the view.
    FitWidth,
    /// The whole page fits in the view.
    FitPage,
    /// A free factor, 1.0 being the page at its natural size.
    Free(f32),
}

impl ZoomMode {
    /// The word the store and the QML seam use: `width`, `page`, or
    /// `free:<factor>`.
    #[must_use]
    pub fn to_word(self) -> String {
        match self {
            Self::FitWidth => "width".to_owned(),
            Self::FitPage => "page".to_owned(),
            Self::Free(factor) => format!("free:{factor}"),
        }
    }

    /// The mode `word` spells, or `None` when it spells none.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "width" => Some(Self::FitWidth),
            "page" => Some(Self::FitPage),
            _ => {
                let factor: f32 = word.strip_prefix("free:")?.parse().ok()?;
                (factor.is_finite() && factor > 0.0).then(|| Self::Free(clamp(factor)))
            }
        }
    }
}

/// `factor` held inside the ladder's ends.
#[must_use]
pub fn clamp(factor: f32) -> f32 {
    factor.clamp(LADDER[0], LADDER[LADDER.len() - 1])
}

/// The next rung above `factor`, or the top rung when there is none.
#[must_use]
pub fn zoom_in(factor: f32) -> f32 {
    LADDER
        .iter()
        .copied()
        .find(|rung| *rung > factor + SAME_RUNG)
        .unwrap_or(LADDER[LADDER.len() - 1])
}

/// The next rung below `factor`, or the bottom rung when there is none.
#[must_use]
pub fn zoom_out(factor: f32) -> f32 {
    LADDER
        .iter()
        .rev()
        .copied()
        .find(|rung| *rung < factor - SAME_RUNG)
        .unwrap_or(LADDER[0])
}
