use std::path::Path;

use crate::domain::model::{AppConfig, DoctorReport, RepoTarget, StoredState, WorktreePlan};

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
    fn remove_worktree(&self, repo: &RepoTarget, worktree_path: &Path) -> anyhow::Result<()>;
}

pub trait SessionBackend {
    fn create_session(
        &self,
        session_name: &str,
        working_dir: &Path,
        startup_command: &str,
    ) -> anyhow::Result<()>;
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

pub trait Updater {
    fn install_or_update(&self, github_repo: Option<&str>) -> anyhow::Result<()>;
}
