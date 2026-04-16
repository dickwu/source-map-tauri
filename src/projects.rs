use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectRecord {
    pub name: String,
    pub repo_path: String,
    pub index_uid: String,
    pub meili_host: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectRegistry {
    #[serde(default)]
    pub projects: Vec<ProjectRecord>,
}

impl ProjectRegistry {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create parent directory for {}", path.display()))?;
        }
        fs::write(path, serde_json::to_vec_pretty(self)?)
            .with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn upsert(&mut self, record: ProjectRecord) {
        if let Some(existing) = self
            .projects
            .iter_mut()
            .find(|item| item.repo_path == record.repo_path || item.name == record.name)
        {
            *existing = record;
        } else {
            self.projects.push(record);
        }
        self.projects.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.repo_path.cmp(&right.repo_path))
        });
    }
}

pub fn default_project_registry_path() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".config/meilisearch/project.json")
}

pub fn upsert_project_registry(record: ProjectRecord) -> Result<()> {
    let path = default_project_registry_path();
    let mut registry = ProjectRegistry::load(&path)?;
    registry.upsert(record);
    registry.save(&path)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{ProjectRecord, ProjectRegistry};
    use chrono::Utc;

    fn record(name: &str, repo_path: &str) -> ProjectRecord {
        ProjectRecord {
            name: name.to_owned(),
            repo_path: repo_path.to_owned(),
            index_uid: format!("{name}_index"),
            meili_host: "http://127.0.0.1:7700/".to_owned(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn upsert_replaces_existing_repo_entry() {
        let mut registry = ProjectRegistry::default();
        registry.upsert(record("tool", "/tmp/tool"));
        let mut updated = record("tool-desktop", "/tmp/tool");
        updated.index_uid = "tool_desktop_index".to_owned();
        registry.upsert(updated.clone());

        assert_eq!(registry.projects.len(), 1);
        assert_eq!(registry.projects[0].name, "tool-desktop");
        assert_eq!(registry.projects[0].index_uid, "tool_desktop_index");
    }

    #[test]
    fn load_and_save_round_trip() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("project.json");
        let mut registry = ProjectRegistry::default();
        registry.upsert(record("tool", "/tmp/tool"));
        registry.save(&path).unwrap();
        let loaded = ProjectRegistry::load(&path).unwrap();
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].name, "tool");
    }
}
