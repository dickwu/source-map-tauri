use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "source-map-tauri")]
#[command(version, about = "Build a searchable source map for Tauri apps")]
pub struct Cli {
    #[arg(long, global = true, default_value = ".")]
    pub root: PathBuf,

    #[arg(long, global = true)]
    pub repo: Option<String>,

    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[arg(long, global = true)]
    pub strict: bool,

    #[arg(long, global = true)]
    pub verbose: bool,

    #[arg(long, global = true)]
    pub quiet: bool,

    #[arg(long, global = true)]
    pub include_node_modules: bool,

    #[arg(long, global = true)]
    pub include_target: bool,

    #[arg(long, global = true)]
    pub include_dist: bool,

    #[arg(long, global = true)]
    pub include_vendor: bool,

    #[arg(long, global = true, default_value_t = true)]
    pub redact_secrets: bool,

    #[arg(long, global = true, default_value_t = true)]
    pub detect_phi: bool,

    #[arg(long, global = true, default_value_t = false)]
    pub fail_on_phi: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Init,
    Doctor,
    Scan {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Upload {
        #[arg(long, env = "MEILI_URL")]
        meili_url: Option<String>,
        #[arg(long, env = "MEILI_MASTER_KEY")]
        meili_key: Option<String>,
        #[arg(long)]
        index: Option<String>,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        edges: Option<PathBuf>,
        #[arg(long)]
        warnings: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 5000)]
        batch_size: usize,
    },
    Reindex {
        #[arg(long, env = "MEILI_URL")]
        meili_url: Option<String>,
        #[arg(long, env = "MEILI_MASTER_KEY")]
        meili_key: Option<String>,
        #[arg(long)]
        index: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 5000)]
        batch_size: usize,
    },
    Search {
        #[arg(long, env = "MEILI_URL")]
        meili_url: Option<String>,
        #[arg(long, env = "MEILI_SEARCH_KEY")]
        meili_key: Option<String>,
        #[arg(long)]
        index: Option<String>,
        #[arg(long)]
        query: String,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    Trace {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        line: u32,
        #[arg(long, default_value_t = 0)]
        column: u32,
    },
    Validate {
        #[arg(long)]
        input: PathBuf,
    },
    PrintSchema {
        #[arg(long)]
        kind: String,
    },
}
