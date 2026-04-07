use crate::domain::{
    error::MicoError,
    model::{AgentPreset, AppConfig, AppPaths, StoredState},
};

pub fn resolve_paths() -> anyhow::Result<AppPaths> {
    let home = dirs::home_dir().ok_or(MicoError::HomeDirectoryUnavailable)?;
    let root = home.join(".mico");

    Ok(AppPaths {
        config_path: root.join("config.json"),
        state_path: root.join("state.json"),
        operations_log_path: root.join("operations.jsonl"),
        task_results_path: root.join("task-results.json"),
        worktrees_root: root.join("worktrees"),
        root,
    })
}

pub fn default_agent_presets() -> Vec<AgentPreset> {
    vec![
        AgentPreset {
            name: "terminal".to_string(),
            command: String::new(),
            one_off_command: None,
        },
        AgentPreset {
            name: "claude".to_string(),
            command: "claude".to_string(),
            one_off_command: Some("claude -p {prompt}".to_string()),
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
    ]
}

pub fn default_config() -> AppConfig {
    AppConfig {
        github_repo: None,
        agent_presets: default_agent_presets(),
    }
}

pub fn default_state() -> StoredState {
    StoredState {
        version: 3,
        repos: Vec::new(),
        workstreams: Vec::new(),
    }
}
