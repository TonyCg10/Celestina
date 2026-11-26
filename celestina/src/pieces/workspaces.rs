//! The Workspaces piece: this output's workspaces as dots, the active one a
//! wider accent pill, in one hit target. A click opens the workspaces sheet,
//! the wheel steps to the neighbouring workspace, open or not; the piece
//! keeps its Activity page. The dots are plain children
//! (`("dot", workspace id)`).

use celestina_paint::Role;
use celestina_ui::{view, Key, View};

use super::{
    Axis, BarCx, Face, Gesture, PageEntry, PageId, Piece, PieceAction, PieceId, PieceInput,
    SheetKind,
};
use crate::{
    copy,
    island::{
        pages::activity::Activity,
        view::{centred, DOT, DOT_GAP, PILL_W},
        workspaces::{rows_for, Row},
    },
};

/// The space around a dot's mark, on each side. Two of them make the gap
/// between marks [`DOT_GAP`].
const HIT_PAD: f32 = DOT_GAP / 2.0;
/// A dot's box is this tall.
const HIT_H: f32 = 20.0;
/// The space around the dots that still counts as the piece.
const PIECE_PAD: f32 = 5.0;
/// The piece's node.
const ID: &str = "workspaces";

/// The Workspaces piece.
#[derive(Clone, Copy, Debug)]
pub struct Workspaces;

/// The piece's one hit target.
#[must_use]
pub fn key() -> Key {
    Key::named(ID)
}

/// The workspace `steps` away from the active one in niri's order, clamped
/// to the ends with no wrap (the Wayland sign: positive is down, the next
/// workspace). `None` when nothing is active or the step would not move.
#[must_use]
pub fn step(rows: &[Row], steps: i32) -> Option<u64> {
    step_from(rows, None, steps)
}

/// [`step`] from the workspace `from` when it is one of `rows` (the last
/// target a wheel burst asked for), else from the active one.
#[must_use]
pub fn step_from(rows: &[Row], from: Option<u64>, steps: i32) -> Option<u64> {
    let from = from
        .and_then(|id| rows.iter().position(|r| r.id == id))
        .or_else(|| rows.iter().position(|r| r.active))?;
    let last = i64::try_from(rows.len()).ok()? - 1;
    let to = (i64::try_from(from).ok()? + i64::from(steps)).clamp(0, last);
    let to = usize::try_from(to).ok()?;
    (to != from).then(|| rows[to].id)
}

/// The node of workspace `id`.
fn dot_key(id: u64) -> Key {
    Key::indexed("dot", id)
}

impl Workspaces {
    /// The accessible name of the node `node` of the widget built from `cx`:
    /// the current workspace's label.
    #[must_use]
    pub fn label(node: Key, cx: &BarCx<'_>) -> Option<String> {
        if node != key() {
            return None;
        }
        let rows = rows_of(cx);
        let row = rows.iter().find(|r| r.active)?;
        Some(copy::bar_workspace_name(&row.text, true))
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
        .child(mark)
}

/// What the piece shows, and what the list's subtitle repeats.
#[must_use]
pub fn face() -> Face {
    Face {
        icon: "view-grid",
        text: None,
    }
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
        Some(
            centred(view::row(key()))
                .padding(PIECE_PAD)
                .radius(cx.theme.radius_md)
                .interactive()
                .focusable()
                .children(rows.iter().map(dot)),
        )
    }

    fn page(&self) -> Option<PageEntry> {
        Some(PageEntry {
            id: PageId::ACTIVITY,
            view: &Activity,
        })
    }

    /// The piece is a row of dots, with no icon or text of its own: the
    /// subtitle shows the grid glyph (Lucide `layout-grid`, which the icon
    /// set calls `view-grid`) and the list's name.
    fn face(&self, _cx: &BarCx<'_>) -> Option<Face> {
        Some(face())
    }

    /// A click opens the workspaces sheet; the wheel steps to the
    /// neighbouring workspace, if it moves. Nothing else does anything.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        if input.key != key() {
            return PieceAction::None;
        }
        match input.gesture {
            Gesture::Primary => PieceAction::Sheet(SheetKind::Workspaces),
            Gesture::Scroll(Axis::Vertical, steps) => step(&rows_of(input.cx), steps)
                .map_or(PieceAction::None, PieceAction::FocusWorkspace),
            Gesture::Middle | Gesture::Context | Gesture::Scroll(Axis::Horizontal, _) => {
                PieceAction::None
            }
        }
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
        pieces::{tests_support::workspace as ws, Axis},
        providers::niri::NiriModel,
    };

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
            media: &crate::island::pages::media::NO_MEDIA,
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
        assert!(v.children.iter().all(|d| !d.interactive && !d.focusable));
    }

    #[test]
    fn the_piece_is_one_hit_target_with_one_pill() {
        let m = model();
        let v = with(Some(&m), "DP-1", |cx| Workspaces.bar(cx)).expect("dots");
        assert!(v.interactive && v.focusable);
        let mut interactive = 0;
        let mut pills = 0;
        let mut stack = vec![&v];
        while let Some(n) = stack.pop() {
            interactive += usize::from(n.interactive);
            pills += usize::from(n.style.size.width == Dimension::length(PILL_W));
            stack.extend(n.children.iter());
        }
        assert_eq!(interactive, 1);
        assert_eq!(pills, 1);
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
    fn a_primary_input_anywhere_opens_the_workspaces_sheet() {
        let m = model();
        assert_eq!(
            act(key(), Gesture::Primary, Some(&m)),
            PieceAction::Sheet(SheetKind::Workspaces)
        );
        // A dot is no target of its own any more.
        assert_eq!(
            act(dot_key(11), Gesture::Primary, Some(&m)),
            PieceAction::None
        );
    }

    fn rows(n: u64, active: u64) -> Vec<Row> {
        (1..=n)
            .map(|i| Row {
                id: i * 10,
                text: copy::workspace_number(u8::try_from(i).unwrap_or(0)),
                idx: u8::try_from(i).unwrap_or(0),
                named: false,
                active: i == active,
                columns: 0,
            })
            .collect()
    }

    #[test]
    fn the_wheel_steps_to_the_neighbour_and_stops_at_the_ends() {
        let r = rows(6, 1);
        assert_eq!(step(&r, 1), Some(20));
        assert_eq!(step(&r, -1), None);
        assert_eq!(step(&r, 9), Some(60));
        assert_eq!(step(&r, 0), None);
        let last = rows(6, 6);
        assert_eq!(step(&last, 1), None);
        assert_eq!(step(&last, -2), Some(40));
        assert_eq!(step(&last, i32::MIN), Some(10));
        assert_eq!(step(&last, i32::MAX), None);
        assert_eq!(step(&rows(3, 0), 1), None, "no active workspace");
        assert_eq!(step(&[], 1), None);
    }

    #[test]
    fn a_step_from_a_pending_target_counts_from_it() {
        let r = rows(6, 2);
        assert_eq!(step_from(&r, Some(30), 1), Some(40));
        assert_eq!(
            step_from(&r, Some(999), 1),
            Some(30),
            "a gone target is ignored"
        );
        assert_eq!(step_from(&r, Some(60), 1), None);
    }

    #[test]
    fn the_piece_turns_the_wheel_into_a_focus_request() {
        let m = model();
        // DP-1: workspace 11 (idx 1), 12 (idx 2, active).
        assert_eq!(
            act(key(), Gesture::Scroll(Axis::Vertical, -1), Some(&m)),
            PieceAction::FocusWorkspace(11)
        );
        assert_eq!(
            act(key(), Gesture::Scroll(Axis::Vertical, 1), Some(&m)),
            PieceAction::None
        );
    }

    #[test]
    fn everything_else_asks_for_nothing() {
        let m = model();
        for gesture in [
            Gesture::Middle,
            Gesture::Context,
            Gesture::Scroll(Axis::Horizontal, 1),
            Gesture::Scroll(Axis::Horizontal, -1),
        ] {
            assert_eq!(act(key(), gesture, Some(&m)), PieceAction::None);
        }
        assert_eq!(
            act(Key::named("other"), Gesture::Primary, Some(&m)),
            PieceAction::None
        );
        assert_eq!(
            act(key(), Gesture::Primary, None),
            PieceAction::Sheet(SheetKind::Workspaces)
        );
    }

    #[test]
    fn the_label_names_the_current_workspace() {
        let m = model();
        with(Some(&m), "DP-1", |cx| {
            assert_eq!(
                Workspaces::label(key(), cx),
                Some(copy::bar_workspace_name("web", true))
            );
            assert_eq!(Workspaces::label(dot_key(11), cx), None);
        });
        with(None, "DP-1", |cx| {
            assert_eq!(Workspaces::label(key(), cx), None);
        });
    }

    #[test]
    fn the_piece_opens_the_activity_page() {
        assert_eq!(Workspaces.page().map(|e| e.id), Some(PageId::ACTIVITY));
    }
}
