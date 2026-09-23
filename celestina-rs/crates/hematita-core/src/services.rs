//! What the Services page shows and how a privileged action's result reads,
//! decided without Qt: which units survive the toggles and the search, in
//! what order, and what a `D-Bus` error or a `pkexec` exit status means to
//! the person.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    System,
    User,
}

impl Scope {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitKind {
    Service,
    Socket,
    Timer,
    Target,
    Mount,
    Device,
    Scope,
    Slice,
    Path,
    Other,
}

impl UnitKind {
    /// Reads the kind from the unit name's suffix (`foo.service`,
    /// `bar.socket`, ...); a name with no recognised suffix is `Other`.
    #[must_use]
    pub fn of_name(name: &str) -> Self {
        match name.rsplit_once('.') {
            Some((_, "service")) => Self::Service,
            Some((_, "socket")) => Self::Socket,
            Some((_, "timer")) => Self::Timer,
            Some((_, "target")) => Self::Target,
            Some((_, "mount")) => Self::Mount,
            Some((_, "device")) => Self::Device,
            Some((_, "scope")) => Self::Scope,
            Some((_, "slice")) => Self::Slice,
            Some((_, "path")) => Self::Path,
            _ => Self::Other,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Service => "service",
            Self::Socket => "socket",
            Self::Timer => "timer",
            Self::Target => "target",
            Self::Mount => "mount",
            Self::Device => "device",
            Self::Scope => "scope",
            Self::Slice => "slice",
            Self::Path => "path",
            Self::Other => "other",
        }
    }
}

/// Whether `StartUnit`/`StopUnit`/`RestartUnit` mean anything for this kind:
/// only a service or a socket is ever offered start/stop/restart.
#[must_use]
pub fn is_actionable(kind: UnitKind) -> bool {
    matches!(kind, UnitKind::Service | UnitKind::Socket)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    pub name: String,
    pub description: String,
    pub scope: Scope,
    pub active: String,
    pub sub: String,
}

fn active_rank(active: &str) -> u8 {
    match active {
        "failed" => 0,
        "active" => 1,
        _ => 2,
    }
}

/// Indices of the units that survive the scope toggles, the `services_only`
/// kind filter and `filter` (case-insensitive substring of the name or the
/// description; empty matches all), sorted by scope (user before system),
/// then active state (`failed` before `active` before anything else), then
/// name.
#[must_use]
pub fn project(
    units: &[Unit],
    filter: &str,
    show_system: bool,
    show_user: bool,
    services_only: bool,
) -> Vec<usize> {
    let needle = filter.trim().to_lowercase();
    let mut order: Vec<usize> = units
        .iter()
        .enumerate()
        .filter(|(_, unit)| match unit.scope {
            Scope::System => show_system,
            Scope::User => show_user,
        })
        .filter(|(_, unit)| !services_only || UnitKind::of_name(&unit.name) == UnitKind::Service)
        .filter(|(_, unit)| {
            needle.is_empty()
                || unit.name.to_lowercase().contains(&needle)
                || unit.description.to_lowercase().contains(&needle)
        })
        .map(|(index, _)| index)
        .collect();
    order.sort_by(|&a, &b| {
        let (left, right) = (&units[a], &units[b]);
        let scope_rank = |scope: Scope| match scope {
            Scope::User => 0,
            Scope::System => 1,
        };
        scope_rank(left.scope)
            .cmp(&scope_rank(right.scope))
            .then_with(|| active_rank(&left.active).cmp(&active_rank(&right.active)))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    order
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Done,
    NoAgent,
    Denied,
    Failed,
    Refused,
}

impl Outcome {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::NoAgent => "no-agent",
            Self::Denied => "denied",
            Self::Failed => "failed",
            Self::Refused => "refused",
        }
    }
}

/// Reads a system-bus error name into the outcome the person should be
/// told. An unrecognised or empty name is `Failed`: it is not this
/// function's business to guess what a systemd- or polkit-shaped error it
/// has never seen means.
///
/// `InteractiveAuthorizationRequired` is the one name that means no agent
/// answered: the caller asked for interaction and the bus still could not
/// get an answer. `NotAuthorized` is polkit having asked and been told no —
/// or having refused outright — which is the person's answer, not a missing
/// agent, so it reads as `Denied` (ADR 0010: "the person cancelled or was
/// not authorised").
#[must_use]
pub fn outcome_of_dbus_error(name: &str) -> Outcome {
    match name {
        "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired" => Outcome::NoAgent,
        "org.freedesktop.DBus.Error.AccessDenied"
        | "org.freedesktop.PolicyKit1.Error.NotAuthorized" => Outcome::Denied,
        _ => Outcome::Failed,
    }
}

/// Reads `pkexec`'s exit status into the outcome the person should be told.
/// `126` is the dismissed prompt or the refused authorisation; `127` is
/// `pkexec` unable to run the command at all, which on this session today
/// means no authentication agent answered. `None` is the process dying to a
/// signal, which is a failure like any other.
#[must_use]
pub fn outcome_of_pkexec(status: Option<i32>) -> Outcome {
    match status {
        Some(0) => Outcome::Done,
        Some(126) => Outcome::Denied,
        Some(127) => Outcome::NoAgent,
        _ => Outcome::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_kind_reads_every_suffix_and_falls_back_to_other() {
        assert_eq!(UnitKind::of_name("foo.service"), UnitKind::Service);
        assert_eq!(UnitKind::of_name("bar.socket"), UnitKind::Socket);
        assert_eq!(UnitKind::of_name("x.timer"), UnitKind::Timer);
        assert_eq!(UnitKind::of_name("a.mount"), UnitKind::Mount);
        assert_eq!(UnitKind::of_name("dev-sda.device"), UnitKind::Device);
        assert_eq!(UnitKind::of_name("app-x.scope"), UnitKind::Scope);
        assert_eq!(UnitKind::of_name("user.slice"), UnitKind::Slice);
        assert_eq!(UnitKind::of_name("y.path"), UnitKind::Path);
        assert_eq!(UnitKind::of_name("z.target"), UnitKind::Target);
        assert_eq!(UnitKind::of_name("weird"), UnitKind::Other);
    }

    #[test]
    fn only_a_service_or_a_socket_is_actionable() {
        assert!(is_actionable(UnitKind::Service));
        assert!(is_actionable(UnitKind::Socket));
        for kind in [
            UnitKind::Timer,
            UnitKind::Target,
            UnitKind::Mount,
            UnitKind::Device,
            UnitKind::Scope,
            UnitKind::Slice,
            UnitKind::Path,
            UnitKind::Other,
        ] {
            assert!(!is_actionable(kind));
        }
    }

    fn unit(name: &str, description: &str, scope: Scope, active: &str, sub: &str) -> Unit {
        Unit {
            name: name.to_owned(),
            description: description.to_owned(),
            scope,
            active: active.to_owned(),
            sub: sub.to_owned(),
        }
    }

    fn units() -> Vec<Unit> {
        vec![
            unit(
                "sshd.service",
                "OpenSSH server daemon",
                Scope::System,
                "active",
                "running",
            ),
            unit(
                "at-spi-dbus-bus.service",
                "Accessibility services bus",
                Scope::User,
                "active",
                "running",
            ),
            unit(
                "smartd.service",
                "Self Monitoring and Reporting Technology",
                Scope::System,
                "failed",
                "dead",
            ),
            unit(
                "cups.socket",
                "CUPS Scheduler",
                Scope::System,
                "active",
                "listening",
            ),
            unit(
                "user.slice",
                "User and Session Slice",
                Scope::User,
                "active",
                "active",
            ),
            unit(
                "gpg-agent.socket",
                "GnuPG cryptographic agent",
                Scope::User,
                "inactive",
                "dead",
            ),
        ]
    }

    #[test]
    fn project_orders_by_scope_then_active_state_then_name() {
        let units = units();
        // User before system. Within a scope, `failed` before `active`
        // before anything else, then by name: on the user side both
        // survivors are `active`, so "at-spi-dbus-bus.service" sorts before
        // "user.slice" and the `inactive` "gpg-agent.socket" falls last; on
        // the system side the `failed` "smartd.service" leads, then
        // "cups.socket" before "sshd.service" among the `active` pair.
        let order = project(&units, "", true, true, false);
        assert_eq!(order, vec![1, 4, 5, 2, 3, 0]);
    }

    #[test]
    fn the_scope_toggles_narrow_independently() {
        let units = units();
        assert_eq!(project(&units, "", true, false, false), vec![2, 3, 0]);
        assert_eq!(project(&units, "", false, true, false), vec![1, 4, 5]);
        assert_eq!(
            project(&units, "", false, false, false),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn services_only_keeps_service_units() {
        let units = units();
        let order = project(&units, "", true, true, true);
        assert_eq!(order, vec![1, 2, 0]);
    }

    #[test]
    fn the_filter_matches_name_or_description_case_insensitively() {
        let units = units();
        assert_eq!(project(&units, "cups", true, true, false), vec![3]);
        assert_eq!(project(&units, "ACCESSIBILITY", true, true, false), vec![1]);
        assert_eq!(
            project(&units, "nothing", true, true, false),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn dbus_errors_map_to_the_documented_outcomes() {
        assert_eq!(
            outcome_of_dbus_error("org.freedesktop.DBus.Error.InteractiveAuthorizationRequired"),
            Outcome::NoAgent
        );
        // polkit answered — the person said no, or was not allowed to say
        // yes. That is an answer, not a missing agent.
        assert_eq!(
            outcome_of_dbus_error("org.freedesktop.PolicyKit1.Error.NotAuthorized"),
            Outcome::Denied
        );
        assert_eq!(
            outcome_of_dbus_error("org.freedesktop.DBus.Error.AccessDenied"),
            Outcome::Denied
        );
        assert_eq!(
            outcome_of_dbus_error("org.freedesktop.systemd1.NoSuchUnit"),
            Outcome::Failed
        );
        assert_eq!(outcome_of_dbus_error(""), Outcome::Failed);
    }

    #[test]
    fn pkexec_statuses_map_to_the_documented_outcomes() {
        assert_eq!(outcome_of_pkexec(Some(0)), Outcome::Done);
        assert_eq!(outcome_of_pkexec(Some(126)), Outcome::Denied);
        assert_eq!(outcome_of_pkexec(Some(127)), Outcome::NoAgent);
        assert_eq!(outcome_of_pkexec(Some(1)), Outcome::Failed);
        assert_eq!(outcome_of_pkexec(None), Outcome::Failed);
    }
}
