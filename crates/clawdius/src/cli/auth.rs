#![cfg(feature = "keyring")]

use super::AuthCommands;
use anyhow::Context;

pub(super) async fn handle_auth(action: AuthCommands) -> anyhow::Result<()> {
    use clawdius_core::config::KeyringStorage;
    use rpassword::read_password;
    use std::io::{self, Write};

    let storage = KeyringStorage::global();

    match action {
        AuthCommands::Set { provider } => {
            print!("Enter API key for {provider}: ");
            io::stdout().flush()?;

            let key = read_password()?;

            if key.is_empty() {
                anyhow::bail!("API key cannot be empty");
            }

            storage.set_api_key(&provider, &key)?;
            println!("✓ API key stored for {provider}");
        },
        AuthCommands::Get { provider } => match storage.get_api_key(&provider)? {
            Some(key) => {
                println!("API key for {}: {}***", provider, &key[..8.min(key.len())]);
            },
            None => {
                println!("No API key found for {provider}");
            },
        },
        AuthCommands::Delete { provider } => {
            storage.delete_api_key(&provider)?;
            println!("✓ API key deleted for {provider}");
        },
        AuthCommands::Status => {
            let providers = [
                "anthropic",
                "openai",
                "google",
                "deepseek",
                "zai",
                "openrouter",
                "opencode-go",
            ];
            println!("Auth state:");
            for provider in providers {
                let key = storage.get_api_key(provider)?.is_some();
                let oauth = storage.get_oauth_token(provider)?;
                let oauth_state = match oauth.as_deref() {
                    Some(json) => {
                        let expiry =
                            serde_json::from_str::<clawdius_core::config::OAuthTokenSet>(json)
                                .ok()
                                .and_then(|t| t.expires_at.clone())
                                .map(|e| format!(", expires {e}"))
                                .unwrap_or_default();
                        format!("oauth token set{expiry}")
                    },
                    None => "no oauth token".to_string(),
                };
                let key_state = if key { "api key set" } else { "no api key" };
                println!("  {provider}: {key_state}, {oauth_state}");
            }
        },
        AuthCommands::SetOAuth {
            provider,
            token_json,
        } => {
            let parsed: clawdius_core::config::OAuthTokenSet = serde_json::from_str(&token_json)
                .map_err(|e| anyhow::anyhow!("Invalid token JSON: {e}"))?;
            if parsed.access_token.is_empty() {
                anyhow::bail!("access_token cannot be empty");
            }
            storage.set_oauth_token(&provider, &token_json)?;
            println!("✓ OAuth token stored for {provider}");
        },
        AuthCommands::DeleteOAuth { provider } => {
            storage.delete_oauth_token(&provider)?;
            println!("✓ OAuth token deleted for {provider}");
        },
        AuthCommands::Login {
            provider,
            code,
            port,
        } => {
            use clawdius_core::auth::oauth;
            use std::sync::Arc;

            let endpoints = match provider.as_str() {
                "anthropic" => oauth::anthropic_endpoints(),
                "openai" => oauth::openai_endpoints(),
                other => anyhow::bail!(
                    "OAuth not available for '{other}' (supported: anthropic, openai)"
                ),
            };
            let port = port.unwrap_or(endpoints.redirect_port);
            let pkce = oauth::Pkce::generate()?;

            match code.as_deref() {
                // Headless: the user visited the URL elsewhere and pasted the code.
                Some(pasted) => {
                    let token = oauth::exchange_code(&endpoints, pasted, &pkce.verifier, port)
                        .await
                        .map_err(|e| anyhow::anyhow!("{e}"))?;
                    let json = serde_json::to_string(&token)?;
                    storage.set_oauth_token(&provider, &json)?;
                    println!("✓ OAuth token stored for {provider}");
                },
                // Browser flow: print URL, capture the loopback redirect.
                None => {
                    let url = oauth::authorize_url(&endpoints, &pkce, port);
                    println!("Open this URL to authorize {provider}:");
                    println!("  {url}");
                    println!("Waiting for callback on http://localhost:{port}/callback ...");
                    let params = clawdius_core::auth::redirect::capture_code(port, 300)
                        .await
                        .map_err(|e| anyhow::anyhow!("{e}"))?;
                    if let Some(err) = params.error {
                        anyhow::bail!("Authorization denied: {err}");
                    }
                    let token =
                        oauth::exchange_code(&endpoints, &params.code, &pkce.verifier, port)
                            .await
                            .map_err(|e| anyhow::anyhow!("{e}"))?;
                    let json = serde_json::to_string(&token)?;
                    storage.set_oauth_token(&provider, &json)?;
                    println!("✓ OAuth token stored for {provider}");
                },
            }
        },
    }

    Ok(())
}
