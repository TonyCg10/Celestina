//! The Power page (CEL-2-F): the battery (its charge, what it is doing and
//! the time left or to full) and one row per power profile the daemon
//! offers, the active one marked. A click on another profile asks for it.
//! Either half is simply absent while its service is: a desktop has no
//! battery row, and a machine without power-profiles-daemon has no profile
//! rows. With neither, the page says there is no data.

use celestina_paint::{Role, Theme};
use celestina_shell::connectivity::{
    validate, Action, BatterySection, ChargeState, Degraded, ProfileName, ProfileSection,
};
use celestina_ui::{view, Key, View};

use super::{
    note_list,
    rows::{self, apply_status, Glyph, Line, Trailing},
    PageCx, PageView,
};
use crate::{connectivity::state::ConnectivityState, copy, island::state::PAGE_SIZE};

/// The Power page.
#[derive(Clone, Copy, Debug)]
pub struct Power;

/// The battery's row.
#[must_use]
pub fn battery_key() -> Key {
    Key::named("power-battery")
}

/// The row of profile `name`.
#[must_use]
pub fn profile_key(name: &str) -> Key {
    Key::named(&format!("power-profile:{name}"))
}

/// The note about degraded performance.
#[must_use]
pub fn degraded_key() -> Key {
    Key::named("power-degraded")
}

/// The title of profile `name`'s row: the three the daemon defines, else the
/// name itself (service data, limited to `[a-z0-9-_]`, shown only as a row's
/// name like a network's). It never goes into a sentence of this crate.
#[must_use]
pub fn profile_title(name: &str) -> String {
    match name {
        "performance" => copy::PROFILE_PERFORMANCE.to_owned(),
        "balanced" => copy::PROFILE_BALANCED.to_owned(),
        "power-saver" => copy::PROFILE_POWER_SAVER.to_owned(),
        other => other.to_owned(),
    }
}

/// The icon of profile `name`.
#[must_use]
pub fn profile_icon(name: &str) -> &'static str {
    match name {
        "performance" => "zap",
        "power-saver" => "leaf",
        _ => "gauge",
    }
}

/// The word for what the battery is doing.
#[must_use]
pub fn charge_word(state: ChargeState) -> &'static str {
    match state {
        ChargeState::Charging => copy::BATTERY_CHARGING,
        ChargeState::Discharging => copy::BATTERY_DISCHARGING,
        ChargeState::Empty => copy::BATTERY_EMPTY,
        ChargeState::Full => copy::BATTERY_FULL,
        ChargeState::PendingCharge => copy::BATTERY_PENDING_CHARGE,
        ChargeState::PendingDischarge => copy::BATTERY_PENDING_DISCHARGE,
        ChargeState::Unknown => copy::BATTERY_UNKNOWN,
    }
}

fn battery_line(battery: &BatterySection) -> Line {
    let word = charge_word(battery.state());
    let detail = match battery.state() {
        ChargeState::Discharging => battery
            .time_to_empty()
            .map(|t| copy::battery_remaining(word, &copy::duration(t.as_secs()))),
        ChargeState::Charging => battery
            .time_to_full()
            .map(|t| copy::battery_to_full(word, &copy::duration(t.as_secs()))),
        _ => None,
    }
    .unwrap_or_else(|| word.to_owned());
    let low = battery.state() == ChargeState::Discharging && battery.percentage() <= 15;
    let icon = if battery.state() == ChargeState::Charging {
        "battery-charging"
    } else {
        "zap"
    };
    Line {
        key: battery_key(),
        tag: "power-battery".to_owned(),
        glyph: Glyph::Icon(icon),
        tint: if low { Role::Danger } else { Role::TextHi },
        title: copy::battery_percent(battery.percentage()),
        title_role: Role::TextHi,
        detail: Some(detail),
        trailing: Trailing::Nothing,
        action: None,
        note: None,
    }
}

fn profile_line(state: &ConnectivityState, section: &ProfileSection, name: &ProfileName) -> Line {
    let active = section.active() == Some(name);
    let asked = Action::SetProfile(name.clone());
    let allowed = !active && validate(&asked, &state.inventory()).is_ok();
    let mut line = Line {
        key: profile_key(name.as_str()),
        tag: format!("power-profile:{}", name.as_str()),
        glyph: Glyph::Icon(profile_icon(name.as_str())),
        tint: if active { Role::Accent } else { Role::TextHi },
        title: profile_title(name.as_str()),
        title_role: Role::TextHi,
        detail: None,
        trailing: if active {
            Trailing::Text(copy::PROFILE_ACTIVE.to_owned(), Role::Accent)
        } else {
            Trailing::Nothing
        },
        action: allowed.then_some(asked),
        note: None,
    };
    apply_status(&mut line, state.profile_status(name));
    line
}

fn degraded_line(section: &ProfileSection) -> Option<Line> {
    let text = match section.degraded() {
        Degraded::No => return None,
        Degraded::LapDetected => copy::PROFILE_DEGRADED_LAP,
        Degraded::HighTemperature => copy::PROFILE_DEGRADED_HEAT,
        Degraded::Other => copy::PROFILE_DEGRADED_OTHER,
    };
    let mut line = Line::text("power-degraded", text, Role::TextLo);
    line.key = degraded_key();
    Some(line)
}

/// The lines the page draws for `state` on a surface of `max_size`; `None`
/// while neither the battery nor the profile daemon has anything to show.
fn shown(state: &ConnectivityState, theme: &Theme, max_size: (f32, f32)) -> Option<Vec<Line>> {
    let battery = state.battery().ready().copied();
    let profiles = state.profiles().ready().copied();
    if battery.is_none() && profiles.is_none() {
        return None;
    }
    let top: Vec<Line> = battery.map(battery_line).into_iter().collect();
    let body: Vec<Line> = profiles
        .map(|section| {
            section
                .profiles()
                .iter()
                .map(|name| profile_line(state, section, name))
                .collect()
        })
        .unwrap_or_default();
    let bottom: Vec<Line> = profiles.and_then(degraded_line).into_iter().collect();
    Some(rows::fit(theme, max_size, top, body, bottom))
}

/// What a click or Enter on the node `key` asks of the shell: only a profile
/// row the page draws and the shell would accept.
#[must_use]
pub fn action_for(
    state: &ConnectivityState,
    theme: &Theme,
    max_size: (f32, f32),
    key: Key,
) -> Option<Action> {
    rows::action_of(&shown(state, theme, max_size)?, key)
}

impl PageView for Power {
    fn title(&self) -> &str {
        copy::PAGE_POWER
    }

    fn view(&self, cx: &PageCx<'_>) -> View {
        let content = match shown(cx.connectivity, cx.theme, cx.max_size) {
            Some(lines) => rows::list(cx.theme, Key::named("power-rows"), &lines),
            None => note_list(cx.theme, Key::named("power-rows"), copy::POWER_UNAVAILABLE),
        };
        view::col(Key::named("power")).child(content)
    }

    /// The default size, taller when the rows need it, up to the surface.
    fn wants(&self, cx: &PageCx<'_>) -> (f32, f32) {
        match shown(cx.connectivity, cx.theme, cx.max_size) {
            Some(lines) => (PAGE_SIZE.0, PAGE_SIZE.1.max(rows::needed(cx.theme, &lines))),
            None => PAGE_SIZE,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use celestina_shell::connectivity::{Expected, Failure, Resolution, Service};
    use celestina_ui::ViewKind;

    use super::*;
    use crate::{
        connectivity::{power::PowerEvent, state::sample},
        island::pages::NameOwner,
    };

    const BIG: (f32, f32) = (760.0, 560.0);

    fn cx<'a>(
        state: &'a ConnectivityState,
        owners: &'a NameOwner,
        max: (f32, f32),
        theme: &'a Theme,
    ) -> PageCx<'a> {
        PageCx {
            theme,
            max_size: max,
            model: None,
            output: "DP-1",
            owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,

            connectivity: state,
            phone: &celestina_shell::phone::PhoneState::Absent,
        }
    }

    fn page(state: &ConnectivityState) -> View {
        let (owners, theme) = (NameOwner::default(), Theme::dark());
        Power.view(&cx(state, &owners, BIG, &theme))
    }

    fn wants(state: &ConnectivityState, max: (f32, f32)) -> (f32, f32) {
        let (owners, theme) = (NameOwner::default(), Theme::dark());
        Power.wants(&cx(state, &owners, max, &theme))
    }

    fn nodes(v: &View) -> Vec<&View> {
        let mut out = Vec::new();
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            out.push(n);
            stack.extend(n.children.iter().rev());
        }
        out
    }

    fn texts(v: &View) -> Vec<String> {
        nodes(v)
            .into_iter()
            .filter_map(|n| match &n.kind {
                ViewKind::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn act(state: &ConnectivityState, key: Key) -> Option<Action> {
        action_for(state, &Theme::dark(), BIG, key)
    }

    #[test]
    fn with_no_service_the_page_says_there_is_no_data() {
        let state = ConnectivityState::default();
        let v = page(&state);
        assert_eq!(texts(&v), [copy::POWER_UNAVAILABLE]);
        assert!(nodes(&v).iter().all(|n| !n.interactive && !n.focusable));
        assert_eq!(wants(&state, BIG), PAGE_SIZE);
    }

    #[test]
    fn the_battery_and_the_profiles_each_show_when_their_service_does() {
        let state = sample::ready();
        let said = texts(&page(&state));
        for expected in [
            copy::battery_percent(64).as_str(),
            copy::battery_remaining(copy::BATTERY_DISCHARGING, "2 h").as_str(),
            copy::PROFILE_PERFORMANCE,
            copy::PROFILE_BALANCED,
            copy::PROFILE_POWER_SAVER,
            copy::PROFILE_ACTIVE,
        ] {
            assert!(said.iter().any(|t| t == expected), "{expected}: {said:?}");
        }
        // A desktop: profiles only.
        let mut desktop = ConnectivityState::default();
        desktop.apply_power(&PowerEvent::Profiles(Service::Ready(sample::profiles(
            "balanced",
        ))));
        let said = texts(&page(&desktop));
        assert!(
            said.iter().all(|t| !t.contains(copy::PIECE_BATTERY)),
            "{said:?}"
        );
        assert!(said.iter().any(|t| t == copy::PROFILE_BALANCED));
        // A machine without the daemon: the battery only.
        let mut bare = ConnectivityState::default();
        bare.apply_power(&PowerEvent::Battery(Service::Ready(
            sample::battery(ChargeState::Charging, 20.0).expect("battery"),
        )));
        let said = texts(&page(&bare));
        assert!(
            said.iter().any(|t| t == &copy::battery_percent(20)),
            "{said:?}"
        );
        assert!(said.iter().any(|t| t.starts_with(copy::BATTERY_CHARGING)));
        assert!(said.iter().all(|t| t != copy::PROFILE_BALANCED));
    }

    #[test]
    fn a_click_on_an_inactive_profile_asks_for_it_and_the_active_one_is_inert() {
        let state = sample::ready();
        assert_eq!(
            act(&state, profile_key("performance")),
            Some(Action::SetProfile(sample::profile("performance")))
        );
        assert_eq!(
            act(&state, profile_key("power-saver")),
            Some(Action::SetProfile(sample::profile("power-saver")))
        );
        assert_eq!(act(&state, profile_key("balanced")), None);
        assert_eq!(act(&state, battery_key()), None);
        assert_eq!(act(&state, Key::named("nowhere")), None);
        let v = page(&state);
        let keys: HashSet<Key> = nodes(&v).iter().map(|n| n.key).collect();
        assert_eq!(keys.len(), nodes(&v).len(), "one key per node");
        let focusable: HashSet<Key> = nodes(&v)
            .iter()
            .filter(|n| n.focusable)
            .map(|n| n.key)
            .collect();
        let want: HashSet<Key> = [profile_key("performance"), profile_key("power-saver")].into();
        assert_eq!(focusable, want);
    }

    #[test]
    fn the_battery_row_shows_charging_full_and_unknown_times() {
        let line = |state, percent: f64, tte: i64, ttf: i64| {
            let battery = BatterySection::new(true, state, percent, tte, ttf).expect("battery");
            battery_line(&battery)
        };
        let charging = line(ChargeState::Charging, 50.0, 0, 2100);
        assert_eq!(
            charging.detail.as_deref(),
            Some(copy::battery_to_full(copy::BATTERY_CHARGING, "35 min").as_str())
        );
        assert_eq!(charging.glyph, Glyph::Icon("battery-charging"));
        let full = line(ChargeState::Full, 100.0, 0, 0);
        assert_eq!(full.detail.as_deref(), Some(copy::BATTERY_FULL));
        let unknown_time = line(ChargeState::Discharging, 40.0, 0, 0);
        assert_eq!(
            unknown_time.detail.as_deref(),
            Some(copy::BATTERY_DISCHARGING)
        );
        assert_eq!(unknown_time.tint, Role::TextHi);
        assert_eq!(
            line(ChargeState::Discharging, 10.0, 600, 0).tint,
            Role::Danger
        );
        assert_eq!(copy::duration(7500), "2 h 5 min");
        assert_eq!(copy::duration(3600), "1 h");
        assert_eq!(copy::duration(5), "1 min");
    }

    #[test]
    fn pending_and_failure_show_on_the_profile_row() {
        let mut state = sample::ready();
        assert_eq!(
            state.begin(&Action::SetProfile(sample::profile("performance"))),
            Ok(())
        );
        assert!(texts(&page(&state)).iter().any(|t| t == copy::APPLYING));
        assert_eq!(act(&state, profile_key("performance")), None);
        state.apply_power(&PowerEvent::Resolved(Resolution::Failed(
            Expected::Profile(sample::profile("performance")),
            Failure::Denied,
        )));
        let said = texts(&page(&state));
        assert!(said.iter().any(|t| t == copy::FAIL_DENIED), "{said:?}");
        assert!(act(&state, profile_key("performance")).is_some());
    }

    #[test]
    fn degraded_performance_is_said_by_reason() {
        let mut state = sample::ready();
        for (reason, text) in [
            (Degraded::LapDetected, copy::PROFILE_DEGRADED_LAP),
            (Degraded::HighTemperature, copy::PROFILE_DEGRADED_HEAT),
            (Degraded::Other, copy::PROFILE_DEGRADED_OTHER),
        ] {
            let section = ProfileSection::new("balanced", ["balanced", "performance"], reason);
            state.apply_power(&PowerEvent::Profiles(Service::Ready(section)));
            assert!(texts(&page(&state)).iter().any(|t| t == text), "{reason:?}");
        }
    }

    #[test]
    fn unknown_profile_names_show_as_themselves() {
        assert_eq!(profile_title("custom-1"), "custom-1");
        assert_eq!(profile_icon("custom-1"), "gauge");
        assert_eq!(profile_title("balanced"), copy::PROFILE_BALANCED);
    }

    #[test]
    fn the_page_never_draws_past_the_surface_or_the_profile_bound() {
        let names: Vec<String> = (0..30).map(|n| format!("p{n}")).collect();
        let mut state = sample::ready();
        state.apply_power(&PowerEvent::Profiles(Service::Ready(ProfileSection::new(
            "p0",
            names,
            Degraded::LapDetected,
        ))));
        let theme = Theme::dark();
        for height in [0.0, 120.0, 200.0, 300.0, 560.0, 4000.0] {
            let max = (760.0, height);
            let lines = shown(&state, &theme, max).expect("lines");
            let units: usize = lines
                .iter()
                .map(|l| 1 + usize::from(l.note.is_some()))
                .sum();
            assert!(units <= rows::room(&theme, max), "{height}");
            let (_, wanted) = wants(&state, max);
            assert!(wanted <= height.max(PAGE_SIZE.1), "{height}: {wanted}");
            let profiles = lines
                .iter()
                .filter(|l| l.tag.starts_with("power-profile:"))
                .count();
            assert!(profiles <= celestina_shell::connectivity::MAX_PROFILES);
        }
    }
}
