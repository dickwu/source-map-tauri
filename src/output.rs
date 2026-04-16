use std::{
    collections::BTreeSet,
    fs,
    io::{BufWriter, Write},
    path::Path,
};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    model::{ArtifactDoc, EdgeDoc, ScanSummary, WarningDoc},
    scan::ScanBundle,
};

pub fn write_scan_bundle(output_dir: &Path, bundle: &ScanBundle) -> Result<()> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    write_ndjson(output_dir.join("artifacts.ndjson"), &bundle.artifacts)?;
    write_ndjson(output_dir.join("edges.ndjson"), &bundle.edges)?;
    write_ndjson(output_dir.join("warnings.ndjson"), &bundle.warnings)?;
    fs::write(
        output_dir.join("summary.json"),
        serde_json::to_vec_pretty(&bundle.summary)?,
    )?;
    fs::write(
        output_dir.join("project-info.json"),
        serde_json::to_vec_pretty(&bundle.project_info)?,
    )?;
    fs::write(
        output_dir.join("meili-settings.json"),
        serde_json::to_vec_pretty(&default_meili_settings())?,
    )?;
    Ok(())
}

pub fn write_ndjson<T: Serialize>(path: impl AsRef<Path>, docs: &[T]) -> Result<()> {
    let file = fs::File::create(path.as_ref())
        .with_context(|| format!("failed to write {}", path.as_ref().display()))?;
    let mut writer = BufWriter::new(file);
    for doc in docs {
        serde_json::to_writer(&mut writer, doc)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

pub fn build_summary(
    repo: &str,
    artifacts: &[ArtifactDoc],
    edges: &[EdgeDoc],
    warnings: &[WarningDoc],
) -> ScanSummary {
    let artifact_kinds = artifacts
        .iter()
        .map(|item| item.kind.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let warning_types = warnings
        .iter()
        .map(|item| item.warning_type.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    ScanSummary {
        repo: repo.to_owned(),
        artifact_count: artifacts.len(),
        edge_count: edges.len(),
        warning_count: warnings.len(),
        artifact_kinds,
        warning_types,
        scanned_at: chrono::Utc::now().to_rfc3339(),
    }
}

pub fn default_meili_settings() -> Value {
    json!({
        "searchableAttributes": [
            "name",
            "invoke_key",
            "command_name",
            "plugin_name",
            "plugin_export",
            "hook_name",
            "hook_kind",
            "event_name",
            "channel_name",
            "rust_fqn",
            "component",
            "display_name",
            "signature",
            "source_path",
            "bundle_path",
            "nearest_symbol",
            "permissions",
            "effective_capabilities",
            "target_rust_commands",
            "called_by_frontend",
            "related_symbols",
            "related_php_symbols",
            "related_tests",
            "risk_reasons",
            "tags",
            "comments",
            "package_name"
        ],
        "filterableAttributes": [
            "repo",
            "kind",
            "side",
            "language",
            "source_path",
            "package_name",
            "risk_level",
            "contains_phi",
            "has_related_tests",
            "command_name",
            "invoke_key",
            "plugin_name",
            "plugin_export",
            "hook_name",
            "hook_kind",
            "component",
            "event_name",
            "channel_name",
            "window_label",
            "webview_label",
            "capability_id",
            "permission_id",
            "merged_capabilities",
            "remote_capability",
            "from_id",
            "to_id",
            "from_kind",
            "to_kind",
            "edge_type",
            "warning_type",
            "severity"
        ],
        "sortableAttributes": ["confidence", "updated_at"],
        "rankingRules": [
            "words",
            "typo",
            "proximity",
            "attribute",
            "sort",
            "exactness"
        ]
    })
}
