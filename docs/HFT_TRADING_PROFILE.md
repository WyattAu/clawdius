# HFT Trading Profile

## Overview

Clawdius HFT (High-Frequency Trading) mode provides a Rust-native, low-latency trading infrastructure with LLM-powered signal analysis. The platform combines traditional algorithmic trading with AI-driven market sentiment analysis for enhanced decision-making.

### Key Features

| Feature | Description | Status |
|---------|-------------|--------|
| Lock-free Ring Buffer | SPSC ring buffer with cache-padded atomics | ✅ Implemented |
| SEC 15c3-5 Risk Controls | Pre-trade risk management (WalletGuard) | ✅ Implemented |
| Signal Engine | Multi-strategy signal generation | ✅ Implemented |
| Feed Manager | Multi-source market data distribution | ✅ Implemented |
| Notification Gateway | Multi-channel alerts (Webhook, Matrix) | ✅ Implemented |
| LLM Sentiment Analysis | AI-powered market sentiment | 🔜 Planned |
| News Feed Integration | Real-time news processing | 🔜 Planned |
| Paper Trading Mode | Risk-free strategy testing | 🔜 Planned |
| Broker Connectors | Exchange API integrations | 🔜 Planned |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           CLAWDIUS HFT ARCHITECTURE                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │  News Feed   │  │ Social Feed  │  │  SEC Filings │  │ Market Data  │    │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘    │
│         │                 │                 │                 │            │
│         v                 v                 v                 v            │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         FEED MANAGER                                  │   │
│  │  • Multi-feed aggregation     • Subscriber distribution              │   │
│  │  • Symbol subscription        • Broadcast channels                   │   │
│  └──────────────────────────────┬──────────────────────────────────────┘   │
│                                 │                                           │
│         ┌───────────────────────┼───────────────────────┐                  │
│         v                       v                       v                  │
│  ┌─────────────┐         ┌─────────────┐         ┌─────────────┐          │
│  │  LLM Agent  │         │  Strategy   │         │  Technical  │          │
│  │  (Sentiment)│         │  Engine     │         │  Analysis   │          │
│  └──────┬──────┘         └──────┬──────┘         └──────┬──────┘          │
│         │                       │                       │                  │
│         v                       v                       v                  │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        SIGNAL ENGINE                                  │   │
│  │  • Strategy registration     • Signal aggregation                    │   │
│  │  • Ring buffer storage       • Confidence scoring                    │   │
│  └──────────────────────────────┬──────────────────────────────────────┘   │
│                                 │                                           │
│                                 v                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        WALLET GUARD                                   │   │
│  │  SEC 15c3-5 Pre-Trade Risk Controls:                                  │   │
│  │  • Order value limits        • Position size limits                   │   │
│  │  • Daily volume caps         • Restricted symbol blocking             │   │
│  └──────────────────────────────┬──────────────────────────────────────┘   │
│                                 │                                           │
│                    ┌────────────┴────────────┐                              │
│                    v                         v                              │
│            ┌─────────────┐           ┌─────────────┐                       │
│            │  EXECUTION  │           │ NOTIFICATION│                       │
│            │  ENGINE     │           │  GATEWAY    │                       │
│            └──────┬──────┘           └──────┬──────┘                       │
│                   │                         │                              │
│                   v                         v                              │
│            ┌─────────────┐           ┌─────────────┐                       │
│            │   Broker    │           │  Webhook/   │                       │
│            │  Connector  │           │  Matrix     │                       │
│            └─────────────┘           └─────────────┘                       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Existing Infrastructure

### 1. Ring Buffer (`ring_buffer.rs`)

Lock-free single-producer single-consumer (SPSC) ring buffer optimized for HFT scenarios.

**Features:**
- Cache-padded atomics to prevent false sharing
- Power-of-2 capacity requirement for fast modulo via bitwise AND
- `unsafe` optimizations for minimal overhead
- `Send + Sync` for cross-thread usage

**API:**
```rust
let buffer: RingBuffer<Signal, 1024> = RingBuffer::new();

// Push returns Err if full
buffer.push(signal)?;

// Pop returns None if empty
let signal = buffer.pop();

// Query state
let len = buffer.len();
let is_empty = buffer.is_empty();
let capacity = buffer.capacity(); // N - 1
```

**Performance Target:** `<100ns` per operation

---

### 2. Wallet Guard (`wallet_guard.rs`)

SEC 15c3-5 compliant pre-trade risk controls for market access.

**Risk Checks:**

| Check | Description | Default Limit |
|-------|-------------|---------------|
| `OrderValueLimit` | Maximum value per order | $1,000,000 |
| `DailyVolumeLimit` | Maximum daily trading volume | $10,000,000 |
| `PositionSizeLimit` | Maximum position per symbol | 100,000 shares |
| `RestrictedSymbol` | Blocked symbols | Configurable |

**API:**
```rust
let mut guard = WalletGuard::new(
    Decimal::from(1_000_000),  // max_order_value
    Decimal::from(10_000_000), // max_daily_volume
    Decimal::from(100_000),    // max_position_size
);

guard.restrict_symbol("PENN");
guard.restrict_symbol("GME");

let order = Order {
    symbol: "AAPL".to_string(),
    quantity: Decimal::from(100),
    price: Decimal::from(150),
    side: OrderSide::Buy,
};

// Returns Ok(()) or Err(Vec<RiskCheck>)
match guard.check_order(&order) {
    Ok(()) => execute_order(order),
    Err(failures) => reject_order(failures),
}
```

**Performance Target:** `<100µs` per check

---

### 3. Signal Engine (`signal.rs`)

Processes market data through registered strategies to generate trading signals.

**Signal Structure:**
```rust
pub struct Signal {
    pub symbol: String,
    pub direction: SignalDirection,  // Buy, Sell, Hold
    pub confidence: f64,              // 0.0 - 1.0
    pub strategy: String,             // Strategy name
    pub timestamp: u64,               // Unix millis
}
```

**API:**
```rust
let mut engine = SignalEngine::new();
engine.register_strategy(Box::new(MovingAverageCrossover::new(10, 20)));
engine.register_strategy(Box::new(RsiStrategy::new(14)));

let market_data = MarketData {
    symbol: "AAPL".to_string(),
    price: Decimal::from(150),
    volume: Decimal::from(1_000_000),
    timestamp: 1700000000000,
};

// Process through all strategies
let signals = engine.process(&market_data);

// Drain accumulated signals
let pending = engine.drain_signals();
```

---

### 4. Feed Manager (`feed_manager.rs`)

Manages multiple market data feeds and distributes updates to subscribers.

**Features:**
- Multi-feed registration
- Broadcast channel support
- Wallet guard integration
- Callback and logging subscribers

**API:**
```rust
let mut manager = FeedManager::new();

// Register feeds
manager.register_feed("primary", Box::new(AlpacaFeed::new()));
manager.register_feed("backup", Box::new(IexFeed::new()));

// Subscribe to symbols
manager.subscribe_symbols(&[Symbol::new("AAPL"), Symbol::new("MSFT")]);

// Enable broadcast
let mut rx = manager.enable_broadcast(1000);

// Add subscribers
manager.subscribe_to_updates(Box::new(LoggingSubscriber::new()));

// Set risk guard
manager.set_wallet_guard(Arc::new(RwLock::new(WalletGuard::default())));

// Process updates
manager.process_update(&update).await;

// Validate orders
let result = manager.validate_with_wallet_guard(&order).await;
```

---

### 5. Notification Gateway (`notification.rs`)

Multi-channel notification system for trade alerts and system events.

**Supported Channels:**
- **Webhook** - HTTP POST to any URL
- **Matrix** - Matrix protocol for secure messaging

**API:**
```rust
let gateway = NotificationGateway::new();

// Add channels
gateway.add_channel(Box::new(WebhookChannel::new(
    "slack",
    "https://hooks.slack.com/services/..."
))).await;

gateway.add_channel(Box::new(MatrixChannel::new(
    "trading-alerts",
    "https://matrix.org",
    "!roomid:matrix.org",
    "access_token"
))).await;

// Broadcast to all channels
let results = gateway.broadcast("🚨 Signal: AAPL Buy @ $150 (0.85 confidence)").await;

// Check results
for (channel, result) in results {
    match result {
        Ok(()) => println!("{}: sent", channel),
        Err(e) => eprintln!("{}: failed - {}", channel, e),
    }
}
```

---

### 6. Strategy Trait (`strategy.rs`)

Interface for implementing custom trading strategies.

**Trait Definition:**
```rust
pub trait Strategy: Send + Sync {
    /// Evaluates market data and optionally returns a signal
    fn evaluate(&self, market_data: &MarketData) -> Option<Signal>;
    
    /// Returns the strategy name
    fn name(&self) -> &str;
}
```

**Example Implementation:**
```rust
pub struct MovingAverageCrossover {
    short_period: usize,
    long_period: usize,
    short_prices: VecDeque<Decimal>,
    long_prices: VecDeque<Decimal>,
}

impl Strategy for MovingAverageCrossover {
    fn evaluate(&self, data: &MarketData) -> Option<Signal> {
        // Update price buffers
        // Calculate moving averages
        // Detect crossover
        // Return signal if crossover detected
        todo!()
    }
    
    fn name(&self) -> &str {
        "MovingAverageCrossover"
    }
}
```

---

## LLM-Powered Signal Analysis (Planned)

### Architecture Extension

```
┌─────────────────────────────────────────────────────────────────┐
│                    LLM SIGNAL ANALYSIS LAYER                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  News Feed   │  │ Social Media │  │  SEC Filings │          │
│  │  (Reuters,   │  │  (Twitter/X, │  │  (10-K, 10-Q,│          │
│  │   Bloomberg) │  │   Reddit)    │  │   8-K)       │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                 │                 │                   │
│         v                 v                 v                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              PREPROCESSING PIPELINE                       │   │
│  │  • Text cleaning    • Entity extraction                   │   │
│  │  • Language detect  • Timestamp normalization             │   │
│  └─────────────────────────┬───────────────────────────────┘   │
│                            │                                    │
│                            v                                    │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    LLM ANALYSIS                           │   │
│  │                                                           │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │   │
│  │  │  Sentiment  │  │   Event     │  │   Risk      │      │   │
│  │  │  Analysis   │  │   Detection │  │   Scoring   │      │   │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘      │   │
│  │         │                │                │              │   │
│  │         v                v                v              │   │
│  │  ┌─────────────────────────────────────────────────┐    │   │
│  │  │              SENTIMENT SIGNAL                     │    │   │
│  │  │  {                                               │    │   │
│  │  │    "symbol": "AAPL",                             │    │   │
│  │  │    "sentiment": 0.72,  // -1.0 to 1.0            │    │   │
│  │  │    "confidence": 0.85,                           │    │   │
│  │  │    "events": ["earnings_beat", "guidance_raise"],│    │   │
│  │  │    "risk_factors": ["supply_chain"],             │    │   │
│  │  │    "sources": 15,                                │    │   │
│  │  │    "timestamp": 1700000000000                    │    │   │
│  │  │  }                                               │    │   │
│  │  └─────────────────────────────────────────────────┘    │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### LLM Strategy Implementation

```rust
/// LLM-powered sentiment analysis strategy
pub struct LlmSentimentStrategy {
    llm_client: LlmClient,
    news_sources: Vec<NewsSource>,
    sentiment_threshold: f64,
    min_sources: usize,
}

#[derive(Debug, Clone)]
pub struct SentimentSignal {
    pub symbol: String,
    pub sentiment: f64,           // -1.0 (very bearish) to 1.0 (very bullish)
    pub confidence: f64,
    pub events: Vec<MarketEvent>,
    pub risk_factors: Vec<RiskFactor>,
    pub source_count: usize,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub enum MarketEvent {
    EarningsBeat,
    EarningsMiss,
    GuidanceRaise,
    GuidanceCut,
    DividendIncrease,
    StockSplit,
    MergerAnnouncement,
    RegulatoryAction,
    ProductLaunch,
    ExecutiveChange,
}

impl Strategy for LlmSentimentStrategy {
    fn evaluate(&self, market_data: &MarketData) -> Option<Signal> {
        // 1. Collect recent news/social for symbol
        // 2. Send to LLM for sentiment analysis
        // 3. Extract events and risk factors
        // 4. Calculate aggregate sentiment score
        // 5. Generate signal if threshold exceeded
        
        let sentiment = self.analyze_sentiment(&market_data.symbol).await?;
        
        if sentiment.source_count < self.min_sources {
            return None;
        }
        
        let direction = if sentiment.sentiment > self.sentiment_threshold {
            SignalDirection::Buy
        } else if sentiment.sentiment < -self.sentiment_threshold {
            SignalDirection::Sell
        } else {
            SignalDirection::Hold
        };
        
        if direction == SignalDirection::Hold {
            return None;
        }
        
        Some(Signal {
            symbol: market_data.symbol.clone(),
            direction,
            confidence: sentiment.confidence,
            strategy: self.name().to_string(),
            timestamp: market_data.timestamp,
        })
    }
    
    fn name(&self) -> &str {
        "LlmSentimentStrategy"
    }
}
```

### Signal Source Configuration

```toml
# config/trading/sentiment_sources.toml

[news_sources.reuters]
enabled = true
api_key = "${REUTERS_API_KEY}"
refresh_interval_ms = 5000
symbols = ["AAPL", "MSFT", "GOOGL", "AMZN", "NVDA"]

[news_sources.bloomberg]
enabled = true
api_key = "${BLOOMBERG_API_KEY}"
refresh_interval_ms = 5000

[social_sources.twitter]
enabled = true
bearer_token = "${TWITTER_BEARER_TOKEN}"
# Track cashtags and hashtags
track = ["$AAPL", "$MSFT", "#earnings", "#markets"]

[social_sources.reddit]
enabled = true
subreddits = ["wallstreetbets", "stocks", "investing"]
post_limit = 100

[sec_filings]
enabled = true
user_agent = "Clawdius Trading bot@clawdius.dev"
# Monitor 10-K, 10-Q, 8-K filings
form_types = ["10-K", "10-Q", "8-K", "4"]

[llm_analysis]
provider = "openai"  # or "anthropic", "local"
model = "gpt-4-turbo"
temperature = 0.3
max_tokens = 1000
# System prompt for financial analysis
system_prompt = """
You are a financial sentiment analyst. Analyze the provided text 
and extract:
1. Overall sentiment (-1.0 to 1.0)
2. Confidence (0.0 to 1.0)
3. Key market events
4. Risk factors mentioned

Respond in JSON format.
"""
```

---

## Trading Modes

### Paper Trading Mode

Risk-free strategy testing with simulated execution.

```toml
# config/profiles/trading_paper.toml

[trading]
mode = "paper"

[paper_trading]
# Starting virtual balance
initial_capital = 1_000_000.00
# Simulated slippage (basis points)
slippage_bps = 5
# Simulated latency (milliseconds)
execution_latency_ms = 50
# Include commissions
commission_rate = 0.001  # 0.1%

[persistence]
# Save paper trades for analysis
save_trades = true
trades_file = "data/paper_trades.jsonl"
# Performance metrics
calculate_metrics = true
metrics_interval_secs = 60
```

### Live Trading Mode

Real money execution with full risk controls.

```toml
# config/profiles/trading_live.toml

[trading]
mode = "live"

[broker]
# Supported: alpaca, interactive_brokers, coinbase
provider = "alpaca"
api_key = "${ALPACA_API_KEY}"
api_secret = "${ALPACA_API_SECRET}"
# live or paper endpoint (for testing live config)
endpoint = "live"

[risk]
# SEC 15c3-5 Compliance
max_order_value = 100_000.00
max_daily_volume = 1_000_000.00
max_position_size = 10_000
# Kill switch: halt trading if daily loss exceeds
max_daily_loss_pct = 5.0
# Restricted symbols
restricted_symbols = ["PENN", "GME", "AMC"]

[safety]
# Require manual confirmation for orders above
confirmation_threshold = 50_000.00
# Circuit breaker: pause after consecutive losses
circuit_breaker_losses = 5
circuit_breaker_pause_mins = 30

[notifications]
# Alert channels
channels = ["webhook", "matrix"]
# Notify on every trade
notify_on_fill = true
# Alert on risk limit warnings
notify_on_risk_warning = true
# Critical alerts only
notify_on_circuit_breaker = true
```

---

## Performance Targets

| Component | Target | Measurement |
|-----------|--------|-------------|
| Ring Buffer Push/Pop | <100ns | Criterion benchmark |
| Wallet Guard Check | <100µs | Criterion benchmark |
| Signal Generation | <1ms | End-to-end pipeline |
| HFT Stack Boot | <20ms | Initialization time |
| Order Execution | <10ms | Signal to broker |
| LLM Sentiment | <500ms | Per article analysis |

### Benchmark Commands

```bash
# Run HFT benchmarks
cargo bench --bench hft_bench

# Specific benchmark groups
cargo bench --bench hft_bench -- ring_buffer
cargo bench --bench hft_bench -- wallet_guard
cargo bench --bench hft_bench -- hft_pipeline
cargo bench --bench hft_bench -- boot_simulation
```

---

## Configuration Profiles

### Profile Directory Structure

```
config/
├── profiles/
│   ├── trading_paper.toml      # Paper trading config
│   ├── trading_live.toml       # Live trading config
│   └── trading_hft.toml        # High-frequency config
├── trading/
│   ├── sentiment_sources.toml  # LLM sentiment sources
│   ├── strategies.toml         # Strategy configurations
│   └── brokers.toml            # Broker connections
└── notifications/
    ├── webhooks.toml           # Webhook endpoints
    └── matrix.toml             # Matrix homeserver config
```

### HFT Profile Example

```toml
# config/profiles/trading_hft.toml

[profile]
name = "hft"
description = "High-frequency trading with LLM sentiment"
version = "1.0.0"

[trading]
mode = "live"
max_open_positions = 10
default_order_type = "limit"

[strategies]
# Enable strategies
enabled = [
    "MovingAverageCrossover",
    "RsiStrategy", 
    "LlmSentimentStrategy",
    "VolumeProfileStrategy"
]

[strategies.moving_average]
short_period = 10
long_period = 20
signal_threshold = 0.6

[strategies.rsi]
period = 14
oversold = 30
overbought = 70

[strategies.llm_sentiment]
sentiment_threshold = 0.5
min_sources = 5
confidence_threshold = 0.7

[risk]
# Aggressive HFT limits
max_order_value = 50_000.00
max_daily_volume = 500_000.00
max_position_size = 5_000
max_daily_loss_pct = 3.0

[execution]
# Fast execution settings
order_timeout_ms = 5000
retry_attempts = 3
retry_delay_ms = 100

[notifications]
channels = ["matrix"]
notify_on_fill = true
notify_on_risk_warning = true
```

---

## Order Execution Pipeline

```
┌─────────────────────────────────────────────────────────────────┐
│                    ORDER EXECUTION PIPELINE                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐ │
│  │  Signal  │───>│  Risk    │───>│  Order   │───>│  Broker  │ │
│  │  Source  │    │  Check   │    │  Router  │    │  API     │ │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘ │
│       │               │               │               │         │
│       v               v               v               v         │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐ │
│  │Strategy: │    │WalletGuard│    │Order Type│    │Alpaca/IB │ │
│  │• Technical│   │• Value   │    │• Market  │    │• Coinbase│ │
│  │• Sentiment│   │• Volume  │    │• Limit   │    │• Binance │ │
│  │• LLM     │    │• Position│    │• Stop    │    │• Kraken  │ │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘ │
│                                                                  │
│  TIMING TARGETS:                                                 │
│  Signal ──<1ms──> Risk ──<100µs──> Router ──<10ms──> Broker     │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Execution Flow

```rust
pub async fn execute_signal(
    signal: Signal,
    engine: &TradingEngine,
) -> Result<ExecutionResult, ExecutionError> {
    // 1. Convert signal to order
    let order = engine.signal_to_order(&signal)?;
    
    // 2. Pre-trade risk check (WalletGuard)
    engine.wallet_guard.check_order(&order)
        .map_err(|failures| ExecutionError::RiskRejected(failures))?;
    
    // 3. Route to appropriate broker
    let broker = engine.router.select_broker(&order.symbol);
    
    // 4. Submit order
    let result = broker.submit_order(order).await?;
    
    // 5. Notify
    engine.notify_order_submitted(&result).await;
    
    // 6. Track for circuit breaker
    engine.update_circuit_breaker(&result);
    
    Ok(result)
}
```

---

## Notification Templates

### Trade Alert Template

```json
{
  "event": "order_filled",
  "timestamp": "2024-01-15T14:30:00Z",
  "order": {
    "symbol": "AAPL",
    "side": "buy",
    "quantity": 100,
    "price": 150.25,
    "total_value": 15025.00
  },
  "signal": {
    "strategy": "LlmSentimentStrategy",
    "confidence": 0.85,
    "sentiment": 0.72
  },
  "risk": {
    "daily_pnl": 1250.00,
    "daily_pnl_pct": 0.125,
    "remaining_daily_limit": 98750.00
  }
}
```

### Circuit Breaker Alert

```json
{
  "event": "circuit_breaker_triggered",
  "timestamp": "2024-01-15T14:45:00Z",
  "reason": "consecutive_losses_exceeded",
  "details": {
    "consecutive_losses": 5,
    "total_loss": 5000.00,
    "total_loss_pct": 0.5
  },
  "action": {
    "pause_duration_mins": 30,
    "resume_time": "2024-01-15T15:15:00Z"
  }
}
```

---

## Integration with Clawdius Profiles

The trading profile integrates with Clawdius's multi-purpose platform:

### Profile Switching

```bash
# Switch to trading profile
clawdius profile load trading_hft

# Current profile
clawdius profile show

# List available profiles
clawdius profile list
```

### Profile Configuration

```toml
# ~/.config/clawdius/config.toml

[profiles]
active = "trading_hft"

[profiles.coding]
path = "config/profiles/coding.toml"
description = "AI coding assistant"

[profiles.assistant]
path = "config/profiles/assistant.toml"
description = "General AI assistant"

[profiles.trading_hft]
path = "config/profiles/trading_hft.toml"
description = "HFT with LLM sentiment"

[profiles.server]
path = "config/profiles/server.toml"
description = "LLM proxy server"
```

---

## Security Considerations

### API Key Management

```bash
# Environment variables (recommended)
export ALPACA_API_KEY="your_key"
export ALPACA_API_SECRET="your_secret"
export TWITTER_BEARER_TOKEN="your_token"

# Or use Clawdius secret management
clawdius secrets set alpaca_api_key
clawdius secrets set alpaca_api_secret
```

### Risk Management Rules

1. **Never exceed SEC 15c3-5 limits** - WalletGuard enforces pre-trade checks
2. **Circuit breakers mandatory** - Pause trading after consecutive losses
3. **Restricted symbols** - Block problematic symbols (PENN, GME, etc.)
4. **Daily loss caps** - Automatic halt at configured loss threshold
5. **Manual confirmation** - Require human approval for large orders

---

## Future Enhancements

| Feature | Priority | ETA |
|---------|----------|-----|
| Broker connectors (Alpaca, IB) | High | v2.0.0 |
| LLM sentiment integration | High | v2.0.0 |
| Paper trading mode | High | v2.0.0 |
| Backtesting engine | Medium | v2.1.0 |
| Multi-asset support (crypto) | Medium | v2.1.0 |
| Options strategies | Low | v2.2.0 |
| Portfolio optimization | Low | v2.2.0 |
| Real-time P&L dashboard | Medium | v2.1.0 |

---

## References

- [SEC Rule 15c3-5](https://www.sec.gov/rules/final/2010/34-63241.pdf) - Market Access Rule
- [Alpaca Trading API](https://alpaca.markets/docs/api-references/trading-api/)
- [Interactive Brokers API](https://www.interactivebrokers.com/en/trading/ib-api.php)
- [Ring Buffer Design](https://ferrous-systems.com/blog/lock-free-ring-buffer/)
