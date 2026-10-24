use crate::wire::Wire;
use crate::{
    Document, DocumentKind, FormatError, candidate_digest, generation_id, source_digest,
    task_digest,
};
use madb_model::LogicalPath;
use madb_protocol::{Value, byte_digest};
use std::collections::BTreeSet;

pub struct ProvidedFile<'a> {
    pub path: &'a str,
    pub bytes: &'a [u8],
}

/// Check bytes loaded by the host against a reference from a validated payload.
/// Path confinement and symlink policy must be enforced before loading them.
pub fn verify_reference(reference: &Value, provided: &ProvidedFile<'_>) -> Result<(), FormatError> {
    let file = Wire::at(reference, "/file");
    let path = file.field("path")?.string()?;
    LogicalPath::new(path).map_err(|e| FormatError::new("FILE001", path, e.to_string()))?;
    if path != provided.path {
        return Err(FormatError::new(
            "FILE003",
            path,
            "host supplied a different logical file",
        ));
    }
    if file.field("size")?.uint()? != provided.bytes.len() as u64
        || file.field("sha256")?.digest()?.as_str() != byte_digest(provided.bytes)
    {
        return Err(FormatError::new(
            "FILE004",
            path,
            "file size or raw byte digest mismatch",
        ));
    }
    Ok(())
}

/// Checks a caller-supplied inventory. Host filesystem/link checks are separate;
/// this function never opens a path supplied by a helper.
pub fn verify_files(manifest: &Document, provided: &[ProvidedFile<'_>]) -> Result<(), FormatError> {
    if ![DocumentKind::OutputSet, DocumentKind::Candidate].contains(&manifest.kind()) {
        return Err(FormatError::new("FMT003", "/schema", "not a file manifest"));
    }
    let root = Wire::at(manifest.value(), "");
    let files = root.field("files")?.array()?;
    let mut expected = BTreeSet::new();
    let mut actual = BTreeSet::new();
    for item in provided {
        LogicalPath::new(item.path)
            .map_err(|e| FormatError::new("FILE001", item.path, e.to_string()))?;
        if !actual.insert(item.path) {
            return Err(FormatError::new(
                "FILE001",
                item.path,
                "duplicate supplied file",
            ));
        }
    }
    for file in files {
        let path = file.field("path")?.string()?;
        if !expected.insert(path) {
            return Err(FormatError::new("FILE001", path, "duplicate manifest path"));
        }
        if path == root.field("manifest_path")?.string()? {
            return Err(FormatError::new("FILE002", path, "manifest self-reference"));
        }
        let supplied = provided
            .iter()
            .find(|item| item.path == path)
            .ok_or_else(|| FormatError::new("FILE003", path, "declared file is missing"))?;
        verify_reference(file.value, supplied)?;
    }
    if actual != expected {
        return Err(FormatError::new(
            "FILE005",
            "/files",
            "undeclared output file",
        ));
    }
    let (field, computed) = if manifest.kind() == DocumentKind::OutputSet {
        ("generation_id", generation_id(manifest)?)
    } else {
        ("candidate_digest", candidate_digest(manifest)?)
    };
    if root.field(field)?.string()? != computed {
        return Err(FormatError::new(
            "FILE006",
            field,
            "content identity mismatch",
        ));
    }
    Ok(())
}

/// Match payloads, not just the control frame's `ok` status. Execution and
/// cancellation admission remain the app's protocol/state-machine responsibility.
pub fn validate_port_pair(request: &Document, response: &Document) -> Result<(), FormatError> {
    let expected = match request.kind() {
        DocumentKind::ReadRequest => DocumentKind::ContractIr,
        DocumentKind::GenerateRequest => DocumentKind::OutputSet,
        DocumentKind::BuildTask => DocumentKind::BuildEvidence,
        DocumentKind::PackageRequest => DocumentKind::Candidate,
        DocumentKind::VerifyRequest => DocumentKind::Report,
        _ => {
            return Err(FormatError::new(
                "PORT001",
                "/schema",
                "not an application request",
            ));
        }
    };
    if response.kind() != expected {
        return Err(FormatError::new(
            "PORT001",
            "/schema",
            "wrong response payload for operation",
        ));
    }
    let q = request.value();
    let a = response.value();
    let same = |left: &str, right: &str| -> Result<(), FormatError> {
        if q.pointer(left) != a.pointer(right) {
            Err(FormatError::new(
                "PORT002",
                right,
                format!("response differs from request {left}"),
            ))
        } else {
            Ok(())
        }
    };
    match request.kind() {
        DocumentKind::ReadRequest => {
            same("/boundary_id", "/boundary_id")?;
            same("/selector", "/selector")?;
            if a["source_digest"].as_str() != Some(source_digest(&q["inputs"])?.as_str()) {
                return Err(FormatError::new(
                    "PORT002",
                    "/source_digest",
                    "reader used different inputs",
                ));
            }
        }
        DocumentKind::GenerateRequest => {
            same("/input_digest", "/input_digest")?;
            same("/task_digest", "/task_digest")?;
            same("/selector/profile_digest", "/profile_digest")?;
            if a["generation_id"].as_str() != Some(generation_id(response)?.as_str()) {
                return Err(FormatError::new(
                    "PORT003",
                    "/generation_id",
                    "output identity mismatch",
                ));
            }
        }
        DocumentKind::BuildTask => {
            for (left, right) in [
                ("/id", "/task_id"),
                ("/task_digest", "/task_digest"),
                ("/input_digest", "/input_digest"),
                ("/generation_id", "/generation_id"),
                ("/selector/profile_digest", "/profile_digest"),
                ("/selector/target", "/target"),
                ("/phase", "/phase"),
                ("/tool", "/tool"),
            ] {
                same(left, right)?;
            }
            if q["task_digest"].as_str() != Some(task_digest(request)?.as_str()) {
                return Err(FormatError::new(
                    "PORT003",
                    "/task_digest",
                    "build task identity mismatch",
                ));
            }
            if a["status"] == "passed" {
                if a["exit_code"] != 0 {
                    return Err(FormatError::new(
                        "PORT004",
                        "/exit_code",
                        "failed process cannot be passed",
                    ));
                }
                let expected: BTreeSet<_> = q["outputs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                let outputs: Vec<_> = a["outputs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|f| f["path"].as_str())
                    .collect();
                let actual: BTreeSet<_> = outputs.iter().copied().collect();
                if actual != expected || actual.len() != outputs.len() {
                    return Err(FormatError::new(
                        "PORT004",
                        "/outputs",
                        "successful build lacks its declared outputs",
                    ));
                }
            }
        }
        DocumentKind::PackageRequest => {
            for field in [
                "selector",
                "generation_id",
                "contract_digest",
                "policy_digest",
            ] {
                same(&format!("/{field}"), &format!("/{field}"))?;
            }
            same("/lock/sha256", "/lock_digest")?;
            for field in [
                "public_symbols",
                "required_cases",
                "dependencies",
                "build_evidence",
            ] {
                if q[field] != a[field] {
                    return Err(FormatError::new(
                        "PORT002",
                        field,
                        "candidate omitted or altered a declared requirement",
                    ));
                }
            }
            if a["candidate_digest"].as_str() != Some(candidate_digest(response)?.as_str()) {
                return Err(FormatError::new(
                    "PORT003",
                    "/candidate_digest",
                    "candidate identity mismatch",
                ));
            }
        }
        DocumentKind::VerifyRequest => {
            same("/identity", "/identity")?;
            same("/selector", "/selector")?;
        }
        _ => unreachable!("request kind matched above"),
    }
    Ok(())
}
