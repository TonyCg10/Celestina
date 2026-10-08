//! Following a Wi-Fi join to its outcome. NetworkManager answers the
//! activation call at once; whether the network was reached is told later by
//! the active connection's `StateChanged`. A follower thread waits for it,
//! saves a new profile to disk once the join works, and on a failure deletes
//! the profile it created, marks the SSID failed and says so in a notice.

use std::sync::PoisonError;

use zbus::blocking::{Connection, MessageIterator};
use zbus::message::Type as MessageType;
use zbus::zvariant::OwnedObjectPath;
use zbus::MatchRule;

use super::{
    proxy, FailedJoins, Notify, ACTIVE_ACTIVATED, ACTIVE_DEACTIVATED, IFACE_ACTIVE,
    IFACE_CONNECTION, NM,
};
use crate::error::join_failed_message;

/// `NM_ACTIVE_CONNECTION_STATE_REASON_USER_DISCONNECTED` and
/// `_DEVICE_DISCONNECTED`: the join was called off, it did not fail.
const REASON_USER_DISCONNECTED: u32 = 2;
const REASON_DEVICE_DISCONNECTED: u32 = 3;

/// What one `StateChanged(state, reason)` means for a join under way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinOutcome {
    /// Not decided yet: still activating, or deactivating.
    Pending,
    /// The network was reached: keep (save) the profile.
    Keep,
    /// The join failed: any other reason (a wrong passphrase reads
    /// `NO_SECRETS` 9, a network out of reach `CONNECT_TIMEOUT` 6): drop the
    /// new profile.
    Discard,
    /// Called off by the person or the device going away: not a failure.
    Cancelled,
}

#[must_use]
pub fn join_outcome(state: u32, reason: u32) -> JoinOutcome {
    match state {
        ACTIVE_ACTIVATED => JoinOutcome::Keep,
        ACTIVE_DEACTIVATED
            if reason == REASON_USER_DISCONNECTED || reason == REASON_DEVICE_DISCONNECTED =>
        {
            JoinOutcome::Cancelled
        }
        ACTIVE_DEACTIVATED => JoinOutcome::Discard,
        // Activating, deactivating (the reason comes with deactivated) or a
        // state this client does not know.
        _ => JoinOutcome::Pending,
    }
}

/// One join to follow.
pub(super) struct Join {
    pub connection: Connection,
    pub active: OwnedObjectPath,
    /// The profile `AddAndActivateConnection2` created, still volatile; `None`
    /// for a saved profile, which is never deleted here.
    pub volatile: Option<OwnedObjectPath>,
    pub ssid: Vec<u8>,
    pub failed: FailedJoins,
    pub notify: Option<Notify>,
}

/// Spawns the follower. It ends with the join's outcome: NetworkManager
/// always takes an activation to activated or deactivated, under its own
/// timeouts.
pub(super) fn follow(join: Join) {
    let spawned = std::thread::Builder::new()
        .name("cuprita-nm-join".to_owned())
        .spawn(move || {
            let outcome = wait(&join);
            settle(&join, outcome);
        });
    if let Err(error) = spawned {
        eprintln!("Cuprita: cannot follow the Wi-Fi join: {error}");
    }
}

fn wait(join: &Join) -> JoinOutcome {
    // Subscribe first, then read the state once: a join decided between the
    // call's answer and the subscription is still seen.
    let messages = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender(NM)
        .and_then(|b| b.path(join.active.as_str()))
        .and_then(|b| b.interface(IFACE_ACTIVE))
        .and_then(|b| b.member("StateChanged"))
        .map(|b| b.build())
        .and_then(|rule| MessageIterator::for_match_rule(rule, &join.connection, None));
    let messages = match messages {
        Ok(messages) => messages,
        Err(error) => {
            eprintln!("Cuprita: cannot follow the Wi-Fi join: {error}");
            return JoinOutcome::Pending;
        }
    };
    let state = proxy(&join.connection, join.active.as_str(), IFACE_ACTIVE)
        .and_then(|p| p.get_property::<u32>("State").map_err(super::map_error));
    match state {
        Ok(ACTIVE_ACTIVATED) => return JoinOutcome::Keep,
        Ok(_) => {}
        // The active connection is already gone: the join did not hold.
        Err(_) => return JoinOutcome::Discard,
    }
    for message in messages {
        let Ok(message) = message else { continue };
        let Ok((state, reason)) = message.body().deserialize::<(u32, u32)>() else {
            continue;
        };
        let outcome = join_outcome(state, reason);
        if outcome != JoinOutcome::Pending {
            return outcome;
        }
    }
    JoinOutcome::Pending
}

fn settle(join: &Join, outcome: JoinOutcome) {
    let mut failed = join.failed.lock().unwrap_or_else(PoisonError::into_inner);
    match outcome {
        JoinOutcome::Keep => {
            failed.remove(&join.ssid);
            drop(failed);
            if let Some(profile) = &join.volatile {
                let saved = proxy(&join.connection, profile.as_str(), IFACE_CONNECTION)
                    .and_then(|p| p.call::<_, _, ()>("Save", &()).map_err(super::map_error));
                if let Err(error) = saved {
                    eprintln!("Cuprita: the new Wi-Fi profile could not be saved: {error}");
                }
            }
        }
        JoinOutcome::Discard => {
            failed.insert(join.ssid.clone());
            drop(failed);
            if let Some(profile) = &join.volatile {
                // Best effort: a volatile profile may already be gone.
                let _ = proxy(&join.connection, profile.as_str(), IFACE_CONNECTION)
                    .and_then(|p| p.call::<_, _, ()>("Delete", &()).map_err(super::map_error));
            }
            if let Some(notify) = &join.notify {
                notify(join_failed_message(&String::from_utf8_lossy(&join.ssid)));
            }
        }
        // A volatile profile leaves with its deactivation on its own.
        JoinOutcome::Cancelled | JoinOutcome::Pending => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nm::{ACTIVE_ACTIVATING, ACTIVE_DEACTIVATING};

    /// `NM_ACTIVE_CONNECTION_STATE_REASON_NO_SECRETS` and `_CONNECT_TIMEOUT`.
    const REASON_NO_SECRETS: u32 = 9;
    const REASON_CONNECT_TIMEOUT: u32 = 6;

    #[test]
    fn activated_keeps_the_profile() {
        assert_eq!(join_outcome(ACTIVE_ACTIVATED, 0), JoinOutcome::Keep);
    }

    #[test]
    fn a_wrong_passphrase_discards_it() {
        assert_eq!(
            join_outcome(ACTIVE_DEACTIVATED, REASON_NO_SECRETS),
            JoinOutcome::Discard
        );
        assert_eq!(
            join_outcome(ACTIVE_DEACTIVATED, REASON_CONNECT_TIMEOUT),
            JoinOutcome::Discard
        );
    }

    #[test]
    fn calling_it_off_is_not_a_failure() {
        assert_eq!(
            join_outcome(ACTIVE_DEACTIVATED, REASON_USER_DISCONNECTED),
            JoinOutcome::Cancelled
        );
        assert_eq!(
            join_outcome(ACTIVE_DEACTIVATED, REASON_DEVICE_DISCONNECTED),
            JoinOutcome::Cancelled
        );
    }

    #[test]
    fn activating_and_deactivating_wait() {
        assert_eq!(join_outcome(ACTIVE_ACTIVATING, 0), JoinOutcome::Pending);
        assert_eq!(join_outcome(ACTIVE_DEACTIVATING, 0), JoinOutcome::Pending);
    }
}
