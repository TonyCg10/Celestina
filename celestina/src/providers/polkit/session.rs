//! Which session the agent registers for, and in which locale (design §4.5).
//!
//! polkitd accepts an agent only for the caller's own session. The shell
//! runs under `user@<uid>.service`, outside a session scope, so logind's
//! `GetSessionByPID` fails for it (design fact 13) and is **not** used. The
//! session is the user's `Display` (`GetUser(uid)` → `Display`), which is how
//! polkitd itself resolves such a process, and it must be this person's
//! (`User` is the shell's uid), a `user`-class, `wayland`, local session.
//! `XDG_SESSION_ID`, when set, is a hint that must agree.
//!
//! The locale polkitd translates its message by is `LC_MESSAGES`, else
//! `LANG`, bounded and checked, else `C`.

use std::collections::HashMap;
use std::ffi::OsStr;

use zbus::blocking::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use crate::energy::logind::{LOGIND_NAME, LOGIND_PATH, MANAGER_INTERFACE};

/// logind's user interface.
pub const USER_INTERFACE: &str = "org.freedesktop.login1.User";
/// logind's session interface.
pub const SESSION_INTERFACE: &str = "org.freedesktop.login1.Session";
/// The standard properties interface.
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
/// The longest session id accepted, in bytes (logind's are short: `11`,
/// `c2`).
pub const MAX_SESSION_ID: usize = 64;
/// The longest locale accepted, in bytes.
pub const MAX_LOCALE: usize = 64;
/// The locale used when none is set or the set one is refused.
pub const FALLBACK_LOCALE: &str = "C";

/// Why no session could be resolved.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// A logind call failed.
    #[error("logind's {what} failed")]
    Call {
        what: &'static str,
        #[source]
        source: Box<zbus::Error>,
    },
    /// A logind answer did not have the expected shape.
    #[error("logind's {what} answered with an unexpected value")]
    Reply { what: &'static str },
    /// The user has no display session.
    #[error("the user has no display session")]
    NoDisplay,
    /// The session id is empty, too long or holds unexpected characters.
    #[error("logind gave a session id the agent does not accept")]
    BadId,
    /// The display session is not this person's local Wayland session.
    #[error("the display session is not usable: {0}")]
    Unusable(&'static str),
    /// `XDG_SESSION_ID` names another session than logind does.
    #[error("XDG_SESSION_ID disagrees with logind's display session")]
    HintDisagrees,
}

/// What the agent reads of its process once, at start.
#[derive(Clone, Debug)]
pub struct Environment {
    /// The shell's uid.
    pub own_uid: u32,
    /// `XDG_SESSION_ID`, if set.
    pub session_hint: Option<String>,
    /// The locale sent with the registration.
    pub locale: String,
    /// `/etc/passwd`'s text in tests; production reads the file per request.
    pub passwd: Option<String>,
}

impl Environment {
    /// The process's own values.
    ///
    /// # Errors
    ///
    /// The shell's uid cannot be read.
    pub fn read() -> Result<Environment, String> {
        let own_uid = celestina_core::xdg::effective_uid()
            .map_err(|err| format!("cannot read the shell's uid: {err}"))?;
        Ok(Environment {
            own_uid,
            session_hint: std::env::var("XDG_SESSION_ID").ok(),
            locale: locale(
                std::env::var_os("LC_MESSAGES").as_deref(),
                std::env::var_os("LANG").as_deref(),
            ),
            passwd: None,
        })
    }

    /// The text of `/etc/passwd`, read now (it may change while the shell
    /// runs).
    ///
    /// # Errors
    ///
    /// Why it cannot be read.
    pub fn passwd(&self) -> Result<String, String> {
        match &self.passwd {
            Some(text) => Ok(text.clone()),
            None => super::agent::read_passwd(),
        }
    }
}

/// The locale to register with: `lc_messages`, else `lang`, each only when
/// [`acceptable_locale`], else [`FALLBACK_LOCALE`].
#[must_use]
pub fn locale(lc_messages: Option<&OsStr>, lang: Option<&OsStr>) -> String {
    [lc_messages, lang]
        .into_iter()
        .flatten()
        .filter_map(OsStr::to_str)
        .find(|value| acceptable_locale(value))
        .unwrap_or(FALLBACK_LOCALE)
        .to_owned()
}

/// Whether `value` looks like a locale name: 1 to [`MAX_LOCALE`] bytes of
/// `[A-Za-z0-9_.@-]` (`es_ES.UTF-8`, `ca_ES@valencia`, `C`).
#[must_use]
pub fn acceptable_locale(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_LOCALE
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'@' | b'-'))
}

/// Whether `id` is a session id the agent registers: 1 to
/// [`MAX_SESSION_ID`] bytes of `[A-Za-z0-9_-]`.
#[must_use]
pub fn valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_SESSION_ID
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// What the agent checks of the display session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionFacts {
    /// `Id`.
    pub id: String,
    /// The uid of `User`.
    pub uid: u32,
    /// `Class`.
    pub class: String,
    /// `Type`.
    pub kind: String,
    /// `Remote`.
    pub remote: bool,
}

/// Checks that `facts` describe this person's local Wayland session named
/// `display`, and that `hint` agrees when given.
///
/// # Errors
///
/// What does not hold.
pub fn check(
    facts: &SessionFacts,
    display: &str,
    own_uid: u32,
    hint: Option<&str>,
) -> Result<(), SessionError> {
    if !valid_session_id(display) {
        return Err(SessionError::BadId);
    }
    if facts.id != display {
        return Err(SessionError::Unusable("its id is not the display's"));
    }
    if facts.uid != own_uid {
        return Err(SessionError::Unusable("it belongs to another user"));
    }
    if facts.class != "user" {
        return Err(SessionError::Unusable("its class is not user"));
    }
    if facts.kind != "wayland" {
        return Err(SessionError::Unusable("it is not a Wayland session"));
    }
    if facts.remote {
        return Err(SessionError::Unusable("it is remote"));
    }
    match hint {
        Some(hint) if !hint.is_empty() && hint != display => Err(SessionError::HintDisagrees),
        _ => Ok(()),
    }
}

/// The session id to register: logind's display session for `own_uid`,
/// checked as [`check`] says.
///
/// # Errors
///
/// [`SessionError`] when logind cannot be asked or the session does not
/// qualify.
pub fn resolve(
    connection: &Connection,
    own_uid: u32,
    hint: Option<&str>,
) -> Result<String, SessionError> {
    let user: OwnedObjectPath = call(
        connection,
        LOGIND_PATH,
        MANAGER_INTERFACE,
        "GetUser",
        &(own_uid,),
        "GetUser",
    )?;
    let display: OwnedValue = call(
        connection,
        user.as_str(),
        PROPERTIES,
        "Get",
        &(USER_INTERFACE, "Display"),
        "the user's Display",
    )?;
    let (display, session) = match pair(&display) {
        Some((Value::Str(id), Value::ObjectPath(path))) => {
            (id.as_str().to_owned(), path.as_str().to_owned())
        }
        _ => {
            return Err(SessionError::Reply {
                what: "the user's Display",
            })
        }
    };
    if display.is_empty() {
        return Err(SessionError::NoDisplay);
    }
    let properties: HashMap<String, OwnedValue> = call(
        connection,
        &session,
        PROPERTIES,
        "GetAll",
        &(SESSION_INTERFACE,),
        "the session's properties",
    )?;
    let facts = facts(&properties).ok_or(SessionError::Reply {
        what: "the session's properties",
    })?;
    check(&facts, &display, own_uid, hint)?;
    Ok(display)
}

fn facts(properties: &HashMap<String, OwnedValue>) -> Option<SessionFacts> {
    let text = |name: &str| match properties.get(name).map(|value| &**value) {
        Some(Value::Str(text)) => Some(text.as_str().to_owned()),
        _ => None,
    };
    let uid = match pair(properties.get("User")?) {
        Some((Value::U32(uid), Value::ObjectPath(_))) => *uid,
        _ => return None,
    };
    let remote = match properties.get("Remote").map(|value| &**value) {
        Some(Value::Bool(remote)) => *remote,
        _ => return None,
    };
    Some(SessionFacts {
        id: text("Id")?,
        uid,
        class: text("Class")?,
        kind: text("Type")?,
        remote,
    })
}

/// The two fields of a two-field structure value, such as `(so)` or `(uo)`.
fn pair(value: &OwnedValue) -> Option<(&Value<'static>, &Value<'static>)> {
    match &**value {
        Value::Structure(structure) => match structure.fields() {
            [first, second] => Some((first, second)),
            _ => None,
        },
        _ => None,
    }
}

/// One logind call, its answer deserialised as `T`.
fn call<B, T>(
    connection: &Connection,
    path: &str,
    interface: &str,
    method: &str,
    body: &B,
    what: &'static str,
) -> Result<T, SessionError>
where
    B: zbus::export::serde::Serialize + zbus::zvariant::DynamicType,
    T: for<'d> zbus::export::serde::Deserialize<'d> + zbus::zvariant::Type,
{
    let reply = connection
        .call_method(Some(LOGIND_NAME), path, Some(interface), method, body)
        .map_err(|source| SessionError::Call {
            what,
            source: Box::new(source),
        })?;
    reply
        .body()
        .deserialize::<T>()
        .map_err(|_| SessionError::Reply { what })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn facts() -> SessionFacts {
        SessionFacts {
            id: "c7".into(),
            uid: 4242,
            class: "user".into(),
            kind: "wayland".into(),
            remote: false,
        }
    }

    #[test]
    fn locale_prefers_lc_messages_then_lang_and_falls_back_to_c() {
        let es = OsString::from("es_ES.UTF-8");
        let ca = OsString::from("ca_ES@valencia");
        assert_eq!(locale(Some(&es), Some(&ca)), "es_ES.UTF-8");
        assert_eq!(locale(None, Some(&ca)), "ca_ES@valencia");
        assert_eq!(locale(Some(OsStr::new("")), Some(&ca)), "ca_ES@valencia");
        assert_eq!(locale(None, None), FALLBACK_LOCALE);
    }

    #[test]
    fn locale_refuses_controls_spaces_and_overlong_values() {
        let newline = OsString::from("es_ES\n.UTF-8");
        let space = OsString::from("es ES");
        let long = OsString::from("a".repeat(MAX_LOCALE + 1));
        assert_eq!(locale(Some(&newline), None), FALLBACK_LOCALE);
        assert_eq!(locale(Some(&space), None), FALLBACK_LOCALE);
        assert_eq!(locale(Some(&long), None), FALLBACK_LOCALE);
        assert_eq!(locale(Some(&long), Some(OsStr::new("C.UTF-8"))), "C.UTF-8");
        assert!(acceptable_locale(&"a".repeat(MAX_LOCALE)));
    }

    #[test]
    fn session_ids_are_short_and_plain() {
        assert!(valid_session_id("11"));
        assert!(valid_session_id("c2"));
        assert!(!valid_session_id(""));
        assert!(!valid_session_id("1 1"));
        assert!(!valid_session_id("c/2"));
        assert!(!valid_session_id(&"1".repeat(MAX_SESSION_ID + 1)));
    }

    #[test]
    fn a_local_wayland_session_of_this_user_is_accepted() {
        assert!(check(&facts(), "c7", 4242, None).is_ok());
        assert!(check(&facts(), "c7", 4242, Some("c7")).is_ok());
        // An empty hint is no hint.
        assert!(check(&facts(), "c7", 4242, Some("")).is_ok());
    }

    #[test]
    fn session_checks_refuse_another_uid_class_type_or_a_remote_session() {
        let refused = |change: fn(&mut SessionFacts)| {
            let mut facts = facts();
            change(&mut facts);
            check(&facts, "c7", 4242, None).is_err()
        };
        assert!(refused(|f| f.uid = 0));
        assert!(refused(|f| f.class = "greeter".into()));
        assert!(refused(|f| f.kind = "x11".into()));
        assert!(refused(|f| f.kind = "tty".into()));
        assert!(refused(|f| f.remote = true));
        assert!(refused(|f| f.id = "c8".into()));
        assert!(check(&facts(), "c 7", 4242, None).is_err());
    }

    #[test]
    fn a_session_hint_must_agree() {
        assert!(matches!(
            check(&facts(), "c7", 4242, Some("11")),
            Err(SessionError::HintDisagrees)
        ));
    }
}
