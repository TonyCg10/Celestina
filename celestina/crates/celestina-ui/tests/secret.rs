//! The secret buffer and its field through the public API: what a character
//! is refused for, what the answer line is made of, and the keys the field
//! takes and the ones it leaves alone.

use celestina_ui::{
    Secret, SecretField, SecretFieldState, SecretKey, SecretRefused, MAX_SECRET_BYTES,
};
use celestina_wayland::{InputEvent, Modifiers};

const XKB_KEY_BACKSPACE: u32 = 0xff08;
const XKB_KEY_TAB: u32 = 0xff09;
const XKB_KEY_RETURN: u32 = 0xff0d;
const XKB_KEY_ESCAPE: u32 = 0xff1b;
const XKB_KEY_INSERT: u32 = 0xff63;
const XKB_KEY_KP_ENTER: u32 = 0xff8d;
const XKB_KEY_DELETE: u32 = 0xffff;

fn press(keysym: u32, utf8: Option<&str>, modifiers: Modifiers) -> InputEvent {
    InputEvent::Key {
        keysym,
        utf8: utf8.map(str::to_owned),
        modifiers,
        pressed: true,
        serial: 7,
    }
}

fn plain() -> Modifiers {
    Modifiers::default()
}

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        ..Modifiers::default()
    }
}

fn typed_key(c: char) -> InputEvent {
    press(c as u32, Some(&c.to_string()), plain())
}

/// A writer that records every `write` call it receives.
#[derive(Default)]
struct Recorder {
    calls: Vec<Vec<u8>>,
}

impl std::io::Write for Recorder {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.calls.push(buf.to_vec());
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn line_of(secret: &Secret) -> Vec<u8> {
    let mut out = Vec::new();
    assert!(secret.write_line_to(&mut out).is_ok());
    out
}

#[test]
fn secret_refuses_newline_cr_and_nul() {
    let mut secret = Secret::new();
    assert_eq!(secret.push('a'), Ok(()));
    for c in ['\n', '\r', '\0'] {
        assert_eq!(secret.push(c), Err(SecretRefused::Forbidden), "{c:?}");
    }
    assert_eq!(line_of(&secret), b"a\n", "a refusal changes nothing");
    // The other control characters and non-ASCII text are the person's.
    for c in ['\t', ' ', '\u{e9}', '\u{1f600}'] {
        assert_eq!(secret.push(c), Ok(()), "{c:?}");
    }
}

#[test]
fn secret_refuses_the_511th_byte() {
    let mut secret = Secret::new();
    for _ in 0..MAX_SECRET_BYTES {
        assert_eq!(secret.push('x'), Ok(()));
    }
    assert_eq!(MAX_SECRET_BYTES, 510);
    assert_eq!(secret.push('x'), Err(SecretRefused::Full));
    assert_eq!(secret.push('\u{e9}'), Err(SecretRefused::Full));
    assert_eq!(line_of(&secret).len(), MAX_SECRET_BYTES + 1);

    // A multi-byte character that does not fit whole is refused whole, and a
    // smaller one still fits: nothing is ever cut.
    let mut secret = Secret::new();
    for _ in 0..MAX_SECRET_BYTES - 1 {
        assert_eq!(secret.push('x'), Ok(()));
    }
    assert_eq!(secret.push('\u{e9}'), Err(SecretRefused::Full));
    assert_eq!(secret.push('y'), Ok(()));
    let line = line_of(&secret);
    assert_eq!(line.len(), MAX_SECRET_BYTES + 1);
    assert!(std::str::from_utf8(&line).is_ok());
}

#[test]
fn secret_write_line_appends_one_newline_and_nothing_else() {
    let mut secret = Secret::new();
    for c in "pa55 w\u{f6}rd".chars() {
        assert_eq!(secret.push(c), Ok(()));
    }
    let mut recorder = Recorder::default();
    assert!(secret.write_line_to(&mut recorder).is_ok());
    assert_eq!(
        recorder.calls,
        vec!["pa55 w\u{f6}rd".as_bytes().to_vec(), b"\n".to_vec()],
        "the content, then the newline: two writes and no joined copy"
    );
    // The secret is not consumed.
    assert_eq!(line_of(&secret), "pa55 w\u{f6}rd\n".as_bytes());
}

#[test]
fn field_takes_characters_backspace_enter_and_escape_only() {
    let mut field = SecretField::new();
    let mut secret = Secret::new();
    assert_eq!(field.handle(&mut secret, typed_key('a')), SecretKey::Edited);
    assert_eq!(field.handle(&mut secret, typed_key('b')), SecretKey::Edited);
    assert_eq!(
        field.handle(
            &mut secret,
            press(XKB_KEY_BACKSPACE, Some("\u{8}"), plain())
        ),
        SecretKey::Edited
    );
    assert_eq!(line_of(&secret), b"a\n");
    // Space is a character; a key release is nothing.
    assert_eq!(
        field.handle(&mut secret, press(0x20, Some(" "), plain())),
        SecretKey::Edited
    );
    let release = InputEvent::Key {
        keysym: 'z' as u32,
        utf8: Some("z".to_owned()),
        modifiers: plain(),
        pressed: false,
        serial: 8,
    };
    assert_eq!(field.handle(&mut secret, release), SecretKey::Ignored);
    assert_eq!(line_of(&secret), b"a \n");

    assert_eq!(
        field.handle(&mut secret, press(XKB_KEY_RETURN, Some("\r"), plain())),
        SecretKey::Submit
    );
    assert_eq!(
        field.handle(&mut secret, press(XKB_KEY_KP_ENTER, Some("\r"), plain())),
        SecretKey::Submit
    );
    assert_eq!(
        field.handle(&mut secret, press(XKB_KEY_ESCAPE, Some("\u{1b}"), plain())),
        SecretKey::Cancel
    );
    // Everything else passes through to the card, the secret untouched.
    for ev in [
        press(XKB_KEY_TAB, Some("\t"), plain()),
        press(XKB_KEY_DELETE, Some("\u{7f}"), plain()),
        press(0xff51, None, plain()), // Left
        press(0xff50, None, plain()), // Home
    ] {
        assert_eq!(field.handle(&mut secret, ev), SecretKey::Ignored);
    }
    assert_eq!(line_of(&secret), b"a \n");
}

#[test]
fn field_ignores_enter_on_an_empty_secret() {
    let mut field = SecretField::new();
    let mut secret = Secret::new();
    assert_eq!(
        field.handle(&mut secret, press(XKB_KEY_RETURN, Some("\r"), plain())),
        SecretKey::Ignored
    );
    assert_eq!(field.state(), SecretFieldState::Empty);
}

#[test]
fn field_ignores_control_text_and_refuses_the_overflow_without_cutting() {
    let mut field = SecretField::new();
    let mut secret = Secret::new();
    assert_eq!(field.handle(&mut secret, typed_key('a')), SecretKey::Edited);
    // A key whose text is a control character is not typed text at all.
    assert_eq!(
        field.handle(&mut secret, press(0x6a, Some("\n"), plain())),
        SecretKey::Ignored
    );
    for _ in 1..MAX_SECRET_BYTES {
        assert_eq!(field.handle(&mut secret, typed_key('x')), SecretKey::Edited);
    }
    assert_eq!(
        field.handle(&mut secret, typed_key('x')),
        SecretKey::Refused
    );
    // A composed key of several characters is refused whole, not cut.
    assert_eq!(
        field.handle(&mut secret, press(0xfd01, Some("ab"), plain())),
        SecretKey::Refused
    );
    assert_eq!(line_of(&secret).len(), MAX_SECRET_BYTES + 1);
}

#[test]
fn field_ignores_paste_copy_cut_and_select_all() {
    let mut field = SecretField::new();
    let mut secret = Secret::new();
    assert_eq!(field.handle(&mut secret, typed_key('k')), SecretKey::Edited);
    let before = line_of(&secret);

    let ignored = [
        // The clipboard's text, as the runtime would deliver a paste.
        InputEvent::Paste("pasted".to_owned()),
        // Ctrl+V, Ctrl+C, Ctrl+X, Ctrl+A with the text xkb gives them and
        // with a compositor that gives the letter.
        press('v' as u32, Some("\u{16}"), ctrl()),
        press('c' as u32, Some("\u{3}"), ctrl()),
        press('x' as u32, Some("\u{18}"), ctrl()),
        press('a' as u32, Some("\u{1}"), ctrl()),
        press('v' as u32, Some("v"), ctrl()),
        press('a' as u32, Some("a"), ctrl()),
        // The legacy chords: Shift+Insert, Ctrl+Insert, Shift+Delete.
        press(
            XKB_KEY_INSERT,
            None,
            Modifiers {
                shift: true,
                ..plain()
            },
        ),
        press(XKB_KEY_INSERT, None, ctrl()),
        press(
            XKB_KEY_DELETE,
            None,
            Modifiers {
                shift: true,
                ..plain()
            },
        ),
        // Alt and Logo chords type nothing.
        press(
            'q' as u32,
            Some("q"),
            Modifiers {
                alt: true,
                ..plain()
            },
        ),
        press(
            'q' as u32,
            Some("q"),
            Modifiers {
                logo: true,
                ..plain()
            },
        ),
    ];
    for ev in ignored {
        assert_eq!(
            field.handle(&mut secret, ev.clone()),
            SecretKey::Ignored,
            "{ev:?}"
        );
        assert_eq!(line_of(&secret), before);
    }
}

#[test]
fn field_ignores_preedit_and_commit_text() {
    let mut field = SecretField::new();
    let mut secret = Secret::new();
    assert_eq!(field.handle(&mut secret, typed_key('k')), SecretKey::Edited);
    let before = line_of(&secret);
    let events = [
        InputEvent::Preedit {
            text: "ni".to_owned(),
            cursor: 2,
        },
        InputEvent::Preedit {
            text: String::new(),
            cursor: 0,
        },
        InputEvent::CommitText("\u{4f60}".to_owned()),
    ];
    for ev in events {
        assert_eq!(field.handle(&mut secret, ev), SecretKey::Ignored);
        assert_eq!(line_of(&secret), before);
    }
}

#[test]
fn field_shows_no_length_only_empty_or_content() {
    let mut field = SecretField::new();
    let mut secret = Secret::new();
    assert_eq!(field.state(), SecretFieldState::Empty);
    assert_eq!(field.handle(&mut secret, typed_key('a')), SecretKey::Edited);
    let one = field.state();
    for _ in 0..99 {
        assert_eq!(field.handle(&mut secret, typed_key('b')), SecretKey::Edited);
    }
    assert_eq!(one, SecretFieldState::HasContent);
    assert_eq!(field.state(), one, "100 characters look like 1");
    // The state has these two values and no third that could carry a count.
    match field.state() {
        SecretFieldState::Empty | SecretFieldState::HasContent => {}
    }
    // Backspacing to nothing is "empty" again.
    for _ in 0..100 {
        assert_eq!(
            field.handle(
                &mut secret,
                press(XKB_KEY_BACKSPACE, Some("\u{8}"), plain())
            ),
            SecretKey::Edited
        );
    }
    assert_eq!(field.state(), SecretFieldState::Empty);
}

#[test]
fn field_wipes_the_secret_when_the_keyboard_leaves() {
    let mut field = SecretField::new();
    let mut secret = Secret::new();
    assert!(!field.focused());
    let enter = InputEvent::FocusEnter {
        serial: 3,
        at: std::time::Instant::now(),
    };
    assert_eq!(field.handle(&mut secret, enter), SecretKey::Ignored);
    assert!(field.focused());
    assert_eq!(field.handle(&mut secret, typed_key('a')), SecretKey::Edited);
    assert_eq!(
        field.handle(&mut secret, InputEvent::FocusLost),
        SecretKey::Ignored
    );
    assert!(!field.focused());
    assert!(secret.is_empty());
    assert_eq!(field.state(), SecretFieldState::Empty);
}
