//! The Workspaces piece: this output's workspaces as dots, the active one a
//! wider accent pill, and the Activity page. A dot is a hit target taller
//! than its mark; the keys are the capsule's (`("dot", workspace id)`).

use celestina_paint::Role;
use celestina_ui::{view, Key, View};

use super::{BarCx, Gesture, PageEntry, PageId, Piece, PieceAction, PieceId, PieceInput};
use crate::{
    copy,
    island::{
        pages::activity::Activity,
        view::{centred, DOT, DOT_GAP, PILL_W},
        workspaces::{rows_for, Row},
    },
};

/// The space around a dot's mark that still counts as the dot, on each
/// side. Two of them make the gap between marks [`DOT_GAP`].
const HIT_PAD: f32 = DOT_GAP / 2.0;
/// A dot's hit target is this tall.
const HIT_H: f32 = 20.0;

/// The Workspaces piece.
#[derive(Clone, Copy, Debug)]
pub struct Workspaces;

/// The node of workspace `id`.
fn dot_key(id: u64) -> Key {
    Key::indexed("dot", id)
}

impl Workspaces {
    /// The accessible name of the node `key` of the widget built from `cx`:
    /// the workspace's label, saying whether it is the current one.
    #[must_use]
    pub fn label(key: Key, cx: &BarCx<'_>) -> Option<String> {
        let rows = rows_of(cx);
        let row = rows.iter().find(|r| dot_key(r.id) == key)?;
        Some(copy::bar_workspace_name(&row.text, row.active))
    }
}

fn rows_of(cx: &BarCx<'_>) -> Vec<Row> {
    cx.model.map_or_else(Vec::new, |m| rows_for(m, cx.output))
}

fn dot(row: &Row) -> View {
    let (w, role) = if row.active {
        (PILL_W, Role::Accent)
    } else {
        (DOT, Role::TextLo)
    };
    let mark = view::row(Key::indexed("dot-mark", row.id))
        .fill(role)
        .radius(DOT / 2.0)
        .size(w, DOT);
    centred(view::row(dot_key(row.id)))
        .height(HIT_H)
        .padding_xy(HIT_PAD, 0.0)
        .interactive()
        .focusable()
        .child(mark)
}

impl Piece for Workspaces {
    fn id(&self) -> PieceId {
        PieceId("workspaces")
    }

    fn name(&self) -> &'static str {
        copy::PIECE_WORKSPACES
    }

    /// The dots; absent while the compositor reports no workspace for the
    /// output.
    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let rows = rows_of(cx);
        if rows.is_empty() {
            return None;
        }
        Some(centred(view::row(Key::named("workspaces"))).children(rows.iter().map(dot)))
    }

    fn page(&self) -> Option<PageEntry> {
        Some(PageEntry {
            id: PageId::ACTIVITY,
            view: &Activity,
        })
    }

    /// A click on a dot focuses its workspace, if it is still one
    /// of the output's; nothing else does anything.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        if input.gesture != Gesture::Primary {
            return PieceAction::None;
        }
        rows_of(input.cx)
            .iter()
            .find(|r| dot_key(r.id) == input.key)
            .map_or(PieceAction::None, |r| PieceAction::FocusWorkspace(r.id))
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::Theme;
    use celestina_ui::ViewKind;
    use taffy::Dimension;

    use super::*;
    use crate::{
        island::pages::NameOwner,
        pieces::Axis,
        providers::niri::{NiriModel, Workspace},
    };

    fn ws(id: u64, idx: u8, name: Option<&str>, output: &str, active: bool) -> Workspace {
        Workspace {
            id,
            idx,
            name: name.map(str::to_owned),
            output: Some(output.to_owned()),
            is_active: active,
            is_focused: active,
            columns: 1,
        }
    }

    fn model() -> NiriModel {
        NiriModel {
            workspaces: vec![
                ws(11, 1, None, "DP-1", false),
                ws(12, 2, Some("web"), "DP-1", true),
                ws(31, 1, None, "HDMI-A-1", true),
            ],
            ..NiriModel::default()
        }
    }

    fn with<R>(model: Option<&NiriModel>, output: &str, f: impl FnOnce(&BarCx<'_>) -> R) -> R {
        let theme = Theme::dark();
        let owners = NameOwner::default();
        f(&BarCx {
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            theme: &theme,
            output,
            model,
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            tray_hidden: &[],
            sysmon: None,
            weather: None,
            today: None,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            levels: crate::pieces::Levels::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
            bubbles: &crate::bubbles::NONE,
        })
    }

    /// The pre-order `(key, width)` of the marks and the targets.
    fn shape(v: &View) -> Vec<(Key, Dimension)> {
        let mut out = Vec::new();
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            out.push((n.key, n.style.size.width));
            stack.extend(n.children.iter().rev());
        }
        out
    }

    #[test]
    fn the_dots_are_this_outputs_workspaces_with_the_active_one_a_pill() {
        let m = model();
        let v = with(Some(&m), "DP-1", |cx| Workspaces.bar(cx)).expect("dots");
        assert_eq!(
            shape(&v),
            [
                (Key::named("workspaces"), Dimension::auto()),
                (dot_key(11), Dimension::auto()),
                (Key::indexed("dot-mark", 11), Dimension::length(DOT)),
                (dot_key(12), Dimension::auto()),
                (Key::indexed("dot-mark", 12), Dimension::length(PILL_W)),
            ]
        );
        let mark = &v.children[1].children[0];
        assert!(matches!(
            mark.kind,
            ViewKind::Box {
                fill: Some(Role::Accent),
                ..
            }
        ));
        assert!(v.children.iter().all(|d| d.interactive && d.focusable));
    }

    #[test]
    fn without_workspaces_the_piece_is_absent() {
        let m = model();
        assert!(with(None, "DP-1", |cx| Workspaces.bar(cx)).is_none());
        assert!(with(Some(&m), "DP-9", |cx| Workspaces.bar(cx)).is_none());
        let empty = NiriModel::default();
        assert!(with(Some(&empty), "DP-1", |cx| Workspaces.bar(cx)).is_none());
    }

    fn act(key: Key, gesture: Gesture, model: Option<&NiriModel>) -> PieceAction {
        with(model, "DP-1", |cx| {
            Workspaces.activate(&PieceInput { key, gesture, cx })
        })
    }

    #[test]
    fn a_primary_input_on_a_dot_focuses_its_workspace() {
        let m = model();
        assert_eq!(
            act(dot_key(11), Gesture::Primary, Some(&m)),
            PieceAction::FocusWorkspace(11)
        );
        assert_eq!(
            act(dot_key(12), Gesture::Primary, Some(&m)),
            PieceAction::FocusWorkspace(12)
        );
    }

    #[test]
    fn everything_else_asks_for_nothing() {
        let m = model();
        for gesture in [
            Gesture::Middle,
            Gesture::Context,
            Gesture::Scroll(Axis::Vertical, 1),
            Gesture::Scroll(Axis::Horizontal, -1),
        ] {
            assert_eq!(act(dot_key(11), gesture, Some(&m)), PieceAction::None);
        }
        // Another output's workspace, a stale one, a foreign key, no model.
        assert_eq!(
            act(dot_key(31), Gesture::Primary, Some(&m)),
            PieceAction::None
        );
        assert_eq!(
            act(dot_key(99), Gesture::Primary, Some(&m)),
            PieceAction::None
        );
        assert_eq!(
            act(Key::named("workspaces"), Gesture::Primary, Some(&m)),
            PieceAction::None
        );
        assert_eq!(act(dot_key(11), Gesture::Primary, None), PieceAction::None);
    }

    #[test]
    fn the_labels_name_the_workspace_and_say_which_is_current() {
        let m = model();
        with(Some(&m), "DP-1", |cx| {
            assert_eq!(
                Workspaces::label(dot_key(11), cx),
                Some(copy::bar_workspace_name(&copy::workspace_number(1), false))
            );
            assert_eq!(
                Workspaces::label(dot_key(12), cx),
                Some(copy::bar_workspace_name("web", true))
            );
            assert_eq!(Workspaces::label(dot_key(31), cx), None);
        });
    }

    #[test]
    fn the_piece_opens_the_activity_page() {
        assert_eq!(Workspaces.page().map(|e| e.id), Some(PageId::ACTIVITY));
    }
}
