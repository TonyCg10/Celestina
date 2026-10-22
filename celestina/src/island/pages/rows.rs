//! The action rows of the connectivity pages (network, Bluetooth, power):
//! one keyed [`Line`] per thing, with at most one action, the request's
//! state (pending, or the sentence of its failure) and an optional note
//! row beneath it. A page builds its lines once ([`fit`] bounds them to what
//! the surface can draw) and both its view ([`list`]) and its
//! `action_for` read that one list, so a node can only ask for what the page
//! drew for it.

use celestina_paint::{Role, Theme, Weight};
use celestina_shell::connectivity::{Action, Failure, Refusal};
use celestina_ui::{view, Key, View};
use taffy::{AlignItems, JustifyContent};

use super::NOTE_MAX_W;
use crate::{
    connectivity::state::RowStatus,
    copy,
    island::view::{centred, chrome_height, switch, text_style},
};

/// A row's icon box, logical px.
pub const GLYPH: f32 = 24.0;
/// The signal bars' width and the gap between two.
const BAR_W: f32 = 3.0;
const BAR_GAP: f32 = 2.0;
/// How many bars a signal has.
const BARS: u8 = 4;

/// What a row draws as its icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    /// A semantic icon.
    Icon(&'static str),
    /// Wi-Fi signal bars for a strength in percent.
    Bars(u8),
    /// Nothing, but the space is kept so titles line up.
    Blank,
}

/// What a row shows at its trailing edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trailing {
    Nothing,
    /// A state word.
    Text(String, Role),
    /// A switch showing `on`, drawn live only when `enabled`.
    Switch {
        on: bool,
        enabled: bool,
    },
}

/// One row of a connectivity page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// The row's node. Its parts' keys derive from it.
    pub key: Key,
    /// What the parts' keys are derived from; unique per row of a page.
    pub tag: String,
    pub glyph: Glyph,
    pub tint: Role,
    pub title: String,
    pub title_role: Role,
    pub detail: Option<String>,
    pub trailing: Trailing,
    /// What a click or Enter on the row asks for; `None` for a row that is
    /// only shown (it is neither focusable nor interactive).
    pub action: Option<Action>,
    /// The sentence of a failure, in its own row beneath this one.
    pub note: Option<(String, Role)>,
}

impl Line {
    /// A row that only says `title`, with nothing to do.
    #[must_use]
    pub fn text(tag: &str, title: &str, role: Role) -> Line {
        Line {
            key: Key::named(tag),
            tag: tag.to_owned(),
            glyph: Glyph::Blank,
            tint: role,
            title: title.to_owned(),
            title_role: role,
            detail: None,
            trailing: Trailing::Nothing,
            action: None,
            note: None,
        }
    }

    /// The rows of height this line takes: itself and its note.
    fn units(&self) -> usize {
        1 + usize::from(self.note.is_some())
    }
}

/// How the request state of a row shows: a word at the trailing edge while
/// it waits, and a note once it failed.
pub fn apply_status(line: &mut Line, status: RowStatus) {
    match status {
        RowStatus::Idle => {}
        RowStatus::Pending => {
            if matches!(line.trailing, Trailing::Switch { .. }) {
                line.detail = Some(copy::APPLYING.to_owned());
            } else {
                line.trailing = Trailing::Text(copy::APPLYING.to_owned(), Role::TextLo);
            }
            line.action = None;
        }
        RowStatus::Failed(failure) => {
            line.note = Some((failure_sentence(failure).to_owned(), Role::Danger));
        }
    }
}

/// The sentence for a failure, by kind (never service text).
#[must_use]
pub fn failure_sentence(failure: Failure) -> &'static str {
    match failure {
        Failure::Timeout => copy::FAIL_TIMEOUT,
        Failure::NeedsSecret => copy::FAIL_NEEDS_SECRET,
        Failure::Denied => copy::FAIL_DENIED,
        Failure::NotFound => copy::FAIL_NOT_FOUND,
        Failure::Unreachable => copy::FAIL_UNREACHABLE,
        Failure::Busy => copy::FAIL_BUSY,
        Failure::Refused(refusal) => refusal_sentence(refusal),
        Failure::Other => copy::FAIL_OTHER,
    }
}

fn refusal_sentence(refusal: Refusal) -> &'static str {
    match refusal {
        Refusal::NoInventory(_) => copy::REFUSED_NO_INVENTORY,
        Refusal::UnknownId => copy::REFUSED_UNKNOWN,
        Refusal::NotKnown => copy::REFUSED_NOT_KNOWN,
        Refusal::KindNotAllowed => copy::REFUSED_KIND,
        Refusal::NeedsSecretClass => copy::REFUSED_SECRET_CLASS,
        Refusal::NotActive => copy::REFUSED_NOT_ACTIVE,
        Refusal::NotPaired => copy::REFUSED_NOT_PAIRED,
        Refusal::WifiDisabled => copy::REFUSED_WIFI_OFF,
        Refusal::HardwareBlocked => copy::REFUSED_HARDWARE,
        Refusal::NoSuchProfile => copy::REFUSED_NO_PROFILE,
        Refusal::NothingToCycle => copy::REFUSED_NOTHING_TO_CYCLE,
    }
}

/// How many rows of the page's content fit on a surface of `max_size`.
#[must_use]
pub fn room(theme: &Theme, max_size: (f32, f32)) -> usize {
    let free = (max_size.1 - chrome_height(theme)).max(0.0);
    (free / theme.row_height).floor() as usize
}

/// The lines a page draws on a surface of `max_size`: the `top` and
/// `bottom` lines whenever they fit, and as many `body` lines (with their
/// notes) as the room leaves. Nothing the surface cannot show is built, so
/// nothing invisible can be focused or clicked.
#[must_use]
pub fn fit(
    theme: &Theme,
    max_size: (f32, f32),
    top: Vec<Line>,
    body: Vec<Line>,
    bottom: Vec<Line>,
) -> Vec<Line> {
    let room = room(theme, max_size);
    let fixed: usize = top.iter().chain(&bottom).map(Line::units).sum();
    let mut out = Vec::new();
    if fixed > room {
        let mut used = 0;
        for line in top.into_iter().chain(bottom) {
            used += line.units();
            if used > room {
                break;
            }
            out.push(line);
        }
        return out;
    }
    let mut spare = room - fixed;
    out.extend(top);
    for line in body {
        if line.units() > spare {
            break;
        }
        spare -= line.units();
        out.push(line);
    }
    out.extend(bottom);
    out
}

/// The height the lines need, with the page's chrome.
#[must_use]
pub fn needed(theme: &Theme, lines: &[Line]) -> f32 {
    chrome_height(theme) + lines.iter().map(Line::units).sum::<usize>() as f32 * theme.row_height
}

/// The line drawn for `key` that has an action, if any.
#[must_use]
pub fn action_of(lines: &[Line], key: Key) -> Option<Action> {
    lines
        .iter()
        .find(|line| line.key == key)
        .and_then(|line| line.action.clone())
}

fn sub(tag: &str, part: &str) -> Key {
    Key::named(&format!("{tag}:{part}"))
}

/// Wi-Fi signal bars `size` tall, `lit` of the four in `on` and the rest in
/// `off`: stepped heights, aligned at the bottom.
#[must_use]
pub fn signal_bars(tag: &str, signal: u8, size: f32, on: Role, off: Role) -> View {
    let lit = u16::from(signal.min(100)).div_ceil(25);
    let mut bars = view::row(sub(tag, "bars")).gap(BAR_GAP).size(
        f32::from(BARS) * BAR_W + f32::from(BARS - 1) * BAR_GAP,
        size,
    );
    bars.style.align_items = AlignItems::FLEX_END;
    bars.children((1..=u16::from(BARS)).map(|step| {
        let role = if step <= lit { on } else { off };
        let height = size * f32::from(step) / f32::from(BARS);
        view::row(sub(tag, &format!("bar-{step}")))
            .fill(role)
            .radius(BAR_W / 2.0)
            .size(BAR_W, height)
    }))
}

fn glyph_view(line: &Line) -> View {
    let mut box_ = centred(view::row(sub(&line.tag, "glyph"))).size(GLYPH, GLYPH);
    box_.style.justify_content = JustifyContent::CENTER;
    match line.glyph {
        Glyph::Icon(name) => {
            box_.child(view::icon(sub(&line.tag, "icon"), name, GLYPH * 0.75).tint(line.tint))
        }
        Glyph::Bars(signal) => box_.child(signal_bars(
            &line.tag,
            signal,
            GLYPH * 0.6,
            line.tint,
            Role::TextFaint,
        )),
        Glyph::Blank => box_,
    }
}

fn row_view(theme: &Theme, line: &Line) -> View {
    let live = line.action.is_some();
    let texts = view::col(sub(&line.tag, "texts"))
        .grow()
        .child(view::text(
            sub(&line.tag, "title"),
            &line.title,
            text_style(
                theme.type_scale.body,
                Weight::Regular,
                false,
                line.title_role,
            ),
        ))
        .children(line.detail.as_ref().map(|detail| {
            view::text(
                sub(&line.tag, "detail"),
                detail,
                text_style(
                    theme.type_scale.caption,
                    Weight::Regular,
                    false,
                    Role::TextLo,
                ),
            )
        }));
    let trailing = match &line.trailing {
        Trailing::Nothing => None,
        Trailing::Text(text, role) => Some(view::text(
            sub(&line.tag, "state"),
            text,
            text_style(theme.type_scale.caption, Weight::Regular, false, *role),
        )),
        Trailing::Switch { on, enabled } => Some(switch(sub(&line.tag, "switch"), *on, *enabled)),
    };
    let mut row = centred(view::row(line.key))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_md)
        .child(glyph_view(line))
        .child(texts)
        .children(trailing);
    if live {
        row = row.interactive().focusable();
    }
    row
}

fn note_view(theme: &Theme, line: &Line, text: &str, role: Role) -> View {
    centred(view::row(sub(&line.tag, "note")))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md + GLYPH + theme.spacing.md, 0.0)
        .child(
            view::text(
                sub(&line.tag, "note-text"),
                text,
                text_style(theme.type_scale.caption, Weight::Regular, false, role),
            )
            .max_width(NOTE_MAX_W - GLYPH - 2.0 * theme.spacing.md)
            .grow(),
        )
}

/// The column of `lines`, each followed by its note.
#[must_use]
pub fn list(theme: &Theme, key: Key, lines: &[Line]) -> View {
    view::col(key).children(lines.iter().flat_map(|line| {
        let note = line
            .note
            .as_ref()
            .map(|(text, role)| note_view(theme, line, text, *role));
        std::iter::once(row_view(theme, line)).chain(note)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(tag: &str, note: bool) -> Line {
        let mut line = Line::text(tag, tag, Role::TextHi);
        if note {
            line.note = Some(("x".to_owned(), Role::Danger));
        }
        line
    }

    fn tags(lines: &[Line]) -> Vec<&str> {
        lines.iter().map(|l| l.tag.as_str()).collect()
    }

    fn surface(rows: usize) -> (Theme, (f32, f32)) {
        let theme = Theme::dark();
        let h = chrome_height(&theme) + rows as f32 * theme.row_height;
        (theme, (760.0, h))
    }

    #[test]
    fn fit_keeps_the_ends_and_cuts_the_body() {
        let (theme, size) = surface(4);
        let top = vec![line("top", false)];
        let body: Vec<Line> = (0..10).map(|n| line(&format!("b{n}"), false)).collect();
        let bottom = vec![line("bottom", false)];
        let out = fit(&theme, size, top, body, bottom);
        assert_eq!(tags(&out), ["top", "b0", "b1", "bottom"]);
    }

    #[test]
    fn a_note_counts_as_a_row_and_a_row_that_does_not_fit_with_it_is_cut() {
        let (theme, size) = surface(4);
        let body = vec![line("a", true), line("b", true), line("c", false)];
        let out = fit(&theme, size, vec![line("top", false)], body, Vec::new());
        // top (1) + a with its note (2) = 3; b needs 2 and only 1 is left.
        assert_eq!(tags(&out), ["top", "a"]);
    }

    #[test]
    fn when_even_the_ends_do_not_fit_the_top_goes_first() {
        let (theme, size) = surface(1);
        let out = fit(
            &theme,
            size,
            vec![line("top", false)],
            vec![line("b", false)],
            vec![line("bottom", false)],
        );
        assert_eq!(tags(&out), ["top"]);
        let (theme, size) = surface(0);
        assert!(fit(&theme, size, vec![line("t", false)], Vec::new(), Vec::new()).is_empty());
    }

    #[test]
    fn a_surface_shorter_than_the_chrome_has_no_room() {
        let theme = Theme::dark();
        assert_eq!(room(&theme, (760.0, 0.0)), 0);
        assert_eq!(room(&theme, (760.0, f32::NAN)), 0);
    }

    #[test]
    fn the_bars_light_one_per_quarter() {
        let lit = |signal: u8| {
            let v = signal_bars("n", signal, 12.0, Role::TextHi, Role::TextFaint);
            v.children
                .iter()
                .filter(|bar| {
                    matches!(
                        bar.kind,
                        celestina_ui::ViewKind::Box {
                            fill: Some(Role::TextHi),
                            ..
                        }
                    )
                })
                .count()
        };
        assert_eq!(
            [0, 1, 25, 26, 50, 75, 76, 100, 255].map(lit),
            [0, 1, 1, 2, 2, 3, 4, 4, 4]
        );
    }

    #[test]
    fn a_pending_row_is_not_actionable_and_a_failed_one_carries_its_note() {
        let mut pending = line("p", false);
        pending.action = Some(Action::CycleProfile);
        apply_status(&mut pending, RowStatus::Pending);
        assert!(pending.action.is_none());
        let mut failed = line("f", false);
        failed.action = Some(Action::CycleProfile);
        apply_status(&mut failed, RowStatus::Failed(Failure::Denied));
        assert!(failed.action.is_some(), "a failed row may be tried again");
        assert_eq!(
            failed.note.as_ref().map(|n| n.0.as_str()),
            Some(copy::FAIL_DENIED)
        );
        assert_eq!(failed.units(), 2);
    }

    #[test]
    fn every_failure_has_its_own_sentence() {
        use std::collections::HashSet;
        let kinds = [
            Failure::Timeout,
            Failure::NeedsSecret,
            Failure::Denied,
            Failure::NotFound,
            Failure::Unreachable,
            Failure::Busy,
            Failure::Other,
        ];
        let sentences: HashSet<&str> = kinds.into_iter().map(failure_sentence).collect();
        assert_eq!(sentences.len(), kinds.len());
        let refusals = [
            Refusal::UnknownId,
            Refusal::NotKnown,
            Refusal::KindNotAllowed,
            Refusal::NeedsSecretClass,
            Refusal::NotActive,
            Refusal::NotPaired,
            Refusal::WifiDisabled,
            Refusal::HardwareBlocked,
            Refusal::NoSuchProfile,
            Refusal::NothingToCycle,
        ];
        let refused: HashSet<&str> = refusals
            .into_iter()
            .map(|r| failure_sentence(Failure::Refused(r)))
            .collect();
        assert_eq!(refused.len(), refusals.len());
    }
}
