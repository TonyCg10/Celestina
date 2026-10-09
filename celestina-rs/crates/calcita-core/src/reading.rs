//! Where a document was left: the page and the zoom it is reopened at, and
//! whether it was being read in the dark reading mode.

use crate::zoom::ZoomMode;

/// The reading position restored per document.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    /// 1-based.
    pub page: u32,
    pub zoom: ZoomMode,
    /// The dark reading mode (the pages inverted) was on.
    pub dark: bool,
}

impl Default for Reading {
    fn default() -> Self {
        Self {
            page: 1,
            zoom: ZoomMode::FitWidth,
            dark: false,
        }
    }
}
