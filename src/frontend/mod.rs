pub mod hooks;
pub mod swc;
pub mod tauri_calls;
pub mod tests;

use std::path::Path;

use anyhow::Result;

use crate::{
    config::ResolvedConfig,
    discovery::RepoDiscovery,
    model::{ArtifactDoc, WarningDoc},
};

pub fn extract(
    config: &ResolvedConfig,
    discovery: &RepoDiscovery,
) -> Result<(Vec<ArtifactDoc>, Vec<WarningDoc>)> {
    let mut artifacts = Vec::new();
    let mut warnings = Vec::new();
    let known_hooks =
        swc::discover_hook_names(discovery).or_else(|_| hooks::discover_hook_names(discovery))?;

    for path in &discovery.frontend_files {
        let text = std::fs::read_to_string(path)?;
        match swc::extract_file(config, path, &text, &known_hooks, false) {
            Ok((file_artifacts, file_warnings)) => {
                artifacts.extend(file_artifacts);
                warnings.extend(file_warnings);
            }
            Err(_) => {
                artifacts.extend(hooks::extract_components_and_hooks(
                    config,
                    path,
                    &text,
                    &known_hooks,
                ));
                let (call_artifacts, call_warnings) =
                    tauri_calls::extract_calls(config, path, &text, false);
                artifacts.extend(call_artifacts);
                warnings.extend(call_warnings);
            }
        }
    }

    for path in &discovery.guest_js_files {
        let text = std::fs::read_to_string(path)?;
        match swc::extract_file(config, path, &text, &known_hooks, true) {
            Ok((file_artifacts, file_warnings)) => {
                artifacts.extend(file_artifacts);
                warnings.extend(file_warnings);
            }
            Err(_) => {
                let (call_artifacts, call_warnings) =
                    tauri_calls::extract_calls(config, path, &text, true);
                artifacts.extend(call_artifacts);
                warnings.extend(call_warnings);
            }
        }
    }

    for path in &discovery.frontend_test_files {
        let text = std::fs::read_to_string(path)?;
        artifacts.extend(tests::extract_frontend_tests(config, path, &text));
    }

    Ok((artifacts, warnings))
}

pub fn language_for_path(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|item| item.to_str())
        .map(|item| item.to_owned())
}
