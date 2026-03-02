//! HFT Broker Component
//!
//! Per BP-HFT-BROKER-001:
//! - Coordinates ring buffer, wallet guard, signal dispatch
//! - Implements Component trait for Host Kernel integration
//! - Sub-millisecond latency requirements

use crate::component::{Component, ComponentId, ComponentInfo, ComponentState};
use crate::error::{ClawdiusError, Result};
use crate::market_data::{MarketDataIngestor, WebSocketConfig};
use crate::ring_buffer::{MarketDataMessage, RingBuffer};
use crate::signal_dispatch::{NotificationChannel, NotificationGateway, Signal, SignalAction, SignalDispatcher};
use crate::wallet_guard::{Order, OrderSide, RiskDecision, RiskParams, Wallet, WalletGuard};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Broker component version
pub const BROKER_VERSION: &str = "0.1.0";

const DEFAULT_RING_BUFFER_SIZE: usize = 1 << 20;
const MAX_SIGNAL_LATENCY_US: u64 = 1000;
const MAX_RISK_CHECK_US: u64 = 100;
const MAX_NOTIFICATION_MS: u64 = 100;

/// Broker operating mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerMode {
    /// Paper trading mode (no real orders)
    Paper,
    /// Live trading mode
    Live,
    /// Simulation mode with synthetic data
    Simulation,
}

/// Broker configuration
#[derive(Debug)]
pub struct BrokerConfig {
    /// Operating mode
    pub mode: BrokerMode,
    /// Ring buffer size (must be power of 2)
    pub ring_buffer_size: usize,
    /// Risk parameters
    pub risk_params: RiskParams,
    /// Notification channels for signal dispatch
    pub notification_channels: Vec<NotificationChannel>,
    /// WebSocket configuration for market data
    pub ws_config: WebSocketConfig,
}

impl Default for BrokerConfig {
    fn default() -> Self {
        Self {
            mode: BrokerMode::Paper,
            ring_buffer_size: DEFAULT_RING_BUFFER_SIZE,
            risk_params: RiskParams::default(),
            notification_channels: vec![NotificationChannel::Log],
            ws_config: WebSocketConfig::default(),
        }
    }
}

/// Broker performance metrics
#[derive(Debug, Default)]
pub struct BrokerMetrics {
    /// Total signals generated
    pub signals_generated: AtomicU64,
    /// Signals approved by risk checks
    pub signals_approved: AtomicU64,
    /// Signals rejected by risk checks
    pub signals_rejected: AtomicU64,
    /// Market data messages received
    pub market_data_received: AtomicU64,
    /// Total time spent in risk checks (microseconds)
    pub risk_check_total_us: AtomicU64,
    /// Number of risk checks performed
    pub risk_check_count: AtomicU64,
}

impl BrokerMetrics {
    /// Create new metrics instance
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get average risk check duration in microseconds
    #[must_use]
    pub fn avg_risk_check_us(&self) -> u64 {
        let count = self.risk_check_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0;
        }
        self.risk_check_total_us.load(Ordering::Relaxed) / count
    }

    /// Get signal rejection rate (0.0 to 1.0)
    #[must_use]
    pub fn rejection_rate(&self) -> f64 {
        let total = self.signals_approved.load(Ordering::Relaxed)
            + self.signals_rejected.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        (self.signals_rejected.load(Ordering::Relaxed) as f64) / (total as f64)
    }
}

/// HFT Broker component implementing the Component trait
#[derive(Debug)]
pub struct Broker {
    state: ComponentState,
    config: BrokerConfig,
    ring_buffer: RingBuffer,
    wallet_guard: WalletGuard,
    wallet: Wallet,
    ingestor: MarketDataIngestor,
    dispatcher: SignalDispatcher,
    metrics: BrokerMetrics,
    running: Arc<AtomicBool>,
}

impl Broker {
    /// Create a new Broker with the given configuration
    ///
    /// # Errors
    /// Returns an error if ring buffer allocation fails
    pub fn new(config: BrokerConfig) -> Result<Self> {
        let ring_buffer = RingBuffer::new(config.ring_buffer_size)
            .map_err(|e| ClawdiusError::Config(format!("Ring buffer creation failed: {e:?}")))?;

        let wallet_guard = WalletGuard::new(config.risk_params.clone());
        let wallet = Wallet::new(100_000_000);
        let ingestor = MarketDataIngestor::new(config.ws_config.clone());
        let gateway = NotificationGateway::new();
        let dispatcher = SignalDispatcher::new(gateway);

        Ok(Self {
            state: ComponentState::Uninitialized,
            config,
            ring_buffer,
            wallet_guard,
            wallet,
            ingestor,
            dispatcher,
            metrics: BrokerMetrics::new(),
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Set a custom wallet for the broker
    #[must_use]
    pub fn with_wallet(mut self, wallet: Wallet) -> Self {
        self.wallet = wallet;
        self
    }

    /// Get component info for the broker
    #[must_use]
    pub fn info(&self) -> ComponentInfo {
        ComponentInfo::new(ComponentId::BROKER, self.name(), BROKER_VERSION)
    }

    /// Get broker metrics
    #[must_use]
    pub fn metrics(&self) -> &BrokerMetrics {
        &self.metrics
    }

    /// Get broker operating mode
    #[must_use]
    pub fn mode(&self) -> BrokerMode {
        self.config.mode
    }

    /// Push market data to the ring buffer
    ///
    /// # Errors
    /// Returns an error if the ring buffer is full
    pub fn push_market_data(&self, msg: MarketDataMessage) -> Result<()> {
        self.ring_buffer
            .try_write(msg)
            .map_err(|e| ClawdiusError::Host(crate::error::HostError::ComponentFailure {
                component: "Broker".to_string(),
                reason: format!("Ring buffer write failed: {e:?}"),
            }))?;

        self.metrics.market_data_received.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Pop market data from the ring buffer
    #[must_use]
    pub fn pop_market_data(&self) -> Option<MarketDataMessage> {
        self.ring_buffer.try_read().ok()
    }

    /// Process a trading signal through risk checks
    ///
    /// # Errors
    /// This function does not return errors but logs WCET violations
    pub fn process_signal(&mut self, signal: &Signal) -> Result<RiskDecision> {
        let start = Instant::now();
        self.metrics.signals_generated.fetch_add(1, Ordering::Relaxed);

        let order = self.signal_to_order(signal);

        let decision = self.wallet_guard.check(&self.wallet, &order);

        let elapsed_us = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX);
        self.metrics.risk_check_total_us.fetch_add(elapsed_us, Ordering::Relaxed);
        self.metrics.risk_check_count.fetch_add(1, Ordering::Relaxed);

        if elapsed_us > MAX_RISK_CHECK_US {
            tracing::warn!(
                duration_us = elapsed_us,
                max_us = MAX_RISK_CHECK_US,
                "Risk check exceeded WCET bound"
            );
        }

        match decision {
            RiskDecision::Approve => {
                self.metrics.signals_approved.fetch_add(1, Ordering::Relaxed);
                self.apply_order_to_wallet(&order);
            }
            RiskDecision::Reject(_) => {
                self.metrics.signals_rejected.fetch_add(1, Ordering::Relaxed);
            }
        }

        Ok(decision)
    }

    fn signal_to_order(&self, signal: &Signal) -> Order {
        let side = match signal.action {
            SignalAction::Buy => OrderSide::Buy,
            SignalAction::Sell => OrderSide::Sell,
            SignalAction::Close => {
                let current = self.wallet.position(signal.symbol);
                if current > 0 {
                    OrderSide::Sell
                } else {
                    OrderSide::Buy
                }
            }
        };

        Order::new(signal.symbol, side, signal.quantity, u64::try_from(signal.price.unwrap_or(0)).unwrap_or(0))
    }

    fn apply_order_to_wallet(&mut self, order: &Order) {
        self.wallet.update_position(order.symbol, order.signed_quantity());
    }

    /// Dispatch notification for a signal
    ///
    /// # Errors
    /// Returns an error if notification dispatch fails
    pub async fn dispatch_notification(
        &self,
        signal: &Signal,
    ) -> Result<Vec<crate::signal_dispatch::DispatchStatus>> {
        let start = Instant::now();

        let results = self.dispatcher
            .dispatch_signal(signal, &self.config.notification_channels)
            .await;

        let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        if elapsed_ms > MAX_NOTIFICATION_MS {
            tracing::warn!(
                duration_ms = elapsed_ms,
                max_ms = MAX_NOTIFICATION_MS,
                "Notification dispatch exceeded latency bound"
            );
        }

        results.into_iter().collect()
    }

    /// Get a reference to the wallet
    pub fn wallet(&self) -> &Wallet {
        &self.wallet
    }

    /// Get a mutable reference to the wallet
    pub fn wallet_mut(&mut self) -> &mut Wallet {
        &mut self.wallet
    }

    /// Get the current number of items in the ring buffer
    pub fn ring_buffer_len(&self) -> usize {
        self.ring_buffer.len()
    }

    /// Get the ring buffer capacity
    pub fn ring_buffer_capacity(&self) -> usize {
        self.ring_buffer.capacity()
    }
}

impl Component for Broker {
    fn id(&self) -> ComponentId {
        ComponentId::BROKER
    }

    fn name(&self) -> &'static str {
        "Broker"
    }

    fn state(&self) -> ComponentState {
        self.state
    }

    fn initialize(&mut self) -> Result<()> {
        if self.state != ComponentState::Uninitialized {
            return Err(ClawdiusError::Config("Broker already initialized".into()));
        }

        tracing::info!(
            mode = ?self.config.mode,
            ring_buffer_size = self.ring_buffer.capacity(),
            "Initializing HFT Broker"
        );

        self.ingestor.connect()?;
        self.state = ComponentState::Initialized;

        tracing::info!("HFT Broker initialized");
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        if self.state != ComponentState::Initialized {
            return Err(ClawdiusError::Config(
                "Broker must be initialized before starting".into(),
            ));
        }

        tracing::info!("Starting HFT Broker");
        self.running.store(true, Ordering::SeqCst);
        self.state = ComponentState::Running;

        tracing::info!("HFT Broker running");
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if self.state != ComponentState::Running {
            return Err(ClawdiusError::Config("Broker is not running".into()));
        }

        tracing::info!("Stopping HFT Broker");
        self.running.store(false, Ordering::SeqCst);
        self.ingestor.disconnect();
        self.state = ComponentState::Stopped;

        tracing::info!(
            signals_generated = self.metrics.signals_generated.load(Ordering::Relaxed),
            signals_approved = self.metrics.signals_approved.load(Ordering::Relaxed),
            signals_rejected = self.metrics.signals_rejected.load(Ordering::Relaxed),
            avg_risk_check_us = self.metrics.avg_risk_check_us(),
            rejection_rate = self.metrics.rejection_rate(),
            "HFT Broker stopped"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_broker() -> Broker {
        Broker::new(BrokerConfig::default()).expect("Failed to create broker")
    }

    #[test]
    fn test_broker_creation() {
        let broker = create_test_broker();
        assert_eq!(broker.state(), ComponentState::Uninitialized);
        assert_eq!(broker.name(), "Broker");
        assert_eq!(broker.id(), ComponentId::BROKER);
    }

    #[test]
    fn test_broker_lifecycle() {
        let mut broker = create_test_broker();
        
        assert_eq!(broker.state(), ComponentState::Uninitialized);
        
        broker.initialize().expect("Initialize failed");
        assert_eq!(broker.state(), ComponentState::Initialized);
        
        broker.start().expect("Start failed");
        assert_eq!(broker.state(), ComponentState::Running);
        
        broker.stop().expect("Stop failed");
        assert_eq!(broker.state(), ComponentState::Stopped);
    }

    #[test]
    fn test_broker_double_initialize() {
        let mut broker = create_test_broker();
        broker.initialize().expect("First init failed");
        let result = broker.initialize();
        assert!(result.is_err());
    }

    #[test]
    fn test_broker_start_without_init() {
        let mut broker = create_test_broker();
        let result = broker.start();
        assert!(result.is_err());
    }

    #[test]
    fn test_broker_ring_buffer() {
        let broker = create_test_broker();
        assert!(broker.ring_buffer.is_empty());
        
        let msg = MarketDataMessage {
            msg_type: 1,
            symbol_id: 42,
            price: 10000,
            quantity: 100,
            timestamp_ns: 12345,
        };
        
        broker.push_market_data(msg).expect("Push failed");
        assert_eq!(broker.ring_buffer_len(), 1);
        
        let read = broker.pop_market_data().expect("Pop failed");
        assert_eq!(read.symbol_id, 42);
    }

    #[test]
    fn test_broker_signal_processing() {
        let mut broker = create_test_broker();
        broker.initialize().expect("Init failed");
        
        let signal = Signal::new(1, 42, SignalAction::Buy, 100, 1)
            .with_price(10000);
        
        let decision = broker.process_signal(&signal).expect("Process failed");
        assert_eq!(decision, RiskDecision::Approve);
        
        assert_eq!(broker.metrics().signals_generated.load(Ordering::Relaxed), 1);
        assert_eq!(broker.metrics().signals_approved.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_broker_signal_rejection() {
        let config = BrokerConfig {
            risk_params: RiskParams {
                pi_max: 100,
                sigma_max: 50,
                lambda_max: 1000,
                margin_ratio: 4,
            },
            ..Default::default()
        };
        
        let mut broker = Broker::new(config).expect("Failed to create broker");
        broker.initialize().expect("Init failed");
        
        let signal = Signal::new(1, 42, SignalAction::Buy, 200, 1)
            .with_price(10000);
        
        let decision = broker.process_signal(&signal).expect("Process failed");
        assert!(matches!(decision, RiskDecision::Reject(_)));
        assert_eq!(broker.metrics().signals_rejected.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_broker_metrics() {
        let metrics = BrokerMetrics::new();
        
        metrics.signals_generated.store(100, Ordering::Relaxed);
        metrics.signals_approved.store(80, Ordering::Relaxed);
        metrics.signals_rejected.store(20, Ordering::Relaxed);
        
        assert!((metrics.rejection_rate() - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_broker_info() {
        let broker = create_test_broker();
        let info = broker.info();
        
        assert_eq!(info.id, ComponentId::BROKER);
        assert_eq!(info.name, "Broker");
        assert_eq!(info.version, BROKER_VERSION);
    }
}
