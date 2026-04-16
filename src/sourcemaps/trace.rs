use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TraceResult {
    pub bundle_path: String,
    pub generated_line: u32,
    pub generated_column: u32,
    pub message: String,
}

pub fn trace_bundle_frame(
    root: &Path,
    bundle: &PathBuf,
    line: u32,
    column: u32,
) -> Result<TraceResult> {
    let bundle_path = if bundle.is_absolute() {
        bundle.clone()
    } else {
        root.join(bundle)
    };
    if !bundle_path.exists() {
        return Err(anyhow!("bundle not found: {}", bundle_path.display()));
    }
    Ok(TraceResult {
        bundle_path: bundle_path.to_string_lossy().to_string(),
        generated_line: line,
        generated_column: column,
        message: "Direct sourcemap tracing is not implemented yet; run scan when sourcemap artifacts are available.".to_owned(),
    })
}
