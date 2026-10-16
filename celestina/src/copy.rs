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

/// The columns of a workspace, after its name ("3 columns"); one is
/// singular.
#[must_use]
pub fn column_count(n: u16) -> String {
    if n == 1 {
        "1 columna".to_owned()
    } else {
        format!("{n} columnas")
    }
}

/// The title of a window that has neither a title nor an application id
/// ("Untitled").
pub const WINDOW_UNTITLED: &str = "Sin título";

/// The Activity page's only row while the compositor has sent nothing for
/// this output ("No compositor data").
pub const ACTIVITY_EMPTY: &str = "Sin datos del compositor";

/// The label of the do-not-disturb switch on the Notifications page
/// ("Do not disturb").
pub const DO_NOT_DISTURB: &str = "No molestar";

/// The Notifications page's only row while nothing holds the
/// notifications name and no history exists yet ("No notifications").
pub const NOTIFICATIONS_EMPTY: &str = "Sin notificaciones";

/// The Tray page's only row while the tray has no items ("No tray items").
pub const TRAY_EMPTY: &str = "Sin elementos en la bandeja";

/// The Notifications page's explanation while another program holds the
/// notifications name; `owner` describes the holder
/// ("Another program serves the notifications: {owner}").
#[must_use]
pub fn notifications_owner(owner: &str) -> String {
    format!("Otro programa atiende las notificaciones: {owner}")
}

/// The Tray page's explanation while another program holds the tray's
/// name; `owner` describes the holder
/// ("Another program serves the tray: {owner}").
#[must_use]
pub fn tray_owner(owner: &str) -> String {
    format!("Otro programa atiende la bandeja: {owner}")
}

/// The overflow menu's row that grows the open page again from the capsule
/// ("Reopen the page").
pub const MENU_REOPEN_PAGE: &str = "Reabrir la página";

/// The overflow menu's row that ends the shell ("Quit").
pub const MENU_QUIT: &str = "Salir";

/// Shown in place of the do-not-disturb switch in the overflow menu while
/// another program holds the notifications name
/// ("Another program handles it").
pub const DO_NOT_DISTURB_HELD: &str = "Lo gestiona otro programa";
