//! Explicit profile/2 build binding. Common task and artifact behavior remains
//! in the existing C04 adapter; composition only supplies a checked context.
use super::{BoundCompositionInputs, ResolvedResponsibility, context::checked_context};
use crate::{
    Document, DocumentKind, SchemaRegistry,
    execution::{BuildArtifacts, assemble_build_files_checked},
    query::format_error,
    task::{BoundBuildTask, CheckedBuildContext, TaskBinding},
};
use madb_app::{Id, Result, dag::ExecutionGraph, task::ValidatedTaskResult};
use std::collections::BTreeMap;

/// Bind every submitted node before selecting a target. Required responsibilities
/// come from the finite host recipe's declarations and wiring, not the submitted
/// responsibility rows. This creates recipes, never a process or commit permit.
pub fn bind_build_graph(
    inputs: &BoundCompositionInputs,
    project: &Document,
    contract: &Document,
    required_responsibilities: &[ResolvedResponsibility],
    tasks: Vec<TaskBinding<'_>>,
    target: Option<&Id>,
    active: &[Id],
) -> Result<ExecutionGraph<BoundBuildTask>> {
    let context = checked_context(inputs, project, contract, required_responsibilities)?;
    CheckedBuildContext::from_checked(
        context.profile,
        context.lock,
        context.selector,
        context.source_digest,
        context.policy,
    )?
    .graph(tasks, target, active)
}

/// Recheck composition identity and selected task bindings before preserving
/// real captured results. The same completion policy and output assembly as
/// profile/1 apply; a partial compile/link set cannot become an ordinary build.
pub fn assemble_build_files(
    inputs: &BoundCompositionInputs,
    project: &Document,
    contract: &Document,
    required_responsibilities: &[ResolvedResponsibility],
    tasks: &[Vec<u8>],
    results: &BTreeMap<Id, ValidatedTaskResult>,
) -> Result<BuildArtifacts> {
    let context = checked_context(inputs, project, contract, required_responsibilities)?;
    let profile_digest = context.profile_digest;
    let policy = context.policy;
    let checked = CheckedBuildContext::from_checked(
        context.profile,
        context.lock,
        context.selector,
        context.source_digest,
        policy,
    )?;
    let registry = SchemaRegistry::new().map_err(format_error)?;
    for bytes in tasks {
        let document = registry
            .decode(DocumentKind::BuildTask, bytes)
            .map_err(format_error)?;
        checked.validate_document(&document)?;
    }
    assemble_build_files_checked(tasks, results, profile_digest.as_str(), policy)
}
