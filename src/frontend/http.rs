use std::path::Path;

use regex::Regex;
use serde_json::{json, Map, Value};

use crate::{
    config::{normalize_path, ResolvedConfig},
    ids::document_id,
    model::ArtifactDoc,
    security::apply_artifact_security,
};

#[derive(Debug)]
struct ExportedFunction<'a> {
    name: &'a str,
    line: u32,
    body: &'a str,
}

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

pub fn extract_http_artifacts(
    config: &ResolvedConfig,
    path: &Path,
    text: &str,
) -> Vec<ArtifactDoc> {
    let mut artifacts = Vec::new();

    for exported in exported_functions(text) {
        if let Some((transport_name, method, normalized_path)) =
            wrapper_transport_call(exported.body)
        {
            let mut doc = base_artifact(
                config,
                path,
                "frontend_api_wrapper",
                exported.name,
                exported.line,
            );
            doc.display_name = Some(format!("{} wrapper", exported.name));
            doc.tags = vec!["api wrapper".to_owned(), "http".to_owned()];
            doc.data.insert(
                "transport_name".to_owned(),
                Value::String(transport_name.to_owned()),
            );
            doc.data
                .insert("http_method".to_owned(), Value::String(method.to_owned()));
            doc.data.insert(
                "normalized_path".to_owned(),
                Value::String(normalized_path.clone()),
            );
            doc.data.insert(
                "endpoint_key".to_owned(),
                Value::String(format!("{method} {normalized_path}")),
            );
            apply_artifact_security(&mut doc);
            artifacts.push(doc);
        } else if let Some((method, normalized_path)) = direct_http_call(exported.body) {
            let mut doc = base_artifact(
                config,
                path,
                "frontend_api_wrapper",
                exported.name,
                exported.line,
            );
            doc.display_name = Some(format!("{} wrapper", exported.name));
            doc.tags = vec!["api wrapper".to_owned(), "http".to_owned()];
            doc.data.insert(
                "transport_name".to_owned(),
                Value::String("tauriFetch".to_owned()),
            );
            doc.data
                .insert("http_method".to_owned(), Value::String(method.to_owned()));
            doc.data.insert(
                "normalized_path".to_owned(),
                Value::String(normalized_path.clone()),
            );
            doc.data.insert(
                "endpoint_key".to_owned(),
                Value::String(format!("{method} {normalized_path}")),
            );
            apply_artifact_security(&mut doc);
            artifacts.push(doc);
        }

        if let Some((method, client_name, path_param, url_pattern)) =
            transport_definition(exported.body)
        {
            let mut doc = base_artifact(
                config,
                path,
                "frontend_transport",
                exported.name,
                exported.line,
            );
            doc.display_name = Some(format!("{} transport", exported.name));
            doc.tags = vec!["transport".to_owned(), "http".to_owned()];
            doc.data
                .insert("http_method".to_owned(), Value::String(method.to_owned()));
            doc.data.insert(
                "http_client".to_owned(),
                Value::String(client_name.to_owned()),
            );
            doc.data.insert(
                "path_param".to_owned(),
                Value::String(path_param.to_owned()),
            );
            doc.data.insert(
                "url_pattern".to_owned(),
                Value::String(url_pattern.to_owned()),
            );
            doc.data.insert(
                "transport_signature".to_owned(),
                json!({
                    "client": client_name,
                    "method": method,
                    "path_param": path_param,
                }),
            );
            apply_artifact_security(&mut doc);
            artifacts.push(doc);
        }
    }

    artifacts
}

fn exported_functions(text: &str) -> Vec<ExportedFunction<'_>> {
    let function_re = Regex::new(r"(?m)^\s*export\s+function\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")
        .expect("valid regex");
    let const_re =
        Regex::new(r"(?m)^\s*export\s+const\s+([A-Za-z_][A-Za-z0-9_]*)\s*=").expect("valid regex");

    let mut items = Vec::new();
    for regex in [&function_re, &const_re] {
        for capture in regex.captures_iter(text) {
            let whole = capture.get(0).expect("match");
            let Some(name) = capture.get(1).map(|value| value.as_str()) else {
                continue;
            };
            let Some(body_start) = text[whole.start()..]
                .find('{')
                .map(|offset| whole.start() + offset)
            else {
                continue;
            };
            let Some(body_end) = find_matching_brace(text, body_start) else {
                continue;
            };
            items.push(ExportedFunction {
                name,
                line: line_number(text, whole.start()),
                body: &text[body_start + 1..body_end],
            });
        }
    }

    items.sort_by_key(|item| item.line);
    items
}

fn find_matching_brace(text: &str, open_index: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0_u32;
    let mut index = open_index;
    let mut in_single = false;
    let mut in_double = false;
    let mut in_template = false;
    let mut line_comment = false;
    let mut block_comment = false;
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];
        let next = bytes.get(index + 1).copied();

        if line_comment {
            if byte == b'\n' {
                line_comment = false;
            }
            index += 1;
            continue;
        }

        if block_comment {
            if byte == b'*' && next == Some(b'/') {
                block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        if escaped {
            escaped = false;
            index += 1;
            continue;
        }

        match byte {
            b'\\' if in_single || in_double || in_template => {
                escaped = true;
                index += 1;
            }
            b'\'' if !in_double && !in_template => {
                in_single = !in_single;
                index += 1;
            }
            b'"' if !in_single && !in_template => {
                in_double = !in_double;
                index += 1;
            }
            b'`' if !in_single && !in_double => {
                in_template = !in_template;
                index += 1;
            }
            b'/' if !in_single && !in_double && !in_template && next == Some(b'/') => {
                line_comment = true;
                index += 2;
            }
            b'/' if !in_single && !in_double && !in_template && next == Some(b'*') => {
                block_comment = true;
                index += 2;
            }
            b'{' if !in_single && !in_double => {
                depth += 1;
                index += 1;
            }
            b'}' if !in_single && !in_double => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }

    None
}

fn wrapper_transport_call(body: &str) -> Option<(&'static str, &'static str, String)> {
    let call_re = Regex::new(
        r#"\b(usePostApi|usePostMutation|usePostUploadMutation|postApi)\s*\(\s*["']([^"']+)["']"#,
    )
    .expect("valid regex");
    let capture = call_re.captures(body)?;
    let transport_name = capture.get(1)?.as_str();
    let raw_path = capture.get(2)?.as_str();
    let method = match transport_name {
        "usePostApi" | "usePostMutation" | "usePostUploadMutation" | "postApi" => "POST",
        _ => return None,
    };
    Some((
        transport_name_static(transport_name)?,
        method,
        normalize_http_path(raw_path),
    ))
}

fn transport_definition(
    body: &str,
) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
    let pattern_re = Regex::new(
        r#"(?s)\btauriFetch\s*\(\s*`[^`]*\$\{API_URL\}/\$\{([A-Za-z_][A-Za-z0-9_]*)\}[^`]*`\s*,\s*\{.*?method\s*:\s*["']([A-Z]+)["']"#,
    )
    .expect("valid regex");
    let capture = pattern_re.captures(body)?;
    let path_param = capture.get(1)?.as_str();
    let method = capture.get(2)?.as_str();
    if path_param != "path" || method != "POST" {
        return None;
    }
    Some(("POST", "tauriFetch", "path", "${API_URL}/${path}"))
}

fn direct_http_call(body: &str) -> Option<(&'static str, String)> {
    let direct_re = Regex::new(
        r#"(?s)\btauriFetch\s*\(\s*`[^`]*\$\{API_URL\}/([^`$]+)`\s*,\s*\{.*?method\s*:\s*["']([A-Z]+)["']"#,
    )
    .expect("valid regex");
    let capture = direct_re.captures(body)?;
    let raw_path = capture.get(1)?.as_str().trim();
    let method = capture.get(2)?.as_str();
    if method != "POST" {
        return None;
    }
    Some((method_static(method)?, normalize_http_path(raw_path)))
}

fn normalize_http_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.starts_with('/') {
        trimmed.to_owned()
    } else {
        format!("/{trimmed}")
    }
}

fn transport_name_static(name: &str) -> Option<&'static str> {
    match name {
        "usePostApi" => Some("usePostApi"),
        "usePostMutation" => Some("usePostMutation"),
        "usePostUploadMutation" => Some("usePostUploadMutation"),
        "postApi" => Some("postApi"),
        _ => None,
    }
}

fn method_static(method: &str) -> Option<&'static str> {
    match method {
        "POST" => Some("POST"),
        "GET" => Some("GET"),
        "PUT" => Some("PUT"),
        "PATCH" => Some("PATCH"),
        "DELETE" => Some("DELETE"),
        _ => None,
    }
}
