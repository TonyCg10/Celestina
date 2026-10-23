//! The lock process's exit codes (design §4.3 (b)).

/// How `celestina-lock` ended, as the shell reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockExit {
    /// 0: `unlocked` was printed; the lock ended by PAM.
    Unlocked,
    /// 1: a recoverable error reached `main` before `requested`.
    EarlyError,
    /// 2: bad argv; a bug of the caller.
    Usage,
    /// 10: `finished` before `locked`, either because another client holds
    /// the lock or because the compositor gave up locking.
    FinishedBeforeLocked,
    /// 11: the compositor has no `ext_session_lock_manager_v1`.
    NoLockManager,
    /// 12: no Wayland connection.
    NoConnection,
    /// 13: `finished` after `locked`; the compositor ended the lock.
    FinishedAfterLocked,
    /// 15: the lock's own setup failed before `requested` (hardening, a
    /// tracer attached, the verifier pin).
    SetupFailed,
    /// 16: the Wayland connection was lost after `requested`.
    ConnectionLost,
}

impl LockExit {
    /// The exit status that stands for this ending.
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::Unlocked => 0,
            Self::EarlyError => 1,
            Self::Usage => 2,
            Self::FinishedBeforeLocked => 10,
            Self::NoLockManager => 11,
            Self::NoConnection => 12,
            Self::FinishedAfterLocked => 13,
            Self::SetupFailed => 15,
            Self::ConnectionLost => 16,
        }
    }

    /// The ending a status stands for, if the table has it. A signal, or any
    /// status after `requested` that the table does not name, means the lock
    /// died (design §4.3 (b), last row): the caller treats `None` that way.
    #[must_use]
    pub const fn from_status(status: i32) -> Option<Self> {
        Some(match status {
            0 => Self::Unlocked,
            1 => Self::EarlyError,
            2 => Self::Usage,
            10 => Self::FinishedBeforeLocked,
            11 => Self::NoLockManager,
            12 => Self::NoConnection,
            13 => Self::FinishedAfterLocked,
            15 => Self::SetupFailed,
            16 => Self::ConnectionLost,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_exit_table_matches_the_spec() {
        let table = [
            (0, LockExit::Unlocked),
            (1, LockExit::EarlyError),
            (2, LockExit::Usage),
            (10, LockExit::FinishedBeforeLocked),
            (11, LockExit::NoLockManager),
            (12, LockExit::NoConnection),
            (13, LockExit::FinishedAfterLocked),
            (15, LockExit::SetupFailed),
            (16, LockExit::ConnectionLost),
        ];
        for (code, exit) in table {
            assert_eq!(exit.code(), code, "{exit:?}");
            assert_eq!(LockExit::from_status(code), Some(exit), "{code}");
        }
        for code in [3, 4, 5, 6, 7, 9, 14, 17, 127, 137, 255, -1, i32::MAX] {
            assert_eq!(LockExit::from_status(code), None, "{code}");
        }
    }
}
