//! The seam between the machine and whatever draws (design §4.6.1).
//!
//! The machine decides *what* the screen says ([`Scene`]); a [`Presenter`]
//! makes pixels of it. The shipped presenter is the CPU painter
//! ([`crate::view::CpuPresenter`]); [`NullPresenter`] draws nothing and leaves
//! the toolkit's black covers as the whole display (tests, and the shape of a
//! painter that cannot start). The scene holds how many characters were typed and
//! never which, so a presenter cannot draw, log or leak the secret.
#![forbid(unsafe_code)]

use celestina_wayland::lock::LockCtl;

pub use crate::backdrop::WakeLoop;

use crate::machine::Scene;

/// Draws a scene. Called only after `locked`, and only when the scene changed.
pub trait Presenter {
    /// Shows `scene`. A presenter that cannot draw keeps the covers (the
    /// field still works blind) and logs; it never fails the lock.
    fn render(&mut self, scene: &Scene, ctl: &mut LockCtl<'_>);

    /// Gives the presenter what to call, from any thread, when something it
    /// draws arrived (the `backdrop` worker's base): the loop then asks for a
    /// redraw. A presenter with no worker ignores it.
    fn set_wake(&mut self, _wake: WakeLoop) {}
}

/// Draws nothing: the covers stay as the toolkit committed them.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullPresenter;

impl Presenter for NullPresenter {
    fn render(&mut self, _scene: &Scene, _ctl: &mut LockCtl<'_>) {}
}
