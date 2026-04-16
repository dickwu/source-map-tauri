use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactDoc {
    pub id: String,
    pub repo: String,
    pub kind: String,
    pub side: Option<String>,
    pub language: Option<String>,
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub source_path: Option<String>,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub column_start: Option<u32>,
    pub column_end: Option<u32>,
    pub package_name: Option<String>,
    pub comments: Vec<String>,
    pub tags: Vec<String>,
    pub related_symbols: Vec<String>,
    pub related_tests: Vec<String>,
    pub risk_level: String,
    pub risk_reasons: Vec<String>,
    pub contains_phi: bool,
    pub has_related_tests: bool,
    pub updated_at: String,
    #[serde(flatten)]
    pub data: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDoc {
    pub id: String,
    pub repo: String,
    pub kind: String,
    pub edge_type: String,
    pub from_id: String,
    pub from_kind: String,
    pub from_name: Option<String>,
    pub to_id: String,
    pub to_kind: String,
    pub to_name: Option<String>,
    pub confidence: f32,
    pub reason: String,
    pub source_path: Option<String>,
    pub line_start: Option<u32>,
    pub risk_level: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarningDoc {
    pub id: String,
    pub repo: String,
    pub kind: String,
    pub warning_type: String,
    pub severity: String,
    pub message: String,
    pub source_path: Option<String>,
    pub line_start: Option<u32>,
    pub related_id: Option<String>,
    pub risk_level: String,
    pub remediation: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    pub repo: String,
    pub artifact_count: usize,
    pub edge_count: usize,
    pub warning_count: usize,
    pub artifact_kinds: Vec<String>,
    pub warning_types: Vec<String>,
    pub scanned_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub repo: String,
    pub repo_path: String,
    pub output_dir: String,
    pub index_uid: String,
    pub artifact_count: usize,
    pub edge_count: usize,
    pub warning_count: usize,
    pub scanned_at: String,
}

pub fn schema_for_kind(kind: &str) -> Result<Value> {
    match kind {
        "artifact" => Ok(json!({
            "type": "object",
            "required": ["id", "repo", "kind", "risk_level", "contains_phi", "has_related_tests", "related_tests"],
            "properties": {
                "id": {"type": "string"},
                "repo": {"type": "string"},
                "kind": {"type": "string"},
                "source_path": {"type": ["string", "null"]},
                "name": {"type": ["string", "null"]},
                "risk_level": {"enum": ["low", "medium", "high", "critical"]},
                "contains_phi": {"type": "boolean"},
                "has_related_tests": {"type": "boolean"},
                "related_tests": {"type": "array", "items": {"type": "string"}}
            }
        })),
        "edge" => Ok(json!({
            "type": "object",
            "required": ["id", "repo", "kind", "edge_type", "from_id", "to_id"],
            "properties": {
                "kind": {"const": "edge"},
                "edge_type": {"type": "string"},
                "from_id": {"type": "string"},
                "to_id": {"type": "string"},
                "confidence": {"type": "number"}
            }
        })),
        "warning" => Ok(json!({
            "type": "object",
            "required": ["id", "repo", "kind", "warning_type", "severity", "message"],
            "properties": {
                "kind": {"const": "warning"},
                "warning_type": {"type": "string"},
                "severity": {"enum": ["info", "warning", "error"]},
                "message": {"type": "string"}
            }
        })),
        other => Err(anyhow!("unsupported schema kind {other}")),
    }
}
