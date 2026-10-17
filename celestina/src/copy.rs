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

/// The accessible name of the bar's Workspaces piece ("Workspaces"). The
/// Notifications and Tray pieces are named by their pages' titles.
pub const PIECE_WORKSPACES: &str = "Espacios de trabajo";

/// The accessible name of a workspace in the bar: its label and, when it is
/// the one shown on the output, that it is the current one.
#[must_use]
pub fn bar_workspace_name(label: &str, active: bool) -> String {
    if active {
        format!("{label}, actual")
    } else {
        label.to_owned()
    }
}

/// The accessible state of the bar's notifications bell: unread
/// notifications and whether do-not-disturb is on.
#[must_use]
pub fn bar_notifications_state(unread: bool, do_not_disturb: bool) -> &'static str {
    match (unread, do_not_disturb) {
        (true, true) => "Notificaciones, sin leer, no molestar activado",
        (true, false) => "Notificaciones, sin leer",
        (false, true) => "Notificaciones, no molestar activado",
        (false, false) => "Notificaciones",
    }
}

/// The accessible name of a tray item in the bar: its title (or the
/// untitled word) and, when it is not simply active, its status.
#[must_use]
pub fn bar_tray_item_name(title: &str, needs_attention: bool, passive: bool) -> String {
    let title = if title.is_empty() {
        TRAY_ITEM_UNTITLED
    } else {
        title
    };
    if needs_attention {
        format!("{title}, {}", TRAY_NEEDS_ATTENTION.to_lowercase())
    } else if passive {
        format!("{title}, {}", TRAY_PASSIVE.to_lowercase())
    } else {
        title.to_owned()
    }
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

// Glance: the system monitor, the weather and the calendar (CEL-2-K).

/// The Notifications tab's short label ("Notif."): the full title does not
/// fit one of five equal pills, and the toolkit has no ellipsis.
pub const TAB_NOTIFICATIONS: &str = "Notific.";

/// The title of the Clock page ("Clock").
pub const PAGE_CLOCK: &str = "Reloj";

/// The title of the System monitor page ("System").
pub const PAGE_SYSMON: &str = "Sistema";

/// The accessible name of the bar's weather piece ("Weather").
pub const PIECE_WEATHER: &str = "Tiempo";

/// The launcher command that opens the Clock page, as typed.
pub const SWITCH_CLOCK: &str = "/reloj";

/// The launcher command that opens the System page, as typed.
pub const SWITCH_SYSMON: &str = "/sistema";

/// What the clock command does, beside it ("Opens the Clock page").
pub const SWITCH_CLOCK_LABEL: &str = "Abre la página Reloj";

/// What the system command does, beside it ("Opens the System page").
pub const SWITCH_SYSMON_LABEL: &str = "Abre la página Sistema";

/// The calendar's button to the previous month ("Previous"). The surface has
/// no accessible-name channel beyond the visible text, so the word stays.
pub const CALENDAR_PREVIOUS: &str = "Anterior";

/// The calendar's button back to the current month ("Today").
pub const CALENDAR_TODAY: &str = "Hoy";

/// The calendar's button to the next month ("Next").
pub const CALENDAR_NEXT: &str = "Siguiente";

/// The Clock page's only row while the shell has no date: the date comes from
/// the bar's clock, so it says what to turn on ("No date: turn the bar on").
pub const CLOCK_NO_DATE: &str = "Sin fecha: activa la barra para verla.";

/// The System page's only row while no reading exists: the sampler runs only
/// for a placed piece ("No data: place the sysmon piece in the bar").
pub const SYSMON_EMPTY: &str = "Sin datos: coloca la pieza «sysmon» en la barra.";

/// The label of the processor's meter ("CPU").
pub const SYSMON_CPU: &str = "CPU";

/// The label of the memory's meter ("RAM").
pub const SYSMON_RAM: &str = "RAM";

/// The label of the processor temperature row ("Temperature").
pub const SYSMON_TEMPERATURE: &str = "Temperatura";

/// The label of the System page's row that opens the resource monitor
/// ("Open the system monitor").
pub const SYSMON_OPEN_MONITOR: &str = "Abrir el monitor del sistema";

/// The weekday names, Monday first, three letters.
const WEEKDAYS_SHORT: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];

/// The weekday initials the calendar's header shows, Monday first.
const WEEKDAYS_INITIAL: [&str; 7] = ["L", "M", "X", "J", "V", "S", "D"];

/// The weekday names, Monday first.
const WEEKDAYS_FULL: [&str; 7] = [
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
    "domingo",
];

/// The month names, January first.
const MONTHS_FULL: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

/// The month abbreviations, January first.
const MONTHS_SHORT: [&str; 12] = [
    "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
];

/// The three-letter name of weekday `weekday` (0 = Monday); empty outside
/// 0-6.
#[must_use]
pub fn weekday_short(weekday: u8) -> &'static str {
    WEEKDAYS_SHORT
        .get(usize::from(weekday))
        .copied()
        .unwrap_or("")
}

/// The initial of weekday `weekday` (0 = Monday); empty outside 0-6.
#[must_use]
pub fn weekday_initial(weekday: u8) -> &'static str {
    WEEKDAYS_INITIAL
        .get(usize::from(weekday))
        .copied()
        .unwrap_or("")
}

/// The full name of weekday `weekday` (0 = Monday); empty outside 0-6.
#[must_use]
pub fn weekday_full(weekday: u8) -> &'static str {
    WEEKDAYS_FULL
        .get(usize::from(weekday))
        .copied()
        .unwrap_or("")
}

/// The full name of month `month` (1 = January); empty outside 1-12.
#[must_use]
pub fn month_full(month: u8) -> &'static str {
    usize::from(month)
        .checked_sub(1)
        .and_then(|i| MONTHS_FULL.get(i))
        .copied()
        .unwrap_or("")
}

/// The abbreviation of month `month` (1 = January); empty outside 1-12.
#[must_use]
pub fn month_short(month: u8) -> &'static str {
    usize::from(month)
        .checked_sub(1)
        .and_then(|i| MONTHS_SHORT.get(i))
        .copied()
        .unwrap_or("")
}

/// The bar's short date: the weekday and the day ("fri 17").
#[must_use]
pub fn short_date(weekday: u8, day: u8) -> String {
    format!("{} {day}", weekday_short(weekday))
}

/// The calendar's title for a month ("February 2024").
#[must_use]
pub fn month_title(year: i32, month: u8) -> String {
    let name = month_full(month);
    let mut chars = name.chars();
    let head: String = chars
        .next()
        .into_iter()
        .flat_map(char::to_uppercase)
        .collect();
    format!("{head}{} de {year}", chars.as_str())
}

/// The bar's system meter ("CPU 23% · RAM 41%").
#[must_use]
pub fn bar_sysmon(cpu: u8, ram: u8) -> String {
    format!("{SYSMON_CPU} {cpu}% · {SYSMON_RAM} {ram}%")
}

/// A percentage ("23%").
#[must_use]
pub fn percent(value: u8) -> String {
    format!("{value}%")
}

/// A size in KiB as GiB with one decimal and a decimal comma ("3,9 GiB").
#[must_use]
pub fn gib(kib: u64) -> String {
    // Tenths of a GiB, rounded to nearest.
    let tenths = (u128::from(kib) * 10 + 524_288) / 1_048_576;
    format!("{},{} GiB", tenths / 10, tenths % 10)
}

/// The memory row's value: used of total ("6,2 of 15,6 GiB").
#[must_use]
pub fn memory_usage(used_kib: u64, total_kib: u64) -> String {
    let total = gib(total_kib);
    let used = gib(used_kib);
    let used = used.strip_suffix(" GiB").unwrap_or(&used);
    format!("{used} de {total}")
}

/// A temperature in whole degrees Celsius ("62 °C").
#[must_use]
pub fn celsius(degrees: i16) -> String {
    format!("{degrees} °C")
}

/// The bar's weather: the temperature in whole degrees ("12°").
#[must_use]
pub fn bar_weather(degrees: i16) -> String {
    format!("{degrees}°")
}

/// The words for an Open-Meteo weather code ("Clear"). Only the codes the
/// service defines have words; every other number, the shell's unknown code
/// among them, is "Undetermined".
#[must_use]
pub fn weather_condition(code: u8) -> &'static str {
    match code {
        0 => "Despejado",
        1 => "Mayormente despejado",
        2 => "Parcialmente nublado",
        3 => "Nublado",
        45 | 48 => "Niebla",
        51 | 53 | 55 | 56 | 57 => "Llovizna",
        61 | 63 | 65 | 66 | 67 => "Lluvia",
        71 | 73 | 75 | 77 => "Nieve",
        80..=82 => "Chubascos",
        85 | 86 => "Chubascos de nieve",
        95 | 96 | 99 => "Tormenta",
        _ => "Sin determinar",
    }
}

/// When the weather reading arrived ("updated 14:05").
#[must_use]
pub fn weather_updated(time: &str) -> String {
    format!("actualizado {time}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_cover_their_ranges_and_nothing_outside() {
        for weekday in 0..7 {
            assert!(!weekday_short(weekday).is_empty());
            assert_eq!(weekday_short(weekday).chars().count(), 3);
            assert_eq!(weekday_initial(weekday).chars().count(), 1);
            assert!(weekday_full(weekday).starts_with(weekday_short(weekday).trim_end_matches('é')));
        }
        assert_eq!(weekday_short(7), "");
        assert_eq!(weekday_initial(200), "");
        assert_eq!(weekday_full(7), "");
        assert_eq!(weekday_short(0), "lun");
        assert_eq!(weekday_full(2), "miércoles");
        for month in 1..=12 {
            assert!(!month_full(month).is_empty());
            assert_eq!(month_short(month).chars().count(), 3);
        }
        assert_eq!(month_full(0), "");
        assert_eq!(month_full(13), "");
        assert_eq!(month_short(0), "");
        assert_eq!(month_short(13), "");
        assert_eq!(month_full(1), "enero");
        assert_eq!(month_short(12), "dic");
    }

    #[test]
    fn the_short_date_and_the_month_title() {
        assert_eq!(short_date(4, 17), "vie 17");
        assert_eq!(short_date(6, 1), "dom 1");
        assert_eq!(month_title(2024, 2), "Febrero de 2024");
        assert_eq!(month_title(2026, 12), "Diciembre de 2026");
        assert_eq!(month_title(2026, 13), " de 2026");
    }

    #[test]
    fn the_system_figures() {
        assert_eq!(bar_sysmon(23, 41), "CPU 23% · RAM 41%");
        assert_eq!(percent(7), "7%");
        assert_eq!(gib(0), "0,0 GiB");
        assert_eq!(gib(1_048_576), "1,0 GiB");
        assert_eq!(gib(4_089_446), "3,9 GiB");
        assert_eq!(gib(u64::MAX), "17592186044416,0 GiB");
        assert_eq!(memory_usage(6_500_000, 16_360_000), "6,2 de 15,6 GiB");
        assert_eq!(celsius(62), "62 °C");
        assert_eq!(celsius(-5), "-5 °C");
        assert_eq!(bar_weather(12), "12°");
        assert_eq!(weather_updated("14:05"), "actualizado 14:05");
    }

    #[test]
    fn every_open_meteo_code_has_its_words() {
        let words = |codes: &[u8]| -> Vec<&'static str> {
            codes.iter().map(|c| weather_condition(*c)).collect()
        };
        assert_eq!(weather_condition(0), "Despejado");
        assert_eq!(
            words(&[1, 2, 3]),
            ["Mayormente despejado", "Parcialmente nublado", "Nublado"]
        );
        assert_eq!(words(&[45, 48]), ["Niebla", "Niebla"]);
        assert!(words(&[51, 53, 55, 56, 57])
            .iter()
            .all(|w| *w == "Llovizna"));
        assert!(words(&[61, 63, 65, 66, 67]).iter().all(|w| *w == "Lluvia"));
        assert!(words(&[71, 73, 75, 77]).iter().all(|w| *w == "Nieve"));
        assert!(words(&[80, 81, 82]).iter().all(|w| *w == "Chubascos"));
        assert_eq!(
            words(&[85, 86]),
            ["Chubascos de nieve", "Chubascos de nieve"]
        );
        assert!(words(&[95, 96, 99]).iter().all(|w| *w == "Tormenta"));
        // Codes the service does not define are not guessed.
        let defined = [
            0, 1, 2, 3, 45, 48, 51, 53, 55, 56, 57, 61, 63, 65, 66, 67, 71, 73, 75, 77, 80, 81, 82,
            85, 86, 95, 96, 99,
        ];
        for code in 0..=u8::MAX {
            let undetermined = weather_condition(code) == "Sin determinar";
            assert_eq!(undetermined, !defined.contains(&code), "code {code}");
        }
        for code in [
            52, 54, 58, 59, 60, 62, 64, 68, 69, 70, 72, 74, 76, 78, 79, 83, 84, 87, 94, 97,
        ] {
            assert_eq!(weather_condition(code), "Sin determinar", "code {code}");
        }
        assert_eq!(
            weather_condition(celestina_shell::weather::UNKNOWN_CODE),
            "Sin determinar"
        );
    }
}
