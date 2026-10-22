//! The System monitor piece: `CPU 23% · RAM 41%` in tabular figures, coloured
//! by how loaded the busier meter is, and the System page. A click opens the
//! page. With no reading (no `/proc`, or the monitor withdrew it) the piece
//! is absent rather than showing zeros.

use celestina_paint::{Role, Weight};
use celestina_shell::glance::Load;
use celestina_ui::{view, Key, View};

use super::{BarCx, Gesture, PageEntry, PageId, Piece, PieceAction, PieceId, PieceInput};
use crate::{
    copy, glance,
    island::{
        pages::sysmon::Sysmon as Page,
        view::{centred, text_style},
    },
};

/// The space around the text that still counts as the widget.
const HIT_PAD: f32 = 5.0;

/// The System monitor piece.
#[derive(Clone, Copy, Debug)]
pub struct Sysmon;

fn widget_key() -> Key {
    Key::named("sysmon")
}

/// The colour of the readout at `load`: the theme's accent for an elevated
/// meter and its danger colour for a critical one (the theme has no warning
/// role).
#[must_use]
pub fn load_role(load: Load) -> Role {
    match load {
        Load::Normal => Role::TextHi,
        Load::Elevated => Role::Accent,
        Load::Critical => Role::Danger,
    }
}

impl Piece for Sysmon {
    fn id(&self) -> PieceId {
        PieceId(glance::SYSMON_PIECE)
    }

    fn name(&self) -> &'static str {
        copy::PAGE_SYSMON
    }

    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let reading = cx.sysmon?;
        let role = load_role(Load::of(reading.cpu.max(reading.ram)));
        Some(
            centred(view::row(widget_key()))
                .padding(HIT_PAD)
                .radius(cx.theme.radius_md)
                .interactive()
                .focusable()
                .child(view::text(
                    Key::named("sysmon-text"),
                    &copy::bar_sysmon(reading.cpu, reading.ram),
                    text_style(cx.theme.type_scale.body, Weight::Regular, true, role),
                )),
        )
    }

    fn page(&self) -> Option<PageEntry> {
        Some(PageEntry {
            id: PageId::SYSMON,
            view: &Page,
        })
    }

    /// Opens the page; the wheel and the other buttons do nothing.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        match (input.key == widget_key(), input.gesture) {
            (true, Gesture::Primary) => PieceAction::OpenPage(PageId::SYSMON),
            _ => PieceAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::Theme;
    use celestina_ui::ViewKind;

    use super::*;
    use crate::{
        island::pages::NameOwner,
        pieces::{Axis, BarCx},
        providers::sysmon::SysReading,
    };

    fn reading(cpu: u8, ram: u8) -> SysReading {
        SysReading {
            cpu,
            ram,
            ram_used_kib: 0,
            ram_total_kib: 0,
            temp_c: None,
            cpu_history: Vec::new(),
            ram_history: Vec::new(),
        }
    }

    fn with<R>(sysmon: Option<&SysReading>, f: impl FnOnce(&BarCx<'_>) -> R) -> R {
        let theme = Theme::dark();
        let owners = NameOwner::default();
        f(&BarCx {
            theme: &theme,
            output: "DP-1",
            model: None,
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            tray_hidden: &[],
            sysmon,
            weather: None,
            today: None,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            levels: crate::pieces::Levels::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
            bubbles: &crate::bubbles::NONE,
        })
    }

    fn input<'a>(key: Key, gesture: Gesture, cx: &'a BarCx<'a>) -> PieceInput<'a> {
        PieceInput { key, gesture, cx }
    }

    #[test]
    fn without_a_reading_the_piece_is_absent() {
        assert!(with(None, |cx| Sysmon.bar(cx)).is_none());
    }

    #[test]
    fn the_widget_reads_cpu_and_ram_in_tabular_figures() {
        let r = reading(23, 41);
        let v = with(Some(&r), |cx| Sysmon.bar(cx)).expect("a widget");
        assert_eq!(v.key, widget_key());
        assert!(v.interactive && v.focusable);
        let text = &v.children[0];
        let ViewKind::Text { text, style, .. } = &text.kind else {
            panic!("a text");
        };
        assert_eq!(text, "CPU 23% · RAM 41%");
        assert!(style.tabular);
        assert_eq!(style.role, Role::TextHi);
    }

    #[test]
    fn the_colour_follows_the_busier_meter() {
        let role = |cpu, ram| {
            let r = reading(cpu, ram);
            let v = with(Some(&r), |cx| Sysmon.bar(cx)).expect("a widget");
            let ViewKind::Text { style, .. } = &v.children[0].kind else {
                panic!("a text");
            };
            style.role
        };
        assert_eq!(role(79, 79), Role::TextHi);
        assert_eq!(role(80, 10), Role::Accent);
        assert_eq!(role(10, 89), Role::Accent);
        assert_eq!(role(10, 90), Role::Danger);
        assert_eq!(role(100, 100), Role::Danger);
    }

    #[test]
    fn a_click_opens_the_page_and_nothing_else_does() {
        let r = reading(1, 1);
        with(Some(&r), |cx| {
            let act = |key, gesture| Sysmon.activate(&input(key, gesture, cx));
            assert_eq!(
                act(widget_key(), Gesture::Primary),
                PieceAction::OpenPage(PageId::SYSMON)
            );
            assert_eq!(act(widget_key(), Gesture::Middle), PieceAction::None);
            assert_eq!(act(widget_key(), Gesture::Context), PieceAction::None);
            assert_eq!(
                act(widget_key(), Gesture::Scroll(Axis::Vertical, 1)),
                PieceAction::None
            );
            assert_eq!(
                act(Key::named("other"), Gesture::Primary),
                PieceAction::None
            );
        });
    }

    #[test]
    fn the_piece_is_named_and_has_its_page() {
        assert_eq!(Sysmon.id().as_str(), "sysmon");
        assert_eq!(Sysmon.name(), "Sistema");
        assert_eq!(Sysmon.page().map(|e| e.id), Some(PageId::SYSMON));
    }
}
