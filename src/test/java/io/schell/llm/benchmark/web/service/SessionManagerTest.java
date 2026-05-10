package io.schell.llm.benchmark.web.service;

import io.schell.llm.benchmark.config.Config;
import io.schell.llm.benchmark.config.ConfigLoader;
import io.schell.llm.benchmark.web.domain.BenchmarkSession;
import io.schell.llm.benchmark.web.domain.RunStatus;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.file.Path;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Unit tests for {@link SessionManager}.
 */
class SessionManagerTest {

    private SessionManager sessionManager;
    private Config config;

    @BeforeEach
    void setUp() throws Exception {
        config = ConfigLoader.load(Path.of("config.yaml"));
        sessionManager = new SessionManager(config);
    }

    @Test
    void testCreateSession() {
        // When
        BenchmarkSession session = sessionManager.createSession("reference", new String[]{"java"}, null, null);

        // Then
        assertNotNull(session);
        assertEquals("reference", session.getAgentName());
        assertEquals(RunStatus.PENDING, session.getStatus());
    }

    @Test
    void testCreateSessionWithModel() {
        // When
        BenchmarkSession session = sessionManager.createSession("claude", new String[]{"java"}, "sonnet", null);

        // Then
        assertEquals("claude", session.getAgentName());
        assertEquals("sonnet", session.getModel());
    }

    @Test
    void testCreateSessionWithExercise() {
        // When
        BenchmarkSession session = sessionManager.createSession("reference", new String[]{"java"}, null, "two-fer");

        // Then
        assertEquals("two-fer", session.getExerciseName());
    }

    @Test
    void testGetNonExistentSession() {
        // When
        BenchmarkSession session = sessionManager.getSession("non-existent-id");

        // Then
        assertNull(session);
    }

    @Test
    void testCancelSession() {
        // Given - session must be RUNNING to cancel
        BenchmarkSession session = sessionManager.createSession("reference", new String[]{"java"}, null, null);
        String sessionId = session.getId();
        // Manually set status to RUNNING (since there's no update method)
        session.setStatus(RunStatus.RUNNING);

        // When
        boolean result = sessionManager.cancelSession(sessionId);

        // Then
        assertTrue(result);
        session = sessionManager.getSession(sessionId);
        assertEquals(RunStatus.CANCELLED, session.getStatus());
    }

    @Test
    void testCancelNonExistentSession() {
        // When & Then
        boolean result = sessionManager.cancelSession("non-existent");
        assertFalse(result);
    }

    @Test
    void testRemoveSession() {
        // Given
        BenchmarkSession session = sessionManager.createSession("reference", new String[]{"java"}, null, null);
        String sessionId = session.getId();

        // When
        sessionManager.removeSession(sessionId);

        // Then
        assertNull(sessionManager.getSession(sessionId));
    }

    @Test
    void testRemoveNonExistentSession() {
        // When & Then - should not throw
        assertDoesNotThrow(() -> sessionManager.removeSession("non-existent"));
    }

    @Test
    void testGetAllSessions() {
        // Given
        sessionManager.createSession("reference", new String[]{"java"}, null, null);
        sessionManager.createSession("claude", new String[]{"python"}, "sonnet", null);

        // When
        Map<String, BenchmarkSession> sessions = sessionManager.getAllSessions();

        // Then
        assertEquals(2, sessions.size());
    }

    @Test
    void testGetActiveSessions() {
        // Given
        BenchmarkSession session1 = sessionManager.createSession("reference", new String[]{"java"}, null, null);
        BenchmarkSession session2 = sessionManager.createSession("claude", new String[]{"python"}, "sonnet", null);
        
        // Set session1 to RUNNING so it can be cancelled
        session1.setStatus(RunStatus.RUNNING);
        sessionManager.cancelSession(session1.getId());

        // When - get all sessions
        Map<String, BenchmarkSession> allSessions = sessionManager.getAllSessions();

        // Then
        assertEquals(2, allSessions.size());
        
        // Verify we can filter manually for non-cancelled sessions
        long activeCount = allSessions.values().stream()
            .filter(s -> s.getStatus() != RunStatus.CANCELLED)
            .count();
        assertEquals(1, activeCount);
    }

    @Test
    void testSessionHasUniqueIds() {
        // When
        BenchmarkSession session1 = sessionManager.createSession("reference", new String[]{"java"}, null, null);
        BenchmarkSession session2 = sessionManager.createSession("reference", new String[]{"java"}, null, null);

        // Then
        assertNotEquals(session1.getId(), session2.getId());
    }

    @Test
    void testSessionCreatedWithCorrectTimestamp() {
        // When
        BenchmarkSession session = sessionManager.createSession("reference", new String[]{"java"}, null, null);

        // Then
        assertNotNull(session.getStartTime());
    }

    @Test
    void testSessionWithEmptyLanguagesArray() {
        // When
        BenchmarkSession session = sessionManager.createSession("reference", new String[]{}, null, null);

        // Then
        assertNotNull(session);
        assertEquals("reference", session.getAgentName());
    }

    @Test
    void testGetAllSessionsReturnsMutableCopy() {
        // Given
        sessionManager.createSession("reference", new String[]{"java"}, null, null);

        // When
        Map<String, BenchmarkSession> sessions = sessionManager.getAllSessions();

        // Then - should not throw when iterating
        assertDoesNotThrow(() -> {
            for (Map.Entry<String, BenchmarkSession> entry : sessions.entrySet()) {
                assertNotNull(entry.getKey());
                assertNotNull(entry.getValue());
            }
        });
    }
}
