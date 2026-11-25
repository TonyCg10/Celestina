//! The monitor list of the brightness popup (CEL-I-003): with two monitors or
//! more, an "All" heading with the mean level and a slider that sets every
//! monitor with a reading, a divider, and one compact row per monitor (its
//! name, its slider, its percent). With one monitor the popup is the single
//! slider of old and none of this is built.
//!
//! The rows, and with them the popup's size, are fixed when the popup opens:
//! a monitor that appears later is listed the next time. A row updates in
//! place, as the readings arrive ([`MonitorMirror::adopt`]), and a monitor
//! with no reading is greyed out and takes no input.

use std::{cell::RefCell, rc::Rc};

use celestina_paint::{Role, Theme, Weight};
use celestina_ui::{view, Key, View};

use super::audio_rows::{compact_h, fixed};
use super::popup::AUDIO_W;
use super::popup::{card_key, heading, heading_h, level_slider, slider_h, slider_key, Control};
use crate::{
    copy,
    island::view::{centred, text_style},
};

/// The width of a monitor's name column, logical px.
pub const MONITOR_NAME_W: f32 = 96.0;
/// The width of a monitor's percent column, logical px.
pub const MONITOR_PERCENT_W: f32 = 38.0;
/// The divider's height, logical px.
pub const DIVIDER_H: f32 = 1.0;

/// A monitor's row as the popup lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorRow {
    pub name: String,
    /// The monitor of the bar the popup hangs from: first, in bold.
    pub primary: bool,
}

/// What the app writes and the popup reads: each listed monitor's level, in
/// the order of the rows; `None` is unknown.
pub type SharedMonitors = Rc<RefCell<Vec<Option<u8>>>>;

/// What the popup opens over: the rows and the shared levels.
#[derive(Clone, Debug, Default)]
pub struct MonitorFeed {
    pub rows: Vec<MonitorRow>,
    pub levels: SharedMonitors,
}

/// The mean of the known levels, rounded half up; `None` when none is known.
#[must_use]
pub fn mean_level(levels: &[Option<u8>]) -> Option<u8> {
    let known: Vec<u32> = levels.iter().flatten().map(|l| u32::from(*l)).collect();
    let n = u32::try_from(known.len()).ok().filter(|n| *n > 0)?;
    u8::try_from((known.iter().sum::<u32>() + n / 2) / n).ok()
}

/// The popup's size when it lists `rows` monitors: the audio width, the card's
/// padding all round, and the heading, the slider, the divider and the rows
/// (which touch each other), a small gap between those four.
#[must_use]
pub fn monitors_size(theme: &Theme, rows: usize) -> (f32, f32) {
    (
        AUDIO_W,
        2.0 * theme.spacing.md
            + heading_h(theme)
            + slider_h(theme)
            + DIVIDER_H
            + rows as f32 * compact_h(theme)
            + 3.0 * theme.spacing.sm,
    )
}

/// The popup's copy of the monitor list: the rows it listed when it opened,
/// the names cut to their column, what it shows now (the app's reading with
/// the popup's own changes on top) and the last reading it took in. The same
/// rule as the single level, for each row on its own: a reading is adopted
/// when it differs from the last one taken in for that row, never merely
/// because it differs from what is shown.
#[derive(Debug, Default)]
pub struct MonitorMirror {
    rows: Vec<MonitorRow>,
    /// The name each row shows, cut to its column.
    labels: Vec<String>,
    shown: Vec<Option<u8>>,
    seen: Vec<Option<u8>>,
}

impl MonitorMirror {
    /// A mirror opened over `feed`; names stand whole until [`cut`](Self::cut).
    #[must_use]
    pub fn open(feed: &MonitorFeed) -> MonitorMirror {
        let mut mirror = MonitorMirror {
            labels: feed.rows.iter().map(|row| row.name.clone()).collect(),
            rows: feed.rows.clone(),
            shown: Vec::new(),
            seen: Vec::new(),
        };
        mirror.seen = mirror.read(&feed.levels);
        mirror.shown = mirror.seen.clone();
        mirror
    }

    fn read(&self, cell: &SharedMonitors) -> Vec<Option<u8>> {
        let mut levels = cell.borrow().clone();
        levels.resize(self.rows.len(), None);
        levels
    }

    /// Whether the card lists the monitors (two or more) rather than the
    /// single slider.
    #[must_use]
    pub fn is_multi(&self) -> bool {
        self.rows.len() >= 2
    }

    /// How many rows the card lists.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Cuts every name to its column, at the weight its row draws it in;
    /// `fit` is the text system's measure, in the caller's hands.
    pub fn cut(&mut self, mut fit: impl FnMut(&str, f32, Weight) -> String) {
        self.labels = self
            .rows
            .iter()
            .map(|row| fit(&row.name, MONITOR_NAME_W, name_weight(row)))
            .collect();
    }

    /// The slider control of `key`, if it is one of the list's.
    #[must_use]
    pub fn control_of(&self, key: Key) -> Option<Control> {
        if !self.is_multi() {
            return None;
        }
        if slider_key(Control::AllMonitors) == key {
            return Some(Control::AllMonitors);
        }
        (0..self.rows.len())
            .filter_map(|at| u8::try_from(at).ok())
            .map(Control::Monitor)
            .find(|control| slider_key(*control) == key)
    }

    /// Takes in the app's newest reading, row by row: a row is adopted only
    /// when its own reading differs from the last one taken in for it, so a
    /// late reading of one monitor never moves another that still shows what
    /// the popup set. Whether anything changed.
    pub fn adopt(&mut self, cell: &SharedMonitors) -> bool {
        let external = self.read(cell);
        let mut changed = false;
        for (at, level) in external.into_iter().enumerate() {
            if self.seen[at] != level {
                self.seen[at] = level;
                self.shown[at] = level;
                changed = true;
            }
        }
        changed
    }

    /// The popup's own change of row `at`, shown until the reading follows.
    /// A row with no reading takes none.
    pub fn set_level(&mut self, at: usize, percent: u8) {
        if let Some(slot) = self.shown.get_mut(at).filter(|slot| slot.is_some()) {
            *slot = Some(percent);
        }
    }

    /// The rows that have a reading, in order.
    #[must_use]
    pub fn known(&self) -> Vec<usize> {
        (0..self.shown.len())
            .filter(|at| self.shown[*at].is_some())
            .collect()
    }

    /// The mean shown, for the All heading.
    #[must_use]
    pub fn mean(&self) -> Option<u8> {
        mean_level(&self.shown)
    }
}

fn name_weight(row: &MonitorRow) -> Weight {
    if row.primary {
        Weight::Semibold
    } else {
        Weight::Regular
    }
}

/// One monitor's row: its name, its slider, which grows, and its percent.
fn monitor_row(theme: &Theme, at: usize, row: &MonitorRow, label: &str, level: Option<u8>) -> View {
    let index = at as u64;
    let role = if level.is_some() {
        Role::TextHi
    } else {
        Role::TextFaint
    };
    let side = compact_h(theme);
    let mut percent = centred(view::row(Key::indexed("level-monitor-percent-slot", index)))
        .size(MONITOR_PERCENT_W, side);
    percent.style.justify_content = taffy::JustifyContent::FLEX_END;
    if let Some(level) = level {
        percent = percent.child(view::text(
            Key::indexed("level-monitor-percent", index),
            &copy::osd_percent(level.min(100)),
            text_style(theme.type_scale.body, Weight::Regular, true, Role::TextLo),
        ));
    }
    let control = Control::Monitor(u8::try_from(at).unwrap_or(u8::MAX));
    centred(view::row(Key::indexed("level-monitor-row", index)))
        .height(side)
        .padding_xy(theme.spacing.sm, 0.0)
        .gap(theme.spacing.sm)
        .child(fixed(
            view::text(
                Key::indexed("level-monitor-name", index),
                label,
                text_style(theme.type_scale.body, name_weight(row), false, role),
            )
            .width(MONITOR_NAME_W),
        ))
        .child(level_slider(control, level, false).grow())
        .child(fixed(percent))
}

/// The card of a popup that lists several monitors.
#[must_use]
pub fn monitors_card(theme: &Theme, mirror: &MonitorMirror) -> View {
    let (w, h) = monitors_size(theme, mirror.rows.len());
    let mean = mirror.mean();
    let rows = mirror
        .rows
        .iter()
        .enumerate()
        .map(|(at, row)| monitor_row(theme, at, row, &mirror.labels[at], mirror.shown[at]));
    view::col(card_key())
        .haze(theme.radius_md)
        .size(w, h)
        .padding(theme.spacing.md)
        .gap(theme.spacing.sm)
        .child(heading(
            theme,
            "level-heading-all",
            "sun",
            copy::BRIGHTNESS_ALL,
            mean.unwrap_or(0),
        ))
        .child(level_slider(Control::AllMonitors, mean, false))
        .child(
            view::row(Key::named("level-monitors-divider"))
                .height(DIVIDER_H)
                .fill(Role::Divider),
        )
        .child(view::col(Key::named("level-monitors")).children(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_level_rounds_half_up_and_skips_unknown() {
        assert_eq!(mean_level(&[Some(30), Some(41), None]), Some(36));
        assert_eq!(mean_level(&[Some(100), Some(99)]), Some(100));
        assert_eq!(mean_level(&[Some(0), Some(1)]), Some(1));
        assert_eq!(mean_level(&[Some(7)]), Some(7));
        assert_eq!(mean_level(&[]), None);
        assert_eq!(mean_level(&[None, None]), None);
    }
}
