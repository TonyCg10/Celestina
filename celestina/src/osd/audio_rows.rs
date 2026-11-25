//! The audio lists of the volume popup (CEL-I-004): the devices under the
//! output and under the microphone and the applications that play sound, as
//! rows, and [`GraphMirror`], the popup's copy of the audio worker's graph.
//!
//! Each list sits under a header that folds it (output, input, applications),
//! and all three start folded every time the popup opens. A
//! folded list shows the default device, or the applications that play now;
//! an unfolded one shows every row. The rows, and with them the popup's size,
//! change only when a header folds or unfolds ([`Shape`]): a row updates in
//! place (the default mark, a level), a stream that ends greys out and takes
//! no input, and one that starts waits for the next fold or unfold.

use std::time::{Duration, Instant};

use celestina_paint::{Role, Theme, Weight};
use celestina_shell::audio::{
    AudioGraph, AudioLevel, GraphDevice, GraphStream, MAX_GRAPH_DEVICES, MAX_GRAPH_STREAMS,
};
use celestina_ui::{view, Key, View};

use super::popup::{level_slider, mute_key, slider_h, slider_key, Control, Kind, AUDIO_W, ICON};
use crate::{
    copy,
    island::view::{centred, text_style, SWITCH_TRACK},
};

/// The width of an application's slider, logical px.
pub(super) const STREAM_SLIDER_W: f32 = 100.0;

/// How long a default mark the popup set itself stands before it gives way to
/// the worker's graph, if the worker's graph has not caught up by then (the
/// worker refused the change, or it never came): longer than the graph's
/// settle, short enough that a wrong mark does not stand.
pub const MARK_HOLD: Duration = Duration::from_secs(1);

/// The row of the device `id`.
#[must_use]
pub fn device_key(id: u32) -> Key {
    Key::indexed("level-device", u64::from(id))
}

/// The name text of the device `id`'s row.
#[must_use]
pub fn device_name_key(id: u32) -> Key {
    Key::indexed("level-device-name", u64::from(id))
}

/// The check that marks the default device `id`; only that row has one.
#[must_use]
pub fn device_mark_key(id: u32) -> Key {
    Key::indexed("level-device-mark", u64::from(id))
}

/// A foldable section of the volume popup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    /// The output devices.
    Output,
    /// The input devices.
    Input,
    /// The applications.
    Apps,
}

impl Section {
    /// The section's number in its header's key.
    fn index(self) -> u64 {
        match self {
            Section::Output => 0,
            Section::Input => 1,
            Section::Apps => 2,
        }
    }

    const ALL: [Section; 3] = [Section::Output, Section::Input, Section::Apps];
}

/// The header of `section`: a row that folds and unfolds it.
#[must_use]
pub fn section_key(section: Section) -> Key {
    Key::indexed("level-section", section.index())
}

/// The section whose header is `key`.
#[must_use]
pub fn section_of(key: Key) -> Option<Section> {
    Section::ALL.into_iter().find(|s| section_key(*s) == key)
}

/// Which sections are unfolded. Every popup starts with none, and nothing
/// saves it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Folds {
    open: [bool; 3],
}

impl Folds {
    /// Whether `section` is unfolded.
    #[must_use]
    pub fn is_open(self, section: Section) -> bool {
        self.open[section.index() as usize]
    }

    fn flip(&mut self, section: Section) {
        let open = &mut self.open[section.index() as usize];
        *open = !*open;
    }
}

/// How many rows of each list a volume popup shows: what its sections list
/// and which of them are unfolded, decided when it opens and at each fold or
/// unfold; a row is one device or one application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shape {
    pub outputs: usize,
    pub inputs: usize,
    pub streams: usize,
    /// Whether the graph lists anything at all, so that the popup has its
    /// device and application sections (and the wider card).
    pub listed: bool,
}

impl Shape {
    /// The rows `graph` shows under `folds`, at most the bounds of its worker
    /// each: a folded list shows the default device, or the applications that
    /// play; an unfolded one shows every row.
    #[must_use]
    pub fn of(graph: &AudioGraph, folds: Folds) -> Shape {
        let devices = |list: &[GraphDevice], section| {
            if folds.is_open(section) {
                list.len().min(MAX_GRAPH_DEVICES)
            } else {
                list.len().min(1)
            }
        };
        let streams = if folds.is_open(Section::Apps) {
            graph.streams.len()
        } else {
            graph.streams.iter().filter(|s| s.playing).count()
        };
        Shape {
            outputs: devices(&graph.outputs, Section::Output),
            inputs: devices(&graph.inputs, Section::Input),
            streams: streams.min(MAX_GRAPH_STREAMS),
            listed: !(graph.outputs.is_empty()
                && graph.inputs.is_empty()
                && graph.streams.is_empty()),
        }
    }

    /// A graph with nothing in it (no worker reading yet, or an unreadable
    /// `wpctl status`): the popup then has no device or application section.
    pub(super) fn is_empty(self) -> bool {
        !self.listed
    }
}

/// What the popup lists, from its kind, the microphone's presence and `shape`:
/// the one place the size and the card agree on. The device rows of the
/// microphone's section exist only with the microphone; the applications'
/// section always exists in a popup that lists anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Lists {
    pub outputs: usize,
    pub inputs: usize,
    pub streams: usize,
    pub apps: bool,
}

impl Lists {
    pub(super) fn of(kind: Kind, microphone: bool, shape: Shape) -> Lists {
        if kind != Kind::Volume || shape.is_empty() {
            return Lists {
                outputs: 0,
                inputs: 0,
                streams: 0,
                apps: false,
            };
        }
        Lists {
            outputs: shape.outputs,
            inputs: if microphone { shape.inputs } else { 0 },
            streams: shape.streams,
            apps: true,
        }
    }
}

/// A device's or an application's row height, and a header's: compact, so
/// that eight of each, unfolded, with their three headers still leave the
/// popup on a 1080 px output.
pub(super) fn compact_h(theme: &Theme) -> f32 {
    (theme.row_height * 0.65).max(slider_h(theme))
}

/// A section header's height: a compact row, so that it is as easy to hit as
/// the rows below it.
pub(super) fn header_h(theme: &Theme) -> f32 {
    compact_h(theme)
}

/// A list's height: its header and `rows` compact rows, or one when it is
/// empty and says so.
pub(super) fn list_h(theme: &Theme, rows: usize) -> f32 {
    header_h(theme) + rows.max(1) as f32 * compact_h(theme)
}

/// The graph a card draws: `opened` is what the popup listed when it opened,
/// or when its section last folded or unfolded (its rows, in order, with
/// their names, never changed in between) and `live` what the worker holds
/// now. A row whose id `live` no longer holds is greyed out.
#[derive(Clone, Copy, Debug)]
pub struct Audio<'a> {
    pub opened: &'a AudioGraph,
    pub live: &'a AudioGraph,
    pub folds: Folds,
    /// The name each listed row shows, by node id: cut to its column when
    /// the popup opened ([`GraphMirror::open`]).
    pub labels: &'a [(u32, String)],
}

impl Audio<'_> {
    /// The name the row of `id` shows; the listed `name` when none was cut.
    fn label<'n>(&'n self, id: u32, name: &'n str) -> &'n str {
        self.labels
            .iter()
            .find(|(of, _)| *of == id)
            .map_or(name, |(_, label)| label.as_str())
    }
}

/// The width of a device row's name: the card's inside, less the row's
/// padding, the check's column and the gap after it. Every row has the same,
/// so every name starts at the same x.
pub(super) fn device_name_w(theme: &Theme) -> f32 {
    AUDIO_W - 2.0 * theme.spacing.md - 3.0 * theme.spacing.sm - ICON
}

/// The width of an application's name column: what is left of the row beside
/// the slider and the mute button's column, with a gap between each.
pub(super) fn stream_name_w(theme: &Theme) -> f32 {
    AUDIO_W - 2.0 * theme.spacing.md - 4.0 * theme.spacing.sm - STREAM_SLIDER_W - SWITCH_TRACK.0
}

/// The id and name of each device of `list`.
fn named(list: &[GraphDevice]) -> Vec<(u32, String)> {
    list.iter().map(|d| (d.id, d.name.clone())).collect()
}

/// A node that keeps its width whatever its row holds.
fn fixed(mut view: View) -> View {
    view.style.flex_shrink = 0.0;
    view
}

/// A section's header: a row, as the device rows are, with a chevron that
/// points down once the section is unfolded and its label. A click, or Enter
/// or Space on the focused row, folds or unfolds it. The toolkit has no
/// accessible-state hook yet, so the chevron is what says which it is.
fn section_header(theme: &Theme, section: Section, label: &str, open: bool) -> View {
    let at = section.index();
    centred(view::row(section_key(section)))
        .height(header_h(theme))
        .padding_xy(theme.spacing.sm, 0.0)
        .gap(theme.spacing.sm)
        .radius(theme.radius_sm)
        .child(fixed(
            view::icon(
                Key::indexed("level-section-chevron", at),
                if open {
                    "chevron-down"
                } else {
                    "chevron-right"
                },
                ICON,
            )
            .tint(Role::TextLo),
        ))
        .child(view::text(
            Key::indexed("level-section-label", at),
            label,
            text_style(
                theme.type_scale.caption,
                Weight::Regular,
                false,
                Role::TextLo,
            ),
        ))
        .interactive()
        .focusable()
}

/// One device row: the check (only on the default), the name, and a click
/// that makes it the default. A device the worker no longer lists is greyed
/// out and takes no input.
fn device_row(theme: &Theme, name: &str, id: u32, live: Option<&GraphDevice>) -> View {
    let present = live.is_some();
    let default = live.is_some_and(|d| d.default);
    let role = if present {
        Role::TextHi
    } else {
        Role::TextFaint
    };
    // The check's column is on every row, so the names line up.
    let mark = if default {
        view::icon(device_mark_key(id), "check", ICON).tint(Role::Accent)
    } else {
        view::row(Key::indexed("level-device-gap", u64::from(id))).size(ICON, ICON)
    };
    let mut row = centred(view::row(device_key(id)))
        .height(compact_h(theme))
        .padding_xy(theme.spacing.sm, 0.0)
        .gap(theme.spacing.sm)
        .radius(theme.radius_sm)
        .clip()
        .child(fixed(mark))
        .child(fixed(
            view::text(
                device_name_key(id),
                name,
                text_style(theme.type_scale.body, Weight::Regular, false, role),
            )
            .width(device_name_w(theme)),
        ));
    if default {
        row = row.fill(Role::ControlFill);
    }
    if present {
        row.interactive().focusable()
    } else {
        row
    }
}

/// The devices a list shows: unfolded, the first `rows` of those `opened`
/// listed; folded, the default the worker holds now (listed or not), else the
/// one listed, else the first listed, so that a folded list is always one row
/// when it lists any, as [`Shape::of`] counts.
fn shown_devices<'a>(
    open: bool,
    rows: usize,
    opened: &'a [GraphDevice],
    live: &'a [GraphDevice],
) -> Vec<&'a GraphDevice> {
    if open {
        return opened.iter().take(rows).collect();
    }
    if opened.is_empty() {
        return Vec::new();
    }
    live.iter()
        .find(|d| d.default)
        .or_else(|| opened.iter().find(|d| d.default))
        .or_else(|| opened.first())
        .into_iter()
        .take(rows)
        .collect()
}

/// A list of devices of `section` under its header: the rows `opened`
/// listed that its fold shows, each looked up in `live` and named as `audio`
/// labels it.
pub(super) fn device_list(
    theme: &Theme,
    audio: Audio<'_>,
    section: Section,
    key: &'static str,
    label: &str,
    lists: (&[GraphDevice], &[GraphDevice]),
    rows: usize,
) -> View {
    let (opened, live) = lists;
    let open = audio.folds.is_open(section);
    view::col(Key::named(key))
        .child(section_header(theme, section, label, open))
        .children(
            shown_devices(open, rows, opened, live)
                .into_iter()
                .map(|listed| {
                    device_row(
                        theme,
                        audio.label(listed.id, &listed.name),
                        listed.id,
                        live.iter().find(|d| d.id == listed.id),
                    )
                }),
        )
}

/// One application's row in fixed columns, the same on every row: its name,
/// its slider and, at the right edge where the popup's other mute switches
/// are, its mute button: a speaker, struck through while muted, with the name
/// dimmed. An application the worker no longer lists, or has no level for, is
/// greyed out and takes no input.
fn stream_row(theme: &Theme, name: &str, id: u32, live: Option<&GraphStream>) -> View {
    let level: Option<AudioLevel> = live.and_then(|s| s.level);
    let enabled = level.is_some();
    let muted = level.is_some_and(|l| l.muted);
    let role = if enabled && !muted {
        Role::TextHi
    } else {
        Role::TextFaint
    };
    let control = Control::Stream(id);
    let side = compact_h(theme);
    let (fill, ink, glyph) = if muted {
        (Role::ControlPressed, Role::TextHi, "media-volume-muted")
    } else {
        (Role::ControlFill, Role::TextLo, "media-volume")
    };
    let button = centred(view::row(mute_key(control)))
        .size(side, side)
        .radius(side / 2.0)
        .fill(fill)
        .child(
            view::icon(
                Key::named("level-stream-mute-glyph"),
                glyph,
                ICON.min(side - 2.0 * theme.spacing.xs),
            )
            .tint(if enabled { ink } else { Role::TextFaint }),
        );
    let mut button = if enabled {
        button.interactive().focusable()
    } else {
        button
    };
    button.style.justify_content = taffy::JustifyContent::CENTER;
    // The button sits at the right edge of the column the popup's other mute
    // switches use, so every row's control lines up.
    let mut mute = view::row(Key::indexed("level-stream-mute-slot", u64::from(id)))
        .size(SWITCH_TRACK.0, side)
        .child(button);
    mute.style.justify_content = taffy::JustifyContent::FLEX_END;
    centred(view::row(Key::indexed("level-stream-row", u64::from(id))))
        .height(compact_h(theme))
        .padding_xy(theme.spacing.sm, 0.0)
        .gap(theme.spacing.sm)
        .child(fixed(
            view::text(
                Key::named("level-stream-name"),
                name,
                text_style(theme.type_scale.body, Weight::Regular, false, role),
            )
            .width(stream_name_w(theme)),
        ))
        .child(fixed(
            level_slider(
                control,
                level.map(|l| l.percent),
                level.is_some_and(|l| l.muted),
            )
            .width(STREAM_SLIDER_W),
        ))
        .child(fixed(mute))
}

/// The applications' list under its header: a row for each stream its fold
/// shows (folded, those that played when the list was last read; unfolded,
/// every one listed), or the one quiet line when there are none.
pub(super) fn apps_list(theme: &Theme, audio: Audio<'_>, rows: usize) -> View {
    let open = audio.folds.is_open(Section::Apps);
    let list = view::col(Key::named("level-apps")).child(section_header(
        theme,
        Section::Apps,
        copy::AUDIO_APPS,
        open,
    ));
    let shown: Vec<&GraphStream> = audio
        .opened
        .streams
        .iter()
        .filter(|s| open || s.playing)
        .take(rows)
        .collect();
    if shown.is_empty() {
        return list.child(
            centred(view::row(Key::named("level-apps-none")))
                .height(compact_h(theme))
                .padding_xy(theme.spacing.sm, 0.0)
                .child(view::text(
                    Key::named("level-apps-none-text"),
                    copy::AUDIO_NO_APPS,
                    text_style(
                        theme.type_scale.row_secondary,
                        Weight::Regular,
                        false,
                        Role::TextLo,
                    ),
                )),
        );
    }
    list.children(shown.into_iter().map(|listed| {
        stream_row(
            theme,
            audio.label(listed.id, &listed.name),
            listed.id,
            audio.live.streams.iter().find(|s| s.id == listed.id),
        )
    }))
}

/// The popup's copy of the audio graph: the rows it listed when it opened, or
/// when their section last folded or unfolded (their order and names never
/// change in between), which sections are unfolded, what it shows now (the
/// worker's graph with the popup's own changes on top) and the last worker
/// graph it took in.
/// The same rule as the levels: a graph is adopted when it differs from the
/// last one taken in, never merely because it differs from what is shown.
#[derive(Debug, Default)]
pub struct GraphMirror {
    opened: AudioGraph,
    shown: AudioGraph,
    seen: AudioGraph,
    folds: Folds,
    /// Whether the graph listed anything when the popup opened.
    listed: bool,
    /// The name each listed row shows, cut to its column.
    labels: Vec<(u32, String)>,
    /// When the default mark the popup set itself gives way, if the worker's
    /// graph has not confirmed it before.
    mark_until: Option<Instant>,
}

impl GraphMirror {
    /// A mirror opened over `graph` (cut to its bounds). `fit` cuts a name to
    /// a width in `theme`'s body text (the text system's measure, in the
    /// product), so the names are cut once, as measured.
    #[must_use]
    pub fn open(
        graph: &AudioGraph,
        theme: &Theme,
        mut fit: impl FnMut(&str, f32) -> String,
    ) -> GraphMirror {
        let opened = graph.clone().bounded();
        let mut mirror = GraphMirror {
            labels: Vec::new(),
            listed: Shape::of(&opened, Folds::default()).listed,
            shown: opened.clone(),
            seen: opened.clone(),
            opened,
            folds: Folds::default(),
            mark_until: None,
        };
        mirror.recut(theme, &mut fit);
        mirror
    }

    /// Cuts every name again to its column in `theme` (the first cut, and a
    /// theme or text size that changed since), and the names of the defaults
    /// the worker holds that no row listed.
    pub fn recut(&mut self, theme: &Theme, mut fit: impl FnMut(&str, f32) -> String) {
        let device_w = device_name_w(theme);
        let stream_w = stream_name_w(theme);
        self.labels.clear();
        for device in self.opened.outputs.iter().chain(&self.opened.inputs) {
            self.labels.push((device.id, fit(&device.name, device_w)));
        }
        for stream in &self.opened.streams {
            self.labels.push((stream.id, fit(&stream.name, stream_w)));
        }
        self.cut_defaults(theme, &mut fit);
    }

    /// The rows the sections show. Whether the popup has its sections at all
    /// is what the graph listed when it opened, whatever a later fold finds.
    #[must_use]
    pub fn shape(&self) -> Shape {
        Shape {
            listed: self.listed,
            ..Shape::of(&self.opened, self.folds)
        }
    }

    /// Folds or unfolds `section`, and lists its rows again from what is
    /// shown now (the worker's graph with the popup's own changes): a stream
    /// that began to play, or a device that appeared, since the last time is
    /// in it. `fit` cuts the names of the rows as [`GraphMirror::open`] does.
    pub fn toggle(
        &mut self,
        section: Section,
        theme: &Theme,
        mut fit: impl FnMut(&str, f32) -> String,
    ) {
        self.folds.flip(section);
        let fresh = self.shown.clone().bounded();
        let (width, rows): (f32, Vec<(u32, String)>) = match section {
            Section::Output => {
                // A list the worker reports empty (an unreadable status, the
                // only microphone unplugged) keeps the rows it had, greyed.
                if !fresh.outputs.is_empty() {
                    self.opened.outputs = fresh.outputs;
                }
                (device_name_w(theme), named(&self.opened.outputs))
            }
            Section::Input => {
                if !fresh.inputs.is_empty() {
                    self.opened.inputs = fresh.inputs;
                }
                (device_name_w(theme), named(&self.opened.inputs))
            }
            Section::Apps => {
                self.opened.streams = fresh.streams;
                let streams = self.opened.streams.iter();
                (
                    stream_name_w(theme),
                    streams.map(|s| (s.id, s.name.clone())).collect(),
                )
            }
        };
        // The other lists keep their cuts; this one's are cut again, as its
        // names may have changed, and a row that is gone loses its own.
        let still: Vec<u32> = Section::ALL
            .into_iter()
            .filter(|other| *other != section)
            .flat_map(|other| self.listed_ids(other))
            .collect();
        self.labels.retain(|(id, _)| still.contains(id));
        for (id, name) in rows {
            self.labels.push((id, fit(&name, width)));
        }
        self.cut_defaults(theme, &mut fit);
    }

    /// Cuts the name of each default device the worker holds that no row
    /// listed (it appeared since the last fold), for a folded list shows it.
    pub fn cut_defaults(&mut self, theme: &Theme, mut fit: impl FnMut(&str, f32) -> String) {
        let width = device_name_w(theme);
        for device in self.shown.outputs.iter().chain(&self.shown.inputs) {
            if device.default && !self.labels.iter().any(|(id, _)| *id == device.id) {
                self.labels.push((device.id, fit(&device.name, width)));
            }
        }
    }

    /// The ids of the rows listed for `section`.
    fn listed_ids(&self, section: Section) -> Vec<u32> {
        match section {
            Section::Output => self.opened.outputs.iter().map(|d| d.id).collect(),
            Section::Input => self.opened.inputs.iter().map(|d| d.id).collect(),
            Section::Apps => self.opened.streams.iter().map(|s| s.id).collect(),
        }
    }

    /// What the card draws.
    #[must_use]
    pub fn audio(&self) -> Audio<'_> {
        Audio {
            opened: &self.opened,
            live: &self.shown,
            folds: self.folds,
            labels: &self.labels,
        }
    }

    /// The slider control of `key` among the listed streams.
    #[must_use]
    pub fn stream_slider(&self, key: Key) -> Option<Control> {
        self.controls().find(|c| slider_key(*c) == key)
    }

    /// The mute control of `key` among the listed streams.
    #[must_use]
    pub fn stream_mute(&self, key: Key) -> Option<Control> {
        self.controls().find(|c| mute_key(*c) == key)
    }

    fn controls(&self) -> impl Iterator<Item = Control> + '_ {
        self.opened.streams.iter().map(|s| Control::Stream(s.id))
    }

    /// The listed device whose row is `key`.
    #[must_use]
    pub fn device_of(&self, key: Key) -> Option<u32> {
        self.opened
            .outputs
            .iter()
            .chain(&self.opened.inputs)
            .map(|d| d.id)
            .find(|id| device_key(*id) == key)
    }

    /// Whether the worker lists the device `id` now (as far as shown).
    #[must_use]
    pub fn has_device(&self, id: u32) -> bool {
        self.shown.has_device(id)
    }

    fn stream_level_mut(&mut self, id: u32) -> Option<&mut AudioLevel> {
        self.shown
            .streams
            .iter_mut()
            .find(|s| s.id == id)
            .and_then(|s| s.level.as_mut())
    }

    /// The popup moved the stream `id`'s slider.
    pub fn set_level(&mut self, id: u32, percent: u8) {
        if let Some(level) = self.stream_level_mut(id) {
            level.percent = percent;
        }
    }

    /// The popup flipped the stream `id`'s mute.
    pub fn flip_mute(&mut self, id: u32) {
        if let Some(level) = self.stream_level_mut(id) {
            level.muted = !level.muted;
        }
    }

    /// Marks the device `id` as the default of its direction at once, ahead of
    /// the worker's graph, for [`MARK_HOLD`]; whether it was not the default
    /// already.
    pub fn mark_default(&mut self, id: u32, now: Instant) -> bool {
        let graph = &mut self.shown;
        let list = if graph.outputs.iter().any(|d| d.id == id) {
            &mut graph.outputs
        } else if graph.inputs.iter().any(|d| d.id == id) {
            &mut graph.inputs
        } else {
            return false;
        };
        if list.iter().any(|d| d.id == id && d.default) {
            return false;
        }
        for device in list.iter_mut() {
            device.default = device.id == id;
        }
        self.mark_until = Some(now + MARK_HOLD);
        true
    }

    /// Takes in the worker's graph `live`; whether what is shown changed. A
    /// graph that differs from the last one taken in replaces everything
    /// shown. Otherwise, once the hold of the popup's own default mark is
    /// over, what the worker says wins (a refused change publishes no graph,
    /// so nothing else would take the mark back). The caller does not call
    /// while a drag holds the sliders.
    pub fn sync(&mut self, live: &AudioGraph, now: Instant) -> bool {
        if *live != self.seen {
            self.seen = live.clone();
            self.shown = live.clone();
            self.mark_until = None;
            return true;
        }
        if self.mark_until.is_some_and(|until| now >= until) {
            self.mark_until = None;
            if self.shown != *live {
                self.shown = live.clone();
                return true;
            }
        }
        false
    }

    /// How long until the popup's own mark gives way; `None` with none held.
    #[must_use]
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        self.mark_until
            .map(|until| until.saturating_duration_since(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: u32, default: bool) -> GraphDevice {
        GraphDevice {
            id,
            name: format!("device {id}"),
            default,
        }
    }

    fn graph() -> AudioGraph {
        AudioGraph {
            outputs: vec![device(5, true), device(6, false)],
            inputs: vec![device(9, true)],
            streams: vec![GraphStream {
                id: 20,
                name: "Firefox".to_owned(),
                level: Some(AudioLevel {
                    percent: 60,
                    muted: false,
                }),
                playing: true,
            }],
        }
    }

    /// Every view under `v`, depth first.
    fn all(v: &View) -> Vec<&View> {
        let mut out = vec![v];
        for child in &v.children {
            out.extend(all(child));
        }
        out
    }

    fn icons_under(v: &View) -> Vec<&'static str> {
        all(v)
            .into_iter()
            .filter_map(|n| match n.kind {
                celestina_ui::view::ViewKind::Icon {
                    key: celestina_paint::IconKey::Semantic(name),
                    ..
                } => Some(name),
                _ => None,
            })
            .collect()
    }

    fn stream(muted: bool) -> GraphStream {
        GraphStream {
            id: 20,
            name: "Firefox".to_owned(),
            level: Some(AudioLevel { percent: 60, muted }),
            playing: true,
        }
    }

    #[test]
    fn a_stream_mutes_with_a_speaker_button_not_a_switch() {
        let theme = Theme::dark();
        for (muted, glyph) in [(false, "media-volume"), (true, "media-volume-muted")] {
            let live = stream(muted);
            let row = stream_row(&theme, "Firefox", 20, Some(&live));
            let button = all(&row)
                .into_iter()
                .find(|n| n.key == mute_key(Control::Stream(20)))
                .expect("the stream's mute control");
            assert!(button.interactive, "muted={muted}: the button takes clicks");
            assert_eq!(icons_under(button), vec![glyph], "muted={muted}");
            assert!(
                all(&row)
                    .iter()
                    .all(|n| n.key != Key::named("level-mute-switch")),
                "no switch is left in the row"
            );
        }
    }

    #[test]
    fn a_stream_without_a_level_has_a_faint_button_that_takes_no_input() {
        let theme = Theme::dark();
        let live = GraphStream {
            level: None,
            ..stream(false)
        };
        let row = stream_row(&theme, "Firefox", 20, Some(&live));
        let button = all(&row)
            .into_iter()
            .find(|n| n.key == mute_key(Control::Stream(20)))
            .expect("the stream's mute control");
        assert!(!button.interactive && !button.focusable);
        let tint = all(button).into_iter().find_map(|n| match n.kind {
            celestina_ui::view::ViewKind::Icon { tint, .. } => Some(tint),
            _ => None,
        });
        assert_eq!(tint, Some(Some(Role::TextFaint)));
    }

    #[test]
    fn a_muted_stream_dims_its_name() {
        let theme = Theme::dark();
        let role_of_name = |muted| {
            let live = stream(muted);
            let row = stream_row(&theme, "Firefox", 20, Some(&live));
            all(&row)
                .into_iter()
                .find(|n| n.key == Key::named("level-stream-name"))
                .and_then(|n| match &n.kind {
                    celestina_ui::view::ViewKind::Text { style, .. } => Some(style.role),
                    _ => None,
                })
        };
        assert_eq!(role_of_name(false), Some(Role::TextHi));
        assert_eq!(role_of_name(true), Some(Role::TextFaint));
    }

    fn default_of(mirror: &GraphMirror) -> Vec<u32> {
        mirror
            .audio()
            .live
            .outputs
            .iter()
            .filter(|d| d.default)
            .map(|d| d.id)
            .collect()
    }

    #[test]
    fn a_shape_is_cut_at_the_bounds_and_an_empty_one_lists_nothing() {
        let many: Vec<GraphDevice> = (0..12).map(|id| device(id, id == 0)).collect();
        let graph = AudioGraph {
            outputs: many.clone(),
            inputs: many,
            streams: Vec::new(),
        };
        let unfolded = Folds { open: [true; 3] };
        let shape = Shape::of(&graph, unfolded);
        assert_eq!((shape.outputs, shape.inputs), (8, 8));
        // Folded, a list shows its default device only.
        let folded = Shape::of(&graph, Folds::default());
        assert_eq!((folded.outputs, folded.inputs, folded.streams), (1, 1, 0));
        assert!(folded.listed);
        let theme = Theme::dark();
        let none = Lists::of(Kind::Volume, true, Shape::default());
        assert!(!none.apps && none.outputs == 0);
        assert!(Shape::of(&AudioGraph::default(), unfolded).is_empty());
        // The microphone's devices need the microphone.
        assert_eq!(Lists::of(Kind::Volume, false, shape).inputs, 0);
        assert_eq!(Lists::of(Kind::Brightness, true, shape).outputs, 0);
        // An empty list is one quiet line tall.
        assert_eq!(list_h(&theme, 0), list_h(&theme, 1));
    }

    #[test]
    fn a_fold_lists_its_rows_again_from_what_is_shown_and_cuts_their_names() {
        let theme = Theme::dark();
        let cut = |name: &str, _: f32| format!("cut {name}");
        let mut mirror = GraphMirror::open(&graph(), &theme, cut);
        assert_eq!(mirror.shape().streams, 1);
        // A second stream starts to play, and the worker's graph says so.
        let mut live = graph();
        live.streams.push(GraphStream {
            id: 21,
            name: "Mpv".to_owned(),
            level: None,
            playing: true,
        });
        assert!(mirror.sync(&live, Instant::now()));
        assert_eq!(mirror.shape().streams, 1, "waits for the next fold");
        mirror.toggle(Section::Apps, &theme, cut);
        assert_eq!(mirror.shape().streams, 2);
        assert!(mirror.audio().folds.is_open(Section::Apps));
        let labels = mirror.audio().labels.to_vec();
        assert!(labels.contains(&(21, "cut Mpv".to_owned())));
        // The other lists keep their rows and their cuts.
        assert!(labels.contains(&(5, "cut device 5".to_owned())));
        mirror.toggle(Section::Apps, &theme, cut);
        assert!(!mirror.audio().folds.is_open(Section::Apps));
        assert_eq!(
            section_of(section_key(Section::Input)),
            Some(Section::Input)
        );
        assert_eq!(section_of(Key::named("nope")), None);
    }

    #[test]
    fn the_mirror_marks_ahead_of_the_worker_and_the_workers_graph_wins() {
        let t0 = Instant::now();
        let mut mirror = GraphMirror::open(&graph(), &Theme::dark(), |name, _| name.to_owned());
        assert!(mirror.mark_default(6, t0));
        assert!(!mirror.mark_default(6, t0), "already the default");
        assert_eq!(default_of(&mirror), [6]);
        assert!(mirror.wait(t0).is_some());
        // The same graph inside the hold: the mark stands.
        assert!(!mirror.sync(&graph(), t0));
        assert_eq!(default_of(&mirror), [6]);
        // A refused change publishes nothing: past the hold the worker's
        // last graph takes the mark back.
        assert!(mirror.sync(&graph(), t0 + MARK_HOLD));
        assert_eq!(default_of(&mirror), [5]);
        assert!(mirror.wait(t0 + MARK_HOLD).is_none());
        // A graph that differs is adopted whole and ends the hold.
        mirror.mark_default(6, t0);
        let mut moved = graph();
        moved.outputs = vec![device(5, false), device(6, true)];
        assert!(mirror.sync(&moved, t0));
        assert_eq!(default_of(&mirror), [6]);
        assert!(mirror.wait(t0).is_none());
        assert!(!mirror.sync(&moved, t0 + MARK_HOLD * 2));
    }

    #[test]
    fn a_stream_changes_in_place_and_a_gone_one_is_ignored() {
        let mut mirror = GraphMirror::open(&graph(), &Theme::dark(), |name, _| name.to_owned());
        mirror.set_level(20, 33);
        mirror.flip_mute(20);
        mirror.set_level(99, 1);
        mirror.flip_mute(99);
        let level = mirror.audio().live.streams[0].level;
        assert_eq!(
            level,
            Some(AudioLevel {
                percent: 33,
                muted: true
            })
        );
        assert_eq!(mirror.device_of(device_key(9)), Some(9));
        assert_eq!(mirror.device_of(device_key(77)), None);
        assert!(mirror.has_device(6) && !mirror.has_device(77));
        // The worker's next graph replaces what the popup showed.
        assert!(mirror.sync(&AudioGraph::default(), Instant::now()));
        assert!(mirror.audio().live.streams.is_empty());
        // The rows listed at open stay.
        assert_eq!(mirror.shape().streams, 1);
    }
}
