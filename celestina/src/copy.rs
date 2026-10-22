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
/// fit one of the equal pills, and the toolkit has no ellipsis.
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

// The Clipboard page and piece (CEL-2-G). Nothing of an entry is ever
// translated or kept here: only the words around it.

/// The title of the Clipboard page ("Clipboard").
pub const PAGE_CLIPBOARD: &str = "Portapapeles";

/// The Clipboard tab's short label ("Clipb."): the full title does not fit
/// one of six equal pills, and the toolkit has no ellipsis.
pub const TAB_CLIPBOARD: &str = "Portap.";

/// The Clipboard page's only row while the history holds nothing
/// ("No entries").
pub const CLIPBOARD_EMPTY: &str = "Sin entradas";

/// The Clipboard page's only row while the duty is off
/// ("The clipboard history is off").
pub const CLIPBOARD_OFF: &str = "El historial del portapapeles está desactivado";

/// The Clipboard page's only row when the session offers no way to watch
/// the clipboard ("The clipboard is not available in this session").
pub const CLIPBOARD_UNAVAILABLE: &str = "El portapapeles no está disponible en esta sesión";

/// The button that stops recording copies ("Pause").
pub const CLIPBOARD_PAUSE: &str = "Pausar";

/// The button that records copies again ("Resume").
pub const CLIPBOARD_RESUME: &str = "Reanudar";

/// Shown beside the buttons while recording is paused ("Paused").
pub const CLIPBOARD_PAUSED: &str = "En pausa";

/// The button that starts emptying the history ("Clear").
pub const CLIPBOARD_CLEAR: &str = "Vaciar";

/// The question the confirmation step asks ("Clear the history?").
pub const CLIPBOARD_CLEAR_ASK: &str = "¿Vaciar el historial?";

/// The button that confirms emptying the history ("Confirm").
pub const CLIPBOARD_CLEAR_CONFIRM: &str = "Confirmar";

/// The button that gives up emptying the history ("Cancel").
pub const CLIPBOARD_CLEAR_CANCEL: &str = "Cancelar";

/// The line under the rows that counts the entries the page has no room for
/// ("12 more").
#[must_use]
pub fn clipboard_more(left_out: usize) -> String {
    format!("{left_out} más")
}

/// The accessible name and label of an image entry: a fixed word and its
/// size, never a pixel ("Image, 340 KB").
#[must_use]
pub fn clipboard_image_name(bytes: usize) -> String {
    const KIB: usize = 1024;
    const MIB: usize = 1024 * 1024;
    if bytes >= MIB {
        let tenths = bytes.saturating_mul(10) / MIB;
        format!("Imagen, {},{} MB", tenths / 10, tenths % 10)
    } else if bytes >= KIB {
        format!("Imagen, {} KB", bytes / KIB)
    } else {
        format!("Imagen, {bytes} B")
    }
}

/// The accessible state of the bar's clipboard icon: whether recording is
/// paused.
#[must_use]
pub fn bar_clipboard_state(paused: bool) -> &'static str {
    if paused {
        "Portapapeles, en pausa"
    } else {
        "Portapapeles"
    }
}

// Connectivity and power (CEL-2-F): the network, Bluetooth, battery and
// power-profile pieces and their pages. Service-supplied text (a network's
// name, a device's alias) is data, shown only as a row's name, and never
// goes into one of these sentences: a failure is described by its kind.

/// The title of the Network page, and the network piece's accessible name.
pub const PAGE_NETWORK: &str = "Red";

/// The title of the Bluetooth page, and the Bluetooth piece's accessible
/// name.
pub const PAGE_BLUETOOTH: &str = "Bluetooth";

/// The title of the Power page ("Energy"), the battery's and the profile's.
pub const PAGE_POWER: &str = "Energía";

/// The accessible name of the bar's battery piece ("Battery").
pub const PIECE_BATTERY: &str = "Batería";

/// The accessible name of the bar's power-profile piece ("Power profile").
pub const PIECE_POWER_PROFILE: &str = "Perfil de energía";

/// The label of the row that opens the editing program ("Edit...").
pub const EDIT: &str = "Editar…";

/// What the network page's edit row opens, beside it
/// ("Network profiles and passwords").
pub const EDIT_NETWORK_DETAIL: &str = "Perfiles de red y contraseñas";

/// What the Bluetooth page's edit row opens, beside it
/// ("Pair and forget devices").
pub const EDIT_BLUETOOTH_DETAIL: &str = "Emparejar y olvidar dispositivos";

/// Shown under the network edit row when its program is not installed
/// ("nm-connection-editor is not installed").
pub const EDITOR_NETWORK_MISSING: &str = "nm-connection-editor no está instalado";

/// Shown under the Bluetooth edit row when its program is not installed
/// ("blueman-manager is not installed").
pub const EDITOR_BLUETOOTH_MISSING: &str = "blueman-manager no está instalado";

/// A row's state while its request waits for the system to confirm it
/// ("Applying...").
pub const APPLYING: &str = "Aplicando…";

/// A row's state while it needs nothing from the shell and the person
/// configures it elsewhere ("Configured in nm-applet").
pub const NET_CONFIGURE_ELSEWHERE: &str = "Se configura en nm-applet";

/// The Wi-Fi switch's label ("Wi-Fi").
pub const NET_WIFI: &str = "Wi-Fi";

/// Under the Wi-Fi switch while the hardware blocks the radio
/// ("Blocked by the hardware").
pub const NET_WIFI_BLOCKED: &str = "Bloqueado por el hardware";

/// The Network page's only row while NetworkManager is not on the bus
/// ("NetworkManager is not available").
pub const NET_UNAVAILABLE: &str = "NetworkManager no está disponible";

/// The Network page's only row while the radio is off and no wired or VPN
/// link exists ("Wi-Fi is off").
pub const NET_WIFI_OFF: &str = "El Wi-Fi está apagado";

/// The Network page's only row while no network is in range
/// ("No networks in range").
pub const NET_EMPTY: &str = "No hay redes al alcance";

/// A network row's title when its name is empty ("Unnamed network").
pub const NET_UNNAMED: &str = "Red sin nombre";

/// A network row's state while it is active ("Connected").
pub const NET_CONNECTED: &str = "Conectada";

/// A network row's state while it activates ("Connecting...").
pub const NET_CONNECTING: &str = "Conectando…";

/// A network row's state when it has a saved profile and is not active
/// ("Saved").
pub const NET_SAVED: &str = "Guardada";

/// A Wi-Fi row's detail while it has no security ("Open").
pub const NET_OPEN: &str = "Abierta";

/// A Wi-Fi row's detail for a passphrase network ("Protected").
pub const NET_PASSPHRASE: &str = "Protegida";

/// A Wi-Fi row's detail for an enterprise network ("Enterprise").
pub const NET_ENTERPRISE: &str = "Empresarial";

/// A Wi-Fi row's detail for a WEP network.
pub const NET_WEP: &str = "WEP";

/// A wired row's detail ("Wired").
pub const NET_WIRED: &str = "Cableada";

/// A VPN row's detail.
pub const NET_VPN: &str = "VPN";

/// A network row's detail: its kind or security, then, while active, its
/// address ("Protected · 192.168.1.20").
#[must_use]
pub fn net_detail(kind: &str, address: Option<&str>) -> String {
    match address {
        Some(address) => format!("{kind} · {address}"),
        None => kind.to_owned(),
    }
}

/// The Bluetooth switch's label ("Bluetooth").
pub const BT_SWITCH: &str = "Bluetooth";

/// Under the Bluetooth switch while the adapter looks for devices
/// ("Looking for devices...").
pub const BT_SEARCHING: &str = "Buscando dispositivos…";

/// The Bluetooth page's only row while BlueZ or its adapter is absent
/// ("Bluetooth is not available").
pub const BT_UNAVAILABLE: &str = "Bluetooth no está disponible";

/// The Bluetooth page's only row while the adapter is off
/// ("Bluetooth is off").
pub const BT_OFF: &str = "Bluetooth apagado";

/// The Bluetooth page's only row while no device is paired
/// ("No paired devices").
pub const BT_EMPTY: &str = "No hay dispositivos emparejados";

/// A device row's title when its alias is empty ("Unnamed device").
pub const BT_UNNAMED: &str = "Dispositivo sin nombre";

/// A device row's state while connected ("Connected").
pub const BT_CONNECTED: &str = "Conectado";

/// A device row's state while paired and not connected ("Paired").
pub const BT_PAIRED: &str = "Emparejado";

/// A device row's state while not paired ("Paired in blueman").
pub const BT_PAIR_ELSEWHERE: &str = "Se empareja en blueman";

/// The Power page's only row while neither UPower nor the profile daemon
/// has anything to show ("No power data").
pub const POWER_UNAVAILABLE: &str = "No hay datos de energía";

/// A battery in percent ("Battery 64 %"): the Power page's battery row title
/// and a Bluetooth device row's detail, one recipe for both.
#[must_use]
pub fn battery_percent(percent: u8) -> String {
    format!("Batería {percent} %")
}

/// The battery is charging ("Charging").
pub const BATTERY_CHARGING: &str = "Cargando";

/// The battery is discharging ("Discharging").
pub const BATTERY_DISCHARGING: &str = "Descargando";

/// The battery is full ("Fully charged").
pub const BATTERY_FULL: &str = "Carga completa";

/// The battery is empty ("Empty").
pub const BATTERY_EMPTY: &str = "Descargada";

/// The battery waits to charge ("Waiting to charge").
pub const BATTERY_PENDING_CHARGE: &str = "Esperando para cargar";

/// The battery waits to discharge ("Waiting to discharge").
pub const BATTERY_PENDING_DISCHARGE: &str = "Esperando para descargar";

/// UPower reports no state ("Unknown state").
pub const BATTERY_UNKNOWN: &str = "Estado desconocido";

/// A span of `secs` seconds, to the minute ("2 h 5 min", "35 min").
#[must_use]
pub fn duration(secs: u64) -> String {
    let minutes = secs / 60;
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{} min", minutes.max(1)),
        (hours, 0) => format!("{hours} h"),
        (hours, minutes) => format!("{hours} h {minutes} min"),
    }
}

/// The battery row's detail while discharging: the state and the time left
/// ("Discharging · 2 h left").
#[must_use]
pub fn battery_remaining(state: &str, span: &str) -> String {
    format!("{state} · quedan {span}")
}

/// The battery row's detail while charging: the state and the time to full
/// ("Charging · 35 min to full").
#[must_use]
pub fn battery_to_full(state: &str, span: &str) -> String {
    format!("{state} · faltan {span}")
}

/// The performance profile's label ("Performance").
pub const PROFILE_PERFORMANCE: &str = "Rendimiento";

/// The balanced profile's label ("Balanced").
pub const PROFILE_BALANCED: &str = "Equilibrado";

/// The power-saver profile's label ("Power saver").
pub const PROFILE_POWER_SAVER: &str = "Ahorro de energía";

/// A profile row's state while it is the active one ("Active").
pub const PROFILE_ACTIVE: &str = "Activo";

/// Under the profiles while the daemon reports a laptop on the lap
/// ("Performance is limited: the computer is on a lap").
pub const PROFILE_DEGRADED_LAP: &str = "Rendimiento limitado: el equipo está sobre las piernas";

/// Under the profiles while the daemon reports heat
/// ("Performance is limited by temperature").
pub const PROFILE_DEGRADED_HEAT: &str = "Rendimiento limitado por la temperatura";

/// Under the profiles while the daemon reports another reason
/// ("Performance is limited").
pub const PROFILE_DEGRADED_OTHER: &str = "Rendimiento limitado";

// The sentence under a row whose request failed, by kind of failure.

/// No confirmation came in time ("No answer in time").
pub const FAIL_TIMEOUT: &str = "No hubo respuesta a tiempo";

/// The saved network needs credentials the system does not hold
/// ("Credentials are missing: configured in nm-applet").
pub const FAIL_NEEDS_SECRET: &str = "Faltan las credenciales: se configuran en nm-applet";

/// The system denied the change ("The system did not allow it").
pub const FAIL_DENIED: &str = "El sistema no lo permitió";

/// The network or device is gone ("It is no longer in range").
pub const FAIL_NOT_FOUND: &str = "Ya no está al alcance";

/// The service stopped answering ("The service stopped answering").
pub const FAIL_UNREACHABLE: &str = "El servicio dejó de responder";

/// Another request is still waiting ("Another request is in progress; try
/// again").
pub const FAIL_BUSY: &str = "Hay otra petición en curso; inténtalo de nuevo";

/// Any other failure ("It could not be completed").
pub const FAIL_OTHER: &str = "No se pudo completar";

// The sentence under a row whose request the shell refused before sending.

/// The service has not reported yet ("There is no data from the service
/// yet").
pub const REFUSED_NO_INVENTORY: &str = "Todavía no hay datos del servicio";

/// The row is gone ("It is no longer available").
pub const REFUSED_UNKNOWN: &str = "Ya no está disponible";

/// The network has no saved profile ("It has no saved profile: configured in
/// nm-applet").
pub const REFUSED_NOT_KNOWN: &str = "No tiene perfil guardado: se configura en nm-applet";

/// A VPN ("VPNs are managed in nm-applet").
pub const REFUSED_KIND: &str = "Las VPN se gestionan en nm-applet";

/// An enterprise or WEP network ("It needs credentials: configured in
/// nm-applet").
pub const REFUSED_SECRET_CLASS: &str = "Necesita credenciales: se configura en nm-applet";

/// Leaving a network that is not active ("It is not connected").
pub const REFUSED_NOT_ACTIVE: &str = "No está conectada";

/// A device that is not paired ("It is not paired: paired in blueman").
pub const REFUSED_NOT_PAIRED: &str = "No está emparejado: se empareja en blueman";

/// Joining a Wi-Fi network with the radio off ("Wi-Fi is off").
pub const REFUSED_WIFI_OFF: &str = "El Wi-Fi está apagado";

/// Turning Wi-Fi on under a hardware block ("Wi-Fi is blocked by the
/// hardware").
pub const REFUSED_HARDWARE: &str = "El Wi-Fi está bloqueado por el hardware";

/// A profile the daemon does not offer ("That profile does not exist").
pub const REFUSED_NO_PROFILE: &str = "Ese perfil no existe";

/// Cycling with nothing to cycle to ("There is no other profile").
pub const REFUSED_NOTHING_TO_CYCLE: &str = "No hay otro perfil";

// The on-screen display (CEL-2-C).

/// The level the on-screen display shows beside its bar ("60 %"), written
/// as the suite writes a percentage: the number, a space, the sign.
#[must_use]
pub fn osd_percent(percent: u8) -> String {
    format!("{percent} %")
}

// The Volume and Brightness pieces and their popup (CEL-2-C).

/// The accessible name of the bar's Volume piece and the popup's heading
/// ("Volume").
pub const PIECE_VOLUME: &str = "Volumen";

/// The accessible name of the bar's Brightness piece and the popup's heading
/// ("Brightness").
pub const PIECE_BRIGHTNESS: &str = "Brillo";

/// The popup's heading for the microphone's row ("Microphone").
pub const MICROPHONE: &str = "Micrófono";

/// The label of the output's mute switch in the volume popup ("Mute").
pub const MUTE: &str = "Silenciar";

/// The label of the microphone's mute switch in the volume popup
/// ("Mute microphone").
pub const MUTE_MICROPHONE: &str = "Silenciar micrófono";

/// The accessible state of the bar's Volume piece: the level, or that the
/// output is muted ("Volume, 60 %" / "Volume, muted").
#[must_use]
pub fn bar_volume_state(percent: u8, muted: bool) -> String {
    if muted {
        format!("{PIECE_VOLUME}, silenciado")
    } else {
        format!("{PIECE_VOLUME}, {}", osd_percent(percent))
    }
}

/// The accessible state of the bar's Brightness piece ("Brightness, 60 %").
#[must_use]
pub fn bar_brightness_state(percent: u8) -> String {
    format!("{PIECE_BRIGHTNESS}, {}", osd_percent(percent))
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
