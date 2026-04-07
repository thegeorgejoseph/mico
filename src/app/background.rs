use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    app::ports::{
        AgentOneOffRequest, CommandRunner, OperationLog, RepoService, SessionBackend,
        SessionCreateRequest, SessionLabel, TaskCompletionStore,
    },
    domain::model::{
        AppConfig, AppPaths, OperationEvent, OperationLevel, RepoTarget, Workstream,
        WorkstreamRequest, WorkstreamSession, WorkstreamStatus, WorktreeOwnership,
    },
    infra::{
        git::GitCliRepoService, operations::JsonlOperationLog, process::ShellCommandRunner,
        tmux::TmuxSessionBackend,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TaskLock {
    RepoMutation(Uuid),
    Workstream(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLaunchTarget {
    Stay,
    Open,
    Attach,
}

#[derive(Debug, Clone)]
pub struct ActiveTask {
    pub id: u64,
    pub label: String,
    pub locks: Vec<TaskLock>,
    pub started_at: Instant,
}

#[derive(Debug, Clone)]
pub enum TaskRequest {
    LoadBranches {
        repo: RepoTarget,
    },
    RefreshRepo {
        repo: RepoTarget,
        tracked_workstreams: Vec<TrackedWorkstreamPath>,
    },
    CreateWorkstream {
        repo: RepoTarget,
        request: WorkstreamRequest,
        agent: String,
        tracked_worktree_paths: Vec<PathBuf>,
    },
    CreateWorkstreamSession {
        repo: RepoTarget,
        workstream: Workstream,
        agent: String,
        launch_target: SessionLaunchTarget,
    },
    ResumeWorkstreamSession {
        repo: RepoTarget,
        workstream: Workstream,
        session_id: Uuid,
        launch_target: SessionLaunchTarget,
    },
    StopWorkstream {
        workstream: Workstream,
    },
    RemoveWorkstream {
        repo: RepoTarget,
        workstream: Workstream,
    },
    RunOneOff {
        workstream: Workstream,
        agent: String,
        prompt: String,
    },
}

#[derive(Debug, Clone)]
pub struct TrackedWorkstreamPath {
    pub workstream_id: Uuid,
    pub worktree_path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum TaskSuccess {
    BranchesLoaded {
        repo_id: Uuid,
        repo_name: String,
        branches: Vec<String>,
    },
    RepoRefreshed {
        repo_id: Uuid,
        branch_updates: Vec<(Uuid, String)>,
    },
    WorkstreamCreated {
        workstream: Workstream,
    },
    WorkstreamSessionCreated {
        workstream_id: Uuid,
        session: WorkstreamSession,
        launch_target: SessionLaunchTarget,
    },
    WorkstreamSessionResumed {
        workstream_id: Uuid,
        session_id: Uuid,
        launch_target: SessionLaunchTarget,
    },
    WorkstreamStopped {
        workstream_id: Uuid,
    },
    WorkstreamRemoved {
        workstream_id: Uuid,
    },
    OneOffCompleted {
        workstream_id: Uuid,
        output: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedTaskCompletion {
    pub id: Uuid,
    pub result: PersistedTaskResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PersistedTaskResult {
    RepoRefreshed {
        repo_id: Uuid,
        branch_updates: Vec<(Uuid, String)>,
    },
    WorkstreamCreated {
        workstream: Workstream,
    },
    WorkstreamSessionCreated {
        workstream_id: Uuid,
        session: WorkstreamSession,
    },
    WorkstreamSessionResumed {
        workstream_id: Uuid,
        session_id: Uuid,
    },
    WorkstreamStopped {
        workstream_id: Uuid,
    },
    WorkstreamRemoved {
        workstream_id: Uuid,
    },
}

#[derive(Debug)]
pub struct TaskUpdate {
    pub task_id: u64,
    pub label: String,
    pub locks: Vec<TaskLock>,
    pub persisted_completion_id: Option<Uuid>,
    pub result: anyhow::Result<TaskSuccess>,
}

pub struct BackgroundTaskManager {
    paths: AppPaths,
    config: AppConfig,
    completion_store: Arc<dyn TaskCompletionStore + Send + Sync>,
    receiver: Receiver<TaskUpdate>,
    sender: Sender<TaskUpdate>,
    next_task_id: u64,
    active_tasks: Vec<ActiveTask>,
}

impl BackgroundTaskManager {
    pub fn new(
        paths: AppPaths,
        config: AppConfig,
        completion_store: Arc<dyn TaskCompletionStore + Send + Sync>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            paths,
            config,
            completion_store,
            receiver,
            sender,
            next_task_id: 1,
            active_tasks: Vec::new(),
        }
    }

    pub fn active_tasks(&self) -> &[ActiveTask] {
        &self.active_tasks
    }

    pub fn active_labels(&self) -> Vec<String> {
        self.active_tasks
            .iter()
            .map(|task| task.label.clone())
            .collect()
    }

    pub fn is_workstream_locked(&self, workstream_id: Uuid) -> bool {
        self.active_tasks.iter().any(|task| {
            task.locks
                .iter()
                .any(|lock| matches!(lock, TaskLock::Workstream(id) if *id == workstream_id))
        })
    }

    pub fn is_repo_mutating(&self, repo_id: Uuid) -> bool {
        self.active_tasks.iter().any(|task| {
            task.locks
                .iter()
                .any(|lock| matches!(lock, TaskLock::RepoMutation(id) if *id == repo_id))
        })
    }

    pub fn submit(&mut self, request: TaskRequest) -> anyhow::Result<u64> {
        let task_id = self.next_task_id;
        let label = request.label();
        let locks = request.locks();
        if let Some(conflict) = self.conflicting_task_label(&locks) {
            bail!("already busy with {conflict}");
        }

        self.next_task_id += 1;
        self.active_tasks.push(ActiveTask {
            id: task_id,
            label: label.clone(),
            locks: locks.clone(),
            started_at: Instant::now(),
        });

        let sender = self.sender.clone();
        let paths = self.paths.clone();
        let config = self.config.clone();
        let completion_store = Arc::clone(&self.completion_store);
        thread::spawn(move || {
            let result = run_task(paths, config, request);
            let (persisted_completion_id, result) = match result {
                Ok(success) => {
                    let persisted_completion_id =
                        persist_success(&*completion_store, &success).transpose();
                    match persisted_completion_id {
                        Ok(id) => (id, Ok(success)),
                        Err(error) => (None, Err(error)),
                    }
                }
                Err(error) => (None, Err(error)),
            };
            let _ = sender.send(TaskUpdate {
                task_id,
                label,
                locks,
                persisted_completion_id,
                result,
            });
        });

        Ok(task_id)
    }

    pub fn refresh_config(&mut self, config: AppConfig) {
        self.config = config;
    }

    pub fn drain_updates(&mut self) -> Vec<TaskUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.receiver.try_recv() {
            self.active_tasks.retain(|task| task.id != update.task_id);
            updates.push(update);
        }
        updates
    }

    fn conflicting_task_label(&self, requested_locks: &[TaskLock]) -> Option<String> {
        self.active_tasks
            .iter()
            .find(|task| {
                task.locks
                    .iter()
                    .any(|active| requested_locks.iter().any(|requested| requested == active))
            })
            .map(|task| task.label.clone())
    }
}

impl TaskRequest {
    pub fn label(&self) -> String {
        match self {
            Self::LoadBranches { repo } => format!("Loading branches for `{}`", repo.display_name),
            Self::RefreshRepo { repo, .. } => format!("Refreshing `{}`", repo.display_name),
            Self::CreateWorkstream { request, .. } => match request {
                WorkstreamRequest::New { branch, .. } | WorkstreamRequest::Existing { branch } => {
                    format!("Creating `{branch}`")
                }
            },
            Self::CreateWorkstreamSession {
                workstream, agent, ..
            } => {
                format!("Launching {agent} for `{}`", workstream.branch)
            }
            Self::ResumeWorkstreamSession { workstream, .. } => {
                format!("Resuming `{}`", workstream.branch)
            }
            Self::StopWorkstream { workstream } => format!("Stopping `{}`", workstream.branch),
            Self::RemoveWorkstream { workstream, .. } => {
                format!("Removing `{}`", workstream.branch)
            }
            Self::RunOneOff {
                workstream, agent, ..
            } => {
                format!("Running {agent} one-off for `{}`", workstream.branch)
            }
        }
    }

    pub fn locks(&self) -> Vec<TaskLock> {
        match self {
            Self::LoadBranches { repo } | Self::RefreshRepo { repo, .. } => {
                vec![TaskLock::RepoMutation(repo.id)]
            }
            Self::CreateWorkstream { repo, .. } => vec![TaskLock::RepoMutation(repo.id)],
            Self::CreateWorkstreamSession { workstream, .. }
            | Self::ResumeWorkstreamSession { workstream, .. }
            | Self::StopWorkstream { workstream }
            | Self::RunOneOff { workstream, .. } => vec![TaskLock::Workstream(workstream.id)],
            Self::RemoveWorkstream { repo, workstream } => {
                vec![
                    TaskLock::RepoMutation(repo.id),
                    TaskLock::Workstream(workstream.id),
                ]
            }
        }
    }
}

fn run_task(
    paths: AppPaths,
    config: AppConfig,
    request: TaskRequest,
) -> anyhow::Result<TaskSuccess> {
    let repo_service = GitCliRepoService::new();
    let session_backend = TmuxSessionBackend::new();
    let command_runner = ShellCommandRunner::new();
    let operation_log = JsonlOperationLog::new(paths.operations_log_path.clone());

    match request {
        TaskRequest::LoadBranches { repo } => {
            fetch_latest_best_effort(&operation_log, &repo_service, &repo, "prepare branch list");
            let branches = repo_service.list_branches(&repo)?;
            Ok(TaskSuccess::BranchesLoaded {
                repo_id: repo.id,
                repo_name: repo.display_name,
                branches,
            })
        }
        TaskRequest::RefreshRepo {
            repo,
            tracked_workstreams,
        } => {
            run_logged(
                &operation_log,
                &repo.display_name,
                "git.fetch",
                format!("refresh {}", repo.path.display()),
                || repo_service.fetch_latest(&repo),
            )?;
            let mut branch_updates = Vec::new();
            for tracked in tracked_workstreams {
                if !tracked.worktree_path.exists() {
                    continue;
                }
                if let Ok(Some(branch)) = repo_service.current_branch(&tracked.worktree_path) {
                    branch_updates.push((tracked.workstream_id, branch));
                }
            }
            Ok(TaskSuccess::RepoRefreshed {
                repo_id: repo.id,
                branch_updates,
            })
        }
        TaskRequest::CreateWorkstream {
            repo,
            request,
            agent,
            tracked_worktree_paths,
        } => {
            fetch_latest_best_effort(&operation_log, &repo_service, &repo, "prepare workstream");
            let plan = match &request {
                WorkstreamRequest::New {
                    branch,
                    base_branch,
                } => repo_service.plan_new_worktree(
                    &repo,
                    &paths.worktrees_root,
                    branch,
                    base_branch,
                )?,
                WorkstreamRequest::Existing { branch } => {
                    repo_service.plan_existing_worktree(&repo, &paths.worktrees_root, branch)?
                }
            };

            if tracked_worktree_paths.contains(&plan.worktree_path) {
                bail!(
                    "checkout already tracked as a workstream: {}",
                    plan.worktree_path.display()
                );
            }

            run_logged(
                &operation_log,
                &repo.display_name,
                "git.worktree.create",
                format!(
                    "create `{}` at {}",
                    plan.branch,
                    plan.worktree_path.display()
                ),
                || repo_service.create_worktree(&repo, &plan),
            )?;

            if matches!(request, WorkstreamRequest::New { .. })
                && let Err(error) = configure_push_target(
                    &operation_log,
                    &repo_service,
                    &repo,
                    &plan.branch,
                    &plan.worktree_path,
                )
            {
                if matches!(plan.worktree_ownership, WorktreeOwnership::Managed) {
                    let _ = repo_service.remove_worktree(&repo, &plan.worktree_path);
                }
                return Err(error);
            }

            let session = new_session_record(&repo, &plan.branch, &agent, 1);
            let create_request = session_create_request(
                &config,
                &repo,
                &plan.branch,
                &plan.worktree_path,
                &session,
                1,
            )?;
            if let Err(error) = run_logged(
                &operation_log,
                &plan.branch,
                "tmux.session.create",
                format!("launch {} in {}", agent, plan.worktree_path.display()),
                || session_backend.create_session(&create_request),
            ) {
                if matches!(plan.worktree_ownership, WorktreeOwnership::Managed) {
                    let _ = repo_service.remove_worktree(&repo, &plan.worktree_path);
                }
                return Err(error);
            }

            let mut workstream = Workstream {
                id: Uuid::new_v4(),
                repo_id: repo.id,
                base_branch: match &request {
                    WorkstreamRequest::New { base_branch, .. } => base_branch.clone(),
                    WorkstreamRequest::Existing { branch } => branch.clone(),
                },
                branch: plan.branch.clone(),
                worktree_path: plan.worktree_path.clone(),
                worktree_ownership: plan.worktree_ownership,
                session_name: session.session_name.clone(),
                agent_preset: session.agent_preset.clone(),
                status: WorkstreamStatus::Running,
                attention: crate::domain::model::WorkstreamAttention::None,
                pinned: false,
                created_at_epoch_secs: now_epoch_secs(),
                status_changed_at_epoch_secs: now_epoch_secs(),
                last_opened_at_epoch_secs: None,
                last_attached_at_epoch_secs: None,
                sessions: vec![session.clone()],
                preferred_session_id: Some(session.id),
            };
            workstream.sync_legacy_summary();
            Ok(TaskSuccess::WorkstreamCreated { workstream })
        }
        TaskRequest::CreateWorkstreamSession {
            repo,
            workstream,
            agent,
            launch_target,
        } => {
            let ordinal = workstream.session_count() + 1;
            let session = new_session_record(&repo, &workstream.branch, &agent, ordinal);
            let create_request = session_create_request(
                &config,
                &repo,
                &workstream.branch,
                &workstream.worktree_path,
                &session,
                ordinal,
            )?;
            run_logged(
                &operation_log,
                &workstream.branch,
                "tmux.session.create",
                format!("launch {} session {}", agent, ordinal),
                || session_backend.create_session(&create_request),
            )?;
            Ok(TaskSuccess::WorkstreamSessionCreated {
                workstream_id: workstream.id,
                session,
                launch_target,
            })
        }
        TaskRequest::ResumeWorkstreamSession {
            repo,
            workstream,
            session_id,
            launch_target,
        } => {
            let Some(session) = workstream.session_by_id(session_id).cloned() else {
                bail!(
                    "no session matched `{session_id}` for `{}`",
                    workstream.branch
                );
            };

            if session_backend.has_session(&session.session_name) {
                sync_session_metadata(
                    &operation_log,
                    &config,
                    &session_backend,
                    &repo,
                    &workstream,
                    &session,
                )?;
                return Ok(TaskSuccess::WorkstreamSessionResumed {
                    workstream_id: workstream.id,
                    session_id,
                    launch_target,
                });
            }

            if !workstream.worktree_path.exists() {
                bail!(
                    "worktree path is missing for `{}`: {}",
                    workstream.branch,
                    workstream.worktree_path.display()
                );
            }

            let ordinal = workstream
                .sessions
                .iter()
                .position(|candidate| candidate.id == session_id)
                .map(|index| index + 1)
                .unwrap_or(1);
            let create_request = session_create_request(
                &config,
                &repo,
                &workstream.branch,
                &workstream.worktree_path,
                &session,
                ordinal,
            )?;
            run_logged(
                &operation_log,
                &workstream.branch,
                "tmux.session.recreate",
                format!("restore {}", session.session_name),
                || session_backend.create_session(&create_request),
            )?;
            Ok(TaskSuccess::WorkstreamSessionResumed {
                workstream_id: workstream.id,
                session_id,
                launch_target,
            })
        }
        TaskRequest::StopWorkstream { workstream } => {
            for session in &workstream.sessions {
                let _ = session_backend.stop(&session.session_name);
            }
            Ok(TaskSuccess::WorkstreamStopped {
                workstream_id: workstream.id,
            })
        }
        TaskRequest::RemoveWorkstream { repo, workstream } => {
            for session in &workstream.sessions {
                let _ = session_backend.stop(&session.session_name);
            }
            if matches!(workstream.worktree_ownership, WorktreeOwnership::Managed) {
                repo_service.remove_worktree(&repo, &workstream.worktree_path)?;
            }
            Ok(TaskSuccess::WorkstreamRemoved {
                workstream_id: workstream.id,
            })
        }
        TaskRequest::RunOneOff {
            workstream,
            agent,
            prompt,
        } => {
            let preset = config
                .agent_presets
                .iter()
                .find(|preset| preset.name == agent)
                .with_context(|| format!("unknown agent preset `{agent}`"))?;
            let command_template = preset
                .one_off_command
                .clone()
                .with_context(|| format!("agent `{agent}` does not support one-off runs"))?;
            let result = run_logged(
                &operation_log,
                &workstream.branch,
                "agent.one-off",
                format!(
                    "run {} prompt in {}",
                    agent,
                    workstream.worktree_path.display()
                ),
                || {
                    command_runner.run_agent_one_off(&AgentOneOffRequest {
                        preset_name: agent,
                        command_template,
                        prompt,
                        working_dir: workstream.worktree_path.clone(),
                    })
                },
            )?;
            let mut output = String::new();
            if !result.stdout.trim().is_empty() {
                output.push_str(result.stdout.trim_end());
            }
            if !result.stderr.trim().is_empty() {
                if !output.is_empty() {
                    output.push_str("\n\n");
                }
                output.push_str(result.stderr.trim_end());
            }
            if output.is_empty() {
                output.push_str("Command completed without output.");
            }
            Ok(TaskSuccess::OneOffCompleted {
                workstream_id: workstream.id,
                output,
            })
        }
    }
}

fn persist_success(
    completion_store: &dyn TaskCompletionStore,
    success: &TaskSuccess,
) -> Option<anyhow::Result<Uuid>> {
    let result = success.persisted_result()?;
    let completion = PersistedTaskCompletion {
        id: Uuid::new_v4(),
        result,
    };
    Some(completion_store.append(&completion).map(|_| completion.id))
}

impl TaskSuccess {
    pub fn persisted_result(&self) -> Option<PersistedTaskResult> {
        match self {
            Self::BranchesLoaded { .. } | Self::OneOffCompleted { .. } => None,
            Self::RepoRefreshed {
                repo_id,
                branch_updates,
            } => Some(PersistedTaskResult::RepoRefreshed {
                repo_id: *repo_id,
                branch_updates: branch_updates.clone(),
            }),
            Self::WorkstreamCreated { workstream } => {
                Some(PersistedTaskResult::WorkstreamCreated {
                    workstream: workstream.clone(),
                })
            }
            Self::WorkstreamSessionCreated {
                workstream_id,
                session,
                ..
            } => Some(PersistedTaskResult::WorkstreamSessionCreated {
                workstream_id: *workstream_id,
                session: session.clone(),
            }),
            Self::WorkstreamSessionResumed {
                workstream_id,
                session_id,
                ..
            } => Some(PersistedTaskResult::WorkstreamSessionResumed {
                workstream_id: *workstream_id,
                session_id: *session_id,
            }),
            Self::WorkstreamStopped { workstream_id } => {
                Some(PersistedTaskResult::WorkstreamStopped {
                    workstream_id: *workstream_id,
                })
            }
            Self::WorkstreamRemoved { workstream_id } => {
                Some(PersistedTaskResult::WorkstreamRemoved {
                    workstream_id: *workstream_id,
                })
            }
        }
    }
}

fn fetch_latest_best_effort(
    operation_log: &JsonlOperationLog,
    repo_service: &GitCliRepoService,
    repo: &RepoTarget,
    purpose: &str,
) {
    if let Err(error) = run_logged(
        operation_log,
        &repo.display_name,
        "git.fetch",
        format!("{purpose} using {}", repo.path.display()),
        || repo_service.fetch_latest(repo),
    ) {
        record_operation(
            operation_log,
            OperationLevel::Warning,
            repo.display_name.as_str(),
            "git.fetch.warning",
            format!("continuing with cached refs: {error}"),
        );
    }
}

fn configure_push_target(
    operation_log: &JsonlOperationLog,
    repo_service: &GitCliRepoService,
    repo: &RepoTarget,
    branch: &str,
    worktree_path: &Path,
) -> anyhow::Result<()> {
    let remote = repo_service.default_remote(repo)?;
    run_logged(
        operation_log,
        branch,
        "git.push-target",
        format!("configure upstream on {}", worktree_path.display()),
        || repo_service.configure_push_target(worktree_path, branch, &remote),
    )
}

fn sync_session_metadata(
    operation_log: &JsonlOperationLog,
    config: &AppConfig,
    session_backend: &TmuxSessionBackend,
    repo: &RepoTarget,
    workstream: &Workstream,
    session: &WorkstreamSession,
) -> anyhow::Result<()> {
    let ordinal = workstream
        .sessions
        .iter()
        .position(|candidate| candidate.id == session.id)
        .map(|index| index + 1)
        .unwrap_or(1);
    let request = session_create_request(
        config,
        repo,
        &workstream.branch,
        &workstream.worktree_path,
        session,
        ordinal,
    )?;
    run_logged(
        operation_log,
        &workstream.branch,
        "tmux.session.sync",
        format!("refresh {}", session.session_name),
        || session_backend.sync_session(&request),
    )
}

fn session_create_request(
    config: &AppConfig,
    repo: &RepoTarget,
    branch: &str,
    worktree_path: &Path,
    session: &WorkstreamSession,
    ordinal: usize,
) -> anyhow::Result<SessionCreateRequest> {
    let startup_command = config
        .agent_presets
        .iter()
        .find(|preset| preset.name == session.agent_preset)
        .map(|preset| preset.command.clone())
        .with_context(|| format!("unknown agent preset `{}`", session.agent_preset))?;
    Ok(SessionCreateRequest {
        session_name: session.session_name.clone(),
        working_dir: worktree_path.to_path_buf(),
        startup_command,
        label: SessionLabel {
            repo_name: repo.display_name.clone(),
            workstream_branch: branch.to_string(),
            agent_preset: session.agent_preset.clone(),
            session_ordinal: ordinal,
        },
    })
}

fn new_session_record(
    repo: &RepoTarget,
    branch: &str,
    agent: &str,
    ordinal: usize,
) -> WorkstreamSession {
    let now = now_epoch_secs();
    WorkstreamSession {
        id: Uuid::new_v4(),
        session_name: session_name(&repo.slug, branch, agent, ordinal),
        agent_preset: agent.to_string(),
        status: WorkstreamStatus::Running,
        created_at_epoch_secs: now,
        status_changed_at_epoch_secs: now,
        last_opened_at_epoch_secs: None,
        last_attached_at_epoch_secs: None,
    }
}

fn session_name(repo_slug: &str, branch: &str, agent: &str, ordinal: usize) -> String {
    let branch = branch
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_lowercase();
    let agent = agent
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_lowercase();
    let short = Uuid::new_v4().simple().to_string();
    let short = &short[..8];
    format!("mico-{repo_slug}-{branch}-{agent}-{ordinal}-{short}")
}

fn run_logged<T, F>(
    operation_log: &JsonlOperationLog,
    scope: &str,
    action: &str,
    detail: String,
    operation: F,
) -> anyhow::Result<T>
where
    F: FnOnce() -> anyhow::Result<T>,
{
    record_operation(
        operation_log,
        OperationLevel::Started,
        scope,
        action,
        detail.clone(),
    );
    match operation() {
        Ok(value) => {
            record_operation(
                operation_log,
                OperationLevel::Succeeded,
                scope,
                action,
                detail,
            );
            Ok(value)
        }
        Err(error) => {
            record_operation(
                operation_log,
                OperationLevel::Failed,
                scope,
                action,
                format!("{detail}: {error}"),
            );
            Err(error)
        }
    }
}

fn record_operation(
    operation_log: &JsonlOperationLog,
    level: OperationLevel,
    scope: &str,
    action: &str,
    detail: String,
) {
    let _ = operation_log.record(&OperationEvent {
        timestamp_epoch_secs: now_epoch_secs(),
        level,
        scope: scope.to_string(),
        action: action.to_string(),
        detail,
    });
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
