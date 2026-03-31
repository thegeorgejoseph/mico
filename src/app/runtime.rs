use std::{
    env,
    path::Path,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, bail};
use uuid::Uuid;

use crate::{
    app::ports::{
        AgentOneOffRequest, CommandRunner, DependencyInspector, OperationLog, RepoService,
        RuntimeStore, SessionBackend, SessionCreateRequest, SessionLabel, TerminalFrontend,
    },
    domain::model::{
        AppConfig, AppPaths, DoctorReport, OperationEvent, OperationLevel, RepoTarget, StoredState,
        Workstream, WorkstreamAttention, WorkstreamRequest, WorkstreamSession, WorkstreamStatus,
        WorktreeOwnership,
    },
    infra::{
        config::default_agent_presets, deps::SystemDependencyInspector, git::GitCliRepoService,
        iterm::ITermFrontend, json_store::JsonFileStore, operations::JsonlOperationLog,
        process::ShellCommandRunner, tmux::TmuxSessionBackend,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum LaunchMode {
    Stay,
    Open,
    Attach,
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
        Self::with_interfaces(
            paths,
            Box::new(store),
            Box::new(GitCliRepoService::new()),
            Box::new(TmuxSessionBackend::new()),
            Box::new(ITermFrontend::new()),
            Box::new(SystemDependencyInspector::new(dependency_paths)),
            Box::new(ShellCommandRunner::new()),
            Box::new(JsonlOperationLog::new(operations_log_path)),
            config,
            state,
        )
    }

    pub fn with_interfaces(
        paths: AppPaths,
        store: Box<dyn RuntimeStore>,
        repo_service: Box<dyn RepoService>,
        session_backend: Box<dyn SessionBackend>,
        terminal: Box<dyn TerminalFrontend>,
        dependency_inspector: Box<dyn DependencyInspector>,
        command_runner: Box<dyn CommandRunner>,
        operation_log: Box<dyn OperationLog>,
        config: AppConfig,
        state: StoredState,
    ) -> anyhow::Result<Self> {
        let report = dependency_inspector.doctor()?;

        let mut runtime = Self {
            paths,
            store,
            repo_service,
            session_backend,
            terminal,
            dependency_inspector,
            command_runner,
            operation_log,
            config,
            state,
            report,
        };

        runtime.hydrate_agent_presets()?;
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
mod tests {
    use super::*;

    use std::{cell::RefCell, collections::HashMap, fs, rc::Rc};

    use crate::{
        app::ports::{AgentOneOffResult, ConfigStore, StateStore},
        domain::model::{AgentPreset, DependencyStatus, OperationEvent, WorktreePlan},
        infra::config::default_state,
    };
    use tempfile::TempDir;

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
        fn discover_repo(
            &self,
            path: &Path,
            display_name: Option<&str>,
        ) -> anyhow::Result<RepoTarget> {
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
        fn open_session(
            &self,
            _session_name: &str,
            _attach_command: &[String],
        ) -> anyhow::Result<()> {
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
        fn run_agent_one_off(
            &self,
            request: &AgentOneOffRequest,
        ) -> anyhow::Result<AgentOneOffResult> {
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

    struct RuntimeFixture {
        _temp: TempDir,
        runtime: MicoRuntime,
        repo_id: Uuid,
        repo_service: FakeRepoService,
        operation_log: FakeOperationLog,
    }

    fn make_fixture() -> anyhow::Result<RuntimeFixture> {
        let temp = TempDir::new()?;
        let paths = AppPaths {
            root: temp.path().join(".mico"),
            config_path: temp.path().join(".mico/config.json"),
            state_path: temp.path().join(".mico/state.json"),
            operations_log_path: temp.path().join(".mico/operations.jsonl"),
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
            Box::new(FakeStore::default()),
            Box::new(repo_service.clone()),
            Box::new(FakeSessionBackend::default()),
            Box::new(FakeTerminalFrontend),
            Box::new(FakeDependencyInspector { paths }),
            Box::new(FakeCommandRunner::default()),
            Box::new(operation_log.clone()),
            config,
            state,
        )?;

        Ok(RuntimeFixture {
            _temp: temp,
            runtime,
            repo_id: repo.id,
            repo_service,
            operation_log,
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

        let session = fixture.runtime.create_workstream_session(
            workstream.id,
            "opencode",
            LaunchMode::Stay,
        )?;
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
}
