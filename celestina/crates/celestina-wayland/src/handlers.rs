//! The sctk handler impls of `Runtime`: compositor, output, layer shell,
//! xdg popup, seat, keyboard, pointer, background effect and registry. They
//! translate protocol events into runtime calls and end each with a flush.
#![forbid(unsafe_code)]

use smithay_client_toolkit::{
    background_effect::{BackgroundEffectHandler, BackgroundEffectState},
    compositor::CompositorHandler,
    delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers as SctkModifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
        xdg::popup::{Popup, PopupConfigure, PopupHandler},
        WaylandSurface,
    },
};
use wayland_client::{
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
    Connection, QueueHandle,
};

use crate::{
    input::{InputEvent, Modifiers},
    runtime::{OutputEvent, Runtime, Shell, SurfaceId, Target},
    surface::PreferredScaleSink,
};

impl<S: Shell> PreferredScaleSink for Runtime<S> {
    fn preferred_scale(&mut self, id: SurfaceId, preferred: u32) {
        if let Some(surface) = self.core.surface_mut(id) {
            surface.set_preferred_scale(preferred);
        }
        self.flush();
    }
}

impl<S: Shell> CompositorHandler for Runtime<S> {
    /// Integer output scale: ignored, `wp_fractional_scale_v1` (required)
    /// drives the scale.
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        done_ms: u32,
    ) {
        let Some(target) = self.core.target_by_wl(surface) else {
            return;
        };
        let id = self.core.surface_at(target).id;
        if let Some(metrics) = &mut self.core.metrics {
            metrics.on_callback(id, std::time::Instant::now(), done_ms);
        }
        if self.core.surface_at(target).pacing_mut().frame_done() {
            self.draw(target);
        } else {
            tracing::trace!(
                surface = id.0,
                "frame callback without redraw request: sleeping"
            );
        }
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        let name = self.core.output_state.info(output).and_then(|i| i.name);
        tracing::debug!(output = ?name, "surface entered output");
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl<S: Shell> OutputHandler for Runtime<S> {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.core.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if let Some(info) = self.core.output_info(&output) {
            tracing::info!(output = %info.name, id = info.id.0, size = ?info.logical_size, scale = info.scale, refresh_mhz = info.refresh_mhz, "output added");
            self.output_event(OutputEvent::Added(info));
        }
    }

    fn update_output(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.core.output_info(&output) {
            tracing::info!(output = %info.name, size = ?info.logical_size, scale = info.scale, refresh_mhz = info.refresh_mhz, "output changed");
            self.output_event(OutputEvent::Changed(info));
        }
    }

    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.core.output_info(&output) {
            tracing::info!(output = %info.name, "output removed");
            self.output_destroyed(info.id);
        }
    }
}

impl<S: Shell> LayerShellHandler for Runtime<S> {
    /// The compositor closed the surface (its output is going away): its
    /// popups and the surface are destroyed; the product learns through
    /// `OutputEvent::Removed`, which follows.
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        if let Some(idx) = self.core.surface_index_by_wl(layer.wl_surface()) {
            let id = self.core.surfaces[idx].id;
            tracing::warn!(surface = id.0, "layer surface closed by the compositor");
            self.core.close_popups_of(id);
            self.core.destroy_layer_surface(id);
        }
        self.flush();
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        serial: u32,
    ) {
        let Some(idx) = self.core.surface_index_by_wl(layer.wl_surface()) else {
            return;
        };
        tracing::debug!(surface = self.core.surfaces[idx].id.0, size = ?configure.new_size, serial, "configure");
        match self.core.surfaces[idx].configure(configure.new_size, &self.core.egl) {
            Ok(true) => self.draw(Target::Layer(idx)),
            Ok(false) => self.flush(),
            Err(err) => {
                tracing::error!(%err, "cannot create the EGL window, exiting");
                self.core.signal.stop();
            }
        }
    }
}

impl<S: Shell> PopupHandler for Runtime<S> {
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        popup: &Popup,
        config: PopupConfigure,
    ) {
        self.popup_configure(popup, config);
    }

    fn done(&mut self, _: &Connection, _: &QueueHandle<Self>, popup: &Popup) {
        tracing::info!("popup: popup_done from the compositor");
        self.popup_done(popup);
    }
}

impl<S: Shell> SeatHandler for Runtime<S> {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.core.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Keyboard if self.core.keyboard.is_none() => {
                match self.core.seat_state.get_keyboard(qh, &seat, None) {
                    Ok(keyboard) => self.core.keyboard = Some(keyboard),
                    Err(err) => tracing::warn!(%err, "cannot create the keyboard"),
                }
            }
            Capability::Pointer if self.core.pointer.is_none() => {
                match self.core.seat_state.get_pointer(qh, &seat) {
                    Ok(pointer) => {
                        self.core.pointer_added(pointer);
                        // The popup grab goes to the seat whose pointer
                        // pressed.
                        self.core.seat = Some(seat);
                    }
                    Err(err) => tracing::warn!(%err, "cannot create the pointer"),
                }
            }
            _ => {}
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Keyboard => {
                if let Some(keyboard) = self.core.keyboard.take() {
                    keyboard.release();
                }
            }
            Capability::Pointer => self.core.pointer_removed(),
            _ => {}
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl<S: Shell> KeyboardHandler for Runtime<S> {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        let target = self.core.target_by_wl(surface);
        self.core.keyboard_focus = target.map(|target| self.core.surface_at(target).id);
        tracing::debug!(focus = ?self.core.keyboard_focus, "keyboard entered");
        if let Some(Target::Layer(idx)) = target {
            let id = self.core.surfaces[idx].id;
            self.dispatch_input(id, InputEvent::FocusEnter);
        }
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        match self.core.target_by_wl(surface) {
            Some(Target::Layer(idx)) => {
                let id = self.core.surfaces[idx].id;
                self.core.keyboard_focus = None;
                if self.core.has_popup(id) {
                    // The popup grab moved the keyboard into our own popup:
                    // not a focus loss for the page.
                    tracing::debug!(surface = id.0, "keyboard moved into the popup");
                } else {
                    self.dispatch_input(id, InputEvent::FocusLost);
                }
            }
            Some(Target::Popup(_)) => self.core.keyboard_focus = None,
            None => {}
        }
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        // Before dispatch: Enter on a menu button opens the menu inside this
        // call, grabbing with this serial.
        self.core.grab_serial.key_press(serial);
        self.key_event(event, true, serial);
    }

    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        self.key_event(event, true, serial);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        self.key_event(event, false, serial);
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: SctkModifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.core.modifiers = Modifiers {
            ctrl: modifiers.ctrl,
            alt: modifiers.alt,
            shift: modifiers.shift,
            logo: modifiers.logo,
        };
    }
}

impl<S: Shell> Runtime<S> {
    /// Keys go to the open popup first (whether the keyboard is on the
    /// popup or still on its parent), else to the focused surface.
    fn key_event(&mut self, event: KeyEvent, pressed: bool, serial: u32) {
        let Some(id) = self.core.keyboard_focus else {
            return;
        };
        let ev = InputEvent::Key {
            keysym: event.keysym.raw(),
            utf8: event.utf8,
            modifiers: self.core.modifiers,
            pressed,
            serial,
        };
        match self.core.popup_for_focus(id) {
            Some(idx) => self.dispatch_popup_input(idx, ev),
            None => self.dispatch_input(id, ev),
        }
    }
}

impl<S: Shell> PointerHandler for Runtime<S> {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if let PointerEventKind::Press { serial, .. } = event.kind {
                self.core.grab_serial.pointer_press(serial);
            }
            let Some(target) = self.core.target_by_wl(&event.surface) else {
                continue;
            };
            let (x, y) = (event.position.0 as f32, event.position.1 as f32);
            let ev = match event.kind {
                PointerEventKind::Enter { .. } => {
                    self.core.pointer_entered();
                    InputEvent::PointerMotion { x, y }
                }
                PointerEventKind::Motion { .. } => InputEvent::PointerMotion { x, y },
                PointerEventKind::Press { button, serial, .. } => InputEvent::PointerPress {
                    x,
                    y,
                    button,
                    serial,
                },
                PointerEventKind::Release { button, serial, .. } => InputEvent::PointerRelease {
                    x,
                    y,
                    button,
                    serial,
                },
                PointerEventKind::Leave { .. } => InputEvent::PointerLeave,
                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    if horizontal.absolute == 0.0 && vertical.absolute == 0.0 {
                        // Source, stop and discrete-only events carry no
                        // movement of their own.
                        continue;
                    }
                    InputEvent::Axis {
                        x,
                        y,
                        horizontal: horizontal.absolute as f32,
                        vertical: vertical.absolute as f32,
                    }
                }
            };
            match target {
                Target::Layer(idx) => {
                    let id = self.core.surfaces[idx].id;
                    self.dispatch_input(id, ev);
                }
                Target::Popup(idx) => self.dispatch_popup_input(idx, ev),
            }
        }
    }
}

impl<S: Shell> BackgroundEffectHandler for Runtime<S> {
    fn background_effect_state(&mut self) -> &mut BackgroundEffectState {
        self.core.blur.state_mut()
    }

    fn update_capabilities(&mut self) {
        tracing::debug!(capabilities = ?self.core.blur.state_mut().capabilities(), "ext_background_effect_v1 capabilities");
    }
}

impl<S: Shell> ProvidesRegistryState for Runtime<S> {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.core.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

delegate_registry!(@<S: Shell> Runtime<S>);
smithay_client_toolkit::delegate_dispatch2!(@<S: Shell> Runtime<S>);
