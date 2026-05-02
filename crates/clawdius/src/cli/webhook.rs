use super::{OutputFormat, WebhookCommands};

use std::path::{Path, PathBuf};

pub(super) async fn handle_webhook(
    action: WebhookCommands,
    _config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::webhooks::{DeliveryStatus, WebhookConfig, WebhookEvent, WebhookManager};

    let manager = WebhookManager::new();

    match action {
        WebhookCommands::List => {
            let webhooks = manager.list().await;

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&webhooks)?);
            } else if webhooks.is_empty() {
                println!("No webhooks registered");
            } else {
                println!("Registered webhooks:\n");
                for webhook in &webhooks {
                    let status = if webhook.active { "active" } else { "inactive" };
                    println!("  {} [{}] - {}", webhook.name, status, webhook.url);
                    println!("    ID: {}", webhook.id);
                    println!("    Events: {:?}", webhook.events);
                    println!();
                }
            }
        },

        WebhookCommands::Create {
            name,
            url,
            events,
            secret,
        } => {
            let mut config = WebhookConfig::new(&name, &url);

            if let Some(events_str) = events {
                let event_list: Vec<WebhookEvent> = events_str
                    .split(',')
                    .filter_map(|s| match s.trim() {
                        "session.created" => Some(WebhookEvent::SessionCreated),
                        "session.updated" => Some(WebhookEvent::SessionUpdated),
                        "session.deleted" => Some(WebhookEvent::SessionDeleted),
                        "message.sent" => Some(WebhookEvent::MessageSent),
                        "message.received" => Some(WebhookEvent::MessageReceived),
                        "tool.executed" => Some(WebhookEvent::ToolExecuted),
                        "file.changed" => Some(WebhookEvent::FileChanged),
                        "checkpoint.created" => Some(WebhookEvent::CheckpointCreated),
                        "checkpoint.restored" => Some(WebhookEvent::CheckpointRestored),
                        "workflow.started" => Some(WebhookEvent::WorkflowStarted),
                        "workflow.completed" => Some(WebhookEvent::WorkflowCompleted),
                        "workflow.failed" => Some(WebhookEvent::WorkflowFailed),
                        "task.started" => Some(WebhookEvent::TaskStarted),
                        "task.completed" => Some(WebhookEvent::TaskCompleted),
                        "task.failed" => Some(WebhookEvent::TaskFailed),
                        "code.generated" => Some(WebhookEvent::CodeGenerated),
                        "tests.generated" => Some(WebhookEvent::TestsGenerated),
                        "error.occurred" => Some(WebhookEvent::ErrorOccurred),
                        "*" | "all" => Some(WebhookEvent::All),
                        _ => None,
                    })
                    .collect();
                config = config.with_events(event_list);
            }

            if let Some(s) = secret {
                config = config.with_secret(&s);
            }

            let id = manager.register(config).await?;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": id.to_string(),
                        "name": name,
                        "url": url,
                        "status": "created"
                    })
                );
            } else {
                println!("✓ Webhook created: {name} ({id})");
            }
        },

        WebhookCommands::Show { id } => {
            use clawdius_core::webhooks::WebhookId;
            let webhook_id = WebhookId::new(&id);

            match manager.get(&webhook_id).await {
                Some(webhook) => {
                    if output_format == OutputFormat::Json {
                        println!("{}", serde_json::to_string_pretty(&webhook)?);
                    } else {
                        println!("Webhook: {}", webhook.name);
                        println!("ID: {}", webhook.id);
                        println!("URL: {}", webhook.url);
                        println!("Active: {}", webhook.active);
                        println!("Events: {:?}", webhook.events);
                        if webhook.secret.is_some() {
                            println!("Secret: configured");
                        }
                        println!("Timeout: {}s", webhook.timeout_secs);
                        println!("Max retries: {}", webhook.max_retries);
                    }
                },
                None => {
                    anyhow::bail!("Webhook not found: {id}");
                },
            }
        },

        WebhookCommands::Update {
            id,
            url,
            events,
            enable,
            disable,
        } => {
            use clawdius_core::webhooks::WebhookId;
            let webhook_id = WebhookId::new(&id);

            let Some(mut webhook) = manager.get(&webhook_id).await else {
                anyhow::bail!("Webhook not found: {id}");
            };

            if let Some(new_url) = url {
                webhook.url = new_url;
            }

            if let Some(events_str) = events {
                let event_list: Vec<WebhookEvent> = events_str
                    .split(',')
                    .filter_map(|s| match s.trim() {
                        "session.created" => Some(WebhookEvent::SessionCreated),
                        "session.updated" => Some(WebhookEvent::SessionUpdated),
                        "session.deleted" => Some(WebhookEvent::SessionDeleted),
                        "message.sent" => Some(WebhookEvent::MessageSent),
                        "message.received" => Some(WebhookEvent::MessageReceived),
                        "tool.executed" => Some(WebhookEvent::ToolExecuted),
                        "file.changed" => Some(WebhookEvent::FileChanged),
                        "*" | "all" => Some(WebhookEvent::All),
                        _ => None,
                    })
                    .collect();
                webhook.events = event_list;
            }

            if enable {
                webhook.active = true;
            }
            if disable {
                webhook.active = false;
            }

            manager.update(&webhook_id, webhook).await?;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": id,
                        "status": "updated"
                    })
                );
            } else {
                println!("✓ Webhook updated: {id}");
            }
        },

        WebhookCommands::Delete { id } => {
            use clawdius_core::webhooks::WebhookId;
            let webhook_id = WebhookId::new(&id);

            let deleted = manager.unregister(&webhook_id).await?;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": id,
                        "deleted": deleted
                    })
                );
            } else if deleted {
                println!("✓ Webhook deleted: {id}");
            } else {
                anyhow::bail!("Webhook not found: {id}");
            }
        },

        WebhookCommands::Test { id, event } => {
            let test_event = event
                .map_or(WebhookEvent::SessionCreated, |s| match s.as_str() {
                    "message.sent" => WebhookEvent::MessageSent,
                    "tool.executed" => WebhookEvent::ToolExecuted,
                    _ => WebhookEvent::SessionCreated,
                });

            let test_data = serde_json::json!({
                "test": true,
                "timestamp": chrono::Utc::now().to_rfc3339()
            });

            manager.trigger(test_event, test_data.clone()).await;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "webhook_id": id,
                        "event": test_event.as_str(),
                        "test_data": test_data,
                        "status": "triggered"
                    })
                );
            } else {
                println!("✓ Test webhook triggered: {id} ({test_event})");
            }
        },

        WebhookCommands::Deliveries { id, limit } => {
            use clawdius_core::webhooks::WebhookId;

            let webhook_id = id.as_ref().map(WebhookId::new);
            let deliveries = manager.get_deliveries(webhook_id.as_ref()).await;
            let recent: Vec<_> = deliveries.into_iter().rev().take(limit).collect();

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&recent)?);
            } else if recent.is_empty() {
                println!("No deliveries found");
            } else {
                println!("Recent deliveries:\n");
                for delivery in &recent {
                    let status_icon = match delivery.status {
                        DeliveryStatus::Success => "✓",
                        DeliveryStatus::Failed => "✗",
                        DeliveryStatus::Timeout => "⏱",
                        DeliveryStatus::Pending => "⏳",
                    };
                    println!(
                        "  {} {} - {:?} ({}ms)",
                        status_icon, delivery.delivery_id, delivery.status, delivery.duration_ms
                    );
                    println!("     Event: {:?}", delivery.event);
                    if let Some(ref error) = delivery.error {
                        println!("     Error: {error}");
                    }
                    println!();
                }
            }
        },

        WebhookCommands::Stats => {
            let stats = manager.get_stats().await;

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&stats)?);
            } else {
                println!("Webhook Statistics:\n");
                println!("  Total webhooks: {}", stats.total_webhooks);
                println!("  Active webhooks: {}", stats.active_webhooks);
                println!();
                println!("  Total deliveries: {}", stats.total_deliveries);
                println!("  Successful: {}", stats.successful_deliveries);
                println!("  Failed: {}", stats.failed_deliveries);
                println!("  Pending: {}", stats.pending_deliveries);
                println!("  Timeouts: {}", stats.timeout_deliveries);
            }
        },
    }

    Ok(())
}
