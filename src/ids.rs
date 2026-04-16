use sha2::{Digest, Sha256};

pub fn sanitize_fragment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_owned()
}

pub fn short_sha256(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())[..12].to_owned()
}

pub fn document_id(
    repo: &str,
    kind: &str,
    source_path: Option<&str>,
    line: Option<u32>,
    name: Option<&str>,
) -> String {
    let raw = format!(
        "{repo}|{kind}|{}|{}|{}",
        source_path.unwrap_or(""),
        line.map(|item| item.to_string()).unwrap_or_default(),
        name.unwrap_or("")
    );
    format!(
        "{}__{}__{}",
        sanitize_fragment(repo),
        sanitize_fragment(kind),
        short_sha256(&raw)
    )
}

pub fn is_safe_document_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}
