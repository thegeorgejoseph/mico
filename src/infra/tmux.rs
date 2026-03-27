use std::{
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{Context, bail};

use crate::app::ports::SessionBackend;

#[derive(Debug, Default, Clone)]
pub struct TmuxSessionBackend;

impl TmuxSessionBackend {
    pub fn new() -> Self {
        Self
    }

    fn status(args: &[&str]) -> anyhow::Result<()> {
        let output = Command::new("tmux")
            .args(args)
            .output()
            .with_context(|| format!("failed to run `tmux {}`", args.join(" ")))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            bail!("tmux {} failed: {detail}", args.join(" "))
        }
    }

    fn session_exists(session_name: &str) -> bool {
        Command::new("tmux")
            .args(["has-session", "-t", session_name])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn output(args: &[&str]) -> anyhow::Result<String> {
        let output = Command::new("tmux")
            .args(args)
            .output()
            .with_context(|| format!("failed to run `tmux {}`", args.join(" ")))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout)
                .trim_end()
                .to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            bail!("tmux {} failed: {detail}", args.join(" "))
        }
    }
}

impl SessionBackend for TmuxSessionBackend {
    fn create_session(
        &self,
        session_name: &str,
        working_dir: &Path,
        startup_command: &str,
    ) -> anyhow::Result<()> {
        if Self::session_exists(session_name) {
            bail!("tmux session `{session_name}` already exists")
        }

        let working_dir = working_dir.to_string_lossy().to_string();
        Self::status(&["new-session", "-d", "-s", session_name, "-c", &working_dir])?;
        Self::status(&["set-option", "-t", session_name, "remain-on-exit", "on"])?;

        if !startup_command.trim().is_empty() {
            Self::status(&["send-keys", "-t", session_name, startup_command, "C-m"])?;
        }

        Ok(())
    }

    fn has_session(&self, session_name: &str) -> bool {
        Self::session_exists(session_name)
    }

    fn attach(&self, session_name: &str) -> anyhow::Result<()> {
        let status = Command::new("tmux")
            .args(["attach", "-t", session_name])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .with_context(|| format!("failed to attach tmux session `{session_name}`"))?;

        if status.success() {
            Ok(())
        } else {
            bail!("`tmux attach -t {session_name}` exited with status {status}")
        }
    }

    fn stop(&self, session_name: &str) -> anyhow::Result<()> {
        Self::status(&["kill-session", "-t", session_name])
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
        lines: usize,
    ) -> anyhow::Result<Vec<String>> {
        let target = format!("{session_name}:0.0");
        let output = Self::output(&["capture-pane", "-p", "-J", "-t", &target])?;

        Ok(output
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.trim().is_empty())
            .rev()
            .take(lines)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(ToOwned::to_owned)
            .collect())
    }
}
