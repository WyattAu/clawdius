//! Feed Manager
//!
//! Manages multiple market data feeds and distributes updates to subscribers.

use super::feeds::{FeedError, FeedStatus, MarketFeed, MarketUpdate, Symbol};
use super::wallet_guard::{Order, RiskDecision, Wallet, WalletGuard};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

pub type FeedId = String;

#[async_trait]
pub trait FeedSubscriber: Send + Sync {
    async fn on_update(&mut self, update: &MarketUpdate);
    async fn on_error(&mut self, error: &FeedError);
}

pub struct FeedManager {
    feeds: HashMap<FeedId, Box<dyn MarketFeed>>,
    subscribers: Vec<Box<dyn FeedSubscriber>>,
    broadcast_tx: Option<broadcast::Sender<MarketUpdate>>,
    wallet_guard: Option<Arc<RwLock<WalletGuard>>>,
}

impl FeedManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            feeds: HashMap::new(),
            subscribers: Vec::new(),
            broadcast_tx: None,
            wallet_guard: None,
        }
    }

    pub fn register_feed(&mut self, name: impl Into<FeedId>, feed: Box<dyn MarketFeed>) {
        self.feeds.insert(name.into(), feed);
    }

    pub fn remove_feed(&mut self, name: &str) -> Option<Box<dyn MarketFeed>> {
        self.feeds.remove(name)
    }

    #[must_use]
    pub fn feed_count(&self) -> usize {
        self.feeds.len()
    }

    #[must_use]
    pub fn feed_status(&self, name: &str) -> Option<FeedStatus> {
        self.feeds.get(name).map(|f| f.status())
    }

    pub fn subscribe_to_updates(&mut self, subscriber: Box<dyn FeedSubscriber>) {
        self.subscribers.push(subscriber);
    }

    pub fn enable_broadcast(&mut self, capacity: usize) -> broadcast::Receiver<MarketUpdate> {
        let (tx, rx) = broadcast::channel(capacity);
        self.broadcast_tx = Some(tx);
        rx
    }

    pub fn set_wallet_guard(&mut self, guard: Arc<RwLock<WalletGuard>>) {
        self.wallet_guard = Some(guard);
    }

    pub fn subscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<(), FeedError> {
        for feed in self.feeds.values_mut() {
            feed.subscribe(symbols)?;
        }
        Ok(())
    }

    pub async fn process_update(&mut self, update: &MarketUpdate) {
        if let Some(tx) = &self.broadcast_tx {
            let _ = tx.send(update.clone());
        }

        for subscriber in &mut self.subscribers {
            subscriber.on_update(update).await;
        }
    }

    pub async fn validate_with_wallet_guard(&self, wallet: &Wallet, order: &Order) -> RiskDecision {
        if let Some(guard) = &self.wallet_guard {
            let guard = guard.read().await;
            guard.check(wallet, order)
        } else {
            RiskDecision::Approve
        }
    }
}

impl Default for FeedManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LoggingSubscriber {
    pub update_count: u64,
    pub error_count: u64,
}

impl LoggingSubscriber {
    #[must_use]
    pub fn new() -> Self {
        Self {
            update_count: 0,
            error_count: 0,
        }
    }
}

impl Default for LoggingSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FeedSubscriber for LoggingSubscriber {
    async fn on_update(&mut self, _update: &MarketUpdate) {
        self.update_count += 1;
    }

    async fn on_error(&mut self, _error: &FeedError) {
        self.error_count += 1;
    }
}

pub struct CallbackSubscriber<F, E>
where
    F: FnMut(&MarketUpdate) + Send + Sync,
    E: FnMut(&FeedError) + Send + Sync,
{
    on_update_cb: F,
    on_error_cb: E,
}

impl<F, E> CallbackSubscriber<F, E>
where
    F: FnMut(&MarketUpdate) + Send + Sync,
    E: FnMut(&FeedError) + Send + Sync,
{
    #[must_use]
    pub fn new(on_update: F, on_error: E) -> Self {
        Self {
            on_update_cb: on_update,
            on_error_cb: on_error,
        }
    }
}

#[async_trait]
impl<F, E> FeedSubscriber for CallbackSubscriber<F, E>
where
    F: FnMut(&MarketUpdate) + Send + Sync,
    E: FnMut(&FeedError) + Send + Sync,
{
    async fn on_update(&mut self, update: &MarketUpdate) {
        (self.on_update_cb)(update);
    }

    async fn on_error(&mut self, error: &FeedError) {
        (self.on_error_cb)(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_feed_manager_new() {
        let manager = FeedManager::new();
        assert_eq!(manager.feed_count(), 0);
    }

    #[tokio::test]
    async fn test_logging_subscriber() {
        let mut sub = LoggingSubscriber::new();
        let update = MarketUpdate::Heartbeat;

        sub.on_update(&update).await;
        sub.on_update(&update).await;
        sub.on_error(&FeedError::NotConnected).await;

        assert_eq!(sub.update_count, 2);
        assert_eq!(sub.error_count, 1);
    }

    #[tokio::test]
    async fn test_callback_subscriber() {
        use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
        use std::sync::Arc as StdArc;

        let counter = StdArc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();

        let mut sub = CallbackSubscriber::new(
            move |_| {
                counter_clone.fetch_add(1, AtomicOrdering::SeqCst);
            },
            move |_| {},
        );

        sub.on_update(&MarketUpdate::Heartbeat).await;
        sub.on_update(&MarketUpdate::Heartbeat).await;

        assert_eq!(counter.load(AtomicOrdering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_wallet_guard_integration() {
        use crate::broker::wallet_guard::{OrderSide, RiskDecision};

        let mut manager = FeedManager::new();
        let guard = Arc::new(RwLock::new(WalletGuard::with_defaults()));
        manager.set_wallet_guard(guard);

        let order = Order::new(1, OrderSide::Buy, 10, 100);
        let wallet = Wallet::new(1_000_000); // Sufficient cash for margin

        let result = manager.validate_with_wallet_guard(&wallet, &order).await;
        assert_eq!(result, RiskDecision::Approve);
    }
}
