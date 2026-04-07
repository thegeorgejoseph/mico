use std::{path::PathBuf, sync::Arc, time::Instant};

use uuid::Uuid;

use super::*;

#[derive(Default)]
struct NoopTaskCompletionStore;

impl TaskCompletionStore for NoopTaskCompletionStore {
    fn load(&self) -> anyhow::Result<Vec<PersistedTaskCompletion>> {
        Ok(Vec::new())
    }

    fn append(&self, _completion: &PersistedTaskCompletion) -> anyhow::Result<()> {
        Ok(())
    }

    fn remove(&self, _completion_id: Uuid) -> anyhow::Result<()> {
        Ok(())
    }
}

fn sample_repo(repo_id: Uuid) -> RepoTarget {
    RepoTarget {
        id: repo_id,
        path: PathBuf::from("/tmp/repo"),
        display_name: "repo".to_string(),
        slug: "repo".to_string(),
    }
}

fn sample_paths() -> AppPaths {
    AppPaths {
        root: PathBuf::from("/tmp"),
        config_path: PathBuf::from("/tmp/config.json"),
        state_path: PathBuf::from("/tmp/state.json"),
        operations_log_path: PathBuf::from("/tmp/operations.jsonl"),
        task_results_path: PathBuf::from("/tmp/task-results.json"),
        worktrees_root: PathBuf::from("/tmp/worktrees"),
    }
}

fn sample_manager(active_tasks: Vec<ActiveTask>) -> BackgroundTaskManager {
    let (sender, receiver) = mpsc::channel();
    BackgroundTaskManager {
        paths: sample_paths(),
        config: AppConfig {
            github_repo: None,
            agent_presets: Vec::new(),
        },
        completion_store: Arc::new(NoopTaskCompletionStore),
        receiver,
        sender,
        next_task_id: 1,
        active_tasks,
    }
}

#[test]
fn repo_mutation_conflicts_with_other_repo_mutations_only() {
    let repo_id = Uuid::new_v4();
    let workstream_id = Uuid::new_v4();
    let manager = sample_manager(vec![ActiveTask {
        id: 1,
        label: "Creating `feature/demo`".to_string(),
        locks: vec![TaskLock::RepoMutation(repo_id)],
        started_at: Instant::now(),
    }]);

    assert_eq!(
        manager.conflicting_task_label(&[TaskLock::RepoMutation(repo_id)]),
        Some("Creating `feature/demo`".to_string())
    );
    assert_eq!(
        manager.conflicting_task_label(&[TaskLock::Workstream(workstream_id)]),
        None
    );
}

#[test]
fn workstream_lock_blocks_same_workstream_only() {
    let workstream_id = Uuid::new_v4();
    let other_workstream_id = Uuid::new_v4();
    let manager = sample_manager(vec![ActiveTask {
        id: 7,
        label: "Stopping `feature/demo`".to_string(),
        locks: vec![TaskLock::Workstream(workstream_id)],
        started_at: Instant::now(),
    }]);

    assert_eq!(
        manager.conflicting_task_label(&[TaskLock::Workstream(workstream_id)]),
        Some("Stopping `feature/demo`".to_string())
    );
    assert_eq!(
        manager.conflicting_task_label(&[TaskLock::Workstream(other_workstream_id)]),
        None
    );
}

#[test]
fn create_workstream_request_uses_repo_lock() {
    let repo = sample_repo(Uuid::new_v4());
    let request = TaskRequest::CreateWorkstream {
        repo: repo.clone(),
        request: WorkstreamRequest::Existing {
            branch: "feature/demo".to_string(),
        },
        agent: "codex".to_string(),
        tracked_worktree_paths: Vec::new(),
    };

    assert_eq!(request.locks(), vec![TaskLock::RepoMutation(repo.id)]);
}
