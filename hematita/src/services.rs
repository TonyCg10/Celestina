//! The Services page's state, as Qt properties, and the only place a unit is
//! started, stopped or restarted.
//!
//! Units are listed on the sampler thread; an action is one call on one named
//! worker thread, whose outcome is queued back to Qt. The user's own manager
//! authorises nothing; the system manager lets polkit decide per call, and
//! whatever polkit answers becomes one of the typed outcomes the page turns
//! into Spanish. See ADR 0010.
//!
//! Nothing here is prose a person reads: scopes, kinds, reasons and outcomes
//! are tokens; a unit's name and description are systemd's own data, shown raw.

use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList, QVariant};

use hematita_core::services::{self, is_actionable, Outcome, Scope, Unit, UnitKind};

use crate::lists::{doubles, strings};
use crate::publish;
use crate::sampler::{self, Reason, Section, ServiceSnapshot, Snapshot};

const SYSTEMD_SERVICE: &str = "org.freedesktop.systemd1";
const SYSTEMD_OBJECT: &str = "/org/freedesktop/systemd1";
const SYSTEMD_MANAGER: &str = "org.freedesktop.systemd1.Manager";
/// systemd's job mode: whatever conflicts with this job is stopped. It is what
/// `systemctl` itself asks for.
const REPLACE: &str = "replace";

/// How long an action's connection waits for systemd's reply. The default 25
/// seconds is a bus timeout, not a human one: a system unit's call does not
/// return until polkit has asked the person and the person has answered,
/// which is a dialog somebody may reach for a password manager to answer.
/// Five minutes is longer than any prompt anyone should be left with, and it
/// is still bounded — a call that never answers is not a thread left forever.
const ACTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        // revision — bumped once, after every list is in place
        // filterText / showSystem / showUser / servicesOnly — the state QML sets
        // unit* — index-aligned rows: name, description, scope token, active
        //   state, raw sub state, kind token and whether an action is offered
        // totalCount / shownCount — before and after the filters
        // systemAvailable / userAvailable and their reasons — one per bus
        // actionOutcome / actionUnit / actionKind — the last action asked for
        // startFailed — the sampling thread could not be created
        #[qobject]
        #[qml_element]
        #[qproperty(i32, revision)]
        #[qproperty(QString, filter_text)]
        #[qproperty(bool, show_system)]
        #[qproperty(bool, show_user)]
        #[qproperty(bool, services_only)]
        #[qproperty(QStringList, unit_names)]
        #[qproperty(QStringList, unit_descriptions)]
        #[qproperty(QStringList, unit_scopes)]
        #[qproperty(QStringList, unit_actives)]
        #[qproperty(QStringList, unit_subs)]
        #[qproperty(QStringList, unit_kinds)]
        #[qproperty(QVariant, unit_actionable)]
        #[qproperty(i32, total_count)]
        #[qproperty(i32, shown_count)]
        #[qproperty(bool, system_available)]
        #[qproperty(QString, system_reason_kind)]
        #[qproperty(QString, system_reason_path)]
        #[qproperty(bool, user_available)]
        #[qproperty(QString, user_reason_kind)]
        #[qproperty(QString, user_reason_path)]
        #[qproperty(QString, action_outcome)]
        #[qproperty(QString, action_unit)]
        #[qproperty(QString, action_kind)]
        #[qproperty(QString, action_scope)]
        #[qproperty(bool, start_failed)]
        type HematitaServices = super::HematitaServicesRust;

        /// Subscribes to the shared sampler, once. The window calls it when it
        /// is up.
        #[qinvokable]
        fn start(self: Pin<&mut HematitaServices>);

        /// Re-projects the last listing with the current state. QML calls it
        /// after changing the filter or a toggle.
        #[qinvokable]
        fn refresh(self: Pin<&mut HematitaServices>);

        /// `StartUnit` on the manager `scope` names.
        #[qinvokable]
        fn start_unit(self: Pin<&mut HematitaServices>, name: QString, scope: QString);

        /// `StopUnit` on the manager `scope` names.
        #[qinvokable]
        fn stop_unit(self: Pin<&mut HematitaServices>, name: QString, scope: QString);

        /// `RestartUnit` on the manager `scope` names.
        #[qinvokable]
        fn restart_unit(self: Pin<&mut HematitaServices>, name: QString, scope: QString);

        /// Forgets the last action's outcome. The page calls it when the
        /// selection moves, because an answer about one unit is not an answer
        /// about the next.
        #[qinvokable]
        fn clear_action(self: Pin<&mut HematitaServices>);
    }

    impl cxx_qt::Threading for HematitaServices {}
}

pub struct HematitaServicesRust {
    revision: i32,
    filter_text: QString,
    show_system: bool,
    show_user: bool,
    services_only: bool,
    unit_names: QStringList,
    unit_descriptions: QStringList,
    unit_scopes: QStringList,
    unit_actives: QStringList,
    unit_subs: QStringList,
    unit_kinds: QStringList,
    unit_actionable: QVariant,
    total_count: i32,
    shown_count: i32,
    system_available: bool,
    system_reason_kind: QString,
    system_reason_path: QString,
    user_available: bool,
    user_reason_kind: QString,
    user_reason_path: QString,
    action_outcome: QString,
    action_unit: QString,
    action_kind: QString,
    action_scope: QString,
    start_failed: bool,
    started: bool,
    last_generation: u64,
    /// The token of the action being waited for. Every action takes the next
    /// one, so an outcome that comes back under an older token is dropped.
    action_token: u64,
    /// The last listing of each bus, kept so a toggle re-projects without
    /// waiting for the next service tick.
    system_units: Vec<Unit>,
    user_units: Vec<Unit>,
}

impl Default for HematitaServicesRust {
    fn default() -> Self {
        Self {
            revision: 0,
            filter_text: QString::default(),
            show_system: true,
            show_user: true,
            // A machine has hundreds of mounts, slices and devices and a few
            // dozen services; the page opens on what a person came for.
            services_only: true,
            unit_names: QStringList::default(),
            unit_descriptions: QStringList::default(),
            unit_scopes: QStringList::default(),
            unit_actives: QStringList::default(),
            unit_subs: QStringList::default(),
            unit_kinds: QStringList::default(),
            unit_actionable: doubles(&[]),
            total_count: 0,
            shown_count: 0,
            system_available: true,
            system_reason_kind: QString::default(),
            system_reason_path: QString::default(),
            user_available: true,
            user_reason_kind: QString::default(),
            user_reason_path: QString::default(),
            action_outcome: QString::default(),
            action_unit: QString::default(),
            action_kind: QString::default(),
            action_scope: QString::default(),
            start_failed: false,
            started: false,
            last_generation: 0,
            action_token: 0,
            system_units: Vec::new(),
            user_units: Vec::new(),
        }
    }
}

/// Which bus a scope token names. An unknown token is the session bus: it is
/// the manager that authorises nothing, so a token nobody recognises can only
/// fail locally, never ask the system for something.
fn scope_of_token(token: &str) -> Scope {
    if token == Scope::System.as_str() {
        Scope::System
    } else {
        Scope::User
    }
}

/// What one `StartUnit`/`StopUnit`/`RestartUnit` call means to the person: a
/// method error is read through its name, anything else is a failure.
fn outcome_of_call(result: Result<(), zbus::Error>) -> Outcome {
    match result {
        Ok(()) => Outcome::Done,
        Err(zbus::Error::MethodError(name, ..)) => services::outcome_of_dbus_error(name.as_str()),
        Err(_) => Outcome::Failed,
    }
}

/// What one call answered, including the reply that is not a reply: a `None`
/// body is a call that was sent but whose answer never arrived as one, which
/// is a failure like any other rather than a silent success.
fn outcome_of_reply(
    result: Result<Option<zbus::zvariant::OwnedObjectPath>, zbus::Error>,
) -> Outcome {
    match result {
        Ok(Some(_job)) => Outcome::Done,
        Ok(None) => Outcome::Failed,
        Err(error) => outcome_of_call(Err(error)),
    }
}

/// One unit action, start to finish, on this worker thread.
///
/// The connection is this call's own and is built with [`ACTION_TIMEOUT`],
/// because the reply does not come back until the person has answered polkit.
/// The call carries `AllowInteractiveAuth`: systemd passes that message flag
/// to polkit as `AllowUserInteraction`, and without it polkit never consults
/// an authentication agent — it refuses with
/// `InteractiveAuthorizationRequired` even on a session that has one (see
/// ADR 0010). The returned object path is the job systemd queued; the page
/// reports the call, not the job, so it is dropped.
fn call_unit(scope: Scope, name: &str, method: &str) -> Outcome {
    let builder = match scope {
        Scope::System => zbus::blocking::connection::Builder::system(),
        Scope::User => zbus::blocking::connection::Builder::session(),
    };
    let Ok(connection) = builder.and_then(|builder| builder.method_timeout(ACTION_TIMEOUT).build())
    else {
        return Outcome::Failed;
    };
    let Ok(proxy) = zbus::blocking::Proxy::new(
        &connection,
        SYSTEMD_SERVICE,
        SYSTEMD_OBJECT,
        SYSTEMD_MANAGER,
    ) else {
        return Outcome::Failed;
    };
    outcome_of_reply(
        proxy.call_with_flags::<_, _, zbus::zvariant::OwnedObjectPath>(
            method,
            zbus::proxy::MethodFlags::AllowInteractiveAuth.into(),
            &(name, REPLACE),
        ),
    )
}

impl qobject::HematitaServices {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let qt = self.qt_thread();
        let outcome = sampler::subscribe(move |snapshot: &Snapshot| {
            let Some(services) = &snapshot.services else {
                return;
            };
            let generation = snapshot.generation;
            let services = services.clone();
            let _ = qt.queue(move |hub: Pin<&mut qobject::HematitaServices>| {
                hub.apply(generation, services);
            });
        });
        if outcome.is_err() {
            self.as_mut().set_start_failed(true);
        }
    }

    /// Applies one listing whole. A listing that arrived out of order is
    /// dropped rather than shown beside a newer one.
    fn apply(mut self: Pin<&mut Self>, generation: u64, snapshot: ServiceSnapshot) {
        if !publish::accepts(generation, self.rust().last_generation) {
            return;
        }
        self.as_mut().rust_mut().last_generation = generation;
        match snapshot.system {
            Section::Available(units) => {
                self.as_mut().rust_mut().system_units = units;
                self.as_mut().set_system_available(true);
                self.as_mut().set_system_reason_kind(QString::default());
                self.as_mut().set_system_reason_path(QString::default());
            }
            Section::Unavailable(Reason { kind, path }) => {
                self.as_mut().rust_mut().system_units = Vec::new();
                self.as_mut().set_system_available(false);
                self.as_mut()
                    .set_system_reason_kind(QString::from(kind.as_str()));
                self.as_mut()
                    .set_system_reason_path(QString::from(path.as_str()));
            }
        }
        match snapshot.user {
            Section::Available(units) => {
                self.as_mut().rust_mut().user_units = units;
                self.as_mut().set_user_available(true);
                self.as_mut().set_user_reason_kind(QString::default());
                self.as_mut().set_user_reason_path(QString::default());
            }
            Section::Unavailable(Reason { kind, path }) => {
                self.as_mut().rust_mut().user_units = Vec::new();
                self.as_mut().set_user_available(false);
                self.as_mut()
                    .set_user_reason_kind(QString::from(kind.as_str()));
                self.as_mut()
                    .set_user_reason_path(QString::from(path.as_str()));
            }
        }
        self.as_mut().refresh();
    }

    pub fn refresh(mut self: Pin<&mut Self>) {
        let units = self.rust().all_units();
        let filter = self.rust().filter_text.to_string();
        let show_system = self.rust().show_system;
        let show_user = self.rust().show_user;
        let services_only = self.rust().services_only;
        let order = services::project(&units, &filter, show_system, show_user, services_only);

        let names = strings(order.iter().map(|&index| units[index].name.clone()));
        let descriptions = strings(order.iter().map(|&index| units[index].description.clone()));
        let scopes = strings(
            order
                .iter()
                .map(|&index| units[index].scope.as_str().to_owned()),
        );
        let actives = strings(order.iter().map(|&index| units[index].active.clone()));
        let subs = strings(order.iter().map(|&index| units[index].sub.clone()));
        let kinds = strings(
            order
                .iter()
                .map(|&index| UnitKind::of_name(&units[index].name).as_str().to_owned()),
        );
        let actionable = doubles(
            &order
                .iter()
                .map(|&index| {
                    if is_actionable(UnitKind::of_name(&units[index].name)) {
                        1.0
                    } else {
                        0.0
                    }
                })
                .collect::<Vec<_>>(),
        );

        self.as_mut().set_unit_names(names);
        self.as_mut().set_unit_descriptions(descriptions);
        self.as_mut().set_unit_scopes(scopes);
        self.as_mut().set_unit_actives(actives);
        self.as_mut().set_unit_subs(subs);
        self.as_mut().set_unit_kinds(kinds);
        self.as_mut().set_unit_actionable(actionable);
        self.as_mut()
            .set_total_count(i32::try_from(units.len()).unwrap_or(i32::MAX));
        self.as_mut()
            .set_shown_count(i32::try_from(order.len()).unwrap_or(i32::MAX));
        // Last, so the page rebuilds once, with every list in place.
        let next = self.rust().revision.wrapping_add(1).max(1);
        self.as_mut().set_revision(next);
    }

    pub fn clear_action(mut self: Pin<&mut Self>) {
        // Forgetting the question also retires its token: an answer that
        // arrives after the person moved on is not written back.
        let token = self.rust().action_token.wrapping_add(1);
        self.as_mut().rust_mut().action_token = token;
        self.as_mut().set_action_outcome(QString::default());
        self.as_mut().set_action_unit(QString::default());
        self.as_mut().set_action_kind(QString::default());
        self.as_mut().set_action_scope(QString::default());
    }

    pub fn start_unit(self: Pin<&mut Self>, name: QString, scope: QString) {
        self.act(name, scope, "StartUnit", "start");
    }

    pub fn stop_unit(self: Pin<&mut Self>, name: QString, scope: QString) {
        self.act(name, scope, "StopUnit", "stop");
    }

    pub fn restart_unit(self: Pin<&mut Self>, name: QString, scope: QString) {
        self.act(name, scope, "RestartUnit", "restart");
    }

    /// The one action path. A kind that has nothing to start or stop is
    /// refused here, without a thread and without a bus; everything else is
    /// one call on one worker thread, whose outcome comes back through the Qt
    /// thread.
    fn act(
        mut self: Pin<&mut Self>,
        name: QString,
        scope: QString,
        method: &'static str,
        kind: &'static str,
    ) {
        let unit = name.to_string();
        let scope = scope_of_token(&scope.to_string());
        // The four properties are written together, so the page never reads
        // this action's outcome beside the last one's unit.
        self.as_mut().set_action_unit(QString::from(unit.as_str()));
        self.as_mut().set_action_kind(QString::from(kind));
        self.as_mut()
            .set_action_scope(QString::from(scope.as_str()));
        // The hub is the authority on what exists: a name the latest listing
        // of that manager does not carry is refused here, rather than sent to
        // systemd to be answered with `NoSuchUnit`.
        if !self.rust().lists(&unit, scope) || !is_actionable(UnitKind::of_name(&unit)) {
            let token = self.rust().action_token.wrapping_add(1);
            self.as_mut().rust_mut().action_token = token;
            self.as_mut()
                .set_action_outcome(QString::from(Outcome::Refused.as_str()));
            return;
        }
        // "asked, waiting": the prompt may be up, and a page saying nothing
        // while polkit waits would read as a click that did not land.
        self.as_mut()
            .set_action_outcome(QString::from(publish::PENDING));
        let token = self.rust().action_token.wrapping_add(1);
        self.as_mut().rust_mut().action_token = token;
        let qt = self.qt_thread();
        let spawned = std::thread::Builder::new()
            .name("hematita-systemd".to_owned())
            .spawn(move || {
                let outcome = call_unit(scope, &unit, method);
                let _ = qt.queue(move |mut hub: Pin<&mut qobject::HematitaServices>| {
                    if publish::still_current(token, hub.rust().action_token) {
                        hub.as_mut()
                            .set_action_outcome(QString::from(outcome.as_str()));
                    }
                });
            });
        if spawned.is_err() {
            self.as_mut()
                .set_action_outcome(QString::from(Outcome::Failed.as_str()));
        }
    }
}

impl HematitaServicesRust {
    /// Whether the latest listing of that manager carries this unit.
    fn lists(&self, name: &str, scope: Scope) -> bool {
        let units = match scope {
            Scope::System => &self.system_units,
            Scope::User => &self.user_units,
        };
        units.iter().any(|unit| unit.name == name)
    }

    /// Both buses' units in one list, user first, for `project` to order.
    fn all_units(&self) -> Vec<Unit> {
        let mut units = Vec::with_capacity(self.user_units.len() + self.system_units.len());
        units.extend(self.user_units.iter().cloned());
        units.extend(self.system_units.iter().cloned());
        units
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(name: &str, scope: Scope) -> Unit {
        Unit {
            name: name.to_owned(),
            description: "a unit".to_owned(),
            scope,
            active: "active".to_owned(),
            sub: "running".to_owned(),
        }
    }

    fn method_error(name: &str) -> zbus::Error {
        let message = zbus::message::Message::method_call("/org/freedesktop/systemd1", "StopUnit")
            .expect("a well-formed path and member")
            .build(&())
            .expect("a message with an empty body");
        zbus::Error::MethodError(
            zbus::names::OwnedErrorName::try_from(name).expect("a well-formed error name"),
            None,
            message,
        )
    }

    #[test]
    fn a_scope_token_names_a_bus_and_an_unknown_one_is_the_session() {
        assert_eq!(scope_of_token("system"), Scope::System);
        assert_eq!(scope_of_token("user"), Scope::User);
        assert_eq!(scope_of_token(""), Scope::User);
        assert_eq!(scope_of_token("root"), Scope::User);
    }

    #[test]
    fn a_call_reads_as_the_outcome_its_error_name_means() {
        assert_eq!(outcome_of_call(Ok(())), Outcome::Done);
        assert_eq!(
            outcome_of_call(Err(method_error(
                "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired"
            ))),
            Outcome::NoAgent
        );
        assert_eq!(
            outcome_of_call(Err(method_error("org.freedesktop.DBus.Error.AccessDenied"))),
            Outcome::Denied
        );
        assert_eq!(
            outcome_of_call(Err(method_error("org.freedesktop.systemd1.NoSuchUnit"))),
            Outcome::Failed
        );
        // Anything that is not a method error is a failure, not a refusal.
        assert_eq!(
            outcome_of_call(Err(zbus::Error::InvalidReply)),
            Outcome::Failed
        );
    }

    #[test]
    fn a_reply_without_a_job_path_is_a_failure_not_a_success() {
        let job = zbus::zvariant::OwnedObjectPath::try_from("/org/freedesktop/systemd1/job/7")
            .expect("a well-formed object path");
        assert_eq!(outcome_of_reply(Ok(Some(job))), Outcome::Done);
        assert_eq!(outcome_of_reply(Ok(None)), Outcome::Failed);
        assert_eq!(
            outcome_of_reply(Err(method_error(
                "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired"
            ))),
            Outcome::NoAgent
        );
    }

    #[test]
    fn a_unit_the_latest_listing_does_not_carry_is_not_asked_for() {
        let state = HematitaServicesRust {
            system_units: vec![unit("sshd.service", Scope::System)],
            user_units: vec![unit("at-spi-dbus-bus.service", Scope::User)],
            ..HematitaServicesRust::default()
        };
        assert!(state.lists("sshd.service", Scope::System));
        assert!(state.lists("at-spi-dbus-bus.service", Scope::User));
        // The right name on the wrong manager is not this unit.
        assert!(!state.lists("sshd.service", Scope::User));
        assert!(!state.lists("nothing.service", Scope::System));
    }

    #[test]
    fn only_a_service_or_a_socket_is_ever_asked_of_the_bus() {
        // The refusal `act` applies before it spawns anything, expressed over
        // the same predicate it calls.
        for name in ["sshd.service", "cups.socket"] {
            assert!(is_actionable(UnitKind::of_name(name)));
        }
        for name in ["user.slice", "boot.mount", "x.timer", "weird"] {
            assert!(!is_actionable(UnitKind::of_name(name)));
        }
    }

    #[test]
    fn both_buses_units_are_projected_together_with_the_user_first() {
        let state = HematitaServicesRust {
            system_units: vec![unit("sshd.service", Scope::System)],
            user_units: vec![unit("at-spi-dbus-bus.service", Scope::User)],
            ..HematitaServicesRust::default()
        };
        let units = state.all_units();
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].scope, Scope::User);
        assert_eq!(units[1].scope, Scope::System);
        assert_eq!(
            services::project(&units, "", true, true, true),
            vec![0, 1],
            "the user's unit leads"
        );
    }
}
