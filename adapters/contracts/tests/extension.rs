use madb_contracts::extension::{
    ExtensionManifest, require_component_features, validate_registration,
};
use madb_protocol::{Value, byte_digest};
use serde_json::json;

fn component(id: &str, port: &str) -> Value {
    json!({"id":id,"port":port,"supported_features":["core.values@1"],
        "entrypoint":{"kind":"process","path":"bin/adapter","args":[]}})
}

fn package(id: &str) -> Value {
    json!({
        "schema":"madb.extension/1","id":id,"version":"0.1.0",
        "compatibility":{"protocol":["1.0"],"ir":["1.0"]},
        "languages":[],"components":[component("reader","contract-reader")],
        "dependencies":[],"routes":[],"assets":[],
        "execution":{"transport":"stdio-jsonl","network":"none","env":[]}
    })
}

fn decode(value: &Value) -> ExtensionManifest {
    ExtensionManifest::decode(&serde_json::to_vec(value).unwrap()).unwrap()
}

fn route() -> Value {
    json!({"id":"test.value-route",
        "provider":{"language":"c","runtime":"native"},
        "consumer":{"language":"rust","runtime":"native"},
        "directions":["consumer-to-provider"],
        "targets":[{"os":"macos","arch":"arm64","abi":"aarch64-apple-darwin"}],
        "supported_features":["core.values@1"],"restrictions":[],
        "bindings":{"contract-reader":"test.reader/reader",
            "generator":"test.route/generator","toolchain":"test.route/toolchain",
            "verifier":"test.route/verifier","artifact-packager":"test.route/packager"}})
}

fn linked_packages() -> [Value; 2] {
    let reader = package("test.reader");
    let mut consumer = package("test.route");
    consumer["components"] = json!([
        component("generator", "generator"),
        component("toolchain", "toolchain"),
        component("verifier", "verifier"),
        component("packager", "artifact-packager")
    ]);
    consumer["dependencies"] = json!([{"id":"test.reader","kind":"extension"}]);
    consumer["routes"] = json!([route()]);
    [reader, consumer]
}

fn reject(values: &[Value], message: &str) {
    let manifests: Vec<_> = values.iter().map(decode).collect();
    let error = validate_registration(&manifests).unwrap_err();
    assert_eq!(error.code, "InputInvalid");
    assert!(error.message.contains(message), "{error}");
}

#[test]
fn existing_manifest_vectors_use_the_bundled_schema() {
    let vectors: Value = serde_json::from_str(include_str!(
        "../../../docs/examples/extensions/schema-vectors.json"
    ))
    .unwrap();
    let mut count = 0;
    for vector in vectors["vectors"].as_array().unwrap() {
        if vector["subject"] != "manifest" {
            continue;
        }
        let result = ExtensionManifest::decode(&serde_json::to_vec(&vector["document"]).unwrap());
        assert_eq!(
            result.is_ok(),
            vector["expected_schema"] == "valid",
            "{}",
            vector["id"]
        );
        count += 1;
    }
    assert_eq!(count, 6);
}

#[test]
fn manifest_identity_uses_the_original_bytes() {
    let value = package("test.reader");
    let compact = serde_json::to_vec(&value).unwrap();
    let spaced = serde_json::to_vec_pretty(&value).unwrap();
    let a = ExtensionManifest::decode(&compact).unwrap();
    let b = ExtensionManifest::decode(&spaced).unwrap();
    assert_eq!(a.id(), "test.reader");
    assert_eq!(a.version(), "0.1.0");
    assert_eq!(a.manifest_digest(), byte_digest(&compact));
    assert_eq!(a.value(), b.value());
    assert_ne!(a.manifest_digest(), b.manifest_digest());
}

#[test]
fn malformed_duplicate_key_and_unknown_fields_are_rejected() {
    assert!(ExtensionManifest::decode(b"{\"schema\":1,\"schema\":2}").is_err());
    let mut value = package("test.reader");
    value["certified"] = json!(true);
    assert!(ExtensionManifest::decode(&serde_json::to_vec(&value).unwrap()).is_err());
    assert!(ExtensionManifest::decode(&vec![b' '; 1024 * 1024 + 1]).is_err());
}

#[test]
fn explicit_reference_closure_is_order_independent() {
    let [a, b] = linked_packages().map(|v| decode(&v));
    validate_registration(&[a.clone(), b.clone()]).unwrap();
    validate_registration(&[b, a]).unwrap();
    validate_registration(&[]).unwrap();
}

#[test]
fn design_example_is_not_registration_authority() {
    let value: Value = serde_json::from_str(include_str!(
        "../../../docs/examples/extensions/dart-ffi.extension.example.json"
    ))
    .unwrap();
    reject(&[value], "design example");
}

#[test]
fn duplicate_package_component_route_and_language_claims_fail() {
    let value = package("test.reader");
    reject(&[value.clone(), value.clone()], "duplicate extension");
    let mut duplicate = value.clone();
    duplicate["components"]
        .as_array_mut()
        .unwrap()
        .push(component("reader", "generator"));
    reject(&[duplicate], "duplicate component");
    let mut values = linked_packages();
    let mut second_route = values[1]["routes"][0].clone();
    second_route["restrictions"] = json!(["different text is not another route identity"]);
    values[1]["routes"]
        .as_array_mut()
        .unwrap()
        .push(second_route);
    reject(&values, "duplicate route");
    for value in &mut values {
        value["languages"] = json!([{"id":"new-language","runtimes":["native"]}]);
    }
    values[1]["routes"] = json!([route()]);
    reject(&values, "duplicate language");
}

#[test]
fn missing_dependencies_cycles_and_undeclared_cross_package_references_fail() {
    let [reader, consumer] = linked_packages();
    reject(
        std::slice::from_ref(&consumer),
        "missing extension dependency",
    );
    let mut cyclic_reader = reader.clone();
    cyclic_reader["dependencies"] = json!([{"id":"test.route","kind":"extension"}]);
    reject(&[cyclic_reader, consumer.clone()], "dependency cycle");
    let mut undeclared = consumer.clone();
    undeclared["dependencies"] = json!([]);
    reject(
        &[reader.clone(), undeclared],
        "not a declared extension dependency",
    );
    let mut missing_component = consumer;
    missing_component["routes"][0]["bindings"]["contract-reader"] = json!("test.reader/missing");
    reject(&[reader, missing_component], "does not resolve");
}

#[test]
fn wrong_port_cannot_satisfy_a_binding() {
    let mut values = linked_packages();
    values[0]["components"][0]["port"] = json!("generator");
    reject(&values, "port does not match");
}

#[test]
fn schema_valid_noncanonical_paths_are_rejected_before_registration() {
    for path in ["./adapter", "bin//adapter", "bin/./adapter", "bin/"] {
        let mut value = package("test.reader");
        value["components"][0]["entrypoint"]["path"] = json!(path);
        reject(&[value], "invalid package path");
    }
    let mut value = package("test.reader");
    value["assets"] = json!([{"kind":"sdk","path":"sdk/./api"}]);
    reject(&[value], "invalid package path");
}

#[test]
fn required_semantics_use_the_implemented_core_catalog() {
    let manifest = decode(&package("test.reader"));
    require_component_features(&manifest, "reader", &["core.values@1"]).unwrap();
    for feature in ["core.async@1", "vendor.magic@1", "core.value.scalar@1"] {
        let error = require_component_features(&manifest, "reader", &[feature]).unwrap_err();
        assert_eq!(error.code, "CapabilityUnsupported");
        assert!(error.message.contains(feature));
    }
    assert!(require_component_features(&manifest, "missing", &[]).is_err());
}

#[test]
fn self_declared_unknown_feature_and_duplicate_component_do_not_bypass_admission() {
    let mut value = package("test.reader");
    value["components"][0]["supported_features"] = json!(["vendor.magic@1"]);
    let manifest = decode(&value);
    assert_eq!(
        require_component_features(&manifest, "reader", &["vendor.magic@1"])
            .unwrap_err()
            .code,
        "CapabilityUnsupported"
    );
    value["components"]
        .as_array_mut()
        .unwrap()
        .push(component("reader", "generator"));
    assert!(require_component_features(&decode(&value), "reader", &[]).is_err());
}

#[test]
fn build_component_only_needs_its_operation_features() {
    let mut value = package("test.builder");
    value["components"] = json!([component("builder", "toolchain")]);
    value["components"][0]["supported_features"] = json!([]);
    require_component_features(&decode(&value), "builder", &[]).unwrap();
}
