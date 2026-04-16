pub mod cli;
pub mod config;
pub mod discovery;
pub mod frontend;
pub mod ids;
pub mod linker;
pub mod lsp;
pub mod meili;
pub mod model;
pub mod output;
pub mod projects;
pub mod r#rust;
pub mod scan;
pub mod security;
pub mod sourcemaps;
pub mod tauri_config;
pub mod validate;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command};

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = config::ResolvedConfig::from_cli(&cli)?;

    match &cli.command {
        Command::Init => config::init_project(&config),
        Command::Doctor => {
            let report = discovery::doctor(&config)?;
            let mut value = serde_json::to_value(report)?;
            if let Some(health) = meili::doctor_health(&config) {
                value["meilisearch_health"] = health;
            }
            println!("{}", serde_json::to_string_pretty(&value)?);
            Ok(())
        }
        Command::Scan { out } => {
            let runtime = config.with_output_override(out.clone());
            let bundle = scan::scan_project(&runtime)?;
            output::write_scan_bundle(&runtime.output_dir, &bundle)?;
            println!("{}", serde_json::to_string_pretty(&bundle.summary)?);
            Ok(())
        }
        Command::Upload {
            meili_url,
            meili_key,
            index,
            input,
            edges,
            warnings,
            wait,
            batch_size,
        } => meili::upload(
            &config,
            meili::UploadRequest {
                meili_url: meili_url.as_deref(),
                meili_key: meili_key.as_deref(),
                index: index.as_deref(),
                input,
                edges: edges.as_ref().map(|path| path.as_path()),
                warnings: warnings.as_ref().map(|path| path.as_path()),
                wait: *wait,
                _batch_size: *batch_size,
            },
        ),
        Command::Reindex {
            meili_url,
            meili_key,
            index,
            out,
            wait,
            batch_size,
        } => {
            let runtime = config.with_output_override(out.clone());
            let mut bundle = scan::scan_project(&runtime)?;
            if let Some(index_name) = index.as_ref() {
                bundle.project_info.index_uid = index_name.clone();
            }
            output::write_scan_bundle(&runtime.output_dir, &bundle)?;
            meili::upload(
                &runtime,
                meili::UploadRequest {
                    meili_url: meili_url.as_deref(),
                    meili_key: meili_key.as_deref(),
                    index: index.as_deref(),
                    input: &runtime.output_dir.join("artifacts.ndjson"),
                    edges: Some(&runtime.output_dir.join("edges.ndjson")),
                    warnings: Some(&runtime.output_dir.join("warnings.ndjson")),
                    wait: *wait,
                    _batch_size: *batch_size,
                },
            )
        }
        Command::Search {
            meili_url,
            meili_key,
            index,
            query,
            filter,
            limit,
        } => meili::search(
            &config,
            meili_url.as_deref(),
            meili_key.as_deref(),
            index.as_deref(),
            query,
            filter.as_deref(),
            *limit,
        ),
        Command::Trace {
            bundle,
            line,
            column,
        } => {
            let result =
                sourcemaps::trace::trace_bundle_frame(&config.root, bundle, *line, *column)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
        Command::Validate { input } => {
            validate::validate_output_dir(input)?;
            println!("validation ok");
            Ok(())
        }
        Command::PrintSchema { kind } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&model::schema_for_kind(kind)?)?
            );
            Ok(())
        }
    }
}
