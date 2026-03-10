//! Per-bot update queue for long-polling support.
//!
//! Each bot has an independent polling state. When the app pushes an update,
//! any waiting `getUpdates` poller is woken. Only one poller per bot is
//! allowed at a time (matching Telegram behavior).

use crate::telegram_server::error::BotApiError;
use futures::channel::mpsc;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// Manages per-bot long-polling state.
pub struct UpdateQueueManager {
    bots: Mutex<HashMap<i64, BotPollState>>,
}

struct BotPollState {
    notifier: Option<mpsc::Sender<()>>,
    has_active_poller: bool,
    last_poll_time: Instant,
}

impl UpdateQueueManager {
    /// Creates a new queue manager with no registered bots.
    pub fn new() -> Self {
        Self {
            bots: Mutex::new(HashMap::new()),
        }
    }

    /// Wakes the active poller for a bot so it re-checks for new updates.
    /// No-op if no poller is currently waiting.
    pub fn notify_bot(&self, bot_id: i64) {
        let mut bots = self.bots.lock().expect("queue lock poisoned");
        if let Some(state) = bots.get_mut(&bot_id) {
            if let Some(ref mut tx) = state.notifier {
                let _ = tx.try_send(());
            }
        }
    }

    /// Registers a long-poller for a bot. Returns a receiver that is
    /// signalled when new updates arrive. Fails with [`BotApiError::ConflictPoller`]
    /// if another poller is already active.
    pub fn register_poller(
        &self,
        bot_id: i64,
    ) -> Result<mpsc::Receiver<()>, BotApiError> {
        let mut bots = self.bots.lock().expect("queue lock poisoned");
        let state = bots.entry(bot_id).or_insert_with(|| BotPollState {
            notifier: None,
            has_active_poller: false,
            last_poll_time: Instant::now(),
        });

        if state.has_active_poller {
            return Err(BotApiError::ConflictPoller);
        }

        let (tx, rx) = mpsc::channel(1);
        state.notifier = Some(tx);
        state.has_active_poller = true;
        state.last_poll_time = Instant::now();
        Ok(rx)
    }

    /// Releases the poller slot for a bot (called when `getUpdates` finishes).
    pub fn unregister_poller(&self, bot_id: i64) {
        let mut bots = self.bots.lock().expect("queue lock poisoned");
        if let Some(state) = bots.get_mut(&bot_id) {
            state.notifier = None;
            state.has_active_poller = false;
        }
    }

    /// Returns whether a bot has polled within the given timeout duration.
    pub fn is_bot_online(
        &self,
        bot_id: i64,
        timeout: std::time::Duration,
    ) -> bool {
        let bots = self.bots.lock().expect("queue lock poisoned");
        bots.get(&bot_id)
            .map(|s| s.last_poll_time.elapsed() < timeout)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_unregister_poller() {
        let mgr = UpdateQueueManager::new();
        let _rx = mgr.register_poller(1).unwrap();
        assert!(mgr.register_poller(1).is_err());
        mgr.unregister_poller(1);
        let _rx2 = mgr.register_poller(1).unwrap();
    }

    #[test]
    fn test_notify_without_poller_is_noop() {
        let mgr = UpdateQueueManager::new();
        mgr.notify_bot(999); // should not panic
    }

    #[test]
    fn test_is_bot_online() {
        let mgr = UpdateQueueManager::new();
        assert!(!mgr.is_bot_online(1, std::time::Duration::from_secs(120)));
        let _rx = mgr.register_poller(1).unwrap();
        assert!(mgr.is_bot_online(1, std::time::Duration::from_secs(120)));
    }
}
