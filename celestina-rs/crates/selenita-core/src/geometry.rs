//! A rectangle in the compositor's logical coordinates, in the form `grim -g`
//! takes and `slurp` prints: `x,y wxh`.

use std::fmt;
use std::str::FromStr;

/// A logical rectangle: the top-left corner and the size.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl fmt::Display for Geometry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{},{} {}x{}", self.x, self.y, self.w, self.h)
    }
}

/// Text that is not `x,y wxh` with a non-empty size.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseGeometryError(pub String);

impl fmt::Display for ParseGeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "not a geometry (x,y wxh): {:?}", self.0)
    }
}

impl std::error::Error for ParseGeometryError {}

impl FromStr for Geometry {
    type Err = ParseGeometryError;

    /// `x,y wxh`, surrounding blanks allowed (slurp ends its line with a
    /// newline). A zero width or height is refused: there is nothing to
    /// capture.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = || ParseGeometryError(text.to_owned());
        let (position, size) = text.trim().split_once(' ').ok_or_else(error)?;
        let (x, y) = position.split_once(',').ok_or_else(error)?;
        let (w, h) = size.trim().split_once('x').ok_or_else(error)?;
        let geometry = Self {
            x: x.parse().map_err(|_| error())?,
            y: y.parse().map_err(|_| error())?,
            w: w.parse().map_err(|_| error())?,
            h: h.parse().map_err(|_| error())?,
        };
        if geometry.w == 0 || geometry.h == 0 {
            return Err(error());
        }
        Ok(geometry)
    }
}
