package io.schell.llm.benchmark.docker;

/**
 * Simple smoke tests for CommandWatchdog.
 * Run with: java -cp target/classes io.schell.llm.benchmark.docker.CommandWatchdogTest
 */
public class CommandWatchdogTest {

    public static void main(String[] args) throws Exception {
        System.out.println("=== CommandWatchdog Tests ===");

        testBasicLifecycle();
        testMultipleTimers();
        testCancelOldestEmpty();
        testCancelOldestWithPending();
        testTimerFiresAfterTimeout();
        testTimerCancelledBeforeFiring();

        System.out.println("\nAll tests passed!");
    }

    static void testBasicLifecycle() {
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            w.onToolCallStarted("echo hello");
            w.onToolCallFinished("echo hello");
            System.out.println("  testBasicLifecycle: OK");
        }
    }

    static void testMultipleTimers() {
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            w.onToolCallStarted("cmd1");
            w.onToolCallStarted("cmd2");
            w.onToolCallStarted("cmd3");
            w.cancelOldestTimer();
            w.cancelOldestTimer();
            System.out.println("  testMultipleTimers: OK");
        }
    }

    static void testCancelOldestEmpty() {
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            w.cancelOldestTimer(); // should not throw
            System.out.println("  testCancelOldestEmpty: OK");
        }
    }

    static void testCancelOldestWithPending() {
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            w.onToolCallStarted("echo hello");
            w.cancelOldestTimer();
            System.out.println("  testCancelOldestWithPending: OK");
        }
    }

    static void testTimerFiresAfterTimeout() throws Exception {
        try (CommandWatchdog w = new CommandWatchdog("test", 0)) {
            w.onToolCallStarted("sleep 999");
            Thread.sleep(100); // let the 0s timeout fire
            System.out.println("  testTimerFiresAfterTimeout: OK");
        }
    }

    static void testTimerCancelledBeforeFiring() throws Exception {
        try (CommandWatchdog w = new CommandWatchdog("test", 0)) {
            w.onToolCallStarted("sleep 999");
            w.cancelOldestTimer(); // cancel before it fires
            Thread.sleep(100);
            System.out.println("  testTimerCancelledBeforeFiring: OK");
        }
    }
}
