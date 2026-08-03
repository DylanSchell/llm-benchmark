//! Thread-safe cancellation signaling between the web layer and the Docker
//! execution layer.
//!
//! [`CancellationToken`] is a minimal, dependency-free signal: the web layer
//! (SessionManager) calls [`cancel()`](CancellationToken::cancel) when a user
//! cancels a benchmark session, and the Docker runner polls
//! [`is_cancelled()`](CancellationToken::is_cancelled) while a container is
//! in flight so it can kill the process promptly instead of waiting out the
//! (default 3600s) container timeout.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A one-way cancellation flag that can be shared across threads.
#[derive(Clone, Default, Debug)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates a new, un-cancelled token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks this token as cancelled. Idempotent.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Returns `true` if [`cancel`](CancellationToken::cancel) has been called.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_starts_uncancelled() {
        assert!(!CancellationToken::new().is_cancelled());
    }

    #[test]
    fn cancel_sets_flag() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn cancel_is_idempotent() {
        let token = CancellationToken::new();
        token.cancel();
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn clone_shares_flag() {
        let token = CancellationToken::new();
        let clone = token.clone();
        token.cancel();
        assert!(clone.is_cancelled(), "clone must observe the same flag");
    }
}
