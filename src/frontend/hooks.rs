use std::{collections::BTreeSet, path::Path};

use anyhow::Result;
use regex::Regex;
use serde_json::{Map, Value};

use crate::{
    config::{normalize_path, ResolvedConfig},
    discovery::RepoDiscovery,
    ids::document_id,
    model::ArtifactDoc,
    security::apply_artifact_security,
};

fn line_number(text: &str, offset: usize) -> u32 {
    text[..offset].bytes().filter(|byte| *byte == b'\n').count() as u32 + 1
}

fn base_artifact(
    config: &ResolvedConfig,
    path: &Path,
    kind: &str,
    name: &str,
    line: u32,
) -> ArtifactDoc {
    let source_path = normalize_path(&config.root, path);
    let mut doc = ArtifactDoc {
        id: document_id(
            &config.repo,
            kind,
            Some(&source_path),
            Some(line),
            Some(name),
        ),
        repo: config.repo.clone(),
        kind: kind.to_owned(),
        side: Some("frontend".to_owned()),
        language: crate::frontend::language_for_path(path),
        name: Some(name.to_owned()),
        display_name: Some(name.to_owned()),
        source_path: Some(source_path),
        line_start: Some(line),
        line_end: Some(line),
        column_start: None,
        column_end: None,
        package_name: None,
        comments: Vec::new(),
        tags: Vec::new(),
        related_symbols: Vec::new(),
        related_tests: Vec::new(),
        risk_level: "low".to_owned(),
        risk_reasons: Vec::new(),
        contains_phi: false,
        has_related_tests: false,
        updated_at: chrono::Utc::now().to_rfc3339(),
        data: Map::new(),
    };
    apply_artifact_security(&mut doc);
    doc
}

pub fn extract_components_and_hooks(
    config: &ResolvedConfig,
    path: &Path,
    text: &str,
    known_hooks: &BTreeSet<String>,
) -> Vec<ArtifactDoc> {
    let export_fn =
        Regex::new(r"(?m)^\s*export\s+function\s+([A-Za-z0-9_]+)").expect("valid regex");
    let hook_call = Regex::new(r"\b(use[A-Z][A-Za-z0-9_]*)\(").expect("valid regex");

    let mut docs = Vec::new();
    let mut component_names = Vec::new();

    for capture in export_fn.captures_iter(text) {
        let whole = capture.get(0).expect("match");
        let name = capture.get(1).expect("name").as_str();
        let line = line_number(text, whole.start());
        if name.starts_with("use") {
            let mut doc = base_artifact(config, path, "frontend_hook_def", name, line);
            let hook_kind = if text.contains("new Channel") || text.contains("Channel<") {
                "channel_stream"
            } else if text.contains("listen(") || text.contains("once(") {
                "event_subscription"
            } else if text.contains("invoke(") {
                "invoke_once"
            } else {
                "unknown"
            };
            doc.display_name = Some(format!("{name} hook"));
            doc.tags = vec!["custom hook".to_owned()];
            doc.data
                .insert("hook_kind".to_owned(), Value::String(hook_kind.to_owned()));
            doc.data.insert(
                "requires_cleanup".to_owned(),
                Value::Bool(text.contains("listen(") || text.contains("once(")),
            );
            doc.data.insert(
                "cleanup_present".to_owned(),
                Value::Bool(text.contains("return () =>") || text.contains("unlisten")),
            );
            apply_artifact_security(&mut doc);
            docs.push(doc);
        } else if name
            .chars()
            .next()
            .map(|item| item.is_uppercase())
            .unwrap_or(false)
        {
            let mut doc = base_artifact(config, path, "frontend_component", name, line);
            doc.display_name = Some(format!("{name} component"));
            doc.tags = vec!["component".to_owned()];
            doc.data
                .insert("component".to_owned(), Value::String(name.to_owned()));
            apply_artifact_security(&mut doc);
            component_names.push(name.to_owned());
            docs.push(doc);
        }
    }

    for capture in hook_call.captures_iter(text) {
        let whole = capture.get(0).expect("match");
        let name = capture.get(1).expect("name").as_str();
        if !known_hooks.contains(name) {
            continue;
        }
        if text[whole.start()..whole.end()].starts_with("function ") {
            continue;
        }
        let line = line_number(text, whole.start());
        let mut doc = base_artifact(config, path, "frontend_hook_use", name, line);
        if let Some(component) = component_names.first() {
            doc.data
                .insert("component".to_owned(), Value::String(component.clone()));
            doc.display_name = Some(format!("{component} uses {name}"));
        }
        doc.data
            .insert("hook_kind".to_owned(), Value::String("unknown".to_owned()));
        doc.data
            .insert("hook_def_name".to_owned(), Value::String(name.to_owned()));
        apply_artifact_security(&mut doc);
        docs.push(doc);
    }

    docs
}

pub fn discover_hook_names(discovery: &RepoDiscovery) -> Result<BTreeSet<String>> {
    let export_fn =
        Regex::new(r"(?m)^\s*export\s+function\s+(use[A-Z][A-Za-z0-9_]*)").expect("valid regex");
    let mut names = BTreeSet::new();

    for path in &discovery.frontend_files {
        let text = std::fs::read_to_string(path)?;
        for capture in export_fn.captures_iter(&text) {
            if let Some(name) = capture.get(1) {
                names.insert(name.as_str().to_owned());
            }
        }
    }

    Ok(names)
}
