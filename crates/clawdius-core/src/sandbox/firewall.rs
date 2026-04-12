//! Egress Traffic Filtering
//!
//! Provides firewall rule generation for sandbox environments to prevent
//! unauthorized outbound connections. Generates iptables/nftables rules
//! that block internal IPs, restrict outbound ports, and rate-limit connections.
//!
//! # Architecture
//!
//! ```text
//!   Sandboxed Process
//!         │
//!         ▼
//!   iptables/nftables OUTPUT chain
//!         │
//!    ┌────┼────────────┐
//!    │    │            │
//!    ▼    ▼            ▼
//!  Block   Allow       Rate-limit
//!  10/8   80,443       outbound
//!  192.168 DNS,HTTPS   connections
//!  172.16  (HTTP/S)
//!  127.0
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use clawdius_core::sandbox::firewall::{EgressConfig, Firewall};
//!
//! let config = EgressConfig::saas_defaults();
//! let firewall = Firewall::new(config);
//!
//! // Generate iptables rules
//! let rules = firewall.iptables_rules("clawdius-sess-123");
//! for rule in &rules {
//!     println!("{}", rule);
//! }
//! ```
//!
//! # Platform Support
//!
//! - **Linux**: Full iptables/nftables support
//! - **macOS**: Configuration only (no native firewall)
//! - **Other**: No-op (rules generated but not enforced)

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::IpAddr;
use std::process::Command;

/// Default blocked private IP ranges (RFC 1918 + loopback + link-local).
const BLOCKED_CIDRS: &[&str] = &[
    "10.0.0.0/8",     // RFC 1918 - Class A private
    "172.16.0.0/12",  // RFC 1918 - Class B private
    "192.168.0.0/16", // RFC 1918 - Class C private
    "127.0.0.0/8",    // Loopback
    "169.254.0.0/16", // Link-local
    "0.0.0.0/8",      // Current network
    "224.0.0.0/4",    // Multicast
    "240.0.0.0/4",    // Reserved
    "::1/128",        // IPv6 loopback
    "fc00::/7",       // IPv6 unique local
    "fe80::/10",      // IPv6 link-local
];

/// Default blocked ports (services that should never be reachable from sandbox).
const BLOCKED_PORTS: &[u16] = &[
    22,    // SSH
    23,    // Telnet
    25,    // SMTP
    445,   // SMB
    1433,  // MSSQL
    3306,  // MySQL
    5432,  // PostgreSQL
    6379,  // Redis
    27017, // MongoDB
    9200,  // Elasticsearch
];

/// Default allowed ports (essential for web scraping and LLM API calls).
const ALLOWED_PORTS: &[u16] = &[
    80,  // HTTP
    443, // HTTPS
    53,  // DNS
];

/// Default rate limit: max connections per minute.
const DEFAULT_RATE_LIMIT_PER_MINUTE: u32 = 60;

/// Default rate limit burst.
const DEFAULT_RATE_LIMIT_BURST: u32 = 20;

/// Configuration for egress traffic filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EgressConfig {
    /// Blocked CIDR ranges (private/internal networks)
    pub blocked_cidrs: Vec<String>,
    /// Explicitly blocked ports
    pub blocked_ports: Vec<u16>,
    /// Explicitly allowed ports
    pub allowed_ports: Vec<u16>,
    /// Rate limit: max connections per minute (0 = no limit)
    pub rate_limit_per_minute: u32,
    /// Rate limit burst size
    pub rate_limit_burst: u32,
    /// Whether to allow all outbound DNS (port 53 UDP/TCP)
    pub allow_dns: bool,
    /// Whether to block all outbound traffic (override everything)
    pub block_all: bool,
    /// Custom allowed IP addresses (whitelist)
    pub allowed_ips: Vec<String>,
    /// Custom blocked IP addresses
    pub blocked_ips: Vec<String>,
}

impl Default for EgressConfig {
    fn default() -> Self {
        Self {
            blocked_cidrs: BLOCKED_CIDRS.iter().map(|s| s.to_string()).collect(),
            blocked_ports: BLOCKED_PORTS.to_vec(),
            allowed_ports: ALLOWED_PORTS.to_vec(),
            rate_limit_per_minute: DEFAULT_RATE_LIMIT_PER_MINUTE,
            rate_limit_burst: DEFAULT_RATE_LIMIT_BURST,
            allow_dns: true,
            block_all: false,
            allowed_ips: Vec::new(),
            blocked_ips: Vec::new(),
        }
    }
}

impl EgressConfig {
    /// Creates a configuration suitable for SaaS multi-tenant deployment.
    ///
    /// Blocks private IPs, restricts to HTTP/HTTPS/DNS, rate-limits outbound.
    #[must_use]
    pub fn saas_defaults() -> Self {
        Self::default()
    }

    /// Creates a configuration that blocks all outbound traffic.
    ///
    /// Used for Hardened sandbox tier.
    #[must_use]
    pub fn block_all_traffic() -> Self {
        Self {
            block_all: true,
            ..Default::default()
        }
    }

    /// Creates a permissive configuration (only blocks internal IPs).
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            blocked_ports: vec![],
            allowed_ports: vec![],
            rate_limit_per_minute: 0,
            rate_limit_burst: 0,
            ..Default::default()
        }
    }

    /// Adds a custom blocked port.
    #[must_use]
    pub fn block_port(mut self, port: u16) -> Self {
        self.blocked_ports.push(port);
        self
    }

    /// Adds a custom allowed port.
    #[must_use]
    pub fn allow_port(mut self, port: u16) -> Self {
        self.allowed_ports.push(port);
        self
    }

    /// Sets the rate limit.
    #[must_use]
    pub fn with_rate_limit(mut self, per_minute: u32, burst: u32) -> Self {
        self.rate_limit_per_minute = per_minute;
        self.rate_limit_burst = burst;
        self
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    pub fn validate(&self) -> Result<()> {
        // Check for port conflicts
        let allowed_set: HashSet<_> = self.allowed_ports.iter().collect();
        let blocked_set: HashSet<_> = self.blocked_ports.iter().collect();
        let conflicts: Vec<_> = allowed_set.intersection(&blocked_set).collect();

        if !conflicts.is_empty() {
            return Err(crate::error::Error::Config(format!(
                "Port conflict: ports {:?} are both allowed and blocked",
                conflicts
            )));
        }

        // Parse CIDRs (basic validation)
        for cidr in &self.blocked_cidrs {
            let parts: Vec<&str> = cidr.split('/').collect();
            if parts.len() != 2 {
                return Err(crate::error::Error::Config(format!(
                    "Invalid CIDR (expected ADDR/PREFIX): {}",
                    cidr
                )));
            }
            if parts[0].parse::<IpAddr>().is_err() {
                return Err(crate::error::Error::Config(format!(
                    "Invalid IP in CIDR: {}",
                    cidr
                )));
            }
        }

        Ok(())
    }
}

/// Firewall rule generator and executor.
pub struct Firewall {
    /// Configuration
    config: EgressConfig,
}

impl Firewall {
    /// Creates a new firewall with the given configuration.
    #[must_use]
    pub fn new(config: EgressConfig) -> Self {
        Self { config }
    }

    /// Generates iptables rules for the given chain suffix.
    ///
    /// Returns a list of iptables commands that should be executed as root.
    /// Each rule is a complete iptables command string.
    pub fn iptables_rules(&self, chain_suffix: &str) -> Vec<String> {
        let chain = format!("CLAWDIUS-{}", chain_suffix.to_uppercase());
        let mut rules = Vec::new();

        if self.config.block_all {
            // Block all outbound traffic
            rules.push(format!("iptables -N {} 2>/dev/null || true", chain));
            rules.push(format!("iptables -A {} -j DROP", chain));
            return rules;
        }

        // Create chain
        rules.push(format!("iptables -N {} 2>/dev/null || true", chain));

        // Block private/internal CIDRs
        for cidr in &self.config.blocked_cidrs {
            rules.push(format!(
                "iptables -A {} -d {} -j DROP -m comment --comment 'block-private'",
                chain, cidr
            ));
        }

        // Block specific IPs
        for ip in &self.config.blocked_ips {
            rules.push(format!(
                "iptables -A {} -d {} -j DROP -m comment --comment 'block-ip'",
                chain, ip
            ));
        }

        // Block specific ports
        for port in &self.config.blocked_ports {
            rules.push(format!(
                "iptables -A {} -p tcp --dport {} -j DROP -m comment --comment 'block-port'",
                chain, port
            ));
        }

        // Allow DNS if configured
        if self.config.allow_dns {
            rules.push(format!(
                "iptables -A {} -p udp --dport 53 -j ACCEPT -m comment --comment 'allow-dns-udp'",
                chain
            ));
            rules.push(format!(
                "iptables -A {} -p tcp --dport 53 -j ACCEPT -m comment --comment 'allow-dns-tcp'",
                chain
            ));
        }

        // Allow specific ports
        for port in &self.config.allowed_ports {
            rules.push(format!(
                "iptables -A {} -p tcp --dport {} -j ACCEPT -m comment --comment 'allow-port'",
                chain, port
            ));
        }

        // Allow specific IPs (whitelist)
        for ip in &self.config.allowed_ips {
            rules.push(format!(
                "iptables -A {} -d {} -j ACCEPT -m comment --comment 'allow-ip'",
                chain, ip
            ));
        }

        // Rate limiting (if configured)
        if self.config.rate_limit_per_minute > 0 {
            rules.push(format!(
                "iptables -A {} -m connlimit --connlimit-above {} -j REJECT --reject-with tcp-reset -m comment --comment 'rate-limit'",
                chain, self.config.rate_limit_burst
            ));
        }

        // Default deny
        rules.push(format!(
            "iptables -A {} -j DROP -m comment --comment 'default-deny'",
            chain
        ));

        rules
    }

    /// Generates nftables rules (preferred over iptables on modern Linux).
    pub fn nftables_rules(&self, table_name: &str) -> String {
        let mut rules = String::new();

        rules.push_str(&format!("table inet {} {{\n", table_name));
        rules.push_str("  chain output {\n");
        rules.push_str("    type filter hook output priority 0; policy drop;\n\n");

        if self.config.block_all {
            rules.push_str("  }\n}\n");
            return rules;
        }

        // Allow DNS
        if self.config.allow_dns {
            rules.push_str("    udp dport 53 accept\n");
            rules.push_str("    tcp dport 53 accept\n\n");
        }

        // Block private CIDRs
        for cidr in &self.config.blocked_cidrs {
            rules.push_str(&format!("    ip daddr {} drop\n", cidr));
        }

        // Block specific ports
        for port in &self.config.blocked_ports {
            rules.push_str(&format!("    tcp dport {} drop\n", port));
        }

        // Allow specific ports
        for port in &self.config.allowed_ports {
            rules.push_str(&format!("    tcp dport {} accept\n", port));
        }

        // Allow specific IPs
        for ip in &self.config.allowed_ips {
            rules.push_str(&format!("    ip daddr {} accept\n", ip));
        }

        rules.push_str("  }\n}\n");
        rules
    }

    /// Installs the iptables rules (requires root).
    ///
    /// # Safety
    ///
    /// This executes system commands as root. Only call from trusted code paths.
    ///
    /// # Errors
    ///
    /// Returns an error if any rule fails to install.
    pub fn install_iptables(&self, chain_suffix: &str) -> Result<Vec<String>> {
        let rules = self.iptables_rules(chain_suffix);
        let mut applied = Vec::new();

        for rule in &rules {
            // Skip chain creation (may already exist)
            if rule.contains("2>/dev/null") {
                let _ = Command::new("sh").arg("-c").arg(rule).output();
                applied.push(rule.clone());
                continue;
            }

            let output = Command::new("sh")
                .arg("-c")
                .arg(rule)
                .output()
                .map_err(|e| {
                    crate::error::Error::Internal(format!("Failed to execute iptables: {}", e))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(crate::error::Error::Internal(format!(
                    "iptables rule failed: {}\n{}",
                    rule, stderr
                )));
            }

            applied.push(rule.clone());
        }

        Ok(applied)
    }

    /// Removes all iptables rules for the given chain.
    ///
    /// # Errors
    ///
    /// Returns an error if flush fails.
    pub fn uninstall_iptables(&self, chain_suffix: &str) -> Result<()> {
        let chain = format!("CLAWDIUS-{}", chain_suffix.to_uppercase());

        // Flush and delete chain
        let _ = Command::new("iptables").args(["-F", &chain]).output();

        let _ = Command::new("iptables").args(["-X", &chain]).output();

        Ok(())
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &EgressConfig {
        &self.config
    }
}

/// Validates an IP address string.
///
/// # Errors
///
/// Returns an error if the IP is invalid.
pub fn validate_ip(ip: &str) -> Result<()> {
    ip.parse::<IpAddr>()
        .map_err(|e| crate::error::Error::Config(format!("Invalid IP '{}': {}", ip, e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = EgressConfig::default();
        assert_eq!(config.blocked_cidrs.len(), 11); // 8 IPv4 + 3 IPv6
        assert!(!config.blocked_ports.is_empty());
        assert!(!config.allowed_ports.is_empty());
        assert!(config.allow_dns);
        assert!(!config.block_all);
    }

    #[test]
    fn test_saas_defaults() {
        let config = EgressConfig::saas_defaults();
        assert_eq!(config.rate_limit_per_minute, 60);
        assert!(config.blocked_ports.contains(&22)); // SSH
        assert!(config.blocked_ports.contains(&25)); // SMTP
        assert!(config.allowed_ports.contains(&443)); // HTTPS
    }

    #[test]
    fn test_block_all() {
        let config = EgressConfig::block_all_traffic();
        assert!(config.block_all);
    }

    #[test]
    fn test_permissive() {
        let config = EgressConfig::permissive();
        assert!(config.blocked_ports.is_empty());
        assert!(config.allowed_ports.is_empty());
        assert_eq!(config.rate_limit_per_minute, 0);
    }

    #[test]
    fn test_builder() {
        let config = EgressConfig::default()
            .block_port(8080)
            .allow_port(8443)
            .with_rate_limit(100, 30);

        assert!(config.blocked_ports.contains(&8080));
        assert!(config.allowed_ports.contains(&8443));
        assert_eq!(config.rate_limit_per_minute, 100);
        assert_eq!(config.rate_limit_burst, 30);
    }

    #[test]
    fn test_validate_no_conflicts() {
        let config = EgressConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_port_conflict() {
        let config = EgressConfig {
            blocked_ports: vec![443],
            allowed_ports: vec![443],
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_iptables_rules_saaS() {
        let firewall = Firewall::new(EgressConfig::saas_defaults());
        let rules = firewall.iptables_rules("sess-123");

        assert!(!rules.is_empty());

        // Should have chain creation, CIDR blocks, port blocks, port allows, default deny
        let rule_str = rules.join("\n");
        assert!(rule_str.contains("CLAWDIUS-SESS-123"));
        assert!(rule_str.contains("10.0.0.0/8"));
        assert!(rule_str.contains("192.168.0.0/16"));
        assert!(rule_str.contains("block-port"));
        assert!(rule_str.contains("allow-port"));
        assert!(rule_str.contains("default-deny"));
        assert!(rule_str.contains("allow-dns"));
    }

    #[test]
    fn test_iptables_rules_block_all() {
        let firewall = Firewall::new(EgressConfig::block_all_traffic());
        let rules = firewall.iptables_rules("test");

        // Should only have chain creation and DROP
        assert!(rules.len() <= 2);
        let rule_str = rules.join("\n");
        assert!(rule_str.contains("DROP"));
    }

    #[test]
    fn test_nftables_rules() {
        let firewall = Firewall::new(EgressConfig::saas_defaults());
        let rules = firewall.nftables_rules("clawdius");

        assert!(rules.contains("table inet clawdius"));
        assert!(rules.contains("chain output"));
        assert!(rules.contains("policy drop"));
        assert!(rules.contains("tcp dport 53 accept"));
    }

    #[test]
    fn test_validate_ip() {
        assert!(validate_ip("8.8.8.8").is_ok());
        assert!(validate_ip("::1").is_ok());
        assert!(validate_ip("not-an-ip").is_err());
    }
}
