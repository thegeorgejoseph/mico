use std::{
    env,
    path::Path,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, bail};
use uuid::Uuid;

use crate::{
    app::background::PersistedTaskResult,
    app::ports::{
        AgentOneOffRequest, CommandRunner, DependencyInspector, OperationLog, RepoService,
        RuntimeStore, SessionBackend, SessionCreateRequest, SessionLabel, TaskCompletionStore,
        TerminalFrontend,
    },
    domain::model::{
        AppConfig, AppPaths, DoctorReport, OperationEvent, OperationLevel, RepoTarget, StoredState,
        Workstream, WorkstreamAttention, WorkstreamRequest, WorkstreamSession, WorkstreamStatus,
        WorktreeOwnership,
    },
    infra::{
        config::default_agent_presets, deps::SystemDependencyInspector, git::GitCliRepoService,
        iterm::ITermFrontend, json_store::JsonFileStore, operations::JsonlOperationLog,
        process::ShellCommandRunner, task_store::JsonTaskCompletionStore, tmux::TmuxSessionBackend,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum LaunchMode {
    Stay,
    Open,
    Attach,
}

pub struct RuntimeInterfaces {
    pub store: Box<dyn RuntimeStore>,
    pub repo_service: Box<dyn RepoService>,
    pub session_backend: Box<dyn SessionBackend>,
    pub terminal: Box<dyn TerminalFrontend>,
    pub dependency_inspector: Box<dyn DependencyInspector>,
    pub command_runner: Box<dyn CommandRunner>,
    pub operation_log: Box<dyn OperationLog>,
    pub completion_store: Arc<dyn TaskCompletionStore + Send + Sync>,
}

pub struct MicoRuntime {
    paths: AppPaths,
    store: Box<dyn RuntimeStore>,
    repo_service: Box<dyn RepoService>,
    session_backend: Box<dyn SessionBackend>,
    terminal: Box<dyn TerminalFrontend>,
    dependency_inspector: Box<dyn DependencyInspector>,
    command_runner: Box<dyn CommandRunner>,
    operation_log: Box<dyn OperationLog>,
    completion_store: Arc<dyn TaskCompletionStore + Send + Sync>,
    pub config: AppConfig,
    pub state: StoredState,
    pub report: DoctorReport,
}

impl MicoRuntime {
    pub fn new(
        paths: AppPaths,
        store: JsonFileStore,
        config: AppConfig,
        state: StoredState,
    ) -> anyhow::Result<Self> {
        let dependency_paths = paths.clone();
        let operations_log_path = paths.operations_log_path.clone();
        let completion_store: Arc<dyn TaskCompletionStore + Send + Sync> = Arc::new(
            JsonTaskCompletionStore::new(paths.task_results_path.clone()),
        );
        Self::with_interfaces(
            paths,
            RuntimeInterfaces {
                store: Box::new(store),
                repo_service: Box::new(GitCliRepoService::new()),
                session_backend: Box::new(TmuxSessionBackend::new()),
                terminal: Box::new(ITermFrontend::new()),
                dependency_inspector: Box::new(SystemDependencyInspector::new(dependency_paths)),
                command_runner: Box::new(ShellCommandRunner::new()),
                operation_log: Box::new(JsonlOperationLog::new(operations_log_path)),
                completion_store,
            },
            config,
            state,
        )
    }

    pub fn with_interfaces(
        paths: AppPaths,
        interfaces: RuntimeInterfaces,
        config: AppConfig,
        state: StoredState,
    ) -> anyhow::Result<Self> {
        let report = interfaces.dependency_inspector.doctor()?;

        let mut runtime = Self {
            paths,
            store: interfaces.store,
            repo_service: interfaces.repo_service,
            session_backend: interfaces.session_backend,
            terminal: interfaces.terminal,
            dependency_inspector: interfaces.dependency_inspector,
            command_runner: interfaces.command_runner,
            operation_log: interfaces.operation_log,
            completion_store: interfaces.completion_store,
            config,
            state,
            report,
        };

        runtime.hydrate_agent_presets()?;
        runtime.reconcile_background_completions()?;
        runtime.hydrate_workstream_metadata()?;
        runtime.reconcile_workstream_branches()?;
        runtime.reconcile_workstream_sessions()?;
        Ok(runtime)
    }

    pub fn refresh_doctor(&mut self) -> anyhow::Result<()> {
        self.report = self.dependency_inspector.doctor()?;
        self.reconcile_workstream_branches()?;
        Ok(())
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn config_snapshot(&self) -> AppConfig {
        self.config.clone()
    }

    pub fn completion_store(&self) -> Arc<dyn TaskCompletionStore + Send + Sync> {
        Arc::clone(&self.completion_store)
    }

    pub fn refresh_repo(&mut self, repo_id: Uuid) -> anyhow::Result<()> {
        let repo = self.repo_by_id(repo_id)?.clone();
        self.run_logged(
            &repo.display_name,
            "git.fetch",
            format!("refresh {}", repo.path.display()),
            || self.repo_service.fetch_latest(&repo),
        )?;
        self.reconcile_workstream_branches()
    }

    pub fn open_repo_in_vscode(&self, repo_id: Uuid) -> anyhow::Result<()> {
        let repo = self.repo_by_id(repo_id)?;
        open_in_vscode(&repo.path)
    }

    pub fn add_repo(
        &mut self,
        path: Option<PathBuf>,
        display_name: Option<String>,
    ) -> anyhow::Result<RepoTarget> {
        let repo_path = path.unwrap_or(env::current_dir()?);
        let discovered = self
            .repo_service
            .discover_repo(&repo_path, display_name.as_deref())?;

        if self
            .state
            .repos
            .iter()
            .any(|repo| repo.path == discovered.path)
        {
            bail!(
                "repository is already tracked: {}",
                discovered.path.display()
            );
        }

        self.state.repos.push(discovered.clone());
        self.save_state()?;
        Ok(discovered)
    }

    pub fn remove_repo(&mut self, repo_id: Uuid) -> anyhow::Result<String> {
        let repo = self.repo_by_id(repo_id)?.clone();
        let attached_workstreams = self
            .state
            .workstreams
            .iter()
            .filter(|workstream| workstream.repo_id == repo.id)
            .count();

        if attached_workstreams > 0 {
            bail!(
                "repository `{}` still has {} tracked workstream(s); remove them first",
                repo.display_name,
                attached_workstreams
            );
        }

        self.state.repos.retain(|candidate| candidate.id != repo.id);
        self.save_state()?;
        Ok(repo.display_name)
    }

    pub fn branches_for_repo(&mut self, repo_id: Uuid) -> anyhow::Result<Vec<String>> {
        let repo = self.repo_by_id(repo_id)?.clone();
        self.fetch_latest_best_effort(&repo, "prepare branch list");
        self.repo_service.list_branches(&repo)
    }

    pub fn create_workstream_new(
        &mut self,
        repo_id: Uuid,
        base_branch: &str,
        branch: &str,
        agent: &str,
        launch_mode: LaunchMode,
    ) -> anyhow::Result<Workstream> {
        self.create_workstream(
            repo_id,
            WorkstreamRequest::New {
                branch: branch.to_string(),
                base_branch: base_branch.to_string(),
            },
            agent,
            launch_mode,
        )
    }

    pub fn create_workstream_existing(
        &mut self,
        repo_id: Uuid,
        branch: &str,
        agent: &str,
        launch_mode: LaunchMode,
    ) -> anyhow::Result<Workstream> {
        self.create_workstream(
            repo_id,
            WorkstreamRequest::Existing {
                branch: branch.to_string(),
            },
            agent,
            launch_mode,
        )
    }

    pub fn create_workstream(
        &mut self,
        repo_id: Uuid,
        request: WorkstreamRequest,
        agent: &str,
        launch_mode: LaunchMode,
    ) -> anyhow::Result<Workstream> {
        let repo = self.repo_by_id(repo_id)?.clone();
        self.fetch_latest_best_effort(&repo, "prepare workstream");
        let plan =
            match &request {
                WorkstreamRequest::New {
                    branch,
                    base_branch,
                } => self.repo_service.plan_new_worktree(
                    &repo,
                    &self.paths.worktrees_root,
                    branch,
                    base_branch,
                )?,
                WorkstreamRequest::Existing { branch } => self
                    .repo_service
                    .plan_existing_worktree(&repo, &self.paths.worktrees_root, branch)?,
            };

        if self
            .state
            .workstreams
            .iter()
            .any(|existing| existing.worktree_path == plan.worktree_path)
        {
            bail!(
                "checkout already tracked as a workstream: {}",
                plan.worktree_path.display()
            );
        }

        self.run_logged(
            &repo.display_name,
            "git.worktree.create",
            format!(
                "create `{}` at {}",
                plan.branch,
                plan.worktree_path.display()
            ),
            || self.repo_service.create_worktree(&repo, &plan),
        )?;

        if matches!(request, WorkstreamRequest::New { .. })
            && let Err(error) = self.configure_push_target(&repo, &plan.branch, &plan.worktree_path)
        {
            if matches!(plan.worktree_ownership, WorktreeOwnership::Managed) {
                let _ = self
                    .repo_service
                    .remove_worktree(&repo, &plan.worktree_path);
            }
            return Err(error);
        }

        let session = self.new_session_record(&repo, &plan.branch, agent, 1);
        let create_request =
            self.session_create_request(&repo, &plan.branch, &plan.worktree_path, &session, 1)?;

        if let Err(error) = self.run_logged(
            &plan.branch,
            "tmux.session.create",
            format!("launch {} in {}", agent, plan.worktree_path.display()),
            || self.session_backend.create_session(&create_request),
        ) {
            if matches!(plan.worktree_ownership, WorktreeOwnership::Managed) {
                let _ = self
                    .repo_service
                    .remove_worktree(&repo, &plan.worktree_path);
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
            attention: WorkstreamAttention::None,
            pinned: false,
            created_at_epoch_secs: now_epoch_secs(),
            status_changed_at_epoch_secs: now_epoch_secs(),
            last_opened_at_epoch_secs: None,
            last_attached_at_epoch_secs: None,
            sessions: vec![session.clone()],
            preferred_session_id: Some(session.id),
        };
        workstream.sync_legacy_summary();

        self.state.workstreams.push(workstream.clone());
        self.save_state()?;
        self.launch_session(&session, launch_mode)?;
        Ok(workstream)
    }

    pub fn open_workstream(&mut self, workstream_id: Uuid) -> anyhow::Result<()> {
        let session_id = self.preferred_session_id(workstream_id)?;
        self.open_workstream_session(workstream_id, session_id)
    }

    pub fn open_workstream_session(
        &mut self,
        workstream_id: Uuid,
        session_id: Uuid,
    ) -> anyhow::Result<()> {
        let (_, session) = self.ensure_workstream_session(workstream_id, session_id)?;
        self.mark_session_opened(workstream_id, session_id)?;
        self.terminal.open_session(
            &session.session_name,
            &self.session_backend.attach_command(&session.session_name),
        )
    }

    pub fn attach_workstream(&mut self, workstream_id: Uuid) -> anyhow::Result<()> {
        let session_id = self.preferred_session_id(workstream_id)?;
        self.attach_workstream_session(workstream_id, session_id)
    }

    pub fn attach_workstream_session(
        &mut self,
        workstream_id: Uuid,
        session_id: Uuid,
    ) -> anyhow::Result<()> {
        let (_, session) = self.ensure_workstream_session(workstream_id, session_id)?;
        self.mark_session_attached(workstream_id, session_id)?;
        self.session_backend.attach(&session.session_name)
    }

    pub fn recent_workstream_output(
        &self,
        workstream_id: Uuid,
        lines: usize,
    ) -> anyhow::Result<Vec<String>> {
        let session = self
            .workstream_by_id(workstream_id)?
            .preferred_session()
            .cloned()
            .context("workstream did not have a preferred session")?;
        self.recent_workstream_output_for_session(workstream_id, session.id, lines)
    }

    pub fn recent_workstream_output_for_session(
        &self,
        workstream_id: Uuid,
        session_id: Uuid,
        lines: usize,
    ) -> anyhow::Result<Vec<String>> {
        let workstream = self.workstream_by_id(workstream_id)?;
        let session = workstream
            .session_by_id(session_id)
            .context("no session matched the requested workstream session")?;

        if !self.session_backend.has_session(&session.session_name) {
            return Ok(vec!["session not running".to_string()]);
        }

        let lines = self
            .session_backend
            .capture_recent_lines(&session.session_name, lines)?;

        if lines.is_empty() {
            Ok(vec!["no recent pane output".to_string()])
        } else {
            Ok(lines)
        }
    }

    pub fn open_workstream_in_vscode(&self, workstream_id: Uuid) -> anyhow::Result<()> {
        let workstream = self.workstream_by_id(workstream_id)?;
        open_in_vscode(&workstream.worktree_path)
    }

    pub fn resume_workstream(
        &mut self,
        workstream_id: Uuid,
        launch_mode: LaunchMode,
    ) -> anyhow::Result<Workstream> {
        let session_id = self.preferred_session_id(workstream_id)?;
        self.resume_workstream_session(workstream_id, session_id, launch_mode)?;
        Ok(self.workstream_by_id(workstream_id)?.clone())
    }

    pub fn resume_workstream_session(
        &mut self,
        workstream_id: Uuid,
        session_id: Uuid,
        launch_mode: LaunchMode,
    ) -> anyhow::Result<WorkstreamSession> {
        let (_, session) = self.ensure_workstream_session(workstream_id, session_id)?;
        self.launch_session(&session, launch_mode)?;
        Ok(session)
    }

    pub fn create_workstream_session(
        &mut self,
        workstream_id: Uuid,
        agent: &str,
        launch_mode: LaunchMode,
    ) -> anyhow::Result<WorkstreamSession> {
        let workstream = self.workstream_by_id(workstream_id)?.clone();
        let repo = self.repo_by_id(workstream.repo_id)?.clone();
        let ordinal = workstream.session_count() + 1;
        let session = self.new_session_record(&repo, &workstream.branch, agent, ordinal);
        let request = self.session_create_request(
            &repo,
            &workstream.branch,
            &workstream.worktree_path,
            &session,
            ordinal,
        )?;

        self.run_logged(
            &workstream.branch,
            "tmux.session.create",
            format!("launch {} session {}", agent, ordinal),
            || self.session_backend.create_session(&request),
        )?;

        {
            let target = self.workstream_by_id_mut(workstream_id)?;
            target.add_session(session.clone(), true);
        }
        self.save_state()?;
        self.launch_session(&session, launch_mode)?;
        Ok(session)
    }

    pub fn run_workstream_one_off(
        &mut self,
        workstream_id: Uuid,
        agent: &str,
        prompt: &str,
    ) -> anyhow::Result<String> {
        let workstream = self.workstream_by_id(workstream_id)?.clone();
        let preset = self.agent_preset(agent)?;
        let command_template = preset
            .one_off_command
            .clone()
            .with_context(|| format!("agent `{agent}` does not support one-off runs"))?;
        let result = self.run_logged(
            &workstream.branch,
            "agent.one-off",
            format!(
                "run {} prompt in {}",
                agent,
                workstream.worktree_path.display()
            ),
            || {
                self.command_runner.run_agent_one_off(&AgentOneOffRequest {
                    preset_name: agent.to_string(),
                    command_template,
                    prompt: prompt.to_string(),
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
        Ok(output)
    }

    pub fn set_preferred_session(
        &mut self,
        workstream_id: Uuid,
        session_id: Uuid,
    ) -> anyhow::Result<()> {
        {
            let target = self.workstream_by_id_mut(workstream_id)?;
            if target.session_by_id(session_id).is_none() {
                bail!("no session matched `{session_id}` for `{}`", target.branch);
            }
            target.preferred_session_id = Some(session_id);
            target.sync_legacy_summary();
        }
        self.save_state()?;
        Ok(())
    }

    pub fn recent_operations(&self, limit: usize) -> anyhow::Result<Vec<OperationEvent>> {
        self.operation_log.recent(limit)
    }

    pub fn workstream_sessions(
        &self,
        workstream_id: Uuid,
    ) -> anyhow::Result<Vec<WorkstreamSession>> {
        Ok(self.workstream_by_id(workstream_id)?.sessions.clone())
    }

    pub fn apply_branch_updates(
        &mut self,
        branch_updates: Vec<(Uuid, String)>,
    ) -> anyhow::Result<()> {
        let mut changed = false;
        for (workstream_id, branch) in branch_updates {
            let Ok(target) = self.workstream_by_id_mut(workstream_id) else {
                continue;
            };
            if target.branch != branch {
                target.branch = branch;
                changed = true;
            }
        }
        if changed {
            self.save_state()?;
        }
        Ok(())
    }

    pub fn apply_created_workstream(&mut self, workstream: Workstream) -> anyhow::Result<()> {
        if self
            .state
            .workstreams
            .iter()
            .any(|existing| existing.id == workstream.id)
        {
            return Ok(());
        }
        self.state.workstreams.push(workstream);
        self.save_state()
    }

    pub fn apply_created_workstream_session(
        &mut self,
        workstream_id: Uuid,
        session: WorkstreamSession,
    ) -> anyhow::Result<()> {
        let target = self.workstream_by_id_mut(workstream_id)?;
        if target.session_by_id(session.id).is_none() {
            target.add_session(session, true);
            self.save_state()?;
        }
        Ok(())
    }

    pub fn apply_resumed_workstream_session(
        &mut self,
        workstream_id: Uuid,
        session_id: Uuid,
    ) -> anyhow::Result<()> {
        let target = self.workstream_by_id_mut(workstream_id)?;
        if let Some(session) = target.session_by_id_mut(session_id) {
            session.status = WorkstreamStatus::Running;
            session.status_changed_at_epoch_secs = now_epoch_secs();
        }
        target.preferred_session_id = Some(session_id);
        target.sync_legacy_summary();
        self.save_state()
    }

    pub fn apply_stopped_workstream(&mut self, workstream_id: Uuid) -> anyhow::Result<String> {
        let branch = {
            let target = self.workstream_by_id_mut(workstream_id)?;
            for session in &mut target.sessions {
                session.status = WorkstreamStatus::Stopped;
                session.status_changed_at_epoch_secs = now_epoch_secs();
            }
            target.sync_legacy_summary();
            target.branch.clone()
        };
        self.save_state()?;
        Ok(branch)
    }

    pub fn apply_removed_workstream(&mut self, workstream_id: Uuid) -> anyhow::Result<String> {
        let branch = self.workstream_by_id(workstream_id)?.branch.clone();
        self.state
            .workstreams
            .retain(|candidate| candidate.id != workstream_id);
        self.save_state()?;
        Ok(branch)
    }

    pub fn apply_persisted_completion(
        &mut self,
        result: PersistedTaskResult,
    ) -> anyhow::Result<()> {
        match result {
            PersistedTaskResult::RepoRefreshed { branch_updates, .. } => {
                self.apply_branch_updates(branch_updates)
            }
            PersistedTaskResult::WorkstreamCreated { workstream } => {
                self.apply_created_workstream(workstream)
            }
            PersistedTaskResult::WorkstreamSessionCreated {
                workstream_id,
                session,
            } => {
                if self.workstream_by_id(workstream_id).is_ok() {
                    self.apply_created_workstream_session(workstream_id, session)?;
                }
                Ok(())
            }
            PersistedTaskResult::WorkstreamSessionResumed {
                workstream_id,
                session_id,
            } => {
                if let Ok(workstream) = self.workstream_by_id(workstream_id)
                    && workstream.session_by_id(session_id).is_some()
                {
                    self.apply_resumed_workstream_session(workstream_id, session_id)?;
                }
                Ok(())
            }
            PersistedTaskResult::WorkstreamStopped { workstream_id } => {
                if self.workstream_by_id(workstream_id).is_ok() {
                    let _ = self.apply_stopped_workstream(workstream_id)?;
                }
                Ok(())
            }
            PersistedTaskResult::WorkstreamRemoved { workstream_id } => {
                if self.workstream_by_id(workstream_id).is_ok() {
                    let _ = self.apply_removed_workstream(workstream_id)?;
                }
                Ok(())
            }
        }
    }

    pub fn preferred_session_id(&self, workstream_id: Uuid) -> anyhow::Result<Uuid> {
        self.workstream_by_id(workstream_id)?
            .preferred_session()
            .map(|session| session.id)
            .context("workstream did not have a preferred session")
    }

    pub fn stop_workstream(&mut self, workstream_id: Uuid) -> anyhow::Result<String> {
        let (session_names, branch) = {
            let target = self.workstream_by_id_mut(workstream_id)?;
            for session in &mut target.sessions {
                session.status = WorkstreamStatus::Stopped;
                session.status_changed_at_epoch_secs = now_epoch_secs();
            }
            target.sync_legacy_summary();
            (
                target
                    .sessions
                    .iter()
                    .map(|session| session.session_name.clone())
                    .collect::<Vec<_>>(),
                target.branch.clone(),
            )
        };
        for session_name in session_names {
            let _ = self.session_backend.stop(&session_name);
        }
        self.save_state()?;
        Ok(branch)
    }

    pub fn remove_workstream(&mut self, workstream_id: Uuid) -> anyhow::Result<String> {
        let target = self.workstream_by_id(workstream_id)?.clone();
        let repo = self
            .state
            .repos
            .iter()
            .find(|repo| repo.id == target.repo_id)
            .cloned()
            .with_context(|| {
                format!(
                    "repo for workstream `{}` is no longer tracked",
                    target.branch
                )
            })?;

        for session in &target.sessions {
            let _ = self.session_backend.stop(&session.session_name);
        }
        if matches!(target.worktree_ownership, WorktreeOwnership::Managed) {
            self.repo_service
                .remove_worktree(&repo, &target.worktree_path)?;
        }
        self.state
            .workstreams
            .retain(|candidate| candidate.id != target.id);
        self.save_state()?;
        Ok(target.branch)
    }

    pub fn set_workstream_attention(
        &mut self,
        workstream_id: Uuid,
        attention: WorkstreamAttention,
    ) -> anyhow::Result<String> {
        let branch = {
            let target = self.workstream_by_id_mut(workstream_id)?;
            target.attention = attention.clone();
            target.branch.clone()
        };
        self.save_state()?;
        Ok(branch)
    }

    pub fn clear_workstream_attention(&mut self, workstream_id: Uuid) -> anyhow::Result<String> {
        let branch = {
            let target = self.workstream_by_id_mut(workstream_id)?;
            target.attention = WorkstreamAttention::None;
            target.branch.clone()
        };
        self.save_state()?;
        Ok(branch)
    }

    pub fn toggle_workstream_pinned(
        &mut self,
        workstream_id: Uuid,
    ) -> anyhow::Result<(String, bool)> {
        let result = {
            let target = self.workstream_by_id_mut(workstream_id)?;
            target.pinned = !target.pinned;
            (target.branch.clone(), target.pinned)
        };
        self.save_state()?;
        Ok(result)
    }

    pub fn set_workstream_pinned(
        &mut self,
        workstream_id: Uuid,
        pinned: bool,
    ) -> anyhow::Result<String> {
        let branch = {
            let target = self.workstream_by_id_mut(workstream_id)?;
            target.pinned = pinned;
            target.branch.clone()
        };
        self.save_state()?;
        Ok(branch)
    }

    pub fn repo_by_id(&self, repo_id: Uuid) -> anyhow::Result<&RepoTarget> {
        self.state
            .repos
            .iter()
            .find(|repo| repo.id == repo_id)
            .with_context(|| format!("no tracked repository matched `{repo_id}`"))
    }

    pub fn workstream_by_id(&self, workstream_id: Uuid) -> anyhow::Result<&Workstream> {
        self.state
            .workstreams
            .iter()
            .find(|workstream| workstream.id == workstream_id)
            .with_context(|| format!("no workstream matched `{workstream_id}`"))
    }

    pub fn workstream_by_id_mut(&mut self, workstream_id: Uuid) -> anyhow::Result<&mut Workstream> {
        self.state
            .workstreams
            .iter_mut()
            .find(|workstream| workstream.id == workstream_id)
            .with_context(|| format!("no workstream matched `{workstream_id}`"))
    }

    pub fn find_repo_id(&self, needle: &str) -> anyhow::Result<Uuid> {
        let needle_path = PathBuf::from(needle);
        let matches = self
            .state
            .repos
            .iter()
            .filter(|repo| {
                repo.id.to_string().starts_with(needle)
                    || repo.display_name == needle
                    || repo.slug == needle
                    || repo.path == needle_path
            })
            .map(|repo| repo.id)
            .collect::<Vec<_>>();

        match matches.as_slice() {
            [] => bail!("no tracked repository matched `{needle}`"),
            [repo_id] => Ok(*repo_id),
            _ => bail!("multiple repositories matched `{needle}`; use the full id"),
        }
    }

    pub fn find_workstream_id(&self, needle: &str) -> anyhow::Result<Uuid> {
        let matches = self
            .state
            .workstreams
            .iter()
            .filter(|workstream| {
                workstream.id.to_string().starts_with(needle)
                    || workstream.branch == needle
                    || workstream
                        .sessions
                        .iter()
                        .any(|session| session.session_name == needle)
            })
            .map(|workstream| workstream.id)
            .collect::<Vec<_>>();

        match matches.as_slice() {
            [] => bail!("no workstream matched `{needle}`"),
            [workstream_id] => Ok(*workstream_id),
            _ => bail!("multiple workstreams matched `{needle}`; use the full id"),
        }
    }

    fn launch_session(
        &self,
        session: &WorkstreamSession,
        launch_mode: LaunchMode,
    ) -> anyhow::Result<()> {
        match launch_mode {
            LaunchMode::Stay => Ok(()),
            LaunchMode::Open => self.terminal.open_session(
                &session.session_name,
                &self.session_backend.attach_command(&session.session_name),
            ),
            LaunchMode::Attach => self.session_backend.attach(&session.session_name),
        }
    }

    fn agent_preset(&self, agent: &str) -> anyhow::Result<&crate::domain::model::AgentPreset> {
        self.config
            .agent_presets
            .iter()
            .find(|preset| preset.name == agent)
            .with_context(|| format!("unknown agent preset `{agent}`"))
    }

    fn agent_command(&self, agent: &str) -> anyhow::Result<&str> {
        Ok(self.agent_preset(agent)?.command.as_str())
    }

    pub fn reconcile_workstream_sessions(&mut self) -> anyhow::Result<()> {
        let mut changed = false;

        for workstream in &mut self.state.workstreams {
            workstream.ensure_session_inventory();
            for session in &mut workstream.sessions {
                let has_session = self.session_backend.has_session(&session.session_name);
                match (&session.status, has_session) {
                    (WorkstreamStatus::Running, false) => {
                        session.status = WorkstreamStatus::Stopped;
                        session.status_changed_at_epoch_secs = now_epoch_secs();
                        changed = true;
                    }
                    (WorkstreamStatus::Stopped, true) => {
                        session.status = WorkstreamStatus::Running;
                        session.status_changed_at_epoch_secs = now_epoch_secs();
                        changed = true;
                    }
                    _ => {}
                }
            }
            workstream.sync_legacy_summary();
        }

        if changed {
            self.save_state()?;
        }

        Ok(())
    }

    fn hydrate_workstream_metadata(&mut self) -> anyhow::Result<()> {
        let mut changed = false;
        let now = now_epoch_secs();

        for workstream in &mut self.state.workstreams {
            workstream.ensure_session_inventory();
            if workstream.created_at_epoch_secs == 0 {
                workstream.created_at_epoch_secs = now;
                changed = true;
            }

            if workstream.status_changed_at_epoch_secs == 0 {
                workstream.status_changed_at_epoch_secs = now;
                changed = true;
            }

            for session in &mut workstream.sessions {
                if session.created_at_epoch_secs == 0 {
                    session.created_at_epoch_secs = workstream.created_at_epoch_secs;
                    changed = true;
                }
                if session.status_changed_at_epoch_secs == 0 {
                    session.status_changed_at_epoch_secs = workstream.status_changed_at_epoch_secs;
                    changed = true;
                }
            }
            workstream.sync_legacy_summary();
        }

        if changed {
            self.save_state()?;
        }

        Ok(())
    }

    fn reconcile_workstream_branches(&mut self) -> anyhow::Result<()> {
        let mut changed = false;

        for workstream in &mut self.state.workstreams {
            if !workstream.worktree_path.exists() {
                continue;
            }

            let Ok(Some(branch)) = self.repo_service.current_branch(&workstream.worktree_path)
            else {
                continue;
            };

            if branch != workstream.branch {
                workstream.branch = branch;
                changed = true;
            }
        }

        if changed {
            self.save_state()?;
        }

        Ok(())
    }

    fn hydrate_agent_presets(&mut self) -> anyhow::Result<()> {
        let mut changed = false;

        for preset in default_agent_presets() {
            if self
                .config
                .agent_presets
                .iter()
                .any(|existing| existing.name == preset.name)
            {
                continue;
            }

            self.config.agent_presets.push(preset);
            changed = true;
        }

        if changed {
            self.store.save_config(&self.config)?;
        }

        Ok(())
    }

    fn reconcile_background_completions(&mut self) -> anyhow::Result<()> {
        let completions = self.completion_store.load()?;
        for completion in completions {
            match self.apply_persisted_completion(completion.result.clone()) {
                Ok(()) => {
                    self.completion_store.remove(completion.id)?;
                }
                Err(error) => {
                    self.record_operation(
                        OperationLevel::Warning,
                        "runtime",
                        "background.completion.reconcile",
                        format!("failed to apply completion {}: {error}", completion.id),
                    );
                }
            }
        }
        Ok(())
    }

    fn save_state(&self) -> anyhow::Result<()> {
        self.store.save_state(&self.state)
    }

    fn ensure_workstream_session(
        &mut self,
        workstream_id: Uuid,
        session_id: Uuid,
    ) -> anyhow::Result<(Workstream, WorkstreamSession)> {
        let workstream = self.workstream_by_id(workstream_id)?.clone();
        let session = workstream
            .session_by_id(session_id)
            .cloned()
            .context("no session matched the requested workstream session")?;

        if self.session_backend.has_session(&session.session_name) {
            self.sync_session_metadata(&workstream, &session)?;
            if !matches!(session.status, WorkstreamStatus::Running) {
                let target = self.workstream_by_id_mut(workstream_id)?;
                if let Some(session) = target.session_by_id_mut(session_id) {
                    session.status = WorkstreamStatus::Running;
                    session.status_changed_at_epoch_secs = now_epoch_secs();
                }
                target.sync_legacy_summary();
                self.save_state()?;
                return Ok((
                    self.workstream_by_id(workstream_id)?.clone(),
                    self.workstream_by_id(workstream_id)?
                        .session_by_id(session_id)
                        .cloned()
                        .context("session disappeared after refresh")?,
                ));
            }

            return Ok((workstream, session));
        }

        if !workstream.worktree_path.exists() {
            bail!(
                "worktree path is missing for `{}`: {}",
                workstream.branch,
                workstream.worktree_path.display()
            );
        }

        let repo = self.repo_by_id(workstream.repo_id)?.clone();
        let ordinal = workstream
            .sessions
            .iter()
            .position(|candidate| candidate.id == session_id)
            .map(|index| index + 1)
            .unwrap_or(1);
        let create_request = self.session_create_request(
            &repo,
            &workstream.branch,
            &workstream.worktree_path,
            &session,
            ordinal,
        )?;
        self.run_logged(
            &workstream.branch,
            "tmux.session.recreate",
            format!("restore {}", session.session_name),
            || self.session_backend.create_session(&create_request),
        )?;

        let target = self.workstream_by_id_mut(workstream_id)?;
        if let Some(session) = target.session_by_id_mut(session_id) {
            session.status = WorkstreamStatus::Running;
            session.status_changed_at_epoch_secs = now_epoch_secs();
        }
        target.sync_legacy_summary();
        self.save_state()?;
        Ok((
            self.workstream_by_id(workstream_id)?.clone(),
            self.workstream_by_id(workstream_id)?
                .session_by_id(session_id)
                .cloned()
                .context("session disappeared after recreate")?,
        ))
    }

    fn session_create_request(
        &self,
        repo: &RepoTarget,
        branch: &str,
        worktree_path: &Path,
        session: &WorkstreamSession,
        ordinal: usize,
    ) -> anyhow::Result<SessionCreateRequest> {
        Ok(SessionCreateRequest {
            session_name: session.session_name.clone(),
            working_dir: worktree_path.to_path_buf(),
            startup_command: self.agent_command(&session.agent_preset)?.to_string(),
            label: SessionLabel {
                repo_name: repo.display_name.clone(),
                workstream_branch: branch.to_string(),
                agent_preset: session.agent_preset.clone(),
                session_ordinal: ordinal,
            },
        })
    }

    fn sync_session_metadata(
        &self,
        workstream: &Workstream,
        session: &WorkstreamSession,
    ) -> anyhow::Result<()> {
        let repo = self.repo_by_id(workstream.repo_id)?.clone();
        let ordinal = workstream
            .sessions
            .iter()
            .position(|candidate| candidate.id == session.id)
            .map(|index| index + 1)
            .unwrap_or(1);
        let request = self.session_create_request(
            &repo,
            &workstream.branch,
            &workstream.worktree_path,
            session,
            ordinal,
        )?;
        self.run_logged(
            &workstream.branch,
            "tmux.session.sync",
            format!("refresh {}", session.session_name),
            || self.session_backend.sync_session(&request),
        )
    }

    fn new_session_record(
        &self,
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

    fn mark_session_opened(&mut self, workstream_id: Uuid, session_id: Uuid) -> anyhow::Result<()> {
        {
            let target = self.workstream_by_id_mut(workstream_id)?;
            if let Some(session) = target.session_by_id_mut(session_id) {
                session.last_opened_at_epoch_secs = Some(now_epoch_secs());
            }
            target.preferred_session_id = Some(session_id);
            target.sync_legacy_summary();
        }
        self.save_state()
    }

    fn mark_session_attached(
        &mut self,
        workstream_id: Uuid,
        session_id: Uuid,
    ) -> anyhow::Result<()> {
        {
            let target = self.workstream_by_id_mut(workstream_id)?;
            if let Some(session) = target.session_by_id_mut(session_id) {
                session.last_attached_at_epoch_secs = Some(now_epoch_secs());
            }
            target.preferred_session_id = Some(session_id);
            target.sync_legacy_summary();
        }
        self.save_state()
    }

    fn configure_push_target(
        &self,
        repo: &RepoTarget,
        branch: &str,
        worktree_path: &Path,
    ) -> anyhow::Result<()> {
        let remote = self.repo_service.default_remote(repo)?;
        self.run_logged(
            branch,
            "git.push-target",
            format!("configure upstream on {}", worktree_path.display()),
            || {
                self.repo_service
                    .configure_push_target(worktree_path, branch, &remote)
            },
        )
    }

    fn fetch_latest_best_effort(&self, repo: &RepoTarget, purpose: &str) {
        if let Err(error) = self.run_logged(
            &repo.display_name,
            "git.fetch",
            format!("{purpose} using {}", repo.path.display()),
            || self.repo_service.fetch_latest(repo),
        ) {
            self.record_operation(
                OperationLevel::Warning,
                repo.display_name.as_str(),
                "git.fetch.warning",
                format!("continuing with cached refs: {error}"),
            );
        }
    }

    fn run_logged<T, F>(
        &self,
        scope: &str,
        action: &str,
        detail: String,
        operation: F,
    ) -> anyhow::Result<T>
    where
        F: FnOnce() -> anyhow::Result<T>,
    {
        self.record_operation(OperationLevel::Started, scope, action, detail.clone());
        match operation() {
            Ok(value) => {
                self.record_operation(OperationLevel::Succeeded, scope, action, detail);
                Ok(value)
            }
            Err(error) => {
                self.record_operation(
                    OperationLevel::Failed,
                    scope,
                    action,
                    format!("{detail}: {error}"),
                );
                Err(error)
            }
        }
    }

    fn record_operation(&self, level: OperationLevel, scope: &str, action: &str, detail: String) {
        let _ = self.operation_log.record(&OperationEvent {
            timestamp_epoch_secs: now_epoch_secs(),
            level,
            scope: scope.to_string(),
            action: action.to_string(),
            detail,
        });
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

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn open_in_vscode(path: &std::path::Path) -> anyhow::Result<()> {
    let status = Command::new("code")
        .arg(path)
        .status()
        .with_context(|| format!("failed to run `code {}`", path.display()))?;

    if status.success() {
        Ok(())
    } else {
        bail!("`code {}` exited with status {status}", path.display())
    }
}

#[cfg(test)]
mod tests;
