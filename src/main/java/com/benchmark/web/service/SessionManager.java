package com.benchmark.web.service;

import com.benchmark.config.Config;
import com.benchmark.web.domain.BenchmarkSession;
import com.benchmark.web.domain.RunStatus;
import jakarta.annotation.PreDestroy;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;

import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Manages benchmark session lifecycle.
 * Handles session creation, retrieval, cancellation, and removal.
 */
@Service
public class SessionManager {
    private static final Logger logger = LoggerFactory.getLogger(SessionManager.class);

    private final Config config;
    private final Map<String, BenchmarkSession> sessions = new ConcurrentHashMap<>();

    /**
     * Creates a new benchmark session.
     *
     * @param agentName   The agent to use
     * @param languages   The programming languages
     * @param model       The model to use (optional)
     * @param exerciseName The exercise name, or null for all exercises
     * @return The created session with its ID
     */
    public SessionManager(Config config) {
        this.config = config;
    }

    public BenchmarkSession createSession(String agentName, String[] languages, String model, String exerciseName) {
        String sessionId = UUID.randomUUID().toString();
        long timeoutMs = config.getDocker().getTimeout() * 1000L;
        BenchmarkSession session = new BenchmarkSession(sessionId, agentName, languages, model, exerciseName, timeoutMs);
        sessions.put(sessionId, session);

        logger.info("Created benchmark session: {} for {}/{} (model: {})", 
                sessionId, String.join(",", languages),
                exerciseName != null ? exerciseName : "all", model);

        return session;
    }

    /**
     * Gets a session by ID.
     *
     * @param sessionId The session ID
     * @return The session, or null if not found
     */
    public BenchmarkSession getSession(String sessionId) {
        return sessions.get(sessionId);
    }

    /**
     * Gets all sessions.
     *
     * @return A copy of all sessions
     */
    public Map<String, BenchmarkSession> getAllSessions() {
        return new ConcurrentHashMap<>(sessions);
    }

    /**
     * Cancels a running session.
     *
     * @param sessionId The session ID
     * @return true if cancelled successfully, false otherwise
     */
    public boolean cancelSession(String sessionId) {
        BenchmarkSession session = sessions.get(sessionId);
        if (session != null && session.getStatus() == RunStatus.RUNNING) {
            session.setStatus(RunStatus.CANCELLED);
            session.emitOutput("Cancelled by user");
            session.completeOutput();
            logger.info("Cancelled session: {}", sessionId);
            return true;
        }
        return false;
    }

    /**
     * Removes a completed session.
     *
     * @param sessionId The session ID
     */
    public void removeSession(String sessionId) {
        sessions.remove(sessionId);
        logger.debug("Removed session: {}", sessionId);
    }

    /**
     * Gets the number of active sessions.
     *
     * @return Number of sessions with PENDING or RUNNING status
     */
    public int getActiveSessionCount() {
        return (int) sessions.values().stream()
                .filter(s -> s.getStatus() == RunStatus.RUNNING || s.getStatus() == RunStatus.PENDING)
                .count();
    }

    /**
     * Gets all active sessions.
     *
     * @return List of sessions with PENDING or RUNNING status
     */
    public java.util.List<BenchmarkSession> getActiveSessions() {
        return sessions.values().stream()
                .filter(s -> s.getStatus() == RunStatus.RUNNING || s.getStatus() == RunStatus.PENDING)
                .toList();
    }

    /**
     * Clears all completed sessions.
     */
    public void clearCompletedSessions() {
        sessions.entrySet().removeIf(entry -> {
            var status = entry.getValue().getStatus();
            return status == RunStatus.COMPLETED || 
                   status == RunStatus.FAILED || 
                   status == RunStatus.CANCELLED;
        });
        logger.info("Cleared completed sessions, {} remaining", sessions.size());
    }

    /**
     * Force complete all active sessions during shutdown.
     */
    @PreDestroy
    public void shutdown() {
        logger.info("Shutting down session manager, completing {} active sessions", sessions.size());
        for (BenchmarkSession session : sessions.values()) {
            if (session.getStatus() == RunStatus.RUNNING || session.getStatus() == RunStatus.PENDING) {
                session.setStatus(RunStatus.CANCELLED);
                session.forceComplete();
            }
        }
        sessions.clear();
    }
}
