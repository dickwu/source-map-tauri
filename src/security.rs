use once_cell::sync::Lazy;
use regex::Regex;

use crate::model::ArtifactDoc;

pub struct RiskAssessment {
    pub level: String,
    pub reasons: Vec<String>,
    pub contains_phi: bool,
}

static BEARER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Bearer\s+[A-Za-z0-9._\-]+").expect("valid regex"));
static URL_AUTH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"https?://[^/\s:@]+:[^/\s:@]+@").expect("valid regex"));

pub fn redact_text(input: &str) -> String {
    let step = BEARER_RE.replace_all(input, "Bearer [REDACTED_SECRET]");
    URL_AUTH_RE
        .replace_all(&step, "https://[REDACTED_SECRET]@")
        .into_owned()
}

pub fn assess_risk(values: &[String]) -> RiskAssessment {
    let joined = values.join(" ").to_lowercase();
    let mut reasons = Vec::new();

    for keyword in [
        "patient",
        "phi",
        "mrn",
        "consent",
        "medication",
        "lab",
        "diagnosis",
        "billing",
        "insurance",
        "discharge",
        "audit",
        "upload",
        "export",
    ] {
        if joined.contains(keyword) {
            reasons.push(format!("{keyword} keyword"));
        }
    }

    let level = if joined.contains("database")
        || joined.contains("filesystem_export")
        || joined.contains("external_integration")
    {
        "critical"
    } else if !reasons.is_empty() {
        "high"
    } else {
        "low"
    };

    RiskAssessment {
        level: level.to_owned(),
        reasons,
        contains_phi: false,
    }
}

pub fn apply_artifact_security(doc: &mut ArtifactDoc) {
    doc.comments = doc.comments.iter().map(|item| redact_text(item)).collect();
    doc.tags = doc.tags.iter().map(|item| redact_text(item)).collect();
    let mut samples = Vec::new();
    if let Some(name) = &doc.name {
        samples.push(name.clone());
    }
    if let Some(path) = &doc.source_path {
        samples.push(path.clone());
    }
    samples.extend(doc.tags.clone());
    samples.extend(doc.comments.clone());
    let assessment = assess_risk(&samples);
    if doc.risk_level == "low" || doc.risk_level.is_empty() {
        doc.risk_level = assessment.level;
    }
    if doc.risk_reasons.is_empty() {
        doc.risk_reasons = assessment.reasons;
    }
    doc.contains_phi = assessment.contains_phi;
}
