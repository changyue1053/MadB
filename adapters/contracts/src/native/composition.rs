//! Explicit profile/2 metadata completion. No execution or commit authority.
use crate::{
    ContractError, DocumentKind, SchemaRegistry, check_contract_with_capability,
    check_policy_resolution_with_defaults,
    composition::{
        CompositionInputContext, RecipeRequirements, bind_composition_inputs,
        check_composition_snapshot_binding,
    },
    convert::{composition_capability, composition_profile_defaults},
};
use madb_protocol::canonical_json;

/// Complete reader declarations against actual frozen composition inputs.
///
/// `recipe` is a host-selected finite recipe, not a recipe reported by the
/// reader. Every call binds the project's semantic identity and the supplied
/// lock/profile/package/schema byte references before resolving policy. The
/// seed must already name that exact
/// selector and whole locked source inventory and contain no effective policy.
/// Source IDs and shape/structure/policy digests are recomputed, as in the
/// profile/1 reader path; forged placeholders do not survive into the output.
///
/// This proves input correspondence and the existing IR/policy rules only. It
/// does not prove that the reader observed declarations, derive the finite
/// recipe's complete required responsibility set, run a process, or grant
/// execution, Source commit, SDK acceptance or publication authority. The
/// composition feature remains on the profile and its interpreting components;
/// IR required features retain their existing core-only meaning.
pub fn finalize_composition_metadata(
    seed: &[u8],
    context: CompositionInputContext<'_>,
    recipe: &RecipeRequirements,
) -> Result<Vec<u8>, ContractError> {
    let project = context.project;
    let bound = bind_composition_inputs(context, recipe)?;
    let registry = SchemaRegistry::new()?;
    let seed = registry.decode(DocumentKind::ContractIr, seed)?;
    check_composition_snapshot_binding(&bound, &seed)?;
    let capabilities = composition_capability(&bound)?;
    let (defaults, origin) = composition_profile_defaults(&bound)?;
    let completed = super::complete_metadata(&registry, seed, project, || {
        Ok((defaults.clone(), origin.clone()))
    })?;
    check_composition_snapshot_binding(&bound, &completed)?;
    check_contract_with_capability(&completed, &capabilities)?;
    check_policy_resolution_with_defaults(&completed, project, &defaults, &origin, &[])?;
    Ok(canonical_json(completed.value())?)
}
