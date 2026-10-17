//! The launcher's `/` commands: the shell's own switches, listed instead of
//! applications while the query starts with `/` (design §5.5, Launcher).
//! Choosing one runs it in place: the app opens the Island's page or turns
//! do-not-disturb over, as the Island's own controls do.

use celestina_shell::launcher::fold;

use crate::{copy, pieces::PageId};

/// One of the shell's switches a `/` command names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Switch {
    /// `/notificaciones`: the Island's Notifications page.
    Notificaciones,
    /// `/no-molestar`: do-not-disturb on or off.
    NoMolestar,
    /// `/actividad`: the Island's Activity page.
    Actividad,
    /// `/bandeja`: the Island's Tray page.
    Bandeja,
    /// `/sistema`: the Island's System page.
    Sistema,
    /// `/reloj`: the Island's Clock page.
    Reloj,
}

impl Switch {
    /// Every switch, in the order the launcher lists them.
    pub const ALL: [Switch; 6] = [
        Switch::Notificaciones,
        Switch::NoMolestar,
        Switch::Actividad,
        Switch::Bandeja,
        Switch::Sistema,
        Switch::Reloj,
    ];

    /// The command as typed, its leading `/` included.
    #[must_use]
    pub fn command(self) -> &'static str {
        match self {
            Switch::Notificaciones => copy::SWITCH_NOTIFICATIONS,
            Switch::NoMolestar => copy::SWITCH_DO_NOT_DISTURB,
            Switch::Actividad => copy::SWITCH_ACTIVITY,
            Switch::Bandeja => copy::SWITCH_TRAY,
            Switch::Sistema => copy::SWITCH_SYSMON,
            Switch::Reloj => copy::SWITCH_CLOCK,
        }
    }

    /// What the command does, shown beside it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Switch::Notificaciones => copy::SWITCH_NOTIFICATIONS_LABEL,
            Switch::NoMolestar => copy::SWITCH_DO_NOT_DISTURB_LABEL,
            Switch::Actividad => copy::SWITCH_ACTIVITY_LABEL,
            Switch::Bandeja => copy::SWITCH_TRAY_LABEL,
            Switch::Sistema => copy::SWITCH_SYSMON_LABEL,
            Switch::Reloj => copy::SWITCH_CLOCK_LABEL,
        }
    }

    /// The semantic icon of the switch's row.
    #[must_use]
    pub fn icon(self) -> &'static str {
        match self {
            Switch::Notificaciones => "bell",
            Switch::NoMolestar => "bell-off",
            Switch::Actividad => "layout-template",
            Switch::Bandeja => "system-tray",
            Switch::Sistema => "cpu",
            Switch::Reloj => "clock-arrow-up",
        }
    }

    /// The Island page the switch opens; `None` for do-not-disturb, which
    /// changes a setting instead.
    #[must_use]
    pub fn page(self) -> Option<PageId> {
        match self {
            Switch::Notificaciones => Some(PageId::NOTIFICATIONS),
            Switch::Actividad => Some(PageId::ACTIVITY),
            Switch::Bandeja => Some(PageId::TRAY),
            Switch::Sistema => Some(PageId::SYSMON),
            Switch::Reloj => Some(PageId::CLOCK),
            Switch::NoMolestar => None,
        }
    }
}

/// The switches a query names: every one for `/` alone, the ones whose
/// command starts with what was typed for `/x` (case and the optional
/// accents folded away), and `None` for a query that is not a command, which
/// searches the applications instead. A command nobody has gives
/// `Some(vec![])`: it lists nothing and never falls back to applications.
#[must_use]
pub fn commands_for(query: &str) -> Option<Vec<Switch>> {
    let typed = query.trim();
    if !typed.starts_with('/') {
        return None;
    }
    let typed = fold(typed);
    Some(
        Switch::ALL
            .into_iter()
            .filter(|switch| fold(switch.command()).starts_with(&typed))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glance_switches_open_their_pages() {
        assert_eq!(Switch::Sistema.page(), Some(PageId::SYSMON));
        assert_eq!(Switch::Reloj.page(), Some(PageId::CLOCK));
        assert_eq!(Switch::Sistema.command(), "/sistema");
        assert_eq!(Switch::Reloj.command(), "/reloj");
        for switch in [Switch::Sistema, Switch::Reloj] {
            assert!(!switch.label().is_empty() && !switch.icon().is_empty());
        }
    }

    #[test]
    fn slash_alone_lists_every_switch() {
        assert_eq!(commands_for("/"), Some(Switch::ALL.to_vec()));
        assert_eq!(commands_for("  /  "), Some(Switch::ALL.to_vec()));
    }

    #[test]
    fn a_prefix_lists_the_switches_it_starts() {
        assert_eq!(
            commands_for("/no"),
            Some(vec![Switch::Notificaciones, Switch::NoMolestar])
        );
        assert_eq!(commands_for("/NO-M"), Some(vec![Switch::NoMolestar]));
        assert_eq!(commands_for("/bandeja"), Some(vec![Switch::Bandeja]));
        assert_eq!(commands_for("/SIS"), Some(vec![Switch::Sistema]));
        assert_eq!(commands_for("/rel"), Some(vec![Switch::Reloj]));
    }

    #[test]
    fn a_query_without_a_slash_is_no_command() {
        assert_eq!(commands_for(""), None);
        assert_eq!(commands_for("firefox"), None);
        assert_eq!(commands_for("a/b"), None);
    }

    #[test]
    fn every_switch_is_typed_with_its_slash() {
        for switch in Switch::ALL {
            assert!(switch.command().starts_with('/'), "{switch:?}");
            assert!(!switch.label().is_empty(), "{switch:?}");
        }
    }
}
