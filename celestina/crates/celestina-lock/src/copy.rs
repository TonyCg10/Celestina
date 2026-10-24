// language-contract: product-copy
//! Every word the lock screen shows, in Spanish (ADR 0007 of the suite,
//! `docs/decisions/0007-spanish-product-copy.md`; design §4.6). The string
//! literals of this file are the product copy; the comments and identifiers
//! around them are development truth, in English. No other file of the lock
//! holds a Spanish string: the view names these items, and the tests name them
//! too.
//!
//! The literals are the design's proposals; the author may reword them.
#![forbid(unsafe_code)]

/// Placeholder of the empty password field ("Password").
pub const LOCK_FIELD: &str = "Contraseña";

/// Shown while the verifier answers ("Checking…").
pub const LOCK_CHECKING: &str = "Comprobando…";

/// The password was refused ("Wrong password").
pub const LOCK_REFUSED: &str = "Contraseña incorrecta";

/// The check could not be made: the verifier failed, died or timed out
/// ("The password could not be checked. Try again.").
pub const LOCK_UNAVAILABLE: &str = "No se ha podido comprobar la contraseña. Vuelve a intentarlo.";

/// The account may not log in now, for instance an expired password.
pub const LOCK_ACCOUNT: &str = "La cuenta no permite entrar ahora (por ejemplo, la contraseña ha caducado). Entra desde otra consola.";

/// The authentication stack asks for more than the password.
pub const LOCK_UNSUPPORTED: &str =
    "La autenticación pide algo más que la contraseña, y este bloqueo solo pide la contraseña.";

/// The PAM service file is not installed.
pub const LOCK_SERVICE_MISSING: &str = "Falta el servicio PAM celestina-lock: instálalo como explica celestina/README.md y entra desde otra consola.";

/// From the third counted failure: `pam_faillock` may refuse even the right
/// password for a while, and PAM cannot say why.
pub const LOCK_FAILLOCK_HINT: &str =
    "Tras varios intentos fallidos la cuenta puede quedar bloqueada unos minutos.";

/// Caps Lock is on while the password is typed ("Caps Lock is on").
pub const LOCK_CAPS: &str = "Bloq Mayús activado";

/// The seat has no keyboard ("No keyboard is connected").
pub const LOCK_NO_KEYBOARD: &str = "No hay ningún teclado conectado";

/// The banner of a rehearsal: this lock never unlocks.
pub const LOCK_REHEARSAL: &str = "Ensayo: este bloqueo no se desbloquea";

/// The countdown of the delay between attempts ("Wait N s to try again").
#[must_use]
pub fn lock_wait(seconds: u32) -> String {
    format!("Espera {seconds} s para volver a intentarlo")
}

/// The weekdays of the date line, Sunday first ("Sunday" to "Saturday").
pub const LOCK_WEEKDAYS: [&str; 7] = [
    "domingo",
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
];

/// The months of the date line, January first ("January" to "December").
pub const LOCK_MONTHS: [&str; 12] = [
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

/// The date line ("Friday 23 October"): `weekday` counts from Sunday = 0,
/// `month` from January = 1. An index out of range leaves its word out
/// rather than panic; the clock never produces one.
#[must_use]
pub fn lock_date(weekday: u8, day: u8, month: u8) -> String {
    let weekday = LOCK_WEEKDAYS.get(usize::from(weekday)).copied();
    let month = usize::from(month)
        .checked_sub(1)
        .and_then(|index| LOCK_MONTHS.get(index))
        .copied();
    match (weekday, month) {
        (Some(weekday), Some(month)) => format!("{weekday} {day} de {month}"),
        (None, Some(month)) => format!("{day} de {month}"),
        _ => day.to_string(),
    }
}
