//! SessionManager - mirrors Java SessionManager.java
//! Manages benchmark session lifecycle: creation, retrieval, cancellation, removal.

use crate::models::session::BenchmarkSession;
use crate::models::status::RunStatus;
use benchmark_types::cancellation::CancellationToken;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tracing::{info, debug, warn};

/// Manages benchmark session lifecycle.
#[derive(Debug, Clone)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, BenchmarkSession>>>,
    /// Per-session cancellation tokens. `cancel_session` fires the token so
    /// any in-flight Docker container is aborted promptly.
    tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,
}

impl SessionManager {
    /// Create a new SessionManager.
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new benchmark session.
    pub fn create_session(
        &self,
        agent_name: String,
        languages: Vec<String>,
        model: String,
        thinking_level: Option<String>,
        exercise_name: Option<String>,
        retry: bool,
        timeout_ms: u64,
    ) -> BenchmarkSession {
        let session = BenchmarkSession::new(
            agent_name.clone(),
            languages.clone(),
            model.clone(),
            thinking_level,
            exercise_name.clone(),
            retry,
            timeout_ms,
        );
        let id = session.id.clone();

        {
            let mut sessions = self.sessions.write().unwrap();
            sessions.insert(id.clone(), session.clone());
        }
        {
            let mut tokens = self.tokens.write().unwrap();
            tokens.insert(id.clone(), CancellationToken::new());
        }

        info!(
            "Created benchmark session: {} for {}/{} (model: {})",
            id,
            languages.join(","),
            exercise_name.unwrap_or_else(|| "all".to_string()),
            session.model
        );

        session
    }

    /// Get a session by ID (cloned, read-only).
    pub fn get_session(&self, session_id: &str) -> Option<BenchmarkSession> {
        let sessions = self.sessions.read().unwrap();
        sessions.get(session_id).cloned()
    }

    /// Get the cancellation token registered for a session.
    /// The token is fired by [`cancel_session`](Self::cancel_session); the
    /// benchmark executor attaches it to the agent so in-flight Docker runs
    /// abort promptly.
    pub fn get_cancellation_token(&self, session_id: &str) -> Option<CancellationToken> {
        self.tokens.read().unwrap().get(session_id).cloned()
    }

    /// Take the internal message receiver from a session.
    /// Returns a broadcast subscriber if the session exists, None otherwise.
    /// Broadcast channels support multiple consumers — each call creates a fresh subscriber.
    pub fn take_session_receiver(&self, session_id: &str) -> Option<broadcast::Receiver<String>> {
        let sessions = self.sessions.read().unwrap();
        sessions.get(session_id).map(|s| s.setup_sse())
    }

    /// Get all sessions.
    pub fn get_all_sessions(&self) -> HashMap<String, BenchmarkSession> {
        let sessions = self.sessions.read().unwrap();
        sessions.clone()
    }

    /// Cancel a running session.
    pub fn cancel_session(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.write().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            if session.status == RunStatus::RUNNING || session.status == RunStatus::PENDING {
                session.cancel();
                // Fire the cancellation token so the in-flight Docker
                // container (and any between-exercise loops) abort promptly
                // instead of waiting out the container timeout.
                if let Some(token) = self.tokens.read().unwrap().get(session_id) {
                    token.cancel();
                }
                info!("Cancelled session: {}", session_id);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Update an existing session (for status updates during execution).
    pub fn update_session(&self, session: BenchmarkSession) {
        let mut sessions = self.sessions.write().unwrap();
        let session_id = session.id.clone();
        if let Some(existing) = sessions.get_mut(&session_id) {
            *existing = session;
            debug!("Updated session: {}", session_id);
        } else {
            warn!("Cannot update non-existent session: {}", session_id);
        }
    }

    /// Get the number of active sessions.
    pub fn get_active_session_count(&self) -> usize {
        let sessions = self.sessions.read().unwrap();
        sessions
            .values()
            .filter(|s| s.status.is_active())
            .count()
    }

    /// Get all active sessions.
    pub fn get_active_sessions(&self) -> Vec<BenchmarkSession> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .values()
            .filter(|s| s.status.is_active())
            .cloned()
            .collect()
    }

    /// Force to complete all active sessions (for shutdown).
    pub fn shutdown(&self) {
        let mut sessions = self.sessions.write().unwrap();
        info!(
            "Shutting down session manager, completing {} active sessions",
            sessions.len()
        );
        for session in sessions.values_mut() {
            if session.status.is_active() {
                session.force_complete();
            }
        }
        sessions.clear();
    }

}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_manager() -> SessionManager {
        SessionManager::new()
    }

    #[test]
    fn cancel_session_cancels_associated_cancellation_token() {
        let manager = create_test_manager();
        let session = manager.create_session(
            "reference".to_string(),
            vec!["java".to_string()],
            "default".to_string(),
            None,
            None,
            false,
            300_000,
        );
        let session_id = session.id.clone();

        let token = manager
            .get_cancellation_token(&session_id)
            .expect("token should be registered at create_session");
        assert!(!token.is_cancelled());

        // Set to RUNNING first (cancel only works on RUNNING sessions)
        let mut sessions = manager.sessions.write().unwrap();
        if let Some(s) = sessions.get_mut(&session_id) {
            s.status = RunStatus::RUNNING;
        }
        drop(sessions);

        assert!(manager.cancel_session(&session_id));
        assert!(token.is_cancelled(), "cancelling a session must fire its token");
    }

    #[test]
    fn get_cancellation_token_returns_none_for_unknown_session() {
        let manager = create_test_manager();
        assert!(manager.get_cancellation_token("does-not-exist").is_none());
    }

    #[test]
    fn test_create_session() {
        let manager = create_test_manager();
        let session = manager.create_session(
            "reference".to_string(),
            vec!["java".to_string()],
            "default".to_string(),
            None,
            None,
            false,
            300_000,
        );

        assert!(!session.id.is_empty());
        assert_eq!(session.agent_name, "reference");
        assert_eq!(session.status, RunStatus::PENDING);
    }

    #[test]
    fn test_create_session_with_model() {
        let manager = create_test_manager();
        let session = manager.create_session(
            "claude".to_string(),
            vec!["java".to_string()],
            "sonnet".to_string(),
            None,
            None,
            false,
            300_000,
        );

        assert_eq!(session.agent_name, "claude");
        assert_eq!(session.model, "sonnet".to_string());
    }

    #[test]
    fn test_create_session_with_exercise() {
        let manager = create_test_manager();
        let session = manager.create_session(
            "reference".to_string(),
            vec!["java".to_string()],
            "default".to_string(),
            None,
            Some("two-fer".to_string()),
            false,
            300_000,
        );

        assert_eq!(session.exercise_name, Some("two-fer".to_string()));
    }

    #[test]
    fn test_get_non_existent_session() {
        let manager = create_test_manager();
        let session = manager.get_session("non-existent-id");
        assert!(session.is_none());
    }

    #[test]
    fn test_cancel_session() {
        let manager = create_test_manager();
        let session = manager.create_session(
            "reference".to_string(),
            vec!["java".to_string()],
            "default".to_string(),
            None,
            None,
            false,
            300_000,
        );
        let session_id = session.id.clone();

        // Set to RUNNING first (cancel only works on RUNNING sessions)
        let mut sessions = manager.sessions.write().unwrap();
        if let Some(s) = sessions.get_mut(&session_id) {
            s.status = RunStatus::RUNNING;
        }
        drop(sessions);

        let result = manager.cancel_session(&session_id);
        assert!(result);

        let session = manager.get_session(&session_id).unwrap();
        assert_eq!(session.status, RunStatus::CANCELLED);
    }

    #[test]
    fn test_cancel_non_existent_session() {
        let manager = create_test_manager();
        let result = manager.cancel_session("non-existent");
        assert!(!result);
    }

    #[test]
    fn test_get_all_sessions() {
        let manager = create_test_manager();
        manager.create_session(
            "reference".to_string(),
            vec!["java".to_string()],
            "default".to_string(),
            None,
            None,
            false,
            300_000,
        );
        manager.create_session(
            "claude".to_string(),
            vec!["python".to_string()],
            "sonnet".to_string(),
            None,
            None,
            false,
            300_000,
        );

        let sessions = manager.get_all_sessions();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_get_active_sessions() {
        let manager = create_test_manager();
        let session1 = manager.create_session(
            "reference".to_string(),
            vec!["java".to_string()],
            "default".to_string(),
            None,
            None,
            false,
            300_000,
        );
        let session2 = manager.create_session(
            "claude".to_string(),
            vec!["python".to_string()],
            "sonnet".to_string(),
            None,
            None,
            false,
            300_000,
        );

        // Set session1 to RUNNING so it can be cancelled
        let mut sessions = manager.sessions.write().unwrap();
        if let Some(s) = sessions.get_mut(&session1.id) {
            s.status = RunStatus::RUNNING;
        }
        drop(sessions);
        manager.cancel_session(&session1.id);

        let active = manager.get_active_sessions();
        // session1 is CANCELLED, session2 is PENDING (active)
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, session2.id);
    }

    #[test]
    fn test_session_has_unique_ids() {
        let manager = create_test_manager();
        let session1 = manager.create_session(
            "reference".to_string(),
            vec!["java".to_string()],
            "default".to_string(),
            None,
            None,
            false,
            300_000,
        );
        let session2 = manager.create_session(
            "reference".to_string(),
            vec!["java".to_string()],
            "default".to_string(),
            None,
            None,
            false,
            300_000,
        );

        assert_ne!(session1.id, session2.id);
    }

    #[test]
    fn test_session_created_with_correct_status() {
        let manager = create_test_manager();
        let session = manager.create_session(
            "reference".to_string(),
            vec!["java".to_string()],
            "default".to_string(),
            None,
            None,
            false,
            300_000,
        );

        assert_eq!(session.status, RunStatus::PENDING);
    }

    #[test]
    fn test_shutdown() {
        let manager = create_test_manager();
        let session1 = manager.create_session(
            "reference".to_string(),
            vec!["java".to_string()],
            "default".to_string(),
            None,
            None,
            false,
            300_000,
        );

        // Set to RUNNING so shutdown affects it
        let mut sessions = manager.sessions.write().unwrap();
        if let Some(s) = sessions.get_mut(&session1.id) {
            s.status = RunStatus::RUNNING;
        }
        drop(sessions);

        manager.shutdown();

        // All sessions should be cleared
        assert!(manager.get_all_sessions().is_empty());
    }

    #[test]
    fn test_get_active_session_count() {
        let manager = create_test_manager();
        manager.create_session(
            "reference".to_string(),
            vec!["java".to_string()],
            "default".to_string(),
            None,
            None,
            false,
            300_000,
        );
        manager.create_session(
            "claude".to_string(),
            vec!["python".to_string()],
            "default".to_string(),
            None,
            None,
            false,
            300_000,
        );

        // Both are PENDING (active)
        assert_eq!(manager.get_active_session_count(), 2);
    }

    #[test]
    fn test_sessions_are_clonable() {
        let manager = SessionManager::new();
        let _clone = manager.clone(); // Should compile - SessionManager derives Clone
    }
}
