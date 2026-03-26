use anyhow::Context;

use crate::{
    app::{
        cli::{Cli, Command, RepoCommand, WorkstreamCommand},
        ports::{ConfigStore, DependencyInspector, StateStore, Updater},
        runtime::{LaunchMode, MicoRuntime},
    },
    infra::{
        config::{default_config, default_state, resolve_paths},
        deps::SystemDependencyInspector,
        json_store::JsonFileStore,
        platform::SystemUpdater,
    },
    tui,
};

pub fn run(cli: Cli) -> anyhow::Result<()> {
    let command = cli.command.unwrap_or(Command::Dashboard);
    let paths = resolve_paths()?;
    let store = JsonFileStore::new(paths.clone());

    let config = store
        .load_or_create_config(default_config())
        .context("failed to load config")?;
    let state = store
        .load_or_create_state(default_state())
        .context("failed to load state")?;

    match command {
        Command::Dashboard => {
            let runtime = MicoRuntime::new(paths, store, config, state)?;
            tui::run_dashboard(runtime)
        }
        Command::Doctor { json } => {
            let report = SystemDependencyInspector::new(paths).doctor()?;

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("mico doctor");
                println!("  root: {}", report.paths.root.display());
                println!("  config: {}", report.paths.config_path.display());
                println!("  state: {}", report.paths.state_path.display());
                println!("  worktrees: {}", report.paths.worktrees_root.display());
                println!();

                for dependency in report.dependencies {
                    let status = if dependency.found { "ok" } else { "missing" };
                    println!(
                        "{:>10}: {:<7} {}",
                        dependency.name, status, dependency.detail
                    );
                }
            }

            Ok(())
        }
        Command::Paths => {
            println!("{}", serde_json::to_string_pretty(&paths)?);
            Ok(())
        }
        Command::Repo { command } => {
            let mut runtime = MicoRuntime::new(paths, store, config, state)?;
            run_repo_command(command, &mut runtime)
        }
        Command::Workstream { command } => {
            let mut runtime = MicoRuntime::new(paths, store, config, state)?;
            run_workstream_command(command, &mut runtime)
        }
        Command::Install => SystemUpdater::new().install_or_update(config.github_repo.as_deref()),
    }
}

fn run_repo_command(command: RepoCommand, runtime: &mut MicoRuntime) -> anyhow::Result<()> {
    match command {
        RepoCommand::Add { path, name } => {
            let repo = runtime.add_repo(path, name)?;
            println!(
                "Added repo `{}` at {}",
                repo.display_name,
                repo.path.display()
            );
            println!("  id:   {}", repo.id);
            println!("  slug: {}", repo.slug);
            Ok(())
        }
        RepoCommand::List => {
            if runtime.state.repos.is_empty() {
                println!("No repositories tracked yet. Use `mico repo add <path>`.");
                return Ok(());
            }

            for repo in &runtime.state.repos {
                println!("{}  {}  {}", repo.id, repo.slug, repo.path.display());
            }

            Ok(())
        }
        RepoCommand::Remove { repo } => {
            let repo_id = runtime.find_repo_id(&repo)?;
            let removed = runtime.remove_repo(repo_id)?;
            println!("Removed repo `{removed}`.");
            Ok(())
        }
        RepoCommand::Branches { repo } => {
            let repo_id = runtime.find_repo_id(&repo)?;
            let repo_name = runtime.repo_by_id(repo_id)?.display_name.clone();
            let branches = runtime.branches_for_repo(repo_id)?;

            if branches.is_empty() {
                println!("No branches found for `{repo_name}`.");
            } else {
                println!("Branches for `{repo_name}`:");
                for branch in branches {
                    println!("  {branch}");
                }
            }

            Ok(())
        }
        RepoCommand::Fetch { repo } => {
            let repo_id = runtime.find_repo_id(&repo)?;
            let repo_name = runtime.repo_by_id(repo_id)?.display_name.clone();
            runtime.refresh_repo(repo_id)?;
            println!("Fetched latest refs for `{repo_name}`.");
            Ok(())
        }
    }
}

fn run_workstream_command(
    command: WorkstreamCommand,
    runtime: &mut MicoRuntime,
) -> anyhow::Result<()> {
    match command {
        WorkstreamCommand::Create {
            repo,
            base,
            branch,
            agent,
            open,
            attach,
        } => {
            if open && attach {
                anyhow::bail!("choose either `--open` or `--attach`, not both");
            }

            let repo_id = runtime.find_repo_id(&repo)?;
            let repo_name = runtime.repo_by_id(repo_id)?.display_name.clone();
            let launch_mode = if open {
                LaunchMode::Open
            } else if attach {
                LaunchMode::Attach
            } else {
                LaunchMode::Stay
            };

            let workstream =
                runtime.create_workstream_new(repo_id, &base, &branch, &agent, launch_mode)?;

            println!(
                "Created workstream `{}` for `{}`.",
                workstream.branch, repo_name
            );
            println!("  id:       {}", workstream.id);
            println!("  session:  {}", workstream.session_name);
            println!("  worktree: {}", workstream.worktree_path.display());
            println!("  agent:    {}", workstream.agent_preset);

            if matches!(launch_mode, LaunchMode::Stay) {
                println!();
                println!("Next steps:");
                println!("  mico workstream open {}", workstream.id.simple());
                println!("  mico workstream attach {}", workstream.id.simple());
            }

            Ok(())
        }
        WorkstreamCommand::List => {
            if runtime.state.workstreams.is_empty() {
                println!("No workstreams yet. Use `mico workstream create ...`.");
                return Ok(());
            }

            for workstream in &runtime.state.workstreams {
                let repo_name = runtime
                    .state
                    .repos
                    .iter()
                    .find(|repo| repo.id == workstream.repo_id)
                    .map(|repo| repo.display_name.as_str())
                    .unwrap_or("<missing repo>");
                println!(
                    "{}  {}  {}  {}  {}",
                    workstream.id,
                    repo_name,
                    workstream.branch,
                    workstream.session_name,
                    workstream.worktree_path.display()
                );
            }

            Ok(())
        }
        WorkstreamCommand::Open { workstream } => {
            let workstream_id = runtime.find_workstream_id(&workstream)?;
            runtime.open_workstream(workstream_id)
        }
        WorkstreamCommand::Attach { workstream } => {
            let workstream_id = runtime.find_workstream_id(&workstream)?;
            runtime.attach_workstream(workstream_id)
        }
        WorkstreamCommand::Resume {
            workstream,
            open,
            attach,
        } => {
            if open && attach {
                anyhow::bail!("choose either `--open` or `--attach`, not both");
            }

            let workstream_id = runtime.find_workstream_id(&workstream)?;
            let launch_mode = if open {
                LaunchMode::Open
            } else if attach {
                LaunchMode::Attach
            } else {
                LaunchMode::Stay
            };
            let workstream = runtime.resume_workstream(workstream_id, launch_mode)?;
            println!(
                "Resumed workstream `{}` in {}.",
                workstream.branch,
                workstream.worktree_path.display()
            );
            Ok(())
        }
        WorkstreamCommand::Stop { workstream } => {
            let workstream_id = runtime.find_workstream_id(&workstream)?;
            let branch = runtime.stop_workstream(workstream_id)?;
            println!("Stopped workstream `{branch}`.");
            Ok(())
        }
        WorkstreamCommand::Remove { workstream } => {
            let workstream_id = runtime.find_workstream_id(&workstream)?;
            let branch = runtime.remove_workstream(workstream_id)?;
            println!("Removed workstream `{branch}`.");
            Ok(())
        }
    }
}
