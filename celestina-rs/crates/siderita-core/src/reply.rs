//! Which asynchronous answer a view still wants.
//!
//! A tab asks questions whose answers arrive later, on its own thread: a
//! recursive search, a listing of the Trash or of Recientes, the applications
//! for a type. Each answer used to be checked only against "is that location
//! still shown?", which is not the same question as "is this the answer to the
//! request that is still standing?". A search closed while it walked still
//! reopened its results when it finished; retyping a query flashed the old
//! hits; a slow Trash listing could land after a newer one and repaint entries
//! a purge had already removed.
//!
//! Two gates answer that question, each for one shape of request:
//!
//! - [`Latest`]: every request supersedes the one before it, and closing the
//!   view retires them all. Only the answer to the newest request still
//!   standing is taken, once.
//! - [`SingleFlight`]: a listing that reads slow mounts runs at most once at a
//!   time. A request made while one is in flight does not start a second
//!   thread; it asks for one more run when the first lands, and the first
//!   answer, which predates it, is not published.

use celestina_core::{Generation, GenerationClock, GenerationExhausted};

/// The newest request wins; closing the view retires them all.
#[derive(Debug, Default)]
pub struct Latest {
    clock: GenerationClock,
    wanted: Option<Generation>,
}

impl Latest {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A new request, superseding whatever was asked before.
    ///
    /// # Errors
    ///
    /// [`GenerationExhausted`] after `u64::MAX` requests; nothing is wanted then.
    pub fn issue(&mut self) -> Result<Generation, GenerationExhausted> {
        self.wanted = None;
        let generation = self.clock.issue()?;
        self.wanted = Some(generation);
        Ok(generation)
    }

    /// Nobody is waiting for any answer any more.
    pub fn retire(&mut self) {
        self.wanted = None;
    }

    /// Whether the answer to `generation` is still the one wanted.
    #[must_use]
    pub fn is_wanted(&self, generation: Generation) -> bool {
        self.wanted == Some(generation)
    }

    /// Takes the answer to `generation` if it is the one wanted. It is taken
    /// once: a second delivery of the same answer is refused.
    pub fn accept(&mut self, generation: Generation) -> bool {
        let wanted = self.is_wanted(generation);
        if wanted {
            self.wanted = None;
        }
        wanted
    }
}

/// What to do with a listing that has landed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Landing {
    /// It answers the request still standing: show it.
    Publish,
    /// A newer request arrived while it ran: drop it and run once more.
    Rerun,
    /// Nobody is waiting for it.
    Drop,
}

/// At most one listing in flight; requests made meanwhile coalesce into one
/// more run.
#[derive(Debug, Default)]
pub struct SingleFlight {
    clock: GenerationClock,
    running: Option<Generation>,
    wanted: bool,
    again: bool,
}

impl SingleFlight {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A request for a fresh listing. Answers the generation to run with, or
    /// `None` when one is already running and this request will be served by
    /// the run that follows it.
    ///
    /// # Errors
    ///
    /// [`GenerationExhausted`] after `u64::MAX` runs.
    pub fn request(&mut self) -> Result<Option<Generation>, GenerationExhausted> {
        self.wanted = true;
        if self.running.is_some() {
            self.again = true;
            return Ok(None);
        }
        let generation = self.clock.issue()?;
        self.running = Some(generation);
        self.again = false;
        Ok(Some(generation))
    }

    /// Whether a listing is still running, wanted or not.
    #[must_use]
    pub const fn in_flight(&self) -> bool {
        self.running.is_some()
    }

    /// The view closed: whatever is running is no longer wanted.
    pub fn retire(&mut self) {
        self.wanted = false;
        self.again = false;
    }

    /// The listing run as `generation` has landed.
    pub fn land(&mut self, generation: Generation) -> Landing {
        if self.running != Some(generation) {
            return Landing::Drop;
        }
        self.running = None;
        if self.again {
            self.again = false;
            return Landing::Rerun;
        }
        if self.wanted {
            Landing::Publish
        } else {
            Landing::Drop
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Landing, Latest, SingleFlight};

    /// SID-9: a search superseded by a newer one, or closed while it walked,
    /// must not publish when it finishes; only the newest search still open
    /// does, and only once.
    #[test]
    fn a_superseded_or_closed_search_answer_is_refused() {
        let mut searches = Latest::new();
        let first = searches.issue().expect("a generation");
        let second = searches.issue().expect("a generation");
        assert!(!searches.accept(first), "superseded by the newer search");
        assert!(searches.accept(second), "the newest search publishes");
        assert!(!searches.accept(second), "and publishes once");

        let third = searches.issue().expect("a generation");
        searches.retire();
        assert!(!searches.accept(third), "closed while it walked");
    }

    /// SID-24: a listing requested while one runs does not start a second
    /// thread; the running one is dropped when it lands and one more run
    /// follows.
    #[test]
    fn a_listing_requested_while_one_runs_coalesces_into_one_more_run() {
        let mut trash = SingleFlight::new();
        let first = trash.request().expect("clock").expect("starts at once");
        assert!(
            trash.request().expect("clock").is_none(),
            "no second thread"
        );
        assert!(trash.request().expect("clock").is_none(), "still none");
        assert_eq!(trash.land(first), Landing::Rerun);
        let second = trash.request().expect("clock").expect("the one more run");
        assert_eq!(trash.land(second), Landing::Publish);
        assert!(!trash.in_flight());
    }

    /// Leaving the location retires the running listing: it lands to nobody,
    /// and a stale generation never publishes.
    #[test]
    fn a_listing_nobody_waits_for_is_dropped() {
        let mut recent = SingleFlight::new();
        let run = recent.request().expect("clock").expect("starts");
        recent.retire();
        assert!(recent.in_flight(), "the thread is still out there");
        assert_eq!(recent.land(run), Landing::Drop);
        assert_eq!(recent.land(run), Landing::Drop, "and a replay is stale");
    }

    /// Reopening the location while a retired listing still runs waits for it
    /// and runs once more, rather than publishing an answer from before.
    #[test]
    fn reopening_while_a_retired_listing_runs_reruns_after_it() {
        let mut trash = SingleFlight::new();
        let run = trash.request().expect("clock").expect("starts");
        trash.retire();
        assert!(trash.request().expect("clock").is_none());
        assert_eq!(trash.land(run), Landing::Rerun);
    }
}
