package io.schell.llm.benchmark.util;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.Callable;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;

/**
 * Utility class for parallel execution of tasks with proper resource management.
 */
public class ParallelExecutor {
    private static final Logger logger = LoggerFactory.getLogger(ParallelExecutor.class);

    /**
     * Executes a list of tasks in parallel with the specified parallelism level.
     * Returns results in the same order as input tasks, skipping failed tasks.
     *
     * @param tasks        List of callable tasks
     * @param parallelism  Number of concurrent threads
     * @param <T>          Result type
     * @return List of successful results
     */
    public static <T> List<T> executeParallel(List<Callable<T>> tasks, int parallelism) {
        if (tasks == null || tasks.isEmpty()) {
            return new ArrayList<>();
        }

        ExecutorService executor = Executors.newFixedThreadPool(parallelism);
        try {
            List<Future<T>> futures = executor.invokeAll(tasks);
            
            List<T> results = new ArrayList<>();
            for (Future<T> future : futures) {
                try {
                    T result = future.get();
                    if (result != null) {
                        results.add(result);
                    }
                } catch (Exception e) {
                    logger.error("Task execution failed: {}", e.getMessage());
                }
            }
            return results;
        } catch (InterruptedException e) {
            logger.error("Parallel execution interrupted: {}", e.getMessage());
            executor.shutdownNow();
            Thread.currentThread().interrupt();
            return new ArrayList<>();
        } finally {
            shutdownExecutor(executor);
        }
    }

    /**
     * Executes tasks and collects results, allowing null values in the result list.
     *
     * @param tasks        List of callable tasks
     * @param parallelism  Number of concurrent threads
     * @param <T>          Result type
     * @return List of all results (including nulls for failed tasks)
     */
    public static <T> List<T> executeParallelAllowNulls(List<Callable<T>> tasks, int parallelism) {
        if (tasks == null || tasks.isEmpty()) {
            return new ArrayList<>();
        }

        ExecutorService executor = Executors.newFixedThreadPool(parallelism);
        try {
            List<Future<T>> futures = executor.invokeAll(tasks);
            
            List<T> results = new ArrayList<>();
            for (Future<T> future : futures) {
                try {
                    results.add(future.get());
                } catch (Exception e) {
                    logger.error("Task execution failed: {}", e.getMessage());
                    results.add(null);
                }
            }
            return results;
        } catch (InterruptedException e) {
            logger.error("Parallel execution interrupted: {}", e.getMessage());
            executor.shutdownNow();
            Thread.currentThread().interrupt();
            return new ArrayList<>();
        } finally {
            shutdownExecutor(executor);
        }
    }

    /**
     * Gracefully shuts down an executor service.
     */
    private static void shutdownExecutor(ExecutorService executor) {
        if (executor == null || executor.isShutdown()) {
            return;
        }

        try {
            executor.shutdown();
            if (!executor.awaitTermination(5, TimeUnit.SECONDS)) {
                logger.warn("Executor did not terminate in time, forcing shutdown");
                executor.shutdownNow();
            }
        } catch (InterruptedException e) {
            executor.shutdownNow();
            Thread.currentThread().interrupt();
        }
    }
}
