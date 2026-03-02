//! Signal Dispatch for HFT Broker
//!
//! Per BP-HFT-BROKER-001:
//! - Signal types (Buy, Sell, Close)
//! - Notification gateway (Matrix, WhatsApp)
//! - Dispatch within 100ms requirement

use std::collections::HashMap;
use std::time::{Duration, Instant};

pub type SignalId = u64;
pub type StrategyId = u32;
pub type OrderId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalAction {
    Buy,
    Sell,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub struct Signal {
    pub id: SignalId,
    pub symbol: crate::market_data::Symbol,
    pub action: SignalAction,
    pub quantity: u64,
    pub price: Option<i64>,
    pub strategy: StrategyId,
    pub priority: SignalPriority,
    pub generated_at: Instant,
    pub metadata: HashMap<String, String>,
}

impl Signal {
    pub fn new(
        id: SignalId,
        symbol: crate::market_data::Symbol,
        action: SignalAction,
        quantity: u64,
        strategy: StrategyId,
    ) -> Self {
        Self {
            id,
            symbol,
            action,
            quantity,
            price: None,
            strategy,
            priority: SignalPriority::Normal,
            generated_at: Instant::now(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_price(mut self, price: i64) -> Self {
        self.price = Some(price);
        self
    }

    pub fn with_priority(mut self, priority: SignalPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn age(&self) -> Duration {
        self.generated_at.elapsed()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationChannel {
    Matrix,
    WhatsApp,
    Telegram,
    Log,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub channel: NotificationChannel,
    pub message: String,
    pub priority: SignalPriority,
    pub signal_id: SignalId,
    pub created_at: Instant,
}

impl Notification {
    pub fn new(channel: NotificationChannel, message: String, signal_id: SignalId) -> Self {
        Self {
            channel,
            message,
            priority: SignalPriority::Normal,
            signal_id,
            created_at: Instant::now(),
        }
    }

    pub fn with_priority(mut self, priority: SignalPriority) -> Self {
        self.priority = priority;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchResult {
    Sent,
    Queued,
    Failed,
    Timeout,
}

#[derive(Debug, Clone)]
pub struct DispatchStatus {
    pub signal_id: SignalId,
    pub result: DispatchResult,
    pub channel: NotificationChannel,
    pub latency: Duration,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct NotificationGateway {
    matrix_enabled: bool,
    whatsapp_enabled: bool,
    telegram_enabled: bool,
}

impl NotificationGateway {
    pub fn new() -> Self {
        Self {
            matrix_enabled: false,
            whatsapp_enabled: false,
            telegram_enabled: false,
        }
    }

    pub fn with_matrix(mut self, enabled: bool) -> Self {
        self.matrix_enabled = enabled;
        self
    }

    pub fn with_whatsapp(mut self, enabled: bool) -> Self {
        self.whatsapp_enabled = enabled;
        self
    }

    pub fn with_telegram(mut self, enabled: bool) -> Self {
        self.telegram_enabled = enabled;
        self
    }

    pub async fn dispatch(&self, notification: Notification) -> crate::error::Result<DispatchStatus> {
        let start = Instant::now();
        let signal_id = notification.signal_id;
        let channel = notification.channel;

        match channel {
            NotificationChannel::Matrix if self.matrix_enabled => {
                self.send_matrix(&notification).await?;
                Ok(DispatchStatus {
                    signal_id,
                    result: DispatchResult::Sent,
                    channel,
                    latency: start.elapsed(),
                    error: None,
                })
            }
            NotificationChannel::WhatsApp if self.whatsapp_enabled => {
                self.send_whatsapp(&notification).await?;
                Ok(DispatchStatus {
                    signal_id,
                    result: DispatchResult::Sent,
                    channel,
                    latency: start.elapsed(),
                    error: None,
                })
            }
            NotificationChannel::Telegram if self.telegram_enabled => {
                self.send_telegram(&notification).await?;
                Ok(DispatchStatus {
                    signal_id,
                    result: DispatchResult::Sent,
                    channel,
                    latency: start.elapsed(),
                    error: None,
                })
            }
            NotificationChannel::Log => {
                self.send_log(&notification);
                Ok(DispatchStatus {
                    signal_id,
                    result: DispatchResult::Sent,
                    channel,
                    latency: start.elapsed(),
                    error: None,
                })
            }
            _ => Ok(DispatchStatus {
                signal_id,
                result: DispatchResult::Failed,
                channel,
                latency: start.elapsed(),
                error: Some("Channel not enabled".to_string()),
            }),
        }
    }

    async fn send_matrix(&self, notification: &Notification) -> crate::error::Result<()> {
        tracing::info!(
            channel = "Matrix",
            signal_id = notification.signal_id,
            priority = ?notification.priority,
            "Dispatching notification: {}",
            notification.message
        );
        Ok(())
    }

    async fn send_whatsapp(&self, notification: &Notification) -> crate::error::Result<()> {
        tracing::info!(
            channel = "WhatsApp",
            signal_id = notification.signal_id,
            priority = ?notification.priority,
            "Dispatching notification: {}",
            notification.message
        );
        Ok(())
    }

    async fn send_telegram(&self, notification: &Notification) -> crate::error::Result<()> {
        tracing::info!(
            channel = "Telegram",
            signal_id = notification.signal_id,
            priority = ?notification.priority,
            "Dispatching notification: {}",
            notification.message
        );
        Ok(())
    }

    fn send_log(&self, notification: &Notification) {
        match notification.priority {
            SignalPriority::Critical => {
                tracing::error!(
                    signal_id = notification.signal_id,
                    "CRITICAL: {}",
                    notification.message
                );
            }
            SignalPriority::High => {
                tracing::warn!(
                    signal_id = notification.signal_id,
                    "HIGH: {}",
                    notification.message
                );
            }
            _ => {
                tracing::info!(
                    signal_id = notification.signal_id,
                    "{}",
                    notification.message
                );
            }
        }
    }
}

impl Default for NotificationGateway {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct SignalDispatcher {
    gateway: NotificationGateway,
    signal_counter: SignalId,
}

impl SignalDispatcher {
    pub fn new(gateway: NotificationGateway) -> Self {
        Self {
            gateway,
            signal_counter: 0,
        }
    }

    pub fn next_signal_id(&mut self) -> SignalId {
        self.signal_counter += 1;
        self.signal_counter
    }

    pub async fn dispatch_signal(
        &self,
        signal: &Signal,
        channels: &[NotificationChannel],
    ) -> Vec<crate::error::Result<DispatchStatus>> {
        let message = format!(
            "Signal #{}: {:?} {} @ {} (strategy: {})",
            signal.id,
            signal.action,
            signal.symbol,
            signal.price.map(|p| p.to_string()).unwrap_or_else(|| "MARKET".to_string()),
            signal.strategy
        );

        let mut results = Vec::with_capacity(channels.len());
        for &channel in channels {
            let notification = Notification::new(channel, message.clone(), signal.id)
                .with_priority(signal.priority);
            results.push(self.gateway.dispatch(notification).await);
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_creation() {
        let signal = Signal::new(1, 42, SignalAction::Buy, 100, 1)
            .with_price(10000)
            .with_priority(SignalPriority::High);

        assert_eq!(signal.id, 1);
        assert_eq!(signal.symbol, 42);
        assert_eq!(signal.action, SignalAction::Buy);
        assert_eq!(signal.quantity, 100);
        assert_eq!(signal.price, Some(10000));
        assert_eq!(signal.priority, SignalPriority::High);
    }

    #[test]
    fn test_signal_age() {
        let signal = Signal::new(1, 42, SignalAction::Buy, 100, 1);
        std::thread::sleep(Duration::from_millis(10));
        assert!(signal.age() >= Duration::from_millis(10));
    }

    #[test]
    fn test_notification_dispatch_log() {
        let gateway = NotificationGateway::new();
        let notification = Notification::new(NotificationChannel::Log, "Test message".to_string(), 1);
        
        let mut rt = monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
            .enable_timer()
            .build()
            .expect("Failed to create runtime");
        let status = rt.block_on(gateway.dispatch(notification)).unwrap();
        assert_eq!(status.result, DispatchResult::Sent);
        assert!(status.latency < Duration::from_millis(100));
    }

    #[test]
    fn test_signal_dispatcher_id_generation() {
        let gateway = NotificationGateway::new();
        let mut dispatcher = SignalDispatcher::new(gateway);
        
        assert_eq!(dispatcher.next_signal_id(), 1);
        assert_eq!(dispatcher.next_signal_id(), 2);
        assert_eq!(dispatcher.next_signal_id(), 3);
    }
}
