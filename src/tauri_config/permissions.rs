use anyhow::Result;

use crate::{config::ResolvedConfig, discovery::RepoDiscovery, model::ArtifactDoc};

pub fn extract_app_permissions(
    _config: &ResolvedConfig,
    _discovery: &RepoDiscovery,
) -> Result<Vec<ArtifactDoc>> {
    Ok(Vec::new())
}
