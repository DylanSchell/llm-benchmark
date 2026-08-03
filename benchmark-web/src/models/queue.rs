//! BenchmarkQueue - mirrors Java BenchmarkQueue.java
//! Thread-safe concurrent queue for benchmark items.

use crate::models::queue_item::{BenchmarkQueueItem, QueueItemStatus};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
use benchmark_types::util::recover_poisoned;


/// Internal state, protected by a single Mutex to ensure atomic transitions.
#[derive(Debug)]
struct InnerQueue {
    inner: VecDeque<BenchmarkQueueItem>,
    all_items: Vec<BenchmarkQueueItem>,
    current_items: HashMap<String, BenchmarkQueueItem>,
}

/// A concurrent queue for benchmark queue items.
/// All state is guarded by a single Mutex so that add/poll/complete/fail are
/// atomic and readers always observe a consistent view.
#[derive(Debug)]
pub struct BenchmarkQueue {
    data: Arc<Mutex<InnerQueue>>,
    /// Notified whenever items are added to the queue.
    notifier: Arc<Notify>,
}

impl Clone for BenchmarkQueue {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
            notifier: Arc::clone(&self.notifier),
        }
    }
}

impl BenchmarkQueue {
    /// Create a new empty queue.
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(InnerQueue {
                inner: VecDeque::new(),
                all_items: Vec::new(),
                current_items: HashMap::new(),
            })),
            notifier: Arc::new(Notify::new()),
        }
    }

    /// Add a single item to the queue.
    pub fn add(&self, item: BenchmarkQueueItem) {
        let mut data = recover_poisoned(self.data.lock());
        data.inner.push_back(item.clone());
        data.all_items.push(item);
        drop(data);
        // Wake the queue worker so it picks up the new item
        self.notifier.notify_waiters();
    }

    /// Add multiple items to the queue.
    pub fn add_all(&self, items: Vec<BenchmarkQueueItem>) {
        let mut data = recover_poisoned(self.data.lock());
        for item in items {
            data.inner.push_back(item.clone());
            data.all_items.push(item);
        }
        // Wake the queue worker — items are available
        self.notifier.notify_waiters();
    }

    /// Poll the next item from the queue (removes it).
    pub fn poll_next(&self) -> Option<BenchmarkQueueItem> {
        let mut data = recover_poisoned(self.data.lock());
        if let Some(mut item) = data.inner.pop_front() {
            item.status = QueueItemStatus::RUNNING;
            // Update all_items so get_all_items() reflects RUNNING status
            for existing in data.all_items.iter_mut() {
                if existing.id == item.id {
                    *existing = item.clone();
                    break;
                }
            }
            // Track the item by ID so multiple parallel workers can coexist
            data.current_items.insert(item.id.clone(), item.clone());
            Some(item)
        } else {
            None
        }
    }

    /// Complete a specific item by ID (matches Java completeCurrent).
    pub fn complete_current(&self, item_id: &str) -> bool {
        let mut data = recover_poisoned(self.data.lock());
        if data.current_items.remove(item_id).is_some() {
            for existing in data.all_items.iter_mut() {
                if existing.id == item_id {
                    existing.status = QueueItemStatus::COMPLETED;
                    existing.finished_at = Some(chrono::Utc::now());
                    break;
                }
            }
            true
        } else {
            false
        }
    }

    /// Fail a specific item by ID.
    pub fn fail_current(&self, item_id: &str) -> bool {
        let mut data = recover_poisoned(self.data.lock());
        if data.current_items.remove(item_id).is_some() {
            for existing in data.all_items.iter_mut() {
                if existing.id == item_id {
                    existing.status = QueueItemStatus::FAILED;
                    existing.finished_at = Some(chrono::Utc::now());
                    break;
                }
            }
            true
        } else {
            false
        }
    }

    /// Cancel a specific item by ID.
    pub fn cancel_item(&self, item_id: &str) -> bool {
        let mut data = recover_poisoned(self.data.lock());
        let was_in_queue = data.inner.iter().any(|item| item.id == item_id);
        for item in data.all_items.iter_mut() {
            if item.id == item_id {
                item.cancel();
                if was_in_queue {
                    data.inner.retain(|item| item.id != item_id);
                }
                // Also remove from current_items if it was running
                data.current_items.remove(item_id);
                return true;
            }
        }
        false
    }

    /// Clear all pending items.
    pub fn clear_pending(&self) {
        let mut data = recover_poisoned(self.data.lock());
        data.inner.retain(|item| item.status != QueueItemStatus::PENDING);
        data.all_items.retain(|item| item.status != QueueItemStatus::PENDING);
    }

    /// Clear terminal items (COMPLETED, FAILED, CANCELLED).
    /// Only removes items that are in the inner queue AND have terminal status.
    /// Does NOT affect items already polled (RUNNING in current_items) or
    /// pending items still waiting in the queue.
    pub fn clear_terminal_items(&self) -> usize {
        let mut data = recover_poisoned(self.data.lock());
        // Only remove COMPLETED and CANCELLED — keep FAILED items visible for retry.
        let removed = data.inner.iter().filter(|item| {
            matches!(item.status, QueueItemStatus::COMPLETED | QueueItemStatus::CANCELLED)
        }).count();
        data.inner.retain(|item| {
            !matches!(item.status, QueueItemStatus::COMPLETED | QueueItemStatus::CANCELLED)
        });
        data.all_items.retain(|item| {
            !matches!(item.status, QueueItemStatus::COMPLETED | QueueItemStatus::CANCELLED)
        });
        removed
    }

    /// Clear ALL items from the queue, including pending and running.
    /// Returns the number of items removed.
    pub fn clear_all(&self) -> usize {
        let mut data = recover_poisoned(self.data.lock());
        let removed = data.all_items.len();
        data.inner.clear();
        data.all_items.clear();
        data.current_items.clear();
        removed
    }

    /// Retry a failed item by re-adding it to the queue.
    /// Lock is released before calling self.add() to avoid deadlock
    /// (std::sync::Mutex is not reentrant).
    pub fn retry_item(&self, item_id: &str) -> Option<BenchmarkQueueItem> {
        let new_item = {
            let mut data = recover_poisoned(self.data.lock());
            let mut found = None;
            for item in data.all_items.iter_mut() {
                if item.id == item_id && item.status == QueueItemStatus::FAILED {
                    found = Some(item.retry());
                    break;
                }
            }
            found
            // MutexGuard dropped here — lock released before add()
        };

        if let Some(ref item) = new_item {
            self.add(item.clone());
        }
        new_item
    }

    /// Set the session ID on a queue item.
    pub fn set_session_id(&self, item_id: &str, session_id: String) {
        let mut data = recover_poisoned(self.data.lock());
        for item in data.all_items.iter_mut() {
            if item.id == item_id {
                item.session_id = Some(session_id);
                break;
            }
        }
    }

    /// Get the session ID currently linked to an item, if any.
    /// Used by cancellation so a running item's session (and its Docker
    /// container) can be aborted alongside the queue entry.
    pub fn session_id_for(&self, item_id: &str) -> Option<String> {
        let data = recover_poisoned(self.data.lock());
        data.all_items
            .iter()
            .find(|item| item.id == item_id)
            .and_then(|item| item.session_id.clone())
    }

    /// Get all items (pending, running, completed).
    pub fn get_all_items(&self) -> Vec<BenchmarkQueueItem> {
        let data = recover_poisoned(self.data.lock());
        data.all_items.clone()
    }

    /// Get pending items only.
    pub fn get_pending_items(&self) -> Vec<BenchmarkQueueItem> {
        let data = recover_poisoned(self.data.lock());
        data.all_items
            .iter()
            .filter(|item| item.status == QueueItemStatus::PENDING)
            .cloned()
            .collect()
    }
    /// Returns a future that resolves when the queue may have new items.
    /// Used by the queue worker to avoid busy-polling.
    pub async fn wait_for_item(&self) {
        self.notifier.notified().await;
    }

    /// Wake the queue worker — used when a worker slot frees up
    /// (not when items are added, which is handled by add_all).
    pub fn notify_capacity(&self) {
        self.notifier.notify_one();
    }
}

impl Default for BenchmarkQueue {
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

    /// Regression: a panic while holding the queue lock must not permanently
    /// break the queue. Previously every recover_poisoned(`.lock())` panicked on the
    /// poisoned lock, taking the whole app down with it.
    #[test]
    fn queue_recovers_from_poisoned_lock() {
        let queue = BenchmarkQueue::new();

        // Poison the internal mutex: panic while holding the lock.
        let data = std::sync::Arc::clone(&queue.data);
        let handle = std::thread::spawn(move || {
            let _guard = recover_poisoned(data.lock());
            panic!("boom");
        });
        assert!(handle.join().is_err());

        // Must not panic — with poison recovery every access still works.
        assert!(queue.get_all_items().is_empty());
        queue.add(crate::models::queue_item::BenchmarkQueueItem::new(
            "reference".to_string(),
            "default".to_string(),
            None,
            "java".to_string(),
            "two-fer".to_string(),
            false,
        ));
        assert_eq!(queue.get_all_items().len(), 1);
    }
}
