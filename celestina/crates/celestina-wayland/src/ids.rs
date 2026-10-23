//! Plain value types that name things on a surface: the surface itself, a
//! popup and the reason a popup ended. They carry no protocol object, so the
//! input events both profiles deliver can mention them (`InputEvent::PopupClosed`)
//! whether or not the `gl` feature compiles the shell runtime.
#![forbid(unsafe_code)]

/// A layer surface or a popup surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub u32);

/// A popup, by the id `SurfaceCtl::open_popup` returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PopupId(pub u32);

/// Why a popup is gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    /// The popup's shell returned `PopupAction::Close`.
    Shell,
    /// The parent shell called `close_popup`, or the parent popup closed
    /// (a nested popup closes before its parent, for whatever reason the
    /// parent closes).
    Parent,
    /// The compositor sent `popup_done`.
    Dismissed,
    /// The parent opened another popup.
    Replaced,
    /// The parent surface went away.
    ParentGone,
    /// The popup could not be created or mapped.
    Failed,
    /// The runtime is shutting down.
    Exit,
}
