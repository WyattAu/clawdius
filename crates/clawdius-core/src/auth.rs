//! Authentication: OAuth flows for subscription-plan providers.
//!
//! Phase 2 groundwork of `docs/provider-oauth-design.md` (in the
//! clawdius repo). Token storage lives in [`crate::config::KeyringStorage`]
//! (`oauth/<provider>` namespacing); the interactive browser step is the
//! CLI's job (`clawdius auth login <provider>` — Phase 2 CLI surface).

pub mod oauth;

pub use oauth::{
    anthropic_endpoints, authorize_url, base64_url_safe, exchange_code, openai_endpoints,
    token_set_from_response, OAuthEndpoints, Pkce,
};
