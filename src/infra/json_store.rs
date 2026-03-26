use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    app::ports::{ConfigStore, StateStore},
    domain::model::{AppConfig, AppPaths, StoredState},
};

#[derive(Debug, Clone)]
pub struct JsonFileStore {
    paths: AppPaths,
}

impl JsonFileStore {
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }

    fn ensure_parent_dir(path: &Path) -> anyhow::Result<()> {
        let parent = path
            .parent()
            .context("path did not have a parent directory")?;
        fs::create_dir_all(parent)?;
        Ok(())
    }

    fn load_or_create<T>(&self, path: PathBuf, default: T) -> anyhow::Result<T>
    where
        T: Clone + DeserializeOwned + Serialize,
    {
        Self::ensure_parent_dir(&path)?;

        if !path.exists() {
            let payload = serde_json::to_string_pretty(&default)?;
            fs::write(&path, payload)?;
            return Ok(default);
        }

        let raw = fs::read_to_string(&path)?;
        let value = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(value)
    }

    fn save<T>(&self, path: &Path, value: &T) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        Self::ensure_parent_dir(path)?;
        fs::write(path, serde_json::to_string_pretty(value)?)?;
        Ok(())
    }
}

impl ConfigStore for JsonFileStore {
    fn load_or_create_config(&self, default: AppConfig) -> anyhow::Result<AppConfig> {
        self.load_or_create(self.paths.config_path.clone(), default)
    }

    fn save_config(&self, config: &AppConfig) -> anyhow::Result<()> {
        self.save(&self.paths.config_path, config)
    }
}

impl StateStore for JsonFileStore {
    fn load_or_create_state(&self, default: StoredState) -> anyhow::Result<StoredState> {
        self.load_or_create(self.paths.state_path.clone(), default)
    }

    fn save_state(&self, state: &StoredState) -> anyhow::Result<()> {
        self.save(&self.paths.state_path, state)
    }
}
