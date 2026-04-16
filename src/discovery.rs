use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use walkdir::{DirEntry, WalkDir};

use crate::config::{normalize_path, ResolvedConfig};

#[derive(Debug, Clone, Default)]
pub struct RepoDiscovery {
    pub frontend_files: Vec<PathBuf>,
    pub frontend_test_files: Vec<PathBuf>,
    pub rust_files: Vec<PathBuf>,
    pub rust_test_files: Vec<PathBuf>,
    pub guest_js_files: Vec<PathBuf>,
    pub plugin_rust_files: Vec<PathBuf>,
    pub tauri_configs: Vec<PathBuf>,
    pub capability_files: Vec<PathBuf>,
    pub permission_files: Vec<PathBuf>,
    pub package_json: Option<PathBuf>,
    pub tsconfig: Option<PathBuf>,
    pub vite_configs: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub root: String,
    pub repo: String,
    pub src_tauri_exists: bool,
    pub cargo_toml_exists: bool,
    pub tauri_config_exists: bool,
    pub package_json_exists: bool,
    pub frontend_files_found: usize,
    pub capability_files_found: usize,
    pub permission_files_found: usize,
    pub plugin_roots_found: usize,
    pub sourcemap_support_hint: String,
}

fn has_segment(path: &str, segment: &str) -> bool {
    path.starts_with(&format!("{segment}/")) || path.contains(&format!("/{segment}/"))
}

fn is_ignored_dir(entry: &DirEntry, config: &ResolvedConfig) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    let name = entry.file_name().to_string_lossy();
    match name.as_ref() {
        ".git" => true,
        "node_modules" => !config.file.scan.include_node_modules,
        "target" => !config.file.scan.include_target,
        "dist" | "build" => !config.file.scan.include_dist,
        "vendor" => !config.file.scan.include_vendor,
        "coverage" | "logs" | "tmp" | "storage" => true,
        _ => false,
    }
}

pub fn discover(config: &ResolvedConfig) -> Result<RepoDiscovery> {
    let mut discovery = RepoDiscovery::default();
    let walker = WalkDir::new(&config.root)
        .into_iter()
        .filter_entry(|entry| !is_ignored_dir(entry, config));

    for item in walker.flatten().filter(|entry| entry.file_type().is_file()) {
        let path = item.path().to_path_buf();
        let normalized = normalize_path(&config.root, &path);
        let file_name = path
            .file_name()
            .and_then(|item| item.to_str())
            .unwrap_or_default();

        if normalized == "package.json" {
            discovery.package_json = Some(path.clone());
        }
        if normalized == "tsconfig.json" {
            discovery.tsconfig = Some(path.clone());
        }
        if file_name == "vite.config.ts" || file_name == "vite.config.js" {
            discovery.vite_configs.push(path.clone());
        }
        if normalized.ends_with("src-tauri/tauri.conf.json")
            || (normalized.contains("src-tauri/tauri.") && normalized.ends_with(".conf.json"))
        {
            discovery.tauri_configs.push(path.clone());
        }
        if has_segment(&normalized, "capabilities")
            && (normalized.ends_with(".json") || normalized.ends_with(".toml"))
        {
            discovery.capability_files.push(path.clone());
        }
        if has_segment(&normalized, "permissions")
            && (normalized.ends_with(".json") || normalized.ends_with(".toml"))
        {
            discovery.permission_files.push(path.clone());
        }

        let is_test = normalized.contains(".test.")
            || normalized.contains(".spec.")
            || normalized.contains("__tests__/");
        if normalized.ends_with(".ts")
            || normalized.ends_with(".tsx")
            || normalized.ends_with(".js")
            || normalized.ends_with(".jsx")
        {
            if has_segment(&normalized, "guest-js") || has_segment(&normalized, "dist-js") {
                discovery.guest_js_files.push(path.clone());
            } else if is_test {
                discovery.frontend_test_files.push(path.clone());
            } else if has_segment(&normalized, "src") {
                discovery.frontend_files.push(path.clone());
            }
        }

        if normalized.ends_with(".rs") {
            if has_segment(&normalized, "plugins") && has_segment(&normalized, "src") {
                discovery.plugin_rust_files.push(path.clone());
            } else if normalized.contains("/tests/") {
                discovery.rust_test_files.push(path.clone());
            } else {
                discovery.rust_files.push(path.clone());
            }
        }
    }

    Ok(discovery)
}

pub fn doctor(config: &ResolvedConfig) -> Result<DoctorReport> {
    let discovery = discover(config)?;
    let src_tauri = config.root.join("src-tauri");
    let plugin_root_count = config
        .file
        .project
        .plugin_roots
        .iter()
        .filter(|item| config.root.join(item).exists())
        .count();

    Ok(DoctorReport {
        root: normalize_path(Path::new("."), &config.root),
        repo: config.repo.clone(),
        src_tauri_exists: src_tauri.exists(),
        cargo_toml_exists: src_tauri.join("Cargo.toml").exists(),
        tauri_config_exists: !discovery.tauri_configs.is_empty(),
        package_json_exists: discovery.package_json.is_some(),
        frontend_files_found: discovery.frontend_files.len(),
        capability_files_found: discovery.capability_files.len(),
        permission_files_found: discovery.permission_files.len(),
        plugin_roots_found: plugin_root_count,
        sourcemap_support_hint: if discovery.vite_configs.is_empty() {
            "vite config not found".to_owned()
        } else {
            "vite config found; enable build.sourcemap for trace support".to_owned()
        },
    })
}
