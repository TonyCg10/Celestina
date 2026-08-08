//! Requests the helper has carried out and is waiting to see the effect of.
//!
//! Running a tool successfully means the request was accepted, not that the
//! machine changed. `nmcli connection up` returns before a link is usable,
//! `bluetoothctl power on` returns before the adapter answers, and a device may
//! refuse a connection long after its command exited zero. So an action leaves
//! an entry here, and only a later observation of the provider's own state
//! settles it.
//!
//! Nothing in this module knows a clock, a process or a provider's data. Time
//! arrives as a millisecond stamp and the caller judges its own observations,
//! which is what makes every rule below testable without either.

use crate::snapshot::ProviderId;

/// How many requests may be in flight at once.
///
/// A person clicking menu entries produces a handful; anything beyond this is a
/// host that has stopped waiting for answers, and an unbounded ledger would
/// keep every one of them alive against a timer.
pub const MAX_PENDING: usize = 16;

/// What one observation says about a request that is waiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The observation contains the state the request asked for.
    Confirmed,
    /// The observation shows the request can no longer succeed — what it was
    /// aimed at is gone.
    Contradicted,
    /// Neither yet. The request keeps waiting until it is confirmed,
    /// contradicted, or its deadline passes.
    Waiting,
}

/// Why a request stopped waiting. Every variant is a fixed phrase or a bounded
/// one built from it, so nothing a tool printed reaches a frame through here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ended {
    Confirmed,
    /// What it was aimed at is no longer there.
    Contradicted,
    /// The deadline passed with nothing to show for it.
    Expired,
    /// A newer request for the same thing took its place.
    Superseded,
    /// The helper is shutting down, or the generation it belonged to is gone.
    Cancelled,
}

impl Ended {
    #[must_use]
    pub fn is_confirmed(&self) -> bool {
        matches!(self, Self::Confirmed)
    }

    /// The sentence the host is told. Fixed text: a reason is a diagnosis, and
    /// a diagnosis assembled from another program's output is how hostile text
    /// reaches a frame.
    #[must_use]
    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Confirmed => None,
            Self::Contradicted => Some("what the request was aimed at is gone"),
            Self::Expired => Some("the request was accepted but never took effect"),
            Self::Superseded => Some("a newer request for the same thing replaced it"),
            Self::Cancelled => Some("the helper stopped before the request took effect"),
        }
    }
}

/// Why a request was not even recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The ledger is full. A bounded ledger is the only kind whose entries all
    /// get an answer.
    TooMany,
}

impl Refused {
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::TooMany => "too many requests are already waiting to be confirmed",
        }
    }
}

/// One request, and what it is waiting to see.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Awaiting<T> {
    pub id: String,
    pub provider: ProviderId,
    pub expected: T,
    /// The stamp after which this stops waiting, whatever the machine is doing.
    pub deadline_ms: u64,
}

/// A request that stopped waiting, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settled {
    pub id: String,
    pub ended: Ended,
}

/// The requests in flight.
///
/// Deliberately a plain `Vec`: it is bounded to [`MAX_PENDING`], every
/// operation walks it once, and the order requests were made in is the order
/// their answers are emitted.
#[derive(Clone, Debug)]
pub struct Pending<T> {
    entries: Vec<Entry<T>>,
}

#[derive(Clone, Debug)]
struct Entry<T> {
    request: Awaiting<T>,
    armed: bool,
}

impl<T> Default for Pending<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<T> Pending<T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether anything is waiting on this provider. What `refresh` consults so
    /// two of them cannot be in flight at once.
    #[must_use]
    pub fn awaits(&self, provider: &ProviderId) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.request.provider == *provider)
    }

    #[must_use]
    pub fn awaits_matching<F>(&self, provider: &ProviderId, matches: F) -> bool
    where
        F: Fn(&T) -> bool,
    {
        self.entries
            .iter()
            .any(|entry| entry.request.provider == *provider && matches(&entry.request.expected))
    }

    /// Makes one reserved request visible to observations and its deadline.
    ///
    /// The command worker calls this only after it has written `accepted`, so
    /// no other thread can publish the request's terminal answer first.
    pub fn arm(&mut self, provider: &ProviderId, id: &str, deadline_ms: u64) -> bool {
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.request.provider == *provider && entry.request.id == id)
        else {
            return false;
        };
        entry.armed = true;
        entry.request.deadline_ms = deadline_ms;
        true
    }
}

impl<T: PartialEq> Pending<T> {
    /// Records a request, replacing any earlier one aimed at the same thing.
    ///
    /// # Errors
    ///
    /// Refuses a request that would take the ledger past [`MAX_PENDING`],
    /// because a bounded ledger is the only kind whose entries all get an
    /// answer. The refusal is the caller's to report, before the request is
    /// carried out.
    pub fn accept(&mut self, request: Awaiting<T>) -> Result<Option<Settled>, Refused> {
        let id = request.id.clone();
        let provider = request.provider.clone();
        let deadline_ms = request.deadline_ms;
        let settled = self.reserve_matching(request, PartialEq::eq)?;
        let _ = self.arm(&provider, &id, deadline_ms);
        Ok(settled)
    }

    /// Reserves an unarmed request and replaces an earlier one for the same
    /// target.
    ///
    /// `same_target` deliberately differs from equality: opposite requested
    /// states may still address one adapter or one device.
    pub fn reserve_matching<F>(
        &mut self,
        request: Awaiting<T>,
        same_target: F,
    ) -> Result<Option<Settled>, Refused>
    where
        F: Fn(&T, &T) -> bool,
    {
        // Same provider, same target: the newer request wins and the older one
        // is answered rather than left waiting for an effect nobody will look
        // for. Deterministic in both directions — a duplicate is a supersede.
        let replaces = self.entries.iter().position(|entry| {
            entry.request.provider == request.provider
                && same_target(&entry.request.expected, &request.expected)
        });

        // Checked before anything is removed, so a refusal leaves the ledger
        // exactly as it was rather than discarding an entry it never replaced.
        if replaces.is_none() && self.entries.len() >= MAX_PENDING {
            return Err(Refused::TooMany);
        }

        let superseded = replaces.map(|at| Settled {
            id: self.entries.remove(at).request.id,
            ended: Ended::Superseded,
        });
        self.entries.push(Entry {
            request,
            armed: false,
        });
        Ok(superseded)
    }

    /// Drops one request without an answer.
    ///
    /// For the request whose own tool failed: the caller is about to report
    /// that failure directly, and an entry left behind would answer the same id
    /// a second time when its deadline passed.
    pub fn forget(&mut self, provider: &ProviderId, id: &str) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|entry| entry.request.provider != *provider || entry.request.id != id);
        self.entries.len() != before
    }

    /// Settles every request on one provider against a fresh observation.
    ///
    /// `judge` is handed each expectation and says what the observation shows.
    /// The observation itself never enters this module: what "connected" means
    /// belongs to whatever published it.
    pub fn settle<F>(&mut self, provider: &ProviderId, judge: F) -> Vec<Settled>
    where
        F: Fn(&T) -> Verdict,
    {
        let mut settled = Vec::new();
        self.entries.retain(|entry| {
            if !entry.armed || entry.request.provider != *provider {
                return true;
            }
            match judge(&entry.request.expected) {
                Verdict::Confirmed => {
                    settled.push(Settled {
                        id: entry.request.id.clone(),
                        ended: Ended::Confirmed,
                    });
                    false
                }
                Verdict::Contradicted => {
                    settled.push(Settled {
                        id: entry.request.id.clone(),
                        ended: Ended::Contradicted,
                    });
                    false
                }
                Verdict::Waiting => true,
            }
        });
        settled
    }

    /// Ends every request whose deadline has passed.
    ///
    /// The bound that makes the ledger finite: a request nothing ever confirms
    /// or contradicts still gets an answer, and the host is not left waiting on
    /// its own timeout.
    pub fn expire(&mut self, now_ms: u64) -> Vec<Settled> {
        let mut settled = Vec::new();
        self.entries.retain(|entry| {
            if !entry.armed || entry.request.deadline_ms > now_ms {
                return true;
            }
            settled.push(Settled {
                id: entry.request.id.clone(),
                ended: Ended::Expired,
            });
            false
        });
        settled
    }

    /// Ends everything, for a helper that is stopping or a generation that is
    /// gone. Requests do not survive either: a new process has never run them.
    pub fn cancel_all(&mut self) -> Vec<Settled> {
        self.entries
            .drain(..)
            .map(|entry| Settled {
                id: entry.request.id,
                ended: Ended::Cancelled,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(raw: &str) -> ProviderId {
        ProviderId::new(raw).expect("a valid provider name")
    }

    fn awaiting(id: &str, expected: u8, deadline_ms: u64) -> Awaiting<u8> {
        Awaiting {
            id: id.to_owned(),
            provider: provider("network"),
            expected,
            deadline_ms,
        }
    }

    #[test]
    fn a_request_waits_until_something_is_observed() {
        let mut pending = Pending::new();
        assert_eq!(pending.accept(awaiting("1", 7, 100)), Ok(None));
        assert!(pending.awaits(&provider("network")));

        // Nothing shows the expected state yet.
        assert!(pending
            .settle(&provider("network"), |_| Verdict::Waiting)
            .is_empty());
        assert_eq!(pending.len(), 1);

        let settled = pending.settle(&provider("network"), |_| Verdict::Confirmed);
        assert_eq!(
            settled,
            [Settled {
                id: "1".to_owned(),
                ended: Ended::Confirmed
            }]
        );
        assert!(settled[0].ended.is_confirmed());
        assert_eq!(settled[0].ended.reason(), None);
        assert!(pending.is_empty());
    }

    #[test]
    fn a_reserved_request_cannot_settle_before_accepted_is_written() {
        let mut pending = Pending::new();
        pending
            .reserve_matching(awaiting("1", 7, 0), PartialEq::eq)
            .expect("reserved");

        assert!(pending
            .settle(&provider("network"), |_| Verdict::Confirmed)
            .is_empty());
        assert!(pending.expire(u64::MAX).is_empty());
        assert!(pending.arm(&provider("network"), "1", 100));

        let settled = pending.settle(&provider("network"), |_| Verdict::Confirmed);
        assert_eq!(settled[0].id, "1");
        assert_eq!(settled[0].ended, Ended::Confirmed);

        pending
            .reserve_matching(awaiting("2", 8, 0), PartialEq::eq)
            .expect("reserved");
        assert!(pending.arm(&provider("network"), "2", 100));
        assert!(pending.expire(99).is_empty());
        assert_eq!(pending.expire(100)[0].ended, Ended::Expired);
    }

    /// The rule the whole module exists for: a tool that exited zero has not
    /// confirmed anything, and the request is still waiting afterwards.
    #[test]
    fn a_request_that_nothing_confirms_ends_at_its_deadline() {
        let mut pending = Pending::new();
        pending.accept(awaiting("1", 7, 20_000)).expect("accepted");

        // Every poll before the deadline leaves it waiting.
        for now in [0, 5_000, 10_000, 19_999] {
            assert!(pending.expire(now).is_empty());
        }

        let settled = pending.expire(20_000);
        assert_eq!(settled[0].ended, Ended::Expired);
        assert_eq!(
            settled[0].ended.reason(),
            Some("the request was accepted but never took effect")
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn an_observation_that_rules_a_request_out_ends_it_at_once() {
        let mut pending = Pending::new();
        pending.accept(awaiting("1", 7, 20_000)).expect("accepted");

        let settled = pending.settle(&provider("network"), |_| Verdict::Contradicted);
        assert_eq!(settled[0].ended, Ended::Contradicted);
        assert_eq!(
            settled[0].ended.reason(),
            Some("what the request was aimed at is gone")
        );
    }

    #[test]
    fn a_newer_request_for_the_same_thing_replaces_and_answers_the_older() {
        let mut pending = Pending::new();
        pending.accept(awaiting("1", 7, 20_000)).expect("accepted");

        let superseded = pending
            .accept(awaiting("2", 7, 30_000))
            .expect("accepted")
            .expect("the older request was answered");
        assert_eq!(superseded.id, "1");
        assert_eq!(superseded.ended, Ended::Superseded);
        // Exactly one is left, and it is the newer.
        assert_eq!(pending.len(), 1);
        let settled = pending.settle(&provider("network"), |_| Verdict::Confirmed);
        assert_eq!(settled[0].id, "2");
    }

    #[test]
    fn a_request_for_a_different_thing_waits_beside_it() {
        let mut pending = Pending::new();
        pending.accept(awaiting("1", 7, 20_000)).expect("accepted");

        assert_eq!(pending.accept(awaiting("2", 8, 20_000)), Ok(None));
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn opposite_states_for_one_target_replace_each_other() {
        let mut pending = Pending::new();
        pending.accept(awaiting("1", 10, 20_000)).expect("accepted");

        let superseded = pending
            .reserve_matching(awaiting("2", 11, 20_000), |left, right| {
                left / 10 == right / 10
            })
            .expect("reserved")
            .expect("the earlier state was replaced");

        assert_eq!(superseded.id, "1");
        assert_eq!(superseded.ended, Ended::Superseded);
        assert_eq!(pending.len(), 1);
    }

    /// Requests on one provider are not settled by another's observation.
    #[test]
    fn one_providers_observation_settles_only_its_own_requests() {
        let mut pending = Pending::new();
        pending.accept(awaiting("1", 7, 20_000)).expect("accepted");
        pending
            .accept(Awaiting {
                provider: provider("bluetooth"),
                ..awaiting("2", 7, 20_000)
            })
            .expect("accepted");

        let settled = pending.settle(&provider("bluetooth"), |_| Verdict::Confirmed);
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].id, "2");
        assert!(pending.awaits(&provider("network")));
        assert!(!pending.awaits(&provider("bluetooth")));
    }

    #[test]
    fn a_reused_id_cannot_arm_or_forget_another_provider() {
        let mut pending = Pending::new();
        pending
            .reserve_matching(awaiting("1", 7, 0), PartialEq::eq)
            .expect("network reserved");
        pending
            .reserve_matching(
                Awaiting {
                    provider: provider("bluetooth"),
                    ..awaiting("1", 8, 0)
                },
                PartialEq::eq,
            )
            .expect("bluetooth reserved");

        assert!(pending.arm(&provider("bluetooth"), "1", 100));
        let settled = pending.settle(&provider("bluetooth"), |_| Verdict::Confirmed);
        assert_eq!(settled.len(), 1);
        assert!(pending.awaits(&provider("network")));
        assert!(pending.forget(&provider("network"), "1"));
        assert!(pending.is_empty());
    }

    #[test]
    fn the_ledger_is_bounded_and_says_so_before_carrying_anything_out() {
        let mut pending = Pending::new();
        for index in 0..MAX_PENDING {
            pending
                .accept(awaiting(
                    &index.to_string(),
                    u8::try_from(index).unwrap_or(u8::MAX),
                    20_000,
                ))
                .expect("accepted");
        }

        assert_eq!(
            pending.accept(awaiting("late", 200, 20_000)),
            Err(Refused::TooMany)
        );
        assert_eq!(pending.len(), MAX_PENDING);
    }

    #[test]
    fn a_helper_that_stops_answers_everything_it_was_holding() {
        let mut pending = Pending::new();
        pending.accept(awaiting("1", 7, 20_000)).expect("accepted");
        pending.accept(awaiting("2", 8, 20_000)).expect("accepted");

        let settled = pending.cancel_all();
        assert_eq!(settled.len(), 2);
        assert!(settled.iter().all(|one| one.ended == Ended::Cancelled));
        assert!(pending.is_empty());
        // And nothing is left to resurrect: a second cancel has nothing to say.
        assert!(pending.cancel_all().is_empty());
    }
}
