use anyhow::Result;

use crate::{
    config::ResolvedConfig,
    discovery, frontend, linker,
    model::{ArtifactDoc, EdgeDoc, ProjectInfo, ScanSummary, WarningDoc},
    output, r#rust,
    security::apply_artifact_security,
    tauri_config,
};

pub struct ScanBundle {
    pub artifacts: Vec<ArtifactDoc>,
    pub edges: Vec<EdgeDoc>,
    pub warnings: Vec<WarningDoc>,
    pub summary: ScanSummary,
    pub project_info: ProjectInfo,
}

pub fn scan_project(config: &ResolvedConfig) -> Result<ScanBundle> {
    let discovery = discovery::discover(config)?;

    let mut artifacts = Vec::new();
    let mut warnings = Vec::new();

    let (frontend_artifacts, frontend_warnings) = frontend::extract(config, &discovery)?;
    artifacts.extend(frontend_artifacts);
    warnings.extend(frontend_warnings);
    artifacts.extend(r#rust::extract(config, &discovery)?);
    artifacts.extend(tauri_config::extract(config, &discovery)?);

    for artifact in &mut artifacts {
        artifact.has_related_tests = !artifact.related_tests.is_empty();
        apply_artifact_security(artifact);
    }

    let edges = linker::link(&mut artifacts, &mut warnings)?;
    let summary = output::build_summary(&config.repo, &artifacts, &edges, &warnings);
    let project_info = ProjectInfo {
        repo: config.repo.clone(),
        repo_path: config.root.to_string_lossy().to_string(),
        output_dir: config.output_dir.to_string_lossy().to_string(),
        index_uid: config.file.meilisearch.index.clone(),
        artifact_count: summary.artifact_count,
        edge_count: summary.edge_count,
        warning_count: summary.warning_count,
        scanned_at: summary.scanned_at.clone(),
    };

    Ok(ScanBundle {
        artifacts,
        edges,
        warnings,
        summary,
        project_info,
    })
}
