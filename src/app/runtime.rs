use std::{env, path::PathBuf};

use anyhow::{Context, bail};
use uuid::Uuid;

use crate::{
    app::ports::{DependencyInspector, RepoService, SessionBackend, StateStore, TerminalFrontend},
    domain::model::{
        AppConfig, AppPaths, DoctorReport, RepoTarget, StoredState, Workstream, WorkstreamStatus,
    },
    infra::{
        deps::SystemDependencyInspector, git::GitCliRepoService, iterm::ITermFrontend,
        json_store::JsonFileStore, tmux::TmuxSessionBackend,
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
    store: JsonFileStore,
    repo_service: GitCliRepoService,
    session_backend: TmuxSessionBackend,
    terminal: ITermFrontend,
    dependency_inspector: SystemDependencyInspector,
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
        let dependency_inspector = SystemDependencyInspector::new(paths.clone());
        let report = dependency_inspector.doctor()?;

        let mut runtime = Self {
            paths,
            store,
            repo_service: GitCliRepoService::new(),
            session_backend: TmuxSessionBackend::new(),
            terminal: ITermFrontend::new(),
            dependency_inspector,
            config,
            state,
            report,
        };

        runtime.reconcile_workstream_sessions()?;
        Ok(runtime)
    }

    pub fn refresh_doctor(&mut self) -> anyhow::Result<()> {
        self.report = self.dependency_inspector.doctor()?;
        Ok(())
    }

    pub fn refresh_repo(&mut self, repo_id: Uuid) -> anyhow::Result<()> {
        let repo = self.repo_by_id(repo_id)?.clone();
        self.repo_service.fetch_latest(&repo)
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
        self.repo_service.fetch_latest(&repo)?;
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
        let repo = self.repo_by_id(repo_id)?.clone();
        let startup_command = self.agent_command(agent)?.to_string();
        self.repo_service.fetch_latest(&repo)?;
        let plan = self.repo_service.plan_new_worktree(
            &repo,
            &self.paths.worktrees_root,
            branch,
            base_branch,
        )?;
        self.repo_service.create_worktree(&repo, &plan)?;

        let session_name = session_name(&repo.slug, branch);
        let created = self.session_backend.create_session(
            &session_name,
            &plan.worktree_path,
            &startup_command,
        );

        if let Err(error) = created {
            let _ = self
                .repo_service
                .remove_worktree(&repo, &plan.worktree_path);
            return Err(error);
        }

        let workstream = Workstream {
            id: Uuid::new_v4(),
            repo_id: repo.id,
            base_branch: base_branch.to_string(),
            branch: branch.to_string(),
            worktree_path: plan.worktree_path.clone(),
            session_name,
            agent_preset: agent.to_string(),
            status: WorkstreamStatus::Running,
        };

        self.state.workstreams.push(workstream.clone());
        self.save_state()?;
        self.launch_workstream(&workstream, launch_mode)?;
        Ok(workstream)
    }

    pub fn create_workstream_existing(
        &mut self,
        repo_id: Uuid,
        branch: &str,
        agent: &str,
        launch_mode: LaunchMode,
    ) -> anyhow::Result<Workstream> {
        let repo = self.repo_by_id(repo_id)?.clone();
        let startup_command = self.agent_command(agent)?.to_string();
        self.repo_service.fetch_latest(&repo)?;
        let plan =
            self.repo_service
                .plan_existing_worktree(&repo, &self.paths.worktrees_root, branch)?;
        self.repo_service.create_worktree(&repo, &plan)?;

        let session_name = session_name(&repo.slug, branch);
        let created = self.session_backend.create_session(
            &session_name,
            &plan.worktree_path,
            &startup_command,
        );

        if let Err(error) = created {
            let _ = self
                .repo_service
                .remove_worktree(&repo, &plan.worktree_path);
            return Err(error);
        }

        let workstream = Workstream {
            id: Uuid::new_v4(),
            repo_id: repo.id,
            base_branch: branch.to_string(),
            branch: branch.to_string(),
            worktree_path: plan.worktree_path.clone(),
            session_name,
            agent_preset: agent.to_string(),
            status: WorkstreamStatus::Running,
        };

        self.state.workstreams.push(workstream.clone());
        self.save_state()?;
        self.launch_workstream(&workstream, launch_mode)?;
        Ok(workstream)
    }

    pub fn open_workstream(&mut self, workstream_id: Uuid) -> anyhow::Result<()> {
        let workstream = self.ensure_workstream_session(workstream_id)?;
        self.terminal.open_session(
            &workstream.session_name,
            &self
                .session_backend
                .attach_command(&workstream.session_name),
        )
    }

    pub fn attach_workstream(&mut self, workstream_id: Uuid) -> anyhow::Result<()> {
        let workstream = self.ensure_workstream_session(workstream_id)?;
        self.session_backend.attach(&workstream.session_name)
    }

    pub fn resume_workstream(
        &mut self,
        workstream_id: Uuid,
        launch_mode: LaunchMode,
    ) -> anyhow::Result<Workstream> {
        let workstream = self.ensure_workstream_session(workstream_id)?;
        self.launch_workstream(&workstream, launch_mode)?;
        Ok(workstream)
    }

    pub fn stop_workstream(&mut self, workstream_id: Uuid) -> anyhow::Result<String> {
        let (session_name, branch) = {
            let target = self.workstream_by_id_mut(workstream_id)?;
            target.status = WorkstreamStatus::Stopped;
            (target.session_name.clone(), target.branch.clone())
        };
        self.session_backend.stop(&session_name)?;
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

        let _ = self.session_backend.stop(&target.session_name);
        self.repo_service
            .remove_worktree(&repo, &target.worktree_path)?;
        self.state
            .workstreams
            .retain(|candidate| candidate.id != target.id);
        self.save_state()?;
        Ok(target.branch)
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
                    || workstream.session_name == needle
            })
            .map(|workstream| workstream.id)
            .collect::<Vec<_>>();

        match matches.as_slice() {
            [] => bail!("no workstream matched `{needle}`"),
            [workstream_id] => Ok(*workstream_id),
            _ => bail!("multiple workstreams matched `{needle}`; use the full id"),
        }
    }

    fn launch_workstream(
        &self,
        workstream: &Workstream,
        launch_mode: LaunchMode,
    ) -> anyhow::Result<()> {
        match launch_mode {
            LaunchMode::Stay => Ok(()),
            LaunchMode::Open => self.terminal.open_session(
                &workstream.session_name,
                &self
                    .session_backend
                    .attach_command(&workstream.session_name),
            ),
            LaunchMode::Attach => self.session_backend.attach(&workstream.session_name),
        }
    }

    fn agent_command(&self, agent: &str) -> anyhow::Result<&str> {
        self.config
            .agent_presets
            .iter()
            .find(|preset| preset.name == agent)
            .map(|preset| preset.command.as_str())
            .with_context(|| format!("unknown agent preset `{agent}`"))
    }

    pub fn reconcile_workstream_sessions(&mut self) -> anyhow::Result<()> {
        let mut changed = false;

        for workstream in &mut self.state.workstreams {
            let has_session = self.session_backend.has_session(&workstream.session_name);
            match (&workstream.status, has_session) {
                (WorkstreamStatus::Running, false) => {
                    workstream.status = WorkstreamStatus::Stopped;
                    changed = true;
                }
                (WorkstreamStatus::Stopped, true) => {
                    workstream.status = WorkstreamStatus::Running;
                    changed = true;
                }
                _ => {}
            }
        }

        if changed {
            self.save_state()?;
        }

        Ok(())
    }

    fn ensure_workstream_session(&mut self, workstream_id: Uuid) -> anyhow::Result<Workstream> {
        let workstream = self.workstream_by_id(workstream_id)?.clone();

        if self.session_backend.has_session(&workstream.session_name) {
            if !matches!(workstream.status, WorkstreamStatus::Running) {
                self.workstream_by_id_mut(workstream_id)?.status = WorkstreamStatus::Running;
                self.save_state()?;
                return Ok(self.workstream_by_id(workstream_id)?.clone());
            }

            return Ok(workstream);
        }

        if !workstream.worktree_path.exists() {
            bail!(
                "worktree path is missing for `{}`: {}",
                workstream.branch,
                workstream.worktree_path.display()
            );
        }

        let startup_command = self.agent_command(&workstream.agent_preset)?.to_string();
        self.session_backend.create_session(
            &workstream.session_name,
            &workstream.worktree_path,
            &startup_command,
        )?;

        self.workstream_by_id_mut(workstream_id)?.status = WorkstreamStatus::Running;
        self.save_state()?;
        Ok(self.workstream_by_id(workstream_id)?.clone())
    }

    fn save_state(&self) -> anyhow::Result<()> {
        self.store.save_state(&self.state)
    }
}

fn session_name(repo_slug: &str, branch: &str) -> String {
    let branch = branch
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_lowercase();
    let short = Uuid::new_v4().simple().to_string();
    let short = &short[..8];
    format!("mico-{repo_slug}-{branch}-{short}")
}
