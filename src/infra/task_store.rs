use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use uuid::Uuid;

use crate::{app::background::PersistedTaskCompletion, app::ports::TaskCompletionStore};

#[derive(Debug, Clone)]
pub struct JsonTaskCompletionStore {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl JsonTaskCompletionStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Arc::new(Mutex::new(())),
        }
    }

    fn ensure_parent_dir(&self) -> anyhow::Result<()> {
        let parent = self
            .path
            .parent()
            .context("task results path was missing a parent directory")?;
        fs::create_dir_all(parent)?;
        Ok(())
    }

    fn load_locked(&self) -> anyhow::Result<Vec<PersistedTaskCompletion>> {
        self.ensure_parent_dir()?;
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let raw = fs::read_to_string(&self.path)?;
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }

        serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse {}", self.path.display()))
    }

    fn save_locked(&self, completions: &[PersistedTaskCompletion]) -> anyhow::Result<()> {
        self.ensure_parent_dir()?;
        let payload = serde_json::to_string_pretty(completions)?;
        fs::write(&self.path, payload)?;
        Ok(())
    }
}

impl TaskCompletionStore for JsonTaskCompletionStore {
    fn load(&self) -> anyhow::Result<Vec<PersistedTaskCompletion>> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("task completion store lock poisoned"))?;
        self.load_locked()
    }

    fn append(&self, completion: &PersistedTaskCompletion) -> anyhow::Result<()> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("task completion store lock poisoned"))?;
        let mut completions = self.load_locked()?;
        completions.push(completion.clone());
        self.save_locked(&completions)
    }

    fn remove(&self, completion_id: Uuid) -> anyhow::Result<()> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("task completion store lock poisoned"))?;
        let mut completions = self.load_locked()?;
        completions.retain(|completion| completion.id != completion_id);
        self.save_locked(&completions)
    }
}
