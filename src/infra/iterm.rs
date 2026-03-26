use std::process::Command;

use anyhow::{Context, bail};

use crate::app::ports::TerminalFrontend;

#[derive(Debug, Default, Clone)]
pub struct ITermFrontend;

impl ITermFrontend {
    pub fn new() -> Self {
        Self
    }
}

impl TerminalFrontend for ITermFrontend {
    fn open_session(&self, session_name: &str, attach_command: &[String]) -> anyhow::Result<()> {
        let command = if attach_command.is_empty() {
            bail!("attach command was empty")
        } else {
            attach_command.join(" ")
        };

        let script_lines = [
            r#"tell application "iTerm""#.to_string(),
            "activate".to_string(),
            r#"if (count of windows) = 0 then"#.to_string(),
            r#"  create window with default profile"#.to_string(),
            r#"end if"#.to_string(),
            r#"tell current window"#.to_string(),
            r#"  create tab with default profile"#.to_string(),
            r#"  tell current session"#.to_string(),
            format!(r#"    write text "{}""#, command.replace('"', "\\\"")),
            r#"  end tell"#.to_string(),
            r#"end tell"#.to_string(),
            r#"end tell"#.to_string(),
        ];

        let mut osascript = Command::new("osascript");

        for line in script_lines {
            osascript.arg("-e").arg(line);
        }

        let status = osascript
            .status()
            .with_context(|| format!("failed to open session `{session_name}` in iTerm"))?;

        if status.success() {
            Ok(())
        } else {
            bail!("osascript exited with status {status}")
        }
    }
}
