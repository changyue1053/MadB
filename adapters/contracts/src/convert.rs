use crate::wire::Wire;
use madb_model::contract::*;
use madb_model::evidence::*;
use madb_model::policy::*;
use madb_model::{Digest, Id, Location, LogicalPath, SourceKey};
use madb_protocol::{Document, DocumentKind, FormatError};
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, FormatError>;
pub struct PolicyInputs {
    pub namespace: Id,
    pub bindings: Vec<Binding>,
    pub families: Vec<Family>,
}

fn collect<T>(wire: Wire<'_>, parse: impl Fn(Wire<'_>) -> Result<T>) -> Result<Vec<T>> {
    wire.array()?.into_iter().map(parse).collect()
}
fn ids(wire: Wire<'_>) -> Result<Vec<Id>> {
    collect(wire, |w| w.id())
}
fn location(w: Wire<'_>) -> Result<Location> {
    let path = w.field("path")?;
    Ok(Location {
        path: LogicalPath::new(path.string()?).map_err(|e| path.domain_error(e))?,
        line: w.field("line")?.uint()?,
        column: w.field("column")?.uint()?,
    })
}
fn origin(w: Wire<'_>) -> Result<Origin> {
    Ok(Origin {
        authority: w.field("authority")?.choice(Authority::parse)?,
        id: w.field("id")?.id()?,
        location: location(w.field("location")?)?,
    })
}
fn slot(w: Wire<'_>) -> Result<Slot> {
    match w.field("kind")?.string()? {
        "call" => Ok(Slot::Call),
        "return" => Ok(Slot::Return),
        "parameter" => Ok(Slot::Parameter(w.field("name")?.id()?)),
        _ => Err(w.error("unknown slot")),
    }
}
fn direction(w: Wire<'_>) -> Result<Direction> {
    w.choice(|s| match s {
        "consumer-to-provider" => Some(Direction::ConsumerToProvider),
        "host-callback" => Some(Direction::HostCallback),
        "reverse-export" => Some(Direction::ReverseExport),
        _ => None,
    })
}
fn selector(w: Wire<'_>) -> Result<ResolvedSelector> {
    Ok(ResolvedSelector {
        route: w.field("route")?.id()?,
        profile_id: w.field("profile_id")?.id()?,
        profile_digest: w.field("profile_digest")?.digest()?,
        target: w.field("target")?.id()?,
        direction: direction(w.field("direction")?)?,
    })
}
fn convention(w: Wire<'_>) -> Result<CallingConvention> {
    w.choice(|s| match s {
        "c" => Some(CallingConvention::C),
        "system" => Some(CallingConvention::System),
        _ => None,
    })
}
fn parameter(w: Wire<'_>) -> Result<Parameter> {
    Ok(Parameter {
        name: w.field("name")?.id()?,
        type_id: w.field("type_id")?.id()?,
    })
}
fn constant(w: Wire<'_>) -> Result<ExactConstant> {
    Ok(match w.field("kind")?.string()? {
        "integer" => {
            let t = w.field("type")?.string()?;
            ExactConstant::Integer {
                signed: t.starts_with('i'),
                width: t[1..]
                    .parse()
                    .map_err(|_| w.error("invalid integer width"))?,
                decimal: w.field("decimal")?.string()?.into(),
            }
        }
        "float" => {
            let t = w.field("type")?.string()?;
            ExactConstant::Float {
                width: t[1..].parse().map_err(|_| w.error("invalid float width"))?,
                bits: w.field("bits")?.string()?.into(),
            }
        }
        "string" => ExactConstant::String(w.field("value")?.string()?.into()),
        "boolean" => ExactConstant::Boolean(w.field("value")?.boolean()?),
        _ => return Err(w.error("unknown exact constant")),
    })
}
fn type_definition(w: Wire<'_>) -> Result<ContractType> {
    let id = w.field("id")?.id()?;
    let d = w.field("definition")?;
    let definition = match d.field("kind")?.string()? {
        "unit" => TypeDefinition::Unit,
        "boolean" => TypeDefinition::Boolean,
        "integer" => TypeDefinition::Integer {
            width: d.field("width")?.uint()? as u16,
            signed: d.field("signed")?.boolean()?,
        },
        "float" => TypeDefinition::Float {
            width: d.field("width")?.uint()? as u16,
        },
        "string" => TypeDefinition::String {
            encoding: d.field("encoding")?.string()?.into(),
            max_bytes: d.field("max_bytes")?.uint()?,
        },
        "buffer" => TypeDefinition::Buffer {
            element: d.field("element")?.id()?,
            length_unit: d.field("length_unit")?.string()?.into(),
            encoding: d.field("encoding")?.string()?.into(),
            mutable: d.field("mutable")?.boolean()?,
            max_bytes: d.field("max_bytes")?.uint()?,
        },
        "nullable" => TypeDefinition::Nullable {
            inner: d.field("inner")?.id()?,
        },
        "record" => TypeDefinition::Record {
            fields: collect(d.field("fields")?, parameter)?,
            c_layout: d.field("layout")?.string()? == "c",
        },
        "variant" => TypeDefinition::Variant {
            tag_type: d.field("tag_type")?.id()?,
            cases: collect(d.field("cases")?, |c| {
                let payload = c.field("payload")?;
                Ok(VariantCase {
                    name: c.field("name")?.id()?,
                    tag: constant(c.field("tag")?)?,
                    payload: if payload.value.is_null() {
                        None
                    } else {
                        Some(payload.id()?)
                    },
                })
            })?,
            preserve_unknown: d.field("unknown")?.string()? == "preserve",
        },
        "resource" => TypeDefinition::Resource {
            registry_u64: d.field("representation")?.string()? == "registry-u64",
            owner: d.field("owner")?.id()?,
        },
        "callback" => TypeDefinition::Callback {
            parameters: collect(d.field("parameters")?, parameter)?,
            result: d.field("result")?.id()?,
            calling_convention: convention(d.field("calling_convention")?)?,
        },
        _ => return Err(d.error("unknown type definition")),
    };
    Ok(ContractType { id, definition })
}
fn symbol(w: Wire<'_>) -> Result<Symbol> {
    let key = w.field("source_key")?;
    let shape = w.field("shape")?;
    Ok(Symbol {
        source_key: SourceKey {
            reader: key.field("reader")?.id()?,
            package: key.field("package")?.string()?.into(),
            module: key.field("module")?.string()?.into(),
            kind: key.field("kind")?.string()?.into(),
            qualified_name: key.field("qualified_name")?.string()?.into(),
            overload: key.field("overload")?.string()?.into(),
        },
        source_id: w.field("source_id")?.source_id()?,
        public_id: w.field("public_id")?.public_id()?,
        public_id_origin: w.field("public_id_origin")?.choice(|s| match s {
            "explicit" => Some(PublicIdOrigin::Explicit),
            "derived" => Some(PublicIdOrigin::Derived),
            _ => None,
        })?,
        location: location(w.field("location")?)?,
        precision: w.field("precision")?.choice(|s| match s {
            "compiler-derived" => Some(Precision::CompilerDerived),
            "artifact-derived" => Some(Precision::ArtifactDerived),
            "schema-derived" => Some(Precision::SchemaDerived),
            "heuristic" => Some(Precision::Heuristic),
            _ => None,
        })?,
        kind: shape.field("kind")?.choice(|s| match s {
            "sync" => Some(CallKind::Sync),
            "async" => Some(CallKind::Async),
            "stream" => Some(CallKind::Stream),
            _ => None,
        })?,
        calling_convention: convention(shape.field("calling_convention")?)?,
        link_name: shape.field("link_name")?.id()?,
        parameters: collect(shape.field("parameters")?, parameter)?,
        result: shape.field("result")?.id()?,
        constants: collect(shape.field("constants")?, constant)?,
        shape_digest: w.field("shape_digest")?.digest()?,
    })
}
pub fn contract(document: &Document) -> Result<Contract> {
    let w = Wire::root(document, DocumentKind::ContractIr)?;
    Ok(Contract {
        namespace: w.field("sdk_namespace")?.id()?,
        boundary_id: w.field("boundary_id")?.id()?,
        selector: selector(w.field("selector")?)?,
        source_digest: w.field("source_digest")?.digest()?,
        structure_digest: w.field("structure_digest")?.digest()?,
        policy_digest: w.field("policy_digest")?.digest()?,
        required_features: ids(w.field("required_features")?)?,
        types: collect(w.field("types")?, type_definition)?,
        symbols: collect(w.field("symbols")?, symbol)?,
        policies: collect(w.field("policies")?, effective_policy)?,
        observations: collect(w.field("observations")?, observation)?,
    })
}

fn values(w: Wire<'_>) -> Result<Vec<PolicyValue>> {
    w.object()?
        .into_iter()
        .map(|(key, v)| {
            Ok(match key {
                "ownership" => PolicyValue::Ownership(v.choice(Ownership::parse)?),
                "releaser" => PolicyValue::Releaser(v.public_id()?),
                "borrow_scope" => PolicyValue::BorrowScope(v.choice(BorrowScope::parse)?),
                "keep_alive" => PolicyValue::KeepAlive(v.choice(KeepAlive::parse)?),
                "threading" => PolicyValue::Threading(v.choice(Threading::parse)?),
                "blocking" => PolicyValue::Blocking(v.choice(Blocking::parse)?),
                "delivery" => PolicyValue::Delivery(v.choice(Delivery::parse)?),
                "cancellation" => PolicyValue::Cancellation(v.choice(Cancellation::parse)?),
                "cancellation_confirmation" => PolicyValue::CancellationConfirmation(
                    v.choice(CancellationConfirmation::parse)?,
                ),
                "close" => PolicyValue::Close(v.choice(Close::parse)?),
                "synchronous_callback" => PolicyValue::SynchronousCallback(v.boolean()?),
                "reentrant" => PolicyValue::Reentrant(v.boolean()?),
                "context_owner" => PolicyValue::ContextOwner(v.choice(ContextOwner::parse)?),
                "stop" => PolicyValue::Stop(v.choice(Stop::parse)?),
                "stream_kind" => PolicyValue::StreamKind(v.choice(StreamKind::parse)?),
                "capacity_items" => PolicyValue::CapacityItems(v.uint()?),
                "capacity_bytes" => PolicyValue::CapacityBytes(v.uint()?),
                "overflow" => PolicyValue::Overflow(v.choice(Overflow::parse)?),
                "ordering" => PolicyValue::Ordering(v.choice(Ordering::parse)?),
                "replay" => PolicyValue::Replay(v.choice(Replay::parse)?),
                "recovery" => PolicyValue::Recovery(v.choice(Recovery::parse)?),
                "release_thread" => PolicyValue::ReleaseThread(v.choice(ReleaseThread::parse)?),
                _ => return Err(v.error("unknown policy property")),
            })
        })
        .collect()
}
fn effective_policy(w: Wire<'_>) -> Result<EffectivePolicy> {
    let provenance = w.field("provenance")?.array()?;
    let entries = values(w.field("values")?)?;
    let mut mapped = BTreeMap::new();
    if provenance.len() != entries.len() {
        return Err(w.error("provenance must cover each property exactly once"));
    }
    for value in entries {
        let sources: Vec<_> = provenance
            .iter()
            .filter(|p| p.value.get("property").and_then(|v| v.as_str()) == Some(value.key()))
            .collect();
        if sources.len() != 1 {
            return Err(w.error("missing or duplicate property provenance"));
        }
        let p = sources[0];
        let replaced = p.field("replaced_default")?;
        mapped.insert(
            value.key(),
            SourcedValue {
                value,
                origin: origin(p.field("origin")?)?,
                replaced_default: if replaced.value.is_null() {
                    None
                } else {
                    Some(origin(replaced)?)
                },
            },
        );
    }
    Ok(EffectivePolicy {
        symbol_id: w.field("symbol_id")?.public_id()?,
        slot: slot(w.field("slot")?)?,
        values: mapped,
    })
}
pub fn project_policies(document: &Document) -> Result<PolicyInputs> {
    let w = Wire::root(document, DocumentKind::Project)?;
    Ok(PolicyInputs {
        namespace: w.field("sdk_namespace")?.id()?,
        bindings: collect(w.field("policies")?, |b| {
            let selected = b.field("selector")?;
            let o = origin(b.field("origin")?)?;
            if o.authority != Authority::ProjectPolicy {
                return Err(b.error("project cannot claim source-annotation authority"));
            }
            Ok(Binding {
                selector: match selected.field("kind")?.string()? {
                    "public_id" => ExactSelector::Public(selected.field("id")?.public_id()?),
                    "source_id" => ExactSelector::Source(selected.field("id")?.source_id()?),
                    _ => return Err(selected.error("unknown exact selector")),
                },
                slot: slot(b.field("slot")?)?,
                families: ids(b.field("families")?)?,
                values: values(b.field("values")?)?,
                origin: o,
            })
        })?,
        families: collect(w.field("policy_families")?, |f| {
            Ok(Family {
                id: f.field("id")?.id()?,
                values: values(f.field("values")?)?,
                origin: origin(f.field("origin")?)?,
            })
        })?,
    })
}
pub(crate) fn profile_defaults(document: &Document) -> Result<(Vec<PolicyValue>, Origin)> {
    let w = Wire::root(document, DocumentKind::Profile)?.field("defaults")?;
    Ok((values(w.field("values")?)?, origin(w.field("origin")?)?))
}
pub fn capability(document: &Document) -> Result<Capability> {
    let w = Wire::root(document, DocumentKind::Profile)?;
    if !w.field("backend_config")?.value.is_null() {
        return Err(FormatError::new(
            "CAP005",
            "/backend_config",
            "a backend configuration needs its registered validator; C02 core has none",
        ));
    }
    let supported = ids(w.field("supported_features")?)?;
    for feature in ids(w.field("required_features")?)? {
        if !CORE_FEATURES.contains(&feature.as_str()) || !supported.contains(&feature) {
            return Err(FormatError::new(
                "CAP002",
                "/required_features",
                "profile requires an unknown or unsupported semantic feature",
            ));
        }
    }
    Ok(Capability {
        producers: collect(w.field("tools")?, producer)?,
        selector: ResolvedSelector {
            route: w.field("route")?.id()?,
            profile_id: w.field("id")?.id()?,
            profile_digest: Digest::new(crate::profile_digest(document)?)
                .map_err(|e| w.domain_error(e))?,
            target: w.field("target")?.id()?,
            direction: direction(w.field("direction")?)?,
        },
        supported_features: ids(w.field("supported_features")?)?,
        restrictions: collect(w.field("restrictions")?, |r| {
            Ok((
                r.field("feature")?.id()?,
                r.field("reason")?.string()?.into(),
            ))
        })?,
    })
}

pub fn evidence_identity(document: &Document) -> Result<EvidenceIdentity> {
    if ![DocumentKind::Report, DocumentKind::VerifyRequest].contains(&document.kind()) {
        return Err(FormatError::new(
            "FMT003",
            "/schema",
            "not an evidence-bearing document",
        ));
    }
    let w = Wire::at(document.value(), "").field("identity")?;
    Ok(EvidenceIdentity {
        input: w.field("input_digest")?.digest()?,
        tools: w.field("tool_digest")?.digest()?,
        profile: w.field("profile_digest")?.digest()?,
        candidate: w.field("candidate_digest")?.digest()?,
        lab_revision: w.field("lab_revision")?.string()?.into(),
        case_catalog: w.field("case_catalog_digest")?.digest()?,
    })
}
fn expected(w: Wire<'_>) -> Result<ExpectedOutcome> {
    Ok(match w.field("kind")?.string()? {
        "success" => ExpectedOutcome::Success,
        "rejection" => ExpectedOutcome::Rejection {
            code: w.field("code")?.id()?,
            stage: w.field("stage")?.id()?,
        },
        "isolated-abort" => ExpectedOutcome::IsolatedAbort {
            exit_code: w.field("exit_code")?.int()? as i32,
            reason: w.field("reason")?.id()?,
        },
        _ => return Err(w.error("unknown expected outcome")),
    })
}
pub fn report_results(document: &Document) -> Result<Vec<CaseResult>> {
    let w = Wire::root(document, DocumentKind::Report)?;
    collect(w.field("results")?, |r| {
        let actual = r.field("actual")?;
        Ok(CaseResult {
            case_id: r.field("case_id")?.id()?,
            case_digest: r.field("case_digest")?.digest()?,
            status: r.field("status")?.choice(|s| match s {
                "passed" => Some(ExecutionStatus::Passed),
                "failed" => Some(ExecutionStatus::Failed),
                "error" => Some(ExecutionStatus::Error),
                "skipped" => Some(ExecutionStatus::Skipped),
                "unsupported" => Some(ExecutionStatus::Unsupported),
                "blocked" => Some(ExecutionStatus::Blocked),
                _ => None,
            })?,
            expected: expected(r.field("expected")?)?,
            actual: match actual.field("kind")?.string()? {
                "success" => ActualOutcome::Success,
                "rejection" => ActualOutcome::Rejection {
                    code: actual.field("code")?.id()?,
                    stage: actual.field("stage")?.id()?,
                },
                "isolated-abort" => ActualOutcome::IsolatedAbort {
                    exit_code: actual.field("exit_code")?.int()? as i32,
                    reason: actual.field("reason")?.id()?,
                },
                "unavailable" => ActualOutcome::Unavailable,
                "not-run" => ActualOutcome::NotRun,
                _ => return Err(actual.error("unknown actual outcome")),
            },
            level: r.field("evidence_level")?.choice(|s| match s {
                "static" => Some(EvidenceLevel::Static),
                "generated" => Some(EvidenceLevel::Generated),
                "compiled" => Some(EvidenceLevel::Compiled),
                "linked" => Some(EvidenceLevel::Linked),
                "host-run" => Some(EvidenceLevel::HostRun),
                "target-run" => Some(EvidenceLevel::TargetRun),
                "device-run" => Some(EvidenceLevel::DeviceRun),
                _ => None,
            })?,
            cleanup_complete: r.field("cleanup")?.string()? == "complete",
            attachment_count: r.field("attachments")?.array()?.len(),
            resources: collect(r.field("resource_observations")?, |o| {
                Ok(ResourceObservation {
                    name: o.field("name")?.id()?,
                    before: o.field("before")?.uint()?,
                    after: o.field("after")?.uint()?,
                    created: o.field("created")?.uint()?,
                    released: o.field("released")?.uint()?,
                })
            })?,
        })
    })
}
pub fn requirements(document: &Document) -> Result<(ResolvedSelector, Vec<Requirement>)> {
    let w = Wire::root(document, DocumentKind::Coverage)?;
    Ok((
        selector(w.field("selector")?)?,
        collect(w.field("requirements")?, |r| {
            Ok(Requirement {
                problem_id: r.field("problem_id")?.id()?,
                applicability: r.field("applicability")?.choice(|s| match s {
                    "required" => Some(Applicability::Required),
                    "optional" => Some(Applicability::Optional),
                    "not-applicable" => Some(Applicability::NotApplicable),
                    _ => None,
                })?,
                reason: r.field("reason")?.string()?.into(),
                case_ids: ids(r.field("case_ids")?)?,
            })
        })?,
    ))
}

/// Build an expectation from an independently loaded case, never from a report.
pub fn required_case(
    document: &Document,
    selected: &ResolvedSelector,
    minimum_level: EvidenceLevel,
) -> Result<RequiredCase> {
    let w = Wire::root(document, DocumentKind::Case)?;
    if w.field("kind")?.string()? != "instance" || selector(w.field("selector")?)? != *selected {
        return Err(FormatError::new(
            "CASE001",
            "/selector",
            "case is not instantiated for this target/profile/direction",
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for phase in ["setup", "steps", "cleanup"] {
        for step in w.field(phase)?.array()? {
            if !seen.insert(step.field("id")?.id()?) {
                return Err(FormatError::new(
                    "CASE002",
                    phase,
                    "duplicate case step identity",
                ));
            }
            for arg in step.field("arguments")?.array()? {
                madb_rules::validate_constant(&constant(arg)?)
                    .map_err(|e| FormatError::new("NUM001", phase, e))?;
            }
        }
    }
    for assertion in w.field("assertions")?.array()? {
        for value in assertion.field("expected")?.array()? {
            madb_rules::validate_constant(&constant(value)?)
                .map_err(|e| FormatError::new("NUM001", "/assertions", e))?;
        }
    }
    for feature in ids(w.field("requires")?.field("features")?)? {
        if !CORE_FEATURES.contains(&feature.as_str()) {
            return Err(FormatError::new(
                "CAP002",
                "/requires/features",
                "unknown required case semantics",
            ));
        }
    }
    Ok(RequiredCase {
        id: w.field("id")?.id()?,
        digest: Digest::new(crate::case_digest(document)?).map_err(|e| w.domain_error(e))?,
        expected: expected(w.field("expected_result")?)?,
        minimum_level,
    })
}

fn producer(w: Wire<'_>) -> Result<Producer> {
    Ok(Producer {
        id: w.field("id")?.id()?,
        version: w.field("version")?.string()?.into(),
        digest: w.field("sha256")?.digest()?,
    })
}
fn observation(w: Wire<'_>) -> Result<LayoutObservation> {
    let result = w.field("result")?;
    let precision = w.field("precision")?.choice(|s| {
        if s == "compiler-derived" {
            Some(Precision::CompilerDerived)
        } else {
            None
        }
    })?;
    Ok(LayoutObservation {
        subject: w.field("subject")?.id()?,
        precision,
        input: w.field("input_digest")?.digest()?,
        selector: selector(w.field("selector")?)?,
        producer: producer(w.field("producer")?)?,
        size: result.field("size")?.uint()?,
        alignment: result.field("alignment")?.uint()?,
        fields: collect(result.field("fields")?, |f| {
            Ok(FieldLayout {
                name: f.field("name")?.id()?,
                offset: f.field("offset")?.uint()?,
                size: f.field("size")?.uint()?,
            })
        })?,
    })
}
