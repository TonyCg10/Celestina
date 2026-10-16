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
/// name: the holder's process name `comm` with its unique bus name
/// `unique`, or the unique name alone
/// ("Another program serves the tray: noctalia (:1.42)").
#[must_use]
pub fn tray_owner(unique: &str, comm: Option<&str>) -> String {
    match comm {
        Some(comm) if !comm.is_empty() => {
            format!("Otro programa atiende la bandeja: {comm} ({unique})")
        }
        _ => format!("Otro programa atiende la bandeja: {unique}"),
    }
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

/// The launcher's field while it is empty
/// ("Search applications or type /").
pub const LAUNCHER_PLACEHOLDER: &str = "Buscar aplicaciones o escribir /";

/// The launcher's only line while nothing answers the query ("No results").
pub const LAUNCHER_EMPTY: &str = "Sin resultados";

/// The launcher command that opens the Notifications page, as typed.
pub const SWITCH_NOTIFICATIONS: &str = "/notificaciones";

/// The launcher command that turns do-not-disturb on or off, as typed.
pub const SWITCH_DO_NOT_DISTURB: &str = "/no-molestar";

/// The launcher command that opens the Activity page, as typed.
pub const SWITCH_ACTIVITY: &str = "/actividad";

/// The launcher command that opens the Tray page, as typed.
pub const SWITCH_TRAY: &str = "/bandeja";

/// What the notifications command does, beside it
/// ("Opens the Notifications page").
pub const SWITCH_NOTIFICATIONS_LABEL: &str = "Abre la página Notificaciones";

/// What the do-not-disturb command does, beside it
/// ("Turns Do not disturb on or off").
pub const SWITCH_DO_NOT_DISTURB_LABEL: &str = "Activa o desactiva No molestar";

/// What the activity command does, beside it ("Opens the Activity page").
pub const SWITCH_ACTIVITY_LABEL: &str = "Abre la página Actividad";

/// What the tray command does, beside it ("Opens the Tray page").
pub const SWITCH_TRAY_LABEL: &str = "Abre la página Bandeja";
// Banners (CEL-1-F).

/// The name of the small button that dismisses one banner; the button
/// shows only its icon (the suite has no tooltips), so this is the name an
/// assistive technology reads ("Dismiss").
pub const BANNER_DISMISS: &str = "Descartar";

// The Notifications page's rows (CEL-1-F).

/// The button above the live notifications that dismisses all of them
/// ("Dismiss all").
pub const NOTIFICATIONS_DISMISS_ALL: &str = "Descartar todo";

/// The Tray page's title for an item that has none ("Untitled").
pub const TRAY_ITEM_UNTITLED: &str = "Sin título";

/// The Tray page's status word for an item that wants attention
/// ("Needs attention").
pub const TRAY_NEEDS_ATTENTION: &str = "Requiere atención";

/// The Tray page's status word for a passive item ("Inactive").
pub const TRAY_PASSIVE: &str = "Inactivo";
