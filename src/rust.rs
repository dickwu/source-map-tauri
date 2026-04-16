use std::path::Path;

use anyhow::Result;
use lsp_types::SymbolKind;
use regex::Regex;
use serde_json::{Map, Value};

use crate::{
    config::{normalize_path, ResolvedConfig},
    discovery::RepoDiscovery,
    ids::document_id,
    lsp::{line_contains, range_end_line, range_start_line, LspClient, SymbolLocation},
    model::ArtifactDoc,
    security::apply_artifact_security,
};

fn has_segment(path: &str, segment: &str) -> bool {
    path.starts_with(&format!("{segment}/")) || path.contains(&format!("/{segment}/"))
}

fn line_number(text: &str, offset: usize) -> u32 {
    text[..offset].bytes().filter(|byte| *byte == b'\n').count() as u32 + 1
}

fn new_doc(
    config: &ResolvedConfig,
    path: &Path,
    kind: &str,
    name: &str,
    line: u32,
    side: &str,
) -> ArtifactDoc {
    let source_path = normalize_path(&config.root, path);
    ArtifactDoc {
        id: document_id(
            &config.repo,
            kind,
            Some(&source_path),
            Some(line),
            Some(name),
        ),
        repo: config.repo.clone(),
        kind: kind.to_owned(),
        side: Some(side.to_owned()),
        language: Some("rust".to_owned()),
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
    }
}

pub fn extract(config: &ResolvedConfig, discovery: &RepoDiscovery) -> Result<Vec<ArtifactDoc>> {
    let mut artifacts = Vec::new();
    let command_re = Regex::new(r#"(?s)#\[(?:tauri::)?command\]\s*(?:pub\s+)?(async\s+)?fn\s+([A-Za-z0-9_]+)(?:<[^>]+>)?\s*(\([^)]*\))"#)
        .expect("valid regex");
    let command_attr_re = Regex::new(r#"(?m)^\s*#\[(?:tauri::)?command\]"#).expect("valid regex");
    let builder_re = Regex::new(r#"Builder::new\("([^"]+)"\)"#).expect("valid regex");
    let hook_re = Regex::new(r#"\.(setup|on_navigation|on_webview_ready|on_event|on_drop)\("#)
        .expect("valid regex");
    let permission_re = Regex::new(r#"identifier\s*=\s*"([^"]+)""#).expect("valid regex");
    let commands_allow_re =
        Regex::new(r#"commands\.allow\s*=\s*\[([^\]]+)\]"#).expect("valid regex");
    let mut rust_lsp = LspClient::new("rust-analyzer", &config.root).ok();

    for path in discovery
        .rust_files
        .iter()
        .chain(discovery.plugin_rust_files.iter())
    {
        let text = std::fs::read_to_string(path)?;
        let normalized = normalize_path(&config.root, path);
        let symbol_locations = rust_lsp
            .as_mut()
            .and_then(|client| client.document_symbols(path, &text, "rust").ok())
            .unwrap_or_default();
        let plugin_name = if has_segment(&normalized, "plugins") {
            builder_re
                .captures(&text)
                .and_then(|capture| capture.get(1))
                .map(|item| item.as_str().to_owned())
                .or_else(|| {
                    normalized
                        .strip_prefix("plugins/")
                        .or_else(|| normalized.split("/plugins/").nth(1))
                        .and_then(|tail| tail.split('/').next())
                        .map(|item| item.trim_start_matches("tauri-plugin-").to_owned())
                })
        } else {
            None
        };

        if let Some(plugin_name) = &plugin_name {
            let line = builder_re
                .captures(&text)
                .and_then(|capture| capture.get(0))
                .map(|item| line_number(&text, item.start()))
                .unwrap_or(1);
            let mut plugin_doc = new_doc(config, path, "tauri_plugin", plugin_name, line, "rust");
            plugin_doc
                .data
                .insert("plugin_name".to_owned(), Value::String(plugin_name.clone()));
            apply_artifact_security(&mut plugin_doc);
            artifacts.push(plugin_doc);

            for capture in hook_re.captures_iter(&text) {
                let hook_name = capture.get(1).expect("hook").as_str();
                let line = line_number(&text, capture.get(0).expect("match").start());
                let mut hook_doc = new_doc(
                    config,
                    path,
                    "tauri_plugin_lifecycle_hook",
                    hook_name,
                    line,
                    "rust",
                );
                hook_doc
                    .data
                    .insert("plugin_name".to_owned(), Value::String(plugin_name.clone()));
                hook_doc
                    .data
                    .insert("hook_name".to_owned(), Value::String(hook_name.to_owned()));
                apply_artifact_security(&mut hook_doc);
                artifacts.push(hook_doc);
            }
        }

        let mut lsp_command_docs = build_lsp_command_docs(
            config,
            path,
            &normalized,
            &text,
            &symbol_locations,
            &command_attr_re,
            plugin_name.as_deref(),
        );

        if lsp_command_docs.is_empty() {
            lsp_command_docs = command_re
                .captures_iter(&text)
                .map(|capture| {
                    let full = capture.get(0).expect("match");
                    let name = capture.get(2).expect("name").as_str();
                    let signature = capture
                        .get(3)
                        .map(|item| item.as_str().to_owned())
                        .unwrap_or_default();
                    let line = line_number(&text, full.start());
                    let kind = if plugin_name.is_some() {
                        "tauri_plugin_command"
                    } else {
                        "tauri_command"
                    };
                    let mut doc = new_doc(config, path, kind, name, line, "rust");
                    doc.display_name = Some(name.to_owned());
                    doc.tags = vec!["rust command".to_owned()];
                    doc.data
                        .insert("signature".to_owned(), Value::String(signature.clone()));
                    doc.data.insert(
                        "rust_fqn".to_owned(),
                        Value::String(format!(
                            "{}::{name}",
                            normalized.replace('/', "::").trim_end_matches(".rs")
                        )),
                    );
                    if let Some(plugin_name) = &plugin_name {
                        doc.data
                            .insert("plugin_name".to_owned(), Value::String(plugin_name.clone()));
                        doc.data.insert(
                            "invoke_key".to_owned(),
                            Value::String(format!("plugin:{plugin_name}|{name}")),
                        );
                    } else {
                        doc.data
                            .insert("invoke_key".to_owned(), Value::String(name.to_owned()));
                    }
                    let registered = text.contains("generate_handler!") && text.contains(name);
                    doc.data
                        .insert("registered".to_owned(), Value::Bool(registered));
                    apply_artifact_security(&mut doc);
                    doc
                })
                .collect();
        }

        artifacts.extend(lsp_command_docs);
    }

    for path in &discovery.permission_files {
        let text = std::fs::read_to_string(path)?;
        let normalized = normalize_path(&config.root, path);
        if let Some(capture) = permission_re.captures(&text) {
            let name = capture.get(1).expect("identifier").as_str();
            let line = line_number(&text, capture.get(0).expect("match").start());
            let mut doc = new_doc(config, path, "tauri_permission", name, line, "config");
            let plugin_name = normalized
                .strip_prefix("plugins/")
                .or_else(|| normalized.split("/plugins/").nth(1))
                .and_then(|tail| tail.split('/').next())
                .map(|item| item.trim_start_matches("tauri-plugin-").to_owned());
            if let Some(plugin_name) = plugin_name {
                doc.data
                    .insert("plugin_name".to_owned(), Value::String(plugin_name.clone()));
                doc.name = Some(format!("{plugin_name}:{name}"));
                doc.display_name = doc.name.clone();
            }
            if let Some(allow_capture) = commands_allow_re.captures(&text) {
                let commands = allow_capture[1]
                    .split(',')
                    .map(|item| item.trim().trim_matches('"').to_owned())
                    .filter(|item| !item.is_empty())
                    .collect::<Vec<_>>();
                doc.data.insert(
                    "commands_allow".to_owned(),
                    Value::Array(commands.into_iter().map(Value::String).collect()),
                );
            }
            apply_artifact_security(&mut doc);
            let permission_name = doc.name.clone().unwrap_or_else(|| name.to_owned());
            artifacts.push(doc);

            let mut scope_doc =
                new_doc(config, path, "tauri_permission_scope", name, line, "config");
            scope_doc
                .data
                .insert("permission_id".to_owned(), Value::String(permission_name));
            apply_artifact_security(&mut scope_doc);
            artifacts.push(scope_doc);
        }
    }

    let rust_test_targets_re =
        Regex::new(r#"async\s+fn\s+([A-Za-z0-9_]+)|fn\s+([A-Za-z0-9_]+)"#).expect("valid regex");

    for path in &discovery.rust_test_files {
        let text = std::fs::read_to_string(path)?;
        let normalized = normalize_path(&config.root, path);
        let name = Path::new(&normalized)
            .file_name()
            .and_then(|item| item.to_str())
            .unwrap_or("rust_test");
        let mut doc = new_doc(config, path, "rust_test", name, 1, "test");
        let targets = rust_test_targets_re
            .captures_iter(&text)
            .filter_map(|capture| capture.get(1).or_else(|| capture.get(2)))
            .map(|item| item.as_str().to_owned())
            .collect::<Vec<_>>();
        doc.data.insert(
            "targets".to_owned(),
            Value::Array(targets.into_iter().map(Value::String).collect()),
        );
        doc.data.insert(
            "command".to_owned(),
            Value::String(format!("cargo test {}", normalize_path(&config.root, path))),
        );
        apply_artifact_security(&mut doc);
        artifacts.push(doc);
    }

    Ok(artifacts)
}

fn build_lsp_command_docs(
    config: &ResolvedConfig,
    path: &Path,
    normalized: &str,
    text: &str,
    symbols: &[SymbolLocation],
    command_attr_re: &Regex,
    plugin_name: Option<&str>,
) -> Vec<ArtifactDoc> {
    let function_symbols = symbols
        .iter()
        .filter(|symbol| matches!(symbol.kind, SymbolKind::FUNCTION | SymbolKind::METHOD))
        .collect::<Vec<_>>();

    let mut docs = Vec::new();
    for capture in command_attr_re.find_iter(text) {
        let attr_line = line_number(text, capture.start());
        let Some(symbol) = match_command_symbol(&function_symbols, attr_line) else {
            continue;
        };

        let kind = if plugin_name.is_some() {
            "tauri_plugin_command"
        } else {
            "tauri_command"
        };
        let mut doc = new_doc(
            config,
            path,
            kind,
            &symbol.name,
            range_start_line(&symbol.range),
            "rust",
        );
        doc.display_name = Some(symbol.name.clone());
        doc.tags = vec!["rust command".to_owned()];
        doc.line_end = Some(range_end_line(&symbol.range));
        doc.data.insert(
            "signature".to_owned(),
            Value::String(extract_signature(text, &symbol.name)),
        );
        doc.data.insert(
            "rust_fqn".to_owned(),
            Value::String(format!(
                "{}::{}",
                normalized.replace('/', "::").trim_end_matches(".rs"),
                symbol.name
            )),
        );
        if let Some(plugin_name) = plugin_name {
            doc.data.insert(
                "plugin_name".to_owned(),
                Value::String(plugin_name.to_owned()),
            );
            doc.data.insert(
                "invoke_key".to_owned(),
                Value::String(format!("plugin:{plugin_name}|{}", symbol.name)),
            );
        } else {
            doc.data
                .insert("invoke_key".to_owned(), Value::String(symbol.name.clone()));
        }
        let registered = text.contains("generate_handler!") && text.contains(&symbol.name);
        doc.data
            .insert("registered".to_owned(), Value::Bool(registered));
        doc.data.insert(
            "source_map_backend".to_owned(),
            Value::String("rust-analyzer-lsp".to_owned()),
        );
        apply_artifact_security(&mut doc);
        docs.push(doc);
    }
    docs
}

fn match_command_symbol<'a>(
    symbols: &'a [&'a SymbolLocation],
    attr_line: u32,
) -> Option<&'a SymbolLocation> {
    symbols
        .iter()
        .copied()
        .find(|symbol| line_contains(&symbol.range, attr_line))
        .or_else(|| {
            symbols
                .iter()
                .copied()
                .filter(|symbol| range_start_line(&symbol.range) >= attr_line)
                .min_by_key(|symbol| range_start_line(&symbol.range) - attr_line)
        })
}

fn extract_signature(text: &str, function_name: &str) -> String {
    let pattern = format!(
        r#"(?m)(?:pub\s+)?(?:async\s+)?fn\s+{}\b(?:<[^>]+>)?\s*(\([^)]*\))"#,
        regex::escape(function_name)
    );
    Regex::new(&pattern)
        .ok()
        .and_then(|regex| regex.captures(text))
        .and_then(|capture| capture.get(1))
        .map(|capture| capture.as_str().to_owned())
        .unwrap_or_default()
}
