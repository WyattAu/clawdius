//! Alpaca Paper Trading Client
//!
//! Provides integration with Alpaca's paper trading API for simulated
//! order execution. Uses REST API with WebSocket for market data.

use crate::error::Error;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const ALPACA_PAPER_BASE_URL: &str = "https://paper-api.alpaca.markets";
const ALPACA_DATA_URL: &str = "https://data.alpaca.markets";

#[derive(Debug, Clone)]
pub struct AlpacaConfig {
    pub api_key: String,
    pub secret_key: String,
    pub base_url: String,
    pub data_url: String,
}

impl Default for AlpacaConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("ALPACA_API_KEY").unwrap_or_default(),
            secret_key: std::env::var("ALPACA_SECRET_KEY").unwrap_or_default(),
            base_url: ALPACA_PAPER_BASE_URL.to_string(),
            data_url: ALPACA_DATA_URL.to_string(),
        }
    }
}

impl AlpacaConfig {
    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty() && !self.secret_key.is_empty()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Account {
    pub id: String,
    pub cash: String,
    pub equity: String,
    pub buying_power: String,
    pub status: String,
    pub paper: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    #[serde(rename = "stop_limit")]
    StopLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimeInForce {
    Day,
    Gtc,
    Ioc,
    Fok,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrderRequest {
    pub symbol: String,
    pub qty: u32,
    pub side: OrderSide,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    #[serde(rename = "time_in_force")]
    pub time_in_force: TimeInForce,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<String>,
    #[serde(rename = "client_order_id", skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Order {
    pub id: String,
    pub client_order_id: Option<String>,
    pub symbol: String,
    pub qty: String,
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub status: String,
    pub filled_qty: Option<String>,
    pub filled_avg_price: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub qty: String,
    pub side: String,
    pub market_value: String,
    pub cost_basis: String,
    pub unrealized_pl: String,
    pub unrealized_plpc: String,
    pub current_price: String,
}

#[derive(Debug, Clone)]
pub struct AlpacaClient {
    config: AlpacaConfig,
    http: reqwest::Client,
}

impl AlpacaClient {
    pub fn new(config: AlpacaConfig) -> Result<Self, Error> {
        if !config.is_configured() {
            return Err(Error::Config(
                "Alpaca API key and secret required. Set ALPACA_API_KEY and ALPACA_SECRET_KEY."
                    .to_string(),
            ));
        }

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| Error::Other(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, http })
    }

    pub fn from_env() -> Result<Self, Error> {
        Self::new(AlpacaConfig::default())
    }

    pub async fn get_account(&self) -> Result<Account, Error> {
        let url = format!("{}/v2/account", self.config.base_url);
        let response = self
            .http
            .get(&url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.secret_key)
            .send()
            .await
            .map_err(|e| Error::Other(format!("Failed to get account: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Other(format!(
                "Alpaca API error {}: {}",
                status, body
            )));
        }

        response
            .json::<Account>()
            .await
            .map_err(|e| Error::Other(format!("Failed to parse account: {}", e)))
    }

    pub async fn submit_order(&self, request: OrderRequest) -> Result<Order, Error> {
        let url = format!("{}/v2/orders", self.config.base_url);
        let response = self
            .http
            .post(&url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.secret_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Other(format!("Failed to submit order: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Other(format!(
                "Alpaca order error {}: {}",
                status, body
            )));
        }

        response
            .json::<Order>()
            .await
            .map_err(|e| Error::Other(format!("Failed to parse order: {}", e)))
    }

    pub async fn get_positions(&self) -> Result<Vec<Position>, Error> {
        let url = format!("{}/v2/positions", self.config.base_url);
        let response = self
            .http
            .get(&url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.secret_key)
            .send()
            .await
            .map_err(|e| Error::Other(format!("Failed to get positions: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Other(format!(
                "Alpaca positions error {}: {}",
                status, body
            )));
        }

        response
            .json::<Vec<Position>>()
            .await
            .map_err(|e| Error::Other(format!("Failed to parse positions: {}", e)))
    }

    pub async fn cancel_all_orders(&self) -> Result<(), Error> {
        let url = format!("{}/v2/orders", self.config.base_url);
        let response = self
            .http
            .delete(&url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.secret_key)
            .send()
            .await
            .map_err(|e| Error::Other(format!("Failed to cancel orders: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Other(format!(
                "Alpaca cancel error {}: {}",
                status, body
            )));
        }

        Ok(())
    }

    pub async fn get_order(&self, order_id: &str) -> Result<Order, Error> {
        let url = format!("{}/v2/orders/{}", self.config.base_url, order_id);
        let response = self
            .http
            .get(&url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.secret_key)
            .send()
            .await
            .map_err(|e| Error::Other(format!("Failed to get order: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Other(format!(
                "Alpaca get order error {}: {}",
                status, body
            )));
        }

        response
            .json::<Order>()
            .await
            .map_err(|e| Error::Other(format!("Failed to parse order: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env_missing() {
        let config = AlpacaConfig {
            api_key: String::new(),
            secret_key: String::new(),
            ..Default::default()
        };
        assert!(!config.is_configured());
    }

    #[test]
    fn test_config_from_env_present() {
        let config = AlpacaConfig {
            api_key: "test-key".to_string(),
            secret_key: "test-secret".to_string(),
            ..Default::default()
        };
        assert!(config.is_configured());
    }

    #[test]
    fn test_order_request_serialization() {
        let req = OrderRequest {
            symbol: "AAPL".to_string(),
            qty: 100,
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::Day,
            limit_price: None,
            stop_price: None,
            client_order_id: Some("test-client-123".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("AAPL"));
        assert!(json.contains("100"));
        assert!(json.contains("market"));
    }

    #[test]
    fn test_paper_base_url() {
        assert_eq!(ALPACA_PAPER_BASE_URL, "https://paper-api.alpaca.markets");
    }

    #[tokio::test]
    async fn test_client_creation_fails_without_config() {
        let config = AlpacaConfig::default();
        if std::env::var("ALPACA_API_KEY").is_err() {
            assert!(AlpacaClient::new(config).is_err());
        }
    }
}
