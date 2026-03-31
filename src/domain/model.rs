use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPaths {
    pub root: PathBuf,
    pub config_path: PathBuf,
    pub state_path: PathBuf,
    pub operations_log_path: PathBuf,
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
    pub attention: WorkstreamAttention,
    #[serde(default)]
    pub pinned: bool,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkstreamRequest {
    New { branch: String, base_branch: String },
    Existing { branch: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum WorkstreamAttention {
    #[default]
    None,
    ReviewNext,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkstreamStatus {
    Running,
    #[serde(alias = "Archived", alias = "archived")]
    Stopped,
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
