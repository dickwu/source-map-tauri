use std::path::Path;

use regex::Regex;
use serde_json::Value;

use crate::{
    config::{normalize_path, ResolvedConfig},
    ids::document_id,
    model::ArtifactDoc,
    security::apply_artifact_security,
};

pub fn extract_frontend_tests(
    config: &ResolvedConfig,
    path: &Path,
    text: &str,
) -> Vec<ArtifactDoc> {
    let import_re =
        Regex::new(r#"import\s+\{?\s*([A-Za-z0-9_,\s]+)\s*\}?\s+from"#).expect("valid regex");
    let command_re = Regex::new(r#""([^"]+)""#).expect("valid regex");

    let source_path = normalize_path(&config.root, path);
    let name = path
        .file_name()
        .and_then(|item| item.to_str())
        .unwrap_or("frontend_test");
    let mut doc = ArtifactDoc {
        id: document_id(
            &config.repo,
            "frontend_test",
            Some(&source_path),
            Some(1),
            Some(name),
        ),
        repo: config.repo.clone(),
        kind: "frontend_test".to_owned(),
        side: Some("test".to_owned()),
        language: crate::frontend::language_for_path(path),
        name: Some(name.to_owned()),
        display_name: Some(name.to_owned()),
        source_path: Some(source_path),
        line_start: Some(1),
        line_end: Some(text.lines().count() as u32),
        column_start: None,
        column_end: None,
        package_name: None,
        comments: Vec::new(),
        tags: vec!["frontend test".to_owned()],
        related_symbols: Vec::new(),
        related_tests: Vec::new(),
        risk_level: "low".to_owned(),
        risk_reasons: Vec::new(),
        contains_phi: false,
        has_related_tests: false,
        updated_at: chrono::Utc::now().to_rfc3339(),
        data: Default::default(),
    };

    let imports = import_re
        .captures_iter(text)
        .flat_map(|capture| {
            capture[1]
                .split(',')
                .map(|item| item.trim().to_owned())
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mocked_commands = if text.contains("mockIPC") {
        command_re
            .captures_iter(text)
            .filter_map(|capture| {
                let value = capture.get(1).expect("command").as_str();
                if value.contains('|')
                    || value
                        .chars()
                        .all(|item| item.is_ascii_alphanumeric() || item == '_' || item == ':')
                {
                    Some(value.to_owned())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    doc.data.insert(
        "imports".to_owned(),
        Value::Array(imports.into_iter().map(Value::String).collect()),
    );
    doc.data.insert(
        "mocked_commands".to_owned(),
        Value::Array(mocked_commands.into_iter().map(Value::String).collect()),
    );
    doc.data.insert(
        "command".to_owned(),
        Value::String(format!(
            "pnpm vitest {}",
            normalize_path(&config.root, path)
        )),
    );

    apply_artifact_security(&mut doc);
    vec![doc]
}
