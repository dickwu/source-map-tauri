pub mod capabilities;
pub mod effective;
pub mod permissions;

use anyhow::Result;

use crate::{config::ResolvedConfig, discovery::RepoDiscovery, model::ArtifactDoc};

pub fn extract(config: &ResolvedConfig, discovery: &RepoDiscovery) -> Result<Vec<ArtifactDoc>> {
    let mut artifacts = capabilities::extract_capabilities(config, discovery)?;
    artifacts.extend(permissions::extract_app_permissions(config, discovery)?);
    artifacts.extend(effective::extract_effective_capabilities(
        config, discovery,
    )?);
    Ok(artifacts)
}
