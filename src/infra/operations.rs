use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use anyhow::Context;

use crate::{app::ports::OperationLog, domain::model::OperationEvent};

#[derive(Debug, Clone)]
pub struct JsonlOperationLog {
    path: PathBuf,
}

impl JsonlOperationLog {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn ensure_parent_dir(&self) -> anyhow::Result<()> {
        let parent = self
            .path
            .parent()
            .context("operations log path was missing a parent directory")?;
        fs::create_dir_all(parent)?;
        Ok(())
    }
}

impl OperationLog for JsonlOperationLog {
    fn record(&self, event: &OperationEvent) -> anyhow::Result<()> {
        self.ensure_parent_dir()?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("failed to open {}", self.path.display()))?;
        writeln!(file, "{}", serde_json::to_string(event)?)?;
        Ok(())
    }

    fn recent(&self, limit: usize) -> anyhow::Result<Vec<OperationEvent>> {
        self.ensure_parent_dir()?;
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let raw = fs::read_to_string(&self.path)?;
        let mut events = raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str::<OperationEvent>(line).ok())
            .collect::<Vec<_>>();

        if events.len() > limit {
            events = events.split_off(events.len() - limit);
        }

        Ok(events)
    }
}
