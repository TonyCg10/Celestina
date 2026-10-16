//! The text field through the public API: its state across keyed updates,
//! the keys it consumes and the ones that pass through, and the pointer.

use std::{
    sync::{Mutex, MutexGuard, OnceLock},
    time::Instant,
};

use celestina_paint::{default_font_dirs, Role, TextStyle, TextSystem, Theme, Weight};
use celestina_ui::{view, FieldId, Interaction, Key, Tree, UiEvent, View, BTN_LEFT};
use celestina_wayland::{InputEvent, Modifiers};

const AVAILABLE: (f32, f32) = (760.0, 560.0);
const SEARCH: FieldId = FieldId(1);
const OTHER: FieldId = FieldId(2);

const XKB_KEY_TAB: u32 = 0xff09;
const XKB_KEY_RETURN: u32 = 0xff0d;
const XKB_KEY_ESCAPE: u32 = 0xff1b;
const XKB_KEY_SPACE: u32 = 0x20;
const XKB_KEY_BACKSPACE: u32 = 0xff08;
const XKB_KEY_HOME: u32 = 0xff50;
const XKB_KEY_LEFT: u32 = 0xff51;
const XKB_KEY_UP: u32 = 0xff52;
const XKB_KEY_RIGHT: u32 = 0xff53;
const XKB_KEY_DOWN: u32 = 0xff54;
const XKB_KEY_END: u32 = 0xff57;
const XKB_KEY_DELETE: u32 = 0xffff;

fn style() -> TextStyle {
    TextStyle {
        size: 14.0,
        weight: Weight::Regular,
        tabular: false,
        role: Role::TextHi,
    }
}

fn system() -> MutexGuard<'static, TextSystem> {
    static SYSTEM: OnceLock<Mutex<TextSystem>> = OnceLock::new();
    let lock = SYSTEM.get_or_init(|| {
        Mutex::new(TextSystem::new(&default_font_dirs()).expect("Inter Variable in the checkout"))
    });
    lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn search_key() -> Key {
    Key::named("search")
}

fn row_key() -> Key {
    Key::named("row")
}

/// The field at x 20..220 and a focusable row under it, in a padded column,
/// with the field first or second.
fn card(field_first: bool, id: FieldId) -> View {
    let field = view::field(search_key(), id, "Buscar", style()).width(200.0);
    let row = view::row(row_key()).height(40.0).interactive().focusable();
    let kids = if field_first {
        [field, row]
    } else {
        [row, field]
    };
    view::col(Key::named("card"))
        .width(300.0)
        .padding(20.0)
        .children(kids)
}

fn laid_out(root: View) -> Tree {
    let mut tree = Tree::new(false, &Theme::dark());
    tree.apply(root, Instant::now());
    tree.layout(AVAILABLE, &mut system(), 1.0);
    tree
}

fn key_event(keysym: u32, utf8: Option<&str>, modifiers: Modifiers) -> InputEvent {
    InputEvent::Key {
        keysym,
        utf8: utf8.map(str::to_owned),
        modifiers,
        pressed: true,
        serial: 1,
    }
}

fn plain(keysym: u32) -> InputEvent {
    key_event(keysym, None, Modifiers::default())
}

fn typed(s: &str) -> InputEvent {
    key_event(0x61, Some(s), Modifiers::default())
}

fn press(x: f32, y: f32) -> InputEvent {
    InputEvent::PointerPress {
        x,
        y,
        button: BTN_LEFT,
        serial: 1,
    }
}

/// A tree with the field focused by Tab.
fn focused_field() -> (Tree, Interaction) {
    let mut tree = laid_out(card(true, SEARCH));
    let mut ia = Interaction::new();
    ia.key(&mut tree, &plain(XKB_KEY_TAB));
    let field = tree.node_by_key(search_key());
    assert_eq!(tree.highlight().focused, field);
    (tree, ia)
}

fn text(tree: &Tree) -> String {
    tree.field(SEARCH).expect("the field").text().to_owned()
}

#[test]
fn reconcile_keeps_the_field_state_across_keyed_updates() {
    let (mut tree, mut ia) = focused_field();
    let node = tree.node_by_key(search_key()).expect("field node");
    assert_eq!(
        ia.key(&mut tree, &typed("h")),
        vec![UiEvent::FieldChanged(SEARCH)]
    );
    ia.key(&mut tree, &typed("ola"));
    ia.key(&mut tree, &key_event(XKB_KEY_LEFT, None, shift()));
    assert_eq!(text(&tree), "hola");
    assert_eq!(tree.field(SEARCH).and_then(|f| f.selection()), Some(3..4));

    // The same view again, then the field moved below the row: the node, its
    // text, cursor, selection and focus survive.
    let t = Instant::now();
    tree.apply(card(true, SEARCH), t);
    tree.apply(card(false, SEARCH), t);
    tree.layout(AVAILABLE, &mut system(), 1.0);
    assert_eq!(tree.node_by_key(search_key()), Some(node));
    assert_eq!(text(&tree), "hola");
    let f = tree.field(SEARCH).expect("the field");
    assert_eq!((f.cursor(), f.selection()), (3, Some(3..4)));
    assert_eq!(tree.highlight().focused, Some(node));
    // It still edits afterwards.
    ia.key(&mut tree, &typed("X"));
    assert_eq!(text(&tree), "holX");

    // Another field id under the same key is another field: it starts empty.
    tree.apply(card(false, OTHER), t);
    assert!(tree.field(SEARCH).is_none());
    assert_eq!(tree.field(OTHER).map(|f| f.text()), Some(""));

    // A node that stops being a field drops its editor; one that becomes a
    // field gets a new one.
    tree.apply(
        view::col(Key::named("card"))
            .child(view::row(search_key()).height(10.0))
            .child(view::row(row_key()).height(10.0)),
        t,
    );
    assert!(tree.field(OTHER).is_none());
    tree.apply(card(true, SEARCH), t);
    assert_eq!(tree.field(SEARCH).map(|f| f.text()), Some(""));
}

fn shift() -> Modifiers {
    Modifiers {
        shift: true,
        ..Modifiers::default()
    }
}

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        ..Modifiers::default()
    }
}

#[test]
fn text_set_through_the_tree_survives_a_rebuild() {
    let mut tree = laid_out(card(true, SEARCH));
    tree.field_mut(SEARCH).expect("the field").set_text("abc");
    tree.apply(card(true, SEARCH), Instant::now());
    assert_eq!(text(&tree), "abc");
}

#[test]
fn a_focused_field_consumes_its_editing_keys() {
    let (mut tree, mut ia) = focused_field();
    // Space inserts, it does not activate.
    assert_eq!(
        ia.key(
            &mut tree,
            &key_event(XKB_KEY_SPACE, Some(" "), Modifiers::default())
        ),
        vec![UiEvent::FieldChanged(SEARCH)]
    );
    ia.key(&mut tree, &typed("ab"));
    assert_eq!(text(&tree), " ab");
    for ev in [
        plain(XKB_KEY_LEFT),
        plain(XKB_KEY_RIGHT),
        plain(XKB_KEY_HOME),
        plain(XKB_KEY_END),
        plain(XKB_KEY_BACKSPACE),
        plain(XKB_KEY_DELETE),
        key_event(0x61, None, ctrl()),
        key_event(XKB_KEY_LEFT, None, ctrl()),
        typed("x"),
        key_event(XKB_KEY_SPACE, Some(" "), Modifiers::default()),
    ] {
        assert!(ia.field_consumes(&tree, &ev), "{ev:?}");
    }
    // Home then Delete removes the first character; Ctrl+A then Backspace
    // empties the field; Ctrl+Left jumps a word.
    ia.key(&mut tree, &plain(XKB_KEY_HOME));
    ia.key(&mut tree, &plain(XKB_KEY_DELETE));
    assert_eq!(text(&tree), "ab");
    ia.key(&mut tree, &key_event(XKB_KEY_END, None, shift()));
    assert_eq!(tree.field(SEARCH).and_then(|f| f.selection()), Some(0..2));
    ia.key(&mut tree, &key_event(0x61, None, ctrl()));
    ia.key(&mut tree, &plain(XKB_KEY_BACKSPACE));
    assert_eq!(text(&tree), "");
    // An edit that changes nothing consumes the key without an event.
    assert!(ia.key(&mut tree, &plain(XKB_KEY_BACKSPACE)).is_empty());
    assert!(ia.field_consumes(&tree, &plain(XKB_KEY_BACKSPACE)));
    // A key release and a Ctrl/Alt chord with text are not edits.
    let release = InputEvent::Key {
        keysym: 0x61,
        utf8: Some("a".into()),
        modifiers: Modifiers::default(),
        pressed: false,
        serial: 2,
    };
    assert!(!ia.field_consumes(&tree, &release));
    assert!(!ia.field_consumes(
        &tree,
        &key_event(
            0x62,
            Some("b"),
            Modifiers {
                alt: true,
                ..Modifiers::default()
            }
        )
    ));
    assert_eq!(text(&tree), "");
}

#[test]
fn escape_enter_up_down_and_tab_pass_through_a_focused_field() {
    let (mut tree, mut ia) = focused_field();
    ia.key(&mut tree, &typed("q"));
    for keysym in [XKB_KEY_ESCAPE, XKB_KEY_RETURN, XKB_KEY_UP, XKB_KEY_DOWN] {
        assert!(!ia.field_consumes(&tree, &plain(keysym)), "{keysym:#x}");
    }
    // Their control-character text does not insert either.
    for (keysym, s) in [
        (XKB_KEY_RETURN, "\r"),
        (XKB_KEY_ESCAPE, "\u{1b}"),
        (XKB_KEY_TAB, "\t"),
    ] {
        let ev = key_event(keysym, Some(s), Modifiers::default());
        assert!(!ia.field_consumes(&tree, &ev), "{keysym:#x}");
    }
    assert_eq!(
        ia.key(&mut tree, &plain(XKB_KEY_ESCAPE)),
        vec![UiEvent::Escape]
    );
    assert_eq!(
        ia.key(&mut tree, &plain(XKB_KEY_RETURN)),
        vec![UiEvent::Activate(search_key())]
    );
    assert!(ia.key(&mut tree, &plain(XKB_KEY_UP)).is_empty());
    assert!(ia.key(&mut tree, &plain(XKB_KEY_DOWN)).is_empty());
    assert_eq!(text(&tree), "q");
    // Tab still moves the focus off the field.
    ia.key(&mut tree, &plain(XKB_KEY_TAB));
    assert_eq!(tree.highlight().focused, tree.node_by_key(row_key()));
    // Without the focus a key edits nothing.
    assert!(!ia.field_consumes(&tree, &typed("z")));
    ia.key(&mut tree, &typed("z"));
    assert_eq!(text(&tree), "q");
}

#[test]
fn a_press_focuses_the_field_places_the_cursor_and_a_drag_selects() {
    let mut tree = laid_out(card(true, SEARCH));
    let mut ia = Interaction::new();
    tree.field_mut(SEARCH)
        .expect("the field")
        .set_text("hello world");
    tree.layout(AVAILABLE, &mut system(), 1.0);
    let node = tree.node_by_key(search_key()).expect("field node");
    let rect = tree.rect(node);
    assert_eq!((rect.x, rect.w), (20.0, 200.0));
    let (mid, end) = {
        let run = tree.field(SEARCH).and_then(|f| f.run()).expect("shaped");
        (run.caret_x(5), run.size().0)
    };
    let y = rect.y + rect.h / 2.0;
    let events = ia.pointer(&mut tree, &press(rect.x + mid + 0.5, y), Instant::now());
    assert!(
        events.contains(&UiEvent::FieldChanged(SEARCH)),
        "{events:?}"
    );
    assert_eq!(tree.highlight().focused, Some(node));
    assert_eq!(tree.field(SEARCH).map(|f| f.cursor()), Some(5));
    // No recoil for a field.
    assert_eq!(tree.scale(node, Instant::now()), 1.0);
    // Dragging right, outside the field, selects to the end.
    let events = ia.pointer(
        &mut tree,
        &InputEvent::PointerMotion {
            x: rect.x + end + 30.0,
            y: y + 80.0,
        },
        Instant::now(),
    );
    assert!(
        events.contains(&UiEvent::FieldChanged(SEARCH)),
        "{events:?}"
    );
    assert_eq!(tree.field(SEARCH).and_then(|f| f.selection()), Some(5..11));
    // After the release, motion no longer selects.
    ia.pointer(
        &mut tree,
        &InputEvent::PointerRelease {
            x: rect.x + end,
            y,
            button: BTN_LEFT,
            serial: 2,
        },
        Instant::now(),
    );
    let events = ia.pointer(
        &mut tree,
        &InputEvent::PointerMotion { x: rect.x, y },
        Instant::now(),
    );
    assert!(!events.contains(&UiEvent::FieldChanged(SEARCH)));
    assert_eq!(tree.field(SEARCH).and_then(|f| f.selection()), Some(5..11));
    // Typing now replaces the selection; the focused field took the press.
    ia.key(&mut tree, &typed("!"));
    assert_eq!(text(&tree), "hello!");
}

#[test]
fn ime_and_paste_events_edit_the_focused_field_only() {
    let mut tree = laid_out(card(true, SEARCH));
    let mut ia = Interaction::new();
    // Without a focused field they are dropped.
    let paste = InputEvent::Paste("x".into());
    assert!(!ia.field_consumes(&tree, &paste));
    assert!(ia.key(&mut tree, &paste).is_empty());
    assert_eq!(text(&tree), "");

    let (mut tree, mut ia) = focused_field();
    let composing = InputEvent::Preedit {
        text: "\u{4f60}".into(),
        cursor: 3,
    };
    assert!(ia.field_consumes(&tree, &composing));
    assert_eq!(
        ia.key(&mut tree, &composing),
        vec![UiEvent::FieldChanged(SEARCH)]
    );
    assert_eq!(
        tree.field(SEARCH).and_then(|f| f.preedit()),
        Some("\u{4f60}")
    );
    ia.key(
        &mut tree,
        &InputEvent::CommitText("\u{4f60}\u{597d}".into()),
    );
    assert_eq!(tree.field(SEARCH).and_then(|f| f.preedit()), None);
    assert_eq!(text(&tree), "\u{4f60}\u{597d}");
    assert_eq!(
        ia.key(&mut tree, &InputEvent::Paste("!".into())),
        vec![UiEvent::FieldChanged(SEARCH)]
    );
    assert_eq!(text(&tree), "\u{4f60}\u{597d}!");
}

#[test]
fn an_empty_field_measures_one_line_of_its_style() {
    let tree = laid_out(card(true, SEARCH));
    let node = tree.node_by_key(search_key()).expect("field node");
    let rect = tree.rect(node);
    assert_eq!(rect.w, 200.0);
    assert!(rect.h >= 14.0 && rect.h < 40.0, "{}", rect.h);
}
