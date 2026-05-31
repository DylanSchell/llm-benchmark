//! BenchmarkQueue - mirrors Java BenchmarkQueue.java
//! Thread-safe concurrent queue for benchmark items.

use crate::models::queue_item::{BenchmarkQueueItem, QueueItemStatus};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

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
}

impl Clone for BenchmarkQueue {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
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
        }
    }

    /// Add a single item to the queue.
    pub fn add(&self, item: BenchmarkQueueItem) {
        let mut data = self.data.lock().unwrap();
        data.inner.push_back(item.clone());
        data.all_items.push(item);
    }

    /// Add multiple items to the queue.
    pub fn add_all(&self, items: Vec<BenchmarkQueueItem>) {
        let mut data = self.data.lock().unwrap();
        for item in items {
            data.inner.push_back(item.clone());
            data.all_items.push(item);
        }
    }

    /// Poll the next item from the queue (removes it).
    pub fn poll_next(&self) -> Option<BenchmarkQueueItem> {
        let mut data = self.data.lock().unwrap();
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
        let mut data = self.data.lock().unwrap();
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

    /// Complete the oldest current item (for backward compatibility).
    pub fn complete_current_oldest(&self) -> bool {
        let mut data = self.data.lock().unwrap();
        // Remove the item that was first inserted (FIFO among active items)
        // Since HashMap doesn't preserve order, we find by the item still in all_items
        // with RUNNING status that was added earliest (lowest index in all_items)
        let candidates: Vec<String> = data
            .all_items
            .iter()
            .filter(|i| i.status == QueueItemStatus::RUNNING && data.current_items.contains_key(&i.id))
            .map(|i| i.id.clone())
            .collect();
        if let Some(item_id) = candidates.into_iter().next() {
            if data.current_items.remove(&item_id).is_some() {
                for existing in data.all_items.iter_mut() {
                    if existing.id == item_id {
                        existing.status = QueueItemStatus::COMPLETED;
                        existing.finished_at = Some(chrono::Utc::now());
                        break;
                    }
                }
                return true;
            }
        }
        false
    }

    /// Fail a specific item by ID.
    pub fn fail_current(&self, item_id: &str) -> bool {
        let mut data = self.data.lock().unwrap();
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
        let mut data = self.data.lock().unwrap();
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
        let mut data = self.data.lock().unwrap();
        data.inner.retain(|item| item.status != QueueItemStatus::PENDING);
        data.all_items.retain(|item| item.status != QueueItemStatus::PENDING);
    }

    /// Clear terminal items (COMPLETED, FAILED, CANCELLED).
    /// Only removes items that are in the inner queue AND have terminal status.
    /// Does NOT affect items already polled (RUNNING in current_items) or
    /// pending items still waiting in the queue.
    pub fn clear_terminal_items(&self) -> usize {
        let mut data = self.data.lock().unwrap();
        // Count how many terminal items are in the inner queue before removal
        let removed = data.inner.iter().filter(|item| item.status.is_terminal()).count();
        data.inner.retain(|item| !item.status.is_terminal());
        data.all_items.retain(|item| !item.status.is_terminal());
        removed
    }

    /// Retry a failed item by re-adding it to the queue.
    pub fn retry_item(&self, item_id: &str) -> Option<BenchmarkQueueItem> {
        let mut data = self.data.lock().unwrap();
        for item in data.all_items.iter_mut() {
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
        let mut data = self.data.lock().unwrap();
        for item in data.all_items.iter_mut() {
            if item.id == item_id {
                item.session_id = Some(session_id);
                break;
            }
        }
    }

    /// Get all items (pending, running, completed).
    pub fn get_all_items(&self) -> Vec<BenchmarkQueueItem> {
        let data = self.data.lock().unwrap();
        data.all_items.clone()
    }

    /// Get pending items only.
    pub fn get_pending_items(&self) -> Vec<BenchmarkQueueItem> {
        let data = self.data.lock().unwrap();
        data.all_items
            .iter()
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
