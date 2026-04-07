use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPaths {
    pub root: PathBuf,
    pub config_path: PathBuf,
    pub state_path: PathBuf,
    pub operations_log_path: PathBuf,
    pub task_results_path: PathBuf,
    pub worktrees_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub github_repo: Option<String>,
    pub agent_presets: Vec<AgentPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPreset {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub one_off_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoTarget {
    pub id: Uuid,
    pub path: PathBuf,
    pub display_name: String,
    #[serde(default)]
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredState {
    pub version: u32,
    pub repos: Vec<RepoTarget>,
    pub workstreams: Vec<Workstream>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workstream {
    pub id: Uuid,
    pub repo_id: Uuid,
    pub base_branch: String,
    pub branch: String,
    pub worktree_path: PathBuf,
    #[serde(default)]
    pub worktree_ownership: WorktreeOwnership,
    pub session_name: String,
    pub agent_preset: String,
    pub status: WorkstreamStatus,
    #[serde(default)]
    pub created_at_epoch_secs: u64,
    #[serde(default)]
    pub status_changed_at_epoch_secs: u64,
    #[serde(default)]
    pub last_opened_at_epoch_secs: Option<u64>,
    #[serde(default)]
    pub last_attached_at_epoch_secs: Option<u64>,
    #[serde(default)]
    pub sessions: Vec<WorkstreamSession>,
    #[serde(default)]
    pub preferred_session_id: Option<Uuid>,
    #[serde(default)]
    pub attention_events: Vec<AttentionEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkstreamSession {
    pub id: Uuid,
    pub session_name: String,
    pub agent_preset: String,
    pub status: WorkstreamStatus,
    #[serde(default)]
    pub created_at_epoch_secs: u64,
    #[serde(default)]
    pub status_changed_at_epoch_secs: u64,
    #[serde(default)]
    pub last_opened_at_epoch_secs: Option<u64>,
    #[serde(default)]
    pub last_attached_at_epoch_secs: Option<u64>,
    #[serde(default)]
    pub last_output_at_epoch_secs: Option<u64>,
    #[serde(default)]
    pub last_output_digest: Option<String>,
    #[serde(default)]
    pub last_idle_alert_at_epoch_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkstreamRequest {
    New { branch: String, base_branch: String },
    Existing { branch: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkstreamStatus {
    Running,
    #[serde(alias = "Archived", alias = "archived")]
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AttentionReason {
    TaskFailed,
    OneOffCompleted,
    SessionStopped,
    BranchChanged,
    IdleOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionEvent {
    pub reason: AttentionReason,
    pub summary: String,
    #[serde(default)]
    pub detail: Option<String>,
    pub created_at_epoch_secs: u64,
    #[serde(default)]
    pub seen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorktreeCheckout {
    NewBranch { base_ref: String },
    ExistingBranch { start_ref: Option<String> },
    ExistingCheckout,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum WorktreeOwnership {
    #[default]
    Managed,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreePlan {
    pub branch: String,
    pub worktree_path: PathBuf,
    pub worktree_ownership: WorktreeOwnership,
    pub checkout: WorktreeCheckout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationEvent {
    pub timestamp_epoch_secs: u64,
    pub level: OperationLevel,
    pub scope: String,
    pub action: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationLevel {
    Started,
    Succeeded,
    Failed,
    Warning,
    Info,
}

impl Workstream {
    pub fn ensure_session_inventory(&mut self) {
        if self.sessions.is_empty() {
            self.sessions.push(WorkstreamSession {
                id: Uuid::new_v4(),
                session_name: self.session_name.clone(),
                agent_preset: self.agent_preset.clone(),
                status: self.status.clone(),
                created_at_epoch_secs: self.created_at_epoch_secs,
                status_changed_at_epoch_secs: self.status_changed_at_epoch_secs,
                last_opened_at_epoch_secs: self.last_opened_at_epoch_secs,
                last_attached_at_epoch_secs: self.last_attached_at_epoch_secs,
                last_output_at_epoch_secs: None,
                last_output_digest: None,
                last_idle_alert_at_epoch_secs: None,
            });
        }

        if self.preferred_session_id.is_none() {
            self.preferred_session_id = self.sessions.first().map(|session| session.id);
        }

        self.sync_legacy_summary();
    }

    pub fn preferred_session(&self) -> Option<&WorkstreamSession> {
        self.preferred_session_id
            .and_then(|session_id| {
                self.sessions
                    .iter()
                    .find(|session| session.id == session_id)
            })
            .or_else(|| self.sessions.first())
    }

    pub fn preferred_session_mut(&mut self) -> Option<&mut WorkstreamSession> {
        let session_id = self
            .preferred_session_id
            .or_else(|| self.sessions.first().map(|session| session.id))?;
        self.sessions
            .iter_mut()
            .find(|session| session.id == session_id)
    }

    pub fn session_by_id(&self, session_id: Uuid) -> Option<&WorkstreamSession> {
        self.sessions
            .iter()
            .find(|session| session.id == session_id)
    }

    pub fn session_by_id_mut(&mut self, session_id: Uuid) -> Option<&mut WorkstreamSession> {
        self.sessions
            .iter_mut()
            .find(|session| session.id == session_id)
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn running_session_count(&self) -> usize {
        self.sessions
            .iter()
            .filter(|session| matches!(session.status, WorkstreamStatus::Running))
            .count()
    }

    pub fn latest_activity_epoch_secs(&self) -> u64 {
        self.sessions
            .iter()
            .flat_map(|session| {
                session
                    .last_attached_at_epoch_secs
                    .into_iter()
                    .chain(session.last_opened_at_epoch_secs)
            })
            .max()
            .unwrap_or(self.created_at_epoch_secs)
    }

    pub fn add_session(&mut self, session: WorkstreamSession, prefer: bool) {
        let session_id = session.id;
        self.sessions.push(session);
        if prefer || self.preferred_session_id.is_none() {
            self.preferred_session_id = Some(session_id);
        }
        self.sync_legacy_summary();
    }

    pub fn unread_attention_count(&self) -> usize {
        self.attention_events
            .iter()
            .filter(|event| !event.seen)
            .count()
    }

    pub fn has_unread_attention(&self) -> bool {
        self.attention_events.iter().any(|event| !event.seen)
    }

    pub fn latest_attention_event(&self) -> Option<&AttentionEvent> {
        self.attention_events
            .iter()
            .max_by_key(|event| event.created_at_epoch_secs)
    }

    pub fn latest_unread_attention_event(&self) -> Option<&AttentionEvent> {
        self.attention_events
            .iter()
            .filter(|event| !event.seen)
            .max_by_key(|event| event.created_at_epoch_secs)
    }

    pub fn push_attention_event(
        &mut self,
        reason: AttentionReason,
        summary: String,
        detail: Option<String>,
        seen: bool,
    ) -> bool {
        let created_at_epoch_secs = now_epoch_secs();
        let duplicate = self.attention_events.iter().any(|event| {
            event.reason == reason
                && event.summary == summary
                && event.detail == detail
                && !event.seen
        });
        if duplicate {
            return false;
        }

        self.attention_events.push(AttentionEvent {
            reason,
            summary,
            detail,
            created_at_epoch_secs,
            seen,
        });
        if self.attention_events.len() > 12 {
            let overflow = self.attention_events.len() - 12;
            self.attention_events.drain(0..overflow);
        }
        true
    }

    pub fn mark_attention_seen(&mut self) -> bool {
        let mut changed = false;
        for event in &mut self.attention_events {
            if !event.seen {
                event.seen = true;
                changed = true;
            }
        }
        changed
    }

    pub fn mark_attention_reason_seen(&mut self, reason: AttentionReason) -> bool {
        let mut changed = false;
        for event in &mut self.attention_events {
            if event.reason == reason && !event.seen {
                event.seen = true;
                changed = true;
            }
        }
        changed
    }

    pub fn sync_legacy_summary(&mut self) {
        if let Some(session) = self.preferred_session().cloned() {
            self.session_name = session.session_name;
            self.agent_preset = session.agent_preset;
            self.last_opened_at_epoch_secs = self
                .sessions
                .iter()
                .filter_map(|candidate| candidate.last_opened_at_epoch_secs)
                .max();
            self.last_attached_at_epoch_secs = self
                .sessions
                .iter()
                .filter_map(|candidate| candidate.last_attached_at_epoch_secs)
                .max();
        }

        self.status = if self
            .sessions
            .iter()
            .any(|session| matches!(session.status, WorkstreamStatus::Running))
        {
            WorkstreamStatus::Running
        } else {
            WorkstreamStatus::Stopped
        };

        self.status_changed_at_epoch_secs = self
            .sessions
            .iter()
            .map(|session| session.status_changed_at_epoch_secs)
            .max()
            .unwrap_or(self.status_changed_at_epoch_secs);
    }
}

fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyStatus {
    pub name: String,
    pub found: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub paths: AppPaths,
    pub dependencies: Vec<DependencyStatus>,
}
