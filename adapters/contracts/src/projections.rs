use crate::wire::Wire;
use madb_protocol::{DigestDomain, Document, DocumentKind, FormatError, Value, semantic_digest};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

type Result<T> = std::result::Result<T, FormatError>;

fn sorted(value: &Value, key: &str) -> Result<Vec<Value>> {
    let mut items = value
        .as_array()
        .ok_or_else(|| FormatError::new("HASH004", key, "expected collection"))?
        .clone();
    let mut seen = BTreeSet::new();
    for item in &items {
        let id = item
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| FormatError::new("HASH004", key, "missing set identity"))?;
        if !seen.insert(id.to_owned()) {
            return Err(FormatError::new("HASH004", key, "duplicate set identity"));
        }
    }
    items.sort_by(|a, b| {
        a[key]
            .as_str()
            .unwrap_or_default()
            .encode_utf16()
            .cmp(b[key].as_str().unwrap_or_default().encode_utf16())
    });
    Ok(items)
}
fn strings(value: &Value) -> Result<Vec<Value>> {
    let mut items = value
        .as_array()
        .ok_or_else(|| FormatError::new("HASH004", "", "expected string set"))?
        .clone();
    if items.iter().any(|v| !v.is_string()) {
        return Err(FormatError::new("HASH004", "", "non-string set element"));
    }
    items.sort_by(|a, b| {
        a.as_str()
            .unwrap_or_default()
            .encode_utf16()
            .cmp(b.as_str().unwrap_or_default().encode_utf16())
    });
    if items.windows(2).any(|w| w[0] == w[1]) {
        return Err(FormatError::new("HASH004", "", "duplicate set element"));
    }
    Ok(items)
}
fn references(definition: &Value) -> Result<Vec<String>> {
    let d = Wire::at(definition, "/definition");
    let fields = |name: &str| -> Result<Vec<String>> {
        d.field(name)?
            .array()?
            .into_iter()
            .map(|p| Ok(p.field("type_id")?.string()?.into()))
            .collect()
    };
    Ok(match d.field("kind")?.string()? {
        "buffer" => vec![d.field("element")?.string()?.into()],
        "nullable" => vec![d.field("inner")?.string()?.into()],
        "record" => fields("fields")?,
        "callback" => {
            let mut ids = fields("parameters")?;
            ids.push(d.field("result")?.string()?.into());
            if d.value.get("throws").is_some() {
                ids.push(d.field("throws")?.string()?.into());
            }
            ids
        }
        "variant" => {
            let mut ids = vec![d.field("tag_type")?.string()?.into()];
            for case in d.field("cases")?.array()? {
                let payload = case.field("payload")?;
                if !payload.value.is_null() {
                    ids.push(payload.string()?.into());
                }
            }
            ids
        }
        _ => Vec::new(),
    })
}

pub fn shape_digest(ir: &Document, public_id: &str) -> Result<String> {
    let root = Wire::root(ir, DocumentKind::ContractIr)?;
    let value = root.value;
    let symbols = sorted(&value["symbols"], "public_id")?;
    let symbol = symbols
        .iter()
        .find(|s| s["public_id"].as_str() == Some(public_id))
        .ok_or_else(|| FormatError::new("HASH004", "/symbols", "symbol absent"))?;
    let all = sorted(&value["types"], "id")?;
    let index: BTreeMap<_, _> = all
        .iter()
        .map(|v| (v["id"].as_str().unwrap_or_default(), v))
        .collect();
    let shape = Wire::at(&symbol["shape"], "/shape");
    let mut pending: Vec<String> = shape
        .field("parameters")?
        .array()?
        .into_iter()
        .map(|p| Ok(p.field("type_id")?.string()?.into()))
        .collect::<Result<_>>()?;
    pending.push(shape.field("result")?.string()?.into());
    if shape.value.get("throws").is_some() {
        pending.push(shape.field("throws")?.string()?.into());
    }
    let mut visited = BTreeSet::new();
    let mut types = Vec::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        let item = index
            .get(id.as_str())
            .ok_or_else(|| FormatError::new("HASH004", "/types", format!("unknown type {id}")))?;
        pending.extend(references(&item["definition"])?);
        types.push((*item).clone());
    }
    semantic_digest(
        DigestDomain::Shape,
        &json!({"shape":symbol["shape"],"types":sorted(&Value::Array(types),"id")?,"layouts":layouts(value,Some(&visited))?}),
    )
}

pub fn structure_digest(ir: &Document) -> Result<String> {
    let root = Wire::root(ir, DocumentKind::ContractIr)?;
    let v = root.value;
    let symbols: Vec<_> = sorted(&v["symbols"], "public_id")?
        .into_iter()
        .map(|s| json!({"public_id":s["public_id"],"shape":s["shape"]}))
        .collect();
    semantic_digest(
        DigestDomain::Contract,
        &json!({"schema":"madb.structure/1","sdk_namespace":v["sdk_namespace"],"boundary_id":v["boundary_id"],"types":sorted(&v["types"],"id")?,"symbols":symbols,"required_features":strings(&v["required_features"])?,"layouts":layouts(v,None)?}),
    )
}
pub fn policy_digest(ir: &Document) -> Result<String> {
    let root = Wire::root(ir, DocumentKind::ContractIr)?;
    let mut policies = Vec::new();
    let mut seen = BTreeSet::new();
    for p in root.field("policies")?.array()? {
        let id = p.field("symbol_id")?.string()?;
        let slot = p.field("slot")?;
        let kind = slot.field("kind")?.string()?;
        let parameter = if kind == "parameter" {
            slot.field("name")?.string()?
        } else {
            ""
        };
        let key = (id.to_owned(), kind.to_owned(), parameter.to_owned());
        if !seen.insert(key.clone()) {
            return Err(p.error("duplicate policy slot"));
        }
        policies.push((
            key,
            json!({"symbol_id":id,"slot":slot.value,"values":p.field("values")?.value}),
        ));
    }
    policies.sort_by(|a, b| a.0.cmp(&b.0));
    semantic_digest(
        DigestDomain::Policy,
        &Value::Array(policies.into_iter().map(|p| p.1).collect()),
    )
}
pub fn profile_digest(profile: &Document) -> Result<String> {
    let root = Wire::root(profile, DocumentKind::Profile)?;
    semantic_digest(DigestDomain::Profile, &profile_projection(root.value)?)
}

/// The existing profile fields' projection, shared by explicit versioned
/// entry points. Callers select and validate their own document kind and domain.
pub(crate) fn profile_projection(value: &Value) -> Result<Value> {
    let root = Wire::at(value, "");
    let mut v = root.value.clone();
    let object = v
        .as_object_mut()
        .ok_or_else(|| root.error("profile object required"))?;
    object.remove("extensions");
    object.insert(
        "defaults".into(),
        root.field("defaults")?.field("values")?.value.clone(),
    );
    for (field, key) in [
        ("tools", "id"),
        ("macros", "name"),
        ("restrictions", "feature"),
    ] {
        object.insert(
            field.into(),
            Value::Array(sorted(root.field(field)?.value, key)?),
        );
    }
    for field in ["required_features", "supported_features"] {
        object.insert(
            field.into(),
            Value::Array(strings(root.field(field)?.value)?),
        );
    }
    Ok(v)
}
pub fn source_digest(files: &Value) -> Result<String> {
    let mut projected = Vec::new();
    for file in sorted(files, "path")? {
        let f = Wire::at(&file, "/inputs");
        projected.push(json!({"path":f.field("path")?.string()?,"sha256":f.field("sha256")?.digest()?.as_str(),"size":f.field("size")?.uint()?}));
    }
    semantic_digest(DigestDomain::Source, &Value::Array(projected))
}
pub fn task_digest(task: &Document) -> Result<String> {
    let root = Wire::root(task, DocumentKind::BuildTask)?;
    let mut v = root.value.clone();
    let object = v
        .as_object_mut()
        .ok_or_else(|| root.error("task object required"))?;
    object.remove("task_digest");
    for (name, key) in [("inputs", "path"), ("environment", "name")] {
        object.insert(
            name.into(),
            Value::Array(sorted(root.field(name)?.value, key)?),
        );
    }
    for name in ["dependencies", "outputs"] {
        object.insert(name.into(), Value::Array(strings(root.field(name)?.value)?));
    }
    semantic_digest(DigestDomain::Task, &v)
}
pub fn generation_id(outputs: &Document) -> Result<String> {
    let root = Wire::root(outputs, DocumentKind::OutputSet)?;
    let mut files = sorted(root.field("files")?.value, "path")?;
    for f in &mut files {
        if f["path"] == root.field("manifest_path")?.value.clone() {
            return Err(root.error("manifest must not hash itself"));
        }
        let sources = strings(&f["sources"])?;
        f["sources"] = Value::Array(sources);
    }
    semantic_digest(
        DigestDomain::Generation,
        &json!({"task_digest":root.field("task_digest")?.value,"input_digest":root.field("input_digest")?.value,"profile_digest":root.field("profile_digest")?.value,"files":files}),
    )
}
pub fn candidate_digest(candidate: &Document) -> Result<String> {
    let root = Wire::root(candidate, DocumentKind::Candidate)?;
    let mut v = root.value.clone();
    let object = v
        .as_object_mut()
        .ok_or_else(|| root.error("candidate object required"))?;
    object.remove("candidate_digest");
    for field in ["files", "dependencies", "build_evidence"] {
        let items = sorted(root.field(field)?.value, "path")?;
        if field == "files"
            && items
                .iter()
                .any(|f| f["path"] == root.value["manifest_path"])
        {
            return Err(root.error("candidate manifest must not hash itself"));
        }
        object.insert(field.into(), Value::Array(items));
    }
    for field in ["public_symbols", "required_cases"] {
        object.insert(
            field.into(),
            Value::Array(strings(root.field(field)?.value)?),
        );
    }
    semantic_digest(DigestDomain::Candidate, &v)
}
pub fn case_digest(case: &Document) -> Result<String> {
    let root = Wire::root(case, DocumentKind::Case)?;
    let mut v = root.value.clone();
    for field in ["problem_ids", "source_ids"] {
        v[field] = Value::Array(strings(root.field(field)?.value)?);
    }
    v["requires"]["features"] = Value::Array(strings(&v["requires"]["features"])?);
    v["requires"]["environment"] = Value::Array(sorted(&v["requires"]["environment"], "id")?);
    semantic_digest(DigestDomain::Case, &v)
}

/// Author configuration identity: declaration positions and annotations describing
/// provenance do not invalidate semantics; the selected source paths still do.
pub fn project_digest(project: &Document) -> Result<String> {
    let root = Wire::root(project, DocumentKind::Project)?;
    let mut v = root.value.clone();
    v.as_object_mut()
        .ok_or_else(|| root.error("project object required"))?
        .remove("extensions");
    v["boundaries"] = Value::Array(sorted(&v["boundaries"], "id")?);
    let mut families = sorted(&v["policy_families"], "id")?;
    for f in &mut families {
        f.as_object_mut()
            .ok_or_else(|| root.error("family object"))?
            .remove("origin");
    }
    v["policy_families"] = Value::Array(families);
    let mut policies = Vec::new();
    for p in v["policies"]
        .as_array()
        .ok_or_else(|| root.error("policy array"))?
    {
        let mut p = p.clone();
        p.as_object_mut()
            .ok_or_else(|| root.error("policy object"))?
            .remove("origin");
        p["families"] = Value::Array(strings(&p["families"])?);
        policies.push((madb_protocol::canonical_json(&p)?, p));
    }
    policies.sort_by(|a, b| a.0.cmp(&b.0));
    v["policies"] = Value::Array(policies.into_iter().map(|p| p.1).collect());
    semantic_digest(DigestDomain::Project, &v)
}

fn layouts(ir: &Value, reachable: Option<&BTreeSet<String>>) -> Result<Vec<Value>> {
    Ok(sorted(&ir["observations"], "subject")?
        .into_iter()
        .filter(|o| {
            reachable.is_none_or(|set| o["subject"].as_str().is_some_and(|id| set.contains(id)))
        })
        .map(|o| json!({"subject":o["subject"],"result":o["result"]}))
        .collect())
}
