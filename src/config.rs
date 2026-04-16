use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::cli::Cli;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub repo: String,
    pub root: String,
    pub frontend_roots: Vec<String>,
    pub tauri_root: String,
    pub plugin_roots: Vec<String>,
    pub output_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    pub include_node_modules: bool,
    pub include_target: bool,
    pub include_dist: bool,
    pub include_vendor: bool,
    pub redact_secrets: bool,
    pub detect_phi: bool,
    pub fail_on_phi: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendConfig {
    pub frameworks: Vec<String>,
    pub parser: String,
    pub recognize_hooks: bool,
    pub recognize_tests: bool,
    pub recognize_mock_ipc: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauriConfig {
    pub parse_commands: bool,
    pub parse_plugins: bool,
    pub parse_plugin_permissions: bool,
    pub parse_capabilities: bool,
    pub parse_events: bool,
    pub parse_channels: bool,
    pub parse_lifecycle_hooks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcemapsConfig {
    pub enabled: bool,
    pub paths: Vec<String>,
    pub collapse_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeiliConfig {
    pub url: String,
    pub index: String,
    pub batch_size: usize,
    pub wait_for_tasks: bool,
    pub master_key_env: String,
    pub search_key_env: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpBridgeConfig {
    pub enabled: bool,
    pub php_index_export: String,
    pub join_http_routes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub high_keywords: Vec<String>,
    pub critical_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConfig {
    pub project: ProjectConfig,
    pub scan: ScanConfig,
    pub frontend: FrontendConfig,
    pub tauri: TauriConfig,
    pub sourcemaps: SourcemapsConfig,
    pub meilisearch: MeiliConfig,
    pub php_bridge: PhpBridgeConfig,
    pub risk: RiskConfig,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            project: ProjectConfig {
                repo: "source-map-tauri".to_owned(),
                root: ".".to_owned(),
                frontend_roots: vec![
                    "src".to_owned(),
                    "app".to_owned(),
                    "frontend/src".to_owned(),
                ],
                tauri_root: "src-tauri".to_owned(),
                plugin_roots: vec![
                    "plugins".to_owned(),
                    "crates".to_owned(),
                    "src-tauri/plugins".to_owned(),
                ],
                output_dir: ".repo-search/tauri".to_owned(),
            },
            scan: ScanConfig {
                include_node_modules: false,
                include_target: false,
                include_dist: false,
                include_vendor: false,
                redact_secrets: true,
                detect_phi: true,
                fail_on_phi: false,
            },
            frontend: FrontendConfig {
                frameworks: vec!["react".to_owned(), "vue".to_owned(), "svelte".to_owned()],
                parser: "tree-sitter".to_owned(),
                recognize_hooks: true,
                recognize_tests: true,
                recognize_mock_ipc: true,
            },
            tauri: TauriConfig {
                parse_commands: true,
                parse_plugins: true,
                parse_plugin_permissions: true,
                parse_capabilities: true,
                parse_events: true,
                parse_channels: true,
                parse_lifecycle_hooks: true,
            },
            sourcemaps: SourcemapsConfig {
                enabled: true,
                paths: vec!["dist/**/*.map".to_owned(), "build/**/*.map".to_owned()],
                collapse_strategy: "nearest_symbol".to_owned(),
            },
            meilisearch: MeiliConfig {
                url: "http://127.0.0.1:7700".to_owned(),
                index: "tauri_source_map".to_owned(),
                batch_size: 5000,
                wait_for_tasks: true,
                master_key_env: "MEILI_MASTER_KEY".to_owned(),
                search_key_env: "MEILI_SEARCH_KEY".to_owned(),
            },
            php_bridge: PhpBridgeConfig {
                enabled: false,
                php_index_export: ".repo-search/php/symbols.ndjson".to_owned(),
                join_http_routes: true,
            },
            risk: RiskConfig {
                high_keywords: vec![
                    "patient".to_owned(),
                    "phi".to_owned(),
                    "mrn".to_owned(),
                    "consent".to_owned(),
                    "medication".to_owned(),
                    "lab".to_owned(),
                    "diagnosis".to_owned(),
                    "billing".to_owned(),
                    "insurance".to_owned(),
                    "discharge".to_owned(),
                    "audit".to_owned(),
                ],
                critical_kinds: vec![
                    "database_access".to_owned(),
                    "filesystem_export".to_owned(),
                    "external_integration".to_owned(),
                ],
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub root: PathBuf,
    pub repo: String,
    pub output_dir: PathBuf,
    pub file: FileConfig,
}

#[derive(Debug, Clone)]
pub struct MeiliConnection {
    pub host: Url,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ConnectFile {
    host: Option<String>,
    api_key: Option<String>,
    search_key: Option<String>,
}

impl ResolvedConfig {
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        let root = cli.root.canonicalize().unwrap_or_else(|_| cli.root.clone());
        let config_path = cli
            .config
            .clone()
            .unwrap_or_else(|| root.join(".repo-search/tauri/source-map-tauri.toml"));

        let mut file = if config_path.exists() {
            let text = fs::read_to_string(&config_path)
                .with_context(|| format!("failed to read {}", config_path.display()))?;
            toml::from_str::<FileConfig>(&text)
                .with_context(|| format!("failed to parse {}", config_path.display()))?
        } else {
            FileConfig::default()
        };

        file.project.root = root.to_string_lossy().to_string();
        if let Some(repo) = &cli.repo {
            file.project.repo = repo.clone();
        }
        file.scan.include_node_modules = cli.include_node_modules;
        file.scan.include_target = cli.include_target;
        file.scan.include_dist = cli.include_dist;
        file.scan.include_vendor = cli.include_vendor;
        file.scan.redact_secrets = cli.redact_secrets;
        file.scan.detect_phi = cli.detect_phi;
        file.scan.fail_on_phi = cli.fail_on_phi;

        let output_dir = root.join(&file.project.output_dir);
        Ok(Self {
            root,
            repo: file.project.repo.clone(),
            output_dir,
            file,
        })
    }

    pub fn with_output_override(&self, output: Option<PathBuf>) -> Self {
        let mut next = self.clone();
        if let Some(path) = output {
            next.output_dir = if path.is_absolute() {
                path
            } else {
                self.root.join(path)
            };
        }
        next
    }

    pub fn resolve_meili(
        &self,
        host_override: Option<&str>,
        key_override: Option<&str>,
        search_mode: bool,
    ) -> Result<MeiliConnection> {
        let env_host = env::var("MEILI_HOST").ok();
        let env_key = if search_mode {
            env::var(&self.file.meilisearch.search_key_env)
                .ok()
                .or_else(|| env::var(&self.file.meilisearch.master_key_env).ok())
        } else {
            env::var(&self.file.meilisearch.master_key_env).ok()
        };
        let connect_file = ConnectFile::load(&default_connect_file_path())?;

        let host_source = host_override
            .map(ToOwned::to_owned)
            .or(env_host)
            .or(connect_file.host)
            .unwrap_or_else(|| self.file.meilisearch.url.clone());
        let host = Url::parse(&host_source)
            .with_context(|| format!("invalid Meilisearch host {host_source}"))?;

        let key = key_override
            .map(ToOwned::to_owned)
            .or(env_key)
            .or_else(|| {
                if search_mode {
                    connect_file.search_key.or(connect_file.api_key)
                } else {
                    connect_file.api_key
                }
            })
            .ok_or_else(|| {
                if search_mode {
                    anyhow!(
                        "missing meilisearch api key in env {} / {} or {}",
                        self.file.meilisearch.search_key_env,
                        self.file.meilisearch.master_key_env,
                        default_connect_file_path().display()
                    )
                } else {
                    anyhow!(
                        "missing meilisearch api key in env {} or {}",
                        self.file.meilisearch.master_key_env,
                        default_connect_file_path().display()
                    )
                }
            })?;

        Ok(MeiliConnection { host, api_key: key })
    }
}

pub fn init_project(config: &ResolvedConfig) -> Result<()> {
    let output_dir = &config.output_dir;
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    fs::write(
        output_dir.join("source-map-tauri.toml"),
        toml::to_string_pretty(&config.file)?,
    )
    .with_context(|| format!("failed to write {}", output_dir.display()))?;
    fs::write(output_dir.join(".gitignore"), "*\n!.gitignore\n")
        .with_context(|| format!("failed to write {}", output_dir.display()))?;
    write_connect_file_if_missing()?;
    Ok(())
}

pub fn normalize_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn default_connect_file_path() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".config/meilisearch/connect.json")
}

fn write_connect_file_if_missing() -> Result<()> {
    let path = default_connect_file_path();
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let placeholder = serde_json::json!({
        "host": "http://127.0.0.1:7700",
        "api_key": "change-me",
        "search_key": "change-me"
    });
    fs::write(&path, serde_json::to_vec_pretty(&placeholder)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

impl ConnectFile {
    fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        Self::from_json(&raw).with_context(|| format!("parse {}", path.display()))
    }

    fn from_json(raw: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(raw)?;
        Ok(Self {
            host: value_lookup(&value, &["host", "url", "endpoint"]).or_else(|| {
                nested_lookup(
                    &value,
                    &["connection", "default", "meilisearch"],
                    &["host", "url", "endpoint"],
                )
            }),
            api_key: value_lookup(
                &value,
                &["api_key", "apiKey", "master_key", "masterKey", "key"],
            )
            .or_else(|| {
                nested_lookup(
                    &value,
                    &["connection", "default", "meilisearch"],
                    &["api_key", "apiKey", "master_key", "masterKey", "key"],
                )
            }),
            search_key: value_lookup(&value, &["search_key", "searchKey"]).or_else(|| {
                nested_lookup(
                    &value,
                    &["connection", "default", "meilisearch"],
                    &["search_key", "searchKey"],
                )
            }),
        })
    }
}

fn value_lookup(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(|item| item.to_string())
    })
}

fn nested_lookup(value: &Value, containers: &[&str], keys: &[&str]) -> Option<String> {
    containers.iter().find_map(|container| {
        value
            .get(container)
            .and_then(|nested| value_lookup(nested, keys))
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{default_connect_file_path, ConnectFile, FileConfig, ResolvedConfig};

    #[test]
    fn connect_file_reads_host_and_keys() {
        let parsed = ConnectFile::from_json(
            r#"{"host":"http://meili.example:7700","api_key":"master","search_key":"search"}"#,
        )
        .unwrap();
        assert_eq!(parsed.host.as_deref(), Some("http://meili.example:7700"));
        assert_eq!(parsed.api_key.as_deref(), Some("master"));
        assert_eq!(parsed.search_key.as_deref(), Some("search"));
    }

    #[test]
    fn resolve_meili_uses_connect_file() {
        let temp = tempdir().unwrap();
        std::env::set_var("HOME", temp.path());
        let path = default_connect_file_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"host":"http://127.0.0.1:7700","api_key":"master","search_key":"search"}"#,
        )
        .unwrap();

        let config = ResolvedConfig {
            root: temp.path().to_path_buf(),
            repo: "fixture".to_owned(),
            output_dir: temp.path().join("out"),
            file: FileConfig::default(),
        };

        let admin = config.resolve_meili(None, None, false).unwrap();
        let search = config.resolve_meili(None, None, true).unwrap();
        assert_eq!(admin.api_key, "master");
        assert_eq!(search.api_key, "search");
    }
}
