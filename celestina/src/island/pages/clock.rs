//! The Clock page (CEL-2-K): the month calendar, Monday first, with buttons
//! to the previous month, back to today and to the next one, and below it the
//! weather reading when there is a fresh one. The seconds clock is the title
//! row's, as for every page.
//!
//! The page only describes. Which month it shows is the Island's state (an
//! offset from the current month, reset when the page closes); a click or
//! Enter on a button is turned into a [`Step`] by [`month_step`].

use celestina_paint::{Role, Weight};
use celestina_shell::{calendar::Month, glance::Today};
use celestina_ui::{view, Key, View};
use taffy::JustifyContent;

use super::{note_list, PageCx, PageView};
use crate::{
    copy,
    island::{
        clock as wall_clock,
        state::PAGE_SIZE,
        view::{button, centred, chrome_height, text_style},
    },
    providers::weather::WeatherReading,
};

/// The furthest the calendar goes from the current month, either way
/// (a hundred years).
pub const MAX_OFFSET: i32 = 1200;
/// A day cell's height, logical px.
const CELL_H: f32 = 24.0;
/// The weekday header's height.
const HEADER_H: f32 = 18.0;
/// The weather block's height.
const WEATHER_H: f32 = 36.0;
/// The weeks the grid always has, so the page keeps its size across months.
const WEEKS: usize = 6;

/// What a button of the page asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Previous,
    Today,
    Next,
}

impl Step {
    /// The offset after this step from `offset`, never beyond
    /// [`MAX_OFFSET`] either way.
    #[must_use]
    pub fn apply(self, offset: i32) -> i32 {
        match self {
            Step::Previous => offset.saturating_sub(1),
            Step::Today => 0,
            Step::Next => offset.saturating_add(1),
        }
        .clamp(-MAX_OFFSET, MAX_OFFSET)
    }
}

/// The Previous button.
#[must_use]
pub fn previous_key() -> Key {
    Key::named("clock-previous")
}

/// The Today button.
#[must_use]
pub fn today_key() -> Key {
    Key::named("clock-today")
}

/// The Next button.
#[must_use]
pub fn next_key() -> Key {
    Key::named("clock-next")
}

/// The step a click or Enter on `key` asks for, if it is one of the page's
/// buttons.
#[must_use]
pub fn month_step(key: Key) -> Option<Step> {
    if key == previous_key() {
        Some(Step::Previous)
    } else if key == today_key() {
        Some(Step::Today)
    } else if key == next_key() {
        Some(Step::Next)
    } else {
        None
    }
}

/// The month `offset` months from the one `today` is in; `None` only for a
/// year so far out that it is not a calendar year.
#[must_use]
pub fn shown_month(today: Today, offset: i32) -> Option<Month> {
    let index = i64::from(today.year) * 12 + i64::from(today.month) - 1 + i64::from(offset);
    let year = i32::try_from(index.div_euclid(12)).ok()?;
    let month = u8::try_from(index.rem_euclid(12) + 1).ok()?;
    Month::new(year, month)
}

/// The width of one day cell on a page of the default width.
fn cell_width(cx: &PageCx<'_>) -> f32 {
    (PAGE_SIZE.0 - 2.0 * cx.theme.spacing.lg) / 7.0
}

/// The Clock page.
#[derive(Clone, Copy, Debug)]
pub struct Clock;

impl PageView for Clock {
    fn title(&self) -> &str {
        copy::PAGE_CLOCK
    }

    fn view(&self, cx: &PageCx<'_>) -> View {
        let Some(month) = cx
            .today
            .and_then(|today| shown_month(today, cx.month_offset))
        else {
            return view::col(Key::named("clock")).child(note_list(
                cx.theme,
                Key::named("clock-note"),
                copy::CLOCK_NO_DATE,
            ));
        };
        let mut page = view::col(Key::named("clock"))
            .gap(cx.theme.spacing.sm)
            .child(nav(cx, &month))
            .child(weekday_header(cx))
            .child(grid(cx, &month, today_in(cx, &month)));
        if let Some(reading) = cx.weather {
            page = page.child(weather(cx, reading));
        }
        page
    }

    /// The default size, taller when the grid and the weather need it.
    fn wants(&self, cx: &PageCx<'_>) -> (f32, f32) {
        if cx.today.is_none() {
            return PAGE_SIZE;
        }
        let gap = cx.theme.spacing.sm;
        let mut content = BUTTON_ROW + HEADER_H + WEEKS as f32 * CELL_H + 2.0 * gap;
        if cx.weather.is_some() {
            content += gap + WEATHER_H;
        }
        (
            PAGE_SIZE.0,
            PAGE_SIZE.1.max(chrome_height(cx.theme) + content),
        )
    }
}

/// The navigation row's height: the buttons'.
const BUTTON_ROW: f32 = crate::island::view::BUTTON_H;

/// The day to highlight: today's, when the shown month is the current one.
fn today_in(cx: &PageCx<'_>, month: &Month) -> Option<u8> {
    cx.today
        .filter(|today| today.year == month.year && today.month == month.month)
        .map(|today| today.day)
}

/// The month title and the three buttons.
fn nav(cx: &PageCx<'_>, month: &Month) -> View {
    let theme = cx.theme;
    centred(view::row(Key::named("clock-nav")))
        .height(BUTTON_ROW)
        .gap(theme.spacing.sm)
        .child(
            view::text(
                Key::named("clock-month"),
                &copy::month_title(month.year, month.month),
                text_style(
                    theme.type_scale.row_title,
                    Weight::Semibold,
                    false,
                    Role::TextHi,
                ),
            )
            .grow(),
        )
        .child(button(
            theme,
            previous_key(),
            "clock-previous-label",
            copy::CALENDAR_PREVIOUS,
        ))
        .child(button(
            theme,
            today_key(),
            "clock-today-label",
            copy::CALENDAR_TODAY,
        ))
        .child(button(
            theme,
            next_key(),
            "clock-next-label",
            copy::CALENDAR_NEXT,
        ))
}

/// One cell of the grid: `content` centred in a day's box.
fn cell(cx: &PageCx<'_>, key: Key, height: f32) -> View {
    let mut cell = centred(view::row(key)).size(cell_width(cx), height);
    cell.style.justify_content = JustifyContent::CENTER;
    cell
}

fn weekday_header(cx: &PageCx<'_>) -> View {
    view::row(Key::named("clock-weekdays")).children((0..7u8).map(|weekday| {
        cell(
            cx,
            Key::indexed("clock-weekday", u64::from(weekday)),
            HEADER_H,
        )
        .child(view::text(
            Key::indexed("clock-weekday-label", u64::from(weekday)),
            copy::weekday_initial(weekday),
            text_style(
                cx.theme.type_scale.caption,
                Weight::Semibold,
                false,
                Role::TextLo,
            ),
        ))
    }))
}

/// The six weeks of `month`, `highlight` being the day to mark.
fn grid(cx: &PageCx<'_>, month: &Month, highlight: Option<u8>) -> View {
    let theme = cx.theme;
    view::col(Key::named("clock-grid")).children((0..WEEKS).map(|week| {
        view::row(Key::indexed("clock-week", week as u64)).children((0..7usize).map(move |col| {
            let slot = week * 7 + col;
            let day = slot
                .checked_sub(usize::from(month.leading_blanks))
                .map(|d| d + 1)
                .and_then(|d| u8::try_from(d).ok())
                .filter(|d| *d <= month.days);
            let Some(day) = day else {
                return cell(cx, Key::indexed("clock-blank", slot as u64), CELL_H);
            };
            let is_today = highlight == Some(day);
            let (role, weight) = if is_today {
                (Role::AccentInk, Weight::Semibold)
            } else {
                (Role::TextHi, Weight::Regular)
            };
            let mut day_cell = cell(cx, Key::indexed("clock-day", u64::from(day)), CELL_H);
            if is_today {
                day_cell = day_cell.fill(Role::Accent).radius(theme.radius_md);
            }
            day_cell.child(view::text(
                Key::indexed("clock-day-text", u64::from(day)),
                &day.to_string(),
                text_style(theme.type_scale.body, weight, true, role),
            ))
        }))
    }))
}

/// The weather reading: the temperature and the condition, then the place and
/// when it arrived.
fn weather(cx: &PageCx<'_>, reading: &WeatherReading) -> View {
    let theme = cx.theme;
    let updated =
        copy::weather_updated(&wall_clock::format_local(reading.taken_unix, cx.utc_offset));
    let place = if reading.label.is_empty() {
        updated
    } else {
        format!("{} · {updated}", reading.label)
    };
    view::col(Key::named("clock-weather"))
        .height(WEATHER_H)
        .child(view::text(
            Key::named("clock-weather-now"),
            &format!(
                "{} · {}",
                copy::celsius(reading.celsius),
                copy::weather_condition(reading.code)
            ),
            text_style(theme.type_scale.body, Weight::Semibold, false, Role::TextHi),
        ))
        .child(view::text(
            Key::named("clock-weather-place"),
            &place,
            text_style(
                theme.type_scale.caption,
                Weight::Regular,
                false,
                Role::TextLo,
            ),
        ))
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use celestina_paint::{TextStyle, Theme};
    use celestina_ui::{Tree, ViewKind};

    use super::*;
    use crate::island::pages::NameOwner;

    fn today(year: i32, month: u8, day: u8) -> Today {
        Today {
            year,
            month,
            day,
            weekday: celestina_shell::calendar::weekday(year, month, day),
        }
    }

    fn reading() -> WeatherReading {
        WeatherReading {
            celsius: 12,
            code: 3,
            daylight: true,
            label: "Madrid".to_owned(),
            // 2026-10-17 14:05 UTC.
            taken_unix: 1_792_245_900,
        }
    }

    fn with<R>(
        today: Option<Today>,
        weather: Option<&WeatherReading>,
        offset: i32,
        f: impl FnOnce(&PageCx<'_>) -> R,
    ) -> R {
        let theme = Theme::dark();
        let owners = NameOwner::default();
        f(&PageCx {
            theme: &theme,
            max_size: (760.0, 560.0),
            model: None,
            output: "DP-1",
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            sysmon: None,
            weather,
            today,
            month_offset: offset,
            utc_offset: 0,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
        })
    }

    fn find(v: &View, key: Key) -> Option<&View> {
        if v.key == key {
            return Some(v);
        }
        v.children.iter().find_map(|c| find(c, key))
    }

    fn text_of(v: &View, key: Key) -> Option<String> {
        match &find(v, key)?.kind {
            ViewKind::Text { text, .. } => Some(text.clone()),
            _ => None,
        }
    }

    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (6.0 * s.chars().count() as f32, 14.0)
    }

    fn laid_out(v: View) -> Tree {
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(v, Instant::now());
        tree.layout_measured((600.0, 600.0), fake_measure);
        tree
    }

    fn day_keys(v: &View) -> usize {
        (1..=31u64)
            .filter(|d| find(v, Key::indexed("clock-day", *d)).is_some())
            .count()
    }

    #[test]
    fn the_buttons_become_steps_and_nothing_else_does() {
        assert_eq!(month_step(previous_key()), Some(Step::Previous));
        assert_eq!(month_step(today_key()), Some(Step::Today));
        assert_eq!(month_step(next_key()), Some(Step::Next));
        for other in [Key::named("clock"), Key::named("close"), Key::STALE] {
            assert_eq!(month_step(other), None);
        }
    }

    #[test]
    fn steps_move_the_offset_and_stop_at_the_bound() {
        assert_eq!(Step::Next.apply(0), 1);
        assert_eq!(Step::Previous.apply(0), -1);
        assert_eq!(Step::Today.apply(-37), 0);
        assert_eq!(Step::Next.apply(MAX_OFFSET), MAX_OFFSET);
        assert_eq!(Step::Previous.apply(-MAX_OFFSET), -MAX_OFFSET);
        assert_eq!(Step::Next.apply(i32::MAX), MAX_OFFSET);
        assert_eq!(Step::Previous.apply(i32::MIN), -MAX_OFFSET);
    }

    #[test]
    fn the_shown_month_crosses_years_both_ways() {
        let t = today(2026, 1, 15);
        let at = |o| shown_month(t, o).map(|m| (m.year, m.month));
        assert_eq!(at(0), Some((2026, 1)));
        assert_eq!(at(-1), Some((2025, 12)));
        assert_eq!(at(11), Some((2026, 12)));
        assert_eq!(at(12), Some((2027, 1)));
        assert_eq!(at(-13), Some((2024, 12)));
        assert_eq!(at(MAX_OFFSET), Some((2126, 1)));
        assert_eq!(at(-MAX_OFFSET), Some((1926, 1)));
    }

    #[test]
    fn february_of_a_leap_year_has_twenty_nine_days_starting_on_a_thursday() {
        let t = today(2024, 2, 10);
        with(Some(t), None, 0, |cx| {
            let v = Clock.view(cx);
            assert_eq!(
                text_of(&v, Key::named("clock-month")),
                Some("Febrero de 2024".to_owned())
            );
            assert_eq!(day_keys(&v), 29);
            assert!(find(&v, Key::indexed("clock-day", 30)).is_none());
            let tree = laid_out(v);
            let cell = cell_width(cx);
            let rect = |d| {
                let id = tree
                    .node_by_key(Key::indexed("clock-day", d))
                    .expect("a day");
                tree.rect(id)
            };
            let first = rect(1);
            let last = rect(29);
            let grid = tree.node_by_key(Key::named("clock-grid")).expect("grid");
            let origin = tree.rect(grid);
            // 1 February 2024 was a Thursday: the fourth column, first week.
            assert!((first.x - origin.x - 3.0 * cell).abs() < 1.0, "{first:?}");
            assert!((first.y - origin.y).abs() < 0.01);
            // 29 February is a Thursday too, four weeks later.
            assert!((last.x - origin.x - 3.0 * cell).abs() < 1.0);
            assert!((last.y - origin.y - 4.0 * CELL_H).abs() < 1.0);
        });
    }

    #[test]
    fn a_month_that_needs_six_weeks_fits_the_grid() {
        // September 2024 starts on a Sunday and has 30 days: six rows.
        let t = today(2024, 9, 30);
        with(Some(t), None, 0, |cx| {
            let v = Clock.view(cx);
            assert_eq!(day_keys(&v), 30);
            let tree = laid_out(v);
            let grid = tree.rect(tree.node_by_key(Key::named("clock-grid")).expect("grid"));
            let first = tree.rect(tree.node_by_key(Key::indexed("clock-day", 1)).expect("1"));
            let last = tree.rect(tree.node_by_key(Key::indexed("clock-day", 30)).expect("30"));
            assert!(
                (first.x - grid.x - 6.0 * cell_width(cx)).abs() < 1.0,
                "Sunday"
            );
            assert!((last.y - grid.y - 5.0 * CELL_H).abs() < 1.0, "sixth week");
            assert!((last.x - grid.x).abs() < 1.0, "Monday");
            assert!((grid.h - 6.0 * CELL_H).abs() < 0.01);
        });
    }

    #[test]
    fn the_grid_keeps_its_height_whatever_the_month() {
        let heights: Vec<f32> = [(2026, 2), (2024, 9), (2026, 10)]
            .into_iter()
            .map(|(y, m)| {
                with(Some(today(y, m, 1)), None, 0, |cx| {
                    let tree = laid_out(Clock.view(cx));
                    tree.rect(tree.node_by_key(Key::named("clock-grid")).expect("grid"))
                        .h
                })
            })
            .collect();
        assert!(
            heights.iter().all(|h| (*h - heights[0]).abs() < 0.01),
            "{heights:?}"
        );
    }

    #[test]
    fn today_is_highlighted_only_in_the_current_month() {
        let t = today(2024, 2, 29);
        let accent = |v: &View, day| {
            matches!(
                find(v, Key::indexed("clock-day", day)).map(|c| &c.kind),
                Some(ViewKind::Box {
                    fill: Some(Role::Accent),
                    ..
                })
            )
        };
        with(Some(t), None, 0, |cx| {
            let v = Clock.view(cx);
            assert!(accent(&v, 29));
            assert!((1..29).all(|d| !accent(&v, d)));
        });
        for offset in [-1, 1, 12] {
            with(Some(t), None, offset, |cx| {
                let v = Clock.view(cx);
                assert!((1..=28).all(|d| !accent(&v, d)), "offset {offset}");
            });
        }
        // Going back to the current month restores it.
        with(Some(t), None, Step::Today.apply(5), |cx| {
            assert!(accent(&Clock.view(cx), 29));
        });
    }

    #[test]
    fn the_header_has_the_seven_initials_monday_first() {
        with(Some(today(2026, 10, 17)), None, 0, |cx| {
            let v = Clock.view(cx);
            let initials: Vec<String> = (0..7u64)
                .filter_map(|i| text_of(&v, Key::indexed("clock-weekday-label", i)))
                .collect();
            assert_eq!(initials, ["L", "M", "X", "J", "V", "S", "D"]);
        });
    }

    #[test]
    fn the_three_buttons_are_keyed_and_focusable_with_their_labels() {
        with(Some(today(2026, 10, 17)), None, 0, |cx| {
            let v = Clock.view(cx);
            for (key, label) in [
                (previous_key(), "Anterior"),
                (today_key(), "Hoy"),
                (next_key(), "Siguiente"),
            ] {
                let button = find(&v, key).expect("a button");
                assert!(button.interactive && button.focusable);
                let ViewKind::Text { text, .. } = &button.children[0].kind else {
                    panic!("a label");
                };
                assert_eq!(text, label);
            }
            assert_eq!(Clock.title(), "Reloj");
        });
    }

    #[test]
    fn the_weather_shows_below_the_grid_only_when_there_is_a_reading() {
        let t = today(2026, 10, 17);
        with(Some(t), None, 0, |cx| {
            let v = Clock.view(cx);
            assert!(find(&v, Key::named("clock-weather")).is_none());
        });
        let r = reading();
        with(Some(t), Some(&r), 0, |cx| {
            let v = Clock.view(cx);
            assert_eq!(
                text_of(&v, Key::named("clock-weather-now")),
                Some("12 °C · Nublado".to_owned())
            );
            assert_eq!(
                text_of(&v, Key::named("clock-weather-place")),
                Some("Madrid · actualizado 14:05".to_owned())
            );
            let tree = laid_out(v);
            let grid = tree.rect(tree.node_by_key(Key::named("clock-grid")).expect("grid"));
            let block = tree.rect(
                tree.node_by_key(Key::named("clock-weather"))
                    .expect("block"),
            );
            assert!(block.y >= grid.y + grid.h);
        });
        // The local offset moves the time; an empty label leaves only it.
        let unnamed = WeatherReading {
            label: String::new(),
            ..reading()
        };
        let theme = Theme::dark();
        let owners = NameOwner::default();
        let v = Clock.view(&PageCx {
            theme: &theme,
            max_size: (760.0, 560.0),
            model: None,
            output: "DP-1",
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            sysmon: None,
            weather: Some(&unnamed),
            today: Some(t),
            month_offset: 0,
            utc_offset: 7_200,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
        });
        assert_eq!(
            text_of(&v, Key::named("clock-weather-place")),
            Some("actualizado 16:05".to_owned())
        );
    }

    #[test]
    fn the_page_asks_for_room_for_the_grid_and_the_weather() {
        let t = today(2026, 10, 17);
        let r = reading();
        let bare = with(Some(t), None, 0, |cx| Clock.wants(cx));
        let full = with(Some(t), Some(&r), 0, |cx| Clock.wants(cx));
        assert_eq!(bare.0, PAGE_SIZE.0);
        assert!(bare.1 >= PAGE_SIZE.1);
        assert!(full.1 > bare.1);
        // The grid fits the room it asked for.
        with(Some(t), Some(&r), 0, |cx| {
            let tree = laid_out(Clock.view(cx));
            let root = tree.rect(tree.node_by_key(Key::named("clock")).expect("root"));
            assert!(
                root.h <= full.1 - chrome_height(cx.theme) + 0.5,
                "{root:?} {full:?}"
            );
        });
    }

    #[test]
    fn without_a_date_the_page_says_so_at_the_default_size() {
        with(None, None, 0, |cx| {
            let v = Clock.view(cx);
            assert_eq!(
                text_of(&v, Key::named("note-text")),
                Some(copy::CLOCK_NO_DATE.to_owned())
            );
            assert_eq!(Clock.wants(cx), PAGE_SIZE);
        });
        assert!(copy::CLOCK_NO_DATE.contains("barra"));
    }

    #[test]
    fn the_no_date_line_fits_one_row() {
        with(None, None, 0, |cx| {
            let ViewKind::Text {
                text,
                style,
                max_width,
            } = find(&Clock.view(cx), Key::named("note-text"))
                .expect("a note")
                .kind
                .clone()
            else {
                panic!("a text");
            };
            let (_, h) = crate::island::pages::measure_real(&text, &style, max_width);
            let (_, one) = crate::island::pages::measure_real("A", &style, max_width);
            assert!(h <= one + 0.5, "wraps to {h} px");
        });
    }
}
