// language-contract: product-copy
//! Every word of the shell a person reads, in Spanish (ADR 0007 of the suite,
//! `docs/decisions/0007-spanish-product-copy.md`). The string literals of
//! this file are the product copy; the comments and identifiers around them
//! are development truth, in English. No other file of the shell holds a
//! Spanish string: the views and the tests name these items.

/// Shown in the capsule, instead of the workspace dots, until the compositor
/// reports this output's workspaces (no niri, or the socket is gone).
pub const WORKSPACES_FALLBACK: &str = "Espacios";

/// A workspace without a name, by its index on its output ("Workspace 3").
#[must_use]
pub fn workspace_number(idx: u8) -> String {
    format!("Espacio {idx}")
}

/// The title of the Activity page (workspaces and windows).
pub const PAGE_ACTIVITY: &str = "Actividad";

/// The title of the Notifications page.
pub const PAGE_NOTIFICATIONS: &str = "Notificaciones";

/// The title of the Tray page.
pub const PAGE_TRAY: &str = "Bandeja";

/// The label of the button that closes the open page ("Close").
pub const CLOSE: &str = "Cerrar";

/// The label of the overflow menu button ("More").
pub const MORE: &str = "Más";
