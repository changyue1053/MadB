use madb_model::Finding;
use madb_model::contract::{Capability, Contract, Precision, TypeDefinition};
use std::collections::BTreeSet;

/// Check the consistency of a compiler witness. Producing truthful observations
/// is still the locked reader/probe's responsibility; this does not run a compiler.
pub(crate) fn validate_layouts(contract: &Contract, capability: &Capability) -> Vec<Finding> {
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    for o in &contract.observations {
        let subject = o.subject.as_str();
        if !seen.insert(&o.subject) {
            errors.push(Finding::new(
                "ABI001",
                subject,
                "duplicate layout observation",
            ));
        }
        if o.selector != contract.selector || o.input != contract.source_digest {
            errors.push(Finding::new(
                "ABI002",
                subject,
                "layout belongs to a different target or input",
            ));
        }
        if o.precision != Precision::CompilerDerived || !capability.producers.contains(&o.producer)
        {
            errors.push(Finding::new(
                "ABI003",
                subject,
                "layout producer/precision does not match the profile",
            ));
        }
        let definition = contract
            .types
            .iter()
            .find(|t| t.id == o.subject)
            .map(|t| &t.definition);
        let Some(TypeDefinition::Record {
            fields,
            c_layout: true,
        }) = definition
        else {
            errors.push(Finding::new(
                "ABI001",
                subject,
                "v1 layout witness must describe a declared C record",
            ));
            continue;
        };
        if !o.alignment.is_power_of_two() || o.size == 0 || o.size % o.alignment != 0 {
            errors.push(Finding::new(
                "ABI004",
                subject,
                "invalid record size/alignment",
            ));
        }
        if o.fields.len() != fields.len() {
            errors.push(Finding::new("ABI004", subject, "incomplete field layout"));
        }
        let mut end = 0;
        for (field, declared) in o.fields.iter().zip(fields) {
            if field.name != declared.name
                || field.size == 0
                || field.offset < end
                || field
                    .offset
                    .checked_add(field.size)
                    .is_none_or(|n| n > o.size)
            {
                errors.push(Finding::new(
                    "ABI004",
                    subject,
                    "field order, bounds or overlap invalid",
                ));
            }
            end = field.offset.saturating_add(field.size);
        }
    }
    for item in &contract.types {
        if matches!(
            item.definition,
            TypeDefinition::Record { c_layout: true, .. }
        ) && !seen.contains(&item.id)
        {
            errors.push(Finding::new(
                "ABI005",
                item.id.as_str(),
                "C record needs a target compiler layout witness",
            ));
        }
    }
    errors
}
