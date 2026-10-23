//! One authentication, in the order the design fixes (design §4.11): start,
//! authenticate with `PAM_DISALLOW_NULL_AUTHTOK`, account management, the
//! secret zeroed, end.
//!
//! The four `libpam` calls sit behind [`PamApi`]. `pam.rs` implements it with
//! the real library; the tests implement it with a scripted stack, so the flow,
//! the flags, the order and the code-to-verdict mapping are tested without
//! `libpam`, without `/etc` and without a password.
//!
//! No `pam_setcred`, no `pam_open_session`, no `pam_chauthtok`: the trait has
//! no method for them.
#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex, PoisonError};

use celestina_lock_proto::{Service, Verdict};
use zeroize::Zeroizing;

use crate::consts::{PAM_DISALLOW_NULL_AUTHTOK, PAM_SUCCESS};
use crate::conversation::ConvState;
use crate::outcome::{verdict, Report};

/// The conversation state both sides reach: the session through its `Arc`, the
/// conversation callback through the pointer PAM hands back.
pub type SharedConv = Arc<Mutex<ConvState>>;

/// The four `libpam` calls the verifier makes.
pub trait PamApi {
    /// What `pam_start` returns and the other calls take.
    type Handle;

    /// `pam_start(service, user, conv, &handle)`. The implementation keeps
    /// `conv` alive until [`PamApi::end`] and calls the conversation policy
    /// through it. `Err` carries the PAM code; the handle is not valid then.
    ///
    /// # Errors
    ///
    /// The `pam_start` return code when it is not `PAM_SUCCESS`.
    fn start(&mut self, service: &str, user: &str, conv: SharedConv) -> Result<Self::Handle, i32>;

    /// `pam_authenticate(handle, flags)`.
    fn authenticate(&mut self, handle: &mut Self::Handle, flags: i32) -> i32;

    /// `pam_acct_mgmt(handle, flags)`.
    fn acct_mgmt(&mut self, handle: &mut Self::Handle, flags: i32) -> i32;

    /// `pam_end(handle, status)`; consumes the handle.
    fn end(&mut self, handle: Self::Handle, status: i32) -> i32;
}

/// Run one attempt for `user` with `secret` and return its verdict.
///
/// The secret is owned here and dropped (zeroed) on every path: before
/// `pam_end` when the stack was started, at once when it was not.
pub fn authenticate<P: PamApi>(
    api: &mut P,
    service: Service,
    user: &str,
    secret: Zeroizing<Vec<u8>>,
) -> Verdict {
    let conv: SharedConv = Arc::new(Mutex::new(ConvState::new(secret)));
    let mut handle = match api.start(service.name(), user, Arc::clone(&conv)) {
        Ok(handle) => handle,
        Err(_code) => {
            lock(&conv).drop_secret();
            return Verdict::Unavailable;
        }
    };
    let auth = api.authenticate(&mut handle, PAM_DISALLOW_NULL_AUTHTOK);
    let acct = (auth == PAM_SUCCESS).then(|| api.acct_mgmt(&mut handle, 0));
    // The secret is gone before the library tears its state down.
    let unsupported = {
        let mut state = lock(&conv);
        state.drop_secret();
        state.conversation.unsupported()
    };
    let last = acct.unwrap_or(auth);
    let end = api.end(handle, last);
    verdict(Report {
        auth,
        acct,
        end,
        unsupported,
    })
}

fn lock(conv: &SharedConv) -> std::sync::MutexGuard<'_, ConvState> {
    // A poisoned lock means a panic in the conversation, which aborts the
    // process before it can get here; recovering the guard is still safe.
    conv.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::{
        PAM_ABORT, PAM_ACCT_EXPIRED, PAM_AUTH_ERR, PAM_CONV_ERR, PAM_ERROR_MSG,
        PAM_PROMPT_ECHO_OFF, PAM_PROMPT_ECHO_ON, PAM_SYSTEM_ERR, PAM_TEXT_INFO, PAM_USER_UNKNOWN,
    };
    use crate::conversation::Reply;

    /// A scripted stack: it plays `prompts` (styles per conversation call)
    /// through the real policy, then returns `auth`/`acct`/`end`, and records
    /// every call it received.
    struct Fake {
        start: Result<(), i32>,
        calls: Vec<String>,
        prompts: Vec<Vec<i32>>,
        replies: Vec<Result<Vec<Reply>, crate::conversation::ConvError>>,
        conv: Option<SharedConv>,
        auth: i32,
        acct: i32,
        end: i32,
        /// What the secret looked like when `pam_end` ran.
        secret_at_end: Option<bool>,
        secret_seen: Vec<Vec<u8>>,
    }

    impl Fake {
        fn new(auth: i32, acct: i32) -> Self {
            Self {
                start: Ok(()),
                calls: Vec::new(),
                prompts: vec![vec![PAM_PROMPT_ECHO_OFF]],
                replies: Vec::new(),
                conv: None,
                auth,
                acct,
                end: PAM_SUCCESS,
                secret_at_end: None,
                secret_seen: Vec::new(),
            }
        }
    }

    impl PamApi for Fake {
        type Handle = ();

        fn start(&mut self, service: &str, user: &str, conv: SharedConv) -> Result<(), i32> {
            self.calls.push(format!("start {service} {user}"));
            self.conv = Some(conv);
            self.start
        }

        fn authenticate(&mut self, _: &mut (), flags: i32) -> i32 {
            self.calls.push(format!("authenticate {flags}"));
            let conv = self.conv.clone().expect("started");
            for styles in self.prompts.clone() {
                let mut state = conv.lock().unwrap();
                let out = state.conversation.respond(&styles);
                if let Ok(replies) = &out {
                    if replies.contains(&Reply::Secret) {
                        self.secret_seen
                            .push(state.secret_bytes().unwrap_or_default().to_vec());
                    }
                }
                self.replies.push(out);
            }
            self.auth
        }

        fn acct_mgmt(&mut self, _: &mut (), flags: i32) -> i32 {
            self.calls.push(format!("acct_mgmt {flags}"));
            self.acct
        }

        fn end(&mut self, _: (), status: i32) -> i32 {
            self.calls.push(format!("end {status}"));
            self.secret_at_end = self.conv.as_ref().map(|c| c.lock().unwrap().holds_secret());
            self.end
        }
    }

    fn secret() -> Zeroizing<Vec<u8>> {
        Zeroizing::new(b"correct horse".to_vec())
    }

    #[test]
    fn a_good_attempt_runs_start_authenticate_acct_end_in_that_order() {
        let mut fake = Fake::new(PAM_SUCCESS, PAM_SUCCESS);
        let v = authenticate(&mut fake, Service::Production, "alice", secret());
        assert_eq!(v, Verdict::Authenticated);
        assert_eq!(
            fake.calls,
            [
                "start celestina-lock alice",
                "authenticate 1",
                "acct_mgmt 0",
                "end 0"
            ]
        );
        assert_eq!(fake.secret_seen, [b"correct horse".to_vec()]);
    }

    #[test]
    fn authenticate_is_called_with_disallow_null_authtok() {
        let mut fake = Fake::new(PAM_AUTH_ERR, PAM_SUCCESS);
        authenticate(&mut fake, Service::Rehearsal, "alice", secret());
        assert_eq!(fake.calls[0], "start celestina-lock-rehearsal alice");
        assert_eq!(
            fake.calls[1],
            format!("authenticate {PAM_DISALLOW_NULL_AUTHTOK}")
        );
        assert_eq!(PAM_DISALLOW_NULL_AUTHTOK, 1);
    }

    #[test]
    fn a_refused_authentication_never_reaches_account_management() {
        let mut fake = Fake::new(PAM_AUTH_ERR, PAM_SUCCESS);
        let v = authenticate(&mut fake, Service::Production, "alice", secret());
        assert_eq!(v, Verdict::Refused);
        assert_eq!(
            fake.calls,
            ["start celestina-lock alice", "authenticate 1", "end 7"]
        );
    }

    #[test]
    fn t_v1_every_row_of_the_exit_table_through_the_session() {
        // (auth, acct, unsupported prompts?) -> verdict -> exit code.
        let rows: [(i32, i32, Verdict, i32); 8] = [
            (PAM_SUCCESS, PAM_SUCCESS, Verdict::Authenticated, 0),
            (PAM_AUTH_ERR, PAM_SUCCESS, Verdict::Refused, 1),
            (PAM_USER_UNKNOWN, PAM_SUCCESS, Verdict::Refused, 1),
            (PAM_SYSTEM_ERR, PAM_SUCCESS, Verdict::Unavailable, 2),
            (PAM_ABORT, PAM_SUCCESS, Verdict::Unavailable, 2),
            (PAM_SUCCESS, PAM_ACCT_EXPIRED, Verdict::AccountRefused, 3),
            (PAM_SUCCESS, PAM_SYSTEM_ERR, Verdict::Unavailable, 2),
            (PAM_CONV_ERR, PAM_SUCCESS, Verdict::Unavailable, 2),
        ];
        for (auth, acct, want, code) in rows {
            let mut fake = Fake::new(auth, acct);
            let got = authenticate(&mut fake, Service::Production, "alice", secret());
            assert_eq!(got, want, "{auth} {acct}");
            assert_eq!(got.exit_code(), code, "{auth} {acct}");
        }
        // Unsupported (4): the stack asks a second secret prompt.
        let mut fake = Fake::new(PAM_CONV_ERR, PAM_SUCCESS);
        fake.prompts = vec![vec![PAM_PROMPT_ECHO_OFF], vec![PAM_PROMPT_ECHO_OFF]];
        let got = authenticate(&mut fake, Service::Production, "alice", secret());
        assert_eq!(got, Verdict::Unsupported);
        assert_eq!(got.exit_code(), 4);
        // Unsupported (4): an echo-on prompt.
        let mut fake = Fake::new(PAM_CONV_ERR, PAM_SUCCESS);
        fake.prompts = vec![vec![PAM_PROMPT_ECHO_ON]];
        assert_eq!(
            authenticate(&mut fake, Service::Production, "alice", secret()),
            Verdict::Unsupported
        );
        // Messages are answered empty and never change the verdict.
        let mut fake = Fake::new(PAM_SUCCESS, PAM_SUCCESS);
        fake.prompts = vec![vec![PAM_TEXT_INFO, PAM_PROMPT_ECHO_OFF, PAM_ERROR_MSG]];
        assert_eq!(
            authenticate(&mut fake, Service::Production, "alice", secret()),
            Verdict::Authenticated
        );
    }

    #[test]
    fn pam_start_failure_is_unavailable_and_the_secret_is_gone() {
        let mut fake = Fake::new(PAM_SUCCESS, PAM_SUCCESS);
        fake.start = Err(PAM_SYSTEM_ERR);
        let conv_probe;
        let v = {
            let v = authenticate(&mut fake, Service::Production, "alice", secret());
            conv_probe = fake.conv.clone();
            v
        };
        assert_eq!(v, Verdict::Unavailable);
        assert_eq!(fake.calls, ["start celestina-lock alice"]);
        let conv = conv_probe.expect("start was called");
        assert!(!conv.lock().unwrap().holds_secret());
    }

    #[test]
    fn the_secret_is_zeroed_before_pam_end() {
        let mut fake = Fake::new(PAM_SUCCESS, PAM_SUCCESS);
        authenticate(&mut fake, Service::Production, "alice", secret());
        assert_eq!(fake.secret_at_end, Some(false));
        let mut fake = Fake::new(PAM_AUTH_ERR, PAM_SUCCESS);
        authenticate(&mut fake, Service::Production, "alice", secret());
        assert_eq!(fake.secret_at_end, Some(false));
    }

    #[test]
    fn a_pam_end_failure_after_a_good_check_is_not_an_unlock() {
        let mut fake = Fake::new(PAM_SUCCESS, PAM_SUCCESS);
        fake.end = PAM_SYSTEM_ERR;
        assert_eq!(
            authenticate(&mut fake, Service::Production, "alice", secret()),
            Verdict::Unavailable
        );
    }
}
