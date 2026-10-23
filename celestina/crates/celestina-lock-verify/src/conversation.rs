//! Which prompt of the PAM stack gets the secret (design §4.11, "Conversation").
//!
//! This is the whole policy and it is pure: it sees only the *styles* of the
//! messages PAM sends, never their text, and it answers with a [`Reply`] per
//! message. `pam.rs` turns the replies into `malloc`'d responses; the secret's
//! bytes are never copied into anything built here.
//!
//! - The first `PAM_PROMPT_ECHO_OFF` the stack ever sends gets the secret.
//! - A second echo-off prompt, an echo-on prompt or a style this code does not
//!   know is refused (`PAM_CONV_ERR`) and latches the verdict `Unsupported`:
//!   nothing further is ever answered, so the secret cannot reach a prompt it
//!   does not belong to (an OTP, a user name; TM-17).
//! - `PAM_ERROR_MSG` and `PAM_TEXT_INFO` are answered with an empty response
//!   and counted. Their text is never looked at.
//! - `num_msg` outside `1..=PAM_MAX_NUM_MSG` is refused.
//!
//! A call is judged as a whole before anything is answered: a batch that holds
//! a refused prompt gets no secret at all, whatever the prompt's position.
#![forbid(unsafe_code)]

use zeroize::Zeroizing;

use crate::consts::{
    PAM_ERROR_MSG, PAM_MAX_NUM_MSG, PAM_PROMPT_ECHO_OFF, PAM_PROMPT_ECHO_ON, PAM_TEXT_INFO,
};

/// What a PAM message asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// A prompt whose answer is not echoed: the password.
    EchoOff,
    /// A prompt whose answer is echoed: a user name, a code.
    EchoOn,
    /// `PAM_ERROR_MSG`.
    ErrorMsg,
    /// `PAM_TEXT_INFO`.
    TextInfo,
}

impl Style {
    /// The style a raw `msg_style` stands for, if this code knows it.
    #[must_use]
    pub const fn from_raw(raw: i32) -> Option<Self> {
        match raw {
            PAM_PROMPT_ECHO_OFF => Some(Self::EchoOff),
            PAM_PROMPT_ECHO_ON => Some(Self::EchoOn),
            PAM_ERROR_MSG => Some(Self::ErrorMsg),
            PAM_TEXT_INFO => Some(Self::TextInfo),
            _ => None,
        }
    }
}

/// The answer to one message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reply {
    /// The secret, as a NUL-terminated copy `pam.rs` makes.
    Secret,
    /// No answer (`resp` is left NULL): for messages that are not prompts.
    Empty,
}

/// Why a call was refused. Every variant is answered `PAM_CONV_ERR`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvError {
    /// `num_msg` outside `1..=32`, or a message pointer that was NULL.
    BadCount,
    /// The stack asked for something the lock cannot answer: a second secret
    /// prompt, an echo-on prompt, or an unknown style. Latches `Unsupported`.
    Unsupported,
}

/// The conversation's state over one authentication.
#[derive(Debug, Default)]
pub struct Conversation {
    secret_given: bool,
    unsupported: bool,
    /// Counted for the tests only: nothing in production reads a number, and
    /// the property that matters (the text is never copied) holds because the
    /// type never sees the text.
    #[cfg(test)]
    messages: u32,
}

impl Conversation {
    /// A conversation that has answered nothing yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            secret_given: false,
            unsupported: false,
            #[cfg(test)]
            messages: 0,
        }
    }

    /// Answer one call of the PAM conversation function. `styles` holds the
    /// `msg_style` of each message, raw, in order.
    ///
    /// # Errors
    ///
    /// [`ConvError::BadCount`] for a count outside `1..=32`;
    /// [`ConvError::Unsupported`] for a prompt this verifier must not answer
    /// (and for every call after one).
    pub fn respond(&mut self, styles: &[i32]) -> Result<Vec<Reply>, ConvError> {
        if styles.is_empty() || styles.len() > PAM_MAX_NUM_MSG {
            return Err(ConvError::BadCount);
        }
        if self.unsupported {
            return Err(ConvError::Unsupported);
        }
        // Judge the whole call before answering any part of it.
        let mut echo_off = 0_usize;
        for &raw in styles {
            match Style::from_raw(raw) {
                Some(Style::EchoOff) => echo_off += 1,
                Some(Style::ErrorMsg | Style::TextInfo) => {}
                Some(Style::EchoOn) | None => {
                    self.unsupported = true;
                    return Err(ConvError::Unsupported);
                }
            }
        }
        if echo_off > 1 || (echo_off == 1 && self.secret_given) {
            self.unsupported = true;
            return Err(ConvError::Unsupported);
        }
        let mut replies = Vec::with_capacity(styles.len());
        for &raw in styles {
            replies.push(match Style::from_raw(raw) {
                Some(Style::EchoOff) => {
                    self.secret_given = true;
                    Reply::Secret
                }
                _ => {
                    #[cfg(test)]
                    {
                        self.messages = self.messages.saturating_add(1);
                    }
                    Reply::Empty
                }
            });
        }
        Ok(replies)
    }

    /// Whether the stack asked for something this verifier refuses to answer.
    #[must_use]
    pub const fn unsupported(&self) -> bool {
        self.unsupported
    }

    /// How many `PAM_ERROR_MSG` and `PAM_TEXT_INFO` messages were seen. Their
    /// text is never read; the count is all that is kept (TM-16).
    #[cfg(test)]
    #[must_use]
    pub const fn messages_seen(&self) -> u32 {
        self.messages
    }
}

/// What the conversation callback shares with the session: the policy and the
/// secret it may hand out, behind one `Mutex` that `pam.rs` reaches through the
/// `appdata_ptr` PAM hands back.
#[derive(Debug)]
pub struct ConvState {
    pub conversation: Conversation,
    secret: Option<Zeroizing<Vec<u8>>>,
}

impl ConvState {
    /// A state that may hand out `secret`, once.
    #[must_use]
    pub const fn new(secret: Zeroizing<Vec<u8>>) -> Self {
        Self {
            conversation: Conversation::new(),
            secret: Some(secret),
        }
    }

    /// The secret's bytes for the one reply that carries it; `None` once the
    /// secret was dropped, in which case the callback answers `PAM_CONV_ERR`.
    #[must_use]
    pub fn secret_bytes(&self) -> Option<&[u8]> {
        self.secret.as_deref().map(Vec::as_slice)
    }

    /// Drop and zero the secret. After this no prompt can be answered with it.
    pub fn drop_secret(&mut self) {
        self.secret = None;
    }

    /// Whether the secret is still held.
    #[cfg(test)]
    #[must_use]
    pub const fn holds_secret(&self) -> bool {
        self.secret.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::{PAM_ERROR_MSG, PAM_PROMPT_ECHO_OFF, PAM_PROMPT_ECHO_ON, PAM_TEXT_INFO};

    const OFF: i32 = PAM_PROMPT_ECHO_OFF;
    const ON: i32 = PAM_PROMPT_ECHO_ON;
    const ERR: i32 = PAM_ERROR_MSG;
    const INFO: i32 = PAM_TEXT_INFO;

    #[test]
    fn t_v2_only_the_first_echo_off_prompt_gets_the_secret() {
        let mut c = Conversation::new();
        assert_eq!(c.respond(&[OFF]), Ok(vec![Reply::Secret]));
        // A second secret prompt, in a later call, is refused and latches.
        assert_eq!(c.respond(&[OFF]), Err(ConvError::Unsupported));
        assert!(c.unsupported());
        // Once refused, nothing further is answered, not even a message.
        assert_eq!(c.respond(&[INFO]), Err(ConvError::Unsupported));
        assert_eq!(c.respond(&[OFF]), Err(ConvError::Unsupported));
    }

    #[test]
    fn two_echo_off_prompts_in_one_call_get_no_secret_at_all() {
        let mut c = Conversation::new();
        assert_eq!(c.respond(&[OFF, OFF]), Err(ConvError::Unsupported));
        assert!(c.unsupported());
        assert_eq!(c.respond(&[OFF]), Err(ConvError::Unsupported));
    }

    #[test]
    fn the_secret_goes_to_the_echo_off_prompt_among_messages() {
        let mut c = Conversation::new();
        assert_eq!(
            c.respond(&[INFO, OFF, ERR]),
            Ok(vec![Reply::Empty, Reply::Secret, Reply::Empty])
        );
        assert!(!c.unsupported());
    }

    #[test]
    fn echo_on_prompt_is_refused_and_unsupported() {
        let mut c = Conversation::new();
        assert_eq!(c.respond(&[ON]), Err(ConvError::Unsupported));
        assert!(c.unsupported());
        // The secret prompt after it is not answered either.
        assert_eq!(c.respond(&[OFF]), Err(ConvError::Unsupported));
    }

    #[test]
    fn an_echo_on_prompt_next_to_the_secret_prompt_gets_no_secret() {
        for styles in [[OFF, ON], [ON, OFF]] {
            let mut c = Conversation::new();
            assert_eq!(
                c.respond(&styles),
                Err(ConvError::Unsupported),
                "{styles:?}"
            );
            assert!(c.unsupported());
        }
    }

    #[test]
    fn an_unknown_style_is_unsupported() {
        for style in [0, 5, 6, -1, i32::MAX, i32::MIN] {
            let mut c = Conversation::new();
            assert_eq!(c.respond(&[style]), Err(ConvError::Unsupported), "{style}");
            assert!(c.unsupported(), "{style}");
        }
    }

    #[test]
    fn t_v3_messages_are_counted_never_copied() {
        let mut c = Conversation::new();
        assert_eq!(
            c.respond(&[ERR, INFO]),
            Ok(vec![Reply::Empty, Reply::Empty])
        );
        assert_eq!(c.respond(&[INFO]), Ok(vec![Reply::Empty]));
        assert_eq!(c.messages_seen(), 3);
        assert!(!c.unsupported());
        // The type carries no text at all: only styles go in, only a count
        // stays. (`Reply` has no payload.)
        assert_eq!(std::mem::size_of::<Reply>(), 1);
    }

    #[test]
    fn a_message_only_stack_never_receives_the_secret() {
        let mut c = Conversation::new();
        for _ in 0..100 {
            let out = c.respond(&[INFO, ERR]);
            assert_eq!(out, Ok(vec![Reply::Empty, Reply::Empty]));
        }
        assert_eq!(c.messages_seen(), 200);
    }

    #[test]
    fn num_msg_zero_and_thirty_three_are_refused() {
        let mut c = Conversation::new();
        assert_eq!(c.respond(&[]), Err(ConvError::BadCount));
        assert_eq!(c.respond(&[INFO; 33]), Err(ConvError::BadCount));
        assert_eq!(c.respond(&[INFO; 1000]), Err(ConvError::BadCount));
        // A bad count is not an unsupported prompt: the verdict stays the
        // generic one (Unavailable), and the secret prompt is still free.
        assert!(!c.unsupported());
        assert_eq!(c.respond(&[OFF]), Ok(vec![Reply::Secret]));
        // The bounds themselves are allowed.
        let mut c = Conversation::new();
        assert_eq!(c.respond(&[INFO; 32]).map(|r| r.len()), Ok(32));
    }

    #[test]
    fn conv_state_hands_out_the_secret_until_it_is_dropped() {
        let mut state = ConvState::new(Zeroizing::new(b"hunter2".to_vec()));
        assert!(state.holds_secret());
        assert_eq!(state.secret_bytes(), Some(&b"hunter2"[..]));
        state.drop_secret();
        assert!(!state.holds_secret());
        assert_eq!(state.secret_bytes(), None);
    }
}
