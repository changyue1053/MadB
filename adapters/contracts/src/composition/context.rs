//! Common pure context for explicitly admitted /2 build and package recipes.
use super::{
    BoundCompositionInputs, ResolvedResponsibility, check_composition_snapshot_binding,
    compare_responsibility_sets,
};
use crate::{Document, query::format_error};
use madb_app::{Digest, ErrorKind, Id, QueryError, Result};
use madb_model::build::BuildCompletionPolicy;
use madb_protocol::Value;
use std::collections::BTreeSet;

pub(super) struct CompositionExecutionContext {
    pub profile: Document,
    pub lock: Document,
    pub selector: Value,
    pub source_digest: Digest,
    pub profile_digest: Digest,
    pub policy: BuildCompletionPolicy,
    pub symbols: BTreeSet<String>,
}

pub(super) fn checked_context(
    inputs: &BoundCompositionInputs,
    project: &Document,
    contract: &Document,
    required_responsibilities: &[ResolvedResponsibility],
) -> Result<CompositionExecutionContext> {
    check_composition_snapshot_binding(inputs, contract).map_err(format_error)?;
    compare_responsibility_sets(required_responsibilities, inputs.responsibilities())
        .map_err(format_error)?;
    if crate::project_digest(project).map_err(format_error)?
        != inputs.locked_document.value()["project_digest"]
    {
        return Err(invalid(
            "LOCK002",
            "project",
            "project differs from fixed composition lock",
        ));
    }
    let capability = crate::convert::composition_capability(inputs).map_err(format_error)?;
    let model = crate::check_contract_with_capability(contract, &capability)
        .map_err(|e| invalid("InputInvalid", "contract", e.to_string()))?;
    let (defaults, origin) =
        crate::convert::composition_profile_defaults(inputs).map_err(format_error)?;
    crate::check_policy_resolution_with_defaults(contract, project, &defaults, &origin, &[])
        .map_err(|e| invalid("InputInvalid", "policy", e.to_string()))?;
    let required = inputs.profile().value()["required_features"]
        .as_array()
        .expect("checked profile features")
        .iter()
        .map(|v| Id::new(v.as_str().expect("checked feature")))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| invalid("InputInvalid", "required_features", e.to_string()))?;
    Ok(CompositionExecutionContext {
        profile: inputs.profile().clone(),
        lock: inputs.locked_document.clone(),
        selector: contract.value()["selector"].clone(),
        source_digest: model.source_digest,
        profile_digest: model.selector.profile_digest,
        policy: BuildCompletionPolicy::from_required_features(&required),
        symbols: model
            .symbols
            .iter()
            .map(|s| s.public_id.as_str().to_owned())
            .collect(),
    })
}

pub(super) fn invalid(code: &'static str, subject: &str, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, code, subject, message)
}
