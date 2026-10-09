// language-contract: product-copy
//! The page field's grammar: what a person may type to go to a page.
//!
//! `12` is a page, `+3` and `-2` are steps from the current one, and `inicio`
//! and `fin` are the first and the last.

/// One request typed in the page field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GoTo {
    Absolute(u32),
    Relative(i32),
    First,
    Last,
}

/// Why a request names no page of this document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GoToError {
    /// The text is neither a number, a step nor one of the two words.
    NotAPage,
    /// The page it names is outside `1..=count`.
    OutOfRange,
}

impl GoToError {
    /// What the window says, in Spanish.
    #[must_use]
    pub fn message_es(self) -> &'static str {
        match self {
            Self::NotAPage => "Escribe un número de página, +n, -n, «inicio» o «fin».",
            Self::OutOfRange => "Esa página no existe en este documento.",
        }
    }
}

impl std::fmt::Display for GoToError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAPage => formatter.write_str("the text names no page"),
            Self::OutOfRange => formatter.write_str("the page is out of range"),
        }
    }
}

impl std::error::Error for GoToError {}

impl GoTo {
    /// The request `text` spells.
    ///
    /// # Errors
    ///
    /// [`GoToError::NotAPage`] when it spells none.
    pub fn read(text: &str) -> Result<Self, GoToError> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("inicio") {
            return Ok(Self::First);
        }
        if text.eq_ignore_ascii_case("fin") {
            return Ok(Self::Last);
        }
        if let Some(step) = text.strip_prefix('+') {
            return digits(step)
                .and_then(|value| i32::try_from(value).ok())
                .map(Self::Relative)
                .ok_or(GoToError::NotAPage);
        }
        if let Some(step) = text.strip_prefix('-') {
            return digits(step)
                .and_then(|value| i32::try_from(value).ok())
                .map(|value| Self::Relative(-value))
                .ok_or(GoToError::NotAPage);
        }
        digits(text).map(Self::Absolute).ok_or(GoToError::NotAPage)
    }

    /// The 1-based page `self` names from page `current` of `count`.
    ///
    /// # Errors
    ///
    /// [`GoToError::OutOfRange`] when that page is not in `1..=count`.
    pub fn resolve(self, current: u32, count: u32) -> Result<u32, GoToError> {
        let target = match self {
            Self::Absolute(page) => i64::from(page),
            Self::Relative(step) => i64::from(current) + i64::from(step),
            Self::First => 1,
            Self::Last => i64::from(count),
        };
        if target >= 1 && target <= i64::from(count) {
            u32::try_from(target).map_err(|_| GoToError::OutOfRange)
        } else {
            Err(GoToError::OutOfRange)
        }
    }

    /// [`GoTo::read`] then [`GoTo::resolve`].
    ///
    /// # Errors
    ///
    /// Either step's [`GoToError`].
    pub fn parse(text: &str, current: u32, count: u32) -> Result<u32, GoToError> {
        Self::read(text)?.resolve(current, count)
    }
}

fn digits(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}
