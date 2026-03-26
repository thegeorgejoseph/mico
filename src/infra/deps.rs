use std::{path::PathBuf, process::Command};

use crate::{
    app::ports::DependencyInspector,
    domain::model::{AppPaths, DependencyStatus, DoctorReport},
};

#[derive(Debug, Clone)]
pub struct SystemDependencyInspector {
    paths: AppPaths,
}

impl SystemDependencyInspector {
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }

    fn probe_command(name: &str, version_args: &[&str]) -> DependencyStatus {
        match Command::new(name).args(version_args).output() {
            Ok(output) if output.status.success() => DependencyStatus {
                name: name.to_string(),
                found: true,
                detail: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            },
            Ok(output) => DependencyStatus {
                name: name.to_string(),
                found: false,
                detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            },
            Err(error) => DependencyStatus {
                name: name.to_string(),
                found: false,
                detail: error.to_string(),
            },
        }
    }

    fn iterm_status() -> DependencyStatus {
        let candidates = [
            PathBuf::from("/Applications/iTerm.app"),
            dirs::home_dir()
                .map(|home| home.join("Applications/iTerm.app"))
                .unwrap_or_else(|| PathBuf::from("~/Applications/iTerm.app")),
        ];

        let found_path = candidates.into_iter().find(|path| path.exists());

        match found_path {
            Some(path) => DependencyStatus {
                name: "iTerm".to_string(),
                found: true,
                detail: path.display().to_string(),
            },
            None => DependencyStatus {
                name: "iTerm".to_string(),
                found: false,
                detail: "not found in /Applications or ~/Applications".to_string(),
            },
        }
    }
}

impl DependencyInspector for SystemDependencyInspector {
    fn doctor(&self) -> anyhow::Result<DoctorReport> {
        Ok(DoctorReport {
            paths: self.paths.clone(),
            dependencies: vec![
                Self::probe_command("git", &["--version"]),
                Self::probe_command("tmux", &["-V"]),
                Self::probe_command("osascript", &["-e", "return \"ok\""]),
                Self::iterm_status(),
            ],
        })
    }
}
