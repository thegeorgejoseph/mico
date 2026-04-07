use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
};

use tempfile::TempDir;
use uuid::Uuid;

use super::*;
use crate::{
    app::ports::{AgentOneOffResult, ConfigStore, StateStore},
    domain::model::{AgentPreset, DependencyStatus, OperationEvent, WorktreePlan},
    infra::config::default_state,
};

#[derive(Clone, Default)]
struct FakeStore {
    saved_state: Rc<RefCell<Option<StoredState>>>,
    saved_config: Rc<RefCell<Option<AppConfig>>>,
}

impl ConfigStore for FakeStore {
    fn load_or_create_config(&self, default: AppConfig) -> anyhow::Result<AppConfig> {
        Ok(default)
    }

    fn save_config(&self, config: &AppConfig) -> anyhow::Result<()> {
        *self.saved_config.borrow_mut() = Some(config.clone());
        Ok(())
    }
}

impl StateStore for FakeStore {
    fn load_or_create_state(&self, default: StoredState) -> anyhow::Result<StoredState> {
        Ok(default)
    }

    fn save_state(&self, state: &StoredState) -> anyhow::Result<()> {
        *self.saved_state.borrow_mut() = Some(state.clone());
        Ok(())
    }
}

#[derive(Clone)]
struct FakeRepoService {
    fetch_error: Rc<RefCell<Option<String>>>,
    branches: Rc<RefCell<Vec<String>>>,
    push_target_calls: Rc<RefCell<Vec<(PathBuf, String, String)>>>,
}

impl Default for FakeRepoService {
    fn default() -> Self {
        Self {
            fetch_error: Rc::new(RefCell::new(None)),
            branches: Rc::new(RefCell::new(vec!["main".to_string()])),
            push_target_calls: Rc::new(RefCell::new(Vec::new())),
        }
    }
}

impl RepoService for FakeRepoService {
    fn discover_repo(&self, path: &Path, display_name: Option<&str>) -> anyhow::Result<RepoTarget> {
        Ok(RepoTarget {
            id: Uuid::new_v4(),
            path: path.to_path_buf(),
            display_name: display_name.unwrap_or("repo").to_string(),
            slug: "repo".to_string(),
        })
    }

    fn current_branch(&self, path: &Path) -> anyhow::Result<Option<String>> {
        let branch_path = path.join(".branch");
        if branch_path.exists() {
            Ok(Some(fs::read_to_string(branch_path)?.trim().to_string()))
        } else {
            Ok(None)
        }
    }

    fn list_branches(&self, _repo: &RepoTarget) -> anyhow::Result<Vec<String>> {
        Ok(self.branches.borrow().clone())
    }

    fn default_remote(&self, _repo: &RepoTarget) -> anyhow::Result<String> {
        Ok("origin".to_string())
    }

    fn fetch_latest(&self, _repo: &RepoTarget) -> anyhow::Result<()> {
        if let Some(message) = self.fetch_error.borrow().clone() {
            bail!(message)
        }
        Ok(())
    }

    fn plan_new_worktree(
        &self,
        _repo: &RepoTarget,
        worktrees_root: &Path,
        branch: &str,
        base_branch: &str,
    ) -> anyhow::Result<WorktreePlan> {
        Ok(WorktreePlan {
            branch: branch.to_string(),
            worktree_path: worktrees_root.join(branch),
            worktree_ownership: WorktreeOwnership::Managed,
            checkout: crate::domain::model::WorktreeCheckout::NewBranch {
                base_ref: format!("origin/{base_branch}"),
            },
        })
    }

    fn plan_existing_worktree(
        &self,
        _repo: &RepoTarget,
        worktrees_root: &Path,
        branch: &str,
    ) -> anyhow::Result<WorktreePlan> {
        Ok(WorktreePlan {
            branch: branch.to_string(),
            worktree_path: worktrees_root.join(branch),
            worktree_ownership: WorktreeOwnership::Managed,
            checkout: crate::domain::model::WorktreeCheckout::ExistingBranch {
                start_ref: Some(format!("origin/{branch}")),
            },
        })
    }

    fn create_worktree(&self, _repo: &RepoTarget, plan: &WorktreePlan) -> anyhow::Result<()> {
        fs::create_dir_all(&plan.worktree_path)?;
        fs::write(plan.worktree_path.join(".branch"), &plan.branch)?;
        Ok(())
    }

    fn configure_push_target(
        &self,
        worktree_path: &Path,
        branch: &str,
        remote: &str,
    ) -> anyhow::Result<()> {
        self.push_target_calls.borrow_mut().push((
            worktree_path.to_path_buf(),
            branch.to_string(),
            remote.to_string(),
        ));
        Ok(())
    }

    fn remove_worktree(&self, _repo: &RepoTarget, worktree_path: &Path) -> anyhow::Result<()> {
        if worktree_path.exists() {
            fs::remove_dir_all(worktree_path)?;
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
struct FakeSessionBackend {
    sessions: Rc<RefCell<HashMap<String, SessionCreateRequest>>>,
}

impl SessionBackend for FakeSessionBackend {
    fn create_session(&self, request: &SessionCreateRequest) -> anyhow::Result<()> {
        self.sessions
            .borrow_mut()
            .insert(request.session_name.clone(), request.clone());
        Ok(())
    }

    fn sync_session(&self, request: &SessionCreateRequest) -> anyhow::Result<()> {
        self.sessions
            .borrow_mut()
            .insert(request.session_name.clone(), request.clone());
        Ok(())
    }

    fn has_session(&self, session_name: &str) -> bool {
        self.sessions.borrow().contains_key(session_name)
    }

    fn attach(&self, _session_name: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn stop(&self, session_name: &str) -> anyhow::Result<()> {
        self.sessions.borrow_mut().remove(session_name);
        Ok(())
    }

    fn attach_command(&self, session_name: &str) -> Vec<String> {
        vec![
            "tmux".to_string(),
            "attach".to_string(),
            "-t".to_string(),
            session_name.to_string(),
        ]
    }

    fn capture_recent_lines(
        &self,
        session_name: &str,
        _lines: usize,
    ) -> anyhow::Result<Vec<String>> {
        Ok(vec![format!("output from {session_name}")])
    }
}

#[derive(Clone, Default)]
struct FakeTerminalFrontend;

impl TerminalFrontend for FakeTerminalFrontend {
    fn open_session(&self, _session_name: &str, _attach_command: &[String]) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
struct FakeDependencyInspector {
    paths: AppPaths,
}

impl DependencyInspector for FakeDependencyInspector {
    fn doctor(&self) -> anyhow::Result<DoctorReport> {
        Ok(DoctorReport {
            paths: self.paths.clone(),
            dependencies: vec![DependencyStatus {
                name: "git".to_string(),
                found: true,
                detail: "ok".to_string(),
            }],
        })
    }
}

#[derive(Clone, Default)]
struct FakeCommandRunner {
    last_request: Rc<RefCell<Option<AgentOneOffRequest>>>,
}

impl CommandRunner for FakeCommandRunner {
    fn run_agent_one_off(&self, request: &AgentOneOffRequest) -> anyhow::Result<AgentOneOffResult> {
        *self.last_request.borrow_mut() = Some(request.clone());
        Ok(AgentOneOffResult {
            stdout: "done".to_string(),
            stderr: String::new(),
        })
    }
}

#[derive(Clone, Default)]
struct FakeOperationLog {
    events: Rc<RefCell<Vec<OperationEvent>>>,
}

impl OperationLog for FakeOperationLog {
    fn record(&self, event: &OperationEvent) -> anyhow::Result<()> {
        self.events.borrow_mut().push(event.clone());
        Ok(())
    }

    fn recent(&self, limit: usize) -> anyhow::Result<Vec<OperationEvent>> {
        let events = self.events.borrow();
        let start = events.len().saturating_sub(limit);
        Ok(events[start..].to_vec())
    }
}

#[derive(Clone, Default)]
struct FakeCompletionStore {
    completions: Arc<Mutex<Vec<crate::app::background::PersistedTaskCompletion>>>,
}

impl TaskCompletionStore for FakeCompletionStore {
    fn load(&self) -> anyhow::Result<Vec<crate::app::background::PersistedTaskCompletion>> {
        Ok(self
            .completions
            .lock()
            .map_err(|_| anyhow::anyhow!("completion store lock poisoned"))?
            .clone())
    }

    fn append(
        &self,
        completion: &crate::app::background::PersistedTaskCompletion,
    ) -> anyhow::Result<()> {
        self.completions
            .lock()
            .map_err(|_| anyhow::anyhow!("completion store lock poisoned"))?
            .push(completion.clone());
        Ok(())
    }

    fn remove(&self, completion_id: Uuid) -> anyhow::Result<()> {
        self.completions
            .lock()
            .map_err(|_| anyhow::anyhow!("completion store lock poisoned"))?
            .retain(|completion| completion.id != completion_id);
        Ok(())
    }
}

struct RuntimeFixture {
    _temp: TempDir,
    runtime: MicoRuntime,
    repo_id: Uuid,
    repo_service: FakeRepoService,
    operation_log: FakeOperationLog,
    completion_store: Arc<FakeCompletionStore>,
}

fn make_fixture() -> anyhow::Result<RuntimeFixture> {
    let temp = TempDir::new()?;
    let paths = AppPaths {
        root: temp.path().join(".mico"),
        config_path: temp.path().join(".mico/config.json"),
        state_path: temp.path().join(".mico/state.json"),
        operations_log_path: temp.path().join(".mico/operations.jsonl"),
        task_results_path: temp.path().join(".mico/task-results.json"),
        worktrees_root: temp.path().join(".mico/worktrees"),
    };
    fs::create_dir_all(&paths.worktrees_root)?;

    let repo_path = temp.path().join("repo");
    fs::create_dir_all(&repo_path)?;
    let repo = RepoTarget {
        id: Uuid::new_v4(),
        path: repo_path,
        display_name: "Repo".to_string(),
        slug: "repo".to_string(),
    };

    let repo_service = FakeRepoService::default();
    let operation_log = FakeOperationLog::default();
    let completion_store = Arc::new(FakeCompletionStore::default());
    let config = AppConfig {
        github_repo: None,
        agent_presets: vec![
            AgentPreset {
                name: "terminal".to_string(),
                command: String::new(),
                one_off_command: None,
            },
            AgentPreset {
                name: "codex".to_string(),
                command: "codex".to_string(),
                one_off_command: Some("codex exec {prompt}".to_string()),
            },
            AgentPreset {
                name: "opencode".to_string(),
                command: "opencode".to_string(),
                one_off_command: Some("opencode run {prompt}".to_string()),
            },
        ],
    };
    let mut state = default_state();
    state.repos.push(repo.clone());

    let runtime = MicoRuntime::with_interfaces(
        paths.clone(),
        RuntimeInterfaces {
            store: Box::new(FakeStore::default()),
            repo_service: Box::new(repo_service.clone()),
            session_backend: Box::new(FakeSessionBackend::default()),
            terminal: Box::new(FakeTerminalFrontend),
            dependency_inspector: Box::new(FakeDependencyInspector { paths }),
            command_runner: Box::new(FakeCommandRunner::default()),
            operation_log: Box::new(operation_log.clone()),
            completion_store: completion_store.clone(),
        },
        config,
        state,
    )?;

    Ok(RuntimeFixture {
        _temp: temp,
        runtime,
        repo_id: repo.id,
        repo_service,
        operation_log,
        completion_store,
    })
}

#[test]
fn branches_for_repo_uses_cached_refs_when_fetch_fails() -> anyhow::Result<()> {
    let mut fixture = make_fixture()?;
    *fixture.repo_service.fetch_error.borrow_mut() = Some("prune failed".to_string());

    let branches = fixture.runtime.branches_for_repo(fixture.repo_id)?;

    assert_eq!(branches, vec!["main".to_string()]);
    assert!(
        fixture
            .operation_log
            .events
            .borrow()
            .iter()
            .any(|event| matches!(event.level, OperationLevel::Warning))
    );
    Ok(())
}

#[test]
fn create_workstream_configures_push_target_for_new_branches() -> anyhow::Result<()> {
    let mut fixture = make_fixture()?;

    fixture.runtime.create_workstream(
        fixture.repo_id,
        WorkstreamRequest::New {
            branch: "feature-a".to_string(),
            base_branch: "main".to_string(),
        },
        "codex",
        LaunchMode::Stay,
    )?;

    assert_eq!(fixture.repo_service.push_target_calls.borrow().len(), 1);
    Ok(())
}

#[test]
fn create_workstream_session_adds_a_second_preferred_session() -> anyhow::Result<()> {
    let mut fixture = make_fixture()?;
    let workstream = fixture.runtime.create_workstream(
        fixture.repo_id,
        WorkstreamRequest::New {
            branch: "feature-b".to_string(),
            base_branch: "main".to_string(),
        },
        "codex",
        LaunchMode::Stay,
    )?;

    let session =
        fixture
            .runtime
            .create_workstream_session(workstream.id, "opencode", LaunchMode::Stay)?;
    let updated = fixture.runtime.workstream_by_id(workstream.id)?;

    assert_eq!(updated.session_count(), 2);
    assert_eq!(updated.preferred_session_id, Some(session.id));
    assert_eq!(
        updated
            .preferred_session()
            .map(|entry| entry.agent_preset.as_str()),
        Some("opencode")
    );
    Ok(())
}

#[test]
fn run_workstream_one_off_uses_selected_agent_template() -> anyhow::Result<()> {
    let mut fixture = make_fixture()?;
    let workstream = fixture.runtime.create_workstream(
        fixture.repo_id,
        WorkstreamRequest::New {
            branch: "feature-c".to_string(),
            base_branch: "main".to_string(),
        },
        "codex",
        LaunchMode::Stay,
    )?;

    let output = fixture.runtime.run_workstream_one_off(
        workstream.id,
        "opencode",
        "summarize the branch",
    )?;

    assert_eq!(output, "done");
    Ok(())
}

#[test]
fn runtime_reconciles_persisted_background_completions_on_boot() -> anyhow::Result<()> {
    let fixture = make_fixture()?;
    let repo_id = fixture.repo_id;
    let workstream = Workstream {
        id: Uuid::new_v4(),
        repo_id,
        base_branch: "main".to_string(),
        branch: "feature-durable".to_string(),
        worktree_path: fixture.runtime.paths.worktrees_root.join("feature-durable"),
        worktree_ownership: WorktreeOwnership::Managed,
        session_name: "mico-repo-feature-durable-codex-1-deadbeef".to_string(),
        agent_preset: "codex".to_string(),
        status: WorkstreamStatus::Running,
        attention: WorkstreamAttention::None,
        pinned: false,
        created_at_epoch_secs: 1,
        status_changed_at_epoch_secs: 1,
        last_opened_at_epoch_secs: None,
        last_attached_at_epoch_secs: None,
        sessions: vec![WorkstreamSession {
            id: Uuid::new_v4(),
            session_name: "mico-repo-feature-durable-codex-1-deadbeef".to_string(),
            agent_preset: "codex".to_string(),
            status: WorkstreamStatus::Running,
            created_at_epoch_secs: 1,
            status_changed_at_epoch_secs: 1,
            last_opened_at_epoch_secs: None,
            last_attached_at_epoch_secs: None,
        }],
        preferred_session_id: None,
    };
    fixture
        .completion_store
        .append(&crate::app::background::PersistedTaskCompletion {
            id: Uuid::new_v4(),
            result: PersistedTaskResult::WorkstreamCreated {
                workstream: workstream.clone(),
            },
        })?;

    let runtime = MicoRuntime::with_interfaces(
        fixture.runtime.paths.clone(),
        RuntimeInterfaces {
            store: Box::new(FakeStore::default()),
            repo_service: Box::new(fixture.repo_service.clone()),
            session_backend: Box::new(FakeSessionBackend::default()),
            terminal: Box::new(FakeTerminalFrontend),
            dependency_inspector: Box::new(FakeDependencyInspector {
                paths: fixture.runtime.paths.clone(),
            }),
            command_runner: Box::new(FakeCommandRunner::default()),
            operation_log: Box::new(fixture.operation_log.clone()),
            completion_store: fixture.completion_store.clone(),
        },
        fixture.runtime.config.clone(),
        fixture.runtime.state.clone(),
    )?;

    assert!(runtime.workstream_by_id(workstream.id).is_ok());
    assert!(fixture.completion_store.load()?.is_empty());
    Ok(())
}
