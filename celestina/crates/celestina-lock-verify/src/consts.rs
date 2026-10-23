//! The `libpam` constants the verifier uses, with the values of the installed
//! header (design §4.11). The test `t_v5_pam_constants_match_the_installed_header`
//! reads `/usr/include/security/_pam_types.h` and fails if any value differs.
//!
//! They are plain numbers, so they live outside `pam.rs` and the pure modules
//! (`conversation`, `outcome`, `session`) can use them.
#![forbid(unsafe_code)]

pub const PAM_SUCCESS: i32 = 0;
pub const PAM_SYSTEM_ERR: i32 = 4;
pub const PAM_BUF_ERR: i32 = 5;
pub const PAM_PERM_DENIED: i32 = 6;
pub const PAM_AUTH_ERR: i32 = 7;
pub const PAM_CRED_INSUFFICIENT: i32 = 8;
pub const PAM_AUTHINFO_UNAVAIL: i32 = 9;
pub const PAM_USER_UNKNOWN: i32 = 10;
pub const PAM_MAXTRIES: i32 = 11;
pub const PAM_NEW_AUTHTOK_REQD: i32 = 12;
pub const PAM_ACCT_EXPIRED: i32 = 13;
pub const PAM_CONV_ERR: i32 = 19;
pub const PAM_ABORT: i32 = 26;

pub const PAM_PROMPT_ECHO_OFF: i32 = 1;
pub const PAM_PROMPT_ECHO_ON: i32 = 2;
pub const PAM_ERROR_MSG: i32 = 3;
pub const PAM_TEXT_INFO: i32 = 4;

pub const PAM_DISALLOW_NULL_AUTHTOK: i32 = 0x0001;

pub const PAM_MAX_NUM_MSG: usize = 32;
pub const PAM_MAX_RESP_SIZE: usize = 512;

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "/usr/include/security/_pam_types.h";

    /// The value of `#define NAME value` in the header text, in decimal or
    /// hexadecimal, through at most one level of `#define NAME OTHER`.
    fn define(text: &str, name: &str) -> Option<i64> {
        for line in text.lines() {
            let mut words = line.split_whitespace();
            if words.next() != Some("#define") || words.next() != Some(name) {
                continue;
            }
            let token = words.next()?;
            let token = token
                .trim_matches(|c| c == '(' || c == ')')
                .trim_end_matches(['U', 'u', 'L', 'l']);
            if let Some(hex) = token.strip_prefix("0x") {
                return i64::from_str_radix(hex, 16).ok();
            }
            if let Ok(value) = token.parse() {
                return Some(value);
            }
            return define(text, token);
        }
        None
    }

    #[test]
    fn t_v5_pam_constants_match_the_installed_header() {
        let Ok(text) = std::fs::read_to_string(HEADER) else {
            // The header is a build-time development file; without it the
            // comparison cannot run, and says so instead of passing silently.
            eprintln!("skipped: {HEADER} is absent");
            return;
        };
        let table: [(&str, i64); 19] = [
            ("PAM_SUCCESS", i64::from(PAM_SUCCESS)),
            ("PAM_SYSTEM_ERR", i64::from(PAM_SYSTEM_ERR)),
            ("PAM_BUF_ERR", i64::from(PAM_BUF_ERR)),
            ("PAM_PERM_DENIED", i64::from(PAM_PERM_DENIED)),
            ("PAM_AUTH_ERR", i64::from(PAM_AUTH_ERR)),
            ("PAM_CRED_INSUFFICIENT", i64::from(PAM_CRED_INSUFFICIENT)),
            ("PAM_AUTHINFO_UNAVAIL", i64::from(PAM_AUTHINFO_UNAVAIL)),
            ("PAM_USER_UNKNOWN", i64::from(PAM_USER_UNKNOWN)),
            ("PAM_MAXTRIES", i64::from(PAM_MAXTRIES)),
            ("PAM_NEW_AUTHTOK_REQD", i64::from(PAM_NEW_AUTHTOK_REQD)),
            ("PAM_ACCT_EXPIRED", i64::from(PAM_ACCT_EXPIRED)),
            ("PAM_CONV_ERR", i64::from(PAM_CONV_ERR)),
            ("PAM_ABORT", i64::from(PAM_ABORT)),
            ("PAM_PROMPT_ECHO_OFF", i64::from(PAM_PROMPT_ECHO_OFF)),
            ("PAM_PROMPT_ECHO_ON", i64::from(PAM_PROMPT_ECHO_ON)),
            ("PAM_ERROR_MSG", i64::from(PAM_ERROR_MSG)),
            ("PAM_TEXT_INFO", i64::from(PAM_TEXT_INFO)),
            (
                "PAM_DISALLOW_NULL_AUTHTOK",
                i64::from(PAM_DISALLOW_NULL_AUTHTOK),
            ),
            ("PAM_MAX_NUM_MSG", PAM_MAX_NUM_MSG as i64),
        ];
        for (name, ours) in table {
            assert_eq!(define(&text, name), Some(ours), "{name}");
        }
        assert_eq!(
            define(&text, "PAM_MAX_RESP_SIZE"),
            Some(PAM_MAX_RESP_SIZE as i64),
            "PAM_MAX_RESP_SIZE"
        );
    }
}
