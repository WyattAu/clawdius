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
    }

    Ok(())
}
