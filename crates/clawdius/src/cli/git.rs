use super::{load_config, GitCommands};

use std::path::PathBuf;

pub(super) async fn handle_git(
    action: GitCommands,
    config_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    match action {
        GitCommands::Commit { files, message } => {
            handle_git_commit(files, message, config_path).await
        },
        GitCommands::Diff { staged, file } => handle_git_diff(staged, file.as_deref()),
        GitCommands::Status => handle_git_status(),
    }
}

pub(super) async fn handle_git_commit(
    files: Vec<String>,
    message: Option<String>,
    config_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    use std::process::Command;

    let cwd = std::env::current_dir()?;

    let files_to_stage: Vec<&str> = if files.is_empty() {
        vec!["-A"]
    } else {
        files.iter().map(std::string::String::as_str).collect()
    };

    let add_output = Command::new("git")
        .args(["add"])
        .args(&files_to_stage)
        .current_dir(&cwd)
        .output();

    let add_output = match add_output {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!("git not found on PATH. Please install git.");
        },
        Err(e) => anyhow::bail!("Failed to run git add: {e}"),
    };

    if !add_output.status.success() {
        let stderr = String::from_utf8_lossy(&add_output.stderr);
        anyhow::bail!("git add failed: {stderr}");
    }

    let commit_message = if let Some(msg) = message {
        msg
    } else {
        let diff_output = Command::new("git")
            .args(["diff", "--cached"])
            .current_dir(&cwd)
            .output();

        let diff_output = match diff_output {
            Ok(output) => output,
            Err(e) => anyhow::bail!("Failed to run git diff --cached: {e}"),
        };

        let diff_text = String::from_utf8_lossy(&diff_output.stdout).to_string();

        if diff_text.trim().is_empty() {
            anyhow::bail!(
                "No staged changes to commit. Stage files first or provide files as arguments."
            );
        }

        match generate_commit_message(&diff_text, config_path.as_ref()).await {
            Ok(msg) => msg,
            Err(_) => {
                anyhow::bail!("No LLM configured and no --message provided. Please provide a commit message with -m.");
            },
        }
    };

    let commit_output = Command::new("git")
        .args(["commit", "-m", &commit_message])
        .current_dir(&cwd)
        .output();

    let commit_output = match commit_output {
        Ok(output) => output,
        Err(e) => anyhow::bail!("Failed to run git commit: {e}"),
    };

    if commit_output.status.success() {
        let stdout = String::from_utf8_lossy(&commit_output.stdout);
        println!("Committed successfully:");
        for line in stdout.lines() {
            println!("  {line}");
        }
    } else {
        let stderr = String::from_utf8_lossy(&commit_output.stderr);
        anyhow::bail!("git commit failed: {stderr}");
    }

    Ok(())
}

async fn generate_commit_message(
    diff: &str,
    config_path: Option<&PathBuf>,
) -> anyhow::Result<String> {
    use clawdius_core::llm::providers::LlmClient;
    use clawdius_core::llm::{create_provider, ChatMessage, ChatRole, LlmConfig};

    let config = load_config(config_path.map(PathBuf::as_path))?;
    let provider_name = config
        .llm
        .default_provider
        .clone()
        .unwrap_or_else(|| "anthropic".to_string());
    let llm_config = LlmConfig::from_config(&config.llm, &provider_name)?;
    let llm_client =
        create_provider(&llm_config).map_err(|_| anyhow::anyhow!("Failed to create LLM client"))?;

    let prompt = format!(
        "Generate a concise conventional commit message for these changes:\n```\n{diff}\n```\nRules:\n- Use conventional commit format (feat/fix/refactor/docs/test/chore)\n- First line <=72 chars\n- No quotes around the message\n- Output ONLY the commit message, nothing else"
    );

    let messages = vec![ChatMessage {
        role: ChatRole::User,
        content: prompt,
    }];

    let response = llm_client.chat(messages).await?;
    let mut msg = response
        .lines()
        .next()
        .unwrap_or(&response)
        .trim()
        .to_string();

    msg = msg.trim_matches('"').trim_matches('\'').to_string();
    if let Some(first_newline) = msg.find('\n') {
        msg.truncate(first_newline);
    }

    Ok(msg)
}

pub(super) fn handle_git_diff(staged: bool, file: Option<&str>) -> anyhow::Result<()> {
    use std::process::Command;

    let cwd = std::env::current_dir()?;

    let mut args = vec!["diff".to_string()];
    if staged {
        args.push("--cached".to_string());
    }
    if let Some(f) = file {
        args.push("--".to_string());
        args.push(f.to_string());
    }

    let output = Command::new("git").args(&args).current_dir(&cwd).output();

    let output = match output {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!("git not found on PATH. Please install git.");
        },
        Err(e) => anyhow::bail!("Failed to run git diff: {e}"),
    };

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.is_empty() {
            if staged {
                println!("No staged changes.");
            } else {
                println!("No unstaged changes.");
            }
        } else {
            print!("{stdout}");
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git diff failed: {stderr}");
    }

    Ok(())
}

pub(super) fn handle_git_status() -> anyhow::Result<()> {
    use std::process::Command;

    let cwd = std::env::current_dir()?;

    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&cwd)
        .output();

    let output = match output {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!("git not found on PATH. Please install git.");
        },
        Err(e) => anyhow::bail!("Failed to run git status: {e}"),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git status failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    if lines.is_empty() {
        println!("Clean working tree.");
        return Ok(());
    }

    let mut modified = 0usize;
    let mut added = 0usize;
    let mut deleted = 0usize;
    let mut untracked = 0usize;
    let mut renamed = 0usize;
    let mut copied = 0usize;
    let mut other = 0usize;

    for line in &lines {
        if line.len() < 2 {
            continue;
        }
        let index_status = line.chars().next().unwrap_or(' ');
        let worktree_status = line.chars().nth(1).unwrap_or(' ');

        match (index_status, worktree_status) {
            ('?', _) => untracked += 1,
            ('A', _) | (_, 'A') => added += 1,
            ('D', _) | (_, 'D') => deleted += 1,
            ('R', _) | (_, 'R') => renamed += 1,
            ('C', _) | (_, 'C') => copied += 1,
            ('M', _) | (_, 'M') => modified += 1,
            _ => other += 1,
        }
    }

    println!("Summary:");
    if modified > 0 {
        println!("  Modified:  {modified}");
    }
    if added > 0 {
        println!("  Added:     {added}");
    }
    if deleted > 0 {
        println!("  Deleted:   {deleted}");
    }
    if untracked > 0 {
        println!("  Untracked: {untracked}");
    }
    if renamed > 0 {
        println!("  Renamed:   {renamed}");
    }
    if copied > 0 {
        println!("  Copied:    {copied}");
    }
    if other > 0 {
        println!("  Other:     {other}");
    }
    println!();
    println!("Total: {} file(s)", lines.len());

    Ok(())
}

#[cfg(test)]
mod tests {
    struct StatusCounts {
        modified: usize,
        added: usize,
        deleted: usize,
        untracked: usize,
        renamed: usize,
        copied: usize,
        other: usize,
    }

    fn parse_porcelain(lines: &[&str]) -> StatusCounts {
        let mut c = StatusCounts {
            modified: 0,
            added: 0,
            deleted: 0,
            untracked: 0,
            renamed: 0,
            copied: 0,
            other: 0,
        };
        for line in lines {
            if line.len() < 2 {
                continue;
            }
            let idx = line.chars().next().unwrap_or(' ');
            let wt = line.chars().nth(1).unwrap_or(' ');
            match (idx, wt) {
                ('?', _) => c.untracked += 1,
                ('A', _) | (_, 'A') => c.added += 1,
                ('D', _) | (_, 'D') => c.deleted += 1,
                ('R', _) | (_, 'R') => c.renamed += 1,
                ('C', _) | (_, 'C') => c.copied += 1,
                ('M', _) | (_, 'M') => c.modified += 1,
                _ => c.other += 1,
            }
        }
        c
    }

    #[test]
    fn test_parse_untracked_files() {
        let c = parse_porcelain(&["?? newfile.txt", "??.gitignore"]);
        assert_eq!(c.untracked, 2);
        assert_eq!(c.modified, 0);
    }

    #[test]
    fn test_parse_modified_files() {
        let c = parse_porcelain(&[" M src/main.rs", "M  src/lib.rs", "MM src/both.rs"]);
        assert_eq!(c.modified, 3);
    }

    #[test]
    fn test_parse_added_files() {
        let c = parse_porcelain(&["A  new.rs", " A staged_new.rs"]);
        assert_eq!(c.added, 2);
    }

    #[test]
    fn test_parse_deleted_files() {
        let c = parse_porcelain(&[" D gone.rs", "D  staged_gone.rs"]);
        assert_eq!(c.deleted, 2);
    }

    #[test]
    fn test_parse_renamed_files() {
        let c = parse_porcelain(&["R  old.rs -> new.rs"]);
        assert_eq!(c.renamed, 1);
    }

    #[test]
    fn test_parse_mixed_status() {
        let c = parse_porcelain(&[
            " M modified.rs",
            "A  added.rs",
            "?? untracked.rs",
            "D  deleted.rs",
            "R  renamed.rs -> new_name.rs",
        ]);
        assert_eq!(c.modified, 1);
        assert_eq!(c.added, 1);
        assert_eq!(c.untracked, 1);
        assert_eq!(c.deleted, 1);
        assert_eq!(c.renamed, 1);
    }

    #[test]
    fn test_parse_empty_input() {
        let c = parse_porcelain(&[]);
        assert_eq!(c.modified, 0);
        assert_eq!(c.untracked, 0);
    }

    #[test]
    fn test_parse_short_lines_skipped() {
        let c = parse_porcelain(&["X"]);
        assert_eq!(c.other, 0);
    }
}
