//! The non-dumpable window of an open polkit request (CEL-2-I; design S-9).
//!
//! While a request is active the shell holds a typed answer in memory. The
//! window sets `PR_SET_DUMPABLE=0` through rustix's safe
//! `process::set_dumpable_behavior` for as long as at least one request is
//! active, and restores the **previous** value when the last one ends: a shell
//! that was not dumpable to begin with stays so. It protects the answer's
//! buffer from a same-user `ptrace` or `/proc/<pid>/mem` read, not the
//! toolkit's freed key copies, and it makes `/proc/<pid>/fd` root-owned for
//! the window (the design's U-11).
//!
//! The flag is behind [`Dumpable`] so a unit test does not change the test
//! process's real flag; the one test that does restores it.

use std::io;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use rustix::process::DumpableBehavior;

/// The process's dumpable attribute, as the window needs it.
pub trait Dumpable: Send {
    /// The current value.
    ///
    /// # Errors
    ///
    /// The kernel's refusal.
    fn current(&mut self) -> io::Result<DumpableBehavior>;
    /// Sets the value.
    ///
    /// # Errors
    ///
    /// The kernel's refusal.
    fn set(&mut self, behavior: DumpableBehavior) -> io::Result<()>;
}

/// The real flag of this process.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessFlag;

impl Dumpable for ProcessFlag {
    fn current(&mut self) -> io::Result<DumpableBehavior> {
        rustix::process::dumpable_behavior().map_err(io::Error::from)
    }

    fn set(&mut self, behavior: DumpableBehavior) -> io::Result<()> {
        rustix::process::set_dumpable_behavior(behavior).map_err(io::Error::from)
    }
}

struct Inner {
    flag: Option<Box<dyn Dumpable>>,
    depth: usize,
    /// The value to restore; `Some` only while this window changed the flag.
    previous: Option<DumpableBehavior>,
}

impl Inner {
    fn open(&mut self) {
        if self.depth == 0 {
            self.engage();
        }
        self.depth += 1;
    }

    fn close(&mut self) {
        if self.depth == 0 {
            return;
        }
        self.depth -= 1;
        if self.depth == 0 {
            self.release();
        }
    }

    fn engage(&mut self) {
        let Some(flag) = self.flag.as_mut() else {
            return;
        };
        let previous = match flag.current() {
            Ok(previous) => previous,
            Err(err) => {
                tracing::warn!(%err, "polkit: the dumpable flag cannot be read; the window stays shut");
                return;
            }
        };
        match flag.set(DumpableBehavior::NotDumpable) {
            Ok(()) => self.previous = Some(previous),
            Err(err) => tracing::warn!(%err, "polkit: the process cannot be made non-dumpable"),
        }
    }

    fn release(&mut self) {
        let (Some(flag), Some(previous)) = (self.flag.as_mut(), self.previous.take()) else {
            return;
        };
        if let Err(err) = flag.set(previous) {
            tracing::warn!(%err, ?previous, "polkit: the previous dumpable value was not restored");
        }
    }
}

fn lock(inner: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    // Plain bookkeeping: a panic elsewhere leaves it usable.
    match inner.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// The window: [`DumpableWindow::open`] for each request that becomes active,
/// [`DumpableWindow::close`] for each that ends. Nested windows restore the
/// value that was in force before the first one opened.
///
/// A request's typed answer can outlive the request for a moment, in the
/// helper's thread (a cancel held back until PAM has read it). The window
/// therefore lends out [`DumpHold`]s: a hold keeps the window shut, from
/// whichever thread drops it, and the previous value comes back only when the
/// window and every hold are gone.
///
/// **The process's flag has one counter.** Every window of
/// [`DumpableWindow::process`], whichever duty start made it, shares one
/// count and one saved value, so a hold that outlives the window of an
/// earlier start (a duty switched off and on again while a thread still holds
/// a secret) neither restores under a request of the next start nor leaves
/// the process non-dumpable for good. A window over a fake flag
/// ([`DumpableWindow::over`]) has a counter of its own, so tests do not
/// meet.
pub struct DumpableWindow {
    inner: Arc<Mutex<Inner>>,
    /// How many `open`s this window has not yet closed.
    open: usize,
}

impl std::fmt::Debug for DumpableWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = lock(&self.inner);
        f.debug_struct("DumpableWindow")
            .field("depth", &inner.depth)
            .field("engaged", &inner.previous.is_some())
            .finish_non_exhaustive()
    }
}

/// A claim on an open window that ends when it is dropped, on any thread.
/// The helper's reaper holds one until the thread that may still hold a typed
/// answer has ended.
pub struct DumpHold {
    inner: Option<Arc<Mutex<Inner>>>,
}

impl std::fmt::Debug for DumpHold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DumpHold")
            .field("held", &self.inner.is_some())
            .finish()
    }
}

impl DumpHold {
    /// A hold on nothing: the window was not open.
    #[must_use]
    pub fn none() -> DumpHold {
        DumpHold { inner: None }
    }
}

impl Drop for DumpHold {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            lock(&inner).close();
        }
    }
}

impl DumpableWindow {
    /// A window that touches nothing: for tests and for states built without
    /// a launcher.
    #[must_use]
    pub fn inert() -> DumpableWindow {
        DumpableWindow::from_flag(None)
    }

    /// A window over this process's real flag. The counter and the saved
    /// value are the process's, shared by every window and hold made this way.
    #[must_use]
    pub fn process() -> DumpableWindow {
        static PROCESS: OnceLock<Arc<Mutex<Inner>>> = OnceLock::new();
        let inner = PROCESS.get_or_init(|| {
            Arc::new(Mutex::new(Inner {
                flag: Some(Box::new(ProcessFlag)),
                depth: 0,
                previous: None,
            }))
        });
        DumpableWindow {
            inner: Arc::clone(inner),
            open: 0,
        }
    }

    /// A second window over the same counter, as two duty starts have over the
    /// process's: for the tests of that sharing.
    #[cfg(test)]
    fn sharing(&self) -> DumpableWindow {
        DumpableWindow {
            inner: Arc::clone(&self.inner),
            open: 0,
        }
    }

    /// A window over `flag`.
    #[must_use]
    pub fn over(flag: Box<dyn Dumpable>) -> DumpableWindow {
        DumpableWindow::from_flag(Some(flag))
    }

    fn from_flag(flag: Option<Box<dyn Dumpable>>) -> DumpableWindow {
        DumpableWindow {
            inner: Arc::new(Mutex::new(Inner {
                flag,
                depth: 0,
                previous: None,
            })),
            open: 0,
        }
    }

    /// Whether at least one request holds the window open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open > 0
    }

    /// A request became active. The first one makes the process
    /// non-dumpable; a flag that cannot be read or set is reported and the
    /// request goes on (the window is a defence in depth, not a gate).
    pub fn open(&mut self) {
        lock(&self.inner).open();
        self.open += 1;
    }

    /// A request ended. The last one (and the last hold) restores the
    /// previous value.
    pub fn close(&mut self) {
        if self.open == 0 {
            return;
        }
        self.open -= 1;
        lock(&self.inner).close();
    }

    /// Keeps the window shut until the returned hold is dropped, even after
    /// the [`DumpableWindow::close`] that follows. Call it before `close`; a
    /// window that is not open gives a hold on nothing.
    #[must_use]
    pub fn hold(&self) -> DumpHold {
        if self.open == 0 {
            return DumpHold::none();
        }
        lock(&self.inner).depth += 1;
        DumpHold {
            inner: Some(Arc::clone(&self.inner)),
        }
    }
}

impl Drop for DumpableWindow {
    /// Closes what is still open; the value is restored when the last hold
    /// is gone too.
    fn drop(&mut self) {
        while self.open > 0 {
            self.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shared cell a test reads what the window did through.
    #[derive(Clone)]
    struct Shared(Arc<Mutex<DumpableBehavior>>);

    impl Shared {
        fn get(&self) -> DumpableBehavior {
            *lock_plain(&self.0)
        }
    }

    fn lock_plain<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        match mutex.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// A flag that lives in a cell, so a test reads what the window did.
    #[derive(Clone)]
    struct Cellular(Shared);

    impl Dumpable for Cellular {
        fn current(&mut self) -> io::Result<DumpableBehavior> {
            Ok(self.0.get())
        }

        fn set(&mut self, behavior: DumpableBehavior) -> io::Result<()> {
            *lock_plain(&(self.0).0) = behavior;
            Ok(())
        }
    }

    fn over(start: DumpableBehavior) -> (DumpableWindow, Shared) {
        let cell = Shared(Arc::new(Mutex::new(start)));
        (DumpableWindow::over(Box::new(Cellular(cell.clone()))), cell)
    }

    #[test]
    fn the_dumpable_window_opens_with_the_first_request_and_restores_the_previous_value() {
        let (mut window, flag) = over(DumpableBehavior::Dumpable);
        assert_eq!(flag.get(), DumpableBehavior::Dumpable, "closed at rest");
        window.open();
        assert_eq!(flag.get(), DumpableBehavior::NotDumpable);
        assert!(window.is_open());
        window.close();
        assert_eq!(flag.get(), DumpableBehavior::Dumpable);
        assert!(!window.is_open());

        // A process that was not dumpable to begin with stays so.
        let (mut window, flag) = over(DumpableBehavior::NotDumpable);
        window.open();
        window.close();
        assert_eq!(flag.get(), DumpableBehavior::NotDumpable);

        // An unbalanced close does nothing.
        let (mut window, flag) = over(DumpableBehavior::Dumpable);
        window.close();
        assert_eq!(flag.get(), DumpableBehavior::Dumpable);
    }

    #[test]
    fn nested_dumpable_windows_restore_the_original_value() {
        // One window opened by two requests.
        let (mut window, flag) = over(DumpableBehavior::Dumpable);
        window.open();
        window.open();
        window.close();
        assert_eq!(
            flag.get(),
            DumpableBehavior::NotDumpable,
            "one is still open"
        );
        window.close();
        assert_eq!(flag.get(), DumpableBehavior::Dumpable);

        // Two windows over one flag, closed last in, first out.
        let (mut outer, flag) = over(DumpableBehavior::Dumpable);
        let mut inner = DumpableWindow::over(Box::new(Cellular(flag.clone())));
        outer.open();
        inner.open();
        inner.close();
        assert_eq!(flag.get(), DumpableBehavior::NotDumpable);
        outer.close();
        assert_eq!(flag.get(), DumpableBehavior::Dumpable);
    }

    #[test]
    fn a_window_dropped_while_open_restores_the_value() {
        let (mut window, flag) = over(DumpableBehavior::Dumpable);
        window.open();
        drop(window);
        assert_eq!(flag.get(), DumpableBehavior::Dumpable);
    }

    #[test]
    fn a_hold_keeps_the_window_shut_after_the_request_closes_it_from_any_thread() {
        let (mut window, flag) = over(DumpableBehavior::Dumpable);
        window.open();
        let hold = window.hold();
        window.close();
        assert_eq!(
            flag.get(),
            DumpableBehavior::NotDumpable,
            "the hold keeps it shut"
        );
        assert!(!window.is_open(), "the request's own claim is gone");
        // Another request opens and closes meanwhile: still shut.
        window.open();
        window.close();
        assert_eq!(flag.get(), DumpableBehavior::NotDumpable);
        // Dropped on another thread (the helper's reaper), it restores the
        // value that was in force before the first open.
        let ended = std::thread::spawn(move || drop(hold)).join();
        assert!(ended.is_ok());
        assert_eq!(flag.get(), DumpableBehavior::Dumpable);

        // A window dropped with a hold pending leaves it to the hold.
        let (mut window, flag) = over(DumpableBehavior::Dumpable);
        window.open();
        let hold = window.hold();
        drop(window);
        assert_eq!(flag.get(), DumpableBehavior::NotDumpable);
        drop(hold);
        assert_eq!(flag.get(), DumpableBehavior::Dumpable);

        // A window that is not open lends a hold on nothing.
        let (window, flag) = over(DumpableBehavior::Dumpable);
        drop(window.hold());
        assert_eq!(flag.get(), DumpableBehavior::Dumpable);
    }

    /// The duty switched off and on again while a thread still holds a secret:
    /// the old hold outlives the first window and the second window opens
    /// meanwhile. The value comes back once, when the last of them is gone.
    #[test]
    fn a_hold_that_outlives_a_window_does_not_restore_under_the_next_one() {
        let (mut first, flag) = over(DumpableBehavior::Dumpable);
        let mut second = first.sharing();
        first.open();
        let old_hold = first.hold();
        drop(first); // the duty goes off; the hold is in a reaper
        assert_eq!(flag.get(), DumpableBehavior::NotDumpable);
        second.open(); // the duty is on again and a request is active
        drop(old_hold); // the old thread ends
        assert_eq!(
            flag.get(),
            DumpableBehavior::NotDumpable,
            "the new request's secret is still covered"
        );
        second.close();
        assert_eq!(
            flag.get(),
            DumpableBehavior::Dumpable,
            "restored once, to what it was before the first open"
        );
    }

    #[test]
    fn the_process_windows_share_one_counter_and_fake_ones_do_not() {
        let (one, _) = over(DumpableBehavior::Dumpable);
        let (other, _) = over(DumpableBehavior::Dumpable);
        assert!(!Arc::ptr_eq(&one.inner, &other.inner));
        let (a, b) = (DumpableWindow::process(), DumpableWindow::process());
        assert!(Arc::ptr_eq(&a.inner, &b.inner));
    }

    #[test]
    fn an_inert_window_touches_nothing() {
        let mut window = DumpableWindow::inert();
        window.open();
        assert!(window.is_open());
        window.close();
        assert!(!window.is_open());
    }

    /// The one test that toggles the real flag of the test process, and puts
    /// it back. Nothing else in the crate's tests builds a process window.
    #[test]
    fn the_real_process_flag_is_toggled_and_restored() {
        let before = rustix::process::dumpable_behavior();
        assert!(before.is_ok(), "the flag is readable: {before:?}");
        let Ok(before) = before else { return };
        let mut window = DumpableWindow::process();
        window.open();
        assert_eq!(
            rustix::process::dumpable_behavior(),
            Ok(DumpableBehavior::NotDumpable)
        );
        window.close();
        assert_eq!(rustix::process::dumpable_behavior(), Ok(before));
    }
}
