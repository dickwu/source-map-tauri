use std::path::Path;

use regex::Regex;
use serde_json::{Map, Value};

use crate::{
    config::{normalize_path, ResolvedConfig},
    ids::document_id,
    model::{ArtifactDoc, WarningDoc},
    security::apply_artifact_security,
};

fn line_number(text: &str, offset: usize) -> u32 {
    text[..offset].bytes().filter(|byte| *byte == b'\n').count() as u32 + 1
}

fn new_artifact(
    config: &ResolvedConfig,
    path: &Path,
    kind: &str,
    name: &str,
    line: u32,
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
    }
}

fn warning(
    config: &ResolvedConfig,
    path: &Path,
    warning_type: &str,
    message: &str,
    line: u32,
) -> WarningDoc {
    let source_path = normalize_path(&config.root, path);
    WarningDoc {
        id: document_id(
            &config.repo,
            "warning",
            Some(&source_path),
            Some(line),
            Some(warning_type),
        ),
        repo: config.repo.clone(),
        kind: "warning".to_owned(),
        warning_type: warning_type.to_owned(),
        severity: "warning".to_owned(),
        message: message.to_owned(),
        source_path: Some(source_path),
        line_start: Some(line),
        related_id: None,
        risk_level: "medium".to_owned(),
        remediation: None,
        updated_at: chrono::Utc::now().to_rfc3339(),
    }
}

pub fn extract_calls(
    config: &ResolvedConfig,
    path: &Path,
    text: &str,
    guest_binding: bool,
) -> (Vec<ArtifactDoc>, Vec<WarningDoc>) {
    let invoke_re = Regex::new(
        r#"(?:\binvoke|\btauri\.invoke|\bwindow\.__TAURI__\.invoke)\(\s*['"]([^'"]+)['"]"#,
    )
    .expect("valid regex");
    let event_re = Regex::new(r#"\b(listen|once|emit)\(\s*['"]([^'"]+)['"]"#).expect("valid regex");
    let channel_re =
        Regex::new(r"\b(?:const|let)\s+([A-Za-z0-9_]+)\s*=\s*new\s+Channel").expect("valid regex");
    let export_fn =
        Regex::new(r"(?m)^\s*export\s+async\s+function\s+([A-Za-z0-9_]+)").expect("valid regex");

    let mut artifacts = Vec::new();
    let mut warnings = Vec::new();

    for capture in invoke_re.captures_iter(text) {
        let whole = capture.get(0).expect("match");
        let invoke_key = capture.get(1).expect("key").as_str();
        let line = line_number(text, whole.start());
        let name = invoke_key
            .split('|')
            .next_back()
            .unwrap_or(invoke_key)
            .split(':')
            .next_back()
            .unwrap_or(invoke_key);
        let mut doc = new_artifact(config, path, "tauri_invoke", name, line);
        doc.display_name = Some(format!("invoke {invoke_key}"));
        doc.tags = vec!["tauri invoke".to_owned()];
        doc.data.insert(
            "invoke_key".to_owned(),
            Value::String(invoke_key.to_owned()),
        );
        doc.data
            .insert("command_name".to_owned(), Value::String(name.to_owned()));
        if let Some(plugin_name) = invoke_key
            .strip_prefix("plugin:")
            .and_then(|value| value.split('|').next())
        {
            doc.data.insert(
                "plugin_name".to_owned(),
                Value::String(plugin_name.to_owned()),
            );
        }
        apply_artifact_security(&mut doc);
        artifacts.push(doc);
    }

    for capture in channel_re.captures_iter(text) {
        let whole = capture.get(0).expect("match");
        let channel_name = capture.get(1).expect("name").as_str();
        let line = line_number(text, whole.start());
        let mut doc = new_artifact(config, path, "tauri_channel", channel_name, line);
        doc.display_name = Some(format!("Channel {channel_name}"));
        doc.data.insert(
            "channel_name".to_owned(),
            Value::String(channel_name.to_owned()),
        );
        apply_artifact_security(&mut doc);
        artifacts.push(doc);
    }

    for capture in event_re.captures_iter(text) {
        let whole = capture.get(0).expect("match");
        let verb = capture.get(1).expect("verb").as_str();
        let event_name = capture.get(2).expect("event").as_str();
        let line = line_number(text, whole.start());
        let kind = match verb {
            "emit" => "tauri_event_emit",
            _ => "tauri_event_listener",
        };
        let mut doc = new_artifact(config, path, kind, event_name, line);
        doc.data.insert(
            "event_name".to_owned(),
            Value::String(event_name.to_owned()),
        );
        doc.tags = vec!["event".to_owned()];
        apply_artifact_security(&mut doc);
        artifacts.push(doc);
    }

    if guest_binding {
        for capture in export_fn.captures_iter(text) {
            let whole = capture.get(0).expect("match");
            let export_name = capture.get(1).expect("name").as_str();
            let line = line_number(text, whole.start());
            let mut doc = new_artifact(config, path, "tauri_plugin_binding", export_name, line);
            doc.display_name = Some(format!("{export_name} plugin binding"));
            doc.data.insert(
                "plugin_export".to_owned(),
                Value::String(export_name.to_owned()),
            );
            if let Some(call) = invoke_re.captures(text) {
                let invoke_key = call.get(1).expect("invoke key").as_str();
                doc.data.insert(
                    "invoke_key".to_owned(),
                    Value::String(invoke_key.to_owned()),
                );
                if let Some(plugin_name) = invoke_key
                    .strip_prefix("plugin:")
                    .and_then(|value| value.split('|').next())
                {
                    doc.data.insert(
                        "plugin_name".to_owned(),
                        Value::String(plugin_name.to_owned()),
                    );
                }
            }
            apply_artifact_security(&mut doc);
            artifacts.push(doc);
        }
    }

    let dynamic_invoke = Regex::new(
        r"\b(?:invoke|tauri\.invoke|window\.__TAURI__\.invoke)\(\s*([A-Za-z_][A-Za-z0-9_]*)",
    )
    .expect("valid regex");
    for capture in dynamic_invoke.captures_iter(text) {
        let variable = capture.get(1).expect("name").as_str();
        let line = line_number(text, capture.get(0).expect("match").start());
        warnings.push(warning(
            config,
            path,
            "dynamic_invoke",
            &format!("Cannot statically resolve Tauri command name from {variable}"),
            line,
        ));
    }

    (artifacts, warnings)
}
