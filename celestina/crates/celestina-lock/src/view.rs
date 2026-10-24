//! What the lock screen looks like and how it is put on the outputs (design
//! §4.6, §4.6.1, F16, F20).
//!
//! The view takes only the machine's [`Scene`]: how many characters were typed
//! (never which), the status, the wait, the Caps Lock and keyboard flags. It
//! knows no user name, no notification, no media and no window title. The
//! clock and the date are the one thing it takes from outside the scene: the
//! wall clock and the bundle's UTC offset ([`crate::clock`]), formatted at
//! each frame, drawn on the output that holds the prompt and on no other.
//! Without a known offset it draws the date and no time (design §4.8, M1).
//!
//! * Every output shows its backdrop with the scrim over it: the shell's
//!   blurred picture of the wallpaper, upscaled once per output by the
//!   `backdrop` worker and cached ([`crate::backdrop::Base`], scrim already
//!   composited), or plain black where the output has no picture, the bundle
//!   was invalid or slow, or the base is not there yet. The prompt (banner,
//!   field capsule, status capsule) is drawn on the one output the machine
//!   names, and on no other, over that base, so its contrast depends on the
//!   scrim and never on the picture. When the screen is faded nothing is
//!   drawn anywhere: black, the prompt included.
//! * One rectangle holds the whole prompt, always the same for an output. A
//!   key press restores that rectangle from the cached base in one of the
//!   output's two buffers, repaints it and damages only it (plus the
//!   rectangle of the frame shown before), so a 4K output costs a few hundred
//!   thousand pixels per press, not 8 million. The whole base is copied only
//!   when a buffer first shows it (or stops showing it, at the fade).
//! * A painter that cannot draw (no font, no shared memory) leaves the covers
//!   as they are; the field still takes keys blind (F16).
//!
//! There is no motion anywhere, so `reducedMotion` is honoured trivially:
//! nothing animates, not even the fade, which is a cut to black.
//!
//! Sizes and colours come from `celestina-tokens`. Two values are local
//! constants for now, [`SCRIM`] and [`VEIL`], and the fade delay is a local
//! constant of the machine; the author decided (2026-10-22) that they become
//! style tokens when the style program decides, not in this unit.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::time::SystemTime;

use celestina_tokens::{radius, size, type_scale, Rgba, DARK};
use celestina_wayland::lock::{FrameBuf, LockCtl, PresentError};
use celestina_wayland::{OutputId, PixelRect};
use cosmic_text::Align;
use tiny_skia::Pixmap;

use crate::backdrop::{Backdrop, Base, WakeLoop};
use crate::clock::local_time;
use crate::copy;
use crate::machine::{Phase, Scene, Status};
use crate::paint::{
    clear_xrgb, copy_rect_xrgb, copy_to_xrgb, disc, fill, read_xrgb, rounded_rect, stroke, Fonts,
    TextStyle,
};
use crate::present::Presenter;

/// The dimming laid over the backdrop, black at 60 % (local constant, see the
/// module head). Chosen so that the ink of the status text over the *whitest*
/// possible backdrop, scrim included, still reaches 4.5:1 (T-T1).
pub const SCRIM: Rgba = Rgba {
    r: 0,
    g: 0,
    b: 0,
    a: 0.60,
};

/// The veil behind the status text, black at 35 % over the scrim (local
/// constant, see the module head).
pub const VEIL: Rgba = Rgba {
    r: 0,
    g: 0,
    b: 0,
    a: 0.35,
};

/// Ink of the text that sits on the scrim: the primary text token.
pub const STATUS_INK: Rgba = DARK.text;
/// Ink of the field's placeholder and of the "checking" word, on the field's
/// opaque fill.
pub const FIELD_HINT_INK: Rgba = DARK.text_muted;
/// Fill of the field capsule.
pub const FIELD_FILL: Rgba = DARK.elevated;
/// Ink of the clock: large text on the scrim, which needs 3:1 (T-T1).
pub const CLOCK_INK: Rgba = DARK.text;
/// Ink of the date line: normal text on the scrim, which needs 4.5:1 (T-T1).
pub const DATE_INK: Rgba = DARK.text;

/// Logical width of the panel (field and status capsules).
const PANEL_W: f32 = 360.0;
/// Margin around the panel inside the prompt rectangle.
const PAD: f32 = 16.0;
/// The clock's size: twice the display step, large text by the WCAG measure.
const CLOCK_SIZE: f32 = type_scale::DISPLAY * 2.0;
/// The height of the clock's line: the painter's line height is 1.3 times the
/// size, rounded (a test pins the two numbers to that rule).
const CLOCK_LINE_H: f32 = 88.0;
/// The date's line, under the clock, and the gap after it.
const DATE_LINE_H: f32 = 22.0;
/// The height of the clock and date block at the top of the prompt rectangle.
const CLOCK_BLOCK: f32 = CLOCK_LINE_H + DATE_LINE_H + 10.0;
/// Logical height of the prompt rectangle: the clock and the date, banner,
/// field, up to three lines of status, the wait, the hint, Caps Lock.
const RECT_H: f32 = CLOCK_BLOCK + 320.0;
/// Where the field's top edge sits inside the prompt rectangle.
const FIELD_TOP: f32 = CLOCK_BLOCK + 32.0;
/// Where the rehearsal banner sits: just above the field.
const BANNER_TOP: f32 = FIELD_TOP - 26.0;
const GAP: f32 = 12.0;
const STATUS_PAD: f32 = 12.0;
const LINE_GAP: f32 = 6.0;
/// The prompt rectangle starts this fraction of the output's height down.
const TOP_FRACTION: f32 = 0.30;
/// Bullets shown at most; beyond this the last ones are shown.
const MAX_BULLETS: usize = 20;
const BULLET_PITCH: f32 = 14.0;
const BULLET_RADIUS: f32 = 4.0;

/// The logical rectangle of the prompt on an output of `logical` size:
/// `(x, y, w, h)`.
fn logical_rect(logical: (u32, u32)) -> (f32, f32, f32, f32) {
    let (lw, lh) = (logical.0 as f32, logical.1 as f32);
    let w = (PANEL_W + 2.0 * PAD).min(lw);
    let h = RECT_H.min(lh);
    let x = ((lw - w) / 2.0).max(0.0);
    let y = (lh * TOP_FRACTION).min((lh - h).max(0.0)).max(0.0);
    (x, y, w, h)
}

/// The prompt rectangle in buffer pixels, rounded outwards and clamped to the
/// buffer; `None` for an output with no area.
#[must_use]
pub fn prompt_rect(logical: (u32, u32), scale: f64, buffer: (u32, u32)) -> Option<PixelRect> {
    let (x, y, w, h) = logical_rect(logical);
    let s = scale as f32;
    let x0 = ((x * s).floor() as i64).clamp(0, i64::from(buffer.0));
    let y0 = ((y * s).floor() as i64).clamp(0, i64::from(buffer.1));
    let x1 = (((x + w) * s).ceil() as i64).clamp(0, i64::from(buffer.0));
    let y1 = (((y + h) * s).ceil() as i64).clamp(0, i64::from(buffer.1));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(PixelRect {
        x: i32::try_from(x0).ok()?,
        y: i32::try_from(y0).ok()?,
        w: i32::try_from(x1 - x0).ok()?,
        h: i32::try_from(y1 - y0).ok()?,
    })
}

/// The smallest rectangle holding both.
#[must_use]
fn union(a: PixelRect, b: PixelRect) -> PixelRect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = (a.x + a.w).max(b.x + b.w);
    let y1 = (a.y + a.h).max(b.y + b.h);
    PixelRect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    }
}

/// The clock and the date of one frame.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ClockLines {
    /// `HH:MM`; `None` when the offset is not known (M1): the lock does not
    /// show a time that may be an hour off.
    time: Option<String>,
    date: String,
}

/// The clock and date at `now` for the offset the bundle carried. An unknown
/// offset has no clock; the date then reads the day in UTC (the lock has no
/// other zone to offer).
fn clock_lines(now: SystemTime, utc_offset_s: Option<i32>) -> ClockLines {
    let local = local_time(now, utc_offset_s.unwrap_or(0));
    ClockLines {
        time: utc_offset_s.map(|_| local.clock()),
        date: local.date(),
    }
}

/// The words of the status capsule, top to bottom, with their size.
fn status_lines(scene: &Scene) -> Vec<(String, f32)> {
    let mut lines = Vec::new();
    let message = match scene.status {
        Status::Idle | Status::Checking => None,
        Status::Refused => Some(copy::LOCK_REFUSED),
        Status::Account => Some(copy::LOCK_ACCOUNT),
        Status::Unavailable => Some(copy::LOCK_UNAVAILABLE),
        Status::Unsupported => Some(copy::LOCK_UNSUPPORTED),
        Status::ServiceMissing => Some(copy::LOCK_SERVICE_MISSING),
    };
    if let Some(message) = message {
        lines.push((message.to_owned(), type_scale::TITLE));
    }
    if let Some(seconds) = scene.wait_secs {
        lines.push((copy::lock_wait(seconds), type_scale::ROW_TITLE));
    }
    if scene.faillock_hint {
        lines.push((copy::LOCK_FAILLOCK_HINT.to_owned(), type_scale::BODY));
    }
    if scene.caps_lock {
        lines.push((copy::LOCK_CAPS.to_owned(), type_scale::BODY));
    }
    if !scene.keyboard {
        lines.push((copy::LOCK_NO_KEYBOARD.to_owned(), type_scale::BODY));
    }
    lines
}

/// Where the prompt rectangle sits in its pixmap: the pixmap's top-left is at
/// buffer pixel `(x0, y0)`, which is a little before the logical rectangle's
/// origin because it was rounded outwards.
struct Geometry {
    scale: f32,
    /// Offset in pixmap pixels of the logical rectangle's origin.
    origin: (f32, f32),
}

impl Geometry {
    /// Pixmap pixels of a rectangle-relative logical coordinate.
    fn px(&self, x: f32, y: f32) -> (f32, f32) {
        (
            x * self.scale + self.origin.0,
            y * self.scale + self.origin.1,
        )
    }
}

/// The backdrop under a prompt: the cached base, and the buffer pixel the
/// prompt's pixmap starts at.
#[derive(Clone, Copy)]
struct Under<'a> {
    base: &'a Base,
    at: (u32, u32),
}

/// Draws the prompt of `scene` into `pixmap`, which covers the prompt
/// rectangle of an output of `logical` size, over `under`.
fn draw_prompt(
    fonts: &mut Fonts,
    pixmap: &mut Pixmap,
    scene: &Scene,
    geometry: &Geometry,
    under: Option<Under<'_>>,
    clock: &ClockLines,
) {
    let s = geometry.scale;
    match under {
        // The base already has the scrim over the picture.
        Some(Under { base, at }) => read_xrgb(
            pixmap,
            base.pixels(),
            base.stride(),
            (base.width(), base.height()),
            at,
        ),
        // No picture: black, the scrim over it.
        None => {
            pixmap.fill(tiny_skia::Color::BLACK);
            if let Some(all) =
                rounded_rect(0.0, 0.0, pixmap.width() as f32, pixmap.height() as f32, 0.0)
            {
                fill(pixmap, &all, SCRIM);
            }
        }
    }

    // The clock, large, and the date under it: on the scrim, no capsule.
    if let Some(time) = &clock.time {
        let (x, y) = geometry.px(PAD, 0.0);
        fonts.draw(
            pixmap,
            time,
            TextStyle {
                size: CLOCK_SIZE * s,
                ink: CLOCK_INK,
                align: Align::Center,
            },
            (x, y),
            PANEL_W * s,
        );
    }
    let (x, y) = geometry.px(PAD, CLOCK_LINE_H);
    fonts.draw(
        pixmap,
        &clock.date,
        TextStyle {
            size: type_scale::TITLE * s,
            ink: DATE_INK,
            align: Align::Center,
        },
        (x, y),
        PANEL_W * s,
    );

    // Rehearsal banner.
    if scene.rehearsal {
        let (x, y) = geometry.px(PAD, BANNER_TOP);
        fonts.draw(
            pixmap,
            copy::LOCK_REHEARSAL,
            TextStyle {
                size: type_scale::BODY * s,
                ink: STATUS_INK,
                align: Align::Center,
            },
            (x, y),
            PANEL_W * s,
        );
    }

    // The field capsule.
    let field_h = size::CONTROL_HEIGHT_XL;
    let (fx, fy) = geometry.px(PAD, FIELD_TOP);
    if let Some(capsule) = rounded_rect(fx, fy, PANEL_W * s, field_h * s, radius::PILL * s) {
        fill(pixmap, &capsule, FIELD_FILL);
        stroke(pixmap, &capsule, DARK.input_border, (s).max(1.0));
    }
    let line_h = (type_scale::TITLE * 1.3).round();
    let text_y = fy + (field_h * s - line_h * s) / 2.0;
    match (scene.phase, scene.field_chars) {
        (Phase::Checking, _) => {
            fonts.draw(
                pixmap,
                copy::LOCK_CHECKING,
                TextStyle {
                    size: type_scale::TITLE * s,
                    ink: FIELD_HINT_INK,
                    align: Align::Center,
                },
                (fx, text_y),
                PANEL_W * s,
            );
        }
        (Phase::Prompt, 0) => {
            fonts.draw(
                pixmap,
                copy::LOCK_FIELD,
                TextStyle {
                    size: type_scale::TITLE * s,
                    ink: FIELD_HINT_INK,
                    align: Align::Center,
                },
                (fx, text_y),
                PANEL_W * s,
            );
        }
        (Phase::Prompt, chars) => {
            // One bullet per character, never the text.
            let shown = chars.min(MAX_BULLETS);
            let span = (shown.saturating_sub(1)) as f32 * BULLET_PITCH;
            let cy = fy + field_h * s / 2.0;
            let start = fx + (PANEL_W * s - span * s) / 2.0;
            for n in 0..shown {
                disc(
                    pixmap,
                    start + n as f32 * BULLET_PITCH * s,
                    cy,
                    BULLET_RADIUS * s,
                    DARK.elevated_ink,
                );
            }
        }
    }

    // The status capsule: only when there is something to say.
    let lines = status_lines(scene);
    if lines.is_empty() {
        return;
    }
    let text_w = PANEL_W - 2.0 * STATUS_PAD;
    let mut heights = Vec::with_capacity(lines.len());
    for (text, size_px) in &lines {
        heights.push(fonts.measure(text, size_px * s, Some(text_w * s)).height);
    }
    let body: f32 = heights.iter().sum::<f32>() + LINE_GAP * s * (lines.len() - 1) as f32;
    let capsule_h = body + 2.0 * STATUS_PAD * s;
    let (sx, sy) = geometry.px(PAD, FIELD_TOP + field_h + GAP);
    if let Some(capsule) = rounded_rect(sx, sy, PANEL_W * s, capsule_h, radius::LG * s) {
        fill(pixmap, &capsule, VEIL);
    }
    let mut y = sy + STATUS_PAD * s;
    for ((text, size_px), height) in lines.iter().zip(heights) {
        fonts.draw(
            pixmap,
            text,
            TextStyle {
                size: size_px * s,
                ink: STATUS_INK,
                align: Align::Center,
            },
            (sx + STATUS_PAD * s, y),
            text_w * s,
        );
        y += height + LINE_GAP * s;
    }
}

/// Where the painter puts a frame.
pub trait Frames {
    /// Draws one frame on `output`; see [`LockCtl::present`].
    ///
    /// # Errors
    ///
    /// Why no frame was shown.
    fn present(
        &mut self,
        output: OutputId,
        paint: &mut dyn FnMut(FrameBuf<'_>) -> Option<PixelRect>,
    ) -> Result<(), PresentError>;
}

impl Frames for LockCtl<'_> {
    fn present(
        &mut self,
        output: OutputId,
        paint: &mut dyn FnMut(FrameBuf<'_>) -> Option<PixelRect>,
    ) -> Result<(), PresentError> {
        LockCtl::present(self, output, paint)
    }
}

/// What a whole buffer shows behind the prompt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Layer {
    /// Black: the cover, a faded screen, an output with no base.
    #[default]
    Black,
    /// The cached base, scrim included.
    Base,
}

/// What the painter remembers of one output: which rectangle it drew into
/// each of the two buffers, what is behind it there, and which is on screen.
#[derive(Debug, Default)]
struct OutputPaint {
    /// The rectangle last drawn into buffer 0 and 1.
    dirty: [Option<PixelRect>; 2],
    /// What each buffer holds behind its rectangle.
    layer: [Layer; 2],
    /// The rectangle of the frame on screen; `None` when the screen shows
    /// nothing but its layer (the cover, or a frame without a prompt).
    shown: Option<PixelRect>,
    /// The layer of the frame on screen.
    shown_layer: Layer,
    /// The buffer size of the last frame, which is the size to ask the
    /// worker for while the output keeps the logical size and scale in
    /// `sized_for`.
    last_size: Option<(u32, u32)>,
    /// The logical size and scale `last_size` was seen at.
    sized_for: Option<((u32, u32), f64)>,
}

/// The buffer size an output is expected to have, until a frame says.
fn expected_size(logical: (u32, u32), scale: f64) -> (u32, u32) {
    let side = |px: u32| ((f64::from(px) * scale).round() as u32).max(1);
    (side(logical.0), side(logical.1))
}

/// Draws one output into `frame`: puts back what this buffer last drew over
/// its layer, draws the prompt when `want_prompt`, and returns the damage.
fn paint_output(
    state: &mut OutputPaint,
    frame: FrameBuf<'_>,
    scene: &Scene,
    logical: (u32, u32),
    want_prompt: bool,
    fonts: Option<&mut Fonts>,
    base: Option<&Base>,
    clock: &ClockLines,
) -> Option<PixelRect> {
    let size = (frame.width, frame.height);
    let index = frame.index.min(1);
    let whole = PixelRect {
        x: 0,
        y: 0,
        w: i32::try_from(size.0).unwrap_or(i32::MAX),
        h: i32::try_from(size.1).unwrap_or(i32::MAX),
    };
    state.last_size = Some(size);
    // A base of another size than the buffer is no base: black until the
    // worker has the right one.
    let base = base.filter(|b| (b.width(), b.height()) == size);
    let layer = match base {
        Some(_) if !scene.faded => Layer::Base,
        _ => Layer::Black,
    };
    if frame.fresh {
        // Zeroed memory: whatever was believed about it is gone.
        state.dirty[index] = None;
        state.layer[index] = Layer::Black;
    }
    let everything = (0, 0, size.0, size.1);
    let mut touched = false;
    if state.layer[index] != layer {
        // The one full-output copy: this buffer first shows the base, or
        // stops showing it.
        match (layer, base) {
            (Layer::Base, Some(base)) => copy_rect_xrgb(
                (base.pixels(), base.stride()),
                (frame.pixels, frame.stride),
                size,
                everything,
            ),
            _ => clear_xrgb(frame.pixels, frame.stride, size, everything),
        }
        state.layer[index] = layer;
        state.dirty[index] = None;
        touched = true;
    } else if let Some(old) = state.dirty[index].take() {
        let rect = (
            u32::try_from(old.x).unwrap_or(0),
            u32::try_from(old.y).unwrap_or(0),
            u32::try_from(old.w).unwrap_or(0),
            u32::try_from(old.h).unwrap_or(0),
        );
        match (layer, base) {
            (Layer::Base, Some(base)) => copy_rect_xrgb(
                (base.pixels(), base.stride()),
                (frame.pixels, frame.stride),
                size,
                rect,
            ),
            _ => clear_xrgb(frame.pixels, frame.stride, size, rect),
        }
        touched = true;
    }

    let mut drawn = None;
    if want_prompt {
        if let (Some(fonts), Some(rect)) = (fonts, prompt_rect(logical, frame.scale, size)) {
            if let Some(mut pixmap) = Pixmap::new(rect.w.unsigned_abs(), rect.h.unsigned_abs()) {
                let (lx, ly, _, _) = logical_rect(logical);
                let s = frame.scale as f32;
                let geometry = Geometry {
                    scale: s,
                    origin: (lx * s - rect.x as f32, ly * s - rect.y as f32),
                };
                let at = (rect.x.unsigned_abs(), rect.y.unsigned_abs());
                let under = match layer {
                    Layer::Base => base.map(|base| Under { base, at }),
                    Layer::Black => None,
                };
                draw_prompt(fonts, &mut pixmap, scene, &geometry, under, clock);
                copy_to_xrgb(&pixmap, frame.pixels, frame.stride, size, at);
                drawn = Some(rect);
                touched = true;
            }
        }
    }
    state.dirty[index] = drawn;

    let previous = state.shown;
    state.shown = drawn;
    let layer_changed = state.shown_layer != layer;
    state.shown_layer = layer;
    if frame.fresh || layer_changed {
        return Some(whole);
    }
    match (previous, drawn) {
        (Some(a), Some(b)) => Some(union(a, b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        // Nothing was on screen and nothing is: a buffer that was only
        // cleared changed nothing visible.
        (None, None) => touched.then_some(PixelRect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        }),
    }
}

/// The embedded font is loaded on the first frame that needs text, after
/// `locked`: nothing of the painter weighs on the path to the covers.
enum FontState {
    Pending,
    Ready(Box<Fonts>),
    /// The font could not be loaded; the covers stay (F16).
    Failed,
}

impl FontState {
    /// The fonts, loading them on first use.
    fn get(&mut self) -> Option<&mut Fonts> {
        if matches!(self, Self::Pending) {
            *self = match Fonts::embedded() {
                Ok(fonts) => Self::Ready(Box::new(fonts)),
                Err(err) => {
                    tracing::error!(%err, "the painter cannot start; the covers stay");
                    Self::Failed
                }
            };
        }
        match self {
            Self::Ready(fonts) => Some(fonts),
            Self::Pending | Self::Failed => None,
        }
    }
}

/// The CPU painter: the lock's [`Presenter`].
pub struct CpuPresenter {
    fonts: FontState,
    outputs: BTreeMap<OutputId, OutputPaint>,
    /// The last failure that was logged, so a stuck painter says it once.
    last_failure: Option<String>,
    /// How long the last frame that drew a prompt took to paint and convert
    /// (CPU only: not the commit, not the compositor). Read once by
    /// [`CpuPresenter::take_paint_time`]: the `paint_probe` example and the ignored
    /// measurement test read it (the rehearsal does not).
    last_paint: Option<std::time::Duration>,
    /// The `backdrop` worker and its cache; `None` is a black backdrop.
    backdrop: Option<Backdrop>,
    /// The bundle's UTC offset in seconds; `None` hides the clock (M1).
    utc_offset_s: Option<i32>,
    /// The wall clock, read once per render; a field so the tests move it.
    now: fn() -> SystemTime,
}

impl CpuPresenter {
    /// Starts the painter. It loads its font on the first frame; when that
    /// fails it stays blind: covers only, and the field still takes keys
    /// (F16).
    #[must_use]
    pub fn new() -> Self {
        Self::with_backdrop(None)
    }

    /// Starts the painter over the `backdrop` worker's bases; `None` shows
    /// black behind the prompt (no picture, an invalid or slow bundle).
    #[must_use]
    pub fn with_backdrop(backdrop: Option<Backdrop>) -> Self {
        Self {
            fonts: FontState::Pending,
            outputs: BTreeMap::new(),
            last_failure: None,
            last_paint: None,
            backdrop,
            utc_offset_s: None,
            now: SystemTime::now,
        }
    }

    /// Sets the zone of the clock: the offset the bundle carried, or `None`
    /// when the shell could not read it (the date alone is drawn then).
    #[must_use]
    pub fn with_utc_offset(mut self, utc_offset_s: Option<i32>) -> Self {
        self.utc_offset_s = utc_offset_s;
        self
    }

    /// The CPU time of the last prompt frame painted since the previous call.
    pub fn take_paint_time(&mut self) -> Option<std::time::Duration> {
        self.last_paint.take()
    }

    /// Whether the painter can still draw text (it has not failed to start).
    #[must_use]
    pub fn can_draw(&self) -> bool {
        !matches!(self.fonts, FontState::Failed)
    }

    /// Draws `scene` on `frames`: the prompt on the output the scene names,
    /// black everywhere else, black everywhere when faded.
    pub fn render_to(&mut self, scene: &Scene, frames: &mut impl Frames) {
        self.outputs
            .retain(|id, _| scene.outputs.iter().any(|o| o.id == *id));
        if let Some(backdrop) = self.backdrop.as_mut() {
            backdrop.retain_outputs(scene.outputs.iter().map(|o| o.name.as_str()));
        }
        // One reading of the clock per render: every output of the render
        // shows the same minute.
        let clock = clock_lines((self.now)(), self.utc_offset_s);
        for output in &scene.outputs {
            let Some(logical) = output.size else {
                continue;
            };
            let want_prompt = !scene.faded && scene.prompt == Some(output.id);
            let state = self.outputs.entry(output.id).or_default();
            // The base for the buffer this output has (or is expected to
            // have): `None` while the worker computes, and for an output
            // with no picture.
            let key = (logical, output.scale);
            let expected = match (state.sized_for, state.last_size) {
                (Some(seen), Some(size)) if seen == key => size,
                _ => expected_size(logical, output.scale),
            };
            let base = self
                .backdrop
                .as_mut()
                .and_then(|backdrop| backdrop.base_for(&output.name, expected));
            let layer = if base.is_some() && !scene.faded {
                Layer::Base
            } else {
                Layer::Black
            };
            if !want_prompt && state.shown.is_none() && state.shown_layer == layer {
                // What is on screen is what is wanted: the cover, or the
                // base already shown, says it.
                continue;
            }
            let fonts = if want_prompt { self.fonts.get() } else { None };
            if want_prompt && fonts.is_none() {
                continue;
            }
            let mut fonts = fonts;
            let last_paint = &mut self.last_paint;
            let result = frames.present(output.id, &mut |frame| {
                let started = std::time::Instant::now();
                let damage = paint_output(
                    state,
                    frame,
                    scene,
                    logical,
                    want_prompt,
                    fonts.as_deref_mut(),
                    base.as_deref(),
                    &clock,
                );
                if want_prompt {
                    *last_paint = Some(started.elapsed());
                }
                damage
            });
            match result {
                Ok(()) => self.last_failure = None,
                Err(err) => {
                    let said = err.to_string();
                    if self.last_failure.as_deref() != Some(said.as_str()) {
                        // F16: the covers stay; typing blind still works.
                        tracing::warn!(output = output.id.0, %err, "a frame was not shown");
                        self.last_failure = Some(said);
                    }
                }
            }
            // The buffer is what the frame said it was: remember it for this
            // logical size and scale, and when the guess was wrong ask the
            // worker for the real size now, because no other redraw is coming
            // to ask.
            if let Some(actual) = state.last_size {
                state.sized_for = Some(key);
                if actual != expected {
                    if let Some(backdrop) = self.backdrop.as_mut() {
                        let _ = backdrop.base_for(&output.name, actual);
                    }
                }
            }
        }
    }
}

impl Default for CpuPresenter {
    fn default() -> Self {
        Self::new()
    }
}

impl Presenter for CpuPresenter {
    fn render(&mut self, scene: &Scene, ctl: &mut LockCtl<'_>) {
        self.render_to(scene, ctl);
    }

    fn set_wake(&mut self, wake: WakeLoop) {
        if let Some(backdrop) = self.backdrop.as_mut() {
            backdrop.set_wake(wake);
        }
    }
}

// The scanner helper shared by the lock's source guards (round 7: the one
// owner of "production code without its test items").
#[cfg(test)]
#[path = "../../celestina-wayland/tests/support/test_gates.rs"]
mod test_gates;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::SceneOutput;

    const LOGICAL: (u32, u32) = (2560, 1440);
    const SCALE: f64 = 1.5;
    const PHYSICAL: (u32, u32) = (3840, 2160);

    fn scene() -> Scene {
        Scene {
            outputs: vec![
                SceneOutput {
                    id: OutputId(1),
                    name: "DP-1".to_owned(),
                    size: Some(LOGICAL),
                    scale: 2.0,
                },
                SceneOutput {
                    id: OutputId(2),
                    name: "DP-2".to_owned(),
                    size: Some((1920, 1080)),
                    scale: 1.0,
                },
            ],
            prompt: Some(OutputId(1)),
            phase: Phase::Prompt,
            field_chars: 0,
            status: Status::Idle,
            wait_secs: None,
            faillock_hint: false,
            caps_lock: false,
            keyboard: true,
            faded: false,
            rehearsal: false,
        }
    }

    /// A fake compositor: two buffers per output, all frames accepted.
    struct Screen {
        outputs: BTreeMap<OutputId, Fake>,
        refuse: Option<fn() -> PresentError>,
        frames: usize,
    }

    struct Fake {
        buffers: [Vec<u8>; 2],
        size: (u32, u32),
        scale: f64,
        next: usize,
        fresh: [bool; 2],
        damage: Vec<PixelRect>,
    }

    impl Fake {
        fn new(size: (u32, u32), scale: f64) -> Self {
            let len = size.0 as usize * size.1 as usize * 4;
            Self {
                buffers: [vec![0; len], vec![0; len]],
                size,
                scale,
                next: 0,
                fresh: [true; 2],
                damage: Vec::new(),
            }
        }
    }

    impl Screen {
        fn new(outputs: &[(u32, (u32, u32), f64)]) -> Self {
            Self {
                outputs: outputs
                    .iter()
                    .map(|(id, size, scale)| (OutputId(*id), Fake::new(*size, *scale)))
                    .collect(),
                refuse: None,
                frames: 0,
            }
        }
    }

    impl Frames for Screen {
        fn present(
            &mut self,
            output: OutputId,
            paint: &mut dyn FnMut(FrameBuf<'_>) -> Option<PixelRect>,
        ) -> Result<(), PresentError> {
            if let Some(make) = self.refuse {
                return Err(make());
            }
            let fake = self
                .outputs
                .get_mut(&output)
                .ok_or(PresentError::UnknownOutput)?;
            let index = fake.next;
            let fresh = fake.fresh[index];
            let stride = fake.size.0 as usize * 4;
            let frame = FrameBuf {
                index,
                fresh,
                width: fake.size.0,
                height: fake.size.1,
                stride,
                scale: fake.scale,
                pixels: &mut fake.buffers[index],
            };
            let damage = paint(frame);
            fake.fresh[index] = false;
            if let Some(damage) = damage {
                fake.damage.push(damage);
                fake.next = (index + 1) % 2;
                self.frames += 1;
            }
            Ok(())
        }
    }

    fn non_black(fake: &Fake, index: usize) -> usize {
        fake.buffers[index]
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[0] | px[1] | px[2] != 0)
            .count()
    }

    /// Pixels of the bright ink (bullets, status text) in a buffer.
    fn bright(fake: &Fake, index: usize) -> usize {
        fake.buffers[index]
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[0] > 200 && px[1] > 200 && px[2] > 200)
            .count()
    }

    fn painter() -> CpuPresenter {
        let painter = CpuPresenter::new();
        assert!(painter.can_draw(), "the embedded font loads");
        painter
    }

    fn two_outputs() -> Screen {
        Screen::new(&[(1, PHYSICAL, SCALE), (2, (1920, 1080), 1.0)])
    }

    #[test]
    fn the_prompt_is_drawn_only_on_the_focused_output() {
        let mut painter = painter();
        let mut screen = two_outputs();
        painter.render_to(&scene(), &mut screen);
        assert!(non_black(&screen.outputs[&OutputId(1)], 0) > 0);
        assert_eq!(
            screen.outputs[&OutputId(2)].damage.len(),
            0,
            "the other output stays on its cover: no frame at all"
        );
        // The prompt moves: the first output goes black again, the second shows it.
        let mut moved = scene();
        moved.prompt = Some(OutputId(2));
        painter.render_to(&moved, &mut screen);
        assert!(non_black(&screen.outputs[&OutputId(2)], 0) > 0);
        let first = &screen.outputs[&OutputId(1)];
        assert_eq!(first.damage.len(), 2, "a black frame replaced the prompt");
        assert_eq!(non_black(first, 1), 0, "and it holds no prompt");
    }

    #[test]
    fn the_field_shows_one_bullet_per_char_and_never_the_text() {
        let mut painter = painter();
        let mut screen = two_outputs();
        let mut counts = Vec::new();
        for chars in [1usize, 4, 8] {
            let mut s = scene();
            s.field_chars = chars;
            painter.render_to(&s, &mut screen);
            // The frame just shown is in the buffer drawn last.
            let fake = &screen.outputs[&OutputId(1)];
            counts.push(bright(fake, (fake.next + 1) % 2));
        }
        assert!(
            counts[0] < counts[1] && counts[1] < counts[2],
            "more characters, more bullets: {counts:?}"
        );
        // The scene has no field to read: it cannot draw what it never had.
        let source = include_str!("view.rs");
        let production = super::test_gates::without_test_blocks(source);
        assert!(!production.contains("utf8") && !production.contains("keysym"));
    }

    #[test]
    fn prompt_follows_the_scene_before_any_keyboard_enter_and_defaults_to_the_first_output_by_name()
    {
        // The machine decides the output (its own tests cover keyboard,
        // pointer and name order); the painter draws exactly where the scene
        // says, and nowhere when the scene names none.
        let mut painter = painter();
        let mut screen = two_outputs();
        let mut none = scene();
        none.prompt = None;
        painter.render_to(&none, &mut screen);
        assert_eq!(
            screen.frames, 0,
            "no prompt, nothing to draw over the covers"
        );
        let mut first = scene();
        first.prompt = Some(OutputId(1));
        painter.render_to(&first, &mut screen);
        assert_eq!(screen.outputs[&OutputId(1)].damage.len(), 1);
    }

    #[test]
    fn a_painter_failure_keeps_the_covers_and_typing_blind_still_works() {
        // F16: the shared memory cannot be had.
        let mut painter = painter();
        let mut screen = two_outputs();
        screen.refuse = Some(|| PresentError::Busy);
        for chars in 0..3 {
            let mut s = scene();
            s.field_chars = chars;
            painter.render_to(&s, &mut screen);
        }
        assert_eq!(screen.frames, 0, "no frame was shown, no panic");
        assert_eq!(non_black(&screen.outputs[&OutputId(1)], 0), 0);
        // The painter recovers when memory comes back.
        screen.refuse = None;
        painter.render_to(&scene(), &mut screen);
        assert_eq!(screen.frames, 1);

        // The field itself does not depend on the painter: the machine takes
        // keys with no presenter at all (blind typing), see
        // `machine::tests` and `driver::tests`; here a painter that cannot
        // load its font draws nothing and says so.
        let mut blind = CpuPresenter::new();
        blind.fonts = FontState::Failed;
        let mut screen = two_outputs();
        blind.render_to(&scene(), &mut screen);
        assert_eq!(screen.frames, 0);
    }

    #[test]
    fn faded_outputs_are_drawn_black_prompt_included() {
        let mut painter = painter();
        let mut screen = two_outputs();
        let mut s = scene();
        s.field_chars = 5;
        painter.render_to(&s, &mut screen);
        assert!(non_black(&screen.outputs[&OutputId(1)], 0) > 0);
        s.faded = true;
        painter.render_to(&s, &mut screen);
        let fake = &screen.outputs[&OutputId(1)];
        assert_eq!(non_black(fake, 1), 0, "the faded frame is all black");
        // The prompt comes back, the field's count intact.
        s.faded = false;
        painter.render_to(&s, &mut screen);
        let fake = &screen.outputs[&OutputId(1)];
        assert!(non_black(fake, 0) > 0);
    }

    #[test]
    fn damage_is_the_prompt_rectangle_only_per_key_press() {
        let mut painter = painter();
        let mut screen = two_outputs();
        let mut s = scene();
        painter.render_to(&s, &mut screen);
        // Second and third presses: a steady state of the two buffers.
        for chars in 1..=3 {
            s.field_chars = chars;
            painter.render_to(&s, &mut screen);
        }
        let fake = &screen.outputs[&OutputId(1)];
        let prompt = prompt_rect(LOGICAL, SCALE, PHYSICAL).expect("rect");
        let steady = &fake.damage[2..];
        assert!(!steady.is_empty());
        for damage in steady {
            assert_eq!(*damage, prompt, "a press damages the prompt rectangle only");
        }
        let area = i64::from(prompt.w) * i64::from(prompt.h);
        let whole = i64::from(PHYSICAL.0) * i64::from(PHYSICAL.1);
        assert!(
            area * 20 < whole,
            "the rectangle is a small part of a 4K output"
        );
        // Nothing outside the rectangle was ever drawn.
        for index in 0..2 {
            let buffer = &fake.buffers[index];
            for (n, px) in buffer.as_chunks::<4>().0.iter().enumerate() {
                if px[0] | px[1] | px[2] == 0 {
                    continue;
                }
                let (x, y) = (
                    (n as u32 % PHYSICAL.0) as i32,
                    (n as u32 / PHYSICAL.0) as i32,
                );
                assert!(
                    x >= prompt.x
                        && x < prompt.x + prompt.w
                        && y >= prompt.y
                        && y < prompt.y + prompt.h,
                    "pixel ({x},{y}) lies outside the prompt rectangle"
                );
            }
        }
    }

    /// The seconds the measurement's wall clock reads (the painter's clock is
    /// a plain `fn`, so the minute it shows moves through this).
    static MEASURED_NOW: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(1_792_791_015);

    fn measured_now() -> SystemTime {
        std::time::UNIX_EPOCH
            + Duration::from_secs(MEASURED_NOW.load(std::sync::atomic::Ordering::Relaxed))
    }

    /// H-15's number: the CPU time of one key press on a 4K output at 1.5,
    /// painted over a real [`crate::backdrop::Base`] (a bright block through
    /// the worker, scrim composited) and with the clock lines, plus the
    /// redraw a new minute causes. Needs no Wayland and no binary.
    /// Run with `cargo test --release -p celestina-lock -- --ignored --nocapture measure_the_cpu_time`.
    #[test]
    #[ignore = "a measurement, not a check"]
    fn measure_the_cpu_time_of_a_key_press_at_4k_scale_1_5() {
        use std::sync::atomic::Ordering::Relaxed;
        let mut painter =
            painter_over(vec![solid("DP-1", 64, 36, [250, 250, 250])]).with_utc_offset(Some(7_200));
        painter.now = measured_now;
        let mut screen = two_outputs();
        let mut s = scene();
        // The output's real scale, so the base is asked at its real buffer.
        s.outputs[0].scale = SCALE;
        settle(&mut painter, &s, &mut screen);
        let shown = on_screen(&screen.outputs[&OutputId(1)]).to_vec();
        assert!(
            shown
                .as_chunks::<4>()
                .0
                .iter()
                .any(|px| px[0] > 90 && px[0] < 110),
            "the dimmed backdrop (white under the 60 % scrim, about 102) is on screen"
        );
        let _ = painter.take_paint_time();

        let mut presses = Vec::new();
        for n in 0..600usize {
            s.field_chars = n % 24 + 1;
            painter.render_to(&s, &mut screen);
            if let Some(took) = painter.take_paint_time() {
                presses.push(took.as_micros());
            }
        }
        // The redraw of a new minute: same field, the clock text changed.
        let mut minutes = Vec::new();
        for _ in 0..40 {
            MEASURED_NOW.fetch_add(60, Relaxed);
            painter.render_to(&s, &mut screen);
            if let Some(took) = painter.take_paint_time() {
                minutes.push(took.as_micros());
            }
        }
        for (what, mut micros) in [("key press", presses), ("minute redraw", minutes)] {
            micros.sort_unstable();
            let at = |p: f64| micros[((micros.len() - 1) as f64 * p).round() as usize];
            println!(
                "paint time of a {what} over the backdrop with the clock, 3840x2160 at 1.5: \
                 n={} p50={}us p99={}us max={}us",
                micros.len(),
                at(0.5),
                at(0.99),
                micros[micros.len() - 1]
            );
        }
    }

    #[test]
    fn caps_lock_shows_the_message() {
        let mut painter = painter();
        let mut plain = two_outputs();
        let mut caps = two_outputs();
        painter.render_to(&scene(), &mut plain);
        let mut s = scene();
        s.caps_lock = true;
        let mut painter2 = CpuPresenter::new();
        painter2.render_to(&s, &mut caps);
        let a = bright(&plain.outputs[&OutputId(1)], 0);
        let b = bright(&caps.outputs[&OutputId(1)], 0);
        assert!(
            b > a,
            "the Caps Lock capsule and message add pixels: {a} vs {b}"
        );
        assert!(status_lines(&s).iter().any(|(t, _)| t == copy::LOCK_CAPS));
        assert!(!status_lines(&scene())
            .iter()
            .any(|(t, _)| t == copy::LOCK_CAPS));
    }

    #[test]
    fn no_keyboard_capability_shows_the_message() {
        let mut s = scene();
        s.keyboard = false;
        assert!(status_lines(&s)
            .iter()
            .any(|(t, _)| t == copy::LOCK_NO_KEYBOARD));
        let mut painter = painter();
        let mut with = two_outputs();
        let mut without = two_outputs();
        painter.render_to(&s, &mut with);
        CpuPresenter::new().render_to(&scene(), &mut without);
        assert!(bright(&with.outputs[&OutputId(1)], 0) > bright(&without.outputs[&OutputId(1)], 0));
    }

    #[test]
    fn the_status_words_follow_the_status_and_the_countdown() {
        let mut s = scene();
        s.status = Status::Refused;
        s.wait_secs = Some(4);
        s.faillock_hint = true;
        let lines: Vec<String> = status_lines(&s).into_iter().map(|(t, _)| t).collect();
        assert_eq!(
            lines,
            [
                copy::LOCK_REFUSED.to_owned(),
                copy::lock_wait(4),
                copy::LOCK_FAILLOCK_HINT.to_owned()
            ]
        );
        for (status, word) in [
            (Status::Account, copy::LOCK_ACCOUNT),
            (Status::Unavailable, copy::LOCK_UNAVAILABLE),
            (Status::Unsupported, copy::LOCK_UNSUPPORTED),
            (Status::ServiceMissing, copy::LOCK_SERVICE_MISSING),
        ] {
            let mut s = scene();
            s.status = status;
            assert_eq!(status_lines(&s)[0].0, word);
        }
        for quiet in [Status::Idle, Status::Checking] {
            let mut s = scene();
            s.status = quiet;
            assert!(status_lines(&s).is_empty());
        }
    }

    #[test]
    fn the_worst_status_fits_the_prompt_rectangle() {
        // Three long lines of account text, the wait, the hint, Caps Lock and
        // no keyboard at once: still inside the rectangle, at 1.5.
        let mut fonts = Fonts::embedded().expect("font");
        let mut s = scene();
        s.status = Status::ServiceMissing;
        s.wait_secs = Some(30);
        s.faillock_hint = true;
        s.caps_lock = true;
        s.keyboard = false;
        let s_px = 1.5_f32;
        let text_w = (PANEL_W - 2.0 * STATUS_PAD) * s_px;
        let lines = status_lines(&s);
        let body: f32 = lines
            .iter()
            .map(|(t, size)| fonts.measure(t, size * s_px, Some(text_w)).height)
            .sum::<f32>()
            + LINE_GAP * s_px * (lines.len() - 1) as f32;
        let bottom =
            (FIELD_TOP + size::CONTROL_HEIGHT_XL + GAP) * s_px + body + 2.0 * STATUS_PAD * s_px;
        assert!(bottom <= RECT_H * s_px, "{bottom} > {}", RECT_H * s_px);
    }

    #[test]
    fn a_fresh_buffer_after_a_resize_is_damaged_whole() {
        let mut painter = painter();
        let mut screen = two_outputs();
        painter.render_to(&scene(), &mut screen);
        let first = screen.outputs[&OutputId(1)].damage[0];
        assert_eq!(
            first,
            PixelRect {
                x: 0,
                y: 0,
                w: PHYSICAL.0 as i32,
                h: PHYSICAL.1 as i32
            },
            "a buffer never shown before is sent whole"
        );
    }

    #[test]
    fn t_t1_field_and_status_text_tokens_over_the_scrim_composited_over_pure_white_reach_4_5_to_1()
    {
        let white = Rgb(255.0, 255.0, 255.0);
        let over_scrim = composite(SCRIM, white);
        // The status text lies on the scrim (and on the veil, which only
        // darkens it): the worst case is the scrim over pure white.
        let ratio = contrast(Rgb::of(STATUS_INK), over_scrim);
        assert!(
            ratio >= 4.5,
            "status text over the scrim over white: {ratio:.2}"
        );
        let on_veil = contrast(Rgb::of(STATUS_INK), composite(VEIL, over_scrim));
        assert!(on_veil >= ratio, "the veil never lowers it: {on_veil:.2}");
        // The placeholder and the checking word sit on the field's opaque
        // fill, not on the backdrop; the bullets too.
        let field = Rgb::of(FIELD_FILL);
        let hint = contrast(Rgb::of(FIELD_HINT_INK), field);
        assert!(hint >= 4.5, "placeholder over the field fill: {hint:.2}");
        let bullet = contrast(Rgb::of(DARK.elevated_ink), field);
        assert!(bullet >= 4.5, "bullets over the field fill: {bullet:.2}");
        // The existing modal scrim token is *not* enough over white, which is
        // why the lock has its own.
        let modal = contrast(Rgb::of(STATUS_INK), composite(DARK.scrim, white));
        assert!(modal < 4.5, "the modal scrim alone: {modal:.2}");
    }

    #[derive(Clone, Copy)]
    struct Rgb(f32, f32, f32);

    impl Rgb {
        fn of(token: Rgba) -> Self {
            Self(f32::from(token.r), f32::from(token.g), f32::from(token.b))
        }
    }

    /// `top` (straight alpha) over an opaque `under`.
    fn composite(top: Rgba, under: Rgb) -> Rgb {
        let mix = |t: u8, u: f32| f32::from(t) * top.a + u * (1.0 - top.a);
        Rgb(
            mix(top.r, under.0),
            mix(top.g, under.1),
            mix(top.b, under.2),
        )
    }

    fn luminance(c: Rgb) -> f32 {
        let lin = |v: f32| {
            let v = v / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c.0) + 0.7152 * lin(c.1) + 0.0722 * lin(c.2)
    }

    /// WCAG contrast ratio.
    fn contrast(a: Rgb, b: Rgb) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn the_prompt_rectangle_is_centred_clamped_and_never_empty_on_a_real_output() {
        let rect = prompt_rect(LOGICAL, SCALE, PHYSICAL).expect("rect");
        let centre = rect.x + rect.w / 2;
        assert!(
            (centre - PHYSICAL.0 as i32 / 2).abs() <= 2,
            "centred: {rect:?}"
        );
        assert!(rect.y > 0 && rect.y + rect.h <= PHYSICAL.1 as i32);
        // A tiny output still gets a rectangle inside it.
        let small = prompt_rect((200, 100), 1.0, (200, 100)).expect("rect");
        assert!(small.x >= 0 && small.x + small.w <= 200 && small.y + small.h <= 100);
        assert_eq!(prompt_rect((0, 0), 1.0, (0, 0)), None);
    }

    /// The accented vowels, the diaeresis u, the tilde n and the inverted
    /// marks, lower and upper case.
    const SPANISH_CODE_POINTS: [u32; 15] = [
        0xe1, 0xe9, 0xed, 0xf3, 0xfa, 0xfc, 0xf1, 0xc1, 0xc9, 0xcd, 0xd3, 0xda, 0xd1, 0xbf, 0xa1,
    ];

    /// Whether `text` holds an accented vowel, a tilde n or an inverted mark,
    /// written out or as a `\u{..}` escape (round B7, M2). The escapes are
    /// built from the code points so this scanner does not contain what it
    /// looks for.
    fn holds_spanish(text: &str) -> bool {
        let letters = SPANISH_CODE_POINTS.map(|cp| char::from_u32(cp).unwrap_or('?'));
        if text.chars().any(|c| letters.contains(&c)) {
            return true;
        }
        // Leading zeros in the braces are the same escape.
        let squeezed = text
            .to_lowercase()
            .replace("\\u{00", "\\u{0")
            .replace("\\u{0", "\\u{");
        SPANISH_CODE_POINTS
            .iter()
            .any(|cp| squeezed.contains(&format!("\\u{{{cp:x}}}")))
    }

    #[test]
    fn the_spanish_scanner_sees_letters_and_escapes_alike() {
        let escape = |cp: u32, pad: &str| format!("\"s\\u{{{pad}{cp:x}}}bado\"");
        for cp in SPANISH_CODE_POINTS {
            assert!(holds_spanish(&escape(cp, "")), "{cp:x}");
            assert!(holds_spanish(&escape(cp, "00")), "{cp:x} padded");
            assert!(
                holds_spanish(&escape(cp, "").to_uppercase()),
                "{cp:x} upper"
            );
            let letter = char::from_u32(cp).unwrap_or('?');
            assert!(holds_spanish(&format!("s{letter}bado")), "{cp:x} letter");
        }
        assert!(!holds_spanish("saturday \\u{41} \\n \\u{2014}"));
    }

    #[test]
    fn only_the_copy_file_holds_spanish_and_its_constants_exist() {
        for word in [
            copy::LOCK_FIELD,
            copy::LOCK_CHECKING,
            copy::LOCK_REFUSED,
            copy::LOCK_UNAVAILABLE,
            copy::LOCK_ACCOUNT,
            copy::LOCK_UNSUPPORTED,
            copy::LOCK_SERVICE_MISSING,
            copy::LOCK_FAILLOCK_HINT,
            copy::LOCK_CAPS,
            copy::LOCK_NO_KEYBOARD,
            copy::LOCK_REHEARSAL,
        ] {
            assert!(!word.is_empty());
        }
        assert!(copy::lock_wait(7).contains('7'));
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("src") {
            let path = entry.expect("entry").path();
            if path.extension().is_none_or(|ext| ext != "rs") || path.ends_with("copy.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("source");
            // A file that types international input says so in its head.
            if text
                .lines()
                .take(3)
                .any(|l| l.contains("allow-non-english"))
            {
                continue;
            }
            if holds_spanish(&text) {
                offenders.push(path.display().to_string());
            }
        }
        assert!(
            offenders.is_empty(),
            "Spanish outside copy.rs: {offenders:?}"
        );
        let head = std::fs::read_to_string(dir.join("copy.rs")).expect("copy.rs");
        assert!(head.starts_with("// language-contract: product-copy"));
    }

    // ---- the dimmed backdrop (CEL-2-H-b) ----

    use crate::backdrop::fixtures::{bundle_of, memfd, solid};
    use crate::backdrop::{read_bundle, READ_DEADLINE};
    use std::os::fd::AsFd;
    use std::time::Duration;

    /// Two small outputs at scale 1 so a base is cheap to compute.
    const SMALL: (u32, u32) = (800, 450);
    const OTHER: (u32, u32) = (640, 360);

    fn small_scene() -> Scene {
        let mut scene = scene();
        scene.outputs = vec![
            SceneOutput {
                id: OutputId(1),
                name: "DP-1".to_owned(),
                size: Some(SMALL),
                scale: 1.0,
            },
            SceneOutput {
                id: OutputId(2),
                name: "DP-2".to_owned(),
                size: Some(OTHER),
                scale: 1.0,
            },
        ];
        // Three bullets: ink on the field whatever is behind the prompt.
        scene.field_chars = 3;
        scene
    }

    fn small_screen() -> Screen {
        Screen::new(&[(1, SMALL, 1.0), (2, OTHER, 1.0)])
    }

    fn painter_over(blocks: Vec<celestina_lock_proto::OutputBlock>) -> CpuPresenter {
        let backdrop = Backdrop::spawn(blocks, SCRIM);
        let painter = CpuPresenter::with_backdrop(backdrop);
        assert!(painter.can_draw());
        painter
    }

    /// Asks for the bases, waits for the worker, and draws until both of an
    /// output's buffers show them.
    fn settle(painter: &mut CpuPresenter, scene: &Scene, screen: &mut Screen) {
        painter.render_to(scene, screen);
        if let Some(backdrop) = painter.backdrop.as_mut() {
            assert!(
                backdrop.settle(Duration::from_secs(20)),
                "the worker answers"
            );
        }
        painter.render_to(scene, screen);
        painter.render_to(scene, screen);
    }

    /// The buffer on screen: the one drawn last.
    fn on_screen(fake: &Fake) -> &[u8] {
        &fake.buffers[(fake.next + 1) % 2]
    }

    fn pixel(fake: &Fake, x: u32, y: u32) -> [u8; 3] {
        let i = (y as usize * fake.size.0 as usize + x as usize) * 4;
        let px = on_screen(fake);
        [px[i], px[i + 1], px[i + 2]]
    }

    fn rect_of(size: (u32, u32)) -> PixelRect {
        prompt_rect(size, 1.0, size).expect("rect")
    }

    #[test]
    fn the_backdrop_shows_dimmed_everywhere_and_the_prompt_sits_over_it() {
        let mut painter = painter_over(vec![
            solid("DP-1", 16, 9, [255, 255, 255]),
            solid("DP-2", 16, 9, [255, 255, 255]),
        ]);
        let mut screen = small_screen();
        let scene = small_scene();
        settle(&mut painter, &scene, &mut screen);
        // White picture, black scrim at 60 %: 102 on both outputs, the prompt
        // output included outside its text.
        for (id, size) in [(1, SMALL), (2, OTHER)] {
            let fake = &screen.outputs[&OutputId(id)];
            assert_eq!(pixel(fake, 0, 0), [102, 102, 102], "output {id}");
            assert_eq!(
                pixel(fake, size.0 - 1, size.1 - 1),
                [102, 102, 102],
                "output {id}"
            );
        }
        let focused = &screen.outputs[&OutputId(1)];
        let rect = rect_of(SMALL);
        assert_eq!(
            pixel(focused, rect.x as u32 + 1, rect.y as u32 + 1),
            [102, 102, 102]
        );
        assert!(
            bright(focused, (focused.next + 1) % 2) > 0,
            "the prompt is drawn"
        );
        let other = &screen.outputs[&OutputId(2)];
        assert_eq!(bright(other, (other.next + 1) % 2), 0, "no prompt there");
    }

    #[test]
    fn outputs_without_a_block_stay_black_with_the_scrim() {
        // Only DP-2 has a picture; the prompt is on DP-1.
        let mut painter = painter_over(vec![solid("DP-2", 16, 9, [255, 255, 255])]);
        let mut screen = small_screen();
        let scene = small_scene();
        settle(&mut painter, &scene, &mut screen);
        let pictured = &screen.outputs[&OutputId(2)];
        assert_eq!(pixel(pictured, 5, 5), [102, 102, 102]);
        let blank = &screen.outputs[&OutputId(1)];
        // Black with the scrim over it is black: the whole output, the prompt's
        // own background included, apart from the ink.
        assert_eq!(pixel(blank, 0, 0), [0, 0, 0]);
        assert_eq!(pixel(blank, SMALL.0 - 1, SMALL.1 - 1), [0, 0, 0]);
        let rect = rect_of(SMALL);
        assert_eq!(
            pixel(blank, rect.x as u32 + 1, rect.y as u32 + 1),
            [0, 0, 0]
        );
        assert!(
            bright(blank, (blank.next + 1) % 2) > 0,
            "the prompt is there"
        );
        // The worker was never asked about the output without a picture.
        assert_eq!(painter.backdrop.as_ref().map(Backdrop::computed), Some(1));
        // And with no backdrop at all every output is black under the prompt.
        let mut bare = CpuPresenter::new();
        let mut screen = small_screen();
        bare.render_to(&scene, &mut screen);
        assert_eq!(pixel(&screen.outputs[&OutputId(1)], 0, 0), [0, 0, 0]);
        assert_eq!(
            screen.outputs[&OutputId(2)].damage.len(),
            0,
            "the cover says it"
        );
    }

    #[test]
    fn only_the_damaged_rectangle_is_redrawn_over_the_cached_base() {
        let mut painter = painter_over(vec![solid("DP-1", 16, 9, [255, 255, 255])]);
        let mut screen = small_screen();
        let mut scene = small_scene();
        settle(&mut painter, &scene, &mut screen);
        assert_eq!(
            painter.backdrop.as_ref().map(Backdrop::computed),
            Some(1),
            "the upscale ran once, not once per frame"
        );
        let rect = rect_of(SMALL);
        // Mark the buffer the next frame goes to, outside the prompt's
        // rectangle: only a full redraw could erase the mark.
        let next = screen.outputs[&OutputId(1)].next;
        let mark = (3 * SMALL.0 as usize + 3) * 4;
        screen.outputs.get_mut(&OutputId(1)).expect("fake").buffers[next][mark..mark + 4]
            .copy_from_slice(&[1, 2, 3, 0xff]);
        let before = screen.outputs[&OutputId(1)].damage.len();
        scene.field_chars = 3;
        painter.render_to(&scene, &mut screen);
        let fake = &screen.outputs[&OutputId(1)];
        assert_eq!(fake.damage.len(), before + 1);
        let damage = fake.damage.last().copied().expect("damage");
        assert_eq!(damage, rect, "a key press damages the prompt rectangle");
        assert_eq!(
            &fake.buffers[next][mark..mark + 4],
            &[1, 2, 3, 0xff],
            "the base outside the rectangle was not copied again"
        );
        // Inside the rectangle the base is back behind the new prompt.
        assert_eq!(
            pixel(fake, rect.x as u32 + 1, rect.y as u32 + 1),
            [102, 102, 102]
        );
        assert_eq!(painter.backdrop.as_ref().map(Backdrop::computed), Some(1));
        // The next key goes to the other buffer, which was marked by nobody:
        // it is likewise a rectangle, not an output.
        scene.field_chars = 4;
        painter.render_to(&scene, &mut screen);
        let damage = screen.outputs[&OutputId(1)]
            .damage
            .last()
            .copied()
            .expect("damage");
        assert_eq!(damage, rect);
    }

    #[test]
    fn a_base_arriving_late_replaces_the_black_in_both_buffers() {
        let mut painter = painter_over(vec![solid("DP-1", 16, 9, [255, 255, 255])]);
        let mut screen = small_screen();
        let scene = small_scene();
        // First frame: the worker has not answered, the output is black with
        // its prompt.
        painter.render_to(&scene, &mut screen);
        assert_eq!(pixel(&screen.outputs[&OutputId(1)], 0, 0), [0, 0, 0]);
        let frames = screen.frames;
        assert!(painter
            .backdrop
            .as_mut()
            .expect("worker")
            .settle(Duration::from_secs(20)));
        // The first buffer to show it is damaged whole; the second already
        // matches what is on screen outside the prompt, so only the prompt's
        // rectangle is.
        painter.render_to(&scene, &mut screen);
        let fake = &screen.outputs[&OutputId(1)];
        assert_eq!(pixel(fake, 0, 0), [102, 102, 102]);
        let whole = fake.damage.last().copied().expect("damage");
        assert_eq!((whole.w, whole.h), (SMALL.0 as i32, SMALL.1 as i32));
        painter.render_to(&scene, &mut screen);
        let fake = &screen.outputs[&OutputId(1)];
        assert_eq!(pixel(fake, 0, 0), [102, 102, 102]);
        assert_eq!(fake.damage.last().copied(), Some(rect_of(SMALL)));
        assert_eq!(screen.frames, frames + 2);
    }

    #[test]
    fn the_fade_blacks_the_whole_output_and_waking_restores_the_base() {
        let mut painter = painter_over(vec![solid("DP-1", 16, 9, [255, 255, 255])]);
        let mut screen = small_screen();
        let mut scene = small_scene();
        settle(&mut painter, &scene, &mut screen);
        scene.faded = true;
        for _ in 0..2 {
            painter.render_to(&scene, &mut screen);
            let fake = &screen.outputs[&OutputId(1)];
            assert_eq!(
                non_black(fake, (fake.next + 1) % 2),
                0,
                "faded is all black"
            );
            let whole = fake.damage.last().copied().expect("damage");
            assert_eq!((whole.w, whole.h), (SMALL.0 as i32, SMALL.1 as i32));
        }
        scene.faded = false;
        for _ in 0..2 {
            painter.render_to(&scene, &mut screen);
            assert_eq!(pixel(&screen.outputs[&OutputId(1)], 0, 0), [102, 102, 102]);
        }
        assert_eq!(painter.backdrop.as_ref().map(Backdrop::computed), Some(1));
    }

    #[test]
    fn a_resized_output_gets_a_base_of_its_new_size() {
        let mut painter = painter_over(vec![solid("DP-1", 16, 9, [255, 255, 255])]);
        let mut screen = small_screen();
        let mut scene = small_scene();
        settle(&mut painter, &scene, &mut screen);
        // The output becomes smaller: new fake buffers, new logical size.
        screen = Screen::new(&[(1, OTHER, 1.0), (2, OTHER, 1.0)]);
        scene.outputs[0].size = Some(OTHER);
        settle(&mut painter, &scene, &mut screen);
        assert_eq!(pixel(&screen.outputs[&OutputId(1)], 0, 0), [102, 102, 102]);
        assert_eq!(painter.backdrop.as_ref().map(Backdrop::computed), Some(2));
    }

    #[test]
    fn a_wrong_size_guess_corrects_itself_without_another_redraw() {
        // The scene says 800x450 at scale 1, the compositor's buffer is
        // 640x360: the guess is wrong, the first frame is black, and the base
        // of the real size is asked for at once.
        let mut painter = painter_over(vec![solid("DP-1", 16, 9, [255, 255, 255])]);
        let mut screen = Screen::new(&[(1, OTHER, 1.0), (2, OTHER, 1.0)]);
        let scene = small_scene();
        painter.render_to(&scene, &mut screen);
        assert_eq!(pixel(&screen.outputs[&OutputId(1)], 0, 0), [0, 0, 0]);
        assert!(painter
            .backdrop
            .as_mut()
            .expect("worker")
            .settle(Duration::from_secs(20)));
        painter.render_to(&scene, &mut screen);
        assert_eq!(pixel(&screen.outputs[&OutputId(1)], 0, 0), [102, 102, 102]);
    }

    #[test]
    fn an_invalid_or_slow_bundle_keeps_black_and_the_prompt() {
        let good = bundle_of(vec![solid("DP-1", 16, 9, [255, 255, 255])]);
        let unsealed = memfd(&good, false);
        let sealed = memfd(&good, true);
        let mut garbage = good.clone();
        garbage[0] = b'X';
        let bad_magic = memfd(&garbage, true);
        let (reader, _writer) = std::io::pipe().expect("pipe");
        let loads = [
            read_bundle(unsealed.as_fd(), READ_DEADLINE),
            read_bundle(bad_magic.as_fd(), READ_DEADLINE),
            // Slow: the deadline has already passed.
            read_bundle(sealed.as_fd(), Duration::ZERO),
            read_bundle(reader.as_fd(), READ_DEADLINE),
        ];
        for loaded in loads {
            let mut painter =
                CpuPresenter::with_backdrop(Backdrop::spawn(loaded.into_blocks(), SCRIM));
            assert!(painter.backdrop.is_none(), "no picture, no worker");
            let mut screen = small_screen();
            painter.render_to(&small_scene(), &mut screen);
            let fake = &screen.outputs[&OutputId(1)];
            assert_eq!(pixel(fake, 0, 0), [0, 0, 0], "black");
            assert!(
                bright(fake, (fake.next + 1) % 2) > 0,
                "and the prompt is there"
            );
            assert_eq!(screen.outputs[&OutputId(2)].damage.len(), 0);
        }
    }

    #[test]
    fn t_t1_field_and_status_text_over_the_scrim_composited_over_pure_white_still_reach_4_5_to_1_over_a_white_backdrop(
    ) {
        // The brightest possible picture, drawn for real: the contrast is
        // measured on the pixels the painter put on screen, not only on the
        // tokens.
        let mut painter = painter_over(vec![solid("DP-1", 16, 9, [255, 255, 255])]);
        let mut screen = small_screen();
        let mut scene = small_scene();
        scene.status = Status::Refused;
        settle(&mut painter, &scene, &mut screen);
        let fake = &screen.outputs[&OutputId(1)];
        let rect = rect_of(SMALL);

        // The base: no channel of any pixel outside the prompt is brighter
        // than the scrim lets white be.
        let ceiling = composite(SCRIM, Rgb(255.0, 255.0, 255.0));
        for (i, px) in on_screen(fake).as_chunks::<4>().0.iter().enumerate() {
            let (x, y) = ((i as u32) % SMALL.0, (i as u32) / SMALL.0);
            let inside = x as i32 >= rect.x
                && (x as i32) < rect.x + rect.w
                && y as i32 >= rect.y
                && (y as i32) < rect.y + rect.h;
            if !inside {
                assert!(f32::from(px[0]) <= ceiling.0 + 1.0, "({x},{y}) {px:?}");
            }
        }
        // The text sits on the scrim over white...
        let drawn = pixel(fake, rect.x as u32 + 1, rect.y as u32 + 1);
        let under_text = Rgb(
            f32::from(drawn[2]),
            f32::from(drawn[1]),
            f32::from(drawn[0]),
        );
        assert!(contrast(Rgb::of(STATUS_INK), under_text) >= 4.5);
        // ...and on the veil of the status capsule, which only darkens it.
        let sx = rect.x as u32 + PAD as u32 + 4;
        let sy = rect.y as u32 + (FIELD_TOP + size::CONTROL_HEIGHT_XL + GAP) as u32 + 12;
        let veiled = pixel(fake, sx, sy);
        let under_status = Rgb(
            f32::from(veiled[2]),
            f32::from(veiled[1]),
            f32::from(veiled[0]),
        );
        let expected = composite(VEIL, under_text);
        assert!(
            (under_status.0 - expected.0).abs() <= 2.0,
            "veil over the scrim: {veiled:?}"
        );
        assert!(contrast(Rgb::of(STATUS_INK), under_status) >= 4.5);
        // The field's own text sits on its opaque fill, picture or no picture.
        assert!(contrast(Rgb::of(FIELD_HINT_INK), Rgb::of(FIELD_FILL)) >= 4.5);
    }

    // The clock and the date (CEL-2-H-b, Task B3): what the frame draws,
    // where, and how often.
    mod clock_face {
        // The clock and the date on the lock screen (CEL-2-H-b, Task B3): what the
        // frame draws, where, and how often. A child of the view's `tests` module,
        // which owns the fake compositor these tests draw on.

        use super::*;
        use std::ops::Range;
        use std::time::{Duration, Instant, UNIX_EPOCH};

        use celestina_wayland::{InputEvent, Modifiers, OutputInfo};

        use crate::machine::{Action, Config, Input, Machine, State};
        use crate::verify::{Denial, Outcome};

        /// 2026-10-23 21:30:15 UTC, a Friday.
        fn friday_evening() -> SystemTime {
            UNIX_EPOCH + Duration::from_secs(1_792_791_015)
        }

        fn with_clock(offset: Option<i32>) -> CpuPresenter {
            let mut painter = painter().with_utc_offset(offset);
            painter.now = friday_evening;
            painter
        }

        /// Pixels of bright ink on the rows `rows` (relative to the prompt rectangle's
        /// top) of the frame on screen, across the whole width of the rectangle.
        fn ink(fake: &Fake, rect: PixelRect, rows: Range<f32>) -> usize {
            let px = on_screen(fake);
            let (top, bottom) = (rows.start.floor() as i32, rows.end.ceil() as i32);
            let mut count = 0;
            for y in rect.y + top..(rect.y + bottom).min(rect.y + rect.h) {
                for x in rect.x..rect.x + rect.w {
                    let at = (y as usize * fake.size.0 as usize + x as usize) * 4;
                    if px[at] > 200 && px[at + 1] > 200 && px[at + 2] > 200 {
                        count += 1;
                    }
                }
            }
            count
        }

        #[test]
        fn the_clock_is_formatted_from_the_bundle_offset_at_each_frame() {
            let lines = clock_lines(friday_evening(), Some(7_200));
            assert_eq!(lines.time.as_deref(), Some("23:30"));
            assert_eq!(lines.date, copy::lock_date(5, 23, 10));
            // Across midnight the date follows the offset, not UTC.
            let lines = clock_lines(friday_evening(), Some(9 * 3_600));
            assert_eq!(lines.time.as_deref(), Some("06:30"));
            assert_eq!(lines.date, copy::lock_date(6, 24, 10));
        }

        #[test]
        fn offset_known_0_hides_the_clock_and_shows_the_date() {
            let lines = clock_lines(friday_evening(), None);
            assert_eq!(lines.time, None, "no clock rather than a wrong hour");
            assert_eq!(lines.date, copy::lock_date(5, 23, 10));

            let size = SMALL;
            let rect = prompt_rect(size, 1.0, size).expect("rect");
            let rows = |a: f32, b: f32| a..b;
            let (clock_rows, date_rows) =
                (rows(0.0, CLOCK_LINE_H), rows(CLOCK_LINE_H, CLOCK_BLOCK));
            let draw = |offset| {
                let mut painter = with_clock(offset);
                let mut screen = small_screen();
                painter.render_to(&small_scene(), &mut screen);
                let fake = &screen.outputs[&OutputId(1)];
                (
                    ink(fake, rect, clock_rows.clone()),
                    ink(fake, rect, date_rows.clone()),
                )
            };
            let (known_clock, known_date) = draw(Some(0));
            let (unknown_clock, unknown_date) = draw(None);
            assert!(known_clock > 0, "a known offset draws the clock");
            assert!(known_date > 0, "and the date");
            assert_eq!(unknown_clock, 0, "an unknown offset draws no clock");
            assert!(unknown_date > 0, "but still the date");
            // The field is where it always was: the prompt is not rearranged.
            let mut painter = with_clock(None);
            let mut screen = small_screen();
            painter.render_to(&small_scene(), &mut screen);
            let fake = &screen.outputs[&OutputId(1)];
            assert!(
                ink(fake, rect, FIELD_TOP..FIELD_TOP + size::CONTROL_HEIGHT_XL) > 0,
                "the three bullets of the field remain"
            );
        }

        #[test]
        fn the_clock_and_date_are_drawn_only_on_the_output_with_the_prompt() {
            let mut painter = with_clock(Some(0));
            let mut screen = small_screen();
            painter.render_to(&small_scene(), &mut screen);
            let first = &screen.outputs[&OutputId(1)];
            let rect = prompt_rect(SMALL, 1.0, SMALL).expect("rect");
            assert!(ink(first, rect, 0.0..CLOCK_BLOCK) > 0);
            // The other output has no frame at all: zero content of the person's,
            // and no time either.
            let other = &screen.outputs[&OutputId(2)];
            assert!(other.damage.is_empty());
            assert_eq!(non_black(other, 0) + non_black(other, 1), 0);
            // Faded, the clock goes with the rest.
            let mut faded = small_scene();
            faded.faded = true;
            painter.render_to(&faded, &mut screen);
            let first = &screen.outputs[&OutputId(1)];
            assert_eq!(ink(first, rect, 0.0..CLOCK_BLOCK), 0);
        }

        #[test]
        fn the_clock_tick_redraws_only_the_prompt_rectangle() {
            let mut painter = with_clock(Some(0));
            let mut screen = small_screen();
            let scene = small_scene();
            painter.render_to(&scene, &mut screen);
            // A later minute: the same scene, a new time, one frame of the same
            // rectangle.
            painter.now = || friday_evening() + Duration::from_secs(60);
            painter.render_to(&scene, &mut screen);
            painter.render_to(&scene, &mut screen);
            let rect = prompt_rect(SMALL, 1.0, SMALL).expect("rect");
            let fake = &screen.outputs[&OutputId(1)];
            assert_eq!(fake.damage.last().copied(), Some(rect));
            assert_eq!(screen.frames, 3);
        }

        /// The machine and the painter together, with the time it takes to arm.
        struct Lock {
            machine: Machine,
            painter: CpuPresenter,
            screen: Screen,
            now: Instant,
            /// Every deadline the machine armed, in order.
            armed: Vec<Instant>,
        }

        impl Lock {
            fn new(cfg: Config) -> Self {
                let now = Instant::now();
                let mut lock = Self {
                    machine: Machine::new(cfg, now),
                    painter: with_clock(Some(0)),
                    screen: Screen::new(&[(1, SMALL, 1.0)]),
                    now,
                    armed: Vec::new(),
                };
                lock.feed(Input::Keyboard { present: true });
                lock.feed(Input::OutputAdded(OutputInfo {
                    id: OutputId(1),
                    name: "DP-1".to_owned(),
                    logical_size: SMALL,
                    scale: 1.0,
                    refresh_mhz: 60_000,
                }));
                lock.feed(Input::SurfaceConfigured {
                    output: OutputId(1),
                    size: SMALL,
                });
                lock.feed(Input::Connected);
                lock.feed(Input::Locked);
                lock
            }

            fn feed(&mut self, input: Input) -> Vec<Action> {
                let actions = self.machine.handle(input, self.now);
                for action in &actions {
                    match action {
                        Action::Render(scene) => self.painter.render_to(scene, &mut self.screen),
                        Action::Arm(Some(at)) => self.armed.push(*at),
                        _ => {}
                    }
                }
                actions
            }

            fn advance(&mut self, by: Duration) {
                self.now += by;
            }

            fn key(&mut self, keysym: u32, text: Option<&str>) {
                self.feed(Input::Input {
                    output: OutputId(1),
                    event: InputEvent::Key {
                        keysym,
                        utf8: text.map(str::to_owned),
                        modifiers: Modifiers::default(),
                        pressed: true,
                        serial: 0,
                    },
                });
            }
        }

        #[test]
        fn at_rest_one_frame_a_minute_and_one_per_second_only_during_a_countdown() {
            const HOUR: Duration = Duration::from_secs(3_600);
            let mut cfg = Config::new(false, false, 3);
            // No fade inside this test: the cadence is of an awake lock.
            cfg.fade_after = 10 * HOUR;
            let mut lock = Lock::new(cfg);
            let started = lock.now;
            assert_eq!(lock.screen.frames, 1, "the prompt, once");

            // At rest: the machine arms nothing but its fade, so the only thing that
            // wakes the screen is the clock's minute, one frame each.
            for minute in 1..=5 {
                lock.advance(Duration::from_secs(60));
                lock.feed(Input::Redraw);
                assert_eq!(lock.screen.frames, 1 + minute, "minute {minute}");
            }
            assert!(
                lock.armed.iter().all(|at| *at >= started + 10 * HOUR),
                "no deadline shorter than the fade at rest: {:?}",
                lock.armed
            );

            // A refused attempt starts a delay: a frame per second while it counts,
            // each armed one second ahead; the last second ends it.
            lock.key(u32::from('x'), Some("x"));
            lock.key(0xff0d, Some("\r"));
            lock.feed(Input::Verdict {
                attempt: 1,
                outcome: Outcome::Denied(Denial::Refused),
            });
            let wait = lock
                .machine
                .scene(lock.now)
                .wait_secs
                .expect("a delay runs");
            assert_eq!(wait, 16, "three prior failures and this one");
            let before = lock.screen.frames;
            for second in 1..=u64::from(wait) {
                lock.advance(Duration::from_secs(1));
                let actions = lock.feed(Input::Tick);
                assert_eq!(
                    lock.screen.frames,
                    before + usize::try_from(second).expect("small"),
                    "second {second} of the delay"
                );
                if second < u64::from(wait) {
                    assert!(
                    actions
                        .iter()
                        .any(|a| matches!(a, Action::Arm(Some(at)) if *at == lock.now + Duration::from_secs(1))),
                    "the next wake is a second away"
                );
                }
            }
            assert_eq!(lock.machine.scene(lock.now).wait_secs, None);
            // The delay over, a tick that changes nothing draws nothing, and the
            // minute is the only thing left.
            let quiet = lock.screen.frames;
            lock.advance(Duration::from_secs(5));
            lock.feed(Input::Tick);
            assert_eq!(lock.screen.frames, quiet);
            lock.advance(Duration::from_secs(60));
            lock.feed(Input::Redraw);
            assert_eq!(lock.screen.frames, quiet + 1);
        }

        #[test]
        fn the_machine_transitions_are_unchanged_by_the_clock() {
            // The clock reaches the machine as `Input::Redraw` and nothing else: the
            // transitions of design §4.5 (T-M1, in machine.rs, edited by nobody)
            // never see a new input. A redraw in any state leaves the state alone
            // and asks for a frame of the same scene or for nothing.
            let mut lock = Lock::new(Config::new(false, false, 0));
            assert_eq!(lock.machine.state(), State::Prompt);
            let actions = lock.feed(Input::Redraw);
            assert_eq!(lock.machine.state(), State::Prompt);
            assert!(
                actions.iter().all(|a| matches!(a, Action::Render(_))),
                "{actions:?}"
            );
            lock.key(u32::from('x'), Some("x"));
            lock.key(0xff0d, Some("\r"));
            let State::Verifying { attempt, .. } = lock.machine.state() else {
                panic!("an attempt runs: {:?}", lock.machine.state());
            };
            let actions = lock.feed(Input::Redraw);
            assert!(matches!(
                lock.machine.state(),
                State::Verifying { attempt: a, .. } if a == attempt
            ));
            assert!(
                actions.iter().all(|a| matches!(a, Action::Render(_))),
                "a redraw while checking starts, reports and unlocks nothing: {actions:?}"
            );
            // Before `locked` the clock draws nothing either (the covers stay).
            let mut early = Machine::new(Config::new(false, false, 0), Instant::now());
            assert!(early
                .handle(Input::Redraw, Instant::now())
                .iter()
                .all(|a| !matches!(a, Action::Render(_))));
            // And the shell's glue is one line: the tick is a redraw.
            let app = include_str!("app.rs");
            assert!(
                app.contains("Message::ClockTick => self.feed(Input::Redraw, ctl)"),
                "the clock tick must map to a redraw and nothing else"
            );
            // The machine's source mentions no clock at all.
            let machine = include_str!("machine.rs");
            let production = super::super::test_gates::without_test_blocks(machine);
            assert!(!production.contains("SystemTime") && !production.contains("ClockTick"));
        }

        #[test]
        fn t_t1_the_large_clock_text_over_the_scrim_reaches_3_to_1() {
            let white = Rgb(255.0, 255.0, 255.0);
            let over_scrim = composite(SCRIM, white);
            // The clock is large text by the WCAG measure (24 px or more), which asks
            // for 3:1; the date is normal text and keeps 4.5:1. Both lie on the scrim
            // alone (no capsule), so the worst case is the scrim over pure white.
            let clock_size = std::hint::black_box(CLOCK_SIZE);
            assert!(clock_size >= 24.0, "{clock_size}");
            // The block's measures follow the painter's line height rule.
            assert_eq!(CLOCK_LINE_H, (CLOCK_SIZE * 1.3).round());
            assert_eq!(DATE_LINE_H, (type_scale::TITLE * 1.3).round());
            let clock = contrast(Rgb::of(CLOCK_INK), over_scrim);
            assert!(clock >= 3.0, "clock over the scrim over white: {clock:.2}");
            let date = contrast(Rgb::of(DATE_INK), over_scrim);
            assert!(date >= 4.5, "date over the scrim over white: {date:.2}");
        }

        #[test]
        fn the_date_names_are_product_copy_and_complete() {
            assert_eq!(copy::LOCK_WEEKDAYS.len(), 7);
            assert_eq!(copy::LOCK_MONTHS.len(), 12);
            assert!(copy::LOCK_WEEKDAYS
                .iter()
                .chain(copy::LOCK_MONTHS.iter())
                .all(|name| !name.is_empty() && *name == name.to_lowercase()));
            // The words come from the two tables, in the order of the line.
            let first = copy::lock_date(0, 1, 1);
            assert!(first.starts_with(copy::LOCK_WEEKDAYS[0]), "{first}");
            assert!(first.ends_with(copy::LOCK_MONTHS[0]), "{first}");
            let last = copy::lock_date(6, 31, 12);
            assert!(last.starts_with(copy::LOCK_WEEKDAYS[6]), "{last}");
            assert!(last.ends_with(copy::LOCK_MONTHS[11]), "{last}");
            // An index the clock never produces degrades, it does not panic.
            let no_weekday = copy::lock_date(9, 3, 4);
            assert!(no_weekday.starts_with('3'), "{no_weekday}");
            assert!(no_weekday.ends_with(copy::LOCK_MONTHS[3]), "{no_weekday}");
            assert!(!copy::LOCK_WEEKDAYS.iter().any(|d| no_weekday.contains(d)));
            assert_eq!(copy::lock_date(1, 3, 0), "3");
            assert_eq!(copy::lock_date(1, 3, 13), "3");
        }
    }
}
