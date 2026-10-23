//! From the codes PAM returned to the verdict (design §4.3 (c) table).
//!
//! Pure. `Authenticated` is produced in exactly one arm: the authentication,
//! the account check and `pam_end` all returned `PAM_SUCCESS`, and the
//! conversation never refused a prompt. Every other combination is a refusal or
//! `Unavailable`, never an unlock.
#![forbid(unsafe_code)]

use celestina_lock_proto::Verdict;

use crate::consts::{
    PAM_ABORT, PAM_ACCT_EXPIRED, PAM_AUTHINFO_UNAVAIL, PAM_AUTH_ERR, PAM_BUF_ERR, PAM_CONV_ERR,
    PAM_CRED_INSUFFICIENT, PAM_MAXTRIES, PAM_NEW_AUTHTOK_REQD, PAM_PERM_DENIED, PAM_SUCCESS,
    PAM_SYSTEM_ERR, PAM_USER_UNKNOWN,
};

/// What one run of the PAM calls returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    /// `pam_authenticate`.
    pub auth: i32,
    /// `pam_acct_mgmt`, called only when `auth` succeeded.
    pub acct: Option<i32>,
    /// `pam_end`.
    pub end: i32,
    /// The conversation refused a prompt it must not answer.
    pub unsupported: bool,
}

/// The verdict a report stands for.
#[must_use]
pub const fn verdict(report: Report) -> Verdict {
    if report.auth != PAM_SUCCESS {
        if report.unsupported {
            return Verdict::Unsupported;
        }
        return match report.auth {
            PAM_AUTH_ERR
            | PAM_USER_UNKNOWN
            | PAM_MAXTRIES
            | PAM_CRED_INSUFFICIENT
            | PAM_PERM_DENIED => Verdict::Refused,
            // The stack could not decide (the table's second row), spelled
            // out; any other code is the same by the table's last row.
            PAM_SYSTEM_ERR | PAM_BUF_ERR | PAM_ABORT | PAM_AUTHINFO_UNAVAIL | PAM_CONV_ERR => {
                Verdict::Unavailable
            }
            _ => Verdict::Unavailable,
        };
    }
    if report.unsupported {
        return Verdict::Unsupported;
    }
    match report.acct {
        Some(PAM_SUCCESS) if report.end == PAM_SUCCESS => Verdict::Authenticated,
        Some(PAM_NEW_AUTHTOK_REQD | PAM_ACCT_EXPIRED | PAM_PERM_DENIED | PAM_AUTH_ERR) => {
            Verdict::AccountRefused
        }
        _ => Verdict::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(auth: i32, acct: Option<i32>, end: i32, unsupported: bool) -> Report {
        Report {
            auth,
            acct,
            end,
            unsupported,
        }
    }

    #[test]
    fn authenticated_needs_auth_acct_and_end_all_successful() {
        assert_eq!(
            verdict(report(0, Some(0), 0, false)),
            Verdict::Authenticated
        );
        // pam_end failing after a good check is not an unlock.
        assert_eq!(
            verdict(report(0, Some(0), PAM_SYSTEM_ERR, false)),
            Verdict::Unavailable
        );
        // Success without an account check never happens, and is not an unlock.
        assert_eq!(verdict(report(0, None, 0, false)), Verdict::Unavailable);
        // A refused prompt is never an unlock, whatever the codes say.
        assert_eq!(verdict(report(0, Some(0), 0, true)), Verdict::Unsupported);
    }

    #[test]
    fn explicit_refusals_in_authentication() {
        for code in [
            PAM_AUTH_ERR,
            PAM_USER_UNKNOWN,
            PAM_MAXTRIES,
            PAM_CRED_INSUFFICIENT,
            PAM_PERM_DENIED,
        ] {
            assert_eq!(
                verdict(report(code, None, 0, false)),
                Verdict::Refused,
                "{code}"
            );
        }
    }

    #[test]
    fn every_other_authentication_code_is_unavailable() {
        for code in [
            PAM_SYSTEM_ERR,
            PAM_BUF_ERR,
            PAM_ABORT,
            PAM_AUTHINFO_UNAVAIL,
            PAM_CONV_ERR,
            PAM_NEW_AUTHTOK_REQD,
            PAM_ACCT_EXPIRED,
            1,
            2,
            3,
            14,
            100,
            -1,
            i32::MAX,
        ] {
            assert_eq!(
                verdict(report(code, None, 0, false)),
                Verdict::Unavailable,
                "{code}"
            );
        }
    }

    #[test]
    fn a_refused_prompt_makes_the_verdict_unsupported_not_unavailable() {
        assert_eq!(
            verdict(report(PAM_CONV_ERR, None, 0, true)),
            Verdict::Unsupported
        );
        assert_eq!(
            verdict(report(PAM_AUTH_ERR, None, 0, true)),
            Verdict::Unsupported
        );
    }

    #[test]
    fn account_refusals_after_a_good_authentication() {
        for code in [
            PAM_NEW_AUTHTOK_REQD,
            PAM_ACCT_EXPIRED,
            PAM_PERM_DENIED,
            PAM_AUTH_ERR,
        ] {
            assert_eq!(
                verdict(report(0, Some(code), 0, false)),
                Verdict::AccountRefused,
                "{code}"
            );
        }
        for code in [PAM_SYSTEM_ERR, PAM_AUTHINFO_UNAVAIL, PAM_USER_UNKNOWN, 99] {
            assert_eq!(
                verdict(report(0, Some(code), 0, false)),
                Verdict::Unavailable,
                "{code}"
            );
        }
    }

    #[test]
    fn only_one_combination_in_a_sweep_authenticates() {
        let mut winners = 0;
        for auth in -2..30 {
            for acct in [None].into_iter().chain((-2..30).map(Some)) {
                for end in [0, 4, 19] {
                    for unsupported in [false, true] {
                        let v = verdict(report(auth, acct, end, unsupported));
                        if v == Verdict::Authenticated {
                            winners += 1;
                            assert_eq!((auth, acct, end, unsupported), (0, Some(0), 0, false));
                        }
                    }
                }
            }
        }
        assert_eq!(winners, 1);
    }
}
