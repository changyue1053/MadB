use super::*;
use madb_app::{inputs::capture_inputs, ports::ReadWorkspace};

// These literal role and policy facts describe the approved finite ABI, not a
// responsibility array supplied by the producer or the production planner.
fn fixture(diagnostic: bool) -> (Value, Vec<Value>, BTreeMap<String, String>, Owners) {
    let specs = [
        (
            "bootstrap",
            vec![("requested_version", "u32")],
            "identity_bytes",
            "bounded",
            "work-completed",
        ),
        ("create", vec![], "handle", "bounded", "work-completed"),
        (
            "checksum",
            vec![("handle", "handle"), ("data", "bytes")],
            "u64",
            "bounded",
            "work-completed",
        ),
        (
            "request_close",
            vec![("handle", "handle")],
            "unit",
            "non-blocking",
            "work-completed",
        ),
        (
            "await_closed",
            vec![("handle", "handle"), ("timeout_ms", "u32")],
            "unit",
            "bounded",
            "work-completed",
        ),
        (
            "release_handle",
            vec![("handle", "handle")],
            "unit",
            "bounded",
            "work-completed",
        ),
        (
            "observe",
            vec![("handle", "handle"), ("callback", "state_callback")],
            "handle",
            "unbounded",
            "registered",
        ),
        (
            "diagnostics_configure",
            vec![("enabled", "u32")],
            "unit",
            "non-blocking",
            "work-completed",
        ),
        (
            "diagnostics_drain",
            vec![],
            "diagnostic_pull",
            "non-blocking",
            "work-completed",
        ),
    ];
    let mut exports = vec![];
    let mut roles = BTreeMap::new();
    let mut symbols = vec![];
    let mut policies = vec![];
    for (role, parameters, result, blocking, delivery) in
        specs.into_iter().take(if diagnostic { 9 } else { 7 })
    {
        let id = format!("example.{role}.v1");
        let name = format!("madb_fixture_{role}");
        roles.insert(role.into(), id.clone());
        exports.push(json!({"role":role,"name":name,"public_id":id}));
        let params: Vec<_> = parameters
            .iter()
            .map(|(name, ty)| json!({"name":name,"type_id":ty}))
            .collect();
        symbols.push(json!({"public_id":id,"shape":{"kind":"sync","calling_convention":"c","link_name":name,"parameters":params,"result":result}}));
        let mut add = |slot, values| {
            policies.push(json!({"symbol_id":id,"slot":slot,"values":values,"provenance":[]}));
        };
        add(
            json!({"kind":"call"}),
            json!({"threading":"caller","blocking":blocking,"delivery":delivery,"cancellation":"none","cancellation_confirmation":"not-supported"}),
        );
        if result == "handle" {
            add(
                json!({"kind":"return"}),
                json!({"ownership":"transfer","releaser":"example.release_handle.v1","close":"request-await-release","release_thread":"caller"}),
            );
        } else if result != "unit" {
            add(json!({"kind":"return"}), json!({"ownership":"copy"}));
        }
        for (name, ty) in parameters {
            let values = match ty {
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
            add(json!({"kind":"parameter","name":name}), values);
        }
    }
    let mut types = json!([
        {"id":"unit","definition":{"kind":"unit"}},
        {"id":"u8","definition":{"kind":"integer","width":8,"signed":false}},
        {"id":"u32","definition":{"kind":"integer","width":32,"signed":false}},
        {"id":"u64","definition":{"kind":"integer","width":64,"signed":false}},
        {"id":"i32","definition":{"kind":"integer","width":32,"signed":true}},
        {"id":"bytes","definition":{"kind":"buffer","element":"u8","length_unit":"bytes","encoding":"none","mutable":false,"max_bytes":65536}},
        {"id":"identity_bytes","definition":{"kind":"buffer","element":"u8","length_unit":"bytes","encoding":"none","mutable":false,"max_bytes":64}},
        {"id":"error","definition":{"kind":"record","layout":"c","fields":[{"name":"domain","type_id":"u32"},{"name":"code","type_id":"i32"}]}},
        {"id":"handle","definition":{"kind":"resource","representation":"registry-u64","owner":"example"}},
        {"id":"state_callback","definition":{"kind":"callback","parameters":[{"name":"sequence","type_id":"u64"},{"name":"state","type_id":"u32"}],"result":"unit","calling_convention":"c"}}
    ]);
    if diagnostic {
        types.as_array_mut().unwrap().extend(array(&json!([
            {"id":"i64","definition":{"kind":"integer","width":64,"signed":true}},
            {"id":"diagnostic_event","definition":{"kind":"record","layout":"c","fields":[{"name":"record_id","type_id":"u64"},{"name":"symbol_id","type_id":"u64"},{"name":"phase","type_id":"u64"},{"name":"resource_id","type_id":"u64"},{"name":"related_resource_id","type_id":"u64"},{"name":"duration_ns","type_id":"u64"},{"name":"status","type_id":"i64"},{"name":"status_present","type_id":"u64"}]}},
            {"id":"diagnostic_stats","definition":{"kind":"record","layout":"c","fields":[{"name":"completed_calls","type_id":"u64"},{"name":"failed_calls","type_id":"u64"},{"name":"completed_callbacks","type_id":"u64"},{"name":"dropped_events","type_id":"u64"},{"name":"capture_enabled","type_id":"u64"}]}},
            {"id":"diagnostic_pull","definition":{"kind":"record","layout":"logical","fields":[{"name":"present","type_id":"u64"},{"name":"event","type_id":"diagnostic_event"},{"name":"stats","type_id":"diagnostic_stats"}]}}
        ])).unwrap().clone());
    }
    let digest = Digest::of_bytes(b"reader image");
    let native = Digest::of_bytes(b"native source inventory");
    let owners = Owners {
        reader: Owner {
            label: format!("{READER}@{VERSION}"),
            source: digest.clone(),
        },
        native: Owner {
            label: format!("{NATIVE}@{VERSION}"),
            source: native,
        },
        helper: Owner {
            label: format!("include/madb_rust_client.h via {READER}@{VERSION}"),
            source: digest,
        },
    };
    let hash = Digest::of_bytes(b"fixture");
    (
        json!({"symbols":symbols,"policies":policies,"types":types,"boundary_id":"example-boundary","selector":{"profile_id":"example-profile","profile_digest":hash.as_str()},"source_digest":hash.as_str(),"structure_digest":hash.as_str(),"policy_digest":hash.as_str()}),
        exports,
        roles,
        owners,
    )
}

#[test]
fn seven_and_nine_symbols_have_finite_sorted_coverage() {
    for (diagnostic, count) in [(false, 27), (true, 33)] {
        let (mut ir, exports, roles, owners) = fixture(diagnostic);
        let rows = plan(&ir, &exports, &roles, &owners).unwrap();
        assert_eq!(rows.len(), count);
        assert!(rows.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert_eq!(
            rows.iter().filter(|r| r.0.stage == "subscription").count(),
            1
        );
        assert_eq!(
            rows.iter()
                .filter(|r| r.0.subject.starts_with("helper:"))
                .count(),
            1
        );
        ir["symbols"].as_array_mut().unwrap().reverse();
        ir["policies"].as_array_mut().unwrap().reverse();
        assert_eq!(plan(&ir, &exports, &roles, &owners).unwrap(), rows);
    }
}

#[test]
fn policy_changes_cannot_inherit_native_lifetime_claims() {
    let (ir, exports, roles, owners) = fixture(true);
    for (subject, slot, property, wrong) in [
        ("observe", "parameter", "keep_alive", json!("call-frame")),
        ("observe", "call", "delivery", json!("work-completed")),
        ("request_close", "call", "blocking", json!("bounded")),
        ("diagnostics_drain", "return", "ownership", json!("borrow")),
        ("create", "return", "releaser", json!("other.release")),
    ] {
        let mut changed = ir.clone();
        let policy = changed["policies"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|p| {
                p["symbol_id"] == format!("example.{subject}.v1")
                    && p["slot"]["kind"] == slot
                    && (subject != "observe"
                        || slot != "parameter"
                        || p["slot"]["name"] == "callback")
            })
            .unwrap();
        policy["values"][property] = wrong;
        assert!(plan(&changed, &exports, &roles, &owners).is_err());
    }
}

#[test]
fn type_names_and_role_names_alone_do_not_authorize_rows() {
    let (ir, exports, roles, owners) = fixture(true);
    for (id, field, wrong) in [
        ("handle", "representation", json!("opaque-pointer")),
        ("state_callback", "result", json!("u64")),
        ("diagnostic_pull", "layout", json!("c")),
        ("identity_bytes", "max_bytes", json!(32)),
    ] {
        let mut changed = ir.clone();
        let ty = changed["types"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|t| t["id"] == id)
            .unwrap();
        ty["definition"][field] = wrong;
        assert!(plan(&changed, &exports, &roles, &owners).is_err());
    }
    let mut changed = ir.clone();
    changed["symbols"][0]["shape"]["result"] = json!("handle");
    assert!(plan(&changed, &exports, &roles, &owners).is_err());
    let mut changed = ir.clone();
    changed["symbols"].as_array_mut().unwrap().pop();
    assert!(plan(&changed, &exports, &roles, &owners).is_err());
    let mut changed = ir;
    let duplicate = changed["policies"][0].clone();
    changed["policies"].as_array_mut().unwrap().push(duplicate);
    assert!(plan(&changed, &exports, &roles, &owners).is_err());
}

#[test]
fn same_label_different_source_and_two_missing_mechanisms_fail_core() {
    let (ir, exports, roles, owners) = fixture(false);
    let required = requirements(&roles, &owners);
    let rows = plan(&ir, &exports, &roles, &owners).unwrap();
    let mut missing = rows.clone();
    missing.retain(|(key, _)| key.stage != "stop" && key.stage != "drain");
    assert_eq!(
        madb_rules::compare_unique_owners(&required, &missing),
        Err(madb_rules::OwnerSetError::Mismatch)
    );
    let mut wrong = rows;
    wrong[0].1.source = Digest::of_bytes(b"another image");
    assert_eq!(
        madb_rules::compare_unique_owners(&required, &wrong),
        Err(madb_rules::OwnerSetError::Mismatch)
    );
}

#[test]
fn rendered_bytes_are_order_stable_and_escape_human_cells() {
    let (mut ir, exports, roles, owners) = fixture(false);
    ir["boundary_id"] = json!("boundary|with`markup\\and\nline");
    let rows = plan(&ir, &exports, &roles, &owners).unwrap();
    let source = FileReference {
        path: LogicalPath::new("sdk/rust/engine/src/lib.rs").unwrap(),
        digest: Digest::of_bytes(b"source"),
        size: 6,
    };
    let component = json!({"sha256":owners.reader.source.as_str()});
    let output = render(
        &ir,
        &component,
        &source,
        owners.native.source.as_str(),
        std::slice::from_ref(&source),
        &rows,
    )
    .unwrap();
    ir["policies"].as_array_mut().unwrap().reverse();
    assert_eq!(
        render(
            &ir,
            &component,
            &source,
            owners.native.source.as_str(),
            std::slice::from_ref(&source),
            &rows
        )
        .unwrap(),
        output
    );
    assert!(output.contains("boundary\\|with\\`markup\\\\and line"));
    assert!(output.ends_with("evidence.\n"));
    assert!(!output.ends_with("\n\n"));
    assert!(!output.contains('\r'));
}

struct Memory(BTreeMap<LogicalPath, Vec<u8>>);
impl ReadWorkspace for Memory {
    fn read(&self, path: &LogicalPath, _: usize) -> madb_app::Result<Option<Vec<u8>>> {
        Ok(self.0.get(path).cloned())
    }
}
#[test]
fn source_references_must_match_actual_frozen_bytes_once() {
    let source = FileReference {
        path: LogicalPath::new(SOURCE).unwrap(),
        digest: Digest::of_bytes(b"source"),
        size: 6,
    };
    let memory = Memory(BTreeMap::from([(source.path.clone(), b"source".to_vec())]));
    let frozen = capture_inputs(&memory, vec![source.clone()]).unwrap();
    let file = json!({"path":SOURCE,"sha256":source.digest.as_str(),"size":6});
    let mut lock = json!({"inputs":[file.clone()]});
    assert_eq!(tracked(&lock, &frozen, SOURCE).unwrap().0, b"source");
    lock["inputs"][0]["sha256"] = json!(Digest::of_bytes(b"Source").as_str());
    assert!(tracked(&lock, &frozen, SOURCE).is_err());
    lock["inputs"] = json!([file.clone(), file]);
    assert!(tracked(&lock, &frozen, SOURCE).is_err());
    assert!(tracked(&lock, &frozen, PROTOCOL).is_err());
}

fn format_fixture(name: &str) -> Value {
    serde_json::from_slice(
        &std::fs::read(format!(
            "{}/../../tests/fixtures/contracts/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap()
}
fn document(kind: DocumentKind, value: Value) -> Document {
    crate::SchemaRegistry::new()
        .unwrap()
        .validate(kind, value)
        .unwrap()
}
#[test]
fn other_readers_and_custom_sources_do_not_inherit_fixed_sdk_guarantees() {
    let data = b"unrelated file".to_vec();
    let other = FileReference {
        path: LogicalPath::new("other.txt").unwrap(),
        digest: Digest::of_bytes(&data),
        size: data.len() as u64,
    };
    let frozen = capture_inputs(
        &Memory(BTreeMap::from([(other.path.clone(), data)])),
        vec![other],
    )
    .unwrap();
    let profile = document(DocumentKind::Profile, format_fixture("profile"));
    let contract = document(DocumentKind::ContractIr, format_fixture("contract-ir"));
    let mut request = format_fixture("generate-request");
    let mut lock = format_fixture("lock");
    assert!(
        first_route_ownership(
            &profile,
            &document(DocumentKind::Lock, lock.clone()),
            &document(DocumentKind::GenerateRequest, request.clone()),
            &contract,
            &frozen
        )
        .unwrap()
        .is_none()
    );
    request["component"]["id"] = json!(READER);
    let request = document(DocumentKind::GenerateRequest, request);
    // A selected known reader cannot silently omit its backend input.
    assert!(
        first_route_ownership(
            &profile,
            &document(DocumentKind::Lock, lock.clone()),
            &request,
            &contract,
            &frozen
        )
        .is_err()
    );
    let data = serde_json::to_vec(
        &json!({"provider":"rust","semantics":"madb.first-route/1","source":"custom/engine.rs"}),
    )
    .unwrap();
    let backend = FileReference {
        path: LogicalPath::new("backend.json").unwrap(),
        digest: Digest::of_bytes(&data),
        size: data.len() as u64,
    };
    let frozen = capture_inputs(
        &Memory(BTreeMap::from([(backend.path.clone(), data)])),
        vec![backend.clone()],
    )
    .unwrap();
    lock["inputs"] =
        json!([{"path":"backend.json","sha256":backend.digest.as_str(),"size":backend.size}]);
    assert!(
        first_route_ownership(
            &profile,
            &document(DocumentKind::Lock, lock),
            &request,
            &contract,
            &frozen
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn actual_sdk_cargo_document_and_wrong_package_identities() {
    let manifest = include_bytes!("../../../../sdk/rust/engine/Cargo.toml");
    check_package(manifest).unwrap();
    let original = std::str::from_utf8(manifest).unwrap();
    for changed in [
        original.replace("madb-first-route-engine", "other-engine"),
        original.replace("0.1.0", "0.2.0"),
        "[package]\nname=1\nversion='0.1.0'\n".into(),
    ] {
        assert!(check_package(changed.as_bytes()).is_err());
    }
}

#[test]
fn fixed_sdk_source_cannot_silently_skip_an_incompatible_profile() {
    let data = serde_json::to_vec(&json!({"schema":"madb.rust-c-backend/1","provider":"rust","semantics":"madb.first-route/1","source":SOURCE})).unwrap();
    let reference = FileReference {
        path: LogicalPath::new("backend.json").unwrap(),
        digest: Digest::of_bytes(&data),
        size: data.len() as u64,
    };
    let frozen = capture_inputs(
        &Memory(BTreeMap::from([(reference.path.clone(), data)])),
        vec![reference.clone()],
    )
    .unwrap();
    let mut lock = format_fixture("lock");
    lock["inputs"] =
        json!([{"path":"backend.json","sha256":reference.digest.as_str(),"size":reference.size}]);
    let lock = document(DocumentKind::Lock, lock);
    let mut request = format_fixture("generate-request");
    request["component"]["id"] = json!(READER);
    let request = document(DocumentKind::GenerateRequest, request);
    let contract = document(DocumentKind::ContractIr, format_fixture("contract-ir"));
    for (party, field, wrong) in [
        ("provider", "language", "c"),
        ("consumer", "language", "rust"),
        ("provider", "runtime", "other"),
    ] {
        let mut profile = format_fixture("profile");
        profile[party][field] = json!(wrong);
        let error = first_route_ownership(
            &document(DocumentKind::Profile, profile),
            &lock,
            &request,
            &contract,
            &frozen,
        )
        .err()
        .unwrap();
        assert!(
            error
                .to_string()
                .contains("fixed native SDK profile or backend differs")
        );
    }
}
