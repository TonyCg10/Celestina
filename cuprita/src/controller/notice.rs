//! The notice every controller raises for the window's pill.

/// What a notice is about. The window styles the pill by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // `Info` is raised once a later unit reports completed actions.
pub enum NoticeKind {
    Info,
    Error,
}

impl NoticeKind {
    /// The token the `notice(kind, text)` signal carries.
    #[must_use]
    pub fn token(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Error => "error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NoticeKind;

    #[test]
    fn tokens_are_stable() {
        assert_eq!(NoticeKind::Info.token(), "info");
        assert_eq!(NoticeKind::Error.token(), "error");
    }
}
