//! How much a container may unpack, and the one place that unpacks it.
//!
//! A compressed file says nothing trustworthy about its own size: a header's
//! declared length is whatever its writer typed, and a few kilobytes of
//! deflate data expand into gigabytes. The ceiling a host sets on a document
//! (`Limits::max_bytes`) therefore applies to what comes *out* of a container,
//! not only to the file on disk, and it applies to the document as a whole:
//! every member of a ZIP and every filter of a PDF stream draws on the same
//! [`Budget`].
//!
//! Every decoder in the crate reads through [`Budget::inflate`], which asks the
//! decoder for at most one byte more than the budget has left. That byte is how
//! an overrun is told apart from an exact fit, and it is the most the excess
//! can ever cost.

use std::fmt;
use std::io::{self, Read};

/// A decode that could not finish inside its budget, or at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InflateError {
    /// The content would pass the ceiling the budget was opened with.
    TooLarge { limit: u64 },
    /// The compressed data is not valid for its codec.
    Corrupt,
}

impl fmt::Display for InflateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { limit } => write!(
                formatter,
                "its content unpacks past the {limit}-byte ceiling"
            ),
            Self::Corrupt => formatter.write_str("its compressed data could not be read"),
        }
    }
}

impl std::error::Error for InflateError {}

/// The bytes a document may still unpack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Budget {
    limit: u64,
    spent: u64,
}

impl Budget {
    /// A budget of `limit` bytes, none of them spent.
    #[must_use]
    pub const fn new(limit: u64) -> Self {
        Self { limit, spent: 0 }
    }

    /// The ceiling this budget was opened with.
    #[must_use]
    pub const fn limit(&self) -> u64 {
        self.limit
    }

    /// What is left to spend.
    #[must_use]
    pub const fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.spent)
    }

    /// Records `bytes` of content that arrived without decoding, such as a
    /// stored ZIP member, refusing it when it does not fit.
    pub fn charge(&mut self, bytes: usize) -> Result<(), InflateError> {
        let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
        if bytes > self.remaining() {
            return Err(InflateError::TooLarge { limit: self.limit });
        }
        self.spent = self.spent.saturating_add(bytes);
        Ok(())
    }

    /// Everything `decoder` produces, charged to this budget.
    ///
    /// The decoder is read through `take(remaining + 1)`, so a bomb stops one
    /// byte past what is left and is refused as [`InflateError::TooLarge`]; the
    /// budget is charged only for content that fits.
    pub fn inflate(&mut self, decoder: impl Read) -> Result<Vec<u8>, InflateError> {
        self.inflate_at_most(decoder, u64::MAX)
    }

    /// Like [`Budget::inflate`], for content that also states its own size.
    ///
    /// The statement is believed only as a further bound: reading stops at the
    /// smaller of `declared` and what the budget has left. Content longer than
    /// `declared` is [`InflateError::Corrupt`] (the file contradicts itself)
    /// unless the budget ran out first, in which case it is
    /// [`InflateError::TooLarge`]. Nothing is ever allocated from `declared`.
    pub fn inflate_at_most(
        &mut self,
        decoder: impl Read,
        declared: u64,
    ) -> Result<Vec<u8>, InflateError> {
        let allowed = self.remaining().min(declared);
        let mut out = Vec::new();
        decoder
            .take(allowed.saturating_add(1))
            .read_to_end(&mut out)
            .map_err(|error| match error.kind() {
                // `read_to_end` reports an allocation it could not make rather
                // than aborting; for this reader that only happens on the way
                // to the ceiling, which is what the refusal says.
                io::ErrorKind::OutOfMemory => InflateError::TooLarge { limit: self.limit },
                _ => InflateError::Corrupt,
            })?;
        let produced = u64::try_from(out.len()).unwrap_or(u64::MAX);
        if produced > allowed {
            return Err(if allowed < declared {
                InflateError::TooLarge { limit: self.limit }
            } else {
                InflateError::Corrupt
            });
        }
        self.spent = self.spent.saturating_add(produced);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::read::DeflateDecoder;
    use flate2::write::DeflateEncoder;
    use flate2::Compression;

    use super::{Budget, InflateError};

    fn deflate(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).expect("compress");
        encoder.finish().expect("compress")
    }

    #[test]
    fn content_that_fits_is_returned_and_charged() {
        let packed = deflate(&[b'x'; 1000]);
        let mut budget = Budget::new(1500);
        let out = budget
            .inflate(DeflateDecoder::new(packed.as_slice()))
            .expect("fits");
        assert_eq!(out.len(), 1000);
        assert_eq!(budget.remaining(), 500);
    }

    #[test]
    fn an_exact_fit_is_not_an_overrun() {
        let packed = deflate(&[b'x'; 1000]);
        let mut budget = Budget::new(1000);
        assert_eq!(
            budget
                .inflate(DeflateDecoder::new(packed.as_slice()))
                .map(|out| out.len()),
            Ok(1000)
        );
        assert_eq!(budget.remaining(), 0);
    }

    #[test]
    fn one_byte_past_the_ceiling_is_too_large_and_charges_nothing() {
        let packed = deflate(&[b'x'; 1001]);
        let mut budget = Budget::new(1000);
        assert_eq!(
            budget.inflate(DeflateDecoder::new(packed.as_slice())),
            Err(InflateError::TooLarge { limit: 1000 })
        );
        assert_eq!(budget.remaining(), 1000);
    }

    #[test]
    fn the_budget_is_shared_by_every_decode_drawn_on_it() {
        let packed = deflate(&[b'x'; 600]);
        let mut budget = Budget::new(1000);
        budget
            .inflate(DeflateDecoder::new(packed.as_slice()))
            .expect("the first fits");
        assert_eq!(
            budget.inflate(DeflateDecoder::new(packed.as_slice())),
            Err(InflateError::TooLarge { limit: 1000 })
        );
        assert_eq!(budget.charge(400), Ok(()));
        assert_eq!(
            budget.charge(1),
            Err(InflateError::TooLarge { limit: 1000 })
        );
    }

    #[test]
    fn a_declared_size_bounds_the_read_but_is_never_believed_further() {
        let packed = deflate(&[b'x'; 1000]);

        // Content longer than its own header says is a contradiction.
        let mut budget = Budget::new(1 << 20);
        assert_eq!(
            budget.inflate_at_most(DeflateDecoder::new(packed.as_slice()), 10),
            Err(InflateError::Corrupt)
        );

        // A header that claims more than the budget holds is only as good as
        // the budget, and the content decides.
        let mut budget = Budget::new(2000);
        let out = budget
            .inflate_at_most(DeflateDecoder::new(packed.as_slice()), u64::from(u32::MAX))
            .expect("the content fits");
        assert_eq!(out.len(), 1000);

        let mut budget = Budget::new(500);
        assert_eq!(
            budget.inflate_at_most(DeflateDecoder::new(packed.as_slice()), u64::from(u32::MAX)),
            Err(InflateError::TooLarge { limit: 500 })
        );
    }

    #[test]
    fn data_that_is_not_deflate_is_corrupt() {
        let mut budget = Budget::new(1000);
        assert_eq!(
            budget.inflate(DeflateDecoder::new(&[0xFF_u8; 16][..])),
            Err(InflateError::Corrupt)
        );
    }
}
