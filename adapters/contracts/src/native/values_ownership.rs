//! Finite value-recipe responsibilities bound to actual immutable author inputs.
//! Syntax replay here does not replace the external rustc witness or target checks.
use super::FirstRouteOwnership;
use crate::{Document, DocumentKind};
use madb_app::{
    Digest, ErrorKind, LogicalPath, QueryError, Result, inputs::FrozenInputs, ports::FileReference,
};
use madb_protocol::{Value, canonical_json, strict_json};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

const READER: &str = "org.madb.native-values/reader-generator";
const VERSION: &str = "0.1.0";
const SEMANTICS: &str = "madb.native-values/1";
const NATIVE: &str = "madb-native-values@1";
const HANDOFFS: &str = "\n## Handoffs and limits\n\n- Generated native entries own synchronous input conversion, one author call, result conversion and typed error mapping; no resource, async or cancellation owner is claimed.\n- The ordinary helper owns bootstrap identity comparison and provider-result validation only; it does not allocate another Rust value or retry a call.\n- Caller memory validity, alignment, non-overlap and absence of data races remain raw-entry preconditions; target execution and independent Lab acceptance require separate evidence.\n";

type Key = (String, String, String);
type Owner = (String, Digest);
type Rows = Vec<(Key, Owner)>;
struct Owners {
    reader: Owner,
    native: Owner,
    helper: Owner,
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
        .ok_or_else(|| invalid("finite value responsibility string absent"))
}
fn array(value: &Value) -> Result<&Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| invalid("finite value responsibility array absent"))
}
fn reference(value: &Value) -> Result<FileReference> {
    Ok(FileReference {
        path: LogicalPath::new(string(&value["path"])?).map_err(|e| invalid(e.to_string()))?,
        digest: Digest::new(string(&value["sha256"])?).map_err(|e| invalid(e.to_string()))?,
        size: value["size"]
            .as_u64()
            .ok_or_else(|| invalid("source size absent"))?,
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
fn semantic(domain: &str, value: &Value) -> Result<Digest> {
    let mut data = domain.as_bytes().to_vec();
    data.push(b'\n');
    data.extend(canonical_json(value).map_err(|e| invalid(e.to_string()))?);
    Ok(Digest::of_bytes(&data))
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
fn row(subject: &str, stage: &str, scope: &str, owner: &Owner) -> (Key, Owner) {
    ((subject.into(), stage.into(), scope.into()), owner.clone())
}
fn bootstrap_rows(subject: &str, owners: &Owners) -> Rows {
    vec![
        row(subject, "type-read", "recipe-bootstrap", &owners.reader),
        row(subject, "abi-lowering", "bootstrap-v1", &owners.reader),
        row(subject, "projection", "identity-payload", &owners.native),
    ]
}
fn helper_rows(name: &str, owners: &Owners) -> Rows {
    ["identity-gate", "provider-result-check"]
        .into_iter()
        .map(|scope| row(name, "projection", scope, &owners.helper))
        .collect()
}
fn source_identity(
    namespace: &str,
    package: &str,
    source: &str,
    name: &str,
) -> Result<(Value, String, String)> {
    let key = madb_model::SourceKey {
        reader: madb_model::Id::new("org.madb.native-values@1").expect("fixed reader"),
        package: package.into(),
        module: source.into(),
        kind: "function".into(),
        qualified_name: name.into(),
        overload: "none".into(),
    };
    let source_id = key.source_id().map_err(|e| invalid(e.to_string()))?;
    let public = madb_model::PublicId::derived(
        &madb_model::Id::new(namespace).map_err(|e| invalid(e.to_string()))?,
        &source_id,
    )
    .map_err(|e| invalid(e.to_string()))?;
    Ok((
        json!({"reader":key.reader.as_str(),"package":key.package,"module":key.module,"kind":key.kind,"qualified_name":key.qualified_name,"overload":key.overload}),
        source_id.as_str().into(),
        public.as_str().into(),
    ))
}
// Requirements use the checked logical IR; no producer responsibility list is accepted.
fn required_rows(ir: &Value, owners: &Owners) -> Result<Rows> {
    let namespace = string(&ir["sdk_namespace"])?;
    let mut rows = Vec::new();
    for symbol in array(&ir["symbols"])? {
        let id = string(&symbol["public_id"])?;
        let shape = &symbol["shape"];
        if shape["result"] == "madb.recipe.identity_bytes" {
            rows.extend(bootstrap_rows(id, owners));
            continue;
        }
        let record = array(&ir["types"])?
            .iter()
            .find(|ty| ty["id"] == shape["result"])
            .ok_or_else(|| invalid("value return type absent"))?;
        if record["definition"]["kind"] != "record" || record["definition"]["layout"] != "logical" {
            return Err(invalid("value return must be a checked logical record"));
        }
        rows.push(row(id, "type-read", "rust-declaration", &owners.reader));
        rows.push(row(id, "abi-lowering", "rust-to-c-values", &owners.reader));
        for scope in ["native-call", "input-conversion", "output-conversion"] {
            rows.push(row(id, "projection", scope, &owners.native));
        }
        if !shape["throws"].is_null() {
            rows.push(row(id, "projection", "domain-error", &owners.native));
        }
        let prefix = format!("{namespace}_raw_");
        let name = string(&shape["link_name"])?
            .strip_prefix(&prefix)
            .ok_or_else(|| invalid("value link name is outside namespace"))?;
        rows.extend(helper_rows(&format!("helper:{namespace}_{name}"), owners));
    }
    Ok(rows)
}
// Provided mechanisms follow actual frozen author declarations and the selected generator.
fn provided_rows(plan: &Value, package: &str, source: &str, owners: &Owners) -> Result<Rows> {
    let namespace = string(&plan["namespace"])?;
    let mut rows = bootstrap_rows(
        &source_identity(namespace, package, source, "$recipe.bootstrap.v1")?.2,
        owners,
    );
    for function in array(&plan["functions"])? {
        let name = string(&function["name"])?;
        let id = source_identity(namespace, package, source, name)?.2;
        rows.push(row(&id, "type-read", "rust-declaration", &owners.reader));
        rows.push(row(&id, "abi-lowering", "rust-to-c-values", &owners.reader));
        rows.push(row(&id, "projection", "input-conversion", &owners.native));
        rows.push(row(&id, "projection", "native-call", &owners.native));
        if !function["error"].is_null() {
            rows.push(row(&id, "projection", "domain-error", &owners.native));
        }
        rows.push(row(&id, "projection", "output-conversion", &owners.native));
        rows.extend(helper_rows(&format!("helper:{namespace}_{name}"), owners));
    }
    Ok(rows)
}
fn types_from_source(plan: &Value) -> Result<Value> {
    let mut types = vec![json!({"id":"madb.recipe.unit","definition":{"kind":"unit"}})];
    for (name, width, signed) in [("u8", 8, false), ("u32", 32, false), ("i32", 32, true)] {
        types
            .push(json!({"id":format!("madb.recipe.{name}"),"definition":{"kind":"integer","width":width,"signed":signed}}));
    }
    types.push(
        json!({"id":"madb.recipe.String","definition":{"kind":"string","encoding":"utf-8","max_bytes":64}}),
    );
    types.push(json!({"id":"madb.recipe.identity_bytes","definition":{"kind":"buffer","element":"madb.recipe.u8","length_unit":"bytes","encoding":"none","mutable":false,"max_bytes":64}}));
    for record in array(&plan["records"])? {
        let fields: Vec<_> = array(&record["fields"])?
            .iter()
            .map(|f| json!({"name":f["name"],"type_id":format!("madb.recipe.{}", f["kind"].as_str().expect("checked field kind"))}))
            .collect();
        types.push(json!({"id":record["name"],"definition":{"kind":"record","layout":"logical","fields":fields}}));
    }
    for error in array(&plan["errors"])? {
        let cases = array(&error["variants"])?.iter().map(|v| {
            let tag = v["value"].as_i64().ok_or_else(|| invalid("error tag absent"))?;
            Ok(json!({"name":v["name"],"tag":{"kind":"integer","type":"i32","decimal":tag.to_string()},"payload":null}))
        }).collect::<Result<Vec<_>>>()?;
        types.push(json!({"id":error["name"],"definition":{"kind":"variant","tag_type":"madb.recipe.i32","unknown":"reject","cases":cases}}));
    }
    types.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    Ok(json!(types))
}
fn slot_text(slot: &Value) -> Result<String> {
    match string(&slot["kind"])? {
        "parameter" => Ok(format!("parameter:{}", string(&slot["name"])?)),
        "call" => Ok("call".into()),
        "return" => Ok("return".into()),
        _ => Err(invalid("unsupported value policy slot")),
    }
}
fn check_source_and_policy(ir: &Value, plan: &Value, package: &str, source: &str) -> Result<()> {
    let namespace = string(&plan["namespace"])?;
    if ir["sdk_namespace"] != namespace {
        return Err(invalid("namespace differs from frozen author plan"));
    }
    let mut actual_types = array(&ir["types"])?.clone();
    actual_types.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    if json!(actual_types) != types_from_source(plan)? {
        return Err(invalid(
            "logical types differ from actual author declarations",
        ));
    }
    let mut expected = BTreeMap::new();
    expected.insert(source_identity(namespace, package, source, "$recipe.bootstrap.v1")?.2, (json!({"kind":"sync","calling_convention":"c","link_name":format!("{namespace}_bootstrap_v1"),"parameters":[{"name":"version","type_id":"madb.recipe.u32"}],"result":"madb.recipe.identity_bytes","constants":[]}), json!({"path":source,"line":1,"column":1}), "$recipe.bootstrap.v1".to_owned()));
    for function in array(&plan["functions"])? {
        let name = string(&function["name"])?;
        let parameters: Vec<_> = array(&function["parameters"])?
            .iter().enumerate()
            .map(|(index, p)| json!({"name":p["name"],"type_id":if index == 0 { p["kind"].clone() } else { json!("madb.recipe.i32") }}))
            .collect();
        let mut shape = json!({"kind":"sync","calling_convention":"c","link_name":format!("{namespace}_raw_{name}"),"parameters":parameters,"result":function["record"],"constants":[]});
        if !function["error"].is_null() {
            shape["throws"] = function["error"].clone();
        }
        if expected
            .insert(
                source_identity(namespace, package, source, name)?.2,
                (shape, function["location"].clone(), name.to_owned()),
            )
            .is_some()
        {
            return Err(invalid("duplicate value declaration"));
        }
    }
    let mut policies = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for symbol in array(&ir["symbols"])? {
        let id = string(&symbol["public_id"])?;
        let (shape, location, name) = expected
            .get(id)
            .ok_or_else(|| invalid("symbol outside author declaration set"))?;
        let (key, source_id, _) = source_identity(namespace, package, source, name)?;
        if !seen.insert(id)
            || &symbol["shape"] != shape
            || &symbol["location"] != location
            || symbol["precision"] != "artifact-derived"
            || symbol["public_id_origin"] != "derived"
            || symbol["source_key"] != key
            || symbol["source_id"] != source_id
        {
            return Err(invalid(
                "symbol shape or origin differs from frozen value source",
            ));
        }
        policies.insert((id.to_owned(),"call".to_owned()),json!({"threading":"caller","blocking":"unbounded","delivery":"work-completed","cancellation":"none","cancellation_confirmation":"not-supported"}));
        policies.insert(
            (id.to_owned(), "return".to_owned()),
            json!({"ownership":"copy"}),
        );
        for parameter in array(&shape["parameters"])? {
            policies.insert(
                (
                    id.to_owned(),
                    format!("parameter:{}", string(&parameter["name"])?),
                ),
                json!({"ownership":"copy"}),
            );
        }
    }
    let mut actual_policies = BTreeMap::new();
    for policy in array(&ir["policies"])? {
        if actual_policies
            .insert(
                (
                    string(&policy["symbol_id"])?.to_owned(),
                    slot_text(&policy["slot"])?,
                ),
                policy["values"].clone(),
            )
            .is_some()
        {
            return Err(invalid("duplicate value policy slot"));
        }
    }
    if seen.len() != expected.len() || policies != actual_policies {
        return Err(invalid(
            "value policy or symbol coverage differs from finite recipe",
        ));
    }
    Ok(())
}
fn component_input(path: &str) -> bool {
    path.starts_with("adapters/generators/rust_c/")
        || path.starts_with("adapters/contracts/src/native_values")
        || path == "crates/madb-cli/src/bin/madb-native-values-read.rs"
}

pub(crate) fn values_ownership(
    project: &Document,
    profile: &Document,
    lock: &Document,
    request: &Document,
    contract: &Document,
    provided: &FrozenInputs,
) -> Result<Option<FirstRouteOwnership>> {
    if (profile.kind(), lock.kind(), request.kind(), contract.kind())
        != (
            DocumentKind::Profile,
            DocumentKind::Lock,
            DocumentKind::GenerateRequest,
            DocumentKind::ContractIr,
        )
    {
        return Err(invalid(
            "value responsibilities require checked generation documents",
        ));
    }
    let q = request.value();
    let l = lock.value();
    let p = profile.value();
    let ir = contract.value();
    let backend = array(&l["inputs"])?
        .iter()
        .find(|r| r["path"] == "backend.json");
    if backend.is_none() && q["component"]["id"] != READER {
        return Ok(None);
    }
    let (config_bytes, _) = tracked(l, provided, "backend.json")?;
    let config = match decode(config_bytes) {
        Ok(value) => value,
        Err(error) if q["component"]["id"] == READER => return Err(error),
        Err(_) => return Ok(None),
    };
    if config["semantics"] != SEMANTICS && q["component"]["id"] != READER {
        return Ok(None);
    }
    if config["semantics"] != SEMANTICS
        || config["provider"] != "rust"
        || config["schema"] != "madb.rust-c-backend/1"
        || q["component"]["id"] != READER
        || q["component"]["version"] != VERSION
        || !array(&l["components"])?.contains(&q["component"])
    {
        return Err(invalid("finite value recipe/reader identity differs"));
    }
    if p["schema"] != "madb.profile/1"
        || p["route"] != "RUST-C-ABI"
        || p["provider"] != json!({"language":"rust","runtime":"native"})
        || p["consumer"] != json!({"language":"c","runtime":"native"})
        || p["direction"] != "consumer-to-provider"
        || !p["backend_config"].is_null()
    {
        return Err(invalid("finite value profile differs"));
    }
    if q["selector"] != ir["selector"]
        || q["boundary_id"] != ir["boundary_id"]
        || q["input_digest"] != ir["source_digest"]
        || q["contract_digest"] != ir["structure_digest"]
        || q["policy_digest"] != ir["policy_digest"]
        || p["id"] != q["selector"]["profile_id"]
        || p["target"] != q["selector"]["target"]
        || p["route"] != q["selector"]["route"]
        || p["direction"] != q["selector"]["direction"]
    {
        return Err(invalid("value responsibility lineage differs"));
    }
    if crate::source_digest(&q["inputs"]).map_err(|e| invalid(e.to_string()))?
        != crate::source_digest(&l["inputs"]).map_err(|e| invalid(e.to_string()))?
    {
        return Err(invalid("value inputs differ from lock"));
    }
    let mut inputs = Vec::new();
    let mut paths = BTreeSet::new();
    let mut component = Vec::new();
    for item in array(&q["inputs"])? {
        let file = reference(item)?;
        if !paths.insert(file.path.as_str().to_owned()) {
            return Err(invalid("duplicate value input"));
        }
        bytes(provided, &file)?;
        if component_input(file.path.as_str()) {
            component.push(item.clone());
        }
        inputs.push(file);
    }
    inputs.sort_by(|a, b| a.path.cmp(&b.path));
    component.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    for required in [
        "adapters/generators/rust_c/values_driver.py",
        "adapters/generators/rust_c/values_lowering.py",
        "adapters/generators/rust_c/values_codegen.py",
        "adapters/generators/rust_c/values_runtime.py",
        "adapters/contracts/src/native_values.rs",
        "adapters/contracts/src/native_values/names.rs",
        "adapters/contracts/src/native_values/witness.rs",
        "adapters/generators/rust_c/values_client.py",
        "adapters/generators/rust_c/values_ownership.py",
        "adapters/generators/rust_c/native_ast.py",
        "crates/madb-cli/src/bin/madb-native-values-read.rs",
        "Cargo.lock",
        "adapters/contracts/Cargo.toml",
    ] {
        if !paths.contains(required) {
            return Err(invalid(format!(
                "value source closure is incomplete: {required}"
            )));
        }
    }
    let component_digest = semantic("madb.native-component/1", &json!(component))?;
    if q["component"]["sha256"] != component_digest.as_str() {
        return Err(invalid(
            "value component differs from frozen reader/generator sources",
        ));
    }
    if project.kind() != DocumentKind::Project {
        return Err(invalid(
            "value responsibilities require the checked frozen project",
        ));
    }
    let package = string(&project.value()["project"])?;
    if project.value()["sdk_namespace"] != ir["sdk_namespace"] {
        return Err(invalid("value namespace differs from frozen project"));
    }
    let source = string(&config["source"])?;
    let boundaries: Vec<_> = array(&project.value()["boundaries"])?
        .iter()
        .filter(|boundary| boundary["id"] == ir["boundary_id"])
        .collect();
    if boundaries.len() != 1
        || boundaries[0]["source"]["kind"] != "rust-c-declaration"
        || boundaries[0]["source"]["path"] != source
    {
        return Err(invalid(
            "value author differs from the checked project boundary source",
        ));
    }
    let (author, _) = tracked(l, provided, source)?;
    let author = std::str::from_utf8(author).map_err(|e| invalid(e.to_string()))?;
    let envelope = json!({"source_path":source,"source":author,"namespace":ir["sdk_namespace"]});
    let plan = decode(
        &crate::native_values::read_envelope(
            &canonical_json(&envelope).map_err(|e| invalid(e.to_string()))?,
        )
        .map_err(|e| invalid(e.to_string()))?,
    )?;
    check_source_and_policy(ir, &plan, package, source)?;
    let sdk = reference(&p["sdk"])?;
    let (sdk_bytes, locked_sdk) = tracked(l, provided, sdk.path.as_str())?;
    if sdk != locked_sdk
        || p["sdk"]["schema"] != "madb.native-toolchain/1"
        || decode(sdk_bytes)?["schema"] != "madb.native-toolchain/1"
    {
        return Err(invalid("value SDK metadata differs"));
    }
    let owners = Owners {
        reader: (format!("{READER}@{VERSION}"), component_digest.clone()),
        native: (
            NATIVE.into(),
            Digest::new(string(&ir["source_digest"])?).map_err(|e| invalid(e.to_string()))?,
        ),
        helper: (
            format!(
                "include/{}.h via {READER}@{VERSION}",
                string(&plan["namespace"])?
            ),
            component_digest,
        ),
    };
    let mut rows = provided_rows(&plan, package, source, &owners)?;
    madb_rules::compare_unique_owners(&required_rows(ir, &owners)?, &rows)
        .map_err(|e| invalid(format!("value responsibility coverage differs: {e:?}")))?;
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    let data = render(ir, &q["component"], &sdk, &inputs, &rows)?.into_bytes();
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

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('`', "\\`")
        .replace(['\r', '\n'], " ")
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
fn scalar(value: &Value) -> Result<String> {
    match value {
        Value::String(s) => Ok(s.clone()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) => Ok(n.to_string()),
        _ => Err(invalid("policy value is not scalar")),
    }
}
fn render(
    ir: &Value,
    component: &Value,
    sdk: &FileReference,
    inputs: &[FileReference],
    rows: &Rows,
) -> Result<String> {
    let mut output = format!(
        "# Native value mechanism responsibilities\n\nFinite generated source evidence; not runtime certification.\n\n- Boundary: `{}`\n- Profile: `{}` (`{}`)\n- Source digest: `{}`\n- Structure digest: `{}`\n- Policy digest: `{}`\n- Reader/generator: `{READER}@{VERSION}` (`{}`)\n- Native SDK: `{NATIVE}`\n- SDK metadata: `{}` (`{}`, {} bytes)\n\n## Responsibilities\n\n| Subject | Stage | Scope | Owner |\n| --- | --- | --- | --- |\n",
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
    for ((subject, stage, scope), (owner, _)) in rows {
        output.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            escape(subject),
            escape(stage),
            escape(scope),
            escape(owner)
        ));
    }
    output.push_str("\n## Effective policy inputs\n\n| Symbol | Slot | Values | Origins |\n| --- | --- | --- | --- |\n");
    let mut policies = array(&ir["policies"])?
        .iter()
        .map(|p| Ok((string(&p["symbol_id"])?, slot_text(&p["slot"])?, p)))
        .collect::<Result<Vec<_>>>()?;
    policies.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    for (symbol, slot, policy) in policies {
        let values = policy["values"]
            .as_object()
            .ok_or_else(|| invalid("policy values absent"))?;
        let values: BTreeMap<_, _> = values.iter().collect();
        let values = values
            .into_iter()
            .map(|(k, v)| Ok(format!("{k}={}", scalar(v)?)))
            .collect::<Result<Vec<_>>>()?
            .join("; ");
        let mut provenance = array(&policy["provenance"])?
            .iter()
            .map(|p| Ok((string(&p["property"])?, p)))
            .collect::<Result<Vec<_>>>()?;
        provenance.sort_by_key(|p| p.0);
        let origins = provenance
            .into_iter()
            .map(|(property, p)| {
                let mut v = format!("{property}={}", origin(&p["origin"])?);
                if !p["replaced_default"].is_null() {
                    v.push_str(&format!(" (replaces {})", origin(&p["replaced_default"])?));
                }
                Ok(v)
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
    output
        .push_str("\n## Frozen recipe inputs\n\n| Path | SHA-256 | Bytes |\n| --- | --- | --- |\n");
    for source in inputs {
        output.push_str(&format!(
            "| {} | {} | {} |\n",
            escape(source.path.as_str()),
            source.digest.as_str(),
            source.size
        ));
    }
    output.push_str(HANDOFFS);
    Ok(output)
}

#[cfg(test)]
#[path = "values_ownership_tests.rs"]
mod tests;
