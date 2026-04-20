package com.benchmark.web.domain;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Manages the queue of benchmark items to be executed.
 * Supports sequential execution with a single worker pattern.
 */
public class BenchmarkQueue {
    private final ConcurrentLinkedQueue<BenchmarkQueueItem> pendingQueue = new ConcurrentLinkedQueue<>();
    private final List<BenchmarkQueueItem> completedItems = Collections.synchronizedList(new ArrayList<>());
    private final AtomicReference<BenchmarkQueueItem> currentItem = new AtomicReference<>();

    /**
     * Add an item to the queue.
     */
    public void add(BenchmarkQueueItem item) {
        pendingQueue.offer(item);
    }

    /**
     * Add multiple items to the queue.
     */
    public void addAll(List<BenchmarkQueueItem> items) {
        for (BenchmarkQueueItem item : items) {
            pendingQueue.offer(item);
        }
    }

    /**
     * Get the next pending item and mark it as running.
     * Returns null if no items are pending.
     */
    public BenchmarkQueueItem pollNext() {
        BenchmarkQueueItem item = pendingQueue.poll();
        if (item != null) {
            item.setStatus(BenchmarkQueueItem.QueueItemStatus.RUNNING);
            currentItem.set(item);
        }
        return item;
    }

    /**
     * Mark the current item as completed.
     */
    public void completeCurrent() {
        BenchmarkQueueItem item = currentItem.getAndSet(null);
        if (item != null) {
            item.setStatus(BenchmarkQueueItem.QueueItemStatus.COMPLETED);
            completedItems.add(item);
        }
    }

    /**
     * Mark the current item as failed.
     */
    public void failCurrent() {
        BenchmarkQueueItem item = currentItem.getAndSet(null);
        if (item != null) {
            item.setStatus(BenchmarkQueueItem.QueueItemStatus.FAILED);
            completedItems.add(item);
        }
    }

    /**
     * Get the currently running item.
     */
    public BenchmarkQueueItem getCurrentItem() {
        return currentItem.get();
    }

    /**
     * Get all pending items.
     */
    public List<BenchmarkQueueItem> getPendingItems() {
        return new ArrayList<>(pendingQueue);
    }

    /**
     * Get all completed items.
     */
    public List<BenchmarkQueueItem> getCompletedItems() {
        return new ArrayList<>(completedItems);
    }

    /**
     * Get all items (pending, running, completed).
     */
    public List<BenchmarkQueueItem> getAllItems() {
        List<BenchmarkQueueItem> all = new ArrayList<>();
        all.addAll(pendingQueue);
        if (currentItem.get() != null) {
            all.add(currentItem.get());
        }
        all.addAll(completedItems);
        return all;
    }

    /**
     * Check if the queue is empty (no pending items).
     */
    public boolean isEmpty() {
        return pendingQueue.isEmpty();
    }

    /**
     * Get the size of the pending queue.
     */
    public int getPendingCount() {
        return pendingQueue.size();
    }

    /**
     * Clear all pending items.
     */
    public void clearPending() {
        pendingQueue.clear();
    }

    /**
     * Cancel a specific item by ID.
     */
    public boolean cancelItem(String itemId) {
        // Check pending queue — use removeIf to avoid ConcurrentModificationException
        // that would occur if we iterated and removed simultaneously.
        final String targetId = itemId;
        boolean foundInPending = pendingQueue.removeIf(item -> {
            if (item.getId().equals(targetId)) {
                item.setStatus(BenchmarkQueueItem.QueueItemStatus.CANCELLED);
                completedItems.add(item);
                return true;
            }
            return false;
        });

        if (foundInPending) {
            return true;
        }

        // Check current running item
        BenchmarkQueueItem current = currentItem.get();
        if (current != null && current.getId().equals(itemId)) {
            current.setStatus(BenchmarkQueueItem.QueueItemStatus.CANCELLED);
            currentItem.set(null);
            completedItems.add(current);
            return true;
        }
        return false;
    }

    /**
     * Remove a specific item by ID (only if pending).
     */
    public boolean removeItem(String itemId) {
        final String targetId = itemId;
        return pendingQueue.removeIf(item -> item.getId().equals(targetId));
    }

    /**
     * Clear all completed items.
     */
    public void clearCompleted() {
        completedItems.clear();
    }

    /**
     * Get total count of all items.
     */
    public int getTotalCount() {
        return pendingQueue.size() + (currentItem.get() != null ? 1 : 0) + completedItems.size();
    }
}
