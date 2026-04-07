use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::app::background::PersistedTaskCompletion;
use crate::domain::model::{
    AppConfig, DoctorReport, OperationEvent, RepoTarget, StoredState, WorktreePlan,
};

pub trait RuntimeStore: ConfigStore + StateStore {}

impl<T> RuntimeStore for T where T: ConfigStore + StateStore {}

#[derive(Debug, Clone)]
pub struct SessionCreateRequest {
    pub session_name: String,
    pub working_dir: PathBuf,
    pub startup_command: String,
    pub label: SessionLabel,
}

#[derive(Debug, Clone)]
pub struct SessionLabel {
    pub repo_name: String,
    pub workstream_branch: String,
    pub agent_preset: String,
    pub session_ordinal: usize,
}

#[derive(Debug, Clone)]
pub struct AgentOneOffRequest {
    pub preset_name: String,
    pub command_template: String,
    pub prompt: String,
    pub working_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct AgentOneOffResult {
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct NotificationRequest {
    pub title: String,
    pub body: String,
}

pub trait ConfigStore {
    fn load_or_create_config(&self, default: AppConfig) -> anyhow::Result<AppConfig>;
    fn save_config(&self, config: &AppConfig) -> anyhow::Result<()>;
}

pub trait StateStore {
    fn load_or_create_state(&self, default: StoredState) -> anyhow::Result<StoredState>;
    fn save_state(&self, state: &StoredState) -> anyhow::Result<()>;
}

pub trait DependencyInspector {
    fn doctor(&self) -> anyhow::Result<DoctorReport>;
}

pub trait RepoService {
    fn discover_repo(&self, path: &Path, display_name: Option<&str>) -> anyhow::Result<RepoTarget>;
    fn current_branch(&self, path: &Path) -> anyhow::Result<Option<String>>;
    fn list_branches(&self, repo: &RepoTarget) -> anyhow::Result<Vec<String>>;
    fn default_remote(&self, repo: &RepoTarget) -> anyhow::Result<String>;
    fn fetch_latest(&self, repo: &RepoTarget) -> anyhow::Result<()>;
    fn plan_new_worktree(
        &self,
        repo: &RepoTarget,
        worktrees_root: &Path,
        branch: &str,
        base_branch: &str,
    ) -> anyhow::Result<WorktreePlan>;
    fn plan_existing_worktree(
        &self,
        repo: &RepoTarget,
        worktrees_root: &Path,
        branch: &str,
    ) -> anyhow::Result<WorktreePlan>;
    fn create_worktree(&self, repo: &RepoTarget, plan: &WorktreePlan) -> anyhow::Result<()>;
    fn configure_push_target(
        &self,
        worktree_path: &Path,
        branch: &str,
        remote: &str,
    ) -> anyhow::Result<()>;
    fn remove_worktree(&self, repo: &RepoTarget, worktree_path: &Path) -> anyhow::Result<()>;
}

pub trait SessionBackend {
    fn create_session(&self, request: &SessionCreateRequest) -> anyhow::Result<()>;
    fn sync_session(&self, request: &SessionCreateRequest) -> anyhow::Result<()>;
    fn has_session(&self, session_name: &str) -> bool;
    fn attach(&self, session_name: &str) -> anyhow::Result<()>;
    fn stop(&self, session_name: &str) -> anyhow::Result<()>;
    fn attach_command(&self, session_name: &str) -> Vec<String>;
    fn capture_recent_lines(&self, session_name: &str, lines: usize)
    -> anyhow::Result<Vec<String>>;
}

pub trait TerminalFrontend {
    fn open_session(&self, session_name: &str, attach_command: &[String]) -> anyhow::Result<()>;
}

pub trait CommandRunner {
    fn run_agent_one_off(&self, request: &AgentOneOffRequest) -> anyhow::Result<AgentOneOffResult>;
}

pub trait OperationLog {
    fn record(&self, event: &OperationEvent) -> anyhow::Result<()>;
    fn recent(&self, limit: usize) -> anyhow::Result<Vec<OperationEvent>>;
}

pub trait Notifier {
    fn notify(&self, request: &NotificationRequest) -> anyhow::Result<()>;
}

pub trait TaskCompletionStore {
    fn load(&self) -> anyhow::Result<Vec<PersistedTaskCompletion>>;
    fn append(&self, completion: &PersistedTaskCompletion) -> anyhow::Result<()>;
    fn remove(&self, completion_id: Uuid) -> anyhow::Result<()>;
}

pub trait Updater {
    fn install_or_update(&self, github_repo: Option<&str>) -> anyhow::Result<()>;
}
