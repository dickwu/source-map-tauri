pub mod edges;
pub mod http_flows;

use anyhow::Result;

use crate::model::{ArtifactDoc, EdgeDoc, WarningDoc};

pub fn link(
    artifacts: &mut Vec<ArtifactDoc>,
    warnings: &mut Vec<WarningDoc>,
) -> Result<Vec<EdgeDoc>> {
    let mut edges = edges::link_all(artifacts.as_mut_slice(), warnings)?;
    http_flows::augment_frontend_http_flows(artifacts, &mut edges, warnings)?;
    Ok(edges)
}
