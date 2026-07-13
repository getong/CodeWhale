use ratatui::layout::Rect;

use crate::localization::MessageId;
use crate::tools::subagent::{AgentWorkerStatus, SubAgentStatus};
use crate::tools::todo::{TodoItem, TodoStatus};
use crate::tui::app::{App, SidebarRowAction, TaskPanelEntry, TaskPanelEntryKind};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkRowId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WorkTone {
    Heading,
    Live,
    Attention,
    Success,
    Muted,
    Worker,
}

#[derive(Debug, Clone)]
pub(super) struct WorkRow {
    pub id: WorkRowId,
    pub mark: &'static str,
    pub label: String,
    pub detail: String,
    pub tone: WorkTone,
    pub selectable: bool,
    pub primary_action: Option<SidebarRowAction>,
    pub stop_action: Option<SidebarRowAction>,
}

#[derive(Debug, Clone)]
pub(super) struct WorkHitbox {
    pub id: WorkRowId,
    pub row_y: u16,
    pub stop_zone_start_col: Option<u16>,
    pub stop_zone_end_col: Option<u16>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkSurfaceState {
    pub focused: bool,
    pub selected: Option<WorkRowId>,
    pub scroll_offset: usize,
    pub last_area: Option<Rect>,
    pub visible_rows: usize,
    pub total_rows: usize,
    pub(super) hovered: Option<WorkRowId>,
    pub(super) hitboxes: Vec<WorkHitbox>,
    pub(super) cached_todos: Vec<TodoItem>,
    pub(super) latest_rows: Vec<WorkRow>,
}

impl WorkSurfaceState {
    pub(super) fn selected_index(&self, rows: &[WorkRow]) -> Option<usize> {
        self.selected
            .as_ref()
            .and_then(|selected| rows.iter().position(|row| &row.id == selected))
    }

    pub(super) fn clamp_selection(&mut self, rows: &[WorkRow]) {
        let selectable = rows.iter().filter(|row| row.selectable).collect::<Vec<_>>();
        if selectable.is_empty() {
            self.selected = None;
            self.focused = false;
            self.scroll_offset = 0;
            return;
        }
        if !selectable
            .iter()
            .any(|row| Some(&row.id) == self.selected.as_ref())
        {
            self.selected = Some(selectable[0].id.clone());
        }
        let selected = self.selected_index(rows).unwrap_or_default();
        if selected < self.scroll_offset {
            self.scroll_offset = selected;
        } else if self.visible_rows > 0
            && selected >= self.scroll_offset.saturating_add(self.visible_rows)
        {
            self.scroll_offset = selected.saturating_add(1).saturating_sub(self.visible_rows);
        }
        self.scroll_offset = self
            .scroll_offset
            .min(rows.len().saturating_sub(self.visible_rows.max(1)));
    }
}

pub(super) fn project(app: &mut App) -> Vec<WorkRow> {
    if let Ok(todos) = app.todos.try_lock() {
        app.work_surface.cached_todos = todos.snapshot().items;
    }

    let tasks = app
        .task_panel
        .iter()
        .filter(|task| task.kind == TaskPanelEntryKind::Background)
        .cloned()
        .collect::<Vec<_>>();
    let attention_hold = tasks
        .iter()
        .any(|task| matches!(task.status.as_str(), "waiting" | "needs_user"));
    let todos = app.work_surface.cached_todos.clone();
    let mut workers = app
        .subagent_cache
        .iter()
        .map(|agent| {
            let name = agent
                .nickname
                .clone()
                .or_else(|| app.agent_label_map.get(&agent.agent_id).cloned())
                .unwrap_or_else(|| agent.name.clone());
            let status = agent
                .worker_status
                .map(worker_status)
                .unwrap_or_else(|| subagent_status(&agent.status));
            let active = worker_is_active(agent.worker_status, &agent.status);
            WorkRow {
                id: WorkRowId(format!("worker:{}", agent.agent_id)),
                mark: worker_mark(status),
                label: format!("{name} · {}", agent.agent_type.as_str()),
                detail: format!("{} · {}", agent.assignment.objective, agent.model),
                tone: match status {
                    "waiting" | "failed" | "canceled" | "interrupted" => WorkTone::Attention,
                    "done" => WorkTone::Success,
                    _ => WorkTone::Worker,
                },
                selectable: true,
                primary_action: Some(SidebarRowAction::OpenAgentDetail {
                    agent_id: agent.agent_id.clone(),
                }),
                stop_action: active.then(|| {
                    SidebarRowAction::PrefillCommand(format!("/agent cancel {}", agent.agent_id))
                }),
            }
        })
        .collect::<Vec<_>>();
    let cached_worker_ids = app
        .subagent_cache
        .iter()
        .map(|agent| agent.agent_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    workers.extend(
        app.agent_progress
            .iter()
            .filter(|(agent_id, _)| !cached_worker_ids.contains(agent_id.as_str()))
            .map(|(agent_id, progress)| {
                let status = if progress.to_ascii_lowercase().contains("waiting") {
                    "waiting"
                } else {
                    "running"
                };
                WorkRow {
                    id: WorkRowId(format!("worker:{agent_id}")),
                    mark: worker_mark(status),
                    label: app
                        .agent_label_map
                        .get(agent_id)
                        .cloned()
                        .unwrap_or_else(|| agent_id.clone()),
                    detail: progress.clone(),
                    tone: if status == "waiting" {
                        WorkTone::Attention
                    } else {
                        WorkTone::Worker
                    },
                    selectable: true,
                    primary_action: Some(SidebarRowAction::OpenAgentDetail {
                        agent_id: agent_id.clone(),
                    }),
                    stop_action: Some(SidebarRowAction::PrefillCommand(format!(
                        "/agent cancel {agent_id}"
                    ))),
                }
            }),
    );

    let mut rows = Vec::new();
    if !tasks.is_empty() {
        let label = app.tr(MessageId::SidebarTasksLabel).into_owned();
        rows.push(section("tasks", &label, tasks.len()));
        rows.extend(tasks.iter().map(|task| task_row(task, attention_hold)));
    }
    if !todos.is_empty() {
        let completed = todos
            .iter()
            .filter(|item| item.status == TodoStatus::Completed)
            .count();
        let label = app.tr(MessageId::SidebarTodoLabel).into_owned();
        rows.push(section(
            "todo",
            &format!("{label} {completed}/{}", todos.len()),
            todos.len(),
        ));
        rows.extend(todos.into_iter().map(todo_row));
    }
    if !workers.is_empty() {
        let label = app.tr(MessageId::FleetRosterWorkers).into_owned();
        rows.push(section("workers", &label, workers.len()));
        rows.extend(workers);
    }

    app.work_surface.latest_rows = rows.clone();
    rows
}

fn section(id: &str, label: &str, count: usize) -> WorkRow {
    WorkRow {
        id: WorkRowId(format!("section:{id}")),
        mark: "▾",
        label: if label.chars().any(char::is_numeric) {
            label.to_string()
        } else {
            format!("{label} {count}")
        },
        detail: label.to_string(),
        tone: WorkTone::Heading,
        selectable: false,
        primary_action: None,
        stop_action: None,
    }
}

fn task_row(task: &TaskPanelEntry, attention_hold: bool) -> WorkRow {
    let namespace = if task.id.starts_with("shell_") {
        "jobs"
    } else {
        "task"
    };
    let open = format!("/{namespace} show {}", task.id);
    let stoppable = matches!(
        task.status.as_str(),
        "running" | "queued" | "waiting" | "needs_user"
    );
    let (mark, tone) = match task.status.as_str() {
        "running" if attention_hold => ("·", WorkTone::Muted),
        "running" => ("›", WorkTone::Live),
        "waiting" | "needs_user" => ("◆", WorkTone::Attention),
        "completed" | "success" => ("✓", WorkTone::Success),
        "failed" | "canceled" => ("✕", WorkTone::Attention),
        _ => ("☐", WorkTone::Muted),
    };
    WorkRow {
        id: WorkRowId(format!("task:{}", task.id)),
        mark,
        label: task.prompt_summary.clone(),
        detail: task.id.clone(),
        tone,
        selectable: true,
        primary_action: Some(SidebarRowAction::Command(open)),
        stop_action: stoppable
            .then(|| SidebarRowAction::PrefillCommand(format!("/{namespace} cancel {}", task.id))),
    }
}

fn todo_row(item: TodoItem) -> WorkRow {
    let (mark, tone) = match item.status {
        TodoStatus::Completed => ("✓", WorkTone::Success),
        TodoStatus::InProgress => ("▸", WorkTone::Live),
        TodoStatus::Pending => ("☐", WorkTone::Muted),
    };
    WorkRow {
        id: WorkRowId(format!("todo:{}", item.id)),
        mark,
        label: item.content.clone(),
        detail: format!("#{}", item.id),
        tone,
        selectable: true,
        primary_action: Some(SidebarRowAction::InspectText {
            label: item.content,
            detail: format!("#{}", item.id),
        }),
        stop_action: None,
    }
}

fn worker_is_active(status: Option<AgentWorkerStatus>, legacy: &SubAgentStatus) -> bool {
    status.map_or_else(
        || matches!(legacy, SubAgentStatus::Running),
        |status| {
            !matches!(
                status,
                AgentWorkerStatus::Completed
                    | AgentWorkerStatus::Failed
                    | AgentWorkerStatus::Cancelled
                    | AgentWorkerStatus::Interrupted
            )
        },
    )
}

fn worker_status(status: AgentWorkerStatus) -> &'static str {
    match status {
        AgentWorkerStatus::Queued => "queued",
        AgentWorkerStatus::Starting => "starting",
        AgentWorkerStatus::Running => "running",
        AgentWorkerStatus::WaitingForUser => "waiting",
        AgentWorkerStatus::ModelWait => "model wait",
        AgentWorkerStatus::RunningTool => "tool",
        AgentWorkerStatus::Completed => "done",
        AgentWorkerStatus::Failed => "failed",
        AgentWorkerStatus::Cancelled => "canceled",
        AgentWorkerStatus::Interrupted => "interrupted",
    }
}

fn subagent_status(status: &SubAgentStatus) -> &'static str {
    match status {
        SubAgentStatus::Running => "running",
        SubAgentStatus::Completed => "done",
        SubAgentStatus::Interrupted(_) => "interrupted",
        SubAgentStatus::Failed(_) => "failed",
        SubAgentStatus::Cancelled => "canceled",
        SubAgentStatus::BudgetExhausted => "budget",
    }
}

fn worker_mark(status: &str) -> &'static str {
    match status {
        "waiting" => "◆",
        "done" => "✓",
        "failed" | "canceled" | "interrupted" => "✕",
        "queued" => "☐",
        _ => "›",
    }
}
