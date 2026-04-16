use std::{fs, path::Path, thread, time::Duration};

use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};

use crate::{
    config::{MeiliConnection, ResolvedConfig},
    model::ProjectInfo,
    projects::{upsert_project_registry, ProjectRecord},
};

#[derive(Debug, Clone)]
pub struct MeiliClient {
    client: Client,
    connection: MeiliConnection,
}

pub struct UploadRequest<'a> {
    pub meili_url: Option<&'a str>,
    pub meili_key: Option<&'a str>,
    pub index: Option<&'a str>,
    pub input: &'a Path,
    pub edges: Option<&'a Path>,
    pub warnings: Option<&'a Path>,
    pub wait: bool,
    pub _batch_size: usize,
}

impl MeiliClient {
    pub fn new(connection: MeiliConnection) -> Result<Self> {
        Ok(Self {
            client: Client::builder().build()?,
            connection,
        })
    }

    pub fn health(&self) -> Result<Value> {
        self.get("health")
    }

    pub fn create_index(&self, name: &str) -> Result<()> {
        let response = self
            .client
            .post(self.url("indexes")?)
            .bearer_auth(&self.connection.api_key)
            .json(&json!({ "uid": name, "primaryKey": "id" }))
            .send()?;
        if response.status().is_success() || response.status().as_u16() == 409 {
            return Ok(());
        }
        Err(anyhow!(
            "failed to create index {name}: {}",
            response.text()?
        ))
    }

    pub fn apply_settings(&self, index: &str, settings: &Value, wait: bool) -> Result<()> {
        let task = self
            .client
            .patch(self.url(&format!("indexes/{index}/settings"))?)
            .bearer_auth(&self.connection.api_key)
            .json(settings)
            .send()?
            .json::<Value>()?;
        if wait {
            self.wait_for_task(task_uid(&task)?)?;
        }
        Ok(())
    }

    pub fn replace_documents_ndjson(&self, index: &str, body: Vec<u8>, wait: bool) -> Result<()> {
        let task = self
            .client
            .post(self.url(&format!("indexes/{index}/documents"))?)
            .bearer_auth(&self.connection.api_key)
            .header("Content-Type", "application/x-ndjson")
            .body(body)
            .send()?
            .json::<Value>()?;
        if wait {
            self.wait_for_task(task_uid(&task)?)?;
        }
        Ok(())
    }

    pub fn search(&self, index: &str, body: Value) -> Result<Value> {
        Ok(self
            .client
            .post(self.url(&format!("indexes/{index}/search"))?)
            .bearer_auth(&self.connection.api_key)
            .json(&body)
            .send()?
            .json()?)
    }

    pub fn wait_for_task(&self, uid: u64) -> Result<()> {
        for _ in 0..50 {
            let task = self.get(&format!("tasks/{uid}"))?;
            match task.get("status").and_then(Value::as_str) {
                Some("succeeded") => return Ok(()),
                Some("failed") => return Err(anyhow!("meilisearch task {uid} failed: {task}")),
                _ => thread::sleep(Duration::from_millis(100)),
            }
        }
        Err(anyhow!("timed out waiting for meilisearch task {uid}"))
    }

    fn get(&self, path: &str) -> Result<Value> {
        Ok(self
            .client
            .get(self.url(path)?)
            .bearer_auth(&self.connection.api_key)
            .send()?
            .json()?)
    }

    fn url(&self, path: &str) -> Result<reqwest::Url> {
        self.connection
            .host
            .join(path)
            .with_context(|| format!("join meilisearch path {path}"))
    }
}

pub fn upload(config: &ResolvedConfig, request: UploadRequest<'_>) -> Result<()> {
    let connection = config.resolve_meili(request.meili_url, request.meili_key, false)?;
    let client = MeiliClient::new(connection.clone())?;
    let index_name = request.index.unwrap_or(&config.file.meilisearch.index);

    client.create_index(index_name)?;

    let settings_path = request
        .input
        .parent()
        .map(|path| path.join("meili-settings.json"));
    if let Some(settings_path) = settings_path.filter(|path| path.exists()) {
        let payload: Value = serde_json::from_str(&fs::read_to_string(&settings_path)?)?;
        client.apply_settings(index_name, &payload, request.wait)?;
    }

    for path in [Some(request.input), request.edges, request.warnings]
        .into_iter()
        .flatten()
    {
        client.replace_documents_ndjson(index_name, fs::read(path)?, request.wait)?;
    }

    if let Some(mut project_info) = read_project_info(request.input.parent())? {
        project_info.index_uid = index_name.to_owned();
        write_project_info(request.input.parent(), &project_info)?;
        upsert_project_registry(ProjectRecord {
            name: project_info.repo,
            repo_path: project_info.repo_path,
            index_uid: index_name.to_owned(),
            meili_host: connection.host.to_string(),
            updated_at: chrono::Utc::now(),
        })?;
    }

    println!("upload complete index={index_name}");
    Ok(())
}

pub fn search(
    config: &ResolvedConfig,
    meili_url: Option<&str>,
    meili_key: Option<&str>,
    index: Option<&str>,
    query: &str,
    filter: Option<&str>,
    limit: usize,
) -> Result<()> {
    let connection = config.resolve_meili(meili_url, meili_key, true)?;
    let client = MeiliClient::new(connection)?;
    let index_name = index.unwrap_or(&config.file.meilisearch.index);
    let response = client.search(
        index_name,
        json!({
            "q": query,
            "filter": filter,
            "limit": limit,
            "showRankingScore": true
        }),
    )?;
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

pub fn doctor_health(config: &ResolvedConfig) -> Option<Value> {
    let connection = config.resolve_meili(None, None, false).ok()?;
    let client = MeiliClient::new(connection).ok()?;
    client.health().ok()
}

fn task_uid(value: &Value) -> Result<u64> {
    value
        .get("taskUid")
        .or_else(|| value.get("uid"))
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("meilisearch response missing task uid: {value}"))
}

fn read_project_info(parent: Option<&Path>) -> Result<Option<ProjectInfo>> {
    let Some(parent) = parent else {
        return Ok(None);
    };
    let path = parent.join("project-info.json");
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    Ok(Some(
        serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?,
    ))
}

fn write_project_info(parent: Option<&Path>, project_info: &ProjectInfo) -> Result<()> {
    let Some(parent) = parent else {
        return Ok(());
    };
    let path = parent.join("project-info.json");
    fs::write(&path, serde_json::to_vec_pretty(project_info)?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}
