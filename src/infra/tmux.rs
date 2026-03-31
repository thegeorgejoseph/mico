use std::process::{Command, Stdio};

use anyhow::{Context, bail};

use crate::app::ports::{SessionBackend, SessionCreateRequest};

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

    fn status_left(request: &SessionCreateRequest) -> String {
        let session_suffix = if request.label.session_ordinal > 1 {
            format!("  #{}", request.label.session_ordinal)
        } else {
            String::new()
        };
        format!(
            " {}{}  {} ",
            request.label.workstream_branch, request.label.agent_preset, session_suffix
        )
    }

    fn configure_session(request: &SessionCreateRequest) -> anyhow::Result<()> {
        let status_left = Self::status_left(request);
        Self::status(&[
            "set-option",
            "-t",
            &request.session_name,
            "remain-on-exit",
            "on",
        ])?;
        Self::status(&["set-option", "-t", &request.session_name, "status", "on"])?;
        Self::status(&[
            "set-option",
            "-t",
            &request.session_name,
            "status-left",
            &status_left,
        ])?;
        Self::status(&[
            "set-option",
            "-t",
            &request.session_name,
            "status-right",
            " #{pane_current_path} | %H:%M ",
        ])?;
        Self::status(&[
            "set-option",
            "-t",
            &request.session_name,
            "allow-rename",
            "off",
        ])?;
        Self::status(&[
            "set-option",
            "-t",
            &request.session_name,
            "automatic-rename",
            "off",
        ])?;
        Self::status(&[
            "rename-window",
            "-t",
            &format!("{}:0", request.session_name),
            &request.label.workstream_branch,
        ])
    }
}

impl SessionBackend for TmuxSessionBackend {
    fn create_session(&self, request: &SessionCreateRequest) -> anyhow::Result<()> {
        if Self::session_exists(&request.session_name) {
            bail!("tmux session `{}` already exists", request.session_name)
        }

        let working_dir = request.working_dir.to_string_lossy().to_string();
        Self::status(&[
            "new-session",
            "-d",
            "-s",
            &request.session_name,
            "-c",
            &working_dir,
        ])?;
        Self::configure_session(request)?;

        if !request.startup_command.trim().is_empty() {
            Self::status(&[
                "send-keys",
                "-t",
                &request.session_name,
                &request.startup_command,
                "C-m",
            ])?;
        }

        Ok(())
    }

    fn sync_session(&self, request: &SessionCreateRequest) -> anyhow::Result<()> {
        if !Self::session_exists(&request.session_name) {
            bail!("tmux session `{}` does not exist", request.session_name)
        }

        Self::configure_session(request)
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
