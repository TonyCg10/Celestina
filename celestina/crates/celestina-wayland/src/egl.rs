//! EGL display, config and the single shared GLES 3 context, plus one EGL
//! window per surface. This is the only module of the shell allowed to use
//! `unsafe` (the design's §5.2 records the author's approval); every block
//! states the invariant it relies on.
#![allow(unsafe_code)]

use std::ffi::c_void;

use glow::HasContext;
use khronos_egl as egl;
use wayland_client::{protocol::wl_surface::WlSurface, Connection, Proxy};
use wayland_egl::WlEglSurface;

/// `EGL_PLATFORM_WAYLAND_KHR` (same value as the EXT alias); not exported by
/// the `khronos-egl` crate.
const EGL_PLATFORM_WAYLAND_KHR: egl::Enum = 0x31D8;

type Api = egl::DynamicInstance<egl::EGL1_5>;

/// What can go wrong between loading libEGL and swapping a buffer.
#[derive(Debug, thiserror::Error)]
pub enum EglError {
    #[error("cannot load libEGL")]
    Load(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("{call} failed")]
    Call {
        call: &'static str,
        #[source]
        source: egl::Error,
    },
    #[error("no RGBA8 GLES3 window EGL config with an 8-bit stencil")]
    NoConfig,
    #[error("wl_egl_window_create failed")]
    Window(#[source] wayland_egl::Error),
    #[error("cannot create femtovg's OpenGL renderer")]
    Renderer(#[source] femtovg::ErrorKind),
}

fn call(call: &'static str) -> impl FnOnce(egl::Error) -> EglError {
    move |source| EglError::Call { call, source }
}

/// The process-wide EGL state: one display, one config, one GLES 3 context
/// and the `glow` loader bound to it.
pub struct Egl {
    api: Api,
    display: egl::Display,
    config: egl::Config,
    context: egl::Context,
    gl: glow::Context,
}

/// A `wl_egl_window` plus its `EGLSurface`, sized to the buffer.
pub struct EglWindow {
    wl_egl: WlEglSurface,
    surface: egl::Surface,
}

impl Egl {
    /// Loads libEGL dynamically, opens the Wayland platform display and
    /// creates the shared GLES 3.0 context (RGBA 8/8/8/8, 8-bit stencil,
    /// window surfaces).
    pub fn new(conn: &Connection) -> Result<Self, EglError> {
        // SAFETY: loading libEGL.so.1 from the system; the EGL ABI is what the
        // crate expects.
        let api = unsafe { Api::load_required() }.map_err(|e| EglError::Load(Box::new(e)))?;

        let native = conn.backend().display_ptr().cast::<c_void>();
        // SAFETY: `native` is the live `wl_display*` of `conn`, which outlives
        // this struct (the runtime keeps the connection until shutdown).
        let display = unsafe {
            api.get_platform_display(EGL_PLATFORM_WAYLAND_KHR, native, &[egl::ATTRIB_NONE])
        }
        .map_err(call("eglGetPlatformDisplay(EGL_PLATFORM_WAYLAND_KHR)"))?;
        let (major, minor) = api.initialize(display).map_err(call("eglInitialize"))?;
        api.bind_api(egl::OPENGL_ES_API)
            .map_err(call("eglBindAPI(OPENGL_ES)"))?;

        let config_attribs = [
            egl::SURFACE_TYPE,
            egl::WINDOW_BIT,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_ES3_BIT,
            egl::RED_SIZE,
            8,
            egl::GREEN_SIZE,
            8,
            egl::BLUE_SIZE,
            8,
            egl::ALPHA_SIZE,
            8,
            // femtovg fills concave paths through the stencil buffer; without
            // one every such fill degrades to its bounding box.
            egl::STENCIL_SIZE,
            8,
            egl::NONE,
        ];
        let config = api
            .choose_first_config(display, &config_attribs)
            .map_err(call("eglChooseConfig"))?
            .ok_or(EglError::NoConfig)?;

        let context_attribs = [
            egl::CONTEXT_MAJOR_VERSION,
            3,
            egl::CONTEXT_MINOR_VERSION,
            0,
            egl::NONE,
        ];
        let context = api
            .create_context(display, config, None, &context_attribs)
            .map_err(call("eglCreateContext(GLES 3.0)"))?;

        // `glow` reads GL_VERSION while loading, so the context must be current;
        // there is no surface yet, hence EGL_KHR_surfaceless_context.
        api.make_current(display, None, None, Some(context))
            .map_err(call(
                "eglMakeCurrent without a surface (EGL_KHR_surfaceless_context)",
            ))?;
        // SAFETY: eglGetProcAddress resolves GL entry points through libglvnd;
        // the returned pointers are only used while this context is current.
        let gl = unsafe { glow::Context::from_loader_function(|name| proc_address(&api, name)) };

        let vendor = api
            .query_string(Some(display), egl::VENDOR)
            .map(|s| s.to_string_lossy().into_owned());
        tracing::info!(egl = format!("{major}.{minor}"), vendor = ?vendor.ok(), "EGL initialised, GLES 3.0 context created");

        Ok(Self {
            api,
            display,
            config,
            context,
            gl,
        })
    }

    /// `eglGetProcAddress`, for loaders (femtovg) that build their own table.
    pub fn proc_address(&self, name: &str) -> *const c_void {
        proc_address(&self.api, name)
    }

    /// Creates the `wl_egl_window` and `EGLSurface` for `surface`, makes the
    /// shared context current on it and sets swap interval 0.
    pub fn create_window(
        &self,
        surface: &WlSurface,
        buffer: (u32, u32),
    ) -> Result<EglWindow, EglError> {
        let (w, h) = (buffer.0 as i32, buffer.1 as i32);
        let wl_egl = WlEglSurface::new(surface.id(), w, h).map_err(EglError::Window)?;
        // SAFETY: `wl_egl.ptr()` is a valid `wl_egl_window*` for the Wayland
        // platform of `self.display`, alive as long as `wl_egl` (owned below).
        let egl_surface = unsafe {
            self.api
                .create_window_surface(self.display, self.config, wl_egl.ptr().cast_mut(), None)
        }
        .map_err(call("eglCreateWindowSurface"))?;
        let window = EglWindow {
            wl_egl,
            surface: egl_surface,
        };
        self.make_current(&window, buffer)?;
        self.api
            .swap_interval(self.display, 0)
            .map_err(call("eglSwapInterval(0)"))?;
        Ok(window)
    }

    /// Binds the shared context to `window` for drawing and sets the GL
    /// viewport to the buffer size.
    pub fn make_current(&self, window: &EglWindow, buffer: (u32, u32)) -> Result<(), EglError> {
        self.api
            .make_current(
                self.display,
                Some(window.surface),
                Some(window.surface),
                Some(self.context),
            )
            .map_err(call("eglMakeCurrent"))?;
        // SAFETY: the context is current on `window`; plain state call.
        unsafe { self.gl.viewport(0, 0, buffer.0 as i32, buffer.1 as i32) };
        Ok(())
    }

    /// Presents the back buffer; this is the `wl_surface.commit`.
    pub fn swap_buffers(&self, window: &EglWindow) -> Result<(), EglError> {
        self.api
            .swap_buffers(self.display, window.surface)
            .map_err(call("eglSwapBuffers"))
    }

    /// Destroys the EGL surface and the `wl_egl_window`.
    pub fn destroy_window(&self, window: EglWindow) {
        if self.api.get_current_surface(egl::DRAW) == Some(window.surface) {
            if let Err(err) = self
                .api
                .make_current(self.display, None, None, Some(self.context))
            {
                tracing::warn!(?err, "eglMakeCurrent without a surface failed");
            }
        }
        if let Err(err) = self.api.destroy_surface(self.display, window.surface) {
            tracing::warn!(?err, "eglDestroySurface failed");
        }
        drop(window.wl_egl);
    }

    /// Releases the context and terminates the display. Call after every
    /// window has been destroyed.
    pub fn shutdown(self) {
        if let Err(err) = self.api.make_current(self.display, None, None, None) {
            tracing::warn!(?err, "eglMakeCurrent(none) failed");
        }
        if let Err(err) = self.api.destroy_context(self.display, self.context) {
            tracing::warn!(?err, "eglDestroyContext failed");
        }
        if let Err(err) = self.api.terminate(self.display) {
            tracing::warn!(?err, "eglTerminate failed");
        }
        tracing::debug!("EGL terminated");
    }
}

impl EglWindow {
    /// Resizes the `wl_egl_window` to the new buffer size; the next swap
    /// allocates buffers of that size.
    pub fn resize(&self, buffer: (u32, u32)) {
        self.wl_egl.resize(buffer.0 as i32, buffer.1 as i32, 0, 0);
    }
}

/// Builds femtovg's OpenGL renderer on the current GLES 3 context, resolving
/// entry points through `proc_address` (`eglGetProcAddress`). The painter
/// crate calls it from `Shell::frame` and forbids `unsafe` itself.
pub fn femtovg_renderer(
    proc_address: &dyn Fn(&str) -> *const c_void,
) -> Result<femtovg::renderer::OpenGl, EglError> {
    // SAFETY: the runtime calls this from `Shell::frame`, where the shared
    // GLES 3 context is current on this thread, and `proc_address` resolves
    // entry points of that same context. femtovg keeps its own `glow` table
    // and the renderer is only used while that context is current.
    unsafe { femtovg::renderer::OpenGl::new_from_function(|name| proc_address(name)) }
        .map_err(EglError::Renderer)
}

/// `GL_MAX_TEXTURE_SIZE` of the current GLES 3 context, the longest texture
/// side it accepts; `None` when the driver answers a non-positive value.
/// Called once, beside [`femtovg_renderer`], so the painter can refuse an
/// oversize texture with a typed error before the driver does.
pub fn max_texture_side(proc_address: &dyn Fn(&str) -> *const c_void) -> Option<u32> {
    // SAFETY: same invariant as `femtovg_renderer`: called from
    // `Shell::frame` with the shared GLES 3 context current on this thread,
    // and `proc_address` resolves entry points of that context. The loader
    // table is dropped before returning, so no pointer outlives the call.
    let gl = unsafe { glow::Context::from_loader_function(|name| proc_address(name)) };
    // SAFETY: `glGetIntegerv(GL_MAX_TEXTURE_SIZE)` only reads a context
    // limit; the context is current (above).
    let side = unsafe { gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE) };
    u32::try_from(side).ok().filter(|&side| side > 0)
}

fn proc_address(api: &Api, name: &str) -> *const c_void {
    match api.get_proc_address(name) {
        Some(f) => f as *const c_void,
        None => std::ptr::null(),
    }
}
