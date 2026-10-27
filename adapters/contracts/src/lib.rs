//! Pure schema adapter. It converts wire data and calls inward-facing rules;
//! it does not read source files, run a backend or commit generated artifacts.
pub mod composition;
mod convert;
pub mod dependencies;
pub mod execution;
pub mod explain;
pub mod extension;
pub mod extension_files;
pub mod extension_lock;
pub mod extension_session;
pub mod native;
pub mod native_values;
pub mod project_init;
mod projections;
mod resolution;
mod wire;
pub use resolution::check_frozen_inputs;
pub mod artifacts;
pub mod query;
pub mod task;

pub use convert::{
    PolicyInputs, build_completion_policy, capability, contract, evidence_identity,
    project_policies, report_results, required_case, requirements,
};
use madb_model::{Finding, policy::Binding};
pub use madb_protocol::{Document, DocumentKind, FormatError, SchemaRegistry};
pub use projections::{
    candidate_digest, case_digest, generation_id, policy_digest, profile_digest, project_digest,
    shape_digest, source_digest, structure_digest, task_digest,
};

#[derive(Debug)]
pub enum ContractError {
    Format(FormatError),
    Findings(Vec<Finding>),
}
impl From<FormatError> for ContractError {
    fn from(error: FormatError) -> Self {
        Self::Format(error)
    }
}
impl std::fmt::Display for ContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Format(e) => e.fmt(f),
            Self::Findings(items) => write!(f, "{} semantic findings", items.len()),
        }
    }
}
impl std::error::Error for ContractError {}

/// Validate an effective IR against the exact profile and its declared hashes.
pub fn check_contract(
    ir: &Document,
    profile: &Document,
) -> Result<madb_model::contract::Contract, ContractError> {
    let model = contract(ir)?;
    let capabilities = capability(profile)?;
    check_contract_model(ir, model, &capabilities)
}

pub(crate) fn check_contract_with_capability(
    ir: &Document,
    capabilities: &madb_model::contract::Capability,
) -> Result<madb_model::contract::Contract, ContractError> {
    check_contract_model(ir, contract(ir)?, capabilities)
}

fn check_contract_model(
    ir: &Document,
    model: madb_model::contract::Contract,
    capabilities: &madb_model::contract::Capability,
) -> Result<madb_model::contract::Contract, ContractError> {
    let mut findings = madb_rules::validate_contract(&model, capabilities);
    if !findings.is_empty() {
        return Err(ContractError::Findings(findings));
    }
    if model.structure_digest.as_str() != structure_digest(ir)? {
        findings.push(Finding::new(
            "HASH001",
            model.boundary_id.as_str(),
            "structure digest mismatch",
        ));
    }
    if model.policy_digest.as_str() != policy_digest(ir)? {
        findings.push(Finding::new(
            "HASH002",
            model.boundary_id.as_str(),
            "policy digest mismatch",
        ));
    }
    for symbol in &model.symbols {
        if symbol.shape_digest.as_str() != shape_digest(ir, symbol.public_id.as_str())? {
            findings.push(
                Finding::new(
                    "HASH003",
                    symbol.public_id.as_str(),
                    "symbol shape digest mismatch",
                )
                .at(&symbol.location),
            );
        }
    }
    if findings.is_empty() {
        Ok(model)
    } else {
        Err(ContractError::Findings(findings))
    }
}

/// Source annotations arrive through the reader channel. A project file cannot
/// label its own values as compiler annotations. Both channels obey one resolver.
pub fn check_policy_resolution(
    ir: &Document,
    project: &Document,
    profile: &Document,
    annotations: &[Binding],
) -> Result<(), ContractError> {
    let (model, inputs) = policy_resolution_inputs(ir, project, annotations)?;
    let (defaults, origin) = convert::profile_defaults(profile)?;
    check_resolved_policies(model, inputs, &defaults, &origin)
}

pub(crate) fn check_policy_resolution_with_defaults(
    ir: &Document,
    project: &Document,
    defaults: &[madb_model::policy::PolicyValue],
    origin: &madb_model::policy::Origin,
    annotations: &[Binding],
) -> Result<(), ContractError> {
    let (model, inputs) = policy_resolution_inputs(ir, project, annotations)?;
    check_resolved_policies(model, inputs, defaults, origin)
}

fn policy_resolution_inputs(
    ir: &Document,
    project: &Document,
    annotations: &[Binding],
) -> Result<(madb_model::contract::Contract, PolicyInputs), ContractError> {
    let model = contract(ir)?;
    let mut inputs = project_policies(project)?;
    if annotations
        .iter()
        .any(|b| b.origin.authority != madb_model::policy::Authority::SourceAnnotation)
    {
        return Err(ContractError::Findings(vec![Finding::new(
            "POL005",
            model.boundary_id.as_str(),
            "reader channel must identify source annotations",
        )]));
    }
    inputs.bindings.extend_from_slice(annotations);
    Ok((model, inputs))
}

fn check_resolved_policies(
    model: madb_model::contract::Contract,
    inputs: PolicyInputs,
    defaults: &[madb_model::policy::PolicyValue],
    origin: &madb_model::policy::Origin,
) -> Result<(), ContractError> {
    let resolved = madb_rules::resolve_policies(
        &inputs.namespace,
        &model.symbols,
        &model.types,
        &inputs.bindings,
        &inputs.families,
        defaults,
        origin,
    )
    .map_err(ContractError::Findings)?;
    if inputs.namespace != model.namespace {
        return Err(ContractError::Findings(vec![Finding::new(
            "ID007",
            model.boundary_id.as_str(),
            "project and IR SDK namespace differ",
        )]));
    }
    let mut recorded = model.policies;
    recorded.sort_by(|a, b| (&a.symbol_id, &a.slot).cmp(&(&b.symbol_id, &b.slot)));
    if recorded != resolved {
        return Err(ContractError::Findings(vec![Finding::new(
            "POL011",
            model.boundary_id.as_str(),
            "effective policies or provenance do not match authoritative inputs",
        )]));
    }
    Ok(())
}

pub mod doctor;
