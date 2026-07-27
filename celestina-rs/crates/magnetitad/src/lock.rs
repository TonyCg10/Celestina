//! A lock that survives a poisoned mutex.
//!
//! The daemon shares small registries (devices, trust, settings, notification
//! ids, the last clipboard) across link threads. If one thread panicked while
//! holding a lock, std poisons the mutex and every later `.lock().unwrap()`
//! would cascade-panic — turning one bug into a dead daemon. Every guarded
//! value here is rewritten wholesale by its next update, so recovering the
//! data is strictly better than wedging: take the guard and carry on.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// `.lock()` that recovers from poisoning instead of panicking.
pub trait LockOk<T> {
    fn lock_ok(&self) -> MutexGuard<'_, T>;
}

impl<T> LockOk<T> for Mutex<T> {
    fn lock_ok(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::LockOk;
    use std::sync::Mutex;

    #[test]
    fn a_poisoned_lock_still_yields_its_data() {
        let m = Mutex::new(7);
        let _ = std::thread::scope(|s| {
            s.spawn(|| {
                let _guard = m.lock().unwrap();
                panic!("poison the lock");
            })
            .join()
        });
        assert_eq!(*m.lock_ok(), 7);
    }
}
