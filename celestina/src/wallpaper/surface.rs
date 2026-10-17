//! The wallpaper surfaces (CEL-2-E): one `celestina-wallpaper` layer surface
//! per output in the `Background` layer, while the duty is on, the output is
//! not held by another client, and a decoded picture exists for it.
//!
//! [`Wallpapers`] is the app's one handle on the duty. It keeps, per output,
//! the size and scale the picture is decoded for, the generation of the last
//! request (a result of an older one is dropped), the surface, the texture in
//! use and the decoded bytes waiting for the painter. The rules:
//!
//! - The fallback state (no folder, no showable file, a file that fails) has
//!   **no** surface: the compositor's own background shows (design decision
//!   7). The surface is created when the first picture for the output arrives.
//! - A picture is decoded at the output's device size,
//!   `buffer_size(logical, scale)`. The runtime tells an output only its
//!   integer scale (a 4K output at 1.5 reports 2), and a surface's real
//!   (fractional) scale only to that surface's frames, which exist only once
//!   a picture created the surface. So the first request uses the integer
//!   estimate; a frame of any other surface on the output (the Island) that
//!   says the real scale asks again before a surface exists, and the
//!   wallpaper surface's own frames decide afterwards. Bytes decoded for
//!   another device size than the frame's are never uploaded; a texture of
//!   another size (the old scale's, until the new one arrives) is stretched
//!   over the whole surface, never drawn 1:1 from the corner.
//! - The bytes are uploaded on the surface's next frame (the painter exists
//!   only once a frame ran) and the CPU copy is dropped. A frame draws the
//!   texture 1:1 and asks for no other: the duty at rest draws nothing. An
//!   upload that fails posts a failure back to the loop, which clears the
//!   output as any failure does (no destroy inside the surface's frame).
//! - The duty off, an output gone, a fallback or a holder: the surface is
//!   destroyed, the texture released and the bytes dropped. A surface the
//!   compositor closed (the runtime destroys it without telling the product)
//!   is noticed before reuse and made again.
//! - Owner check (decision 2): asked when the duty turns on, when an output
//!   appears and on `wallpaper retry`; a held output gets no surface.
//! - Verbs: a target that is not a connected output is refused and nothing
//!   is written (the bus has already answered; the refusal is logged).
//!   `next`/`prev` read the folder again first and step over the fresh
//!   listing when it arrives (the newest waiting step wins).
//!
//! The decisions are written against [`Host`], so they are tested without a
//! compositor or GL; [`Wallpapers`]' public methods supply the real one.

use std::cell::OnceCell;
use std::rc::Rc;
use std::time::Duration;

use celestina_paint::{PixmapId, Rect};
use celestina_shell::settings::{Duty, Settings, WallpaperSettings};
use celestina_shell::wallpaper::{choose, next, Choice};
use celestina_wayland::surface::buffer_size;
use celestina_wayland::{
    Anchoring, FrameCx, KeyboardInteractivity, Layer, LayerSpec, OutputId, OutputInfo, RuntimeCtl,
    SurfaceId,
};

use crate::command::{Target, Verb};
use crate::error_chain;
use crate::island::menu::SharedPainter;
use crate::providers::settings::{ShellSettings, ShellSettingsError};
use crate::providers::wallpaper::{WallpaperCommand, WallpaperEvent, WallpaperProvider};
use crate::wallpaper::decode::Decoded;
use crate::wallpaper::NAMESPACE;

/// What the duty's decisions do to the world: the compositor, the worker and
/// the painter.
pub trait Host {
    /// Creates the wallpaper surface on `output`; `None` when it cannot.
    fn create(&mut self, output: OutputId) -> Option<SurfaceId>;
    /// Destroys a wallpaper surface.
    fn destroy(&mut self, surface: SurfaceId);
    /// Whether `surface` is still live: the compositor may have closed it
    /// (the runtime then destroyed it without telling the product).
    fn alive(&mut self, surface: SurfaceId) -> bool;
    /// Asks for a frame of `surface`.
    fn redraw(&mut self, surface: SurfaceId);
    /// Queues a command for the worker; whether it was taken.
    fn request(&mut self, command: WallpaperCommand) -> bool;
    /// Frees a texture; `false` when the painter cannot be had right now (the
    /// id is then kept and freed later).
    fn release(&mut self, texture: PixmapId) -> bool;
}

/// An uploaded picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Texture {
    id: PixmapId,
    /// Its size in device px.
    px: (u32, u32),
}

/// The request whose answer an output waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Asked {
    generation: u64,
    device_px: (u32, u32),
}

/// One output's part of the duty.
#[derive(Debug)]
struct Output {
    id: OutputId,
    name: String,
    /// Logical size: the output's, then the surface's.
    logical: (u32, u32),
    /// The best scale known: the output's integer one, then the real
    /// (fractional) one a frame on the output said.
    scale: f64,
    /// The integer scale the output last reported; a new one replaces a
    /// fractional `scale` learned under the old.
    reported_scale: f64,
    /// Whether an owner check for it was queued since the duty turned on.
    checked: bool,
    /// The namespace of the client holding its `Background` layer.
    held: Option<String>,
    asked: Option<Asked>,
    surface: Option<SurfaceId>,
    texture: Option<Texture>,
    /// Decoded bytes waiting for the surface's next frame.
    incoming: Option<Decoded>,
}

impl Output {
    fn new(info: &OutputInfo) -> Self {
        Self {
            id: info.id,
            name: info.name.clone(),
            logical: info.logical_size,
            scale: info.scale,
            reported_scale: info.scale,
            checked: false,
            held: None,
            asked: None,
            surface: None,
            texture: None,
            incoming: None,
        }
    }

    /// The failure the loop posts to itself when this output's picture was
    /// not uploaded; it clears the output as a decoding failure does.
    fn upload_failed(&self, why: &str) -> Option<WallpaperEvent> {
        self.asked.map(|asked| WallpaperEvent::Failed {
            output: self.name.clone(),
            generation: asked.generation,
            why: format!("the picture was not uploaded: {why}"),
        })
    }

    /// The device size a picture is decoded at; `None` while the logical
    /// size is unknown.
    fn device_px(&self) -> Option<(u32, u32)> {
        (self.logical.0 > 0 && self.logical.1 > 0 && self.scale.is_finite() && self.scale > 0.0)
            .then(|| buffer_size(self.logical, self.scale))
    }
}

/// The wallpaper duty's state in the app.
#[derive(Debug, Default)]
pub struct Wallpapers {
    outputs: Vec<Output>,
    /// The generation of the last request; every request takes the next.
    generation: u64,
    /// The folder's showable names from the worker's last scan.
    folder: Option<Vec<String>>,
    /// Textures to free once the painter can be had.
    releases: Vec<PixmapId>,
    /// A `next`/`prev` waiting for the folder's fresh listing; the newest
    /// wins.
    pending_step: Option<Step>,
    /// The worker's queue, set once it runs.
    provider: Rc<OnceCell<WallpaperProvider>>,
    refusal_logged: bool,
}

/// A `wallpaper next`/`prev` waiting for the folder's listing.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Step {
    /// One output, or every connected one.
    target: Option<String>,
    /// +1 for next, -1 for prev.
    by: i32,
}

/// The duty is switched on in `settings`.
fn duty_on(settings: &Settings) -> bool {
    settings.duties.wallpaper == Duty::On
}

/// The picture `output` shows now: its explicit choice, else what the
/// selection rules give over the folder's `names`.
fn current_choice(output: &str, wallpaper: &WallpaperSettings, names: &[String]) -> Option<String> {
    let explicit = wallpaper.outputs.get(output).map(String::as_str);
    match choose(output, explicit, names) {
        Choice::Image(name) => Some(name),
        Choice::Fallback => explicit.map(str::to_owned),
    }
}

impl Wallpapers {
    /// Takes the slot the worker's queue is put in once it runs.
    #[must_use]
    pub fn new(provider: Rc<OnceCell<WallpaperProvider>>) -> Self {
        Self {
            provider,
            ..Self::default()
        }
    }

    /// Whether `surface` is a wallpaper surface.
    #[must_use]
    pub fn owns(&self, surface: SurfaceId) -> bool {
        self.outputs.iter().any(|o| o.surface == Some(surface))
    }

    // --- the app's hooks: each builds the real host and runs the decision.

    /// An output appeared.
    pub fn output_added(
        &mut self,
        info: &OutputInfo,
        settings: &Settings,
        ctl: &mut RuntimeCtl<'_>,
        painter: Option<&SharedPainter>,
    ) {
        let provider = Rc::clone(&self.provider);
        let mut host = AppHost::new(ctl, provider.get(), painter);
        self.added(info, settings, &mut host);
    }

    /// An output's mode or scale changed.
    pub fn output_changed(
        &mut self,
        info: &OutputInfo,
        settings: &Settings,
        ctl: &mut RuntimeCtl<'_>,
        painter: Option<&SharedPainter>,
    ) {
        let provider = Rc::clone(&self.provider);
        let mut host = AppHost::new(ctl, provider.get(), painter);
        self.changed(info, settings, &mut host);
    }

    /// An output is gone, and its surface with it.
    pub fn output_removed(
        &mut self,
        output: OutputId,
        ctl: &mut RuntimeCtl<'_>,
        painter: Option<&SharedPainter>,
    ) {
        let provider = Rc::clone(&self.provider);
        let mut host = AppHost::new(ctl, provider.get(), painter);
        self.removed(output, &mut host);
    }

    /// The worker's news (or a frame's failed upload, posted back). A
    /// folder listing also runs a waiting `next`/`prev`, which writes the
    /// settings.
    pub fn message(
        &mut self,
        event: WallpaperEvent,
        settings: &mut ShellSettings,
        ctl: &mut RuntimeCtl<'_>,
        painter: Option<&SharedPainter>,
    ) {
        let provider = Rc::clone(&self.provider);
        let mut host = AppHost::new(ctl, provider.get(), painter);
        self.news(event, settings, &mut host);
    }

    /// A frame of another surface on `output` says the output's real
    /// (fractional) scale. The runtime tells an output only its integer
    /// scale and a surface's real one only to that surface's frames; the
    /// Island's frames on the same output say it before the wallpaper has a
    /// surface, so a picture asked at the integer estimate is asked again at
    /// the real size before it is shown. Once the wallpaper has a surface,
    /// its own frames decide.
    pub fn scale_seen(&mut self, output: &str, scale: f64, settings: &Settings) {
        let provider = Rc::clone(&self.provider);
        self.seen(output, scale, settings, &mut |command| {
            provider
                .get()
                .is_some_and(|provider| provider.request(command).is_ok())
        });
    }

    /// The settings in force changed from `before` (a verb, or the file
    /// changed on disk).
    pub fn settings_changed(
        &mut self,
        before: &Settings,
        after: &Settings,
        ctl: &mut RuntimeCtl<'_>,
        painter: Option<&SharedPainter>,
    ) {
        let provider = Rc::clone(&self.provider);
        let mut host = AppHost::new(ctl, provider.get(), painter);
        self.apply(before, after, &mut host);
    }

    /// A `wallpaper …` verb.
    pub fn command(
        &mut self,
        verb: &Verb,
        settings: &mut ShellSettings,
        ctl: &mut RuntimeCtl<'_>,
        painter: Option<&SharedPainter>,
    ) {
        let provider = Rc::clone(&self.provider);
        let mut host = AppHost::new(ctl, provider.get(), painter);
        self.verb(verb, settings, &mut host);
    }

    /// A frame of a wallpaper surface: asks for the picture again when the
    /// surface's device size is not the one it was decoded for, uploads
    /// waiting bytes decoded for this size (bytes for another size are
    /// dropped: the new request brings the right ones), and draws the
    /// texture. A texture of this size lands 1:1 on the buffer; one of
    /// another size (the scale changed and the new picture is on its way) is
    /// stretched over the whole surface, so it keeps covering it with no
    /// zoom and no uncovered strip. Asks for no further frame, unless the
    /// new request was refused (then it tries again in a second).
    ///
    /// Returns the event the loop must post to itself when the upload
    /// failed: the surface cannot be destroyed inside its own frame, and the
    /// posted failure clears the output as any other failure does (decision
    /// 7: no surface when nothing is shown).
    #[must_use]
    pub fn frame(
        &mut self,
        cx: &mut FrameCx<'_>,
        shared: &SharedPainter,
        settings: &Settings,
    ) -> Option<WallpaperEvent> {
        let logical = (round_px(cx.size.0), round_px(cx.size.1));
        let provider = Rc::clone(&self.provider);
        let asked = self.observed(cx.surface, logical, cx.scale, settings, &mut |command| {
            provider
                .get()
                .is_some_and(|provider| provider.request(command).is_ok())
        });
        if !asked {
            cx.ctl.redraw_after(RETRY_AFTER);
        }
        let wanted = valid_scale(cx.scale).map(|scale| buffer_size(logical, scale));
        let Ok(mut painter) = shared.try_borrow_mut() else {
            tracing::warn!("the painter is in use; the wallpaper's frame is skipped");
            cx.ctl.request_redraw();
            return None;
        };
        for texture in self.releases.drain(..) {
            painter.release_pixmap(texture);
        }
        let output = self
            .outputs
            .iter_mut()
            .find(|o| o.surface == Some(cx.surface))?;
        let mut failed = None;
        if let Some(image) = output.incoming.take() {
            // The CPU copy is dropped at the end of this block, uploaded or not.
            if Some((image.width, image.height)) != wanted {
                tracing::debug!(
                    output = %output.name,
                    width = image.width,
                    height = image.height,
                    ?wanted,
                    "wallpaper decoded for another size; dropped"
                );
            } else {
                match painter.upload_wallpaper(&image.rgba, image.width, image.height) {
                    Ok(id) => {
                        if let Some(old) = output.texture.replace(Texture {
                            id,
                            px: (image.width, image.height),
                        }) {
                            painter.release_pixmap(old.id);
                        }
                        tracing::info!(
                            output = %output.name,
                            width = image.width,
                            height = image.height,
                            "wallpaper uploaded"
                        );
                    }
                    Err(err) => {
                        if let Some(old) = output.texture.take() {
                            painter.release_pixmap(old.id);
                        }
                        failed = output.upload_failed(&error_chain(&err));
                    }
                }
            }
        }
        let dpr = valid_scale(cx.scale).map_or(1.0, |scale| scale as f32);
        painter.begin(cx.size, dpr, cx.blur_available);
        if let Some(texture) = output.texture {
            painter.draw_wallpaper(texture.id, placement(texture.px, wanted, cx.size, dpr));
        }
        painter.end();
        failed
    }

    // --- the decisions, against any host.

    fn added(&mut self, info: &OutputInfo, settings: &Settings, host: &mut impl Host) {
        self.flush_releases(host);
        if let Some(stale) = self.outputs.iter().position(|o| o.id == info.id) {
            let gone = self.outputs.remove(stale);
            self.forget(gone, host);
        }
        self.outputs.push(Output::new(info));
        if !duty_on(settings) {
            return;
        }
        let index = self.outputs.len() - 1;
        self.check(&[index], host);
        self.resolve(index, settings, host);
    }

    fn changed(&mut self, info: &OutputInfo, settings: &Settings, host: &mut impl Host) {
        self.flush_releases(host);
        let Some(index) = self.outputs.iter().position(|o| o.id == info.id) else {
            return;
        };
        self.outputs[index].name.clone_from(&info.name);
        if let Some(surface) = self.live_surface(index, host) {
            // The surface's next frame says its own size and scale; a
            // different device size asks for the picture again there.
            host.redraw(surface);
            return;
        }
        let output = &mut self.outputs[index];
        output.logical = info.logical_size;
        if (info.scale - output.reported_scale).abs() >= f64::EPSILON {
            // A real scale learned under the old integer one is outdated.
            output.scale = info.scale;
            output.reported_scale = info.scale;
        }
        // Only a different device size (or a first request) asks.
        self.ask_if_resized(index, settings, &mut |command| host.request(command));
    }

    fn removed(&mut self, output: OutputId, host: &mut impl Host) {
        self.flush_releases(host);
        if let Some(index) = self.outputs.iter().position(|o| o.id == output) {
            let mut gone = self.outputs.remove(index);
            // The surface went with the output.
            gone.surface = None;
            self.forget(gone, host);
        }
    }

    fn event(&mut self, event: WallpaperEvent, settings: &Settings, host: &mut impl Host) {
        self.flush_releases(host);
        match event {
            WallpaperEvent::Decoded {
                output,
                generation,
                image,
            } => {
                let Some(index) = self.current(&output, generation, settings) else {
                    tracing::debug!(%output, generation, "a stale wallpaper is dropped");
                    return;
                };
                self.outputs[index].incoming = Some(image);
                if let Some(surface) = self.live_surface(index, host) {
                    host.redraw(surface);
                    return;
                }
                let out = &mut self.outputs[index];
                match host.create(out.id) {
                    Some(surface) => {
                        out.surface = Some(surface);
                        tracing::info!(output = %out.name, "wallpaper surface created");
                    }
                    None => out.incoming = None,
                }
            }
            WallpaperEvent::Fallback { output, generation } => {
                if let Some(index) = self.current(&output, generation, settings) {
                    tracing::info!(%output, "wallpaper: nothing to show; no surface");
                    self.clear(index, host);
                }
            }
            WallpaperEvent::Failed {
                output,
                generation,
                why,
            } => {
                if let Some(index) = self.current(&output, generation, settings) {
                    tracing::warn!(%output, "wallpaper: {why}; no surface");
                    self.clear(index, host);
                }
            }
            WallpaperEvent::Held { output, by } => {
                let Some(index) = self.outputs.iter().position(|o| o.name == output) else {
                    return;
                };
                if self.outputs[index].held.as_deref() != Some(by.as_str()) {
                    tracing::info!(%output, holder = %by, "wallpaper: output held by another client");
                }
                self.outputs[index].held = Some(by);
                self.clear(index, host);
            }
            WallpaperEvent::Free { output } => {
                let Some(index) = self.outputs.iter().position(|o| o.name == output) else {
                    return;
                };
                if self.outputs[index].held.take().is_some() && duty_on(settings) {
                    tracing::info!(%output, "wallpaper: the output is free again");
                    self.resolve(index, settings, host);
                }
            }
            WallpaperEvent::Folder { names } => {
                let changed = self.folder.as_ref().is_some_and(|old| *old != names);
                self.folder = Some(names);
                if !duty_on(settings) {
                    return;
                }
                if changed {
                    self.resolve_all(settings, host);
                } else {
                    // The worker answers: outputs that appeared before it
                    // ran were never asked.
                    self.catch_up(settings, host);
                }
            }
        }
    }

    fn apply(&mut self, before: &Settings, after: &Settings, host: &mut impl Host) {
        self.flush_releases(host);
        match (duty_on(before), duty_on(after)) {
            (_, false) => {
                if duty_on(before) {
                    tracing::info!("wallpaper duty off: every surface and texture released");
                }
                for index in 0..self.outputs.len() {
                    self.clear(index, host);
                    let out = &mut self.outputs[index];
                    out.asked = None;
                    out.held = None;
                    out.checked = false;
                }
                self.folder = None;
            }
            (false, true) => {
                tracing::info!("wallpaper duty on");
                self.folder = None;
                let all: Vec<usize> = (0..self.outputs.len()).collect();
                self.check(&all, host);
                self.rescan(after, host);
                self.resolve_all(after, host);
            }
            (true, true) => {
                let (old, new) = (&before.wallpaper, &after.wallpaper);
                if old == new {
                    return;
                }
                if old.folder != new.folder {
                    self.folder = None;
                    self.rescan(after, host);
                }
                if old.folder != new.folder || old.mode != new.mode {
                    self.resolve_all(after, host);
                    return;
                }
                for index in 0..self.outputs.len() {
                    let name = &self.outputs[index].name;
                    if old.outputs.get(name) != new.outputs.get(name) {
                        self.resolve(index, after, host);
                    }
                }
            }
        }
    }

    fn verb(&mut self, verb: &Verb, settings: &mut ShellSettings, host: &mut impl Host) {
        self.flush_releases(host);
        let before = settings.current().clone();
        let written = match verb {
            Verb::WallpaperRetry => {
                if duty_on(&before) {
                    tracing::info!("wallpaper retry: owners checked and pictures asked again");
                    let all: Vec<usize> = (0..self.outputs.len()).collect();
                    for out in &mut self.outputs {
                        out.checked = false;
                    }
                    self.check(&all, host);
                    self.rescan(&before, host);
                    self.resolve_all(&before, host);
                } else {
                    tracing::info!("wallpaper retry: the duty is off");
                }
                return;
            }
            Verb::WallpaperSet { target, file } => {
                let names: Vec<String> = match target {
                    Target::All => self.outputs.iter().map(|o| o.name.clone()).collect(),
                    Target::Output(name) if self.connected(name) => vec![name.clone()],
                    Target::Output(name) => {
                        // A key for an output that is not there would be
                        // kept forever; the bus has already answered, so
                        // the refusal is the log's.
                        tracing::warn!(output = %name, "wallpaper set: no such output is connected; nothing written");
                        return;
                    }
                };
                if names.is_empty() {
                    tracing::info!("wallpaper set: no output to set");
                    return;
                }
                settings.update_wallpaper(|w| {
                    for name in names {
                        w.outputs.insert(name, file.clone());
                    }
                })
            }
            Verb::WallpaperNext(target) | Verb::WallpaperPrev(target) => {
                if let Some(name) = target.as_deref().filter(|name| !self.connected(name)) {
                    tracing::warn!(output = %name, "wallpaper: no such output is connected; nothing written");
                    return;
                }
                let step = Step {
                    target: target.clone(),
                    by: if matches!(verb, Verb::WallpaperNext(_)) {
                        1
                    } else {
                        -1
                    },
                };
                // The listing in hand may be stale or missing: the step runs
                // over the fresh one, when the worker answers.
                if self.rescan(&before, host) {
                    if self.pending_step.replace(step).is_some() {
                        tracing::debug!("wallpaper: a newer step replaces the one waiting");
                    }
                } else if self.folder.is_some() {
                    tracing::info!(
                        "wallpaper: the folder cannot be read again; the last listing is used"
                    );
                    self.step(&step, settings, host);
                } else {
                    tracing::warn!("wallpaper: the folder cannot be read; nothing to step to");
                }
                return;
            }
            Verb::WallpaperMode(mode) => settings.update_wallpaper(|w| w.mode = *mode),
            _ => return,
        };
        self.written(verb, &before, written, settings, host);
    }

    /// The worker's news, then a waiting step once the folder's listing is
    /// fresh.
    fn news(&mut self, event: WallpaperEvent, settings: &mut ShellSettings, host: &mut impl Host) {
        let listing = matches!(event, WallpaperEvent::Folder { .. });
        let current = settings.current().clone();
        self.event(event, &current, host);
        if listing {
            if let Some(step) = self.pending_step.take() {
                self.step(&step, settings, host);
            }
        }
    }

    /// Steps the targets of `step` through the folder's listing from their
    /// current pictures, durably.
    fn step(&mut self, step: &Step, settings: &mut ShellSettings, host: &mut impl Host) {
        let before = settings.current().clone();
        let folder = self.folder.clone().unwrap_or_default();
        let targets: Vec<String> = match &step.target {
            // It may have gone while the listing was read.
            Some(name) if self.connected(name) => vec![name.clone()],
            Some(name) => {
                tracing::warn!(output = %name, "wallpaper: the output is gone; nothing written");
                return;
            }
            None => self.outputs.iter().map(|o| o.name.clone()).collect(),
        };
        let picks: Vec<(String, String)> = targets
            .into_iter()
            .filter_map(|name| {
                let now = current_choice(&name, &before.wallpaper, &folder);
                next(&folder, now.as_deref(), step.by).map(|file| (name, file))
            })
            .collect();
        if picks.is_empty() {
            tracing::info!("wallpaper: nothing to step to (no showable file or no output)");
            return;
        }
        let written = settings.update_wallpaper(|w| {
            for (name, file) in picks {
                w.outputs.insert(name, file);
            }
        });
        self.written(step, &before, written, settings, host);
    }

    /// Applies a verb's change once it is durable.
    fn written(
        &mut self,
        what: &dyn std::fmt::Debug,
        before: &Settings,
        written: Result<bool, ShellSettingsError>,
        settings: &ShellSettings,
        host: &mut impl Host,
    ) {
        match written {
            // Durable now: only then is it applied.
            Ok(true) => {
                let after = settings.current().clone();
                self.apply(before, &after, host);
            }
            Ok(false) => tracing::debug!(?what, "wallpaper: already so"),
            Err(err) => tracing::warn!(?what, "wallpaper not changed: {}", error_chain(&err)),
        }
    }

    /// Whether an output named `name` is connected.
    fn connected(&self, name: &str) -> bool {
        self.outputs.iter().any(|o| o.name == name)
    }

    /// A frame of `surface` says its real logical size and scale; a device
    /// size other than the one asked for asks again. Returns `false` when
    /// that request was refused.
    fn observed(
        &mut self,
        surface: SurfaceId,
        logical: (u32, u32),
        scale: f64,
        settings: &Settings,
        request: &mut dyn FnMut(WallpaperCommand) -> bool,
    ) -> bool {
        let Some(index) = self.outputs.iter().position(|o| o.surface == Some(surface)) else {
            return true;
        };
        let out = &mut self.outputs[index];
        out.logical = logical;
        out.scale = scale;
        self.ask_if_resized(index, settings, request)
    }

    /// A frame of another surface on `output` says its real scale; while the
    /// wallpaper has no surface there, a device size other than the one
    /// asked for asks again.
    fn seen(
        &mut self,
        output: &str,
        scale: f64,
        settings: &Settings,
        request: &mut dyn FnMut(WallpaperCommand) -> bool,
    ) {
        if valid_scale(scale).is_none() {
            return;
        }
        let Some(index) = self
            .outputs
            .iter()
            .position(|o| o.name == output && o.surface.is_none())
        else {
            return;
        };
        let out = &mut self.outputs[index];
        if (out.scale - scale).abs() < f64::EPSILON {
            return;
        }
        tracing::debug!(output = %out.name, scale, "wallpaper: the output's real scale");
        out.scale = scale;
        // Not asked yet (the worker is not running): the first request will
        // use it.
        if out.asked.is_some() {
            self.ask_if_resized(index, settings, request);
        }
    }

    /// Asks for `index`'s picture again when its device size is not the one
    /// asked for; `false` when that request was refused.
    fn ask_if_resized(
        &mut self,
        index: usize,
        settings: &Settings,
        request: &mut dyn FnMut(WallpaperCommand) -> bool,
    ) -> bool {
        let out = &self.outputs[index];
        let wanted = out.device_px();
        if wanted.is_none() || out.asked.map(|a| a.device_px) == wanted || !duty_on(settings) {
            return true;
        }
        tracing::debug!(output = %out.name, ?wanted, "wallpaper: the device size differs; asked again");
        self.resolve_with(index, settings, request)
    }

    // --- helpers.

    /// The index of `output` when `generation` is its latest request and the
    /// duty is on and the output free.
    fn current(&self, output: &str, generation: u64, settings: &Settings) -> Option<usize> {
        if !duty_on(settings) {
            return None;
        }
        self.outputs.iter().position(|o| {
            o.name == output
                && o.held.is_none()
                && o.asked.is_some_and(|a| a.generation == generation)
        })
    }

    fn resolve_all(&mut self, settings: &Settings, host: &mut impl Host) {
        for index in 0..self.outputs.len() {
            self.resolve(index, settings, host);
        }
    }

    /// Outputs never asked (they appeared before the worker ran, or a request
    /// was refused) are checked and asked now.
    fn catch_up(&mut self, settings: &Settings, host: &mut impl Host) {
        let unchecked: Vec<usize> = (0..self.outputs.len())
            .filter(|&i| !self.outputs[i].checked)
            .collect();
        self.check(&unchecked, host);
        for index in 0..self.outputs.len() {
            if self.outputs[index].asked.is_none() {
                self.resolve(index, settings, host);
            }
        }
    }

    fn resolve(&mut self, index: usize, settings: &Settings, host: &mut impl Host) {
        self.resolve_with(index, settings, &mut |command| host.request(command));
    }

    /// Drops a surface the compositor closed: the runtime destroyed it
    /// without telling the product, so it is not destroyed again; its
    /// texture is released. Returns `index`'s live surface, if any.
    fn live_surface(&mut self, index: usize, host: &mut impl Host) -> Option<SurfaceId> {
        let surface = self.outputs.get(index)?.surface?;
        if host.alive(surface) {
            return Some(surface);
        }
        let out = &mut self.outputs[index];
        tracing::warn!(output = %out.name, "the compositor closed the wallpaper surface");
        out.surface = None;
        if let Some(texture) = out.texture.take() {
            self.release(texture.id, host);
        }
        None
    }

    /// Asks for `index`'s picture; `false` only when the request was
    /// refused.
    fn resolve_with(
        &mut self,
        index: usize,
        settings: &Settings,
        request: &mut dyn FnMut(WallpaperCommand) -> bool,
    ) -> bool {
        let Some(out) = self.outputs.get(index) else {
            return true;
        };
        if out.held.is_some() {
            return true;
        }
        let Some(device_px) = out.device_px() else {
            tracing::debug!(output = %out.name, "wallpaper: the output's size is unknown yet");
            return true;
        };
        let generation = self.generation.wrapping_add(1);
        let command = WallpaperCommand::Resolve {
            output: out.name.clone(),
            device_px,
            generation,
            settings: settings.wallpaper.clone(),
        };
        if request(command) {
            self.generation = generation;
            self.outputs[index].asked = Some(Asked {
                generation,
                device_px,
            });
            true
        } else {
            self.refused();
            false
        }
    }

    fn check(&mut self, indices: &[usize], host: &mut impl Host) {
        let outputs: Vec<String> = indices
            .iter()
            .filter_map(|&i| self.outputs.get(i).map(|o| o.name.clone()))
            .collect();
        if outputs.is_empty() {
            return;
        }
        if host.request(WallpaperCommand::Check { outputs }) {
            for &i in indices {
                if let Some(out) = self.outputs.get_mut(i) {
                    out.checked = true;
                }
            }
        } else {
            self.refused();
        }
    }

    /// Asks the worker to read the folder again; whether it took it.
    fn rescan(&mut self, settings: &Settings, host: &mut impl Host) -> bool {
        let folder = settings.wallpaper.folder.clone();
        let taken = host.request(WallpaperCommand::Rescan { folder });
        if !taken {
            self.refused();
        }
        taken
    }

    /// The fallback state for one output: no surface, no texture, no bytes.
    fn clear(&mut self, index: usize, host: &mut impl Host) {
        let Some(out) = self.outputs.get_mut(index) else {
            return;
        };
        out.incoming = None;
        let surface = out.surface.take();
        let texture = out.texture.take();
        if let Some(surface) = surface {
            host.destroy(surface);
        }
        if let Some(texture) = texture {
            self.release(texture.id, host);
        }
    }

    fn forget(&mut self, gone: Output, host: &mut impl Host) {
        if let Some(surface) = gone.surface {
            host.destroy(surface);
        }
        if let Some(texture) = gone.texture {
            self.release(texture.id, host);
        }
    }

    fn release(&mut self, texture: PixmapId, host: &mut impl Host) {
        if !host.release(texture) {
            self.releases.push(texture);
        }
    }

    fn flush_releases(&mut self, host: &mut impl Host) {
        self.releases.retain(|&texture| !host.release(texture));
    }

    /// A refused request is logged once per run; before the worker runs a
    /// refusal is expected and the worker's first answer catches up.
    fn refused(&mut self) {
        if self.provider.get().is_none() {
            tracing::debug!("wallpaper: no worker yet; the request waits for its first answer");
            return;
        }
        if !self.refusal_logged {
            self.refusal_logged = true;
            tracing::warn!(
                "the wallpaper worker refused a request (its queue is full or it stopped); later refusals are not logged"
            );
        }
    }
}

/// Where a texture of `texture_px` device px is drawn on a surface of
/// logical `size` at `dpr`: 1:1 on the buffer when it was decoded for the
/// surface's device size `wanted`, else stretched over the whole surface (a
/// picture for the old scale, until the new one arrives: no zoom, no strip
/// left uncovered).
fn placement(
    texture_px: (u32, u32),
    wanted: Option<(u32, u32)>,
    size: (f32, f32),
    dpr: f32,
) -> Rect {
    let (w, h) = if Some(texture_px) == wanted {
        (texture_px.0 as f32 / dpr, texture_px.1 as f32 / dpr)
    } else {
        size
    };
    Rect {
        x: 0.0,
        y: 0.0,
        w,
        h,
    }
}

/// `scale` when it is a usable device pixel ratio.
fn valid_scale(scale: f64) -> Option<f64> {
    (scale.is_finite() && scale > 0.0).then_some(scale)
}

/// How long a frame whose new request was refused waits before trying
/// again.
const RETRY_AFTER: Duration = Duration::from_secs(1);

/// A logical side in whole px (the runtime's surfaces have integer sizes).
fn round_px(side: f32) -> u32 {
    if side.is_finite() && side > 0.0 {
        side.round() as u32
    } else {
        0
    }
}

/// The real host: the loop's control, the worker's queue and the painter.
struct AppHost<'a, 'b> {
    ctl: &'a mut RuntimeCtl<'b>,
    provider: Option<&'a WallpaperProvider>,
    painter: Option<&'a SharedPainter>,
}

impl<'a, 'b> AppHost<'a, 'b> {
    fn new(
        ctl: &'a mut RuntimeCtl<'b>,
        provider: Option<&'a WallpaperProvider>,
        painter: Option<&'a SharedPainter>,
    ) -> Self {
        Self {
            ctl,
            provider,
            painter,
        }
    }
}

impl Host for AppHost<'_, '_> {
    fn create(&mut self, output: OutputId) -> Option<SurfaceId> {
        // The whole output under every window; -1 keeps it out of every
        // other surface's exclusive zone (it is neither moved nor shrunk by
        // the Island's band), no keyboard, and the empty input region every
        // surface starts with (nothing is ever set on it).
        let spec = LayerSpec {
            output,
            namespace: NAMESPACE,
            layer: Layer::Background,
            anchor: Anchoring::Fill,
            size: (0, 0),
            exclusive_zone: -1,
            keyboard: KeyboardInteractivity::None,
        };
        match self.ctl.create_layer_surface(spec) {
            Ok(surface) => Some(surface),
            Err(err) => {
                tracing::error!(output = output.0, %err, "no wallpaper surface on this output");
                None
            }
        }
    }

    fn destroy(&mut self, surface: SurfaceId) {
        self.ctl.destroy_surface(surface);
    }

    fn alive(&mut self, surface: SurfaceId) -> bool {
        self.ctl.surface(surface).is_some()
    }

    fn redraw(&mut self, surface: SurfaceId) {
        if let Some(mut ctl) = self.ctl.surface(surface) {
            ctl.request_redraw();
        }
    }

    fn request(&mut self, command: WallpaperCommand) -> bool {
        self.provider
            .is_some_and(|provider| provider.request(command).is_ok())
    }

    fn release(&mut self, texture: PixmapId) -> bool {
        match self.painter.map(|p| p.try_borrow_mut()) {
            Some(Ok(mut painter)) => {
                painter.release_pixmap(texture);
                true
            }
            Some(Err(_)) => false,
            // No painter: nothing was ever uploaded.
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_shell::wallpaper::Mode;
    use std::path::PathBuf;

    /// Records what the decisions did.
    #[derive(Debug, Default)]
    struct Fake {
        next_surface: u32,
        created: Vec<(OutputId, SurfaceId)>,
        destroyed: Vec<SurfaceId>,
        redrawn: Vec<SurfaceId>,
        requests: Vec<WallpaperCommand>,
        released: Vec<PixmapId>,
        refuse: bool,
        busy_painter: bool,
        /// Surfaces the compositor closed.
        closed: Vec<SurfaceId>,
        /// The settings file, read back at each request.
        file: Option<PathBuf>,
        file_at_request: Vec<Option<String>>,
    }

    impl Host for Fake {
        fn create(&mut self, output: OutputId) -> Option<SurfaceId> {
            self.next_surface += 1;
            let surface = SurfaceId(100 + self.next_surface);
            self.created.push((output, surface));
            Some(surface)
        }
        fn destroy(&mut self, surface: SurfaceId) {
            self.destroyed.push(surface);
        }
        fn alive(&mut self, surface: SurfaceId) -> bool {
            !self.closed.contains(&surface) && !self.destroyed.contains(&surface)
        }
        fn redraw(&mut self, surface: SurfaceId) {
            self.redrawn.push(surface);
        }
        fn request(&mut self, command: WallpaperCommand) -> bool {
            if let Some(file) = &self.file {
                self.file_at_request
                    .push(std::fs::read_to_string(file).ok());
            }
            if self.refuse {
                return false;
            }
            self.requests.push(command);
            true
        }
        fn release(&mut self, texture: PixmapId) -> bool {
            if self.busy_painter {
                return false;
            }
            self.released.push(texture);
            true
        }
    }

    impl Fake {
        fn resolves(&self) -> Vec<(String, (u32, u32), u64)> {
            self.requests
                .iter()
                .filter_map(|c| match c {
                    WallpaperCommand::Resolve {
                        output,
                        device_px,
                        generation,
                        ..
                    } => Some((output.clone(), *device_px, *generation)),
                    _ => None,
                })
                .collect()
        }

        fn checks(&self) -> Vec<Vec<String>> {
            self.requests
                .iter()
                .filter_map(|c| match c {
                    WallpaperCommand::Check { outputs } => Some(outputs.clone()),
                    _ => None,
                })
                .collect()
        }
    }

    fn info(id: u32, name: &str) -> OutputInfo {
        OutputInfo {
            id: OutputId(id),
            name: name.to_owned(),
            logical_size: (1000, 600),
            scale: 2.0,
            refresh_mhz: 60_000,
        }
    }

    fn on() -> Settings {
        let mut settings = Settings::default();
        settings.duties.wallpaper = Duty::On;
        settings.wallpaper.folder = Some(PathBuf::from("/pictures"));
        settings
    }

    fn image(px: (u32, u32)) -> Decoded {
        Decoded {
            rgba: vec![255; px.0 as usize * px.1 as usize * 4],
            width: px.0,
            height: px.1,
            source_px: px,
        }
    }

    fn decoded(output: &str, generation: u64, px: (u32, u32)) -> WallpaperEvent {
        WallpaperEvent::Decoded {
            output: output.to_owned(),
            generation,
            image: image(px),
        }
    }

    /// Pretends the surface's first frame uploaded the bytes.
    fn upload(w: &mut Wallpapers, name: &str, id: u32) {
        let out = w
            .outputs
            .iter_mut()
            .find(|o| o.name == name)
            .expect("the output");
        let image = out.incoming.take().expect("bytes waiting");
        out.texture = Some(Texture {
            id: PixmapId(id),
            px: (image.width, image.height),
        });
    }

    fn output<'w>(w: &'w Wallpapers, name: &str) -> &'w Output {
        w.outputs
            .iter()
            .find(|o| o.name == name)
            .expect("the output")
    }

    #[test]
    fn an_added_output_is_checked_then_asked_at_its_device_size() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        w.added(&info(1, "DP-1"), &on(), &mut host);
        assert_eq!(host.checks(), vec![vec!["DP-1".to_owned()]]);
        assert!(matches!(
            host.requests.first(),
            Some(WallpaperCommand::Check { .. })
        ));
        assert_eq!(host.resolves(), vec![("DP-1".to_owned(), (2000, 1200), 1)]);
        // No picture yet: no surface.
        assert!(host.created.is_empty());
        assert!(!w.owns(SurfaceId(101)));
    }

    #[test]
    fn with_the_duty_off_nothing_is_asked() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        w.added(&info(1, "DP-1"), &Settings::default(), &mut host);
        assert!(host.requests.is_empty());
    }

    #[test]
    fn a_decoded_picture_creates_the_surface_and_waits_for_its_frame() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        w.event(decoded("DP-1", 1, (2000, 1200)), &settings, &mut host);
        assert_eq!(host.created, vec![(OutputId(1), SurfaceId(101))]);
        assert!(w.owns(SurfaceId(101)));
        assert!(output(&w, "DP-1").incoming.is_some());
        // A second picture for the same surface asks for a frame instead.
        w.resolve(0, &settings, &mut host);
        w.event(decoded("DP-1", 2, (2000, 1200)), &settings, &mut host);
        assert_eq!(host.created.len(), 1);
        assert_eq!(host.redrawn, vec![SurfaceId(101)]);
    }

    #[test]
    fn the_fallback_state_has_no_surface() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        w.event(
            WallpaperEvent::Fallback {
                output: "DP-1".into(),
                generation: 1,
            },
            &settings,
            &mut host,
        );
        assert!(host.created.is_empty());

        // A picture, then a fallback (the file was removed): the surface and
        // its texture go.
        w.resolve(0, &settings, &mut host);
        w.event(decoded("DP-1", 2, (2000, 1200)), &settings, &mut host);
        upload(&mut w, "DP-1", 7);
        w.resolve(0, &settings, &mut host);
        w.event(
            WallpaperEvent::Failed {
                output: "DP-1".into(),
                generation: 3,
                why: "truncated".into(),
            },
            &settings,
            &mut host,
        );
        assert_eq!(host.destroyed, vec![SurfaceId(101)]);
        assert_eq!(host.released, vec![PixmapId(7)]);
        assert!(!w.owns(SurfaceId(101)));
    }

    #[test]
    fn a_stale_result_is_dropped() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        // A newer request (the scale changed) outdates the first.
        w.resolve(0, &settings, &mut host);
        assert_eq!(output(&w, "DP-1").asked.map(|a| a.generation), Some(2));
        w.event(decoded("DP-1", 1, (2000, 1200)), &settings, &mut host);
        w.event(
            WallpaperEvent::Fallback {
                output: "DP-1".into(),
                generation: 1,
            },
            &settings,
            &mut host,
        );
        assert!(host.created.is_empty());
        assert!(output(&w, "DP-1").incoming.is_none());
        // The current one is taken.
        w.event(decoded("DP-1", 2, (2000, 1200)), &settings, &mut host);
        assert_eq!(host.created.len(), 1);
        // A result for an output that is not there is ignored.
        w.event(decoded("HDMI-A-1", 2, (10, 10)), &settings, &mut host);
        assert_eq!(host.created.len(), 1);
    }

    #[test]
    fn a_held_output_gets_no_surface_until_it_is_free() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        w.event(
            WallpaperEvent::Held {
                output: "DP-1".into(),
                by: "noctalia-wallpaper".into(),
            },
            &settings,
            &mut host,
        );
        w.event(decoded("DP-1", 1, (2000, 1200)), &settings, &mut host);
        assert!(host.created.is_empty());
        // Held outputs are not asked.
        let asked = host.resolves().len();
        w.resolve(0, &settings, &mut host);
        assert_eq!(host.resolves().len(), asked);
        // Free again (after `wallpaper retry`): asked again.
        w.event(
            WallpaperEvent::Free {
                output: "DP-1".into(),
            },
            &settings,
            &mut host,
        );
        assert_eq!(host.resolves().len(), asked + 1);
        let generation = w.generation;
        w.event(
            decoded("DP-1", generation, (2000, 1200)),
            &settings,
            &mut host,
        );
        assert_eq!(host.created.len(), 1);
        // Held while showing: the surface and the texture go.
        upload(&mut w, "DP-1", 3);
        w.event(
            WallpaperEvent::Held {
                output: "DP-1".into(),
                by: "swaybg".into(),
            },
            &settings,
            &mut host,
        );
        assert_eq!(host.destroyed, vec![SurfaceId(101)]);
        assert_eq!(host.released, vec![PixmapId(3)]);
    }

    #[test]
    fn a_removed_output_releases_its_texture_and_keeps_the_others() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        w.added(&info(2, "HDMI-A-1"), &settings, &mut host);
        w.event(decoded("DP-1", 1, (2000, 1200)), &settings, &mut host);
        w.event(decoded("HDMI-A-1", 2, (2000, 1200)), &settings, &mut host);
        upload(&mut w, "DP-1", 5);
        w.removed(OutputId(1), &mut host);
        // Its surface went with the output: not destroyed twice.
        assert!(host.destroyed.is_empty());
        assert_eq!(host.released, vec![PixmapId(5)]);
        assert!(!w.owns(SurfaceId(101)));
        assert!(w.owns(SurfaceId(102)));
        assert_eq!(w.outputs.len(), 1);
    }

    #[test]
    fn the_duty_off_releases_everything_and_on_again_resolves_live() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        w.added(&info(2, "HDMI-A-1"), &settings, &mut host);
        w.event(decoded("DP-1", 1, (2000, 1200)), &settings, &mut host);
        w.event(decoded("HDMI-A-1", 2, (2000, 1200)), &settings, &mut host);
        upload(&mut w, "DP-1", 5);
        // HDMI-A-1's bytes are still waiting for its frame.
        let mut off = settings.clone();
        off.duties.wallpaper = Duty::Off;
        host.busy_painter = true;
        w.apply(&settings, &off, &mut host);
        assert_eq!(host.destroyed, vec![SurfaceId(101), SurfaceId(102)]);
        assert!(w.outputs.iter().all(|o| o.surface.is_none()
            && o.texture.is_none()
            && o.incoming.is_none()
            && o.asked.is_none()));
        // The painter was busy: the texture is freed at the next chance.
        assert!(host.released.is_empty());
        assert_eq!(w.releases, vec![PixmapId(5)]);
        // A result asked before the duty went off is dropped.
        w.event(decoded("DP-1", 1, (2000, 1200)), &off, &mut host);
        assert_eq!(host.created.len(), 2);
        assert_eq!(host.released, Vec::<PixmapId>::new());

        host.busy_painter = false;
        host.requests.clear();
        w.apply(&off, &settings, &mut host);
        assert_eq!(host.released, vec![PixmapId(5)]);
        assert!(w.releases.is_empty());
        assert_eq!(
            host.checks(),
            vec![vec!["DP-1".to_owned(), "HDMI-A-1".to_owned()]]
        );
        assert!(host
            .requests
            .iter()
            .any(|c| matches!(c, WallpaperCommand::Rescan { .. })));
        assert_eq!(host.resolves().len(), 2);
    }

    #[test]
    fn the_surfaces_real_scale_asks_again_and_the_old_texture_stays() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        w.event(decoded("DP-1", 1, (2000, 1200)), &settings, &mut host);
        upload(&mut w, "DP-1", 9);
        let mut asked = Vec::new();
        // The first frame says 1.5, not the output's integer 2.
        w.observed(SurfaceId(101), (1000, 600), 1.5, &settings, &mut |c| {
            asked.push(c);
            true
        });
        assert!(matches!(
            asked.as_slice(),
            [WallpaperCommand::Resolve {
                device_px: (1500, 900),
                generation: 2,
                ..
            }]
        ));
        assert_eq!(output(&w, "DP-1").texture.map(|t| t.id), Some(PixmapId(9)));
        // The same size again asks nothing.
        w.observed(SurfaceId(101), (1000, 600), 1.5, &settings, &mut |c| {
            asked.push(c);
            true
        });
        assert_eq!(asked.len(), 1);
    }

    #[test]
    fn outputs_seen_before_the_worker_ran_are_asked_at_its_first_answer() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        host.refuse = true;
        w.added(&info(1, "DP-1"), &settings, &mut host);
        assert!(output(&w, "DP-1").asked.is_none());
        host.refuse = false;
        w.event(
            WallpaperEvent::Folder {
                names: vec!["a.png".into()],
            },
            &settings,
            &mut host,
        );
        assert_eq!(host.checks(), vec![vec!["DP-1".to_owned()]]);
        assert_eq!(host.resolves().len(), 1);
        // The same listing again asks nothing more; a changed one asks all.
        w.event(
            WallpaperEvent::Folder {
                names: vec!["a.png".into()],
            },
            &settings,
            &mut host,
        );
        assert_eq!(host.resolves().len(), 1);
        w.event(
            WallpaperEvent::Folder {
                names: vec!["a.png".into(), "b.png".into()],
            },
            &settings,
            &mut host,
        );
        assert_eq!(host.resolves().len(), 2);
    }

    #[test]
    fn a_mode_or_choice_change_asks_the_affected_outputs() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        w.added(&info(2, "HDMI-A-1"), &settings, &mut host);
        host.requests.clear();
        let mut chosen = settings.clone();
        chosen
            .wallpaper
            .outputs
            .insert("HDMI-A-1".into(), "b.png".into());
        w.apply(&settings, &chosen, &mut host);
        let names: Vec<String> = host.resolves().into_iter().map(|r| r.0).collect();
        assert_eq!(names, vec!["HDMI-A-1".to_owned()]);
        host.requests.clear();
        let mut fit = chosen.clone();
        fit.wallpaper.mode = Mode::Fit;
        w.apply(&chosen, &fit, &mut host);
        assert_eq!(host.resolves().len(), 2);
    }

    /// A settings file in a fresh directory, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("celestina-wallpaper-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("a test directory");
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            celestina_shell::settings::path_in(&self.0)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn shell_settings(scratch: &Scratch) -> ShellSettings {
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        let wanted = on();
        let written = shell.update_wallpaper(|w| *w = wanted.wallpaper.clone());
        assert!(matches!(written, Ok(true)));
        // The duty is turned on through the file, as the person would.
        let bytes = wanted.to_bytes().expect("bytes");
        std::fs::write(scratch.file(), bytes).expect("the file");
        assert!(shell.adopt(wanted));
        shell
    }

    #[test]
    fn a_verb_is_durable_before_it_is_applied() {
        let scratch = Scratch::new("durable");
        let mut shell = shell_settings(&scratch);
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        w.added(&info(1, "DP-1"), shell.current(), &mut host);
        host.requests.clear();
        host.file = Some(scratch.file());

        w.verb(
            &Verb::WallpaperSet {
                target: Target::All,
                file: "sea.png".into(),
            },
            &mut shell,
            &mut host,
        );
        assert_eq!(
            shell
                .current()
                .wallpaper
                .outputs
                .get("DP-1")
                .map(String::as_str),
            Some("sea.png")
        );
        // When the new picture was asked for, the file already held it.
        assert_eq!(host.resolves().len(), 1);
        assert!(host
            .file_at_request
            .iter()
            .all(|text| text.as_deref().is_some_and(|t| t.contains("sea.png"))));
        match host.requests.last() {
            Some(WallpaperCommand::Resolve { settings, .. }) => {
                assert_eq!(
                    settings.outputs.get("DP-1").map(String::as_str),
                    Some("sea.png")
                );
            }
            other => panic!("expected a resolve, got {other:?}"),
        }

        host.requests.clear();
        w.verb(&Verb::WallpaperMode(Mode::Center), &mut shell, &mut host);
        assert_eq!(shell.current().wallpaper.mode, Mode::Center);
        assert_eq!(host.resolves().len(), 1);
        let reread = ShellSettings::load_from(Some(scratch.file()));
        assert_eq!(reread.current().wallpaper.mode, Mode::Center);
    }

    #[test]
    fn a_verb_whose_write_fails_changes_nothing() {
        let mut shell = ShellSettings::load_from(None);
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        host.requests.clear();
        assert!(matches!(
            shell.update_wallpaper(|w| w.mode = Mode::Fit),
            Err(ShellSettingsError::NoConfigHome)
        ));
        w.verb(&Verb::WallpaperMode(Mode::Fit), &mut shell, &mut host);
        assert_eq!(shell.current().wallpaper.mode, Mode::Fill);
        assert!(host.requests.is_empty());
    }

    fn chosen(shell: &ShellSettings, output: &str) -> Option<String> {
        shell.current().wallpaper.outputs.get(output).cloned()
    }

    fn rescans(host: &Fake) -> usize {
        host.requests
            .iter()
            .filter(|c| matches!(c, WallpaperCommand::Rescan { .. }))
            .count()
    }

    fn listing() -> WallpaperEvent {
        WallpaperEvent::Folder {
            names: vec!["a.png".into(), "default.png".into(), "z.png".into()],
        }
    }

    #[test]
    fn next_and_prev_step_through_the_fresh_listing_from_the_current_picture() {
        let scratch = Scratch::new("step");
        let mut shell = shell_settings(&scratch);
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        w.added(&info(1, "DP-1"), shell.current(), &mut host);
        // Before the folder is read, a step asks for it and waits: it is not
        // a silent no-op.
        host.requests.clear();
        w.verb(&Verb::WallpaperNext(None), &mut shell, &mut host);
        assert!(matches!(
            host.requests.as_slice(),
            [WallpaperCommand::Rescan { .. }]
        ));
        assert_eq!(chosen(&shell, "DP-1"), None);
        // The listing arrives: the current picture is `default.png` (no
        // explicit choice), so next is `z.png`.
        w.news(listing(), &mut shell, &mut host);
        assert_eq!(chosen(&shell, "DP-1").as_deref(), Some("z.png"));

        // Every step reads the folder again, even with a listing in hand.
        let before = rescans(&host);
        w.verb(
            &Verb::WallpaperNext(Some("DP-1".into())),
            &mut shell,
            &mut host,
        );
        assert_eq!(rescans(&host), before + 1);
        assert_eq!(chosen(&shell, "DP-1").as_deref(), Some("z.png"));
        w.news(listing(), &mut shell, &mut host);
        assert_eq!(chosen(&shell, "DP-1").as_deref(), Some("a.png"));
        w.verb(&Verb::WallpaperPrev(None), &mut shell, &mut host);
        w.news(listing(), &mut shell, &mut host);
        assert_eq!(chosen(&shell, "DP-1").as_deref(), Some("z.png"));
        // A listing with no step waiting writes nothing.
        w.news(listing(), &mut shell, &mut host);
        assert_eq!(chosen(&shell, "DP-1").as_deref(), Some("z.png"));
    }

    #[test]
    fn of_several_steps_waiting_for_the_listing_the_newest_wins() {
        let scratch = Scratch::new("newest");
        let mut shell = shell_settings(&scratch);
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        w.added(&info(1, "DP-1"), shell.current(), &mut host);
        w.verb(&Verb::WallpaperNext(None), &mut shell, &mut host);
        w.verb(&Verb::WallpaperPrev(None), &mut shell, &mut host);
        w.news(listing(), &mut shell, &mut host);
        // One step, the prev: `default.png` back to `a.png`.
        assert_eq!(chosen(&shell, "DP-1").as_deref(), Some("a.png"));
    }

    #[test]
    fn a_step_over_an_empty_folder_writes_nothing() {
        let scratch = Scratch::new("empty");
        let mut shell = shell_settings(&scratch);
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        w.added(&info(1, "DP-1"), shell.current(), &mut host);
        w.verb(&Verb::WallpaperNext(None), &mut shell, &mut host);
        w.news(
            WallpaperEvent::Folder { names: vec![] },
            &mut shell,
            &mut host,
        );
        assert_eq!(chosen(&shell, "DP-1"), None);
        assert!(w.pending_step.is_none());
    }

    #[test]
    fn a_step_the_worker_cannot_take_uses_the_listing_in_hand() {
        let scratch = Scratch::new("refused-step");
        let mut shell = shell_settings(&scratch);
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        w.added(&info(1, "DP-1"), shell.current(), &mut host);
        // No listing and no worker: nothing to step over.
        host.refuse = true;
        w.verb(&Verb::WallpaperNext(None), &mut shell, &mut host);
        assert_eq!(chosen(&shell, "DP-1"), None);
        assert!(w.pending_step.is_none());
        host.refuse = false;
        let current = shell.current().clone();
        w.event(listing(), &current, &mut host);
        host.refuse = true;
        w.verb(&Verb::WallpaperNext(None), &mut shell, &mut host);
        assert_eq!(chosen(&shell, "DP-1").as_deref(), Some("z.png"));
    }

    #[test]
    fn a_verb_for_an_output_that_is_not_connected_writes_nothing() {
        let scratch = Scratch::new("unknown-output");
        let mut shell = shell_settings(&scratch);
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        w.added(&info(1, "DP-1"), shell.current(), &mut host);
        let file = std::fs::read(scratch.file()).expect("the file");
        host.requests.clear();
        w.verb(
            &Verb::WallpaperSet {
                target: Target::Output("HDMI-A-9".into()),
                file: "sea.png".into(),
            },
            &mut shell,
            &mut host,
        );
        w.verb(
            &Verb::WallpaperNext(Some("HDMI-A-9".into())),
            &mut shell,
            &mut host,
        );
        w.verb(
            &Verb::WallpaperPrev(Some("HDMI-A-9".into())),
            &mut shell,
            &mut host,
        );
        assert!(shell.current().wallpaper.outputs.is_empty());
        assert_eq!(std::fs::read(scratch.file()).expect("the file"), file);
        assert!(host.requests.is_empty());
        assert!(w.pending_step.is_none());

        // A step for an output that leaves while the listing is read writes
        // nothing either.
        w.added(&info(2, "HDMI-A-1"), shell.current(), &mut host);
        w.verb(
            &Verb::WallpaperNext(Some("HDMI-A-1".into())),
            &mut shell,
            &mut host,
        );
        w.removed(OutputId(2), &mut host);
        w.news(listing(), &mut shell, &mut host);
        assert!(shell.current().wallpaper.outputs.is_empty());
    }

    #[test]
    fn retry_checks_the_owners_again() {
        let mut shell = ShellSettings::load_from(None);
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        assert!(shell.adopt(settings));
        host.requests.clear();
        w.verb(&Verb::WallpaperRetry, &mut shell, &mut host);
        assert_eq!(host.checks(), vec![vec!["DP-1".to_owned()]]);
        assert_eq!(host.resolves().len(), 1);
    }

    #[test]
    fn a_surface_the_compositor_closed_is_made_again() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        w.event(decoded("DP-1", 1, (2000, 1200)), &settings, &mut host);
        upload(&mut w, "DP-1", 4);
        host.closed.push(SurfaceId(101));
        w.resolve(0, &settings, &mut host);
        w.event(decoded("DP-1", 2, (2000, 1200)), &settings, &mut host);
        // A new surface; the closed one is not destroyed again; its texture
        // is freed.
        assert_eq!(
            host.created,
            vec![(OutputId(1), SurfaceId(101)), (OutputId(1), SurfaceId(102))]
        );
        assert!(host.destroyed.is_empty());
        assert!(host.redrawn.is_empty());
        assert_eq!(host.released, vec![PixmapId(4)]);
        assert!(!w.owns(SurfaceId(101)));
        assert!(w.owns(SurfaceId(102)));
        assert!(output(&w, "DP-1").incoming.is_some());
    }

    #[test]
    fn a_closed_surface_found_on_an_output_change_asks_at_the_outputs_size() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        w.event(decoded("DP-1", 1, (2000, 1200)), &settings, &mut host);
        host.closed.push(SurfaceId(101));
        let mut wider = info(1, "DP-1");
        wider.logical_size = (1200, 600);
        w.changed(&wider, &settings, &mut host);
        assert!(host.redrawn.is_empty());
        assert_eq!(
            host.resolves().last(),
            Some(&("DP-1".to_owned(), (2400, 1200), 2))
        );
        assert!(!w.owns(SurfaceId(101)));
    }

    #[test]
    fn a_failed_upload_clears_the_output_when_its_failure_comes_back() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        w.event(decoded("DP-1", 1, (2000, 1200)), &settings, &mut host);
        let failed = output(&w, "DP-1")
            .upload_failed("texture too large")
            .expect("the output was asked");
        assert!(matches!(
            &failed,
            WallpaperEvent::Failed { output, generation: 1, why }
                if output == "DP-1" && why.contains("texture too large")
        ));
        w.event(failed, &settings, &mut host);
        assert_eq!(host.destroyed, vec![SurfaceId(101)]);
        assert!(!w.owns(SurfaceId(101)));
    }

    #[test]
    fn a_texture_is_drawn_one_to_one_or_stretched_over_the_surface() {
        // Decoded for this size: 1:1 on the buffer.
        let exact = placement((1500, 900), Some((1500, 900)), (1000.0, 600.0), 1.5);
        assert_eq!(
            (exact.x, exact.y, exact.w, exact.h),
            (0.0, 0.0, 1000.0, 600.0)
        );
        // Decoded at the integer estimate (2) on a 1.5 surface: stretched
        // over the surface, not drawn 1333 px wide from the corner.
        let old = placement((2000, 1200), Some((1500, 900)), (1000.0, 600.0), 1.5);
        assert_eq!((old.x, old.y, old.w, old.h), (0.0, 0.0, 1000.0, 600.0));
        // Smaller than the surface (scale went up): still covers it whole.
        let small = placement((1000, 600), Some((2000, 1200)), (1000.0, 600.0), 2.0);
        assert_eq!((small.w, small.h), (1000.0, 600.0));
    }

    #[test]
    fn the_real_scale_seen_on_the_output_asks_again_before_any_surface() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        assert_eq!(host.resolves(), vec![("DP-1".to_owned(), (2000, 1200), 1)]);
        let mut asked = Vec::new();
        // The Island's frame on DP-1 says 1.5.
        w.seen("DP-1", 1.5, &settings, &mut |c| {
            asked.push(c);
            true
        });
        assert!(matches!(
            asked.as_slice(),
            [WallpaperCommand::Resolve {
                device_px: (1500, 900),
                generation: 2,
                ..
            }]
        ));
        // The integer-scale answer is stale; seen again, nothing more.
        w.event(decoded("DP-1", 1, (2000, 1200)), &settings, &mut host);
        assert!(host.created.is_empty());
        w.seen("DP-1", 1.5, &settings, &mut |c| {
            asked.push(c);
            true
        });
        assert_eq!(asked.len(), 1);
        // An output change with the same integer scale keeps the real one,
        // and the same device size asks nothing.
        let resolves = host.resolves().len();
        w.changed(&info(1, "DP-1"), &settings, &mut host);
        assert_eq!(output(&w, "DP-1").scale, 1.5);
        assert_eq!(host.resolves().len(), resolves);
        // Once the surface exists, its own frames decide.
        let generation = w.generation;
        w.event(
            decoded("DP-1", generation, (1500, 900)),
            &settings,
            &mut host,
        );
        assert_eq!(host.created.len(), 1);
        w.seen("DP-1", 1.25, &settings, &mut |c| {
            asked.push(c);
            true
        });
        assert_eq!(asked.len(), 1);
        assert_eq!(output(&w, "DP-1").scale, 1.5);
    }

    #[test]
    fn a_new_integer_scale_replaces_a_learned_real_one() {
        let (mut w, mut host) = (Wallpapers::default(), Fake::default());
        let settings = on();
        w.added(&info(1, "DP-1"), &settings, &mut host);
        w.seen("DP-1", 1.5, &settings, &mut |_| true);
        let mut one = info(1, "DP-1");
        one.scale = 1.0;
        w.changed(&one, &settings, &mut host);
        assert_eq!(output(&w, "DP-1").scale, 1.0);
        assert_eq!(
            host.resolves().last(),
            Some(&("DP-1".to_owned(), (1000, 600), 3))
        );
    }
}
