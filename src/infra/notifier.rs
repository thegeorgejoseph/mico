use std::process::Command;

use anyhow::{Context, bail};

use crate::app::ports::{NotificationRequest, Notifier};

#[derive(Debug, Default, Clone)]
pub struct MacOsNotifier;

impl MacOsNotifier {
    pub fn new() -> Self {
        Self
    }
}

impl Notifier for MacOsNotifier {
    fn notify(&self, request: &NotificationRequest) -> anyhow::Result<()> {
        let script = format!(
            "display notification \"{}\" with title \"{}\" subtitle \"mico\"",
            request.body.replace('"', "\\\""),
            request.title.replace('"', "\\\""),
        );
        let status = Command::new("osascript")
            .args(["-e", &script])
            .status()
            .context("failed to send macOS notification")?;

        if status.success() {
            Ok(())
        } else {
            bail!("osascript exited with status {status}")
        }
    }
}
