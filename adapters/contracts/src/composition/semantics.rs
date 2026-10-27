//! Finite C07 source semantics. This checks the reviewed source recipe, not a
//! native build, physical assembly, target execution, or SDK certification.
use super::{
    AssetOrigin, BoundCompositionInputs, ResolvedOwner, ResolvedResponsibility, ResolvedScope,
    ResolvedSubject, ResponsibilityKey, Result, check_composition_snapshot_binding, invalid, text,
};
use crate::{Document, DocumentKind};
use madb_model::{Digest, Id, LogicalPath, PublicId};
use madb_protocol::{Value, strict_json};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

const PACKAGE: &str = "madb.local.c07-composition";
const POLICY: &str = "adapters/generators/arkts/profiles/snapshot-v1.json";
const CORE: &str = "sdk/arkts/native-runtime/src/protocol.cpp";
const WORKER: &str = "sdk/arkts/native-runtime/src/worker.cpp";
const API: &str = "sdk/arkts/native-gateway/src/api.cpp";
const RUNTIME: &str = "sdk/arkts/native-gateway/src/runtime.cpp";
const PORT: &str = "sdk/arkts/native-runtime/include/provider_port.hpp";
const PROVIDER: &str = "sdk/c/arkts-provider/src/provider.c";
const STATE: &str = "sdk/arkts/observation/StateObservation.ets";
const UI: &str = "sdk/arkts/observation/ArkUIStateModel.ets";

// These are reviewed mechanism versions. Re-pinning changed applicant sources
// cannot silently grant the same finite recipe a different runtime guarantee.
const SOURCE_PINS: &[(&str, &str)] = &[
    (
        "sdk/arkts/facade/Boundary.ets.in",
        "dd3775ec075333a79efaeb236b6dc8da46622fea22b43a6e021cf4cfe19faa15",
    ),
    (
        "sdk/arkts/facade/RuntimeFrame.ets",
        "a5eacf4b6b7aa7fdd886e1cc19ab3147517f96b60f19b4074df243de66e98492",
    ),
    (
        POLICY,
        "88729ffd4878a7aa991ce3a94f0edd3b3ec156751fbe5b12bca7cf050b2ced75",
    ),
    (
        CORE,
        "eb3978d2dda0e2ac1c67c485a4b74d66bf9b4a3155934ab895245fb23a0bcc25",
    ),
    (
        WORKER,
        "65c5d66f096688c808c9bdd041382227107afb94e29a1f74fcb3b7728cf6fd22",
    ),
    (
        API,
        "523a6a9966d0de617a245ac07b2a7190558bb7d5d757fb6f01a007fe33b53d7d",
    ),
    (
        RUNTIME,
        "2ef9479949934dd8e5d8f9f447f7983f9191162ba7f1e0ec5b782b53363dc87b",
    ),
    (
        "sdk/arkts/native-gateway/include/runtime.hpp",
        "018667b8f0066b4d56dc5659dfc308b6224bb6d2197a9677c8da19a5c934de54",
    ),
    (
        "sdk/arkts/native-gateway/include/codec.hpp",
        "5ef359714bc88fa67cc3e9facde832c23709689f22c7574b3dcdd7d7782781cf",
    ),
    (
        "sdk/arkts/native-gateway/include/handle.hpp",
        "dd31c74e72c28b9d7f3472f8e9a412c1f3d3fa8ce22675e720a0a8cbbd9c8aa4",
    ),
    (
        "sdk/arkts/native-runtime/include/protocol.hpp",
        "2cbd2e55884f5078f0c4fcdde18f6a26a1a308846581b88ad4db8726e12a4f8f",
    ),
    (
        "sdk/arkts/native-runtime/include/worker.hpp",
        "7d3cf854c5ff94eb03b985602bc274a777c77d348623e12e9cdc2985746ef9a1",
    ),
    (
        PORT,
        "b35a99585b30559b5b2261f2d5925badf69b531686a38123b6936f9195112e08",
    ),
    (
        "sdk/arkts/native-runtime/include/provider_abi.h",
        "39828fb09200a11e92d4ce3118f285cd5342afb2b01d49a3bc4e2a6ca44ec41d",
    ),
    (
        PROVIDER,
        "136969a8e0b8376ca4089dd0d2795b2a54681bf7f97f841a78bfcc8de28d322e",
    ),
    (
        "adapters/generators/arkts/lowering.py",
        "18e623e399952d16b6a304f3c4af5cf921a9a92ea1981e009848e641c8760875",
    ),
    (
        "adapters/generators/arkts/driver.py",
        "7124fbd88dd31060eed655bb6f919e661ff0ef6f46637b33ecfbd21ca3ca3323",
    ),
    (
        "sdk/arkts/facade/frame-layout.json",
        "ea14c35161fe8eb6e977a5f9a7a7f63956917a10a4ae1c07e9562718dc30be69",
    ),
    (
        "sdk/arkts/native-runtime/include/validate_snapshot.h",
        "62f29e469fab77b80b2eb015e9a66d65830c25e49bc3453d9ee91950be22cde7",
    ),
];
const OPTIONAL_PINS: &[(&str, &str)] = &[
    (
        STATE,
        "1e26f847c8e37d7c65b236188d86c6ee3ad6dcc53c033d4304049a7ab7531534",
    ),
    (
        UI,
        "7e62d7352ded16641e560f9230b65576d524caa8ac847dc65715e7e877ad77ef",
    ),
];
fn fail(path: &str, message: impl Into<String>) -> crate::FormatError {
    invalid("InputInvalid", path, message)
}
fn id(value: &str) -> Id {
    Id::new(value).expect("reviewed identity")
}
fn logical(value: &str) -> LogicalPath {
    LogicalPath::new(value).expect("reviewed path")
}
fn array<'a>(value: &'a Value, path: &str) -> Result<&'a Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| fail(path, "expected checked array"))
}
fn object<'a>(value: &'a Value, path: &str) -> Result<&'a serde_json::Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| fail(path, "expected checked object"))
}
fn pinned_source<'a>(
    inputs: &'a BoundCompositionInputs,
    name: &str,
    hash: &str,
) -> Result<&'a [u8]> {
    let digest = format!("sha256:{hash}");
    let qualified = format!("packages/{PACKAGE}/0.1.0/{name}");
    let find = |path: &str| {
        inputs
            .frozen_inputs
            .files()
            .find(|(f, _)| f.path.as_str() == path)
    };
    let (raw_ref, raw) =
        find(name).ok_or_else(|| fail(name, "finite semantic source is absent"))?;
    let (package_ref, packaged) =
        find(&qualified).ok_or_else(|| fail(name, "finite package semantic source is absent"))?;
    if raw != packaged
        || raw_ref.digest.as_str() != digest
        || package_ref.digest.as_str() != digest
        || Digest::of_bytes(raw).as_str() != digest
        || raw_ref.size != raw.len() as u64
        || package_ref.size != packaged.len() as u64
    {
        return Err(fail(name, "unsupported finite semantic source bytes"));
    }
    Ok(raw)
}

/// Derive this finite source recipe's requirements from checked IR and fixed
/// mechanisms. Applicant responsibility rows never define required coverage.
/// The result is read-only semantic evidence, not execution or certification.
pub fn c07_source_requirements(
    inputs: &BoundCompositionInputs,
    ir: &Document,
) -> Result<Vec<ResolvedResponsibility>> {
    let profile = inputs.profile().value();
    check_profile(profile)?;
    let mode = text(&profile["id"])?
        .strip_prefix("c07-c-arkts-source-semantics-")
        .and_then(|s| s.strip_suffix("-aarch64-ohos23"))
        .filter(|m| matches!(*m, "ordinary" | "enhanced" | "disabled"))
        .ok_or_else(|| fail("profile", "unsupported finite source semantics profile"))?;
    if inputs.composition().value()["recipe"]
        != format!("madb.local.c07-c-arkts/source-semantics-{mode}@2")
        || inputs.boundary_id() != "c-arkts"
    {
        return Err(fail("recipe", "finite source semantics recipe mismatch"));
    }
    if ir.kind() != DocumentKind::ContractIr {
        return Err(fail("contract", "expected ContractIR"));
    }
    check_composition_snapshot_binding(inputs, ir)?;
    let capability = crate::convert::composition_capability(inputs)?;
    crate::check_contract_with_capability(ir, &capability)
        .map_err(|e| fail("contract", e.to_string()))?;
    // This source recipe has no link/assembly step. A declared owner cannot
    // turn generated exports into proof of an actually assembled native image.
    if inputs
        .responsibilities()
        .iter()
        .any(|r| r.key.stage.as_str() == "physical-assembly")
    {
        return Err(fail(
            "responsibilities",
            "physical-assembly is outside the source recipe",
        ));
    }
    for (name, hash) in SOURCE_PINS {
        pinned_source(inputs, name, hash)?;
    }
    if mode == "enhanced" {
        for (name, hash) in OPTIONAL_PINS {
            pinned_source(inputs, name, hash)?;
        }
    }
    let policy_bytes = pinned_source(inputs, POLICY, SOURCE_PINS[2].1)?;
    let policy = strict_json(policy_bytes).map_err(|e| fail(POLICY, e.to_string()))?;
    check_shape_and_policy(ir.value(), &policy)?;
    let owners = Owners::checked(inputs, mode == "enhanced")?;
    let mut rows = Vec::new();
    for (name, call) in object(&policy["calls"], "calls")? {
        let subject = public(text(&call["public_id"])?)?;
        // The actual reader lowers declarations; the generator owns generated
        // native ABI glue, while the SDK facade owns ordinary call projection.
        push(
            &mut rows,
            &subject,
            "type-read",
            "call",
            &subject,
            "declaration",
            &owners.reader,
        );
        push(
            &mut rows,
            &subject,
            "abi-lowering",
            "call",
            &subject,
            "generated-native",
            &owners.generator,
        );
        push(
            &mut rows,
            &subject,
            "projection",
            "call",
            &subject,
            "facade",
            &owners.facade,
        );
        if call["kind"] == "async" {
            let (kind, anchor) = instance(name);
            push(
                &mut rows,
                &subject,
                "async-bridge",
                kind,
                &call_subject(&policy, anchor)?,
                "facade",
                &owners.facade,
            );
        }
    }
    // Runtime notification ownership is distinct from a business subscription.
    role(
        &mut rows,
        &policy,
        [
            "openRuntime",
            "dispatch",
            "runtime",
            "openRuntime",
            "env-notification",
        ],
        &owners.gateway,
    )?;
    role(
        &mut rows,
        &policy,
        [
            "Runtime.connect",
            "dispatch",
            "engine",
            "Runtime.connect",
            "worker",
        ],
        &owners.worker,
    )?;
    role(
        &mut rows,
        &policy,
        [
            "Engine.startOperation",
            "dispatch",
            "operation",
            "Engine.startOperation",
            "worker",
        ],
        &owners.worker,
    )?;
    role(
        &mut rows,
        &policy,
        [
            "Operation.requestCancel",
            "cancel",
            "operation",
            "Engine.startOperation",
            "source",
        ],
        &owners.core,
    )?;
    role(
        &mut rows,
        &policy,
        [
            "Engine.requestClose",
            "stop",
            "engine",
            "Runtime.connect",
            "source",
        ],
        &owners.core,
    )?;
    for (observe, publish) in [
        ("Engine.observeState", "Engine.publishState"),
        ("Engine.observeEvents", "Engine.publishEvent"),
    ] {
        role(
            &mut rows,
            &policy,
            [observe, "subscription", "subscription", observe, "source"],
            &owners.core,
        )?;
        role(
            &mut rows,
            &policy,
            [publish, "dispatch", "subscription", observe, "source-queue"],
            &owners.core,
        )?;
        role(
            &mut rows,
            &policy,
            [
                observe,
                "dispatch",
                "subscription",
                observe,
                "native-callback",
            ],
            &owners.gateway,
        )?;
        role(
            &mut rows,
            &policy,
            [observe, "dispatch", "subscription", observe, "js-listener"],
            &owners.facade,
        )?;
        role(
            &mut rows,
            &policy,
            [
                "Subscription.requestStop",
                "stop",
                "subscription",
                observe,
                "source",
            ],
            &owners.core,
        )?;
        role(
            &mut rows,
            &policy,
            [
                "Subscription.close",
                "drain",
                "subscription",
                observe,
                "source",
            ],
            &owners.core,
        )?;
        role(
            &mut rows,
            &policy,
            [
                "Subscription.close",
                "release",
                "subscription",
                observe,
                "native-context",
            ],
            &owners.gateway,
        )?;
        role(
            &mut rows,
            &policy,
            [
                "Subscription.close",
                "release",
                "subscription",
                observe,
                "js-listener",
            ],
            &owners.facade,
        )?;
    }
    for (close, kind, anchor) in [
        ("Engine.close", "engine", "Runtime.connect"),
        ("Operation.close", "operation", "Engine.startOperation"),
    ] {
        role(
            &mut rows,
            &policy,
            [close, "drain", kind, anchor, "source"],
            &owners.core,
        )?;
        role(
            &mut rows,
            &policy,
            [close, "release", kind, anchor, "public-record"],
            &owners.facade,
        )?;
    }
    for (name, layer) in [
        ("Engine.roundtrip", "scratch"),
        ("Operation.result", "result-payload"),
    ] {
        let (kind, anchor) = instance(name);
        role(
            &mut rows,
            &policy,
            [name, "release", kind, anchor, layer],
            &owners.provider,
        )?;
    }
    role(
        &mut rows,
        &policy,
        [
            "Operation.result",
            "release",
            "operation",
            "Engine.startOperation",
            "result-ticket",
        ],
        &owners.core,
    )?;
    // Payload::reset pairs provider destruction with Core ticket retirement.
    // Work input, queued publications and unclaimed operation results have
    // distinct origins; a Core drain row cannot stand in for provider release.
    for (name, kind, anchor, payload, ticket) in [
        (
            "Engine.startOperation",
            "operation",
            "Engine.startOperation",
            "input-payload",
            "input-ticket",
        ),
        (
            "Engine.publishState",
            "subscription",
            "Engine.observeState",
            "source-payload",
            "source-ticket",
        ),
        (
            "Engine.publishEvent",
            "subscription",
            "Engine.observeEvents",
            "source-payload",
            "source-ticket",
        ),
        (
            "Operation.close",
            "operation",
            "Engine.startOperation",
            "residual-payload",
            "residual-ticket",
        ),
    ] {
        role(
            &mut rows,
            &policy,
            [name, "release", kind, anchor, payload],
            &owners.provider,
        )?;
        role(
            &mut rows,
            &policy,
            [name, "release", kind, anchor, ticket],
            &owners.core,
        )?;
    }
    for (stage, layer, owner) in [
        ("stop", "child-admission", &owners.gateway),
        ("drain", "physical-worker", &owners.worker),
        ("drain", "env-notification", &owners.gateway),
        ("release", "env-reference", &owners.gateway),
        ("release", "provider-context", &owners.provider),
        ("release", "runtime-receipt", &owners.facade),
    ] {
        role(
            &mut rows,
            &policy,
            ["Runtime.close", stage, "runtime", "openRuntime", layer],
            owner,
        )?;
    }
    if mode == "enhanced" {
        optional_roles(&mut rows, &owners)?;
    }
    let unique = rows.iter().map(|r| &r.key).collect::<BTreeSet<_>>();
    if unique.len() != rows.len() {
        return Err(fail("required", "duplicate finite requirement"));
    }
    Ok(rows)
}

fn public(value: &str) -> Result<ResolvedSubject> {
    PublicId::new(value)
        .map(ResolvedSubject::PublicSymbol)
        .map_err(|e| fail("public_id", e.to_string()))
}
fn call_subject(policy: &Value, name: &str) -> Result<ResolvedSubject> {
    public(text(&policy["calls"][name]["public_id"])?)
}
fn instance(name: &str) -> (&'static str, &'static str) {
    match name {
        "Runtime.close" | "openRuntime" => ("runtime", "openRuntime"),
        "Operation.result"
        | "Operation.requestCancel"
        | "Operation.close"
        | "Engine.startOperation" => ("operation", "Engine.startOperation"),
        // Shared Subscription methods serve either real source. The common
        // Promise bridge is an object slot; source-drain rows below distinguish
        // state and event subscription anchors explicitly.
        "Subscription.close" | "Subscription.requestStop" => ("subscription", "Subscription.close"),
        "Engine.roundtrip" => ("call", "Engine.roundtrip"),
        _ => ("engine", "Runtime.connect"),
    }
}
fn push(
    rows: &mut Vec<ResolvedResponsibility>,
    subject: &ResolvedSubject,
    stage: &str,
    kind: &str,
    anchor: &ResolvedSubject,
    layer: &str,
    owner: &ResolvedOwner,
) {
    rows.push(ResolvedResponsibility {
        key: ResponsibilityKey {
            boundary: id("c-arkts"),
            subject: subject.clone(),
            stage: id(stage),
            scope: ResolvedScope {
                kind: id(kind),
                anchor: anchor.clone(),
                layer: id(layer),
            },
        },
        owner: owner.clone(),
    });
}
fn role(
    rows: &mut Vec<ResolvedResponsibility>,
    policy: &Value,
    [name, stage, kind, anchor, layer]: [&str; 5],
    owner: &ResolvedOwner,
) -> Result<()> {
    push(
        rows,
        &call_subject(policy, name)?,
        stage,
        kind,
        &call_subject(policy, anchor)?,
        layer,
        owner,
    );
    Ok(())
}

struct Owners {
    reader: ResolvedOwner,
    generator: ResolvedOwner,
    facade: ResolvedOwner,
    core: ResolvedOwner,
    worker: ResolvedOwner,
    gateway: ResolvedOwner,
    provider: ResolvedOwner,
    optional: BTreeMap<&'static str, AssetOrigin>,
}
impl Owners {
    fn checked(inputs: &BoundCompositionInputs, enhanced: bool) -> Result<Self> {
        let c = inputs.composition().value();
        let packages = array(&c["packages"], "packages")?;
        if packages.len() != 1 || packages[0]["id"] != PACKAGE || packages[0]["version"] != "0.1.0"
        {
            return Err(fail(
                "packages",
                "unsupported finite source package closure",
            ));
        }
        let package = Digest::new(text(&packages[0]["source_digest"])?)
            .map_err(|e| fail("source_digest", e.to_string()))?;
        if !enhanced {
            let optional: BTreeSet<_> = OPTIONAL_PINS
                .iter()
                .map(|(_, h)| format!("sha256:{h}"))
                .collect();
            for (reference, _) in inputs.frozen_inputs.files() {
                if [STATE, UI]
                    .iter()
                    .any(|name| reference.path.as_str().ends_with(name))
                    || optional.contains(reference.digest.as_str())
                {
                    return Err(fail(
                        "assets",
                        "disabled source recipe contains optional SDK bytes",
                    ));
                }
            }
        }
        let mut assets = BTreeMap::new();
        for asset in array(&c["assets"], "assets")? {
            if asset["package"] != PACKAGE || assets.insert(text(&asset["id"])?, asset).is_some() {
                return Err(fail(
                    "assets",
                    "finite source asset package or identity differs",
                ));
            }
        }
        let mut expected = BTreeSet::from([
            "facade",
            "native-runtime",
            "native-worker",
            "native-gateway",
            "native-provider",
        ]);
        if enhanced {
            expected.extend(["state", "arkui"]);
        }
        if assets.keys().copied().collect::<BTreeSet<_>>() != expected {
            return Err(fail("assets", "finite source asset closure differs"));
        }
        let source = |name: &str, expected_paths: &[&str]| -> Result<AssetOrigin> {
            let a = assets[name];
            if a["source"]["kind"] != "source" {
                return Err(fail(name, "expected fixed source asset"));
            }
            let paths = array(&a["source"]["files"], name)?
                .iter()
                .map(|f| text(&f["path"]))
                .collect::<Result<BTreeSet<_>>>()?;
            if paths != expected_paths.iter().copied().collect::<BTreeSet<_>>() {
                return Err(fail(name, "fixed source asset paths differ"));
            }
            // bind_composition_inputs already checked every exact package
            // FileRef; pinned_source above additionally fixes reviewed bytes.
            Ok(AssetOrigin::Source {
                package: package.clone(),
                paths: paths.into_iter().map(logical).collect(),
            })
        };
        let facade = assets["facade"];
        if facade["source"]["kind"] != "generated"
            || facade["source"]["producer"] != format!("{PACKAGE}/generator")
            || facade["source"]["output_slot"] != "ordinary-source"
        {
            return Err(fail("facade", "fixed generated source slot differs"));
        }
        let mut paths = array(&facade["source"]["paths"], "facade")?
            .iter()
            .map(|p| text(p).map(logical))
            .collect::<Result<Vec<_>>>()?;
        paths.sort();
        let component = |name: &str| ResolvedOwner::Component {
            id: id(&format!("{PACKAGE}/{name}")),
            package: package.clone(),
        };
        let mut optional = BTreeMap::new();
        if enhanced {
            optional.insert("state", source("state", &[STATE])?);
            optional.insert("arkui", source("arkui", &[UI])?);
        }
        Ok(Self {
            reader: component("reader"),
            generator: component("generator"),
            facade: ResolvedOwner::SdkAsset(AssetOrigin::Generated {
                package: package.clone(),
                producer: id(&format!("{PACKAGE}/generator")),
                slot: id("ordinary-source"),
                paths,
            }),
            core: ResolvedOwner::SdkAsset(source("native-runtime", &[CORE])?),
            worker: ResolvedOwner::SdkAsset(source("native-worker", &[WORKER])?),
            gateway: ResolvedOwner::SdkAsset(source("native-gateway", &[API, RUNTIME, PORT])?),
            provider: ResolvedOwner::SdkAsset(source("native-provider", &[PROVIDER])?),
            optional,
        })
    }
}
fn optional_roles(rows: &mut Vec<ResolvedResponsibility>, owners: &Owners) -> Result<()> {
    for (asset, class) in [("state", "StateObservation"), ("arkui", "ArkUIStateModel")] {
        let origin = owners
            .optional
            .get(asset)
            .ok_or_else(|| fail(asset, "optional source owner absent"))?;
        let owner = ResolvedOwner::SdkAsset(origin.clone());
        let subject = |member: &str| ResolvedSubject::SdkExport {
            origin: origin.clone(),
            member: id(&format!("{class}.{member}")),
        };
        let anchor = subject("attach");
        let layer = if asset == "state" {
            "receiver"
        } else {
            "view-model"
        };
        // These wrappers consume the existing source subscription. They never
        // claim another native subscription, executor, or Core drain owner.
        let members: &[&str] = if asset == "arkui" {
            &["attach", "phase", "closeFailure", "snapshot"]
        } else {
            &["attach", "phase", "closeFailure"]
        };
        for member in members {
            push(
                rows,
                &subject(member),
                "projection",
                "receiver",
                &anchor,
                layer,
                &owner,
            );
        }
        push(
            rows,
            &subject("requestStop"),
            "stop",
            "receiver",
            &anchor,
            layer,
            &owner,
        );
        push(
            rows,
            &subject("close"),
            "async-bridge",
            "receiver",
            &anchor,
            layer,
            &owner,
        );
        if asset == "state" {
            push(
                rows,
                &subject("close"),
                "release",
                "receiver",
                &anchor,
                "listener-reference",
                &owner,
            );
        }
    }
    Ok(())
}

fn mapped_type(
    t: &Value,
    context: &str,
    types: &mut BTreeMap<String, Value>,
    error: &str,
) -> Result<String> {
    match text(&t["tag"])? {
        "void" => Ok("unit".into()),
        "reference" if types.contains_key(text(&t["name"])?) => Ok(text(&t["name"])?.into()),
        "callback" => {
            let name = format!("{context}.callback");
            let mut parameters = Vec::new();
            for p in array(&t["params"], context)? {
                let p_name = text(&p["name"])?;
                let type_id =
                    mapped_type(&p["type"], &format!("{context}.{p_name}"), types, error)?;
                parameters.push(json!({"name":p_name,"type_id":type_id}));
            }
            let result = mapped_type(&t["returns"], &format!("{context}.return"), types, error)?;
            types.insert(name.clone(), json!({"kind":"callback","parameters":parameters,"result":result,"calling_convention":"system","throws":error}));
            Ok(name)
        }
        _ => Err(fail(context, "unsupported finite public type")),
    }
}
fn check_shape_and_policy(ir: &Value, policy: &Value) -> Result<()> {
    let surface = &policy["public_surface"];
    let error = text(&policy["error_channel"]["id"])?;
    let mut types = BTreeMap::from([("unit".to_owned(), json!({"kind":"unit"}))]);
    for resource in array(&surface["resources"], "resources")? {
        types.insert(
            text(&resource["name"])?.into(),
            json!({"kind":"resource","representation":"backend-handle","owner":"madb.c07.c-arkts"}),
        );
    }
    types.insert(
        error.into(),
        json!({"kind":"resource","representation":"backend-handle","owner":"arkts-runtime"}),
    );
    for record in array(&surface["records"], "records")? {
        let name = text(&record["name"])?;
        let mut fields = Vec::new();
        for field in array(&record["fields"], name)? {
            let field_name = text(&field["name"])?;
            let mapping = &policy["values"][format!("{name}.{field_name}")];
            let type_id = text(&mapping["type_id"])?;
            types.insert(type_id.into(), mapping["definition"].clone());
            fields.push(json!({"name":field_name,"type_id":type_id}));
        }
        types.insert(
            name.into(),
            json!({"kind":"record","layout":"logical","fields":fields}),
        );
    }
    let mut calls: Vec<&Value> = array(&surface["functions"], "functions")?.iter().collect();
    for resource in array(&surface["resources"], "resources")? {
        calls.extend(array(&resource["methods"], "methods")?.iter());
    }
    let actual = array(&ir["symbols"], "symbols")?
        .iter()
        .map(|s| Ok((text(&s["public_id"])?, s)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    if calls.len() != actual.len() {
        return Err(fail("symbols", "finite public call closure differs"));
    }
    let mut expected_policies = BTreeMap::new();
    for call in calls {
        let method = text(&call["name"])?;
        let name = if call["owner"].is_null() {
            method.into()
        } else {
            format!("{}.{method}", text(&call["owner"])?)
        };
        let role = &policy["calls"][&name];
        let public_id = text(&role["public_id"])?;
        let symbol = actual
            .get(public_id)
            .ok_or_else(|| fail(public_id, "finite public symbol is absent"))?;
        if symbol["precision"] != "compiler-derived"
            || symbol["source_key"]["qualified_name"] != name
        {
            return Err(fail(
                public_id,
                "finite declaration precision or identity differs",
            ));
        }
        let mut parameters = Vec::new();
        if let Some(owner) = call["owner"].as_str() {
            parameters.push(json!({"name":"self","type_id":owner}));
        }
        for p in array(&call["params"], &name)? {
            let p_name = text(&p["name"])?;
            let type_id = mapped_type(&p["type"], &format!("{name}.{p_name}"), &mut types, error)?;
            parameters.push(json!({"name":p_name,"type_id":type_id}));
        }
        let returned = if call["returns"]["tag"] == "promise" {
            &call["returns"]["item"]
        } else {
            &call["returns"]
        };
        let result = mapped_type(returned, &format!("{name}.return"), &mut types, error)?;
        let shape = json!({"kind":role["kind"],"calling_convention":"system","link_name":name,"parameters":parameters,"result":result,"throws":error,"constants":[]});
        if symbol["shape"] != shape {
            return Err(fail(
                public_id,
                "finite public shape differs from reviewed declarations",
            ));
        }
        expected_policies.insert(
            (public_id.to_owned(), "call".to_owned()),
            role["policy"].clone(),
        );
        let slot_values = |type_id: &str| {
            let kind = types[type_id]["kind"].as_str().expect("reviewed type");
            &policy["slots"][if kind == "resource" {
                "resource"
            } else if kind == "callback" {
                "callback"
            } else {
                "value"
            }]
        };
        expected_policies.insert(
            (public_id.to_owned(), "return".to_owned()),
            slot_values(&result).clone(),
        );
        for p in array(&shape["parameters"], public_id)? {
            expected_policies.insert(
                (
                    public_id.to_owned(),
                    format!("parameter.{}", text(&p["name"])?),
                ),
                slot_values(text(&p["type_id"])?).clone(),
            );
        }
    }
    let actual_types = array(&ir["types"], "types")?
        .iter()
        .map(|t| Ok((text(&t["id"])?.to_owned(), t["definition"].clone())))
        .collect::<Result<BTreeMap<_, _>>>()?;
    if actual_types != types {
        return Err(fail("types", "finite type and ownership closure differs"));
    }
    let mut actual_policies = BTreeMap::new();
    for p in array(&ir["policies"], "policies")? {
        let kind = text(&p["slot"]["kind"])?;
        let slot = if kind == "parameter" {
            format!("parameter.{}", text(&p["slot"]["name"])?)
        } else {
            kind.into()
        };
        if actual_policies
            .insert(
                (text(&p["symbol_id"])?.to_owned(), slot),
                p["values"].clone(),
            )
            .is_some()
        {
            return Err(fail("policies", "duplicate finite capability premise"));
        }
    }
    if actual_policies != expected_policies {
        return Err(invalid(
            "CapabilityUnsupported",
            "policies",
            "finite source ownership, threading, cancellation, delivery or stream premise differs",
        ));
    }
    Ok(())
}

// This is the capability tuple of this finite source recipe, not a claim that
// a source run built a HAR or executed on the target. Unsupported tuple changes
// require a separately reviewed recipe; self-reported features cannot widen it.
fn check_profile(profile: &Value) -> Result<()> {
    for (key, expected) in [
        ("provider", json!({"language":"c","runtime":"native"})),
        (
            "consumer",
            json!({"language":"arkts","runtime":"harmonyos"}),
        ),
        ("route", json!("ARKTS-C-NAPI")),
        ("direction", json!("consumer-to-provider")),
        ("target", json!("aarch64-linux-ohos")),
        ("abi", json!("harmonyos-napi8-c07-snapshot")),
        ("artifact_kind", json!("har")),
        ("build_system", json!("clang-hvigor")),
        ("deployment_target", json!("API23")),
        ("macros", json!([])),
        ("restrictions", json!([])),
        ("backend_config", Value::Null),
    ] {
        if profile[key] != expected {
            return Err(invalid(
                "CapabilityUnsupported",
                key,
                "unsupported finite source capability tuple",
            ));
        }
    }
    let expected = BTreeSet::from([
        "core.async@1",
        "core.backend-resources@1",
        "core.callbacks@1",
        "core.errors@1",
        "core.exact-numbers@1",
        "core.explicit-close@1",
        "core.extension-composition@1",
        "core.external-runtime-verification@1",
        "core.resources@1",
        "core.streams@1",
        "core.values@1",
    ]);
    for key in ["required_features", "supported_features"] {
        let actual = array(&profile[key], key)?
            .iter()
            .map(text)
            .collect::<Result<BTreeSet<_>>>()?;
        if actual != expected {
            return Err(invalid(
                "CapabilityUnsupported",
                key,
                "finite source feature closure differs",
            ));
        }
    }
    Ok(())
}
