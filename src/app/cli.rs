use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "mico",
    version,
    about = "Mission control for local CLI AI agents on macOS"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Launch the TUI dashboard.
    Dashboard,
    /// Stream or print recent mico operations.
    Status {
        /// Follow the operations log for new events.
        #[arg(long)]
        follow: bool,
        /// Emit raw JSON lines.
        #[arg(long)]
        json: bool,
        /// Number of recent events to print.
        #[arg(long, default_value_t = 40)]
        lines: usize,
    },
    /// Print dependency and environment checks.
    Doctor {
        /// Emit the doctor report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Print the important local paths mico uses.
    Paths,
    /// Manage repositories tracked by mico.
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
    /// Manage workstreams backed by git worktrees and tmux sessions.
    Workstream {
        #[command(subcommand)]
        command: WorkstreamCommand,
    },
    /// Install or update mico using the configured distribution path.
    Install,
}

#[derive(Debug, Clone, Subcommand)]
pub enum RepoCommand {
    /// Add a repository to mico's local state.
    Add {
        /// Path to any directory inside the git repository.
        path: Option<PathBuf>,
        /// Optional custom display name.
        #[arg(long)]
        name: Option<String>,
    },
    /// List tracked repositories.
    List,
    /// Remove a repository from mico's local state.
    Remove {
        /// Repository id prefix, display name, slug, or path.
        repo: String,
    },
    /// List candidate branches for a tracked repository.
    Branches {
        /// Repository id prefix, display name, slug, or path.
        repo: String,
    },
    /// Fetch the latest remote refs for a tracked repository.
    Fetch {
        /// Repository id prefix, display name, slug, or path.
        repo: String,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum WorkstreamCommand {
    /// Create a workstream from either a new branch or an existing branch.
    Create {
        /// Repository id prefix, display name, slug, or path.
        #[arg(long)]
        repo: String,
        /// Branch name to use for the workstream.
        #[arg(long)]
        branch: String,
        /// Base branch to fetch and branch from when creating a new branch.
        #[arg(long, conflicts_with = "existing")]
        base: Option<String>,
        /// Use an existing local or remote branch as the workstream branch.
        #[arg(long)]
        existing: bool,
        /// Agent preset to launch inside the tmux session.
        #[arg(long, default_value = "claude")]
        agent: String,
        /// Open the new session in iTerm after creation.
        #[arg(long)]
        open: bool,
        /// Attach the new session in the current terminal after creation.
        #[arg(long)]
        attach: bool,
    },
    /// List tracked workstreams.
    List,
    /// Open a workstream in iTerm.
    Open {
        /// Workstream id prefix, branch, or session name.
        workstream: String,
    },
    /// Attach a workstream in the current terminal.
    Attach {
        /// Workstream id prefix, branch, or session name.
        workstream: String,
    },
    /// Resume a workstream by recreating its tmux session in the existing worktree.
    Resume {
        /// Workstream id prefix, branch, or session name.
        workstream: String,
        /// Open the resumed session in iTerm after recreation.
        #[arg(long)]
        open: bool,
        /// Attach the resumed session in the current terminal after recreation.
        #[arg(long)]
        attach: bool,
    },
    /// Stop a workstream's tmux session but keep its record and worktree.
    Stop {
        /// Workstream id prefix, branch, or session name.
        workstream: String,
    },
    /// Remove a workstream, its tmux session, and its git worktree.
    Remove {
        /// Workstream id prefix, branch, or session name.
        workstream: String,
    },
}
