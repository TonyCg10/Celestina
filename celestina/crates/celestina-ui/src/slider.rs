//! The slider: an integer level `0..=MAX` on a horizontal track, filled up
//! to the level, set by pressing or dragging anywhere on the control and by
//! the arrow, Page Up, Page Down, Home and End keys while it has the focus.
//!
//! The level belongs to the product. A view carries it in a [`Slider`]
//! (`view::slider`); an interaction that changes it emits
//! `UiEvent::SliderChanged` with the node's key and the new level, and the
//! tree shows the new level at once, so a drag follows the pointer before
//! the product answers. The next view the product applies sets the level
//! again, to whatever the product chose. The slider's role is its kind,
//! its state ([`Slider`]: level, step, muted, enabled) is read back through
//! `Tree::slider`, and its accessible name is the product's copy, as for
//! every other control.
//!
//! Geometry. The node is `handle + 2 × MARGIN` tall, where [`MARGIN`] is
//! the room the focus ring takes inside a node, so neither the handle nor
//! the track ever crosses the ring. The handle's centre travels from
//! `MARGIN + handle / 2` to `w − MARGIN − handle / 2`; a pointer at `x` names
//! the nearest level on that travel, saturating at both ends, so a press on
//! the track jumps there and a drag past either end pins 0 or `MAX`.
//!
//! Paint. The track in the control plate, lifting under the pointer and
//! sinking while pressed like every control; the fill in the accent up to
//! the level (in the secondary text role while muted, the level kept); the
//! handle in the primary text role, drawn at the press scale while pressed.
//! A disabled slider keeps its level, is drawn at the theme's disabled
//! opacity and takes no input. The slider has no motion of its own (the
//! level is drawn where it is), so reduced motion has nothing to shorten.
#![forbid(unsafe_code)]

use celestina_paint::{Painter, Rect, Role, Theme};
use celestina_wayland::Modifiers;

use crate::{
    highlight::{box_fill, FOCUS_INSET, FOCUS_WIDTH},
    input::{
        XKB_KEY_DOWN, XKB_KEY_END, XKB_KEY_HOME, XKB_KEY_LEFT, XKB_KEY_PAGE_DOWN, XKB_KEY_PAGE_UP,
        XKB_KEY_RIGHT, XKB_KEY_UP,
    },
};

/// The highest level; the lowest is 0.
pub const MAX: u8 = 100;
/// How far Page Up and Page Down move the level.
pub const PAGE_STEP: u8 = 10;
/// The room the focus ring takes inside the node on every side: the
/// handle and the track stay inside it.
pub const MARGIN: f32 = FOCUS_INSET + FOCUS_WIDTH;

/// What a slider shows and how it steps: the product's state, carried by
/// the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slider {
    value: u8,
    step: u8,
    muted: bool,
    enabled: bool,
}

impl Slider {
    /// A slider at `value` (saturated to [`MAX`]), stepping by 1, neither
    /// muted nor disabled.
    pub fn new(value: u8) -> Slider {
        Slider {
            value: value.min(MAX),
            step: 1,
            muted: false,
            enabled: true,
        }
    }

    /// How far the arrow keys move the level, `1..=MAX` (0 counts as 1).
    pub fn with_step(self, step: u8) -> Slider {
        Slider {
            step: step.clamp(1, MAX),
            ..self
        }
    }

    /// A muted slider draws its fill dimmed and keeps its level; it still
    /// takes input (the product decides what a change while muted means).
    pub fn with_muted(self, muted: bool) -> Slider {
        Slider { muted, ..self }
    }

    /// A disabled slider is drawn at the disabled opacity with its level,
    /// and is neither hit nor focused.
    pub fn with_enabled(self, enabled: bool) -> Slider {
        Slider { enabled, ..self }
    }

    pub fn value(self) -> u8 {
        self.value
    }

    pub fn step(self) -> u8 {
        self.step
    }

    pub fn muted(self) -> bool {
        self.muted
    }

    pub fn enabled(self) -> bool {
        self.enabled
    }

    /// The same slider at `value`, saturated to [`MAX`].
    pub(crate) fn with_value(self, value: u8) -> Slider {
        Slider {
            value: value.min(MAX),
            ..self
        }
    }

    /// The level a key press of `keysym` with `m` held sets, or `None` when
    /// the slider does not take that key: the left and down arrows step
    /// down, the right and up arrows step up, Page Down and Page Up move by
    /// [`PAGE_STEP`], Home and End go to 0 and [`MAX`]; every move
    /// saturates. Keys with Ctrl, Alt or Super held are left to the surface.
    pub(crate) fn key_value(self, keysym: u32, m: Modifiers) -> Option<u8> {
        if m.ctrl || m.alt || m.logo {
            return None;
        }
        let v = self.value;
        let value = match keysym {
            XKB_KEY_LEFT | XKB_KEY_DOWN => v.saturating_sub(self.step),
            XKB_KEY_RIGHT | XKB_KEY_UP => v.saturating_add(self.step),
            XKB_KEY_PAGE_DOWN => v.saturating_sub(PAGE_STEP),
            XKB_KEY_PAGE_UP => v.saturating_add(PAGE_STEP),
            XKB_KEY_HOME => 0,
            XKB_KEY_END => MAX,
            _ => return None,
        };
        Some(value.min(MAX))
    }
}

/// The theme's slider anatomy, which the tree keeps so hit-test, layout and
/// paint share one geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SliderMetrics {
    /// The handle's diameter.
    pub handle: f32,
    /// The track's thickness.
    pub track: f32,
    /// The alpha of a disabled slider.
    pub disabled_opacity: f32,
}

impl SliderMetrics {
    pub(crate) fn of(theme: &Theme) -> SliderMetrics {
        SliderMetrics {
            handle: theme.slider_handle_size,
            track: theme.linear_track_height,
            disabled_opacity: theme.disabled_opacity,
        }
    }

    /// The node's height unless the view sets one.
    pub(crate) fn height(&self) -> f32 {
        self.handle + 2.0 * MARGIN
    }

    /// Where the handle's centre is at 0 and at [`MAX`] in `rect`; the two
    /// meet in a rect too narrow for any travel.
    fn travel(&self, rect: Rect) -> (f32, f32) {
        let x0 = rect.x + MARGIN + self.handle / 2.0;
        let x1 = rect.x + rect.w - MARGIN - self.handle / 2.0;
        (x0, x1.max(x0))
    }

    /// The level a pointer at window x `x` names in `rect`: the nearest one
    /// on the travel, saturating at both ends.
    pub(crate) fn value_at(&self, rect: Rect, x: f32) -> u8 {
        let (x0, x1) = self.travel(rect);
        let span = x1 - x0;
        let t = if span > 0.0 {
            (x - x0) / span
        } else if x >= x0 {
            1.0
        } else {
            0.0
        };
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        // `t` is in 0..=1, so the rounded product is in 0..=MAX and the
        // cast is exact.
        (t * f32::from(MAX)).round() as u8
    }

    /// The window x of the handle's centre at `value` in `rect`.
    pub(crate) fn x_of(&self, rect: Rect, value: u8) -> f32 {
        let (x0, x1) = self.travel(rect);
        x0 + (x1 - x0) * f32::from(value.min(MAX)) / f32::from(MAX)
    }
}

/// How a slider is drawn this frame.
pub(crate) struct SliderPaint {
    pub slider: Slider,
    pub rect: Rect,
    pub hovered: bool,
    pub pressed: bool,
}

/// Paints `p` with `m`'s geometry and `theme`'s colours and press scale.
pub(crate) fn paint(painter: &mut Painter, theme: &Theme, m: &SliderMetrics, p: &SliderPaint) {
    let SliderPaint {
        slider,
        rect,
        hovered,
        pressed,
    } = *p;
    painter.push_state();
    if !slider.enabled {
        painter.opacity(m.disabled_opacity);
    }
    let (x0, x1) = m.travel(rect);
    let cy = rect.y + rect.h / 2.0;
    let half = m.track / 2.0;
    let track_role = box_fill(Some(Role::ControlFill), slider.enabled, hovered, pressed)
        .unwrap_or(Role::ControlFill);
    let track = Rect {
        x: x0 - half,
        y: cy - half,
        w: x1 - x0 + m.track,
        h: m.track,
    };
    painter.fill_rounded(track, half, theme.color(track_role));
    let hx = m.x_of(rect, slider.value);
    let fill_role = if slider.muted {
        Role::TextLo
    } else {
        Role::Accent
    };
    let fill = Rect {
        w: hx - x0 + half,
        ..track
    };
    painter.fill_rounded(fill, half, theme.color(fill_role));
    let d = if pressed {
        m.handle * theme.press_scale
    } else {
        m.handle
    };
    let handle = Rect {
        x: hx - d / 2.0,
        y: cy - d / 2.0,
        w: d,
        h: d,
    };
    painter.fill_rounded(handle, d / 2.0, theme.color(Role::TextHi));
    painter.pop_state();
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use celestina_wayland::InputEvent;

    use super::*;
    use crate::{
        input::{Interaction, UiEvent, BTN_LEFT, XKB_KEY_TAB},
        key::Key,
        testing::{fake_measure, id, AVAILABLE},
        tree::{NodeId, Tree},
        view,
    };

    fn metrics() -> SliderMetrics {
        SliderMetrics::of(&Theme::dark())
    }

    /// 200 wide at (20, 20): with the dark theme's 15 px handle the travel
    /// is x 31.5..=208.5, 1.77 px per level.
    const RECT: Rect = Rect {
        x: 20.0,
        y: 20.0,
        w: 200.0,
        h: 23.0,
    };

    fn key_of(keysym: u32) -> InputEvent {
        InputEvent::Key {
            keysym,
            utf8: None,
            modifiers: Modifiers::default(),
            pressed: true,
            serial: 3,
        }
    }

    fn press(x: f32, y: f32) -> InputEvent {
        InputEvent::PointerPress {
            x,
            y,
            button: BTN_LEFT,
            serial: 1,
        }
    }

    fn release(x: f32, y: f32) -> InputEvent {
        InputEvent::PointerRelease {
            x,
            y,
            button: BTN_LEFT,
            serial: 2,
        }
    }

    fn motion(x: f32, y: f32) -> InputEvent {
        InputEvent::PointerMotion { x, y }
    }

    fn changed(v: u8) -> UiEvent {
        UiEvent::SliderChanged(Key::named("level"), v)
    }

    /// A 200-wide slider at (20, 20) in a padded column, with a focusable
    /// row after it, applied and laid out.
    fn setup(slider: Slider) -> (Tree, Interaction, NodeId) {
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(root(slider), Instant::now());
        tree.layout_measured(AVAILABLE, fake_measure);
        let node = id(&tree, Key::named("level"));
        (tree, Interaction::new(), node)
    }

    fn root(slider: Slider) -> view::View {
        view::col(Key::named("card"))
            .width(240.0)
            .padding(20.0)
            .child(view::slider(Key::named("level"), slider).width(200.0))
            .child(
                view::row(Key::named("after"))
                    .height(40.0)
                    .interactive()
                    .focusable(),
            )
    }

    #[test]
    fn a_slider_saturates_its_level_and_step() {
        let s = Slider::new(250);
        assert_eq!(
            (s.value(), s.step(), s.muted(), s.enabled()),
            (100, 1, false, true)
        );
        assert_eq!(Slider::new(7).with_step(0).step(), 1);
        assert_eq!(Slider::new(7).with_step(200).step(), MAX);
        assert_eq!(Slider::new(7).with_value(101).value(), MAX);
        let s = Slider::new(3)
            .with_muted(true)
            .with_enabled(false)
            .with_step(5);
        assert_eq!(
            (s.value(), s.step(), s.muted(), s.enabled()),
            (3, 5, true, false)
        );
    }

    #[test]
    fn levels_and_positions_map_both_ways() {
        let m = metrics();
        assert_eq!(m.height(), 15.0 + 2.0 * MARGIN);
        assert_eq!(m.x_of(RECT, 0), 31.5);
        assert_eq!(m.x_of(RECT, MAX), 208.5);
        assert_eq!(m.x_of(RECT, 50), 120.0);
        // Out of range saturates.
        assert_eq!(m.x_of(RECT, 200), 208.5);
        for v in 0..=MAX {
            assert_eq!(m.value_at(RECT, m.x_of(RECT, v)), v, "round trip of {v}");
        }
        // The nearest level wins: 1.77 px per level.
        assert_eq!(m.value_at(RECT, 31.5 + 0.8), 0);
        assert_eq!(m.value_at(RECT, 31.5 + 0.9), 1);
    }

    #[test]
    fn positions_past_the_travel_saturate() {
        let m = metrics();
        assert_eq!(m.value_at(RECT, -500.0), 0);
        assert_eq!(m.value_at(RECT, 25.0), 0, "inside the margin");
        assert_eq!(m.value_at(RECT, 215.0), MAX, "inside the margin");
        assert_eq!(m.value_at(RECT, 5000.0), MAX);
        assert_eq!(m.value_at(RECT, f32::NAN), 0);
        assert_eq!(m.value_at(RECT, f32::INFINITY), MAX);
        assert_eq!(m.value_at(RECT, f32::NEG_INFINITY), 0);
    }

    #[test]
    fn a_rect_too_narrow_to_travel_is_all_or_nothing() {
        let m = metrics();
        let narrow = Rect { w: 10.0, ..RECT };
        assert_eq!(m.x_of(narrow, 0), m.x_of(narrow, MAX));
        assert_eq!(m.value_at(narrow, 0.0), 0);
        assert_eq!(m.value_at(narrow, 100.0), MAX);
    }

    #[test]
    fn keys_step_page_and_jump_and_saturate() {
        let none = Modifiers::default();
        let s = Slider::new(50).with_step(5);
        assert_eq!(s.key_value(XKB_KEY_RIGHT, none), Some(55));
        assert_eq!(s.key_value(XKB_KEY_UP, none), Some(55));
        assert_eq!(s.key_value(XKB_KEY_LEFT, none), Some(45));
        assert_eq!(s.key_value(XKB_KEY_DOWN, none), Some(45));
        assert_eq!(s.key_value(XKB_KEY_PAGE_UP, none), Some(60));
        assert_eq!(s.key_value(XKB_KEY_PAGE_DOWN, none), Some(40));
        assert_eq!(s.key_value(XKB_KEY_HOME, none), Some(0));
        assert_eq!(s.key_value(XKB_KEY_END, none), Some(MAX));
        // Saturation at both ends, whatever the step.
        let top = Slider::new(98).with_step(5);
        assert_eq!(top.key_value(XKB_KEY_RIGHT, none), Some(MAX));
        assert_eq!(top.key_value(XKB_KEY_PAGE_UP, none), Some(MAX));
        let bottom = Slider::new(3).with_step(5);
        assert_eq!(bottom.key_value(XKB_KEY_LEFT, none), Some(0));
        assert_eq!(bottom.key_value(XKB_KEY_PAGE_DOWN, none), Some(0));
        let huge = Slider::new(MAX).with_step(MAX);
        assert_eq!(huge.key_value(XKB_KEY_UP, none), Some(MAX));
        // Other keys, and shortcuts, are not the slider's.
        assert_eq!(s.key_value(XKB_KEY_TAB, none), None);
        for m in [
            Modifiers { ctrl: true, ..none },
            Modifiers { alt: true, ..none },
            Modifiers { logo: true, ..none },
        ] {
            assert_eq!(s.key_value(XKB_KEY_RIGHT, m), None);
        }
        let shift = Modifiers {
            shift: true,
            ..none
        };
        assert_eq!(s.key_value(XKB_KEY_RIGHT, shift), Some(55));
    }

    #[test]
    fn the_node_is_one_handle_tall_plus_the_ring_s_room() {
        let (tree, _, node) = setup(Slider::new(0));
        let r = tree.rect(node);
        assert_eq!((r.x, r.y, r.w, r.h), (20.0, 20.0, 200.0, 23.0));
    }

    #[test]
    fn a_press_on_the_track_jumps_and_a_drag_follows_then_saturates() {
        let (mut tree, mut ia, node) = setup(Slider::new(10));
        let t0 = Instant::now();
        let m = metrics();
        let rect = tree.rect(node);
        let y = rect.y + rect.h / 2.0;
        let at = |v: u8| m.x_of(rect, v);
        let events = ia.pointer(&mut tree, &press(at(50), y), t0);
        assert_eq!(
            events,
            vec![
                UiEvent::HoverChanged(Key::named("level"), true),
                changed(50)
            ]
        );
        assert_eq!(
            tree.slider(Key::named("level")).map(Slider::value),
            Some(50)
        );
        assert_eq!(tree.highlight().pressed, Some(node));
        // A press neither recoils nor takes the focus: the ring shows only
        // for keyboard focus.
        assert!(!tree.is_animating(t0));
        assert_eq!(tree.highlight().focused, None);
        // Motion to the same level emits nothing.
        assert!(ia
            .pointer(&mut tree, &motion(at(50) + 0.3, y), t0)
            .is_empty());
        assert_eq!(
            ia.pointer(&mut tree, &motion(at(70), y), t0),
            vec![changed(70)]
        );
        // Past the end of the track, and off the node, the level pins.
        let off = ia.pointer(&mut tree, &motion(700.0, 300.0), t0);
        assert_eq!(
            off,
            vec![
                UiEvent::HoverChanged(Key::named("level"), false),
                changed(MAX)
            ]
        );
        assert!(ia.pointer(&mut tree, &motion(900.0, 300.0), t0).is_empty());
        assert_eq!(
            ia.pointer(&mut tree, &motion(-50.0, 0.0), t0),
            vec![changed(0)]
        );
        // The release sets the level where it lands and never clicks.
        let events = ia.pointer(&mut tree, &release(at(30), y), t0);
        assert_eq!(
            events,
            vec![
                UiEvent::HoverChanged(Key::named("level"), true),
                changed(30)
            ]
        );
        assert_eq!(tree.highlight().pressed, None);
        // Motion after the release does not drag.
        assert!(ia.pointer(&mut tree, &motion(at(90), y), t0).is_empty());
        assert_eq!(
            tree.slider(Key::named("level")).map(Slider::value),
            Some(30)
        );
    }

    #[test]
    fn the_next_view_sets_the_level_again() {
        let (mut tree, mut ia, node) = setup(Slider::new(10));
        let t0 = Instant::now();
        let rect = tree.rect(node);
        ia.pointer(
            &mut tree,
            &press(metrics().x_of(rect, 60), rect.y + 5.0),
            t0,
        );
        assert_eq!(
            tree.slider(Key::named("level")).map(Slider::value),
            Some(60)
        );
        tree.apply(root(Slider::new(42)), t0);
        assert_eq!(
            tree.slider(Key::named("level")).map(Slider::value),
            Some(42)
        );
        assert_eq!(tree.node_by_key(Key::named("level")), Some(node));
        assert_eq!(tree.slider(Key::named("card")), None);
    }

    #[test]
    fn a_pointer_leave_ends_the_drag_and_keeps_the_level() {
        let (mut tree, mut ia, node) = setup(Slider::new(10));
        let t0 = Instant::now();
        let rect = tree.rect(node);
        ia.pointer(
            &mut tree,
            &press(metrics().x_of(rect, 25), rect.y + 5.0),
            t0,
        );
        ia.pointer(&mut tree, &InputEvent::PointerLeave, t0);
        assert_eq!(tree.highlight().pressed, None);
        assert!(!tree.is_animating(t0));
        assert!(ia.pointer(&mut tree, &motion(900.0, 30.0), t0).is_empty());
        assert_eq!(
            tree.slider(Key::named("level")).map(Slider::value),
            Some(25)
        );
    }

    #[test]
    fn the_focused_slider_takes_its_keys() {
        let (mut tree, mut ia, node) = setup(Slider::new(50).with_step(5));
        // Unfocused, the keys are not the slider's.
        assert!(!ia.slider_consumes(&tree, &key_of(XKB_KEY_RIGHT)));
        assert!(ia.key(&mut tree, &key_of(XKB_KEY_RIGHT)).is_empty());
        ia.key(&mut tree, &key_of(XKB_KEY_TAB));
        assert_eq!(tree.highlight().focused, Some(node));
        assert!(ia.slider_consumes(&tree, &key_of(XKB_KEY_RIGHT)));
        assert!(!ia.slider_consumes(&tree, &key_of(XKB_KEY_TAB)));
        assert_eq!(ia.key(&mut tree, &key_of(XKB_KEY_RIGHT)), vec![changed(55)]);
        assert_eq!(ia.key(&mut tree, &key_of(XKB_KEY_DOWN)), vec![changed(50)]);
        assert_eq!(
            ia.key(&mut tree, &key_of(XKB_KEY_PAGE_UP)),
            vec![changed(60)]
        );
        assert_eq!(ia.key(&mut tree, &key_of(XKB_KEY_END)), vec![changed(MAX)]);
        // Saturated: the key is still taken, and nothing changes.
        assert!(ia.slider_consumes(&tree, &key_of(XKB_KEY_UP)));
        assert!(ia.key(&mut tree, &key_of(XKB_KEY_UP)).is_empty());
        assert_eq!(ia.key(&mut tree, &key_of(XKB_KEY_HOME)), vec![changed(0)]);
        assert!(ia.key(&mut tree, &key_of(XKB_KEY_PAGE_DOWN)).is_empty());
        assert_eq!(tree.slider(Key::named("level")).map(Slider::value), Some(0));
        // Tab still moves the focus on.
        ia.key(&mut tree, &key_of(XKB_KEY_TAB));
        assert_eq!(
            tree.highlight().focused,
            tree.node_by_key(Key::named("after"))
        );
    }

    #[test]
    fn a_disabled_slider_keeps_its_level_and_takes_no_input() {
        let (mut tree, mut ia, node) = setup(Slider::new(40).with_enabled(false));
        let t0 = Instant::now();
        let rect = tree.rect(node);
        assert!(ia
            .pointer(
                &mut tree,
                &press(metrics().x_of(rect, 90), rect.y + 5.0),
                t0
            )
            .is_empty());
        assert_eq!(tree.highlight().pressed, None);
        ia.pointer(&mut tree, &release(0.0, 0.0), t0);
        // Tab skips it.
        ia.key(&mut tree, &key_of(XKB_KEY_TAB));
        assert_eq!(
            tree.highlight().focused,
            tree.node_by_key(Key::named("after"))
        );
        assert!(ia.key(&mut tree, &key_of(XKB_KEY_END)).is_empty());
        let s = tree.slider(Key::named("level"));
        assert_eq!(s.map(|s| (s.value(), s.enabled())), Some((40, false)));
    }

    #[test]
    fn a_muted_slider_keeps_its_level_and_still_takes_input() {
        let (mut tree, mut ia, node) = setup(Slider::new(40).with_muted(true));
        ia.key(&mut tree, &key_of(XKB_KEY_TAB));
        assert_eq!(tree.highlight().focused, Some(node));
        assert_eq!(ia.key(&mut tree, &key_of(XKB_KEY_RIGHT)), vec![changed(41)]);
        let s = tree.slider(Key::named("level"));
        assert_eq!(s.map(|s| (s.value(), s.muted())), Some((41, true)));
    }

    #[test]
    fn a_slider_hidden_while_focused_gives_its_keys_back() {
        let (mut tree, mut ia, _) = setup(Slider::new(40));
        ia.key(&mut tree, &key_of(XKB_KEY_TAB));
        let hidden = view::col(Key::named("card"))
            .width(240.0)
            .padding(20.0)
            .child(
                view::slider(Key::named("level"), Slider::new(40))
                    .width(200.0)
                    .hidden(true),
            );
        tree.apply(hidden, Instant::now());
        assert_eq!(tree.highlight().focused, None);
        assert!(!ia.slider_consumes(&tree, &key_of(XKB_KEY_RIGHT)));
    }

    #[test]
    fn reduced_motion_changes_nothing_a_slider_draws() {
        for reduced in [false, true] {
            let (mut tree, mut ia, node) = setup(Slider::new(10));
            tree.set_reduced_motion(reduced);
            let t0 = Instant::now();
            let rect = tree.rect(node);
            ia.pointer(
                &mut tree,
                &press(metrics().x_of(rect, 80), rect.y + 5.0),
                t0,
            );
            // The level is where the pointer put it, with nothing in flight.
            assert!(!tree.is_animating(t0));
            assert_eq!(tree.scale(node, t0), 1.0);
            assert_eq!(
                tree.slider(Key::named("level")).map(Slider::value),
                Some(80)
            );
        }
    }

    #[test]
    fn a_new_theme_resizes_the_slider() {
        let (mut tree, _, node) = setup(Slider::new(10));
        let mut theme = Theme::dark();
        theme.slider_handle_size = 21.0;
        tree.set_theme(&theme);
        tree.apply(root(Slider::new(10)), Instant::now());
        tree.layout_measured(AVAILABLE, fake_measure);
        assert_eq!(tree.rect(node).h, 21.0 + 2.0 * MARGIN);
    }
}
