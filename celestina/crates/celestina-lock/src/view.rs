//! What the lock screen looks like and how it is put on the outputs (design
//! §4.6, §4.6.1, F16, F20).
//!
//! The view takes only the machine's [`Scene`]: how many characters were typed
//! (never which), the status, the wait, the Caps Lock and keyboard flags. It
//! knows no user name, no notification, no media, no window title and no
//! clock yet (the clock and the dimmed wallpaper are CEL-2-H-b).
//!
//! * Every output is black, the scrim over a black backdrop. The prompt
//!   (banner, field capsule, status capsule) is drawn on the one output the
//!   machine names, and on no other. When the screen is faded nothing is
//!   drawn anywhere: black, the prompt included.
//! * One rectangle holds the whole prompt, always the same for an output. A
//!   key press repaints that rectangle into one of the output's two buffers
//!   and damages only it (plus the rectangle of the frame shown before), so a
//!   4K output costs a few hundred thousand pixels per press, not 8 million.
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

use celestina_tokens::{radius, size, type_scale, Rgba, DARK};
use celestina_wayland::lock::{FrameBuf, LockCtl, PresentError};
use celestina_wayland::{OutputId, PixelRect};
use cosmic_text::Align;
use tiny_skia::Pixmap;

use crate::copy;
use crate::machine::{Phase, Scene, Status};
use crate::paint::{clear_xrgb, copy_to_xrgb, disc, fill, rounded_rect, stroke, Fonts, TextStyle};
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

/// Logical width of the panel (field and status capsules).
const PANEL_W: f32 = 360.0;
/// Margin around the panel inside the prompt rectangle.
const PAD: f32 = 16.0;
/// Logical height of the prompt rectangle: banner, field, up to three lines of
/// status, the wait, the hint, Caps Lock.
const RECT_H: f32 = 320.0;
/// Where the field's top edge sits inside the prompt rectangle.
const FIELD_TOP: f32 = 32.0;
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

/// Draws the prompt of `scene` into `pixmap`, which covers the prompt
/// rectangle of an output of `logical` size.
fn draw_prompt(fonts: &mut Fonts, pixmap: &mut Pixmap, scene: &Scene, geometry: &Geometry) {
    let s = geometry.scale;
    // The backdrop: solid black until CEL-2-H-b brings the dimmed wallpaper;
    // the scrim lies over it either way.
    pixmap.fill(tiny_skia::Color::BLACK);
    if let Some(all) = rounded_rect(0.0, 0.0, pixmap.width() as f32, pixmap.height() as f32, 0.0) {
        fill(pixmap, &all, SCRIM);
    }

    // Rehearsal banner.
    if scene.rehearsal {
        let (x, y) = geometry.px(PAD, 6.0);
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

/// What the painter remembers of one output: which rectangle it drew into
/// each of the two buffers, and which is on screen.
#[derive(Debug, Default)]
struct OutputPaint {
    /// The rectangle last drawn into buffer 0 and 1.
    dirty: [Option<PixelRect>; 2],
    /// The rectangle of the frame on screen; `None` when the screen shows
    /// black (the cover, or a frame without a prompt).
    shown: Option<PixelRect>,
}

/// Draws one output into `frame`: clears what this buffer last drew, draws
/// the prompt when `want_prompt`, and returns the damage.
fn paint_output(
    state: &mut OutputPaint,
    frame: FrameBuf<'_>,
    scene: &Scene,
    logical: (u32, u32),
    want_prompt: bool,
    fonts: Option<&mut Fonts>,
) -> Option<PixelRect> {
    let size = (frame.width, frame.height);
    let index = frame.index.min(1);
    let whole = PixelRect {
        x: 0,
        y: 0,
        w: i32::try_from(size.0).unwrap_or(i32::MAX),
        h: i32::try_from(size.1).unwrap_or(i32::MAX),
    };
    if frame.fresh {
        // Zeroed memory: whatever was believed about it is gone.
        state.dirty[index] = None;
    }
    let mut touched = false;
    if let Some(old) = state.dirty[index].take() {
        clear_xrgb(
            frame.pixels,
            frame.stride,
            size,
            (
                u32::try_from(old.x).unwrap_or(0),
                u32::try_from(old.y).unwrap_or(0),
                u32::try_from(old.w).unwrap_or(0),
                u32::try_from(old.h).unwrap_or(0),
            ),
        );
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
                draw_prompt(fonts, &mut pixmap, scene, &geometry);
                copy_to_xrgb(
                    &pixmap,
                    frame.pixels,
                    frame.stride,
                    size,
                    (rect.x.unsigned_abs(), rect.y.unsigned_abs()),
                );
                drawn = Some(rect);
                touched = true;
            }
        }
    }
    state.dirty[index] = drawn;

    let previous = state.shown;
    state.shown = drawn;
    if frame.fresh {
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
    /// [`CpuPresenter::take_paint_time`]; the rehearsal measures H-15 with it.
    last_paint: Option<std::time::Duration>,
}

impl CpuPresenter {
    /// Starts the painter. It loads its font on the first frame; when that
    /// fails it stays blind: covers only, and the field still takes keys
    /// (F16).
    #[must_use]
    pub fn new() -> Self {
        Self {
            fonts: FontState::Pending,
            outputs: BTreeMap::new(),
            last_failure: None,
            last_paint: None,
        }
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
        for output in &scene.outputs {
            let Some(logical) = output.size else {
                continue;
            };
            let want_prompt = !scene.faded && scene.prompt == Some(output.id);
            let state = self.outputs.entry(output.id).or_default();
            if !want_prompt && state.shown.is_none() {
                // Black on screen, black wanted: the cover already says it.
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
        let mut blind = CpuPresenter {
            fonts: FontState::Failed,
            outputs: BTreeMap::new(),
            last_failure: None,
            last_paint: None,
        };
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

    /// H-15's number: the CPU time of one key press on a 4K output at 1.5.
    /// Run with `cargo test --release -p celestina-lock -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement, not a check"]
    fn measure_the_cpu_time_of_a_key_press_at_4k_scale_1_5() {
        let mut painter = painter();
        let mut screen = two_outputs();
        let mut s = scene();
        let mut micros = Vec::new();
        for n in 0..600usize {
            s.field_chars = n % 24 + 1;
            painter.render_to(&s, &mut screen);
            if let Some(took) = painter.take_paint_time() {
                micros.push(took.as_micros());
            }
        }
        micros.sort_unstable();
        let at = |p: f64| micros[((micros.len() - 1) as f64 * p).round() as usize];
        println!(
            "paint time of a key press, 3840x2160 at 1.5: n={} p50={}us p99={}us max={}us",
            micros.len(),
            at(0.5),
            at(0.99),
            micros[micros.len() - 1]
        );
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
        assert_eq!(copy::lock_wait(7), "Espera 7 s para volver a intentarlo");
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
            // Accented vowels, the tilde n and the inverted marks, by code point.
            let spanish = "\u{e1}\u{e9}\u{ed}\u{f3}\u{fa}\u{fc}\u{f1}\u{c1}\u{c9}\u{cd}\u{d3}\u{da}\u{d1}\u{bf}\u{a1}";
            if text.chars().any(|c| spanish.contains(c)) {
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
}
