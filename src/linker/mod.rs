pub mod edges;

use anyhow::Result;

use crate::model::{ArtifactDoc, EdgeDoc, WarningDoc};

pub fn link(artifacts: &mut [ArtifactDoc], warnings: &mut Vec<WarningDoc>) -> Result<Vec<EdgeDoc>> {
    edges::link_all(artifacts, warnings)
}
