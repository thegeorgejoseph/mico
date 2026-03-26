use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPaths {
    pub root: PathBuf,
    pub config_path: PathBuf,
    pub state_path: PathBuf,
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
    pub session_name: String,
    pub agent_preset: String,
    pub status: WorkstreamStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkstreamStatus {
    Running,
    #[serde(alias = "Archived", alias = "archived")]
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorktreeCheckout {
    NewBranch { base_ref: String },
    ExistingBranch { start_ref: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreePlan {
    pub branch: String,
    pub worktree_path: PathBuf,
    pub checkout: WorktreeCheckout,
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
