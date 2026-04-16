use anyhow::Result;
use serde_json::Value;

use crate::{
    config::{normalize_path, ResolvedConfig},
    discovery::RepoDiscovery,
    ids::document_id,
    model::ArtifactDoc,
    security::apply_artifact_security,
};

pub fn extract_capabilities(
    config: &ResolvedConfig,
    discovery: &RepoDiscovery,
) -> Result<Vec<ArtifactDoc>> {
    let mut docs = Vec::new();
    for path in &discovery.capability_files {
        let text = std::fs::read_to_string(path)?;
        let value = if path.extension().and_then(|item| item.to_str()) == Some("json") {
            serde_json::from_str::<Value>(&text)?
        } else {
            serde_json::to_value(toml::from_str::<toml::Value>(&text)?)?
        };
        let identifier = value
            .get("identifier")
            .and_then(Value::as_str)
            .unwrap_or("capability");
        let source_path = normalize_path(&config.root, path);
        let mut doc = ArtifactDoc {
            id: document_id(
                &config.repo,
                "tauri_capability",
                Some(&source_path),
                Some(1),
                Some(identifier),
            ),
            repo: config.repo.clone(),
            kind: "tauri_capability".to_owned(),
            side: Some("config".to_owned()),
            language: path
                .extension()
                .and_then(|item| item.to_str())
                .map(str::to_owned),
            name: Some(identifier.to_owned()),
            display_name: Some(identifier.to_owned()),
            source_path: Some(source_path),
            line_start: Some(1),
            line_end: Some(text.lines().count() as u32),
            column_start: None,
            column_end: None,
            package_name: None,
            comments: Vec::new(),
            tags: vec!["capability".to_owned()],
            related_symbols: Vec::new(),
            related_tests: Vec::new(),
            risk_level: "medium".to_owned(),
            risk_reasons: Vec::new(),
            contains_phi: false,
            has_related_tests: false,
            updated_at: chrono::Utc::now().to_rfc3339(),
            data: Default::default(),
        };
        for key in ["windows", "permissions"] {
            if let Some(value) = value.get(key) {
                doc.data.insert(key.to_owned(), value.clone());
            }
        }
        apply_artifact_security(&mut doc);
        docs.push(doc);
    }
    Ok(docs)
}
