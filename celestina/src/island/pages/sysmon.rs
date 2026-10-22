//! The System page (CEL-2-K): the processor and the memory as numbers and as
//! a sparkline of their last [`HISTORY_LEN`] samples, the hottest processor
//! temperature when a chip reports one, and a row that opens the session's
//! resource monitor. The page reads the monitor's newest reading, not the
//! bar's held-back one.

use celestina_paint::{Role, Weight};
use celestina_shell::glance::Load;
use celestina_ui::{view, Key, View};
use taffy::{AlignItems, JustifyContent};

use super::{note_list, PageCx, PageView};
use crate::{
    copy,
    island::{
        state::PAGE_SIZE,
        view::{centred, chrome_height, text_style},
    },
    providers::sysmon::HISTORY_LEN,
};

/// The desktop id of the resource monitor the row opens: the suite's own
/// (Hematita), which replaced the third-party one the session used to have.
pub const MONITOR_DESKTOP_ID: &str = "org.celestina.Hematita.desktop";

/// A sparkline's height, logical px.
const SPARK_H: f32 = 28.0;
/// The gap between two bars.
const BAR_GAP: f32 = 1.0;
/// A meter's label row.
const LABEL_H: f32 = 18.0;
/// The temperature row's height.
const TEMP_H: f32 = 24.0;

/// The row that opens the resource monitor.
#[must_use]
pub fn open_key() -> Key {
    Key::named("sysmon-open")
}

/// The System page.
#[derive(Clone, Copy, Debug)]
pub struct Sysmon;

impl PageView for Sysmon {
    fn title(&self) -> &str {
        copy::PAGE_SYSMON
    }

    fn view(&self, cx: &PageCx<'_>) -> View {
        let Some(reading) = cx.sysmon else {
            return view::col(Key::named("sysmon")).child(note_list(
                cx.theme,
                Key::named("sysmon-note"),
                copy::SYSMON_EMPTY,
            ));
        };
        let theme = cx.theme;
        view::col(Key::named("sysmon"))
            .gap(theme.spacing.md)
            .child(meter(
                cx,
                0,
                copy::SYSMON_CPU,
                &copy::percent(reading.cpu),
                &reading.cpu_history,
            ))
            .child(meter(
                cx,
                1,
                copy::SYSMON_RAM,
                &format!(
                    "{} · {}",
                    copy::memory_usage(reading.ram_used_kib, reading.ram_total_kib),
                    copy::percent(reading.ram)
                ),
                &reading.ram_history,
            ))
            .children(reading.temp_c.map(|degrees| temperature(cx, degrees)))
            .child(open_row(cx))
    }

    /// The default size, taller when the meters need it.
    fn wants(&self, cx: &PageCx<'_>) -> (f32, f32) {
        let Some(reading) = cx.sysmon else {
            return PAGE_SIZE;
        };
        let theme = cx.theme;
        let meter = LABEL_H + theme.spacing.xs + SPARK_H;
        let mut blocks = vec![meter, meter, theme.row_height];
        if reading.temp_c.is_some() {
            blocks.push(TEMP_H);
        }
        let gaps = (blocks.len() - 1) as f32 * theme.spacing.md;
        let content = blocks.iter().sum::<f32>() + gaps;
        (PAGE_SIZE.0, PAGE_SIZE.1.max(chrome_height(theme) + content))
    }
}

/// One meter: its label and value on a row, the sparkline under it.
fn meter(cx: &PageCx<'_>, slot: u64, label: &str, value: &str, history: &[u8]) -> View {
    let theme = cx.theme;
    view::col(Key::indexed("sysmon-meter", slot))
        .gap(theme.spacing.xs)
        .child(
            centred(view::row(Key::indexed("sysmon-head", slot)))
                .height(LABEL_H)
                .child(
                    view::text(
                        Key::indexed("sysmon-label", slot),
                        label,
                        text_style(theme.type_scale.body, Weight::Semibold, false, Role::TextHi),
                    )
                    .grow(),
                )
                .child(view::text(
                    Key::indexed("sysmon-value", slot),
                    value,
                    text_style(theme.type_scale.body, Weight::Regular, true, Role::TextLo),
                )),
        )
        .child(sparkline(cx, slot, history))
}

/// The newest [`HISTORY_LEN`] samples as bars, the newest at the right. A
/// sample at or above the critical level is drawn in the danger colour.
fn sparkline(cx: &PageCx<'_>, slot: u64, history: &[u8]) -> View {
    let theme = cx.theme;
    let width = PAGE_SIZE.0 - 2.0 * theme.spacing.lg;
    let bar_w = ((width - BAR_GAP * (HISTORY_LEN - 1) as f32) / HISTORY_LEN as f32).max(1.0);
    let shown = &history[history.len().saturating_sub(HISTORY_LEN)..];
    let mut track = view::row(Key::indexed("sysmon-spark", slot))
        .size(width, SPARK_H)
        .gap(BAR_GAP)
        .fill(Role::ControlFill)
        .radius(theme.radius_sm);
    track.style.align_items = AlignItems::FLEX_END;
    track.style.justify_content = JustifyContent::FLEX_END;
    track.children(shown.iter().enumerate().map(|(i, &percent)| {
        let role = if Load::of(percent) == Load::Critical {
            Role::Danger
        } else {
            Role::Accent
        };
        let height = (f32::from(percent.min(100)) / 100.0 * SPARK_H).max(1.0);
        view::row(Key::indexed(
            "sysmon-bar",
            slot * HISTORY_LEN as u64 + i as u64,
        ))
        .size(bar_w, height)
        .fill(role)
    }))
}

fn temperature(cx: &PageCx<'_>, degrees: i16) -> View {
    let theme = cx.theme;
    centred(view::row(Key::named("sysmon-temperature")))
        .height(TEMP_H)
        .child(
            view::text(
                Key::named("sysmon-temperature-label"),
                copy::SYSMON_TEMPERATURE,
                text_style(theme.type_scale.body, Weight::Semibold, false, Role::TextHi),
            )
            .grow(),
        )
        .child(view::text(
            Key::named("sysmon-temperature-value"),
            &copy::celsius(degrees),
            text_style(theme.type_scale.body, Weight::Regular, true, Role::TextLo),
        ))
}

/// The row that opens the resource monitor.
fn open_row(cx: &PageCx<'_>) -> View {
    let theme = cx.theme;
    centred(view::row(open_key()))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_md)
        .interactive()
        .focusable()
        .child(view::icon(Key::named("sysmon-open-icon"), "gauge", 20.0).tint(Role::TextHi))
        .child(
            view::text(
                Key::named("sysmon-open-label"),
                copy::SYSMON_OPEN_MONITOR,
                text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi),
            )
            .grow(),
        )
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use celestina_paint::{TextStyle, Theme};
    use celestina_ui::{Tree, ViewKind};

    use super::*;
    use crate::{island::pages::NameOwner, providers::sysmon::SysReading};

    fn reading(temp_c: Option<i16>) -> SysReading {
        SysReading {
            cpu: 23,
            ram: 41,
            // 6.2 of 15.6 GiB.
            ram_used_kib: 6_500_000,
            ram_total_kib: 16_360_000,
            temp_c,
            cpu_history: (0..60).map(|i| i as u8).collect(),
            ram_history: vec![40, 95, 41],
        }
    }

    fn with<R>(sysmon: Option<&SysReading>, f: impl FnOnce(&PageCx<'_>) -> R) -> R {
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
            sysmon,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
            media: &crate::island::pages::media::NO_MEDIA,
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

    fn bars(v: &View, slot: u64) -> usize {
        find(v, Key::indexed("sysmon-spark", slot)).map_or(0, |s| s.children.len())
    }

    #[test]
    fn the_page_shows_cpu_and_memory_with_their_values() {
        let r = reading(None);
        with(Some(&r), |cx| {
            let v = Sysmon.view(cx);
            assert_eq!(
                text_of(&v, Key::indexed("sysmon-label", 0)),
                Some("CPU".to_owned())
            );
            assert_eq!(
                text_of(&v, Key::indexed("sysmon-value", 0)),
                Some("23%".to_owned())
            );
            assert_eq!(
                text_of(&v, Key::indexed("sysmon-label", 1)),
                Some("RAM".to_owned())
            );
            assert_eq!(
                text_of(&v, Key::indexed("sysmon-value", 1)),
                Some("6,2 de 15,6 GiB · 41%".to_owned())
            );
            assert_eq!(Sysmon.title(), "Sistema");
        });
    }

    #[test]
    fn the_temperature_row_exists_only_when_a_chip_reports_one() {
        let without = reading(None);
        with(Some(&without), |cx| {
            assert!(find(&Sysmon.view(cx), Key::named("sysmon-temperature")).is_none());
        });
        let with_temp = reading(Some(62));
        with(Some(&with_temp), |cx| {
            let v = Sysmon.view(cx);
            assert_eq!(
                text_of(&v, Key::named("sysmon-temperature-value")),
                Some("62 °C".to_owned())
            );
        });
    }

    #[test]
    fn a_sparkline_has_a_bar_per_sample_newest_last_and_marks_critical_ones() {
        let r = reading(None);
        with(Some(&r), |cx| {
            let v = Sysmon.view(cx);
            assert_eq!(bars(&v, 0), 60);
            assert_eq!(bars(&v, 1), 3);
            let spark = find(&v, Key::indexed("sysmon-spark", 1)).expect("sparkline");
            let fills: Vec<Option<Role>> = spark
                .children
                .iter()
                .map(|b| match &b.kind {
                    ViewKind::Box { fill, .. } => *fill,
                    _ => None,
                })
                .collect();
            assert_eq!(
                fills,
                [Some(Role::Accent), Some(Role::Danger), Some(Role::Accent)]
            );
        });
    }

    #[test]
    fn a_longer_history_shows_only_the_newest_sixty() {
        let mut r = reading(None);
        r.cpu_history = vec![10; 100];
        with(Some(&r), |cx| assert_eq!(bars(&Sysmon.view(cx), 0), 60));
        r.cpu_history = Vec::new();
        with(Some(&r), |cx| assert_eq!(bars(&Sysmon.view(cx), 0), 0));
    }

    #[test]
    fn bar_heights_follow_the_percentage_inside_the_track() {
        let r = reading(None);
        with(Some(&r), |cx| {
            let mut tree = Tree::new(false, &Theme::dark());
            tree.apply(Sysmon.view(cx), Instant::now());
            tree.layout_measured((600.0, 600.0), fake_measure);
            let rect = |slot: u64, i: u64| {
                tree.rect(
                    tree.node_by_key(Key::indexed("sysmon-bar", slot * 60 + i))
                        .expect("a bar"),
                )
            };
            let track = tree.rect(
                tree.node_by_key(Key::indexed("sysmon-spark", 1))
                    .expect("track"),
            );
            // 95 % of the track, bottom aligned; the newest at the right edge.
            let high = rect(1, 1);
            assert!((high.h - 0.95 * SPARK_H).abs() < 0.6, "{high:?}");
            assert!((high.y + high.h - (track.y + track.h)).abs() < 0.01);
            let newest = rect(1, 2);
            assert!((newest.x + newest.w - (track.x + track.w)).abs() < 0.6);
            assert!(newest.x > high.x);
            // Zero still draws a sliver.
            let zero = rect(0, 0);
            assert!(zero.h >= 1.0 - 0.01);
        });
    }

    #[test]
    fn the_open_row_is_a_focusable_button() {
        let r = reading(None);
        with(Some(&r), |cx| {
            let v = Sysmon.view(cx);
            let row = find(&v, open_key()).expect("the row");
            assert!(row.interactive && row.focusable);
            assert_eq!(
                text_of(&v, Key::named("sysmon-open-label")),
                Some("Abrir el monitor del sistema".to_owned())
            );
        });
        assert_eq!(MONITOR_DESKTOP_ID, "org.celestina.Hematita.desktop");
    }

    #[test]
    fn without_a_reading_the_page_says_so_at_the_default_size() {
        with(None, |cx| {
            let v = Sysmon.view(cx);
            assert_eq!(
                text_of(&v, Key::named("note-text")),
                Some(copy::SYSMON_EMPTY.to_owned())
            );
            assert!(find(&v, open_key()).is_none());
            assert_eq!(Sysmon.wants(cx), PAGE_SIZE);
            // It says what to do, and the line fits its row.
            assert!(copy::SYSMON_EMPTY.contains("«sysmon»"));
            let ViewKind::Text {
                text,
                style,
                max_width,
            } = find(&v, Key::named("note-text"))
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

    #[test]
    fn the_page_asks_for_the_room_its_rows_need() {
        let plain = reading(None);
        let hot = reading(Some(70));
        let (a, b) = (
            with(Some(&plain), |cx| Sysmon.wants(cx)),
            with(Some(&hot), |cx| Sysmon.wants(cx)),
        );
        assert_eq!(a.0, PAGE_SIZE.0);
        assert!(b.1 > a.1 || a.1 > PAGE_SIZE.1);
        with(Some(&hot), |cx| {
            let mut tree = Tree::new(false, &Theme::dark());
            tree.apply(Sysmon.view(cx), Instant::now());
            tree.layout_measured((600.0, 600.0), fake_measure);
            let root = tree.rect(tree.node_by_key(Key::named("sysmon")).expect("root"));
            assert!(
                root.h <= b.1 - chrome_height(cx.theme) + 0.5,
                "{root:?} {b:?}"
            );
        });
    }
}
