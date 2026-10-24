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

use rustix::process::DumpableBehavior;

/// The process's dumpable attribute, as the window needs it.
pub trait Dumpable {
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

/// The window: [`DumpableWindow::open`] for each request that becomes active,
/// [`DumpableWindow::close`] for each that ends. Nested windows restore the
/// value that was in force before the first one opened.
pub struct DumpableWindow {
    flag: Option<Box<dyn Dumpable>>,
    depth: usize,
    /// The value to restore; `Some` only while this window changed the flag.
    previous: Option<DumpableBehavior>,
}

impl std::fmt::Debug for DumpableWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DumpableWindow")
            .field("depth", &self.depth)
            .field("engaged", &self.previous.is_some())
            .finish_non_exhaustive()
    }
}

impl DumpableWindow {
    /// A window that touches nothing: for tests and for states built without
    /// a launcher.
    #[must_use]
    pub fn inert() -> DumpableWindow {
        DumpableWindow {
            flag: None,
            depth: 0,
            previous: None,
        }
    }

    /// The window over this process's real flag.
    #[must_use]
    pub fn process() -> DumpableWindow {
        DumpableWindow::over(Box::new(ProcessFlag))
    }

    /// A window over `flag`.
    #[must_use]
    pub fn over(flag: Box<dyn Dumpable>) -> DumpableWindow {
        DumpableWindow {
            flag: Some(flag),
            depth: 0,
            previous: None,
        }
    }

    /// Whether at least one request holds the window open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.depth > 0
    }

    /// A request became active. The first one makes the process
    /// non-dumpable; a flag that cannot be read or set is reported and the
    /// request goes on (the window is a defence in depth, not a gate).
    pub fn open(&mut self) {
        if self.depth == 0 {
            self.engage();
        }
        self.depth += 1;
    }

    /// A request ended. The last one restores the previous value.
    pub fn close(&mut self) {
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

impl Drop for DumpableWindow {
    fn drop(&mut self) {
        if self.depth > 0 {
            self.depth = 0;
            self.release();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    /// A flag that lives in a cell, so a test reads what the window did.
    #[derive(Clone)]
    struct Cellular(Rc<Cell<DumpableBehavior>>);

    impl Dumpable for Cellular {
        fn current(&mut self) -> io::Result<DumpableBehavior> {
            Ok(self.0.get())
        }

        fn set(&mut self, behavior: DumpableBehavior) -> io::Result<()> {
            self.0.set(behavior);
            Ok(())
        }
    }

    fn over(start: DumpableBehavior) -> (DumpableWindow, Rc<Cell<DumpableBehavior>>) {
        let cell = Rc::new(Cell::new(start));
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
