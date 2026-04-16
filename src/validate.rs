use std::{collections::BTreeSet, fs, path::Path};

use anyhow::{bail, Context, Result};

use crate::{
    ids::is_safe_document_id,
    model::{ArtifactDoc, EdgeDoc, WarningDoc},
};

fn read_ndjson<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Vec<T>> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("invalid ndjson"))
        .collect()
}

pub fn validate_output_dir(path: &Path) -> Result<()> {
    let artifacts: Vec<ArtifactDoc> = read_ndjson(&path.join("artifacts.ndjson"))?;
    let edges: Vec<EdgeDoc> = read_ndjson(&path.join("edges.ndjson"))?;
    let warnings: Vec<WarningDoc> = read_ndjson(&path.join("warnings.ndjson"))?;
    let ids = artifacts
        .iter()
        .map(|item| item.id.clone())
        .collect::<BTreeSet<_>>();

    for artifact in &artifacts {
        if !is_safe_document_id(&artifact.id) {
            bail!("invalid artifact id {}", artifact.id);
        }
        if artifact.repo.is_empty() || artifact.kind.is_empty() {
            bail!("artifact missing repo or kind");
        }
        if artifact.related_tests.is_empty() == artifact.has_related_tests {
            bail!("has_related_tests mismatch for {}", artifact.id);
        }
        if (artifact.risk_level == "high" || artifact.risk_level == "critical")
            && artifact.risk_reasons.is_empty()
        {
            bail!("high-risk artifact missing risk reasons: {}", artifact.id);
        }
    }

    for edge in &edges {
        if !ids.contains(&edge.from_id) || !ids.contains(&edge.to_id) {
            bail!("edge references missing endpoint: {}", edge.id);
        }
    }

    for warning in &warnings {
        if !is_safe_document_id(&warning.id) {
            bail!("invalid warning id {}", warning.id);
        }
    }

    for artifact in artifacts
        .iter()
        .filter(|item| item.kind == "tauri_command" || item.kind == "tauri_plugin_command")
    {
        let has_permission = artifacts.iter().any(|candidate| {
            candidate.kind == "tauri_permission"
                && candidate
                    .data
                    .get("commands_allow")
                    .and_then(serde_json::Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .any(|item| item.as_str() == artifact.name.as_deref())
                    })
                    .unwrap_or(false)
        });
        let has_warning = warnings.iter().any(|candidate| {
            candidate.related_id.as_deref() == Some(&artifact.id)
                && (candidate.warning_type == "command_without_permission_evidence"
                    || candidate.warning_type == "plugin_command_without_permission_evidence")
        });
        if !has_permission && !has_warning && artifact.risk_level != "low" {
            bail!(
                "command missing permission evidence or warning: {}",
                artifact.id
            );
        }
    }

    Ok(())
}
