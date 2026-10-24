use crate::validate_identities;
use madb_model::contract::*;
use madb_model::policy::*;
use madb_model::{Finding, Id};
use std::collections::{BTreeMap, BTreeSet};

pub fn validate_contract(contract: &Contract, capability: &Capability) -> Vec<Finding> {
    let mut errors = validate_identities(&contract.namespace, &contract.symbols, &[]);
    errors.extend(crate::observations::validate_layouts(contract, capability));
    if contract.selector != capability.selector {
        errors.push(Finding::new(
            "CAP001",
            contract.boundary_id.as_str(),
            "resolved profile/target/direction mismatch",
        ));
    }
    let declared: BTreeSet<_> = contract.required_features.iter().map(Id::as_str).collect();
    for feature in &contract.required_features {
        if !CORE_FEATURES.contains(&feature.as_str())
            || !capability.supported_features.contains(feature)
            || capability.restrictions.iter().any(|(id, _)| id == feature)
        {
            errors.push(Finding::new(
                "CAP002",
                feature.as_str(),
                "unknown, restricted or unsupported mandatory feature",
            ));
        }
    }
    let mut types = BTreeMap::new();
    for item in &contract.types {
        if types.insert(&item.id, &item.definition).is_some() {
            errors.push(Finding::new(
                "TYPE001",
                item.id.as_str(),
                "duplicate type identity",
            ));
        }
        for reference in references(&item.definition) {
            if !contract.types.iter().any(|t| &t.id == reference) {
                errors.push(Finding::new(
                    "TYPE002",
                    item.id.as_str(),
                    format!("unknown type {reference}"),
                ));
            }
        }
        validate_type(item, &mut errors);
        let feature = match item.definition {
            TypeDefinition::Resource { .. } => "core.resources@1",
            TypeDefinition::Callback { .. } => "core.callbacks@1",
            TypeDefinition::Variant { .. } => "core.variants@1",
            _ => "core.values@1",
        };
        require_feature(feature, &declared, item.id.as_str(), &mut errors);
    }
    for item in &contract.types {
        if inline_cycle(&item.id, &types, &mut BTreeSet::new()) {
            errors.push(Finding::new(
                "TYPE003",
                item.id.as_str(),
                "recursive or excessively deep value layout requires an explicit indirection",
            ));
        }
        validate_composite(item, &types, &contract.types, &mut errors);
    }
    let mut policy_slots = BTreeSet::new();
    for policy in &contract.policies {
        if !policy_slots.insert((&policy.symbol_id, &policy.slot)) {
            errors.push(Finding::new(
                "POL008",
                policy.symbol_id.as_str(),
                "duplicate effective policy slot",
            ));
        }
        let Some(symbol) = contract
            .symbols
            .iter()
            .find(|s| s.public_id == policy.symbol_id)
        else {
            errors.push(Finding::new(
                "POL009",
                policy.symbol_id.as_str(),
                "effective policy references absent symbol",
            ));
            continue;
        };
        if policy.slot != Slot::Call && symbol.slot_type(&policy.slot).is_none() {
            errors.push(
                Finding::new(
                    "POL002",
                    symbol.public_id.as_str(),
                    "policy slot does not exist",
                )
                .at(&symbol.location),
            );
        }
        for (key, value) in &policy.values {
            if *key != value.value.key() {
                errors.push(
                    Finding::new(
                        "POL010",
                        symbol.public_id.as_str(),
                        "policy value/key mismatch",
                    )
                    .at(&symbol.location),
                );
            }
            if value.origin.authority == Authority::ProfileDefault
                && (value.value != PolicyValue::Ownership(Ownership::Copy)
                    || !matches!(&policy.slot, Slot::Parameter(_))
                    || !symbol.slot_type(&policy.slot).is_some_and(|id| {
                        crate::policies::is_value(id, &contract.types, &mut BTreeSet::new())
                    }))
            {
                errors.push(
                    Finding::new(
                        "POL006",
                        symbol.public_id.as_str(),
                        "unsafe inferred default",
                    )
                    .at(&value.origin.location),
                );
            }
            if value.replaced_default.as_ref().is_some_and(|o| {
                !matches!(
                    o.authority,
                    Authority::PolicyFamily | Authority::ProfileDefault
                )
            }) {
                errors.push(
                    Finding::new(
                        "POL005",
                        symbol.public_id.as_str(),
                        "replaced source is not a default",
                    )
                    .at(&value.origin.location),
                );
            }
        }
    }
    for symbol in &contract.symbols {
        if symbol.precision == Precision::Heuristic {
            errors.push(
                Finding::new(
                    "CAP003",
                    symbol.public_id.as_str(),
                    "heuristic structure cannot authorize a safe wrapper",
                )
                .at(&symbol.location),
            );
        }
        let mut names = BTreeSet::new();
        for parameter in &symbol.parameters {
            if !names.insert(&parameter.name) {
                errors.push(
                    Finding::new(
                        "TYPE004",
                        symbol.public_id.as_str(),
                        "duplicate parameter name",
                    )
                    .at(&symbol.location),
                );
            }
            if !types.contains_key(&parameter.type_id) {
                errors.push(
                    Finding::new(
                        "TYPE002",
                        symbol.public_id.as_str(),
                        "parameter type missing",
                    )
                    .at(&symbol.location),
                );
            }
        }
        if !types.contains_key(&symbol.result) {
            errors.push(
                Finding::new("TYPE002", symbol.public_id.as_str(), "return type missing")
                    .at(&symbol.location),
            );
        }
        if symbol.kind != CallKind::Sync {
            require_feature(
                if symbol.kind == CallKind::Stream {
                    "core.streams@1"
                } else {
                    "core.async@1"
                },
                &declared,
                symbol.public_id.as_str(),
                &mut errors,
            );
        }
        for constant in &symbol.constants {
            if let Err(message) = validate_constant(constant) {
                errors.push(
                    Finding::new("NUM001", symbol.public_id.as_str(), message).at(&symbol.location),
                );
            }
        }
        if !symbol.constants.is_empty() {
            require_feature(
                "core.exact-numbers@1",
                &declared,
                symbol.public_id.as_str(),
                &mut errors,
            );
        }
        let call = policy_for(contract, symbol, &Slot::Call);
        require_properties(
            call,
            &[
                "threading",
                "blocking",
                "delivery",
                "cancellation",
                "cancellation_confirmation",
            ],
            symbol,
            &mut errors,
        );
        validate_call(call, symbol, &mut errors);
        for parameter in &symbol.parameters {
            validate_slot(
                contract,
                symbol,
                &Slot::Parameter(parameter.name.clone()),
                &parameter.type_id,
                &types,
                &mut errors,
            );
        }
        validate_slot(
            contract,
            symbol,
            &Slot::Return,
            &symbol.result,
            &types,
            &mut errors,
        );
    }
    errors
}

fn require_feature(
    feature: &str,
    declared: &BTreeSet<&str>,
    subject: &str,
    errors: &mut Vec<Finding>,
) {
    if !declared.contains(feature) {
        errors.push(Finding::new(
            "CAP004",
            subject,
            format!("required feature not declared: {feature}"),
        ));
    }
}
fn policy_for<'a>(
    contract: &'a Contract,
    symbol: &Symbol,
    slot: &Slot,
) -> Option<&'a EffectivePolicy> {
    contract
        .policies
        .iter()
        .find(|p| p.symbol_id == symbol.public_id && &p.slot == slot)
}
fn get<'a>(policy: Option<&'a EffectivePolicy>, key: &str) -> Option<&'a PolicyValue> {
    policy.and_then(|p| p.values.get(key)).map(|v| &v.value)
}
fn require_properties(
    policy: Option<&EffectivePolicy>,
    keys: &[&str],
    symbol: &Symbol,
    errors: &mut Vec<Finding>,
) {
    for key in keys {
        if get(policy, key).is_none() {
            errors.push(
                Finding::new(
                    "SEM001",
                    symbol.public_id.as_str(),
                    format!("required policy missing: {key}"),
                )
                .at(&symbol.location),
            );
        }
    }
}
fn validate_call(policy: Option<&EffectivePolicy>, symbol: &Symbol, errors: &mut Vec<Finding>) {
    let matching = match get(policy, "cancellation") {
        Some(PolicyValue::Cancellation(Cancellation::None)) => matches!(
            get(policy, "cancellation_confirmation"),
            Some(PolicyValue::CancellationConfirmation(
                CancellationConfirmation::NotSupported
            ))
        ),
        Some(PolicyValue::Cancellation(Cancellation::Wait)) => matches!(
            get(policy, "cancellation_confirmation"),
            Some(PolicyValue::CancellationConfirmation(
                CancellationConfirmation::WaitEnded
            ))
        ),
        Some(PolicyValue::Cancellation(Cancellation::Observe)) => matches!(
            get(policy, "cancellation_confirmation"),
            Some(PolicyValue::CancellationConfirmation(
                CancellationConfirmation::Stopped
            ))
        ),
        Some(PolicyValue::Cancellation(Cancellation::Work)) => matches!(
            get(policy, "cancellation_confirmation"),
            Some(PolicyValue::CancellationConfirmation(
                CancellationConfirmation::OperationTerminal
            ))
        ),
        _ => true,
    };
    if !matching {
        errors.push(
            Finding::new(
                "SEM002",
                symbol.public_id.as_str(),
                "cancellation effect and confirmation disagree",
            )
            .at(&symbol.location),
        );
    }
    if symbol.kind == CallKind::Stream {
        require_properties(
            policy,
            &[
                "stream_kind",
                "capacity_items",
                "capacity_bytes",
                "overflow",
                "ordering",
                "replay",
                "recovery",
                "stop",
            ],
            symbol,
            errors,
        );
        if matches!(
            get(policy, "stream_kind"),
            Some(PolicyValue::StreamKind(StreamKind::None))
        ) || matches!(get(policy, "stop"), Some(PolicyValue::Stop(Stop::None)))
        {
            errors.push(
                Finding::new(
                    "SEM003",
                    symbol.public_id.as_str(),
                    "stream requires delivery and stop semantics",
                )
                .at(&symbol.location),
            );
        }
        if matches!(
            get(policy, "stream_kind"),
            Some(PolicyValue::StreamKind(StreamKind::Event))
        ) && matches!(
            get(policy, "overflow"),
            Some(PolicyValue::Overflow(Overflow::CoalesceLatest))
        ) {
            errors.push(
                Finding::new(
                    "SEM004",
                    symbol.public_id.as_str(),
                    "event cannot silently inherit state coalescing",
                )
                .at(&symbol.location),
            );
        }
    }
}
fn validate_slot(
    contract: &Contract,
    symbol: &Symbol,
    slot: &Slot,
    type_id: &Id,
    types: &BTreeMap<&Id, &TypeDefinition>,
    errors: &mut Vec<Finding>,
) {
    let policy = policy_for(contract, symbol, slot);
    let mut current = type_id;
    let mut visited = BTreeSet::new();
    let definition = loop {
        if !visited.insert(current) {
            return;
        }
        match types.get(current).copied() {
            Some(TypeDefinition::Nullable { inner }) => current = inner,
            other => break other,
        }
    };
    match definition {
        Some(TypeDefinition::Resource { .. }) => {
            require_properties(
                policy,
                &["ownership", "close", "release_thread"],
                symbol,
                errors,
            );
            if !matches!(
                get(policy, "close"),
                Some(PolicyValue::Close(Close::RequestAwaitRelease))
            ) || matches!(
                get(policy, "ownership"),
                Some(PolicyValue::Ownership(Ownership::Copy))
            ) {
                errors.push(
                    Finding::new(
                        "SEM005",
                        symbol.public_id.as_str(),
                        "resource requires explicit close and cannot use value copy",
                    )
                    .at(&symbol.location),
                );
            }
            if !contract
                .required_features
                .iter()
                .any(|f| f.as_str() == "core.explicit-close@1")
            {
                errors.push(Finding::new(
                    "CAP004",
                    symbol.public_id.as_str(),
                    "resource close feature not declared",
                ));
            }
        }
        Some(TypeDefinition::Buffer { .. } | TypeDefinition::String { .. }) => {
            require_properties(policy, &["ownership"], symbol, errors)
        }
        Some(TypeDefinition::Callback { .. }) => {
            require_properties(
                policy,
                &[
                    "synchronous_callback",
                    "reentrant",
                    "threading",
                    "context_owner",
                    "stop",
                    "keep_alive",
                ],
                symbol,
                errors,
            );
            if matches!(get(policy, "stop"), Some(PolicyValue::Stop(Stop::None)))
                && !matches!(
                    get(policy, "keep_alive"),
                    Some(PolicyValue::KeepAlive(KeepAlive::SourceLease))
                )
            {
                errors.push(
                    Finding::new(
                        "SEM006",
                        symbol.public_id.as_str(),
                        "non-unregisterable callback must retain a source lease",
                    )
                    .at(&symbol.location),
                );
            }
        }
        _ => {}
    }
    if matches!(
        get(policy, "ownership"),
        Some(PolicyValue::Ownership(Ownership::Transfer))
    ) {
        require_properties(policy, &["releaser"], symbol, errors);
        if let Some(PolicyValue::Releaser(id)) = get(policy, "releaser") {
            let releaser = contract.symbols.iter().find(|s| &s.public_id == id);
            if !releaser.is_some_and(|s| {
                s.kind == CallKind::Sync
                    && s.parameters.len() == 1
                    && s.parameters[0].type_id == *type_id
                    && matches!(types.get(&s.result), Some(TypeDefinition::Unit))
            }) {
                errors.push(
                    Finding::new(
                        "SEM007",
                        symbol.public_id.as_str(),
                        "release function must exist with the matching logical signature",
                    )
                    .at(&symbol.location),
                );
            }
        }
    }
    if matches!(
        get(policy, "ownership"),
        Some(PolicyValue::Ownership(Ownership::Borrow))
    ) {
        require_properties(policy, &["borrow_scope", "keep_alive"], symbol, errors);
        if symbol.kind != CallKind::Sync
            && matches!(
                get(policy, "keep_alive"),
                Some(PolicyValue::KeepAlive(KeepAlive::CallFrame))
            )
        {
            errors.push(
                Finding::new(
                    "SEM008",
                    symbol.public_id.as_str(),
                    "async/stream borrow outlives call-frame protection",
                )
                .at(&symbol.location),
            );
        }
        if *slot == Slot::Return
            && matches!(
                get(policy, "borrow_scope"),
                Some(PolicyValue::BorrowScope(BorrowScope::Call))
            )
        {
            errors.push(
                Finding::new(
                    "SEM009",
                    symbol.public_id.as_str(),
                    "returned borrow cannot expire with the returning call",
                )
                .at(&symbol.location),
            );
        }
    }
}

fn references(definition: &TypeDefinition) -> Vec<&Id> {
    match definition {
        TypeDefinition::Buffer { element, .. } => vec![element],
        TypeDefinition::Nullable { inner } => vec![inner],
        TypeDefinition::Record { fields, .. } => fields.iter().map(|p| &p.type_id).collect(),
        TypeDefinition::Variant {
            tag_type, cases, ..
        } => std::iter::once(tag_type)
            .chain(cases.iter().filter_map(|c| c.payload.as_ref()))
            .collect(),
        TypeDefinition::Callback {
            parameters, result, ..
        } => parameters
            .iter()
            .map(|p| &p.type_id)
            .chain(std::iter::once(result))
            .collect(),
        _ => Vec::new(),
    }
}
fn inline_cycle<'a>(
    id: &'a Id,
    types: &BTreeMap<&'a Id, &'a TypeDefinition>,
    visiting: &mut BTreeSet<&'a Id>,
) -> bool {
    if visiting.len() >= 64 || !visiting.insert(id) {
        return true;
    }
    let cyclic = match types.get(id).copied() {
        Some(
            definition @ (TypeDefinition::Record { .. }
            | TypeDefinition::Nullable { .. }
            | TypeDefinition::Variant { .. }),
        ) => references(definition)
            .into_iter()
            .any(|next| inline_cycle(next, types, visiting)),
        _ => false,
    };
    visiting.remove(id);
    cyclic
}
fn validate_type(item: &ContractType, errors: &mut Vec<Finding>) {
    let invalid = match &item.definition {
        TypeDefinition::Integer { width, .. } => ![8, 16, 32, 64, 128].contains(width),
        TypeDefinition::Float { width } => ![32, 64].contains(width),
        TypeDefinition::String {
            encoding,
            max_bytes,
        } => !["utf-8", "utf-16"].contains(&encoding.as_str()) || *max_bytes == 0,
        TypeDefinition::Buffer {
            length_unit,
            encoding,
            max_bytes,
            ..
        } => {
            !["bytes", "elements"].contains(&length_unit.as_str())
                || !["none", "utf-8", "utf-16"].contains(&encoding.as_str())
                || *max_bytes == 0
        }
        _ => false,
    };
    if invalid {
        errors.push(Finding::new(
            "TYPE005",
            item.id.as_str(),
            "invalid scalar or buffer definition",
        ));
    }
    let fields = match &item.definition {
        TypeDefinition::Record { fields, .. } => Some(fields),
        TypeDefinition::Callback { parameters, .. } => Some(parameters),
        _ => None,
    };
    if let Some(fields) = fields {
        let names: BTreeSet<_> = fields.iter().map(|f| &f.name).collect();
        if names.len() != fields.len() {
            errors.push(Finding::new(
                "TYPE004",
                item.id.as_str(),
                "duplicate field name",
            ));
        }
    }
    if let TypeDefinition::Variant { cases, .. } = &item.definition {
        let mut names = BTreeSet::new();
        let mut tags = BTreeSet::new();
        for case in cases {
            if !names.insert(&case.name) || !tags.insert(format!("{:?}", case.tag)) {
                errors.push(Finding::new(
                    "TYPE006",
                    item.id.as_str(),
                    "duplicate variant name or tag",
                ));
            }
            if let Err(message) = validate_constant(&case.tag) {
                errors.push(Finding::new("NUM001", item.id.as_str(), message));
            }
        }
    }
}

fn validate_composite(
    item: &ContractType,
    types: &BTreeMap<&Id, &TypeDefinition>,
    all: &[ContractType],
    errors: &mut Vec<Finding>,
) {
    let value = |id: &Id| crate::policies::is_value(id, all, &mut BTreeSet::new());
    let unsupported = match &item.definition {
        TypeDefinition::Buffer { element, .. } => !matches!(
            types.get(element),
            Some(
                TypeDefinition::Integer { .. }
                    | TypeDefinition::Float { .. }
                    | TypeDefinition::Boolean
            )
        ),
        TypeDefinition::Record { fields, .. } => fields.iter().any(|p| !value(&p.type_id)),
        TypeDefinition::Variant { cases, .. } => cases
            .iter()
            .any(|c| c.payload.as_ref().is_some_and(|id| !value(id))),
        _ => false,
    };
    if unsupported {
        errors.push(Finding::new("TYPE007",item.id.as_str(),"nested resource/callback ownership or non-scalar buffer requires an explicit adapter; not supported by v1"));
    }
    if let TypeDefinition::Variant {
        tag_type, cases, ..
    } = &item.definition
    {
        for case in cases {
            if !matches!((types.get(tag_type), &case.tag), (Some(TypeDefinition::Integer{width:a,signed:x}),ExactConstant::Integer{width:b,signed:y,..}) if a == b && x == y)
            {
                errors.push(Finding::new(
                    "TYPE008",
                    item.id.as_str(),
                    "variant tag must match its declared integer type",
                ));
            }
        }
    }
}

pub fn validate_constant(value: &ExactConstant) -> Result<(), &'static str> {
    match value {
        ExactConstant::Integer {
            signed,
            width,
            decimal,
        } => {
            if ![8, 16, 32, 64, 128].contains(width) {
                return Err("invalid integer width");
            }
            if *signed {
                let number = decimal
                    .parse::<i128>()
                    .map_err(|_| "signed constant is outside its domain")?;
                if number.to_string() != *decimal {
                    return Err("integer is not canonical decimal");
                }
                if *width < 128
                    && (number < -(1i128 << (width - 1)) || number > (1i128 << (width - 1)) - 1)
                {
                    return Err("signed constant exceeds declared width");
                }
            } else {
                let number = decimal
                    .parse::<u128>()
                    .map_err(|_| "unsigned constant is outside its domain")?;
                if number.to_string() != *decimal {
                    return Err("integer is not canonical decimal");
                }
                if *width < 128 && number > (1u128 << width) - 1 {
                    return Err("unsigned constant exceeds declared width");
                }
            }
        }
        ExactConstant::Float { width, bits }
            if ![32, 64].contains(width)
                || bits.len() != usize::from(*width / 4)
                || !bits
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) =>
        {
            return Err("expected exact lowercase IEEE bit pattern");
        }
        _ => {}
    }
    Ok(())
}
