//! Pure metadata completion for native declaration readers.
//!
//! The reader supplies a complete schema-valid structure with no effective
//! policies. Project/profile inputs are the only policy authority in this path.
use crate::{
    ContractError, Document, DocumentKind, FormatError, SchemaRegistry, check_contract,
    check_policy_resolution, contract, policy_digest, project_policies, shape_digest,
    structure_digest,
};
use madb_model::policy::{EffectivePolicy, Origin, PolicyValue, Slot};
use madb_protocol::{Value, canonical_json, strict_json};
use serde_json::{Map, json};

mod composition;
pub use composition::finalize_composition_metadata;
mod first_route_ownership;
pub(crate) use first_route_ownership::{FirstRouteOwnership, first_route_ownership};

/// Internal helper input limit, including the three-document envelope.
pub const MAX_ENVELOPE_BYTES: usize = madb_protocol::MAX_DOCUMENT_BYTES;

/// Resolve native metadata from independently schema-validated JSON documents.
///
/// `seed` must contain an empty `policies` array. Its source identities and shape,
/// structure and policy digests are recomputed; all other declaration data is
/// preserved. The source digest is the reader's locked ReadRequest value: byte
/// loading and verification remain the C04 host's responsibility. This function
/// performs no I/O, accepts no source-annotation channel, and returns no output
/// unless the completed IR passes the existing contract and policy checks.
/// Each input and the completed output obey the protocol's document bounds.
pub fn finalize_metadata(
    seed: &[u8],
    project: &[u8],
    profile: &[u8],
) -> Result<Vec<u8>, ContractError> {
    let registry = SchemaRegistry::new()?;
    finalize_documents(
        &registry,
        registry.decode(DocumentKind::ContractIr, seed)?,
        registry.decode(DocumentKind::Project, project)?,
        registry.decode(DocumentKind::Profile, profile)?,
    )
}

/// Decode the internal helper's exact `{contract, project, profile}` envelope.
/// The existing bounded parser rejects duplicate keys, unsafe numbers, trailing
/// data and excessive nesting before any metadata is completed.
pub fn finalize_envelope(bytes: &[u8]) -> Result<Vec<u8>, ContractError> {
    let value = strict_json(bytes)?;
    let fields = value
        .as_object()
        .ok_or_else(|| FormatError::new("NATIVE001", "", "metadata envelope must be an object"))?;
    if fields.len() != 3
        || !["contract", "project", "profile"]
            .iter()
            .all(|name| fields.contains_key(*name))
    {
        return Err(FormatError::new(
            "NATIVE001",
            "",
            "metadata envelope requires exactly contract, project and profile",
        )
        .into());
    }
    let registry = SchemaRegistry::new()?;
    finalize_documents(
        &registry,
        registry.validate(DocumentKind::ContractIr, fields["contract"].clone())?,
        registry.validate(DocumentKind::Project, fields["project"].clone())?,
        registry.validate(DocumentKind::Profile, fields["profile"].clone())?,
    )
}

fn finalize_documents(
    registry: &SchemaRegistry,
    seed: Document,
    project: Document,
    profile: Document,
) -> Result<Vec<u8>, ContractError> {
    let completed = complete_metadata(registry, seed, &project, || {
        crate::convert::profile_defaults(&profile)
    })?;
    check_contract(&completed, &profile)?;
    check_policy_resolution(&completed, &project, &profile, &[])?;
    Ok(canonical_json(completed.value())?)
}

// Keep the declaration/policy computation common while each entry point owns
// its profile version and input admission. The closure preserves the legacy
// parsing order: declaration identities, project policies, then defaults.
fn complete_metadata(
    registry: &SchemaRegistry,
    seed: Document,
    project: &Document,
    defaults: impl FnOnce() -> Result<(Vec<PolicyValue>, Origin), FormatError>,
) -> Result<Document, ContractError> {
    let mut model = contract(&seed)?;
    if !model.policies.is_empty() {
        return Err(FormatError::new(
            "NATIVE001",
            "/policies",
            "native seed must not supply resolved policies or provenance",
        )
        .into());
    }
    let mut value = seed.into_value();
    for (index, symbol) in model.symbols.iter_mut().enumerate() {
        symbol.source_id = symbol.source_key.source_id().map_err(|error| {
            FormatError::new(
                "DTO002",
                format!("/symbols/{index}/source_key"),
                error.to_string(),
            )
        })?;
        value["symbols"][index]["source_id"] = json!(symbol.source_id.as_str());
    }
    let inputs = project_policies(project)?;
    let (defaults, origin) = defaults()?;
    let policies = madb_rules::resolve_policies(
        &inputs.namespace,
        &model.symbols,
        &model.types,
        &inputs.bindings,
        &inputs.families,
        &defaults,
        &origin,
    )
    .map_err(ContractError::Findings)?;
    value["policies"] = Value::Array(policies.iter().map(policy_wire).collect());
    let resolved = registry.validate(DocumentKind::ContractIr, value)?;
    let mut value = resolved.value().clone();
    for (index, symbol) in model.symbols.iter().enumerate() {
        value["symbols"][index]["shape_digest"] =
            json!(shape_digest(&resolved, symbol.public_id.as_str())?);
    }
    value["structure_digest"] = json!(structure_digest(&resolved)?);
    value["policy_digest"] = json!(policy_digest(&resolved)?);
    Ok(registry.validate(DocumentKind::ContractIr, value)?)
}

fn policy_wire(policy: &EffectivePolicy) -> Value {
    let mut values = Map::new();
    let mut provenance = Vec::new();
    for (property, sourced) in &policy.values {
        values.insert((*property).into(), value_wire(&sourced.value));
        provenance.push(json!({
            "property": property,
            "origin": origin_wire(&sourced.origin),
            "replaced_default": sourced.replaced_default.as_ref().map(origin_wire),
        }));
    }
    let slot = match &policy.slot {
        Slot::Call => json!({"kind": "call"}),
        Slot::Return => json!({"kind": "return"}),
        Slot::Parameter(name) => json!({"kind": "parameter", "name": name.as_str()}),
    };
    json!({"symbol_id": policy.symbol_id.as_str(), "slot": slot,
        "values": values, "provenance": provenance})
}

fn origin_wire(origin: &Origin) -> Value {
    json!({"authority": origin.authority.as_str(), "id": origin.id.as_str(),
        "location": {"path": origin.location.path.as_str(),
            "line": origin.location.line, "column": origin.location.column}})
}

fn value_wire(value: &PolicyValue) -> Value {
    match value {
        PolicyValue::Ownership(v) => json!(v.as_str()),
        PolicyValue::Releaser(v) => json!(v.as_str()),
        PolicyValue::BorrowScope(v) => json!(v.as_str()),
        PolicyValue::KeepAlive(v) => json!(v.as_str()),
        PolicyValue::Threading(v) => json!(v.as_str()),
        PolicyValue::Blocking(v) => json!(v.as_str()),
        PolicyValue::Delivery(v) => json!(v.as_str()),
        PolicyValue::Cancellation(v) => json!(v.as_str()),
        PolicyValue::CancellationConfirmation(v) => json!(v.as_str()),
        PolicyValue::Close(v) => json!(v.as_str()),
        PolicyValue::SynchronousCallback(v) | PolicyValue::Reentrant(v) => json!(v),
        PolicyValue::ContextOwner(v) => json!(v.as_str()),
        PolicyValue::Stop(v) => json!(v.as_str()),
        PolicyValue::StreamKind(v) => json!(v.as_str()),
        PolicyValue::CapacityItems(v) | PolicyValue::CapacityBytes(v) => json!(v),
        PolicyValue::Overflow(v) => json!(v.as_str()),
        PolicyValue::Ordering(v) => json!(v.as_str()),
        PolicyValue::Replay(v) => json!(v.as_str()),
        PolicyValue::Recovery(v) => json!(v.as_str()),
        PolicyValue::ReleaseThread(v) => json!(v.as_str()),
    }
}
