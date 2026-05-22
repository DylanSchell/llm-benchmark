//! BenchmarkQueue - mirrors Java BenchmarkQueue.java
//! Thread-safe concurrent queue for benchmark items.

use crate::models::queue_item::{BenchmarkQueueItem, QueueItemStatus};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A concurrent queue for benchmark queue items.
#[derive(Debug)]
pub struct BenchmarkQueue {
    inner: Arc<Mutex<VecDeque<BenchmarkQueueItem>>>,
    all_items: Arc<Mutex<Vec<BenchmarkQueueItem>>>,
    current_item: Arc<Mutex<Option<BenchmarkQueueItem>>>,
}

impl Clone for BenchmarkQueue {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            all_items: Arc::clone(&self.all_items),
            current_item: Arc::clone(&self.current_item),
        }
    }
}

impl BenchmarkQueue {
    /// Create a new empty queue.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
            all_items: Arc::new(Mutex::new(Vec::new())),
            current_item: Arc::new(Mutex::new(None)),
        }
    }

    /// Add a single item to the queue.
    pub fn add(&self, item: BenchmarkQueueItem) {
        let mut queue = self.inner.lock().unwrap();
        let mut all = self.all_items.lock().unwrap();
        queue.push_back(item.clone());
        all.push(item);
    }

    /// Add multiple items to the queue.
    pub fn add_all(&self, items: Vec<BenchmarkQueueItem>) {
        let mut queue = self.inner.lock().unwrap();
        let mut all = self.all_items.lock().unwrap();
        for item in items {
            queue.push_back(item.clone());
            all.push(item);
        }
    }

    /// Poll the next item from the queue (removes it).
    pub fn poll_next(&self) -> Option<BenchmarkQueueItem> {
        let mut queue = self.inner.lock().unwrap();
        if let Some(item) = queue.pop_front() {
            let mut item = item;
            item.status = QueueItemStatus::RUNNING;
            // Update all_items so get_all_items() reflects RUNNING status
            let mut all = self.all_items.lock().unwrap();
            for existing in all.iter_mut() {
                if existing.id == item.id {
                    *existing = item.clone();
                    break;
                }
            }
            // Track the current item being processed (matches Java currentItem)
            let mut current = self.current_item.lock().unwrap();
            *current = Some(item.clone());
            Some(item)
        } else {
            None
        }
    }

    /// Complete the current (processing) item (matches Java completeCurrent).
    pub fn complete_current(&self) -> bool {
        let mut current = self.current_item.lock().unwrap();
        if let Some(item) = current.take() {
            let item_id = item.id.clone();
            // Find and update the item in all_items
            let mut all = self.all_items.lock().unwrap();
            for existing in all.iter_mut() {
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

    /// Fail the current (processing) item (matches Java failCurrent).
    pub fn fail_current(&self) -> bool {
        let mut current = self.current_item.lock().unwrap();
        if let Some(item) = current.take() {
            let item_id = item.id.clone();
            // Find and update the item in all_items
            let mut all = self.all_items.lock().unwrap();
            for existing in all.iter_mut() {
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
        // Also remove from inner queue if it's still there (not yet polled)
        let was_in_queue = {
            let queue = self.inner.lock().unwrap();
            queue.iter().any(|item| item.id == item_id)
        };

        let mut all = self.all_items.lock().unwrap();
        for item in all.iter_mut() {
            if item.id == item_id {
                item.cancel();
                // Also remove from inner queue if present
                if was_in_queue {
                    let mut queue = self.inner.lock().unwrap();
                    queue.retain(|item| item.id != item_id);
                }
                return true;
            }
        }
        false
    }

    /// Clear all pending items.
    pub fn clear_pending(&self) {
        let mut queue = self.inner.lock().unwrap();
        queue.retain(|item| item.status != QueueItemStatus::PENDING);
        // Also remove from all_items so get_pending_items() stays in sync
        let mut all = self.all_items.lock().unwrap();
        all.retain(|item| item.status != QueueItemStatus::PENDING);
    }

    /// Clear terminal items (COMPLETED, FAILED, CANCELLED).
    /// Only removes items that are in the inner queue AND have terminal status.
    /// Does NOT affect items already polled (RUNNING in current_item) or
    /// pending items still waiting in the queue.
    pub fn clear_terminal_items(&self) -> usize {
        let mut queue = self.inner.lock().unwrap();
        let mut all = self.all_items.lock().unwrap();
        // Only remove terminal items from the inner queue (not ALL items)
        queue.retain(|item| !item.status.is_terminal());
        let removed = queue.len();
        // Also remove terminal items from all_items
        all.retain(|item| !item.status.is_terminal());
        removed
    }

    /// Retry a failed item by re-adding it to the queue.
    pub fn retry_item(&self, item_id: &str) -> Option<BenchmarkQueueItem> {
        let mut all = self.all_items.lock().unwrap();
        for item in all.iter_mut() {
            if item.id == item_id && item.status == QueueItemStatus::FAILED {
                let new_item = item.retry();
                self.add(new_item.clone());
                return Some(new_item);
            }
        }
        None
    }

    /// Set the session ID on a queue item.
    pub fn set_session_id(&self, item_id: &str, session_id: String) {
        let mut all = self.all_items.lock().unwrap();
        for item in all.iter_mut() {
            if item.id == item_id {
                item.session_id = Some(session_id);
                break;
            }
        }
    }

    /// Get all items (pending, running, completed).
    pub fn get_all_items(&self) -> Vec<BenchmarkQueueItem> {
        let all = self.all_items.lock().unwrap();
        all.clone()
    }

    /// Get pending items only.
    pub fn get_pending_items(&self) -> Vec<BenchmarkQueueItem> {
        let all = self.all_items.lock().unwrap();
        all.iter()
            .filter(|item| item.status == QueueItemStatus::PENDING)
            .cloned()
            .collect()
    }

}

impl Default for BenchmarkQueue {
    fn default() -> Self {
        Self::new()
    }
}
