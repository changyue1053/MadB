use crate::validate_identities;
use madb_model::contract::{ContractType, Symbol, TypeDefinition};
use madb_model::policy::*;
use madb_model::{Finding, Id, PublicId};
use std::collections::{BTreeMap, BTreeSet};

/// Select exactly, reject conflicting authorities, then apply allowed defaults.
/// Nothing is emitted on failure. Order of input declarations is not precedence.
pub fn resolve_policies(
    namespace: &Id,
    symbols: &[Symbol],
    types: &[ContractType],
    bindings: &[Binding],
    families: &[Family],
    profile_defaults: &[PolicyValue],
    default_origin: &Origin,
) -> Result<Vec<EffectivePolicy>, Vec<Finding>> {
    let mut errors = validate_identities(namespace, symbols, &[]);
    if default_origin.authority != Authority::ProfileDefault
        || profile_defaults
            .iter()
            .any(|v| *v != PolicyValue::Ownership(Ownership::Copy))
        || profile_defaults.len() > 1
    {
        errors.push(
            Finding::new(
                "POL006",
                default_origin.id.as_str(),
                "profile defaults may only copy value inputs",
            )
            .at(&default_origin.location),
        );
    }
    let mut family_index = BTreeMap::new();
    for family in families {
        if family.origin.authority != Authority::PolicyFamily
            || family_index.insert(&family.id, family).is_some()
        {
            errors.push(
                Finding::new(
                    "POL005",
                    family.id.as_str(),
                    "duplicate family or invalid authority",
                )
                .at(&family.origin.location),
            );
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    type Group<'a> = BTreeMap<(PublicId, Slot), Vec<&'a Binding>>;
    let mut groups: Group<'_> = BTreeMap::new();
    for binding in bindings {
        let matches: Vec<_> = symbols
            .iter()
            .filter(|symbol| match &binding.selector {
                ExactSelector::Public(id) => &symbol.public_id == id,
                ExactSelector::Source(id) => &symbol.source_id == id,
            })
            .collect();
        if matches.len() != 1 {
            errors.push(
                Finding::new(
                    "POL001",
                    binding.origin.id.as_str(),
                    "selector must match exactly one complete identity",
                )
                .at(&binding.origin.location),
            );
            continue;
        }
        let symbol = matches[0];
        if binding.slot != Slot::Call && symbol.slot_type(&binding.slot).is_none() {
            errors.push(
                Finding::new(
                    "POL002",
                    symbol.public_id.as_str(),
                    "selected parameter does not exist",
                )
                .at(&binding.origin.location),
            );
            continue;
        }
        if !matches!(
            binding.origin.authority,
            Authority::ProjectPolicy | Authority::SourceAnnotation
        ) {
            errors.push(
                Finding::new(
                    "POL005",
                    symbol.public_id.as_str(),
                    "explicit binding must identify its authoritative source",
                )
                .at(&binding.origin.location),
            );
        }
        groups
            .entry((symbol.public_id.clone(), binding.slot.clone()))
            .or_default()
            .push(binding);
    }
    // A permitted default also applies to value inputs with no explicit binding.
    for symbol in symbols {
        for parameter in &symbol.parameters {
            if is_value(&parameter.type_id, types, &mut BTreeSet::new())
                && !profile_defaults.is_empty()
            {
                groups
                    .entry((
                        symbol.public_id.clone(),
                        Slot::Parameter(parameter.name.clone()),
                    ))
                    .or_default();
            }
        }
    }
    let mut effective = Vec::new();
    for ((id, slot), declarations) in groups {
        let mut defaults: BTreeMap<&'static str, SourcedValue> = BTreeMap::new();
        let mut explicit: BTreeMap<&'static str, SourcedValue> = BTreeMap::new();
        for declaration in declarations {
            for family_id in &declaration.families {
                let Some(family) = family_index.get(family_id) else {
                    errors.push(
                        Finding::new("POL007", id.as_str(), "unknown policy family")
                            .at(&declaration.origin.location),
                    );
                    continue;
                };
                for value in &family.values {
                    insert_unique(
                        &mut defaults,
                        value,
                        &family.origin,
                        "POL003",
                        &id,
                        &mut errors,
                    );
                }
            }
            for value in &declaration.values {
                insert_unique(
                    &mut explicit,
                    value,
                    &declaration.origin,
                    "POL004",
                    &id,
                    &mut errors,
                );
            }
        }
        if let Slot::Parameter(name) = &slot {
            let parameter = symbols
                .iter()
                .find(|s| s.public_id == id)
                .and_then(|s| s.parameters.iter().find(|p| &p.name == name));
            if parameter.is_some_and(|p| is_value(&p.type_id, types, &mut BTreeSet::new())) {
                for value in profile_defaults {
                    defaults.entry(value.key()).or_insert_with(|| SourcedValue {
                        value: value.clone(),
                        origin: default_origin.clone(),
                        replaced_default: None,
                    });
                }
            }
        }
        for (key, mut value) in explicit {
            value.replaced_default = defaults.remove(key).map(|default| default.origin);
            defaults.insert(key, value);
        }
        effective.push(EffectivePolicy {
            symbol_id: id,
            slot,
            values: defaults,
        });
    }
    if errors.is_empty() {
        Ok(effective)
    } else {
        Err(errors)
    }
}

fn insert_unique(
    map: &mut BTreeMap<&'static str, SourcedValue>,
    value: &PolicyValue,
    origin: &Origin,
    code: &'static str,
    id: &PublicId,
    errors: &mut Vec<Finding>,
) {
    if let Some(previous) = map.get(value.key()) {
        errors.push(
            Finding::new(
                code,
                id.as_str(),
                format!(
                    "two authoritative values for {} (even when equal)",
                    value.key()
                ),
            )
            .at(&previous.origin.location)
            .at(&origin.location),
        );
    } else {
        map.insert(
            value.key(),
            SourcedValue {
                value: value.clone(),
                origin: origin.clone(),
                replaced_default: None,
            },
        );
    }
}

pub(crate) fn is_value(id: &Id, types: &[ContractType], visiting: &mut BTreeSet<Id>) -> bool {
    if visiting.len() >= 64 || !visiting.insert(id.clone()) {
        return false;
    }
    let result = types
        .iter()
        .find(|t| &t.id == id)
        .is_some_and(|t| match &t.definition {
            TypeDefinition::Resource { .. } | TypeDefinition::Callback { .. } => false,
            TypeDefinition::Nullable { inner } => is_value(inner, types, visiting),
            TypeDefinition::Buffer { element, .. } => is_value(element, types, visiting),
            TypeDefinition::Record { fields, .. } => {
                fields.iter().all(|p| is_value(&p.type_id, types, visiting))
            }
            TypeDefinition::Variant { cases, .. } => cases.iter().all(|v| {
                v.payload
                    .as_ref()
                    .is_none_or(|p| is_value(p, types, visiting))
            }),
            _ => true,
        });
    visiting.remove(id);
    result
}
