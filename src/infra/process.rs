use std::process::Command;

use anyhow::{Context, bail};

use crate::app::ports::{AgentOneOffRequest, AgentOneOffResult, CommandRunner};

#[derive(Debug, Default, Clone)]
pub struct ShellCommandRunner;

impl ShellCommandRunner {
    pub fn new() -> Self {
        Self
    }

    fn escape_shell_arg(raw: &str) -> String {
        format!("'{}'", raw.replace('\'', "'\\''"))
    }

    fn render_template(template: &str, prompt: &str) -> String {
        let escaped_prompt = Self::escape_shell_arg(prompt);

        if template.contains("{prompt}") {
            template.replace("{prompt}", &escaped_prompt)
        } else if prompt.trim().is_empty() {
            template.to_string()
        } else {
            format!("{template} {escaped_prompt}")
        }
    }
}

impl CommandRunner for ShellCommandRunner {
    fn run_agent_one_off(&self, request: &AgentOneOffRequest) -> anyhow::Result<AgentOneOffResult> {
        let command = Self::render_template(&request.command_template, &request.prompt);
        let output = Command::new("zsh")
            .args(["-lc", &command])
            .current_dir(&request.working_dir)
            .output()
            .with_context(|| {
                format!(
                    "failed to run one-off `{}` in {}",
                    request.preset_name,
                    request.working_dir.display()
                )
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string();
        let stderr = String::from_utf8_lossy(&output.stderr)
            .trim_end()
            .to_string();

        if output.status.success() {
            Ok(AgentOneOffResult { stdout, stderr })
        } else {
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("process exited with {}", output.status)
            };
            bail!("{detail}")
        }
    }
}
