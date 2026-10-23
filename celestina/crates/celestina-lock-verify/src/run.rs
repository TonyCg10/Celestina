//! The verifier from argv to the verdict, with everything it touches behind a
//! seam so the exit table is tested without root, `/etc` or `libpam`
//! (design §4.3 (c), §4.11).
//!
//! The order is the cheap and certain checks first, so a malformed request
//! never reaches PAM: argv, the uid, the secret's bounds, the presence rule of
//! the service, and only then the user name and the PAM stack.
#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::io::Read;
use std::path::PathBuf;

use celestina_lock_proto::{service_check, Verdict};

use crate::args;
use crate::secret::read_secret;
use crate::session::{self, PamApi};

/// What the process knows about the machine it runs on.
pub trait Host {
    /// The real uid.
    fn uid(&self) -> u32;
    /// The login name of `uid`, if the user database has it.
    fn user_name(&self, uid: u32) -> Option<String>;
    /// Which of `candidates` exist (`stat`), in the order given.
    fn existing(&self, candidates: &[PathBuf]) -> Vec<PathBuf>;
}

/// Verify one request: `args` is argv without the program name, `stdin` holds
/// the secret.
pub fn run<H, P, R, I>(host: &H, args: I, stdin: R, api: &mut P) -> Verdict
where
    H: Host,
    P: PamApi,
    R: Read,
    I: IntoIterator<Item = OsString>,
{
    let service = match args::parse(args) {
        Ok(service) => service,
        Err(verdict) => return verdict,
    };
    let uid = host.uid();
    if uid == 0 {
        return Verdict::BadRequest;
    }
    let Ok(secret) = read_secret(stdin) else {
        return Verdict::BadRequest;
    };
    if let Err(verdict) = service_check(service, &host.existing(&service.paths())) {
        return verdict;
    }
    let Some(user) = host.user_name(uid) else {
        return Verdict::Unavailable;
    };
    session::authenticate(api, service, &user, secret)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::{
        PAM_ACCT_EXPIRED, PAM_AUTH_ERR, PAM_CONV_ERR, PAM_SUCCESS, PAM_SYSTEM_ERR,
    };
    use crate::session::SharedConv;
    use std::cell::RefCell;

    struct FakeHost {
        uid: u32,
        name: Option<&'static str>,
        present: Vec<&'static str>,
    }

    impl Host for FakeHost {
        fn uid(&self) -> u32 {
            self.uid
        }
        fn user_name(&self, uid: u32) -> Option<String> {
            assert_eq!(uid, self.uid);
            self.name.map(str::to_owned)
        }
        fn existing(&self, candidates: &[PathBuf]) -> Vec<PathBuf> {
            candidates
                .iter()
                .filter(|path| self.present.iter().any(|p| path.as_os_str() == *p))
                .cloned()
                .collect()
        }
    }

    fn host() -> FakeHost {
        FakeHost {
            uid: 1000,
            name: Some("alice"),
            present: vec!["/etc/pam.d/celestina-lock"],
        }
    }

    /// A stack that answers fixed codes and records that it was reached, and
    /// with which user and what it was handed on the conversation.
    struct Stack {
        auth: i32,
        acct: i32,
        trace: RefCell<Vec<String>>,
        prompts: Vec<i32>,
    }

    impl Stack {
        fn new(auth: i32, acct: i32) -> Self {
            Self {
                auth,
                acct,
                trace: RefCell::new(Vec::new()),
                prompts: vec![crate::consts::PAM_PROMPT_ECHO_OFF],
            }
        }
        fn reached(&self) -> bool {
            !self.trace.borrow().is_empty()
        }
    }

    impl PamApi for Stack {
        type Handle = SharedConv;
        fn start(
            &mut self,
            service: &str,
            user: &str,
            conv: SharedConv,
        ) -> Result<SharedConv, i32> {
            self.trace.borrow_mut().push(format!("{service}/{user}"));
            Ok(conv)
        }
        fn authenticate(&mut self, conv: &mut SharedConv, _: i32) -> i32 {
            let mut state = conv.lock().unwrap();
            let out = state.conversation.respond(&self.prompts);
            drop(state);
            if out.is_err() {
                return PAM_CONV_ERR;
            }
            self.auth
        }
        fn acct_mgmt(&mut self, _: &mut SharedConv, _: i32) -> i32 {
            self.acct
        }
        fn end(&mut self, _: SharedConv, _: i32) -> i32 {
            PAM_SUCCESS
        }
    }

    fn argv(service: &str) -> Vec<OsString> {
        ["--protocol", "1", "--service", service]
            .iter()
            .map(OsString::from)
            .collect()
    }

    #[test]
    fn t_v1_argv_stdin_exit_table() {
        let good = || argv("celestina-lock");
        // 0: authenticated.
        let mut stack = Stack::new(PAM_SUCCESS, PAM_SUCCESS);
        let v = run(&host(), good(), &b"secret"[..], &mut stack);
        assert_eq!(v.exit_code(), 0);
        assert_eq!(stack.trace.borrow().as_slice(), ["celestina-lock/alice"]);
        // 1: refused.
        let mut stack = Stack::new(PAM_AUTH_ERR, PAM_SUCCESS);
        assert_eq!(
            run(&host(), good(), &b"secret"[..], &mut stack).exit_code(),
            1
        );
        // 2: unavailable (system error), and a user the database lacks.
        let mut stack = Stack::new(PAM_SYSTEM_ERR, PAM_SUCCESS);
        assert_eq!(
            run(&host(), good(), &b"secret"[..], &mut stack).exit_code(),
            2
        );
        let mut nameless = host();
        nameless.name = None;
        let mut stack = Stack::new(PAM_SUCCESS, PAM_SUCCESS);
        assert_eq!(
            run(&nameless, good(), &b"secret"[..], &mut stack).exit_code(),
            2
        );
        assert!(!stack.reached());
        // 3: account refused.
        let mut stack = Stack::new(PAM_SUCCESS, PAM_ACCT_EXPIRED);
        assert_eq!(
            run(&host(), good(), &b"secret"[..], &mut stack).exit_code(),
            3
        );
        // 4: unsupported (a second secret prompt).
        let mut stack = Stack::new(PAM_SUCCESS, PAM_SUCCESS);
        stack.prompts = vec![
            crate::consts::PAM_PROMPT_ECHO_OFF,
            crate::consts::PAM_PROMPT_ECHO_OFF,
        ];
        assert_eq!(
            run(&host(), good(), &b"secret"[..], &mut stack).exit_code(),
            4
        );
        // 5: the production service has no file.
        let mut missing = host();
        missing.present.clear();
        let mut stack = Stack::new(PAM_SUCCESS, PAM_SUCCESS);
        assert_eq!(
            run(&missing, good(), &b"secret"[..], &mut stack).exit_code(),
            5
        );
        assert!(!stack.reached(), "PAM must not be called without the file");
        // 6: bad request, in each of its shapes, and PAM is never reached.
        let mut stack = Stack::new(PAM_SUCCESS, PAM_SUCCESS);
        let bad: Vec<(Vec<OsString>, &[u8])> = vec![
            (vec![], b"secret"),
            (good(), b""),
            (good(), b"a\0b"),
            (good(), &[b'x'; 513]),
            (argv("login"), b"secret"),
        ];
        for (args, input) in bad {
            assert_eq!(run(&host(), args, input, &mut stack).exit_code(), 6);
        }
        assert!(!stack.reached());
    }

    #[test]
    fn uid_zero_is_refused_before_pam() {
        let mut root = host();
        root.uid = 0;
        root.name = Some("root");
        let mut stack = Stack::new(PAM_SUCCESS, PAM_SUCCESS);
        let v = run(&root, argv("celestina-lock"), &b"secret"[..], &mut stack);
        assert_eq!(v, Verdict::BadRequest);
        assert!(!stack.reached());
    }

    #[test]
    fn unknown_protocol_and_unknown_service_are_bad_request() {
        let mut stack = Stack::new(PAM_SUCCESS, PAM_SUCCESS);
        let v2: Vec<OsString> = ["--protocol", "2", "--service", "celestina-lock"]
            .iter()
            .map(OsString::from)
            .collect();
        assert_eq!(
            run(&host(), v2, &b"secret"[..], &mut stack),
            Verdict::BadRequest
        );
        assert_eq!(
            run(&host(), argv("other"), &b"secret"[..], &mut stack),
            Verdict::BadRequest
        );
        assert!(!stack.reached());
    }

    #[test]
    fn t_v8_the_rehearsal_service_with_a_file_present_is_bad_request_without_etc() {
        let mut stack = Stack::new(PAM_AUTH_ERR, PAM_SUCCESS);
        // No such file: the stack is reached (and denies, via `other`).
        let v = run(
            &host(),
            argv("celestina-lock-rehearsal"),
            &b"secret"[..],
            &mut stack,
        );
        assert_eq!(v, Verdict::Refused);
        assert_eq!(
            stack.trace.borrow().as_slice(),
            ["celestina-lock-rehearsal/alice"]
        );
        // The file exists in either directory: 6, and PAM is not reached.
        for path in [
            "/etc/pam.d/celestina-lock-rehearsal",
            "/usr/lib/pam.d/celestina-lock-rehearsal",
        ] {
            let mut present = host();
            present.present = vec![path];
            let mut stack = Stack::new(PAM_AUTH_ERR, PAM_SUCCESS);
            let v = run(
                &present,
                argv("celestina-lock-rehearsal"),
                &b"secret"[..],
                &mut stack,
            );
            assert_eq!(v, Verdict::BadRequest, "{path}");
            assert!(!stack.reached());
        }
    }
}
