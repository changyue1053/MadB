//! Finite C-to-Rust responsibility evidence. This is not a composition wire format.
use crate::{Document, DocumentKind};
use madb_app::{
    Digest, ErrorKind, LogicalPath, QueryError, Result, inputs::FrozenInputs, ports::FileReference,
};
use madb_protocol::{Value, strict_json};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

const READER: &str = "org.madb.rust-c/reader-generator";
const NATIVE: &str = "madb-first-route-engine";
const VERSION: &str = "0.1.0";
const SDK_PREFIX: &str = "sdk/rust/engine/";
const SOURCE: &str = "sdk/rust/engine/src/lib.rs";
const PROTOCOL: &str = "sdk/rust/engine/runtime-protocol.json";
const BASE_ROLES: &[&str] = &[
    "bootstrap",
    "create",
    "checksum",
    "request_close",
    "await_closed",
    "release_handle",
    "observe",
];
const DIAGNOSTIC_ROLES: &[&str] = &["diagnostics_configure", "diagnostics_drain"];

pub(crate) struct FirstRouteOwnership {
    pub(crate) bytes: Vec<u8>,
    pub(crate) reference: FileReference,
}

fn invalid(message: impl Into<String>) -> QueryError {
    QueryError::new(
        ErrorKind::InvalidInput,
        "NATIVE006",
        "responsibilities",
        message,
    )
}
fn string(value: &Value) -> Result<&str> {
    value
        .as_str()
        .ok_or_else(|| invalid("required responsibility source string is absent"))
}
fn array(value: &Value) -> Result<&Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| invalid("required responsibility source array is absent"))
}
fn reference(value: &Value) -> Result<FileReference> {
    Ok(FileReference {
        path: LogicalPath::new(string(&value["path"])?).map_err(|e| invalid(e.to_string()))?,
        digest: Digest::new(string(&value["sha256"])?).map_err(|e| invalid(e.to_string()))?,
        size: value["size"]
            .as_u64()
            .ok_or_else(|| invalid("source size is absent"))?,
    })
}
fn bytes<'a>(inputs: &'a FrozenInputs, expected: &FileReference) -> Result<&'a [u8]> {
    let (actual, data) = inputs
        .files()
        .find(|(r, _)| r.path == expected.path)
        .ok_or_else(|| {
            invalid(format!(
                "required frozen source missing: {}",
                expected.path.as_str()
            ))
        })?;
    if actual != expected
        || Digest::of_bytes(data) != expected.digest
        || data.len() as u64 != expected.size
    {
        return Err(invalid(format!(
            "frozen source differs: {}",
            expected.path.as_str()
        )));
    }
    Ok(data)
}
fn decode(data: &[u8]) -> Result<Value> {
    strict_json(data).map_err(|e| invalid(e.to_string()))
}
fn tracked<'a>(
    lock: &Value,
    inputs: &'a FrozenInputs,
    name: &str,
) -> Result<(&'a [u8], FileReference)> {
    let matching: Vec<_> = array(&lock["inputs"])?
        .iter()
        .filter(|r| r["path"] == name)
        .collect();
    if matching.len() != 1 {
        return Err(invalid(format!(
            "source must be locked exactly once: {name}"
        )));
    }
    let file = reference(matching[0])?;
    Ok((bytes(inputs, &file)?, file))
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Key {
    subject: String,
    stage: &'static str,
    scope: &'static str,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Owner {
    label: String,
    source: Digest,
}
type Rows = Vec<(Key, Owner)>;
struct Owners {
    reader: Owner,
    native: Owner,
    helper: Owner,
}
fn row(subject: &str, stage: &'static str, scope: &'static str, owner: &Owner) -> (Key, Owner) {
    (
        Key {
            subject: subject.into(),
            stage,
            scope,
        },
        owner.clone(),
    )
}
fn common_rows(subject: &str, owners: &Owners) -> Rows {
    vec![
        row(subject, "type-read", "c-declaration", &owners.reader),
        row(subject, "abi-lowering", "c-to-logical", &owners.reader),
        row(subject, "projection", "call-frame", &owners.native),
    ]
}
fn helper_row(owners: &Owners) -> (Key, Owner) {
    row(
        "helper:madb_engine_create_checked",
        "projection",
        "identity-gate",
        &owners.helper,
    )
}
// This list comes from the finite ABI recipe, never from planned rows.
fn requirements(roles: &BTreeMap<String, String>, owners: &Owners) -> Rows {
    let mut rows: Rows = roles
        .values()
        .flat_map(|id| common_rows(id, owners))
        .collect();
    for (role, stage, scope) in [
        ("observe", "subscription", "returned-subscription"),
        ("observe", "dispatch", "source-lease"),
        ("request_close", "stop", "argument-handle"),
        ("await_closed", "drain", "argument-handle"),
        ("release_handle", "release", "closed-token"),
    ] {
        rows.push(row(&roles[role], stage, scope, &owners.native));
    }
    rows.push(helper_row(owners));
    rows
}
fn role_shape(role: &str) -> (&'static [&'static str], &'static str) {
    match role {
        "bootstrap" => (&["u32"], "identity_bytes"),
        "create" => (&[], "handle"),
        "checksum" => (&["handle", "bytes"], "u64"),
        "request_close" | "release_handle" => (&["handle"], "unit"),
        "await_closed" => (&["handle", "u32"], "unit"),
        "observe" => (&["handle", "state_callback"], "handle"),
        "diagnostics_configure" => (&["u32"], "unit"),
        "diagnostics_drain" => (&[], "diagnostic_pull"),
        _ => unreachable!("roles validated before planning"),
    }
}
fn check_types(ir: &Value, diagnostics: bool) -> Result<()> {
    let mut actual = BTreeMap::new();
    for ty in array(&ir["types"])? {
        if actual
            .insert(string(&ty["id"])?, ty["definition"].clone())
            .is_some()
        {
            return Err(invalid("duplicate finite native type"));
        }
    }
    let mut expected = BTreeMap::from([("unit", json!({"kind":"unit"}))]);
    for (name, width, signed) in [
        ("u8", 8, false),
        ("u32", 32, false),
        ("u64", 64, false),
        ("i32", 32, true),
    ] {
        expected.insert(
            name,
            json!({"kind":"integer","width":width,"signed":signed}),
        );
    }
    for (name, maximum) in [("bytes", 65536), ("identity_bytes", 64)] {
        expected.insert(name, json!({"kind":"buffer","element":"u8","length_unit":"bytes","encoding":"none","mutable":false,"max_bytes":maximum}));
    }
    expected.insert("error", json!({"kind":"record","layout":"c","fields":[{"name":"domain","type_id":"u32"},{"name":"code","type_id":"i32"}]}));
    let handle = actual
        .get("handle")
        .ok_or_else(|| invalid("finite native handle type absent"))?;
    // The existing checked contract validates the project-specific owner Id.
    // Resource representation and callback ABI are fixed by this recipe.
    expected.insert("handle", json!({"kind":"resource","representation":"registry-u64","owner":string(&handle["owner"])?}));
    expected.insert("state_callback", json!({"kind":"callback","parameters":[{"name":"sequence","type_id":"u64"},{"name":"state","type_id":"u32"}],"result":"unit","calling_convention":"c"}));
    if diagnostics {
        expected.insert("i64", json!({"kind":"integer","width":64,"signed":true}));
        for (id, fields) in [
            (
                "diagnostic_event",
                vec![
                    ("record_id", "u64"),
                    ("symbol_id", "u64"),
                    ("phase", "u64"),
                    ("resource_id", "u64"),
                    ("related_resource_id", "u64"),
                    ("duration_ns", "u64"),
                    ("status", "i64"),
                    ("status_present", "u64"),
                ],
            ),
            (
                "diagnostic_stats",
                vec![
                    ("completed_calls", "u64"),
                    ("failed_calls", "u64"),
                    ("completed_callbacks", "u64"),
                    ("dropped_events", "u64"),
                    ("capture_enabled", "u64"),
                ],
            ),
        ] {
            let fields: Vec<_> = fields
                .into_iter()
                .map(|(name, ty)| json!({"name":name,"type_id":ty}))
                .collect();
            expected.insert(id, json!({"kind":"record","layout":"c","fields":fields}));
        }
        expected.insert("diagnostic_pull", json!({"kind":"record","layout":"logical","fields":[{"name":"present","type_id":"u64"},{"name":"event","type_id":"diagnostic_event"},{"name":"stats","type_id":"diagnostic_stats"}]}));
    }
    if actual != expected {
        return Err(invalid(
            "logical native types differ from the finite recipe",
        ));
    }
    Ok(())
}
fn slot_text(slot: &Value) -> Result<String> {
    match string(&slot["kind"])? {
        "parameter" => Ok(format!("parameter:{}", string(&slot["name"])?)),
        "call" => Ok("call".into()),
        "return" => Ok("return".into()),
        _ => Err(invalid("unknown effective policy slot")),
    }
}
fn policy_index(ir: &Value) -> Result<BTreeMap<(String, String), Value>> {
    let mut result = BTreeMap::new();
    for policy in array(&ir["policies"])? {
        let key = (
            string(&policy["symbol_id"])?.into(),
            slot_text(&policy["slot"])?,
        );
        if result.insert(key, policy["values"].clone()).is_some() {
            return Err(invalid("duplicate effective policy slot"));
        }
    }
    Ok(result)
}
fn expected_policies(
    subject: &str,
    role: &str,
    shape: &Value,
    releaser: &str,
) -> Result<BTreeMap<(String, String), Value>> {
    let mut result = BTreeMap::new();
    let mut add = |slot: String, values: Value| {
        result.insert((subject.into(), slot), values);
    };
    let blocking = match role {
        "observe" => "unbounded",
        "request_close" | "diagnostics_configure" | "diagnostics_drain" => "non-blocking",
        _ => "bounded",
    };
    add(
        "call".into(),
        json!({"threading":"caller","blocking":blocking,
        "delivery":if role == "observe" {"registered"} else {"work-completed"},
        "cancellation":"none","cancellation_confirmation":"not-supported"}),
    );
    match string(&shape["result"])? {
        "unit" => {}
        "handle" => add(
            "return".into(),
            json!({"ownership":"transfer","releaser":releaser,"close":"request-await-release","release_thread":"caller"}),
        ),
        _ => add("return".into(), json!({"ownership":"copy"})),
    }
    for parameter in array(&shape["parameters"])? {
        let values = match string(&parameter["type_id"])? {
            "handle" => {
                json!({"ownership":"borrow","borrow_scope":"call","keep_alive":"call-frame","close":"request-await-release","release_thread":"caller"})
            }
            "bytes" => {
                json!({"ownership":"borrow","borrow_scope":"call","keep_alive":"call-frame"})
            }
            "state_callback" => {
                json!({"ownership":"borrow","borrow_scope":"subscription","keep_alive":"source-lease","synchronous_callback":true,"reentrant":true,"threading":"serial-executor","context_owner":"caller","stop":"request-and-drain"})
            }
            _ => json!({"ownership":"copy"}),
        };
        add(format!("parameter:{}", string(&parameter["name"])?), values);
    }
    Ok(result)
}
fn plan(
    ir: &Value,
    exports: &[Value],
    roles: &BTreeMap<String, String>,
    owners: &Owners,
) -> Result<Rows> {
    check_types(ir, roles.contains_key("diagnostics_drain"))?;
    let actual_policies = policy_index(ir)?;
    let mut expected = BTreeMap::new();
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for symbol in array(&ir["symbols"])? {
        let subject = string(&symbol["public_id"])?;
        if !seen.insert(subject) {
            return Err(invalid("duplicate native symbol"));
        }
        let export = exports
            .iter()
            .find(|e| e["public_id"] == subject)
            .ok_or_else(|| invalid("symbol is outside the finite ABI role mapping"))?;
        let role = string(&export["role"])?;
        let shape = &symbol["shape"];
        let (parameters, result) = role_shape(role);
        let actual: Vec<_> = array(&shape["parameters"])?
            .iter()
            .map(|p| string(&p["type_id"]))
            .collect::<Result<_>>()?;
        if shape["kind"] != "sync"
            || shape["calling_convention"] != "c"
            || shape["link_name"] != export["name"]
            || actual != parameters
            || shape["result"] != result
        {
            return Err(invalid(format!("native role shape differs: {role}")));
        }
        expected.extend(expected_policies(
            subject,
            role,
            shape,
            &roles["release_handle"],
        )?);
        rows.extend(common_rows(subject, owners));
        // Planned mechanism steps follow actual role/policy lowering, separately
        // from the required coverage list above.
        match role {
            "observe" => {
                rows.push(row(
                    subject,
                    "subscription",
                    "returned-subscription",
                    &owners.native,
                ));
                rows.push(row(subject, "dispatch", "source-lease", &owners.native));
            }
            "request_close" => rows.push(row(subject, "stop", "argument-handle", &owners.native)),
            "await_closed" => rows.push(row(subject, "drain", "argument-handle", &owners.native)),
            "release_handle" => rows.push(row(subject, "release", "closed-token", &owners.native)),
            _ => {}
        }
    }
    if seen.len() != roles.len() || expected != actual_policies {
        return Err(invalid(
            "effective policies or symbols differ from the finite native recipe",
        ));
    }
    rows.push(helper_row(owners));
    madb_rules::compare_unique_owners(&requirements(roles, owners), &rows)
        .map_err(|e| invalid(format!("native responsibility coverage differs: {e:?}")))?;
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(rows)
}

/// Called only from the existing checked /1 generation context with actual bytes.
/// Non-SDK readers do not inherit this finite SDK's mechanism guarantees.
pub(crate) fn first_route_ownership(
    profile: &Document,
    lock: &Document,
    request: &Document,
    contract: &Document,
    provided: &FrozenInputs,
) -> Result<Option<FirstRouteOwnership>> {
    if profile.kind() != DocumentKind::Profile
        || lock.kind() != DocumentKind::Lock
        || request.kind() != DocumentKind::GenerateRequest
        || contract.kind() != DocumentKind::ContractIr
    {
        return Err(invalid(
            "responsibility context requires existing /1 generation documents",
        ));
    }
    let p = profile.value();
    if request.value()["component"]["id"] != READER {
        return Ok(None);
    }
    let l = lock.value();
    let (backend, _) = tracked(l, provided, "backend.json")?;
    let config = decode(backend)?;
    if config["source"] != SOURCE
        || config["provider"] != "rust"
        || config["semantics"] != "madb.first-route/1"
    {
        return Ok(None);
    }
    if config["schema"] != "madb.rust-c-backend/1"
        || p["route"] != "RUST-C-ABI"
        || p["provider"]["language"] != "rust"
        || p["consumer"]["language"] != "c"
        || p["direction"] != "consumer-to-provider"
        || p["provider"]["runtime"] != "native"
        || p["consumer"]["runtime"] != "native"
    {
        return Err(invalid("fixed native SDK profile or backend differs"));
    }
    if config.get("test_support").is_some_and(|v| !v.is_boolean()) {
        return Err(invalid("test_support must be a boolean"));
    }
    let diagnostics = match config.get("diagnostics") {
        None => false,
        Some(Value::Bool(value)) => *value,
        _ => return Err(invalid("diagnostics must be a boolean")),
    };
    if config["cbindgen_config"]
        != if diagnostics {
            "sdk/rust/engine/cbindgen-diagnostics.toml"
        } else {
            "sdk/rust/engine/cbindgen.toml"
        }
    {
        return Err(invalid(
            "selected cbindgen config differs from the finite SDK recipe",
        ));
    }
    let q = request.value();
    let ir = contract.value();
    if q["selector"] != ir["selector"]
        || q["boundary_id"] != ir["boundary_id"]
        || q["input_digest"] != ir["source_digest"]
        || q["contract_digest"] != ir["structure_digest"]
        || q["policy_digest"] != ir["policy_digest"]
    {
        return Err(invalid(
            "native responsibility contract/request lineage differs",
        ));
    }
    if p["id"] != q["selector"]["profile_id"]
        || p["target"] != q["selector"]["target"]
        || p["route"] != q["selector"]["route"]
        || p["direction"] != q["selector"]["direction"]
    {
        return Err(invalid("native responsibility profile/selector differs"));
    }
    let component = &q["component"];
    if component["id"] != READER
        || component["version"] != VERSION
        || !array(&l["components"])?.contains(component)
    {
        return Err(invalid(
            "native reader/generator is not the exact locked component",
        ));
    }
    if crate::source_digest(&q["inputs"]).map_err(|e| invalid(e.to_string()))?
        != crate::source_digest(&l["inputs"]).map_err(|e| invalid(e.to_string()))?
    {
        return Err(invalid("native request source inventory differs from lock"));
    }
    if p["sdk"]["schema"] != "madb.native-toolchain/1" {
        return Err(invalid(
            "profile SDK schema differs from the finite native recipe",
        ));
    }
    let sdk_file = reference(&p["sdk"])?;
    let (sdk_bytes, locked_sdk) = tracked(l, provided, sdk_file.path.as_str())?;
    if sdk_file != locked_sdk {
        return Err(invalid("profile SDK reference differs from locked bytes"));
    }
    let sdk = decode(sdk_bytes)?;
    let (protocol_bytes, _) = tracked(l, provided, PROTOCOL)?;
    let protocol = decode(protocol_bytes)?;
    if sdk["schema"] != "madb.native-toolchain/1"
        || sdk["runtime_protocol"] != protocol
        || protocol["schema"] != "madb.native-runtime-protocol/1"
        || protocol["component"] != NATIVE
        || protocol["version"] != VERSION
        || protocol["provider"] != "rust"
        || protocol["direction"] != "c-to-rust"
    {
        return Err(invalid("actual SDK protocol identity or metadata differs"));
    }
    let mut native_sources: Vec<_> = array(&l["inputs"])?
        .iter()
        .filter(|v| {
            v["path"]
                .as_str()
                .is_some_and(|s| s.starts_with(SDK_PREFIX))
        })
        .map(reference)
        .collect::<Result<_>>()?;
    native_sources.sort_by(|a, b| a.path.cmp(&b.path));
    let mut seen = BTreeSet::new();
    for source in &native_sources {
        if !seen.insert(source.path.as_str()) {
            return Err(invalid("duplicate native source"));
        }
        bytes(provided, source)?;
    }
    let mut minimum = vec![
        "sdk/rust/engine/Cargo.toml",
        SOURCE,
        "sdk/rust/engine/src/runtime.rs",
        PROTOCOL,
        "sdk/rust/engine/cbindgen.toml",
    ];
    if diagnostics {
        minimum.extend([
            "sdk/rust/engine/src/diagnostics.rs",
            "sdk/rust/engine/cbindgen-diagnostics.toml",
        ]);
    }
    if minimum.iter().any(|name| !seen.contains(name)) {
        return Err(invalid("finite native SDK source closure is incomplete"));
    }
    let (manifest, _) = tracked(l, provided, "sdk/rust/engine/Cargo.toml")?;
    check_package(manifest)?;
    let refs: Vec<_> = native_sources
        .iter()
        .map(|r| json!({"path":r.path.as_str(),"sha256":r.digest.as_str(),"size":r.size}))
        .collect();
    let source_digest = crate::source_digest(&json!(refs)).map_err(|e| invalid(e.to_string()))?;
    let exports = array(&config["exports"])?;
    let mut roles = BTreeMap::new();
    let mut public_ids = BTreeSet::new();
    for export in exports {
        let role = string(&export["role"])?;
        let public_id = string(&export["public_id"])?;
        if export["name"] != format!("madb_fixture_{role}")
            || !public_ids.insert(public_id)
            || roles
                .insert(role.to_owned(), public_id.to_owned())
                .is_some()
        {
            return Err(invalid(
                "native role mapping is duplicate or has an unsupported export",
            ));
        }
    }
    let required_roles: BTreeSet<_> = BASE_ROLES
        .iter()
        .chain(if diagnostics { DIAGNOSTIC_ROLES } else { &[] })
        .copied()
        .collect();
    if roles.keys().map(String::as_str).collect::<BTreeSet<_>>() != required_roles {
        return Err(invalid(
            "native role mapping is not the exact seven/nine surface",
        ));
    }
    let reader_digest =
        Digest::new(string(&component["sha256"])?).map_err(|e| invalid(e.to_string()))?;
    let owners = Owners {
        reader: Owner {
            label: format!("{READER}@{VERSION}"),
            source: reader_digest.clone(),
        },
        native: Owner {
            label: format!("{NATIVE}@{VERSION}"),
            source: Digest::new(&source_digest).map_err(|e| invalid(e.to_string()))?,
        },
        helper: Owner {
            label: format!("include/madb_rust_client.h via {READER}@{VERSION}"),
            source: reader_digest,
        },
    };
    let rows = plan(ir, exports, &roles, &owners)?;
    let rendered = render(
        ir,
        component,
        &sdk_file,
        &source_digest,
        &native_sources,
        &rows,
    )?;
    let data = rendered.into_bytes();
    let reference = FileReference {
        path: LogicalPath::new("responsibilities.md").expect("fixed output path"),
        digest: Digest::of_bytes(&data),
        size: data.len() as u64,
    };
    Ok(Some(FirstRouteOwnership {
        bytes: data,
        reference,
    }))
}

fn check_package(data: &[u8]) -> Result<()> {
    let manifest: toml::Value =
        toml::from_str(std::str::from_utf8(data).map_err(|e| invalid(e.to_string()))?)
            .map_err(|e| invalid(e.to_string()))?;
    if manifest
        .get("package")
        .and_then(|v| v.get("name"))
        .and_then(toml::Value::as_str)
        != Some(NATIVE)
        || manifest
            .get("package")
            .and_then(|v| v.get("version"))
            .and_then(toml::Value::as_str)
            != Some(VERSION)
    {
        return Err(invalid("actual SDK Cargo package identity differs"));
    }
    Ok(())
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('`', "\\`")
        .replace(['\r', '\n'], " ")
}
fn scalar(value: &Value) -> Result<String> {
    match value {
        Value::String(v) => Ok(v.clone()),
        Value::Bool(v) => Ok(v.to_string()),
        Value::Number(v) => Ok(v.to_string()),
        _ => Err(invalid("policy value is not an existing scalar")),
    }
}
fn origin(value: &Value) -> Result<String> {
    Ok(format!(
        "{}:{}@{}:{}:{}",
        string(&value["authority"])?,
        string(&value["id"])?,
        string(&value["location"]["path"])?,
        value["location"]["line"],
        value["location"]["column"]
    ))
}
fn render(
    ir: &Value,
    component: &Value,
    sdk: &FileReference,
    source_digest: &str,
    sources: &[FileReference],
    rows: &Rows,
) -> Result<String> {
    let mut output = format!(
        "# C-to-Rust mechanism responsibilities\n\nFinite generated source evidence; not runtime certification.\n\n- Boundary: `{}`\n- Profile: `{}` (`{}`)\n- Source digest: `{}`\n- Structure digest: `{}`\n- Policy digest: `{}`\n- Reader/generator: `{READER}@{VERSION}` (`{}`)\n- Native SDK: `{NATIVE}@{VERSION}`\n- Native source digest: `{source_digest}`\n- SDK metadata: `{}` (`{}`, {} bytes)\n\n## Responsibilities\n\n| Subject | Stage | Scope | Owner |\n| --- | --- | --- | --- |\n",
        escape(string(&ir["boundary_id"])?),
        escape(string(&ir["selector"]["profile_id"])?),
        string(&ir["selector"]["profile_digest"])?,
        string(&ir["source_digest"])?,
        string(&ir["structure_digest"])?,
        string(&ir["policy_digest"])?,
        string(&component["sha256"])?,
        escape(sdk.path.as_str()),
        sdk.digest.as_str(),
        sdk.size
    );
    for (key, owner) in rows {
        output.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            escape(&key.subject),
            key.stage,
            key.scope,
            escape(&owner.label)
        ));
    }
    output.push_str("\n## Effective policy inputs\n\n| Symbol | Slot | Values | Origins |\n| --- | --- | --- | --- |\n");
    let mut policies: Vec<_> = array(&ir["policies"])?
        .iter()
        .map(|p| Ok((string(&p["symbol_id"])?, slot_text(&p["slot"])?, p)))
        .collect::<Result<_>>()?;
    policies.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    for (symbol, slot, policy) in policies {
        let values = policy["values"]
            .as_object()
            .ok_or_else(|| invalid("effective policy values absent"))?;
        let values: BTreeMap<_, _> = values.iter().collect();
        let values = values
            .into_iter()
            .map(|(k, v)| Ok(format!("{k}={}", scalar(v)?)))
            .collect::<Result<Vec<_>>>()?
            .join("; ");
        let mut provenance: Vec<_> = array(&policy["provenance"])?
            .iter()
            .map(|p| Ok((string(&p["property"])?, p)))
            .collect::<Result<_>>()?;
        provenance.sort_by_key(|p| p.0);
        let origins = provenance
            .into_iter()
            .map(|(property, p)| {
                let mut value = format!("{property}={}", origin(&p["origin"])?);
                if !p["replaced_default"].is_null() {
                    value.push_str(&format!(" (replaces {})", origin(&p["replaced_default"])?));
                }
                Ok(value)
            })
            .collect::<Result<Vec<_>>>()?
            .join("; ");
        output.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            escape(symbol),
            escape(&slot),
            escape(&values),
            escape(&origins)
        ));
    }
    output.push_str(
        "\n## Frozen native sources\n\n| Path | SHA-256 | Bytes |\n| --- | --- | --- |\n",
    );
    for source in sources {
        output.push_str(&format!(
            "| {} | {} | {} |\n",
            escape(source.path.as_str()),
            source.digest.as_str(),
            source.size
        ));
    }
    output.push_str("\n## Handoffs and limits\n\n- Synchronous calls return one status and initialized outputs; no async bridge or cancellation owner is claimed.\n- The SDK borrows input only through the call. Caller memory validity and callback/context lifetime remain caller preconditions.\n- SourceLease owns callback registration and serialized source-caller delivery; no consumer queue, timer or fixed OS thread is implied.\n- request_close stops admission; await_closed confirms drain; release_handle removes only a Closed token. Native cleanup precedes Closed.\n- The checked helper owns identity comparison only and delegates bootstrap/create to the same native owners; it adds no subscription or diagnostic event.\n- Diagnostic pull returns copied values and never owns the caller exporter, business resources or a second lifecycle state machine.\n- Physical assembly, actual runtime behavior, independent Lab acceptance and certification require their separate existing evidence.\n");
    Ok(output)
}

#[cfg(test)]
#[path = "first_route_ownership_tests.rs"]
mod tests;
