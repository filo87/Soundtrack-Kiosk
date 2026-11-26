use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};

const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const MAX_FAILURES: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(60);

#[derive(Default)]
struct Inner {
    unlocked_until: Option<Instant>,
    failures: u32,
    locked_until: Option<Instant>,
}

#[derive(Default)]
pub struct AdminSession {
    inner: Mutex<Inner>,
}

impl AdminSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attempt(&self, correct: bool) -> AppResult<()> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if let Some(until) = inner.locked_until {
            if until > now {
                let secs = (until - now).as_secs().max(1);
                return Err(AppError::admin_locked(format!(
                    "Too many attempts. Try again in {secs}s."
                )));
            }
            inner.locked_until = None;
        }

        if correct {
            inner.failures = 0;
            inner.unlocked_until = Some(now + IDLE_TIMEOUT);
            Ok(())
        } else {
            inner.failures += 1;
            if inner.failures >= MAX_FAILURES {
                inner.failures = 0;
                inner.locked_until = Some(now + LOCKOUT);
                Err(AppError::admin_locked(
                    "Too many attempts. Locked for 60 seconds.",
                ))
            } else {
                Err(AppError::admin_locked("Incorrect PIN."))
            }
        }
    }

    /// Requires an unlocked session and refreshes the idle timer.
    pub fn require(&self) -> AppResult<()> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        match inner.unlocked_until {
            Some(until) if until > now => {
                inner.unlocked_until = Some(now + IDLE_TIMEOUT);
                Ok(())
            }
            _ => {
                inner.unlocked_until = None;
                Err(AppError::admin_locked("Admin session expired."))
            }
        }
    }

    pub fn lock(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.unlocked_until = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlocks_and_expires() {
        let session = AdminSession::new();
        assert!(session.require().is_err());
        session.attempt(true).unwrap();
        assert!(session.require().is_ok());
        session.lock();
        assert!(session.require().is_err());
    }

    #[test]
    fn locks_out_after_five_failures() {
        let session = AdminSession::new();
        for _ in 0..4 {
            assert!(session.attempt(false).is_err());
        }
        let err = session.attempt(false).unwrap_err();
        assert_eq!(err.code(), crate::error::ADMIN_LOCKED);
        assert!(session.attempt(true).is_err());
    }
}
