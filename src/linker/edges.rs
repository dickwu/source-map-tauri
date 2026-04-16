use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde_json::Value;

use crate::{
    ids::document_id,
    model::{ArtifactDoc, EdgeDoc, WarningDoc},
};

fn edge(
    repo: &str,
    edge_type: &str,
    from: &ArtifactDoc,
    to: &ArtifactDoc,
    reason: &str,
    confidence: f32,
) -> EdgeDoc {
    EdgeDoc {
        id: document_id(
            repo,
            "edge",
            from.source_path.as_deref(),
            from.line_start,
            Some(&format!("{edge_type}:{}:{}", from.id, to.id)),
        ),
        repo: repo.to_owned(),
        kind: "edge".to_owned(),
        edge_type: edge_type.to_owned(),
        from_id: from.id.clone(),
        from_kind: from.kind.clone(),
        from_name: from.name.clone(),
        to_id: to.id.clone(),
        to_kind: to.kind.clone(),
        to_name: to.name.clone(),
        confidence,
        reason: reason.to_owned(),
        source_path: from.source_path.clone(),
        line_start: from.line_start,
        risk_level: from.risk_level.clone(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn warning(
    repo: &str,
    warning_type: &str,
    message: String,
    related: Option<&ArtifactDoc>,
    risk_level: &str,
) -> WarningDoc {
    WarningDoc {
        id: document_id(
            repo,
            "warning",
            related.and_then(|item| item.source_path.as_deref()),
            related.and_then(|item| item.line_start),
            Some(warning_type),
        ),
        repo: repo.to_owned(),
        kind: "warning".to_owned(),
        warning_type: warning_type.to_owned(),
        severity: if risk_level == "critical" {
            "error"
        } else {
            "warning"
        }
        .to_owned(),
        message,
        source_path: related.and_then(|item| item.source_path.clone()),
        line_start: related.and_then(|item| item.line_start),
        related_id: related.map(|item| item.id.clone()),
        risk_level: risk_level.to_owned(),
        remediation: None,
        updated_at: chrono::Utc::now().to_rfc3339(),
    }
}

pub fn link_all(
    artifacts: &mut [ArtifactDoc],
    warnings: &mut Vec<WarningDoc>,
) -> Result<Vec<EdgeDoc>> {
    let mut edges = Vec::new();
    let mut by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut by_invoke: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_kind: BTreeMap<String, Vec<usize>> = BTreeMap::new();

    for (index, artifact) in artifacts.iter().enumerate() {
        if let Some(name) = &artifact.name {
            by_name.entry(name.clone()).or_default().push(index);
        }
        if let Some(invoke_key) = artifact.data.get("invoke_key").and_then(Value::as_str) {
            by_invoke.insert(invoke_key.to_owned(), index);
        }
        by_kind
            .entry(artifact.kind.clone())
            .or_default()
            .push(index);
    }

    let mut test_links: BTreeMap<usize, BTreeSet<String>> = BTreeMap::new();

    for artifact in artifacts.iter() {
        if artifact.kind == "frontend_hook_use" {
            if let Some(name) = artifact.data.get("hook_def_name").and_then(Value::as_str) {
                if let Some(target_index) =
                    by_kind.get("frontend_hook_def").and_then(|candidates| {
                        candidates
                            .iter()
                            .copied()
                            .find(|candidate| artifacts[*candidate].name.as_deref() == Some(name))
                    })
                {
                    edges.push(edge(
                        &artifact.repo,
                        "uses_hook",
                        artifact,
                        &artifacts[target_index],
                        "hook callsite name matches hook definition",
                        0.95,
                    ));
                }
            }
        }

        if artifact.kind == "tauri_invoke" {
            if let Some(invoke_key) = artifact.data.get("invoke_key").and_then(Value::as_str) {
                if let Some(target_index) = by_invoke
                    .get(invoke_key)
                    .copied()
                    .filter(|candidate| artifacts[*candidate].kind == "tauri_plugin_command")
                    .or_else(|| {
                        let name = artifact.data.get("command_name").and_then(Value::as_str)?;
                        by_kind.get("tauri_command").and_then(|candidates| {
                            candidates.iter().copied().find(|candidate| {
                                artifacts[*candidate].name.as_deref() == Some(name)
                            })
                        })
                    })
                {
                    let edge_type = if artifacts[target_index].kind == "tauri_plugin_command" {
                        "invokes_plugin_command"
                    } else {
                        "invokes"
                    };
                    edges.push(edge(
                        &artifact.repo,
                        edge_type,
                        artifact,
                        &artifacts[target_index],
                        "invoke key matches command registration",
                        0.98,
                    ));
                }
            }
        }

        if artifact.kind == "tauri_plugin_binding" {
            if let Some(invoke_key) = artifact.data.get("invoke_key").and_then(Value::as_str) {
                if let Some(target_index) = by_invoke
                    .get(invoke_key)
                    .copied()
                    .filter(|candidate| artifacts[*candidate].kind == "tauri_plugin_command")
                {
                    edges.push(edge(
                        &artifact.repo,
                        "invokes_plugin_command",
                        artifact,
                        &artifacts[target_index],
                        "plugin binding invoke key matches plugin command",
                        0.99,
                    ));
                }
            }
        }

        if artifact.kind == "tauri_capability" {
            let capability_permissions = artifact
                .data
                .get("permissions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for permission in capability_permissions.iter().filter_map(Value::as_str) {
                if let Some(target_index) = by_kind.get("tauri_permission").and_then(|candidates| {
                    candidates.iter().copied().find(|candidate| {
                        artifacts[*candidate]
                            .name
                            .as_deref()
                            .map(|name| {
                                name == permission || name.ends_with(&format!(":{permission}"))
                            })
                            .unwrap_or(false)
                    })
                }) {
                    edges.push(edge(
                        &artifact.repo,
                        "capability_grants_permission",
                        artifact,
                        &artifacts[target_index],
                        "capability permissions include permission identifier",
                        0.9,
                    ));
                }
            }
        }

        if artifact.kind == "frontend_test" {
            let imports = artifact
                .data
                .get("imports")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mocked_commands = artifact
                .data
                .get("mocked_commands")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for import in imports.iter().filter_map(Value::as_str) {
                if let Some(candidates) = by_name.get(import) {
                    for target_index in candidates {
                        test_links
                            .entry(*target_index)
                            .or_default()
                            .insert(artifact.source_path.clone().unwrap_or_default());
                        edges.push(edge(
                            &artifact.repo,
                            "tested_by",
                            &artifacts[*target_index],
                            artifact,
                            "frontend test imports symbol",
                            0.9,
                        ));
                    }
                }
            }
            for command in mocked_commands.iter().filter_map(Value::as_str) {
                if let Some(target_index) = by_invoke.get(command).copied() {
                    test_links
                        .entry(target_index)
                        .or_default()
                        .insert(artifact.source_path.clone().unwrap_or_default());
                    edges.push(edge(
                        &artifact.repo,
                        "mocked_by",
                        &artifacts[target_index],
                        artifact,
                        "frontend test mocks invoke key",
                        0.95,
                    ));
                }
            }
        }

        if artifact.kind == "rust_test" {
            let targets = artifact
                .data
                .get("targets")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for target in targets.iter().filter_map(Value::as_str) {
                if let Some(candidates) = by_name.get(target) {
                    for target_index in candidates {
                        test_links
                            .entry(*target_index)
                            .or_default()
                            .insert(artifact.source_path.clone().unwrap_or_default());
                        edges.push(edge(
                            &artifact.repo,
                            "tested_by",
                            &artifacts[*target_index],
                            artifact,
                            "rust test target name matches artifact name",
                            0.85,
                        ));
                    }
                }
            }
        }
    }

    for (index, related_tests) in test_links {
        let artifact = &mut artifacts[index];
        artifact.related_tests = related_tests.into_iter().collect();
        artifact.has_related_tests = !artifact.related_tests.is_empty();
    }

    let warning_exists = |warning_type: &str, related_id: &str, warnings: &[WarningDoc]| {
        warnings.iter().any(|item| {
            item.warning_type == warning_type && item.related_id.as_deref() == Some(related_id)
        })
    };

    for artifact in artifacts.iter() {
        if (artifact.kind == "tauri_command" || artifact.kind == "tauri_plugin_command")
            && artifact.risk_level != "low"
            && artifact.related_tests.is_empty()
        {
            warnings.push(warning(
                &artifact.repo,
                "missing_related_test",
                format!(
                    "{} is high-risk and has no related tests",
                    artifact.name.clone().unwrap_or_default()
                ),
                Some(artifact),
                &artifact.risk_level,
            ));
        }

        if artifact.kind == "tauri_plugin_command" {
            let plugin_name = artifact.data.get("plugin_name").and_then(Value::as_str);
            let command_name = artifact.name.as_deref().unwrap_or_default();
            let has_permission = artifacts.iter().any(|candidate| {
                candidate.kind == "tauri_permission"
                    && candidate.data.get("plugin_name").and_then(Value::as_str) == plugin_name
                    && candidate
                        .data
                        .get("commands_allow")
                        .and_then(Value::as_array)
                        .map(|items| items.iter().any(|item| item.as_str() == Some(command_name)))
                        .unwrap_or(false)
            });
            if !has_permission
                && !warning_exists(
                    "plugin_command_without_permission_evidence",
                    &artifact.id,
                    warnings,
                )
            {
                warnings.push(warning(
                    &artifact.repo,
                    "plugin_command_without_permission_evidence",
                    format!(
                        "{} has no permission evidence",
                        artifact.name.clone().unwrap_or_default()
                    ),
                    Some(artifact),
                    &artifact.risk_level,
                ));
            }
        }

        if artifact.kind == "tauri_command"
            && !warning_exists(
                "command_without_permission_evidence",
                &artifact.id,
                warnings,
            )
        {
            warnings.push(warning(
                &artifact.repo,
                "command_without_permission_evidence",
                format!(
                    "{} has no permission evidence",
                    artifact.name.clone().unwrap_or_default()
                ),
                Some(artifact),
                &artifact.risk_level,
            ));
        }
    }

    Ok(edges)
}
