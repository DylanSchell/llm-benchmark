package com.benchmark.docker;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import java.util.regex.Pattern;

/**
 * Watches for Bash tool calls that exceed a configured timeout and kills them
 * inside the Docker container using docker exec.
 *
 * <p>Usage: create an instance with the container name and timeout, then call
 * {@link #onToolCallStarted(String)} when a Bash tool call begins and
 * {@link #onToolCallFinished(String)} when it completes. If the timeout expires
 * before the call finishes, the watchdog terminates the matching process inside
 * the container.</p>
 */
public class CommandWatchdog implements AutoCloseable {
    private static final Logger logger = LoggerFactory.getLogger(CommandWatchdog.class);

    private final String containerName;
    private final int timeoutSeconds;
    private final ScheduledExecutorService scheduler = Executors.newSingleThreadScheduledExecutor(r -> {
        Thread t = new Thread(r, "command-watchdog");
        t.setDaemon(true);
        return t;
    });
    private final ConcurrentHashMap<String, ScheduledFuture<?>> timers = new ConcurrentHashMap<>();
    private final Pattern killPattern;

    /**
     * Creates a new CommandWatchdog.
     *
     * @param containerName  the Docker container to exec into for killing processes
     * @param timeoutSeconds maximum seconds allowed for any single Bash tool call
     */
    public CommandWatchdog(String containerName, int timeoutSeconds) {
        this.containerName = containerName;
        this.timeoutSeconds = timeoutSeconds;
        // Build a kill pattern from the container name so we only kill processes
        // that were started inside this container's workspace.
        this.killPattern = Pattern.compile(Pattern.quote("/workspace"));
    }

    /**
     * Called when a Bash tool call starts. Starts a watchdog timer for the
     * given command. The command string is used to build a regex pattern for
     * killing the process if the timeout expires.
     *
     * @param command the full bash command that was issued
     */
    public void onToolCallStarted(String command) {
        // Sanitize the command for use in a regex pattern: escape regex special
        // chars but keep it flexible enough to match the process line.
        String escaped = Pattern.quote(command.split("\n")[0].trim().substring(0,
                Math.min(command.split("\n")[0].trim().length(), 128)));

        ScheduledFuture<?> future = scheduler.schedule(() -> {
            logger.warn("Command watchdog timeout: killing process matching '{}' in container '{}'",
                    command.substring(0, Math.min(command.length(), 120)), containerName);
            killProcessByCommand(command);
        }, timeoutSeconds, TimeUnit.SECONDS);

        timers.put(command, future);
    }

    /**
     * Called when a Bash tool call finishes. Cancels the watchdog timer for
     * the given command if it hasn't fired yet.
     *
     * @param command the command that just finished
     */
    public void onToolCallFinished(String command) {
        ScheduledFuture<?> future = timers.remove(command);
        if (future != null && !future.isDone()) {
            future.cancel(false);
        }
    }

    /**
     * Cancels the oldest pending watchdog timer (FIFO order). Used by
     * StreamParser when a tool result arrives but we can't match it to a
     * specific command.
     */
    public void cancelOldestTimer() {
        if (timers.isEmpty()) {
            return;
        }
        // Get the first entry (insertion order in ConcurrentHashMap doesn't
        // guarantee FIFO, but we use it as a best-effort approach).
        // For a more robust solution, we could use a LinkedBlockingQueue.
        java.util.Map.Entry<String, ScheduledFuture<?>> first = timers.entrySet().iterator().next();
        ScheduledFuture<?> future = timers.remove(first.getKey());
        if (future != null && !future.isDone()) {
            future.cancel(false);
            logger.debug("Cancelled oldest pending watchdog timer");
        }
    }

    /**
     * Kills processes inside the container whose command line matches the
     * given command string. Uses a two-phase approach: SIGTERM first, then
     * SIGKILL after a brief grace period.
     */
    private void killProcessByCommand(String command) {
        String line = command.split("\n")[0].trim();
        // Use pkill with a pattern that matches the command inside /workspace.
        // Escape special regex chars in the command for the -f flag.
        String safeCommand = line.replaceAll("['\"\\\\]", "\\\\$0");

        // Phase 1: SIGTERM
        try {
            ProcessBuilder pb = new ProcessBuilder(
                    "docker", "exec", containerName,
                    "pkill", "-TERM", "-f", safeCommand);
            pb.redirectErrorStream(true);
            Process p = pb.start();
            if (!p.waitFor(5, TimeUnit.SECONDS)) {
                p.destroyForcibly();
            }
        } catch (IOException | InterruptedException e) {
            logger.warn("Failed to send SIGTERM to process matching '{}': {}",
                    line, e.getMessage());
        }

        // Phase 2: SIGKILL after brief grace
        try {
            Thread.sleep(2000);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }

        try {
            ProcessBuilder pb = new ProcessBuilder(
                    "docker", "exec", containerName,
                    "pkill", "-9", "-f", safeCommand);
            pb.redirectErrorStream(true);
            Process p = pb.start();
            if (!p.waitFor(5, TimeUnit.SECONDS)) {
                p.destroyForcibly();
            }
            logger.info("Killed process matching '{}' in container '{}' via SIGKILL",
                    line, containerName);
        } catch (IOException | InterruptedException e) {
            logger.warn("Failed to SIGKILL process matching '{}': {}",
                    line, e.getMessage());
        }
    }

    @Override
    public void close() {
        scheduler.shutdownNow();
        timers.values().forEach(f -> f.cancel(false));
        timers.clear();
    }
}
