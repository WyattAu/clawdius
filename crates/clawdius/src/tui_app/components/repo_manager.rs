//! Fleet repo manager: all supervised repos, the central agent's routing
//! state, and the hard-gate review dashboard.
//!
//! Opening the TUI lands on this screen. It lists every repository the
//! fleet discovered under the workspace root (active and inactive), the
//! dev-cycle lane each repo sits in, and how many tasks/gate flags are
//! queued. Press `d` for the dashboard of hard gates skipped by policy that
//! are awaiting human review (`a` approve, `x` reject).

use std::path::{Path, PathBuf};

use ratatui::{
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use clawdius_core::fleet::{
    CyclePhase, DispatchDecision, FleetState, GateId, GatePolicy, ReviewQueue,
};

use crate::tui_app::theme::Theme;

/// Default task text used when continuing a repo's dev cycle from the TUI.
pub const CONTINUE_CYCLE_TASK: &str = "Continue the dev cycle: inspect the \
working tree, finish in-progress work, run the build and tests, fix what \
you can, and report finish.";

/// One rendered row: a repo plus its fleet state.
#[derive(Debug, Clone)]
pub struct FleetRow {
    /// Repo name.
    pub name: String,
    /// Repo path.
    pub path: String,
    /// Whether the central agent routes work here.
    pub active: bool,
    /// Current dev-cycle lane phase.
    pub lane: Option<CyclePhase>,
    /// Queued dev-cycle tasks.
    pub queued: usize,
    /// Hard-gate flags awaiting review.
    pub flagged: usize,
}

/// Fleet manager state for the TUI.
pub struct FleetManager {
    /// Repos + routing state (persistable).
    pub state: FleetState,
    /// Hard-gate review queue (the dashboard's data).
    pub review: ReviewQueue,
    /// Rendered rows, rebuilt from state.
    pub rows: Vec<FleetRow>,
    /// Selected row.
    pub selected: usize,
    /// Dashboard (review queue) view instead of the repo list.
    pub dashboard: bool,
    /// Root scanned for repos.
    pub scan_root: PathBuf,
    /// Status line shown under the list.
    pub status_line: String,
}

impl FleetManager {
    /// Build the manager for `root`, restoring persisted state when present.
    ///
    /// The default policy enables skip-on-finish: cycles that report finish
    /// skip hard gates and flag them for human review instead of blocking.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        let mut state = FleetState::new();
        state.gate_policy = GatePolicy {
            skip_on_finish: true,
            ..GatePolicy::default()
        };
        let mut review = ReviewQueue::new();
        let persist = root.join(".clawdius").join("fleet.json");
        if let Ok(file) = std::fs::File::open(&persist) {
            if let Ok(saved) = FleetState::load(file) {
                state = saved;
            } else {
                // Review queue lives beside the state file when written by
                // older versions; tolerate absence.
            }
        }
        if let Ok(file) = std::fs::File::open(root.join(".clawdius").join("fleet_review.json")) {
            if let Ok(saved) = serde_json::from_reader(file) {
                review = saved;
            }
        }
        let mut manager = Self {
            state,
            review,
            rows: Vec::new(),
            selected: 0,
            dashboard: false,
            scan_root: root.to_path_buf(),
            status_line: String::new(),
        };
        manager.scan();
        manager
    }

    /// Discover repos under the scan root and refresh rows.
    pub fn scan(&mut self) {
        self.state.registry.scan(&self.scan_root.clone());
        if self.state.registry.entries.is_empty() {
            self.state.registry.scan(std::path::Path::new(".."));
        }
        self.rebuild_rows();
        self.status_line = format!("scanned {} repo(s)", self.state.registry.entries.len());
    }

    /// Rebuild the rendered rows from registry/router/review state.
    pub fn rebuild_rows(&mut self) {
        self.rows = self
            .state
            .registry
            .entries
            .values()
            .map(|entry| FleetRow {
                name: entry.name.clone(),
                path: entry.path.clone(),
                active: entry.active,
                lane: self.state.router.lane(&entry.name),
                queued: self.state.router.queue_len(&entry.name),
                flagged: self
                    .review
                    .pending()
                    .iter()
                    .filter(|r| r.repo == entry.name)
                    .count(),
            })
            .collect();
        if self.selected >= self.rows.len() {
            self.selected = self.rows.len().saturating_sub(1);
        }
    }

    /// Move selection up.
    pub const fn up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Move selection down.
    pub const fn move_down(&mut self) {
        let last = self.rows.len().saturating_sub(1);
        if self.selected < last {
            self.selected += 1;
        }
    }

    /// The selected repo's name.
    #[must_use]
    pub fn selected_repo(&self) -> Option<&str> {
        self.rows.get(self.selected).map(|r| r.name.as_str())
    }

    /// Toggle the selected repo between active and inactive.
    pub fn toggle_active(&mut self) {
        let Some(name) = self.selected_repo().map(ToString::to_string) else {
            return;
        };
        let current = self.state.registry.get(&name).is_some_and(|e| e.active);
        if self.state.registry.set_active(&name, !current) {
            self.status_line = format!("{name}: {}", if current { "inactive" } else { "active" });
        }
        self.rebuild_rows();
    }

    /// Queue a continue-the-dev-cycle task for the selected repo.
    pub fn enqueue_continue(&mut self) {
        let Some(name) = self.selected_repo().map(ToString::to_string) else {
            return;
        };
        self.state.router.enqueue(&name, CONTINUE_CYCLE_TASK);
        self.status_line = format!("{name}: cycle queued");
        self.rebuild_rows();
    }

    /// Dispatch the selected repo's next queued task, if its lane is free.
    #[must_use]
    pub fn dispatch_selected(&mut self) -> Option<DispatchDecision> {
        let name = self.selected_repo()?.to_string();
        let active = vec![name];
        let decision = self.state.router.dispatch_next(&active);
        self.rebuild_rows();
        decision
    }

    /// Hand the selected repo's lane to the agent.
    pub fn mark_running(&mut self) {
        let Some(name) = self.selected_repo().map(ToString::to_string) else {
            return;
        };
        self.state.router.mark_running(&name);
        self.rebuild_rows();
    }

    /// Report finish for a repo: apply the gate policy and flag any skipped
    /// hard gates into the review queue.
    pub fn report_finish(&mut self, repo: &str) {
        let decision = self.state.router.report_finish(
            repo,
            &[],
            "finish reported by agent; gates skipped by fleet policy",
            &self.state.gate_policy,
            &mut self.review,
        );
        self.status_line = match decision {
            Some(clawdius_core::fleet::GateDecision::SkippedAndFlagged(records))
                if !records.is_empty() =>
            {
                format!(
                    "{repo}: finished; {} gate(s) flagged for review",
                    records.len()
                )
            },
            Some(_) => format!("{repo}: finished; gates clean"),
            None => format!("{repo}: no active cycle to finish"),
        };
        self.persist();
        self.rebuild_rows();
    }

    /// Report an execution failure for a repo; its cycle is requeued.
    pub fn report_failure(&mut self, repo: &str) {
        self.state.router.report_failure(repo);
        self.status_line = format!("{repo}: cycle failed; requeued");
        self.rebuild_rows();
    }

    /// Switch to the review dashboard.
    pub const fn open_dashboard(&mut self) {
        self.dashboard = true;
    }

    /// Back to the repo list.
    pub const fn close_dashboard(&mut self) {
        self.dashboard = false;
    }

    /// Approve the next pending flag for the selected repo.
    pub fn approve_selected_flag(&mut self) {
        self.resolve_flag(true);
    }

    /// Reject the next pending flag for the selected repo.
    pub fn reject_selected_flag(&mut self) {
        self.resolve_flag(false);
    }

    fn resolve_flag(&mut self, approved: bool) {
        let Some(name) = self.selected_repo().map(ToString::to_string) else {
            return;
        };
        let resolved = GateId::all().iter().any(|gate| {
            self.review
                .pending()
                .iter()
                .any(|r| r.repo == name && r.gate == *gate)
                && self.review.resolve(&name, *gate, approved)
        });
        self.status_line = if resolved {
            format!(
                "{name}: gate {}",
                if approved { "approved" } else { "rejected" }
            )
        } else {
            format!("{name}: no pending gate flag")
        };
        self.rebuild_rows();
    }

    /// Persist registry, router, gate policy, and review queue.
    pub fn persist(&self) {
        let dir = self.scan_root.join(".clawdius");
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(file) = std::fs::File::create(dir.join("fleet.json")) {
            let _ = self.state.save(file);
        }
        if let Ok(file) = std::fs::File::create(dir.join("fleet_review.json")) {
            let _ = serde_json::to_writer_pretty(file, &self.review);
        }
    }

    /// Count of gates awaiting review across the fleet.
    #[must_use]
    pub fn pending_flags(&self) -> usize {
        self.review.pending().len()
    }
}

/// Fleet repo-manager widget.
pub struct FleetManagerWidget<'a> {
    manager: &'a FleetManager,
    theme: &'a Theme,
}

impl<'a> FleetManagerWidget<'a> {
    /// Render the manager (repo list, or review dashboard when toggled).
    pub const fn new(manager: &'a FleetManager, theme: &'a Theme) -> Self {
        Self { manager, theme }
    }

    const fn lane_label(lane: Option<CyclePhase>) -> &'static str {
        match lane {
            None | Some(CyclePhase::Queued | CyclePhase::Requeued) => "queued",
            Some(CyclePhase::Dispatched) => "dispatched",
            Some(CyclePhase::Running) => "running",
            Some(CyclePhase::FinishReported) => "finishing",
            Some(CyclePhase::Completed) => "done",
            Some(CyclePhase::GatesSkipped) => "done (gates skipped)",
        }
    }
}

impl Widget for FleetManagerWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        Clear.render(area, buf);
        if self.manager.dashboard {
            self.render_dashboard(area, buf);
        } else {
            self.render_repos(area, buf);
        }
    }
}

impl FleetManagerWidget<'_> {
    fn render_repos(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let block = Block::default()
            .title(" Fleet — repos, dev cycles, routing ")
            .borders(Borders::ALL)
            .border_style(self.theme.border_focus());
        let inner_w = area.width.saturating_sub(2) as usize;
        let content_height = area.height.saturating_sub(2) as usize;

        let mut lines: Vec<Line<'static>> = Vec::new();
        let footer = format!(
            "[j/k] move  [s] toggle active  [r] queue cycle  [Enter] dispatch+run  [d] gate dashboard  [S] rescan  [q] chat  |  {}",
            self.manager.status_line
        );
        let rows_visible = content_height.saturating_sub(2); // header + footer

        lines.push(Line::from(Span::styled(
            format!(
                "  {:<22} {:<9} {:>3} {:>4}  {}",
                "REPO", "LANE", "Q", "FLAG", "PATH"
            ),
            self.theme.title().add_modifier(Modifier::BOLD),
        )));

        if self.manager.rows.is_empty() {
            lines.push(Line::from(Span::styled(
                "  no repos discovered — press S to rescan",
                self.theme.muted(),
            )));
        }

        let scroll = self
            .manager
            .selected
            .saturating_sub(rows_visible.saturating_sub(1));
        for (i, row) in self
            .manager
            .rows
            .iter()
            .enumerate()
            .skip(scroll)
            .take(rows_visible)
        {
            let selected = i == self.manager.selected;
            let marker = if row.active { "●" } else { "○" };
            let flag = if row.flagged > 0 {
                format!("⚠{}", row.flagged)
            } else {
                String::new()
            };
            let path_w = inner_w.saturating_sub(46);
            let path = truncate_tail(&row.path, path_w);
            let name = truncate(&row.name, 20);
            let base = if selected {
                self.theme.file_selected().add_modifier(Modifier::BOLD)
            } else if row.active {
                self.theme.file_item()
            } else {
                self.theme.muted()
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {marker} "),
                    if row.active {
                        self.theme.user_message()
                    } else {
                        self.theme.muted()
                    },
                ),
                Span::styled(format!("{name:<21}"), base),
                Span::styled(
                    format!("{:<9}", Self::lane_label(row.lane)),
                    self.theme.title(),
                ),
                Span::styled(format!("{:>3} ", row.queued), base),
                Span::styled(
                    format!("{flag:>4}  "),
                    if row.flagged > 0 {
                        self.theme.error()
                    } else {
                        self.theme.muted()
                    },
                ),
                Span::styled(path, self.theme.muted()),
            ]));
        }

        while lines.len() < content_height {
            lines.push(Line::from(""));
        }
        lines.truncate(content_height.max(1));
        lines.push(Line::from(Span::styled(footer, self.theme.muted())));
        Paragraph::new(lines).block(block).render(area, buf);
    }

    fn render_dashboard(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let pending = self.manager.review.pending();
        let block = Block::default()
            .title(format!(
                " Hard-gate review dashboard — {} awaiting human review ",
                pending.len()
            ))
            .borders(Borders::ALL)
            .border_style(if pending.is_empty() {
                self.theme.border_focus()
            } else {
                self.theme.error()
            });
        let content_height = area.height.saturating_sub(2) as usize;
        let mut lines: Vec<Line<'static>> = Vec::new();

        if pending.is_empty() {
            lines.push(Line::from(Span::styled(
                "  no hard gates awaiting review",
                self.theme.muted(),
            )));
        }
        for record in &pending {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  ⚠ [{}] ", record.repo),
                    self.theme.error().add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{} ", record.gate.label()), self.theme.title()),
                Span::styled(
                    format!("skipped at {}", record.flagged_at.format("%Y-%m-%d %H:%M")),
                    self.theme.muted(),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                format!("      reason: {}", record.reason),
                self.theme.muted(),
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  [a] approve selected repo's flag   [x] reject   [d/Esc] back to repos",
            self.theme.muted(),
        )));
        lines.truncate(content_height.max(1));
        Paragraph::new(lines).block(block).render(area, buf);
    }
}

/// Truncate keeping the tail, with an ellipsis marker.
fn truncate_tail(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    let tail: String = chars[chars.len() - (max - 1)..].iter().collect();
    format!("…{tail}")
}

/// Truncate a string to `max` chars, appending an ellipsis.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    format!(
        "{}…",
        s.chars().take(max.saturating_sub(1)).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use clawdius_core::fleet::GateStatus;

    fn tmp_root() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn scan_and_enqueue_flow() {
        let tmp = tmp_root();
        let repo = tmp.path().join("demo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let mut manager = FleetManager::new(tmp.path());
        assert_eq!(manager.rows.len(), 1);
        manager.selected = 0;
        manager.enqueue_continue();
        assert_eq!(manager.state.router.queue_len("demo"), 1);
        let decision = manager.dispatch_selected().unwrap();
        assert_eq!(decision.repo, "demo");
        assert_eq!(decision.task, CONTINUE_CYCLE_TASK);
        manager.mark_running();
        manager.report_finish("demo");
        assert!(manager.pending_flags() > 0, "skip policy flags gates");
    }

    #[test]
    fn toggle_active_survives_rebuild() {
        let tmp = tmp_root();
        let repo = tmp.path().join("demo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let mut manager = FleetManager::new(tmp.path());
        manager.toggle_active();
        assert!(!manager.rows[0].active);
        manager.rebuild_rows();
        assert!(!manager.rows[0].active);
    }

    #[test]
    fn dashboard_resolution_clears_flags() {
        let tmp = tmp_root();
        let repo = tmp.path().join("demo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let mut manager = FleetManager::new(tmp.path());
        manager.enqueue_continue();
        let _ = manager.dispatch_selected();
        manager.mark_running();
        manager.report_finish("demo");
        manager.open_dashboard();
        let before = manager.pending_flags();
        assert!(before > 0);
        manager.approve_selected_flag();
        assert!(manager.pending_flags() < before);
        assert!(manager
            .review
            .records
            .iter()
            .any(|r| r.status == GateStatus::Approved));
    }

    #[test]
    fn persistence_roundtrip() {
        let tmp = tmp_root();
        let repo = tmp.path().join("demo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        {
            let mut manager = FleetManager::new(tmp.path());
            manager.enqueue_continue();
            manager.persist();
        }
        let reloaded = FleetManager::new(tmp.path());
        assert_eq!(reloaded.state.router.queue_len("demo"), 1);
    }
}
