use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde_json::{json, Value};

use crate::{
    ids::document_id,
    model::{ArtifactDoc, EdgeDoc, WarningDoc},
    security::apply_artifact_security,
};

#[derive(Clone)]
struct FlowCandidate {
    component_name: Option<String>,
    component_path: Option<String>,
    line_start: Option<u32>,
    wrapper_index: usize,
    transport_index: Option<usize>,
    source_paths: BTreeSet<String>,
    related_tests: BTreeSet<String>,
}

#[derive(Clone)]
struct EndpointRecord {
    method: String,
    normalized_path: String,
    wrapper_indices: Vec<usize>,
    endpoint_index: usize,
}

pub fn augment_frontend_http_flows(
    artifacts: &mut Vec<ArtifactDoc>,
    edges: &mut Vec<EdgeDoc>,
    _warnings: &mut Vec<WarningDoc>,
) -> Result<()> {
    let mut transport_by_name: BTreeMap<String, usize> = BTreeMap::new();
    let mut wrappers_by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut endpoint_records: BTreeMap<String, EndpointRecord> = BTreeMap::new();

    for (index, artifact) in artifacts.iter().enumerate() {
        if artifact.kind == "frontend_transport" {
            if let Some(name) = &artifact.name {
                transport_by_name.insert(name.clone(), index);
            }
        }
        if artifact.kind == "frontend_api_wrapper" {
            if let Some(name) = &artifact.name {
                wrappers_by_name
                    .entry(name.clone())
                    .or_default()
                    .push(index);
            }
        }
    }

    let wrapper_indices: Vec<usize> = artifacts
        .iter()
        .enumerate()
        .filter_map(|(index, artifact)| (artifact.kind == "frontend_api_wrapper").then_some(index))
        .collect();

    for wrapper_index in wrapper_indices {
        let Some(method) = artifacts[wrapper_index]
            .data
            .get("http_method")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            continue;
        };
        let Some(normalized_path) = artifacts[wrapper_index]
            .data
            .get("normalized_path")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            continue;
        };
        let key = endpoint_key(&method, &normalized_path);
        if let Some(record) = endpoint_records.get_mut(&key) {
            record.wrapper_indices.push(wrapper_index);
            continue;
        }

        let endpoint = endpoint_artifact(
            &artifacts[wrapper_index].repo,
            &method,
            &normalized_path,
            artifacts[wrapper_index].source_path.as_deref(),
            artifacts[wrapper_index].line_start,
        );
        let endpoint_index = artifacts.len();
        artifacts.push(endpoint);
        endpoint_records.insert(
            key,
            EndpointRecord {
                method,
                normalized_path,
                wrapper_indices: vec![wrapper_index],
                endpoint_index,
            },
        );
    }

    let hook_use_indices: Vec<usize> = artifacts
        .iter()
        .enumerate()
        .filter_map(|(index, artifact)| (artifact.kind == "frontend_hook_use").then_some(index))
        .collect();

    for hook_use_index in hook_use_indices {
        let Some(wrapper_name) = artifacts[hook_use_index]
            .data
            .get("hook_def_name")
            .and_then(Value::as_str)
        else {
            continue;
        };
        let Some(wrapper_indices) = wrappers_by_name.get(wrapper_name).cloned() else {
            continue;
        };
        for wrapper_index in wrapper_indices {
            edges.push(edge(
                &artifacts[hook_use_index].repo,
                "calls_api_wrapper",
                &artifacts[hook_use_index],
                &artifacts[wrapper_index],
                "hook callsite name matches API wrapper definition",
                0.97,
            ));
        }
    }

    let endpoint_records_list: Vec<EndpointRecord> = endpoint_records.values().cloned().collect();
    for record in endpoint_records_list {
        for wrapper_index in &record.wrapper_indices {
            if let Some(transport_name) = artifacts[*wrapper_index]
                .data
                .get("transport_name")
                .and_then(Value::as_str)
            {
                if let Some(transport_index) = transport_by_name.get(transport_name).copied() {
                    edges.push(edge(
                        &artifacts[*wrapper_index].repo,
                        "uses_transport",
                        &artifacts[*wrapper_index],
                        &artifacts[transport_index],
                        "wrapper transport name matches transport definition",
                        0.98,
                    ));
                    edges.push(edge(
                        &artifacts[transport_index].repo,
                        "calls_http_route",
                        &artifacts[transport_index],
                        &artifacts[record.endpoint_index],
                        "transport method and wrapper path resolve to endpoint",
                        0.98,
                    ));
                } else {
                    edges.push(edge(
                        &artifacts[*wrapper_index].repo,
                        "calls_http_route",
                        &artifacts[*wrapper_index],
                        &artifacts[record.endpoint_index],
                        "wrapper directly resolves endpoint",
                        0.92,
                    ));
                }
            } else {
                edges.push(edge(
                    &artifacts[*wrapper_index].repo,
                    "calls_http_route",
                    &artifacts[*wrapper_index],
                    &artifacts[record.endpoint_index],
                    "wrapper directly resolves endpoint",
                    0.9,
                ));
            }
        }

        let candidates =
            flow_candidates_for_endpoint(artifacts, &transport_by_name, &record.wrapper_indices);
        if candidates.is_empty() {
            continue;
        }
        let Some(best) = canonical_candidate(&candidates, artifacts) else {
            continue;
        };
        let flow = flow_artifact(artifacts, &record, &candidates, &best);
        let flow_index = artifacts.len();
        artifacts.push(flow);
        edges.push(edge(
            &artifacts[flow_index].repo,
            "contains",
            &artifacts[flow_index],
            &artifacts[record.endpoint_index],
            "flow summarizes canonical frontend HTTP chain",
            1.0,
        ));
    }

    Ok(())
}

fn flow_candidates_for_endpoint(
    artifacts: &[ArtifactDoc],
    transport_by_name: &BTreeMap<String, usize>,
    wrapper_indices: &[usize],
) -> Vec<FlowCandidate> {
    let mut candidates = Vec::new();
    for wrapper_index in wrapper_indices {
        let wrapper = &artifacts[*wrapper_index];
        let transport_index = wrapper
            .data
            .get("transport_name")
            .and_then(Value::as_str)
            .and_then(|name| transport_by_name.get(name).copied());
        let callers: Vec<&ArtifactDoc> = artifacts
            .iter()
            .filter(|artifact| {
                artifact.kind == "frontend_hook_use"
                    && artifact.data.get("hook_def_name").and_then(Value::as_str)
                        == wrapper.name.as_deref()
            })
            .collect();

        if callers.is_empty() {
            let mut source_paths = BTreeSet::new();
            if let Some(path) = wrapper.source_path.as_ref() {
                source_paths.insert(path.clone());
            }
            let related_tests = wrapper.related_tests.iter().cloned().collect();
            candidates.push(FlowCandidate {
                component_name: None,
                component_path: None,
                line_start: wrapper.line_start,
                wrapper_index: *wrapper_index,
                transport_index,
                source_paths,
                related_tests,
            });
            continue;
        }

        for caller in callers {
            let mut source_paths = BTreeSet::new();
            let mut related_tests = BTreeSet::new();
            if let Some(path) = caller.source_path.as_ref() {
                source_paths.insert(path.clone());
            }
            if let Some(path) = wrapper.source_path.as_ref() {
                source_paths.insert(path.clone());
            }
            if let Some(index) = transport_index {
                if let Some(path) = artifacts[index].source_path.as_ref() {
                    source_paths.insert(path.clone());
                }
                for test in &artifacts[index].related_tests {
                    related_tests.insert(test.clone());
                }
            }
            for test in caller
                .related_tests
                .iter()
                .chain(wrapper.related_tests.iter())
            {
                related_tests.insert(test.clone());
            }
            candidates.push(FlowCandidate {
                component_name: caller
                    .data
                    .get("component")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                component_path: caller.source_path.clone(),
                line_start: caller.line_start.or(wrapper.line_start),
                wrapper_index: *wrapper_index,
                transport_index,
                source_paths,
                related_tests,
            });
        }
    }

    candidates
}

fn canonical_candidate(
    candidates: &[FlowCandidate],
    artifacts: &[ArtifactDoc],
) -> Option<FlowCandidate> {
    candidates
        .iter()
        .max_by_key(|candidate| {
            let mut score = 0_i32;
            if candidate.component_name.is_some() {
                score += 100;
            }
            if candidate
                .component_path
                .as_deref()
                .is_some_and(|path| path.contains("/app/"))
            {
                score += 20;
            }
            if candidate
                .component_name
                .as_deref()
                .is_some_and(|name| name.ends_with("Modal"))
            {
                score += 12;
            }
            if candidate
                .component_name
                .as_deref()
                .is_some_and(|name| name.ends_with("Page"))
            {
                score += 10;
            }
            if !candidate.related_tests.is_empty() {
                score += 5;
            }
            score -= candidate.source_paths.len() as i32;
            (
                score,
                candidate.component_path.clone().unwrap_or_default(),
                artifacts[candidate.wrapper_index]
                    .source_path
                    .clone()
                    .unwrap_or_default(),
            )
        })
        .cloned()
}

fn flow_artifact(
    artifacts: &[ArtifactDoc],
    endpoint: &EndpointRecord,
    candidates: &[FlowCandidate],
    best: &FlowCandidate,
) -> ArtifactDoc {
    let wrapper = &artifacts[best.wrapper_index];
    let transport = best.transport_index.map(|index| &artifacts[index]);
    let source_path = best
        .component_path
        .clone()
        .or_else(|| wrapper.source_path.clone());
    let line_start = best.line_start.or(wrapper.line_start).unwrap_or(1);

    let mut alternate_components = BTreeSet::new();
    let mut related_tests = BTreeSet::new();
    let mut source_paths = BTreeSet::new();
    for candidate in candidates {
        if let Some(component) = candidate.component_name.as_ref() {
            if Some(component) != best.component_name.as_ref() {
                alternate_components.insert(component.clone());
            }
        }
        for test in &candidate.related_tests {
            related_tests.insert(test.clone());
        }
        for path in &candidate.source_paths {
            source_paths.insert(path.clone());
        }
    }

    let mut doc = ArtifactDoc {
        id: document_id(
            &wrapper.repo,
            "frontend_http_flow",
            source_path.as_deref(),
            Some(line_start),
            Some(&endpoint.display_name()),
        ),
        repo: wrapper.repo.clone(),
        kind: "frontend_http_flow".to_owned(),
        side: Some("frontend".to_owned()),
        language: wrapper.language.clone(),
        name: Some(endpoint.normalized_path.clone()),
        display_name: Some(endpoint.display_name()),
        source_path,
        line_start: Some(line_start),
        line_end: Some(line_start),
        column_start: None,
        column_end: None,
        package_name: None,
        comments: Vec::new(),
        tags: vec!["http flow".to_owned()],
        related_symbols: [
            best.component_name.clone(),
            wrapper.name.clone(),
            transport.and_then(|artifact| artifact.name.clone()),
        ]
        .into_iter()
        .flatten()
        .collect(),
        related_tests: related_tests.into_iter().collect(),
        risk_level: "low".to_owned(),
        risk_reasons: Vec::new(),
        contains_phi: false,
        has_related_tests: false,
        updated_at: chrono::Utc::now().to_rfc3339(),
        data: BTreeMap::new().into_iter().collect(),
    };

    doc.data.insert(
        "http_method".to_owned(),
        Value::String(endpoint.method.clone()),
    );
    doc.data.insert(
        "normalized_path".to_owned(),
        Value::String(endpoint.normalized_path.clone()),
    );
    doc.data.insert(
        "primary_component".to_owned(),
        best.component_name
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    doc.data.insert(
        "primary_component_path".to_owned(),
        best.component_path
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    doc.data.insert(
        "primary_wrapper".to_owned(),
        wrapper
            .name
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    doc.data.insert(
        "primary_wrapper_path".to_owned(),
        wrapper
            .source_path
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    doc.data.insert(
        "primary_transport".to_owned(),
        transport
            .and_then(|artifact| artifact.name.clone())
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    doc.data.insert(
        "primary_transport_path".to_owned(),
        transport
            .and_then(|artifact| artifact.source_path.clone())
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    doc.data
        .insert("caller_count".to_owned(), json!(candidates.len()));
    doc.data.insert(
        "alternate_components".to_owned(),
        Value::Array(
            alternate_components
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    doc.data.insert(
        "source_paths".to_owned(),
        Value::Array(source_paths.into_iter().map(Value::String).collect()),
    );
    doc.data.insert(
        "primary_flow".to_owned(),
        json!([
            {
                "kind": "frontend_component",
                "name": best.component_name,
                "path": best.component_path,
                "line": best.line_start,
            },
            {
                "kind": "frontend_api_wrapper",
                "name": wrapper.name,
                "path": wrapper.source_path,
                "line": wrapper.line_start,
            },
            {
                "kind": "frontend_transport",
                "name": transport.and_then(|artifact| artifact.name.clone()),
                "path": transport.and_then(|artifact| artifact.source_path.clone()),
                "line": transport.and_then(|artifact| artifact.line_start),
            },
            {
                "kind": "frontend_http_endpoint",
                "method": endpoint.method,
                "path": endpoint.normalized_path,
            }
        ]),
    );

    apply_artifact_security(&mut doc);
    doc
}

fn endpoint_artifact(
    repo: &str,
    method: &str,
    normalized_path: &str,
    source_path: Option<&str>,
    line_start: Option<u32>,
) -> ArtifactDoc {
    let mut doc = ArtifactDoc {
        id: document_id(
            repo,
            "frontend_http_endpoint",
            source_path,
            line_start,
            Some(&format!("{method} {normalized_path}")),
        ),
        repo: repo.to_owned(),
        kind: "frontend_http_endpoint".to_owned(),
        side: Some("frontend".to_owned()),
        language: Some("ts".to_owned()),
        name: Some(normalized_path.to_owned()),
        display_name: Some(format!("{method} {normalized_path}")),
        source_path: source_path.map(str::to_owned),
        line_start,
        line_end: line_start,
        column_start: None,
        column_end: None,
        package_name: None,
        comments: Vec::new(),
        tags: vec!["http endpoint".to_owned()],
        related_symbols: Vec::new(),
        related_tests: Vec::new(),
        risk_level: "low".to_owned(),
        risk_reasons: Vec::new(),
        contains_phi: false,
        has_related_tests: false,
        updated_at: chrono::Utc::now().to_rfc3339(),
        data: BTreeMap::new().into_iter().collect(),
    };
    doc.data
        .insert("http_method".to_owned(), Value::String(method.to_owned()));
    doc.data.insert(
        "normalized_path".to_owned(),
        Value::String(normalized_path.to_owned()),
    );
    doc.data.insert(
        "endpoint_key".to_owned(),
        Value::String(format!("{method} {normalized_path}")),
    );
    apply_artifact_security(&mut doc);
    doc
}

fn endpoint_key(method: &str, normalized_path: &str) -> String {
    format!("{method} {normalized_path}")
}

impl EndpointRecord {
    fn display_name(&self) -> String {
        endpoint_key(&self.method, &self.normalized_path)
    }
}

fn edge(
    repo: &str,
    edge_type: &str,
    from: &ArtifactDoc,
    to: &ArtifactDoc,
    reason: &str,
    confidence: f32,
) -> EdgeDoc {
    EdgeDoc {
        id: document_id(
            repo,
            "edge",
            from.source_path.as_deref(),
            from.line_start,
            Some(&format!("{edge_type}:{}:{}", from.id, to.id)),
        ),
        repo: repo.to_owned(),
        kind: "edge".to_owned(),
        edge_type: edge_type.to_owned(),
        from_id: from.id.clone(),
        from_kind: from.kind.clone(),
        from_name: from.name.clone(),
        to_id: to.id.clone(),
        to_kind: to.kind.clone(),
        to_name: to.name.clone(),
        confidence,
        reason: reason.to_owned(),
        source_path: from.source_path.clone(),
        line_start: from.line_start,
        risk_level: from.risk_level.clone(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    }
}
