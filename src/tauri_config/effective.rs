use anyhow::Result;
use serde_json::{json, Value};

use crate::{
    config::{normalize_path, ResolvedConfig},
    discovery::RepoDiscovery,
    ids::document_id,
    model::ArtifactDoc,
    security::apply_artifact_security,
};

pub fn extract_effective_capabilities(
    config: &ResolvedConfig,
    discovery: &RepoDiscovery,
) -> Result<Vec<ArtifactDoc>> {
    let mut windows = Vec::new();
    for path in &discovery.tauri_configs {
        let text = std::fs::read_to_string(path)?;
        let value: serde_json::Value = serde_json::from_str(&text)?;
        if let Some(items) = value
            .get("app")
            .and_then(|item| item.get("windows"))
            .and_then(serde_json::Value::as_array)
        {
            for item in items {
                if let Some(label) = item.get("label").and_then(serde_json::Value::as_str) {
                    windows.push(label.to_owned());
                } else {
                    windows.push("main".to_owned());
                }
            }
        }
    }

    let mut docs = Vec::new();
    for window in windows {
        let mut permissions = Vec::new();
        let mut capability_ids = Vec::new();
        for path in &discovery.capability_files {
            let text = std::fs::read_to_string(path)?;
            let value: serde_json::Value =
                serde_json::from_str(&text).unwrap_or_else(|_| json!({}));
            let matches_window = value
                .get("windows")
                .and_then(serde_json::Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .any(|item| item.as_str() == Some(window.as_str()))
                })
                .unwrap_or(false);
            if matches_window {
                if let Some(identifier) =
                    value.get("identifier").and_then(serde_json::Value::as_str)
                {
                    capability_ids.push(identifier.to_owned());
                }
                if let Some(items) = value
                    .get("permissions")
                    .and_then(serde_json::Value::as_array)
                {
                    permissions.extend(
                        items
                            .iter()
                            .filter_map(|item| item.as_str())
                            .map(str::to_owned),
                    );
                }
            }
        }
        let source_path = discovery
            .capability_files
            .first()
            .map(|path| normalize_path(&config.root, path))
            .unwrap_or_else(|| "src-tauri/capabilities".to_owned());
        let mut doc = ArtifactDoc {
            id: document_id(
                &config.repo,
                "tauri_capability_effective",
                Some(&source_path),
                Some(1),
                Some(&window),
            ),
            repo: config.repo.clone(),
            kind: "tauri_capability_effective".to_owned(),
            side: Some("config".to_owned()),
            language: Some("json".to_owned()),
            name: Some(window.clone()),
            display_name: Some(window.clone()),
            source_path: Some(source_path),
            line_start: Some(1),
            line_end: Some(1),
            column_start: None,
            column_end: None,
            package_name: None,
            comments: Vec::new(),
            tags: vec!["capability".to_owned()],
            related_symbols: Vec::new(),
            related_tests: Vec::new(),
            risk_level: "medium".to_owned(),
            risk_reasons: vec!["multiple capabilities merge for window".to_owned()],
            contains_phi: false,
            has_related_tests: false,
            updated_at: chrono::Utc::now().to_rfc3339(),
            data: Default::default(),
        };
        doc.data
            .insert("window_label".to_owned(), Value::String(window));
        doc.data.insert(
            "capability_ids".to_owned(),
            Value::Array(capability_ids.into_iter().map(Value::String).collect()),
        );
        doc.data.insert(
            "permissions".to_owned(),
            Value::Array(permissions.iter().cloned().map(Value::String).collect()),
        );
        doc.data.insert(
            "plugin_permissions".to_owned(),
            Value::Array(
                permissions
                    .iter()
                    .filter(|item| item.contains(':'))
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
        doc.data
            .insert("merged_capabilities".to_owned(), Value::Bool(true));
        doc.data
            .insert("remote_capability".to_owned(), Value::Bool(false));
        apply_artifact_security(&mut doc);
        docs.push(doc);
    }
    Ok(docs)
}
