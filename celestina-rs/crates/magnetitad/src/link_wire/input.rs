//! The phone as trackpad and keyboard: the wire's motion, buttons, scroll,
//! keys and text become events of one virtual device the daemon owns
//! through `uinput`, created when the first input arrives and destroyed
//! with the daemon. Nothing the phone sends is a string a shell sees: text
//! is typed key by key from a fixed table, and everything else is a code.
//!
//! A sink trait keeps the loopback tests off the host's input: they record.

use std::sync::Mutex;
use std::time::Instant;

use magnetita_proto::control::input::{Button, Key, PointerButton, PointerMove, Scroll, Text};

use crate::lock::LockOk;

/// At most this many input events per second per session; beyond it the
/// rest of the second is dropped, since no hand produces more.
pub(crate) const MAX_EVENTS_PER_SECOND: u32 = 4000;

/// Where input events go.
pub(crate) trait InputSink: Send + Sync {
    fn pointer_move(&self, dx: i32, dy: i32);
    fn pointer_button(&self, button: Button, pressed: bool);
    fn scroll(&self, dx: i32, dy: i32);
    fn key(&self, code: u16, pressed: bool);
    /// A character with no key on the table is skipped.
    fn text(&self, text: &str);
}

/// Bounds the phone's input rate before it reaches the sink.
pub(crate) struct Governor {
    window: Mutex<(Instant, u32)>,
}

impl Default for Governor {
    fn default() -> Self {
        Self {
            window: Mutex::new((Instant::now(), 0)),
        }
    }
}

impl Governor {
    /// True when one more event fits in this second.
    pub(crate) fn admit(&self) -> bool {
        let mut w = self.window.lock_ok();
        let now = Instant::now();
        if now.duration_since(w.0).as_secs() >= 1 {
            *w = (now, 0);
        }
        if w.1 >= MAX_EVENTS_PER_SECOND {
            return false;
        }
        w.1 += 1;
        true
    }
}

/// Dispatches one decoded input envelope to the sink.
pub(crate) fn apply(sink: &dyn InputSink, kind: u16, body: &[u8]) -> Result<(), String> {
    match kind {
        PointerMove::KIND => {
            let m = PointerMove::decode(body).map_err(|e| e.to_string())?;
            sink.pointer_move(i32::from(m.dx), i32::from(m.dy));
        }
        PointerButton::KIND => {
            let b = PointerButton::decode(body).map_err(|e| e.to_string())?;
            sink.pointer_button(b.button, b.pressed);
        }
        Scroll::KIND => {
            let s = Scroll::decode(body).map_err(|e| e.to_string())?;
            sink.scroll(i32::from(s.dx), i32::from(s.dy));
        }
        Key::KIND => {
            let k = Key::decode(body).map_err(|e| e.to_string())?;
            if k.code == 0 || k.code > 248 {
                return Err("key code out of range".into());
            }
            sink.key(k.code, k.pressed);
        }
        Text::KIND => {
            let t = Text::decode(body).map_err(|e| e.to_string())?;
            sink.text(&t.text);
        }
        other => return Err(format!("unknown input kind {other}")),
    }
    Ok(())
}

// The one raw evdev code the table needs: left shift.
const KEY_LEFTSHIFT: u16 = 42;

/// The key behind an ASCII character on a US layout, and whether shift is
/// held for it. The phone types text; the desktop's own layout is not
/// consulted, which is the limit of typing by key code.
pub(crate) fn key_for_char(c: char) -> Option<(u16, bool)> {
    let lower = |code: u16| Some((code, false));
    let upper = |code: u16| Some((code, true));
    match c {
        'a'..='z' => lower(letter(c)),
        'A'..='Z' => upper(letter(c.to_ascii_lowercase())),
        '1' => lower(2),
        '2' => lower(3),
        '3' => lower(4),
        '4' => lower(5),
        '5' => lower(6),
        '6' => lower(7),
        '7' => lower(8),
        '8' => lower(9),
        '9' => lower(10),
        '0' => lower(11),
        '!' => upper(2),
        '@' => upper(3),
        '#' => upper(4),
        '$' => upper(5),
        '%' => upper(6),
        '^' => upper(7),
        '&' => upper(8),
        '*' => upper(9),
        '(' => upper(10),
        ')' => upper(11),
        '-' => lower(12),
        '_' => upper(12),
        '=' => lower(13),
        '+' => upper(13),
        '\u{8}' => lower(14),
        '\t' => lower(15),
        '\n' => lower(28),
        ' ' => lower(57),
        '[' => lower(26),
        '{' => upper(26),
        ']' => lower(27),
        '}' => upper(27),
        ';' => lower(39),
        ':' => upper(39),
        '\'' => lower(40),
        '"' => upper(40),
        '`' => lower(41),
        '~' => upper(41),
        '\\' => lower(43),
        '|' => upper(43),
        ',' => lower(51),
        '<' => upper(51),
        '.' => lower(52),
        '>' => upper(52),
        '/' => lower(53),
        '?' => upper(53),
        _ => None,
    }
}

fn letter(c: char) -> u16 {
    match c {
        'q' => 16,
        'w' => 17,
        'e' => 18,
        'r' => 19,
        't' => 20,
        'y' => 21,
        'u' => 22,
        'i' => 23,
        'o' => 24,
        'p' => 25,
        'a' => 30,
        's' => 31,
        'd' => 32,
        'f' => 33,
        'g' => 34,
        'h' => 35,
        'j' => 36,
        'k' => 37,
        'l' => 38,
        'z' => 44,
        'x' => 45,
        'c' => 46,
        'v' => 47,
        'b' => 48,
        'n' => 49,
        'm' => 50,
        _ => 0,
    }
}

/// The virtual device behind `/dev/uinput`, through the `evdev` crate's
/// safe builder: "Magnetita phone" with relative axes, three buttons and
/// the whole keyboard. Fails without `/dev/uinput` or the `uaccess` rule
/// `HOST-HYGIENE.md` records.
pub(crate) struct Uinput {
    device: Mutex<evdev::uinput::VirtualDevice>,
}

impl Uinput {
    pub(crate) fn open() -> std::io::Result<Self> {
        use evdev::{AttributeSet, KeyCode, RelativeAxisCode};
        let mut keys = AttributeSet::<KeyCode>::new();
        for code in 1u16..=248 {
            keys.insert(KeyCode::new(code));
        }
        keys.insert(KeyCode::BTN_LEFT);
        keys.insert(KeyCode::BTN_RIGHT);
        keys.insert(KeyCode::BTN_MIDDLE);
        let mut axes = AttributeSet::<RelativeAxisCode>::new();
        for axis in [
            RelativeAxisCode::REL_X,
            RelativeAxisCode::REL_Y,
            RelativeAxisCode::REL_WHEEL,
            RelativeAxisCode::REL_HWHEEL,
            RelativeAxisCode::REL_WHEEL_HI_RES,
            RelativeAxisCode::REL_HWHEEL_HI_RES,
        ] {
            axes.insert(axis);
        }
        let device = evdev::uinput::VirtualDevice::builder()?
            .name("Magnetita phone")
            .with_keys(&keys)?
            .with_relative_axes(&axes)?
            .build()?;
        Ok(Self {
            device: Mutex::new(device),
        })
    }

    fn emit(&self, events: &[evdev::InputEvent]) {
        let _ = self.device.lock_ok().emit(events);
    }
}

fn rel(axis: evdev::RelativeAxisCode, value: i32) -> evdev::InputEvent {
    evdev::InputEvent::new(evdev::EventType::RELATIVE.0, axis.0, value)
}

fn key_event(code: u16, pressed: bool) -> evdev::InputEvent {
    evdev::InputEvent::new(evdev::EventType::KEY.0, code, i32::from(pressed))
}

impl InputSink for Uinput {
    fn pointer_move(&self, dx: i32, dy: i32) {
        use evdev::RelativeAxisCode as A;
        self.emit(&[rel(A::REL_X, dx), rel(A::REL_Y, dy)]);
    }

    fn pointer_button(&self, button: Button, pressed: bool) {
        let code = match button {
            Button::Left => evdev::KeyCode::BTN_LEFT,
            Button::Right => evdev::KeyCode::BTN_RIGHT,
            Button::Middle => evdev::KeyCode::BTN_MIDDLE,
        };
        self.emit(&[key_event(code.0, pressed)]);
    }

    fn scroll(&self, dx: i32, dy: i32) {
        use evdev::RelativeAxisCode as A;
        // The wire's unit is 1/120 of a wheel step: hi-res as it is, the
        // coarse axes when a whole step accumulated.
        let mut events = vec![rel(A::REL_WHEEL_HI_RES, dy), rel(A::REL_HWHEEL_HI_RES, dx)];
        if dy / 120 != 0 {
            events.push(rel(A::REL_WHEEL, dy / 120));
        }
        if dx / 120 != 0 {
            events.push(rel(A::REL_HWHEEL, dx / 120));
        }
        self.emit(&events);
    }

    fn key(&self, code: u16, pressed: bool) {
        self.emit(&[key_event(code, pressed)]);
    }

    fn text(&self, text: &str) {
        for c in text.chars() {
            let Some((code, shift)) = key_for_char(c) else {
                continue;
            };
            if shift {
                self.emit(&[key_event(KEY_LEFTSHIFT, true)]);
            }
            self.emit(&[key_event(code, true)]);
            self.emit(&[key_event(code, false)]);
            if shift {
                self.emit(&[key_event(KEY_LEFTSHIFT, false)]);
            }
        }
    }
}

/// The production sink: the virtual device, opened on the first event and
/// kept; a failure to open is logged once and every event is dropped.
#[derive(Default)]
pub(crate) struct LazyUinput {
    device: Mutex<Option<Result<Uinput, ()>>>,
}

impl LazyUinput {
    fn with(&self, f: impl FnOnce(&Uinput)) {
        let mut slot = self.device.lock_ok();
        if slot.is_none() {
            *slot = Some(Uinput::open().map_err(|e| {
                crate::runtime::log("link", &format!("input: /dev/uinput unavailable: {e}"));
            }));
        }
        if let Some(Ok(device)) = slot.as_ref() {
            f(device);
        }
    }
}

impl InputSink for LazyUinput {
    fn pointer_move(&self, dx: i32, dy: i32) {
        self.with(|d| d.pointer_move(dx, dy));
    }
    fn pointer_button(&self, button: Button, pressed: bool) {
        self.with(|d| d.pointer_button(button, pressed));
    }
    fn scroll(&self, dx: i32, dy: i32) {
        self.with(|d| d.scroll(dx, dy));
    }
    fn key(&self, code: u16, pressed: bool) {
        self.with(|d| d.key(code, pressed));
    }
    fn text(&self, text: &str) {
        self.with(|d| d.text(text));
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    #[derive(Default)]
    pub(crate) struct Recorder(pub(crate) Mutex<Vec<String>>);

    impl InputSink for Recorder {
        fn pointer_move(&self, dx: i32, dy: i32) {
            self.0.lock_ok().push(format!("move {dx} {dy}"));
        }
        fn pointer_button(&self, button: Button, pressed: bool) {
            self.0
                .lock_ok()
                .push(format!("button {button:?} {pressed}"));
        }
        fn scroll(&self, dx: i32, dy: i32) {
            self.0.lock_ok().push(format!("scroll {dx} {dy}"));
        }
        fn key(&self, code: u16, pressed: bool) {
            self.0.lock_ok().push(format!("key {code} {pressed}"));
        }
        fn text(&self, text: &str) {
            self.0.lock_ok().push(format!("text {text}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_types_ascii_with_shift_where_needed_and_skips_the_rest() {
        assert_eq!(key_for_char('a'), Some((30, false)));
        assert_eq!(key_for_char('A'), Some((30, true)));
        assert_eq!(key_for_char('!'), Some((2, true)));
        assert_eq!(key_for_char('\n'), Some((28, false)));
        assert_eq!(key_for_char('\u{e9}'), None);
    }

    #[test]
    fn events_are_dispatched_by_kind_and_bad_codes_are_refused() {
        let sink = testing::Recorder::default();
        apply(
            &sink,
            PointerMove::KIND,
            &PointerMove { dx: 3, dy: -2 }.encode(),
        )
        .unwrap();
        apply(
            &sink,
            Key::KIND,
            &Key {
                code: 30,
                pressed: true,
            }
            .encode(),
        )
        .unwrap();
        assert!(apply(
            &sink,
            Key::KIND,
            &Key {
                code: 900,
                pressed: true
            }
            .encode()
        )
        .is_err());
        assert!(apply(&sink, 99, &[]).is_err());
        assert_eq!(sink.0.lock_ok().as_slice(), ["move 3 -2", "key 30 true"]);
    }

    #[test]
    fn the_governor_admits_a_second_of_events_and_no_more() {
        let g = Governor::default();
        for _ in 0..MAX_EVENTS_PER_SECOND {
            assert!(g.admit());
        }
        assert!(!g.admit());
    }

    /// Creates and destroys the real device without one event: run by hand
    /// where `/dev/uinput` is granted, never in the suite.
    #[test]
    #[ignore]
    fn the_virtual_device_opens_and_closes() {
        let device = Uinput::open().expect("/dev/uinput with the uaccess rule");
        drop(device);
    }
}
