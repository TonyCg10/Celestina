//! Interaction: hit-test, hover and press with the recoil, click, scroll,
//! keyboard focus in Tab order, activation and Escape.
//!
//! `Interaction` keeps the hovered, pressed and focused nodes and mirrors
//! them into the tree (`Tree::set_highlight`) after every event, so the
//! paint walk needs no other input; the tree forgets ids it prunes, and
//! `Interaction` reads the mirror back before each event.
#![forbid(unsafe_code)]

use std::time::Instant;

use celestina_paint::Rect;
use celestina_wayland::{InputEvent, Modifiers};

use crate::{
    curve::{Motion, ONE_UI},
    field::{EditOp, FieldId},
    highlight::Highlight,
    key::Key,
    tree::{NodeId, Tree},
};

/// Linux `BTN_LEFT`; the only button that presses and clicks.
pub const BTN_LEFT: u32 = 0x110;

pub(crate) const XKB_KEY_SPACE: u32 = 0x20;
pub(crate) const XKB_KEY_TAB: u32 = 0xff09;
pub(crate) const XKB_KEY_RETURN: u32 = 0xff0d;
pub(crate) const XKB_KEY_KP_ENTER: u32 = 0xff8d;
pub(crate) const XKB_KEY_ESCAPE: u32 = 0xff1b;
pub(crate) const XKB_KEY_ISO_LEFT_TAB: u32 = 0xfe20;
const XKB_KEY_BACKSPACE: u32 = 0xff08;
const XKB_KEY_HOME: u32 = 0xff50;
const XKB_KEY_LEFT: u32 = 0xff51;
const XKB_KEY_RIGHT: u32 = 0xff53;
const XKB_KEY_END: u32 = 0xff57;
const XKB_KEY_DELETE: u32 = 0xffff;
const XKB_KEY_LOWER_A: u32 = 0x61;
const XKB_KEY_UPPER_A: u32 = 0x41;

/// What the user did, naming the node by its key.
#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent {
    /// A left press and release inside the same interactive node.
    Click(Key),
    HoverChanged(Key, bool),
    /// Enter or Space on the focused node.
    Activate(Key),
    /// The Escape key; the surface decides what it closes.
    Escape,
    /// A scroll over the topmost interactive node: horizontal, vertical,
    /// in logical px.
    Scroll(Key, f32, f32),
    /// An edit changed the text, the cursor, the selection or the
    /// composition of the field the product named so; the product reads the
    /// text through `Tree::field`.
    FieldChanged(FieldId),
}

/// The pointer and keyboard state of one surface.
#[derive(Debug, Default)]
pub struct Interaction {
    hovered: Option<NodeId>,
    pressed: Option<NodeId>,
    focused: Option<NodeId>,
}

impl Interaction {
    pub fn new() -> Interaction {
        Interaction::default()
    }

    /// Pointer events only (others are ignored): hit-tests the topmost
    /// interactive node, emits `HoverChanged` on enter and leave, starts
    /// the press recoil (scale → the theme's press scale, `Fast`) on a left
    /// press, restores it (→ 1, `Slow`) on release or leave, emits `Click`
    /// on a release inside the pressed node and `Scroll` for an axis event.
    /// A left press on a text field focuses it and places its cursor (no
    /// recoil), and motion while it is held selects (`FieldChanged`).
    pub fn pointer(&mut self, tree: &mut Tree, ev: &InputEvent, now: Instant) -> Vec<UiEvent> {
        self.sync(tree);
        let mut out = Vec::new();
        match *ev {
            InputEvent::PointerMotion { x, y } => {
                let hit = hit(tree, x, y, now);
                self.hover(tree, hit, &mut out);
                if let Some(id) = self.pressed.filter(|&p| is_field(tree, p)) {
                    self.edit_at(tree, id, x, EditOp::PointerDrag, &mut out);
                }
            }
            InputEvent::PointerPress { x, y, button, .. } => {
                let hit = hit(tree, x, y, now);
                self.hover(tree, hit, &mut out);
                if button == BTN_LEFT && self.pressed.is_none() {
                    if let Some(id) = hit {
                        self.pressed = Some(id);
                        if is_field(tree, id) {
                            self.focused = Some(id);
                            self.edit_at(tree, id, x, EditOp::PointerDown, &mut out);
                        } else {
                            recoil(tree, id, tree.press_scale, Motion::Fast, now);
                        }
                    }
                }
            }
            InputEvent::PointerRelease { x, y, button, .. } => {
                let hit = hit(tree, x, y, now);
                self.hover(tree, hit, &mut out);
                if button == BTN_LEFT {
                    if let Some(id) = self.pressed.take() {
                        recoil(tree, id, 1.0, Motion::Slow, now);
                        if hit == Some(id) {
                            out.push(UiEvent::Click(tree.key_of(id)));
                        }
                    }
                }
            }
            InputEvent::PointerLeave => {
                self.hover(tree, None, &mut out);
                if let Some(id) = self.pressed.take() {
                    recoil(tree, id, 1.0, Motion::Slow, now);
                }
            }
            InputEvent::Axis {
                x,
                y,
                horizontal,
                vertical,
            } => {
                let hit = hit(tree, x, y, now);
                self.hover(tree, hit, &mut out);
                if let Some(id) = hit {
                    out.push(UiEvent::Scroll(tree.key_of(id), horizontal, vertical));
                }
            }
            _ => {}
        }
        self.publish(tree);
        out
    }

    /// Key and focus events only: Tab and Shift+Tab (`ISO_Left_Tab`, or Tab
    /// with Shift held) cycle the focus through the focusable nodes in tree
    /// order; Enter, keypad Enter and Space emit `Activate` for the focused
    /// node; Escape emits `Escape`; `FocusLost` clears the focus.
    ///
    /// While a text field has the focus it consumes the keys it edits with
    /// (see [`Interaction::field_consumes`]) and they do nothing else:
    /// printable keys, Space included, insert; the left and right arrows
    /// (Ctrl by word, Shift to select), Home, End, Backspace and Delete move
    /// and delete; Ctrl+A selects all. Tab, Enter, Escape and the up and down
    /// arrows pass through to the rules above and to the surface. The input
    /// method's `Preedit` and `CommitText` and the clipboard's `Paste` edit
    /// the focused field too; without one they are dropped.
    pub fn key(&mut self, tree: &mut Tree, ev: &InputEvent) -> Vec<UiEvent> {
        self.sync(tree);
        let mut out = Vec::new();
        if let Some((id, op)) = field_edit(tree, self.focused, ev) {
            if let Some(field) = tree.slot_mut(id).and_then(|s| s.field.as_mut()) {
                if field.edit(op) {
                    out.extend(field_id(tree, id).map(UiEvent::FieldChanged));
                }
            }
            self.publish(tree);
            return out;
        }
        match ev {
            InputEvent::Key {
                keysym,
                modifiers,
                pressed: true,
                ..
            } => match *keysym {
                XKB_KEY_TAB if modifiers.shift => self.cycle_focus(tree, -1),
                XKB_KEY_TAB => self.cycle_focus(tree, 1),
                XKB_KEY_ISO_LEFT_TAB => self.cycle_focus(tree, -1),
                XKB_KEY_RETURN | XKB_KEY_KP_ENTER | XKB_KEY_SPACE => {
                    if let Some(id) = self.focused.filter(|&f| tree.is_shown(f)) {
                        out.push(UiEvent::Activate(tree.key_of(id)));
                    }
                }
                XKB_KEY_ESCAPE => out.push(UiEvent::Escape),
                _ => {}
            },
            InputEvent::FocusLost => self.focused = None,
            _ => {}
        }
        self.publish(tree);
        out
    }

    /// Whether the focused text field edits with the key event `ev`, so the
    /// surface leaves it alone instead of treating it as a shortcut.
    pub fn field_consumes(&self, tree: &Tree, ev: &InputEvent) -> bool {
        field_edit(tree, tree.highlight().focused, ev).is_some()
    }

    /// Edits field node `id` with `op` for a pointer at window x `x`.
    fn edit_at(
        &self,
        tree: &mut Tree,
        id: NodeId,
        x: f32,
        op: fn(f32) -> EditOp,
        out: &mut Vec<UiEvent>,
    ) {
        let Some(slot) = tree.slot_mut(id) else {
            return;
        };
        let Some(field) = slot.field.as_mut() else {
            return;
        };
        let local = x - slot.rect.x + field.scroll_x();
        if field.edit(op(local)) {
            out.extend(field_id(tree, id).map(UiEvent::FieldChanged));
        }
    }

    /// Takes back the state published last time, as the tree has kept it:
    /// reconciliation keeps the ids of surviving keys, drops the focus of
    /// leaving nodes and forgets pruned ids before any reuse.
    fn sync(&mut self, tree: &Tree) {
        let h = tree.highlight();
        self.hovered = h.hovered;
        self.pressed = h.pressed;
        self.focused = h.focused;
    }

    fn hover(&mut self, tree: &Tree, new: Option<NodeId>, out: &mut Vec<UiEvent>) {
        if new == self.hovered {
            return;
        }
        if let Some(old) = self.hovered {
            out.push(UiEvent::HoverChanged(tree.key_of(old), false));
        }
        if let Some(id) = new {
            out.push(UiEvent::HoverChanged(tree.key_of(id), true));
        }
        self.hovered = new;
    }

    /// Moves the focus `step` places through the focus order, wrapping;
    /// from nothing, Tab goes to the first and Shift+Tab to the last.
    fn cycle_focus(&mut self, tree: &Tree, step: isize) {
        let order = focus_order(tree);
        let count = order.len();
        if count == 0 {
            self.focused = None;
            return;
        }
        let current = self
            .focused
            .and_then(|f| order.iter().position(|&id| id == f));
        let next = match current {
            Some(i) => (i as isize + step).rem_euclid(count as isize) as usize,
            None if step > 0 => 0,
            None => count - 1,
        };
        self.focused = order.get(next).copied();
    }

    fn publish(&self, tree: &mut Tree) {
        tree.set_highlight(Highlight {
            hovered: self.hovered,
            pressed: self.pressed,
            focused: self.focused,
        });
    }
}

fn is_field(tree: &Tree, id: NodeId) -> bool {
    tree.slot(id).is_some_and(|s| s.field.is_some())
}

fn field_id(tree: &Tree, id: NodeId) -> Option<FieldId> {
    match tree.slot(id)?.kind {
        crate::view::ViewKind::Field { id, .. } => Some(id),
        _ => None,
    }
}

/// The edit the focused node, when it is a shown text field, makes with the
/// key event `ev`.
fn field_edit(tree: &Tree, focused: Option<NodeId>, ev: &InputEvent) -> Option<(NodeId, EditOp)> {
    let id = focused.filter(|&f| tree.is_shown(f) && is_field(tree, f))?;
    let op = match ev {
        InputEvent::Key {
            keysym,
            utf8,
            modifiers,
            pressed: true,
            ..
        } => key_to_edit(*keysym, utf8.as_deref(), *modifiers)?,
        InputEvent::Preedit { text, cursor } => EditOp::Preedit {
            text: text.clone(),
            cursor: *cursor,
        },
        InputEvent::CommitText(text) => EditOp::CommitPreedit(text.clone()),
        InputEvent::Paste(text) => EditOp::Paste(text.clone()),
        _ => return None,
    };
    Some((id, op))
}

/// What a field does with a key press, if it takes the key at all.
fn key_to_edit(keysym: u32, utf8: Option<&str>, m: Modifiers) -> Option<EditOp> {
    let (select, word) = (m.shift, m.ctrl);
    let plain = !m.ctrl && !m.alt && !m.logo;
    match keysym {
        XKB_KEY_LEFT if !m.alt && !m.logo => Some(EditOp::Left { select, word }),
        XKB_KEY_RIGHT if !m.alt && !m.logo => Some(EditOp::Right { select, word }),
        XKB_KEY_HOME if !m.alt && !m.logo => Some(EditOp::Home { select }),
        XKB_KEY_END if !m.alt && !m.logo => Some(EditOp::End { select }),
        XKB_KEY_BACKSPACE if plain => Some(EditOp::Backspace),
        XKB_KEY_DELETE if plain => Some(EditOp::Delete),
        XKB_KEY_LOWER_A | XKB_KEY_UPPER_A if m.ctrl && !m.alt && !m.logo => Some(EditOp::SelectAll),
        _ => utf8
            .filter(|s| plain && !s.is_empty() && !s.chars().any(char::is_control))
            .map(|s| EditOp::Insert(s.to_owned())),
    }
}

/// The focusable nodes in tree order, leaving out hidden and leaving
/// subtrees.
pub(crate) fn focus_order(tree: &Tree) -> Vec<NodeId> {
    let mut order = Vec::new();
    let mut stack: Vec<NodeId> = tree.roots.iter().rev().copied().collect();
    while let Some(id) = stack.pop() {
        let Some(slot) = tree.slot(id) else {
            continue;
        };
        if slot.hidden() || slot.leaving {
            continue;
        }
        if slot.focusable {
            order.push(id);
        }
        stack.extend(slot.children.iter().rev());
    }
    order
}

/// The topmost interactive node under (`x`, `y`): the last in paint order
/// whose laid-out rect holds the point, inside every ancestor's clip, drawn
/// with an effective opacity above 0 at `now`, and neither hidden nor in a
/// leaving subtree. Slides and the recoil scale are visual only.
pub(crate) fn hit(tree: &Tree, x: f32, y: f32, now: Instant) -> Option<NodeId> {
    let mut found = None;
    let mut stack: Vec<(NodeId, Option<Rect>, f32)> =
        tree.roots.iter().rev().map(|&r| (r, None, 1.0)).collect();
    while let Some((id, clip, parent_opacity)) = stack.pop() {
        let Some(slot) = tree.slot(id) else {
            continue;
        };
        if slot.hidden() || slot.leaving {
            continue;
        }
        let opacity = parent_opacity * slot.opacity.value(now);
        let inside_clip = clip.is_none_or(|c| c.contains(x, y));
        if slot.interactive && opacity > 0.0 && inside_clip && slot.rect.contains(x, y) {
            found = Some(id);
        }
        let child_clip = if slot.clip && matches!(slot.kind, crate::view::ViewKind::Box { .. }) {
            Some(clip.map_or(slot.rect, |c| intersect(c, slot.rect)))
        } else {
            clip
        };
        stack.extend(
            slot.children
                .iter()
                .rev()
                .map(|&c| (c, child_clip, opacity)),
        );
    }
    found
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    Rect {
        x,
        y,
        w: ((a.x + a.w).min(b.x + b.w) - x).max(0.0),
        h: ((a.y + a.h).min(b.y + b.h) - y).max(0.0),
    }
}

fn recoil(tree: &mut Tree, id: NodeId, to: f32, motion: Motion, now: Instant) {
    let reduced = tree.spatial();
    let duration = tree.duration(motion);
    if let Some(slot) = tree.slot_mut(id) {
        slot.scale.animate_to(to, duration, ONE_UI, now, reduced);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use celestina_wayland::Modifiers;

    use celestina_paint::Theme;

    use super::*;
    use crate::{
        curve::Motion,
        key::Key,
        testing::{card, card_with_rows, dur, fake_measure, id, row_key, row_view, AVAILABLE},
        tree::NodeId,
        view,
    };

    fn setup() -> (Tree, Interaction, [NodeId; 3]) {
        let (tree, _, rows) = card_with_rows();
        (tree, Interaction::new(), rows)
    }

    fn motion(x: f32, y: f32) -> InputEvent {
        InputEvent::PointerMotion { x, y }
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

    fn key_with(keysym: u32, shift: bool, pressed: bool) -> InputEvent {
        InputEvent::Key {
            keysym,
            utf8: None,
            modifiers: Modifiers {
                shift,
                ..Modifiers::default()
            },
            pressed,
            serial: 3,
        }
    }

    fn key(keysym: u32) -> InputEvent {
        key_with(keysym, false, true)
    }

    fn hover(i: u64, on: bool) -> UiEvent {
        UiEvent::HoverChanged(row_key(i), on)
    }

    // Rows are at y 20..60, 60..100, 100..140 and x 20..280.

    #[test]
    fn fully_transparent_nodes_and_subtrees_are_not_hit() {
        let t0 = Instant::now();
        let mut tree = Tree::new(false, &Theme::dark());
        let mut ia = Interaction::new();
        // Own opacity 0: the row is skipped.
        tree.apply(
            card((0..3).map(|i| {
                if i == 0 {
                    row_view(i).opacity(0.0)
                } else {
                    row_view(i)
                }
            })),
            t0,
        );
        tree.layout_measured(AVAILABLE, fake_measure);
        assert!(ia.pointer(&mut tree, &motion(100.0, 30.0), t0).is_empty());
        assert!(ia.pointer(&mut tree, &press(100.0, 30.0), t0).is_empty());
        assert_eq!(tree.highlight().pressed, None);
        ia.pointer(&mut tree, &release(100.0, 30.0), t0);
        // An ancestor at opacity 0 hides every row below it.
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(card((0..3).map(row_view)).opacity(0.0), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        assert!(ia.pointer(&mut tree, &motion(100.0, 70.0), t0).is_empty());
        // Partially visible is still hit.
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(card((0..3).map(row_view)).opacity(0.3), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        assert_eq!(
            ia.pointer(&mut tree, &motion(100.0, 70.0), t0),
            vec![hover(1, true)]
        );
    }

    #[test]
    fn hover_enters_and_leaves_the_topmost_interactive_node() {
        let (mut tree, mut ia, rows) = setup();
        let t0 = Instant::now();
        assert_eq!(
            ia.pointer(&mut tree, &motion(100.0, 30.0), t0),
            vec![hover(0, true)]
        );
        assert_eq!(tree.highlight().hovered, Some(rows[0]));
        assert!(ia.pointer(&mut tree, &motion(120.0, 35.0), t0).is_empty());
        assert_eq!(
            ia.pointer(&mut tree, &motion(100.0, 70.0), t0),
            vec![hover(0, false), hover(1, true)]
        );
        // The card itself is not interactive: its padding hovers nothing.
        assert_eq!(
            ia.pointer(&mut tree, &motion(5.0, 5.0), t0),
            vec![hover(1, false)]
        );
        assert_eq!(tree.highlight().hovered, None);
        assert!(ia
            .pointer(&mut tree, &InputEvent::PointerLeave, t0)
            .is_empty());
    }

    #[test]
    fn the_last_child_wins_the_hit_test_and_non_interactive_is_skipped() {
        let t0 = Instant::now();
        let fill = |name: &str| {
            let mut v = view::col(Key::named(name));
            v.style.position = taffy::Position::Absolute;
            v.style.inset = taffy::Rect::length(0.0);
            v
        };
        let rows = (0..3).map(|i| {
            if i == 2 {
                let mut r = row_view(i)
                    .child(fill("overlay").interactive())
                    .child(fill("top"));
                r.style.position = taffy::Position::Relative;
                r
            } else {
                row_view(i)
            }
        });
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(card(rows), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        let mut ia = Interaction::new();
        assert_eq!(
            tree.rect(id(&tree, Key::named("top"))),
            tree.rect(id(&tree, row_key(2)))
        );
        assert_eq!(
            ia.pointer(&mut tree, &motion(100.0, 120.0), t0),
            vec![UiEvent::HoverChanged(Key::named("overlay"), true)]
        );
    }

    #[test]
    fn press_recoils_and_release_inside_clicks() {
        let (mut tree, mut ia, rows) = setup();
        let t0 = Instant::now();
        ia.pointer(&mut tree, &motion(100.0, 30.0), t0);
        assert!(ia.pointer(&mut tree, &press(100.0, 30.0), t0).is_empty());
        assert_eq!(tree.highlight().pressed, Some(rows[0]));
        assert!(tree.is_animating(t0));
        assert!((tree.scale(rows[0], t0 + dur(Motion::Fast)) - 0.96).abs() < 1e-6);
        assert!(!tree.is_animating(t0 + dur(Motion::Fast)));

        let t1 = t0 + Duration::from_millis(300);
        assert_eq!(
            ia.pointer(&mut tree, &release(110.0, 40.0), t1),
            vec![UiEvent::Click(row_key(0))]
        );
        assert_eq!(tree.highlight().pressed, None);
        assert!(tree.is_animating(t1 + Duration::from_millis(300)));
        assert!(!tree.is_animating(t1 + dur(Motion::Slow)));
        assert_eq!(tree.scale(rows[0], t1 + dur(Motion::Slow)), 1.0);
    }

    #[test]
    fn reduced_motion_makes_the_recoil_a_step() {
        let (mut tree, mut ia, rows) = setup();
        tree.set_reduced_motion(true);
        let t0 = Instant::now();
        ia.pointer(&mut tree, &press(100.0, 30.0), t0);
        assert!((tree.scale(rows[0], t0) - 0.96).abs() < 1e-6);
        assert!(!tree.is_animating(t0));
        ia.pointer(&mut tree, &release(100.0, 30.0), t0);
        assert_eq!(tree.scale(rows[0], t0), 1.0);
    }

    #[test]
    fn release_outside_does_not_click_but_restores_the_scale() {
        let (mut tree, mut ia, rows) = setup();
        let t0 = Instant::now();
        ia.pointer(&mut tree, &press(100.0, 30.0), t0);
        let events = ia.pointer(&mut tree, &release(100.0, 70.0), t0);
        assert_eq!(events, vec![hover(0, false), hover(1, true)]);
        assert_eq!(tree.highlight().pressed, None);
        let later = t0 + dur(Motion::Slow);
        assert_eq!(tree.scale(rows[0], later), 1.0);
        assert_eq!(tree.scale(rows[1], later), 1.0);
    }

    #[test]
    fn pointer_leave_cancels_the_press() {
        let (mut tree, mut ia, rows) = setup();
        let t0 = Instant::now();
        ia.pointer(&mut tree, &press(100.0, 30.0), t0);
        assert_eq!(
            ia.pointer(&mut tree, &InputEvent::PointerLeave, t0),
            vec![hover(0, false)]
        );
        assert_eq!(tree.highlight().pressed, None);
        assert_eq!(tree.scale(rows[0], t0 + dur(Motion::Slow)), 1.0);
        // A release that arrives later re-hovers but never clicks.
        let events = ia.pointer(&mut tree, &release(100.0, 30.0), t0);
        assert_eq!(events, vec![hover(0, true)]);
    }

    #[test]
    fn only_the_left_button_presses() {
        let (mut tree, mut ia, rows) = setup();
        let t0 = Instant::now();
        let right = InputEvent::PointerPress {
            x: 100.0,
            y: 30.0,
            button: 0x111,
            serial: 1,
        };
        ia.pointer(&mut tree, &right, t0);
        assert_eq!(tree.highlight().pressed, None);
        assert_eq!(tree.scale(rows[0], t0), 1.0);
        assert!(!tree.is_animating(t0));
        ia.pointer(&mut tree, &press(5.0, 5.0), t0);
        assert_eq!(
            tree.highlight().pressed,
            None,
            "pressing a non-interactive area"
        );
    }

    #[test]
    fn a_removed_key_drops_its_highlight_once_pruned() {
        let (mut tree, mut ia, _) = setup();
        let t0 = Instant::now();
        ia.pointer(&mut tree, &press(100.0, 30.0), t0);
        ia.key(&mut tree, &key(XKB_KEY_TAB));
        assert_eq!(tree.highlight().focused, tree.node_by_key(row_key(0)));
        tree.apply(card([]), t0);
        // Leaving nodes lose focus at once; hover and press go when the
        // pointer moves or the node is pruned.
        assert_eq!(tree.highlight().focused, None);
        let settled = t0 + dur(Motion::Exit);
        tree.apply(card([]), settled);
        tree.layout_measured(AVAILABLE, fake_measure);
        assert_eq!(tree.node_by_key(row_key(0)), None);
        assert!(ia
            .pointer(&mut tree, &release(100.0, 30.0), settled)
            .is_empty());
        assert_eq!(tree.highlight(), Highlight::default());
    }

    #[test]
    fn a_press_on_a_leaving_node_never_clicks() {
        let (mut tree, mut ia, _) = setup();
        let t0 = Instant::now();
        ia.pointer(&mut tree, &press(100.0, 30.0), t0);
        tree.apply(card([row_view(1), row_view(2)]), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        // Row 1 slid under the pointer; the leaving row 0 is not hit.
        let events = ia.pointer(&mut tree, &release(100.0, 30.0), t0);
        assert_eq!(events, vec![hover(0, false), hover(1, true)]);
    }

    #[test]
    fn an_unrelated_change_keeps_press_hover_and_focus() {
        let (mut tree, mut ia, rows) = setup();
        let t0 = Instant::now();
        ia.pointer(&mut tree, &press(100.0, 30.0), t0);
        ia.key(&mut tree, &key(XKB_KEY_TAB));
        assert_eq!(tree.highlight().focused, Some(rows[0]));
        // Something else is rebuilt while the button is held.
        let with_child = |child: bool| {
            card((0..3).map(move |i| {
                if i == 2 && child {
                    row_view(i).child(view::col(Key::named("extra")))
                } else {
                    row_view(i)
                }
            }))
        };
        tree.apply(with_child(true), t0);
        tree.apply(with_child(false), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        assert_eq!(
            ia.pointer(&mut tree, &release(100.0, 30.0), t0),
            vec![UiEvent::Click(row_key(0))]
        );
        assert_eq!(tree.highlight().hovered, Some(rows[0]));
        assert_eq!(tree.highlight().focused, Some(rows[0]));
    }

    #[test]
    fn tab_skips_hidden_nodes() {
        let (mut tree, mut ia, rows) = setup();
        let t0 = Instant::now();
        tree.apply(card((0..3).map(|i| row_view(i).hidden(i == 1))), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        ia.key(&mut tree, &key(XKB_KEY_TAB));
        ia.key(&mut tree, &key(XKB_KEY_TAB));
        assert_eq!(tree.highlight().focused, Some(rows[2]));
    }

    #[test]
    fn tab_cycles_focusables_and_wraps() {
        let (mut tree, mut ia, rows) = setup();
        assert_eq!(tree.highlight().focused, None);
        for want in [rows[0], rows[1], rows[2], rows[0]] {
            assert!(ia.key(&mut tree, &key(XKB_KEY_TAB)).is_empty());
            assert_eq!(tree.highlight().focused, Some(want));
        }
        for want in [rows[2], rows[1], rows[0], rows[2]] {
            ia.key(&mut tree, &key(XKB_KEY_ISO_LEFT_TAB));
            assert_eq!(tree.highlight().focused, Some(want));
        }
        // Shift+Tab delivered as Tab with Shift held also goes back.
        ia.key(&mut tree, &key_with(XKB_KEY_TAB, true, true));
        assert_eq!(tree.highlight().focused, Some(rows[1]));
        // Key releases do nothing.
        ia.key(&mut tree, &key_with(XKB_KEY_TAB, false, false));
        assert_eq!(tree.highlight().focused, Some(rows[1]));
    }

    #[test]
    fn enter_and_space_activate_the_focused_node() {
        let (mut tree, mut ia, rows) = setup();
        assert!(ia.key(&mut tree, &key(XKB_KEY_RETURN)).is_empty());
        ia.key(&mut tree, &key(XKB_KEY_TAB));
        for sym in [XKB_KEY_RETURN, XKB_KEY_KP_ENTER, XKB_KEY_SPACE] {
            assert_eq!(
                ia.key(&mut tree, &key(sym)),
                vec![UiEvent::Activate(row_key(0))]
            );
        }
        assert_eq!(tree.highlight().focused, Some(rows[0]));
        assert!(ia.key(&mut tree, &InputEvent::FocusLost).is_empty());
        assert_eq!(tree.highlight().focused, None);
    }

    #[test]
    fn escape_is_an_event_with_or_without_focus() {
        let (mut tree, mut ia, _) = setup();
        assert_eq!(
            ia.key(&mut tree, &key(XKB_KEY_ESCAPE)),
            vec![UiEvent::Escape]
        );
        ia.key(&mut tree, &key(XKB_KEY_TAB));
        assert_eq!(
            ia.key(&mut tree, &key(XKB_KEY_ESCAPE)),
            vec![UiEvent::Escape]
        );
        assert!(ia
            .key(&mut tree, &key_with(XKB_KEY_ESCAPE, false, false))
            .is_empty());
    }

    #[test]
    fn tab_with_no_focusables_is_a_no_op() {
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(view::col(Key::named("root")), Instant::now());
        let mut ia = Interaction::new();
        assert!(ia.key(&mut tree, &key(XKB_KEY_TAB)).is_empty());
        assert_eq!(tree.highlight().focused, None);
    }

    #[test]
    fn axis_scrolls_the_topmost_interactive_node() {
        let (mut tree, mut ia, _) = setup();
        let t0 = Instant::now();
        let axis = |x: f32, y: f32| InputEvent::Axis {
            x,
            y,
            horizontal: 0.0,
            vertical: 12.0,
        };
        assert_eq!(
            ia.pointer(&mut tree, &axis(100.0, 70.0), t0),
            vec![hover(1, true), UiEvent::Scroll(row_key(1), 0.0, 12.0)]
        );
        assert_eq!(
            ia.pointer(&mut tree, &axis(5.0, 5.0), t0),
            vec![hover(1, false)]
        );
    }

    #[test]
    fn a_node_hidden_while_focused_loses_the_focus() {
        let (mut tree, mut ia, _) = setup();
        let t0 = Instant::now();
        ia.key(&mut tree, &key(XKB_KEY_TAB));
        assert_eq!(tree.highlight().focused, tree.node_by_key(row_key(0)));
        tree.apply(card((0..3).map(|i| row_view(i).hidden(i == 0))), t0);
        assert_eq!(tree.highlight().focused, None);
        assert!(ia.key(&mut tree, &key(XKB_KEY_RETURN)).is_empty());

        // An ancestor hidden takes the focus too.
        tree.apply(card((0..3).map(row_view)), t0);
        ia.key(&mut tree, &key(XKB_KEY_TAB));
        assert_eq!(tree.highlight().focused, tree.node_by_key(row_key(0)));
        tree.apply(card((0..3).map(row_view)).hidden(true), t0);
        assert_eq!(tree.highlight().focused, None);
        assert!(ia.key(&mut tree, &key(XKB_KEY_SPACE)).is_empty());
    }

    #[test]
    fn the_recoil_follows_the_tree_s_theme() {
        let mut theme = Theme::dark();
        theme.press_scale = 0.9;
        theme.motion.fast = 50;
        let t0 = Instant::now();
        let mut tree = Tree::new(false, &theme);
        tree.apply(card((0..3).map(row_view)), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        let mut ia = Interaction::new();
        ia.pointer(&mut tree, &press(100.0, 30.0), t0);
        let row = id(&tree, row_key(0));
        assert!(tree.is_animating(t0 + Duration::from_millis(49)));
        assert!(!tree.is_animating(t0 + Duration::from_millis(50)));
        assert!((tree.scale(row, t0 + Duration::from_millis(50)) - 0.9).abs() < 1e-6);
        // A new theme applies to the next recoil.
        theme.press_scale = 0.8;
        tree.set_theme(&theme);
        ia.pointer(&mut tree, &release(100.0, 30.0), t0);
        ia.pointer(
            &mut tree,
            &press(100.0, 30.0),
            t0 + Duration::from_millis(400),
        );
        assert!((tree.scale(row, t0 + Duration::from_millis(500)) - 0.8).abs() < 1e-6);
    }
}
