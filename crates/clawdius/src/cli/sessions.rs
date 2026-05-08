use super::{load_config, OutputFormat};

use clawdius_core::output::SessionInfo;
use clawdius_core::output::{OutputFormat as CoreOutputFormat, OutputFormatter, OutputOptions};
use clawdius_core::SessionManager;
use std::path::PathBuf;

pub(super) fn handle_sessions(
    delete: Option<&str>,
    search: Option<&str>,
    config_path: Option<&PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    let config = load_config(config_path.map(PathBuf::as_path))?;
    let session_manager = SessionManager::new(&config)?;

    if let Some(session_id) = delete {
        use std::str::FromStr;
        let id = clawdius_core::session::SessionId::from_str(session_id)?;
        session_manager.delete_session(&id)?;
        println!("✓ Deleted session: {session_id}");
        return Ok(());
    }

    if let Some(query) = search {
        let results = session_manager.search_messages(query)?;
        println!("Search results for '{query}':");
        for (session_id, msg) in results {
            let preview = msg.as_text().map_or_else(
                || "[non-text]".to_string(),
                |t| {
                    if t.len() > 50 {
                        format!("{}...", &t[..50])
                    } else {
                        t.to_string()
                    }
                },
            );
            println!("  {session_id} > {preview}");
        }
        return Ok(());
    }

    let sessions = session_manager.list_sessions()?;

    let session_infos: Vec<SessionInfo> = sessions
        .iter()
        .map(|session| SessionInfo {
            id: session.id.to_string(),
            title: session.title.clone(),
            message_count: session.messages.len(),
            tokens: session.token_usage.total(),
        })
        .collect();

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: false,
        quiet: false,
        include_metadata: true,
    };
    let formatter = OutputFormatter::new(options);

    use std::io::{self};
    formatter.format_session_list(&mut io::stdout(), &session_infos)?;

    Ok(())
}
