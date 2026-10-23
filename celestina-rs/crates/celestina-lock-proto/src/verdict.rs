//! What one authentication attempt came to, and the verifier's exit codes
//! (design §4.3 (c)).

/// The verdict of one attempt. The verifier's exit status is the whole
/// message: no text travels, so no PAM text can reach the lock's log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// `pam_authenticate` and `pam_acct_mgmt` both succeeded (exit 0). The
    /// only verdict that can end a lock.
    ///
    /// This is a plain variant; the lock keeps the rule "only the verdict of
    /// the current attempt unlocks" with its own zero-sized token, minted in
    /// one place from exit status 0 of the child it spawned (design §4.5).
    Authenticated,
    /// The stack said no: wrong password, unknown user, too many tries (1).
    Refused,
    /// PAM could not decide, or the child failed, hung or was killed (2).
    Unavailable,
    /// The password was right but the account may not log in (3).
    AccountRefused,
    /// The stack asked for something the lock cannot answer (4).
    Unsupported,
    /// Neither file of the production service exists (5).
    ServiceMissing,
    /// The request itself was malformed (6).
    BadRequest,
}

impl Verdict {
    /// The verdict of a finished child. `code` is its exit status when it
    /// exited, `signaled` is true when a signal ended it. A child with no
    /// status, a signal or a status outside the table is [`Unavailable`]
    /// (design §4.3 (c): "a signal, any other code, a timeout"), never a
    /// refusal and never an authentication.
    ///
    /// [`Unavailable`]: Verdict::Unavailable
    #[must_use]
    pub const fn from_exit(code: Option<i32>, signaled: bool) -> Self {
        if signaled {
            return Self::Unavailable;
        }
        match code {
            Some(0) => Self::Authenticated,
            Some(1) => Self::Refused,
            Some(3) => Self::AccountRefused,
            Some(4) => Self::Unsupported,
            Some(5) => Self::ServiceMissing,
            Some(6) => Self::BadRequest,
            _ => Self::Unavailable,
        }
    }

    /// The exit status that stands for this verdict.
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Authenticated => 0,
            Self::Refused => 1,
            Self::Unavailable => 2,
            Self::AccountRefused => 3,
            Self::Unsupported => 4,
            Self::ServiceMissing => 5,
            Self::BadRequest => 6,
        }
    }

    /// Whether this is the one verdict that may end the lock.
    #[must_use]
    pub const fn is_authenticated(self) -> bool {
        matches!(self, Self::Authenticated)
    }

    /// Whether the verdict counts for the progressive delay (decision H-6):
    /// the two explicit refusals do, an unavailable or unsupported stack
    /// does not.
    #[must_use]
    pub const fn counts_as_failure(self) -> bool {
        matches!(self, Self::Refused | Self::AccountRefused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Verdict; 7] = [
        Verdict::Authenticated,
        Verdict::Refused,
        Verdict::Unavailable,
        Verdict::AccountRefused,
        Verdict::Unsupported,
        Verdict::ServiceMissing,
        Verdict::BadRequest,
    ];

    #[test]
    fn verdict_from_every_exit_code_and_signal() {
        let table = [
            (0, Verdict::Authenticated),
            (1, Verdict::Refused),
            (2, Verdict::Unavailable),
            (3, Verdict::AccountRefused),
            (4, Verdict::Unsupported),
            (5, Verdict::ServiceMissing),
            (6, Verdict::BadRequest),
        ];
        for (code, verdict) in table {
            assert_eq!(Verdict::from_exit(Some(code), false), verdict, "{code}");
            assert_eq!(verdict.exit_code(), code, "{verdict:?}");
            // A child killed by a signal never yields a verdict of its own,
            // whatever status the platform reports next to the signal.
            assert_eq!(
                Verdict::from_exit(Some(code), true),
                Verdict::Unavailable,
                "signalled {code}"
            );
        }
        for code in [7, 8, 42, 127, 128 + 9, 255, 256, -1, i32::MIN, i32::MAX] {
            assert_eq!(
                Verdict::from_exit(Some(code), false),
                Verdict::Unavailable,
                "{code}"
            );
        }
        assert_eq!(Verdict::from_exit(None, false), Verdict::Unavailable);
        assert_eq!(Verdict::from_exit(None, true), Verdict::Unavailable);
    }

    #[test]
    fn only_a_clean_zero_is_authenticated() {
        for verdict in ALL {
            assert_eq!(
                verdict.is_authenticated(),
                verdict == Verdict::Authenticated,
                "{verdict:?}"
            );
        }
        for code in -300..300 {
            for signaled in [false, true] {
                let verdict = Verdict::from_exit(Some(code), signaled);
                assert_eq!(
                    verdict.is_authenticated(),
                    code == 0 && !signaled,
                    "{code} {signaled}"
                );
            }
        }
    }

    #[test]
    fn only_the_two_explicit_refusals_count_for_the_delay() {
        for verdict in ALL {
            let counts = matches!(verdict, Verdict::Refused | Verdict::AccountRefused);
            assert_eq!(verdict.counts_as_failure(), counts, "{verdict:?}");
        }
    }
}
