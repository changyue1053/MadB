mod common;
use common::{archive, file, parse};
use madb_model::{Id, LogicalPath};
use madb_plan::project::{
    FileOwnership, IdentifierLanguage, JsonBinding, ParameterValue, Renderer, TemplateAsset,
};
use madb_protocol::{DocumentKind, SchemaRegistry, Value};
use madb_templates::{AssetError, AssetErrorKind, render_asset, render_bytes};
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn id(value: &str) -> Id {
    Id::new(value).unwrap()
}
fn registry() -> &'static SchemaRegistry {
    static REGISTRY: OnceLock<SchemaRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| SchemaRegistry::new().unwrap())
}
fn bindings(pairs: &[(&str, &str)]) -> Renderer {
    Renderer::JsonValues {
        bindings: pairs
            .iter()
            .map(|(parameter, pointer)| JsonBinding {
                parameter: id(parameter),
                pointer: (*pointer).into(),
            })
            .collect(),
    }
}
fn identifiers(language: IdentifierLanguage, names: &[&str]) -> Renderer {
    Renderer::Identifiers {
        language,
        parameters: names.iter().map(|name| id(name)).collect(),
    }
}
fn parameters(items: &[(&str, ParameterValue)]) -> BTreeMap<Id, ParameterValue> {
    items
        .iter()
        .map(|(key, value)| (id(key), value.clone()))
        .collect()
}
fn ordinary(
    renderer: &Renderer,
    source: &[u8],
    parameters: &BTreeMap<Id, ParameterValue>,
) -> Result<Vec<u8>, AssetError> {
    render_bytes(renderer, source, parameters, registry(), |_| {
        panic!("ordinary JSON/copy must not invoke MadB semantics")
    })
}

#[test]
fn copy_is_byte_exact_and_does_not_interpret_tokens_or_json() {
    let source = b"\xff@@MADB:name@@\0{invalid-json";
    assert_eq!(
        ordinary(&Renderer::Copy, source, &BTreeMap::new()).unwrap(),
        source
    );
}

#[test]
fn json_values_replace_scalars_with_typed_values_and_canonical_lf() {
    let renderer = bindings(&[("text", "/a~1b"), ("number", "/a~0b/0"), ("flag", "/")]);
    let values = parameters(&[
        ("text", ParameterValue::String("\n\"\\".into())),
        ("number", ParameterValue::Integer(-9_007_199_254_740_991)),
        ("flag", ParameterValue::Boolean(false)),
    ]);
    let output = ordinary(
        &renderer,
        br#"{"z":null,"a/b":null,"a~b":[null],"":null}"#,
        &values,
    )
    .unwrap();
    assert_eq!(
        output,
        b"{\"\":false,\"a/b\":\"\\n\\\"\\\\\",\"a~b\":[-9007199254740991],\"z\":null}\n"
    );
    assert_eq!(
        ordinary(&bindings(&[("number", "")]), b"null", &values).unwrap(),
        b"-9007199254740991\n"
    );
    let object_key = ordinary(&bindings(&[("flag", "/01")]), br#"{"01":null}"#, &values).unwrap();
    assert_eq!(
        object_key, b"{\"01\":false}\n",
        "numeric object keys are not array indices"
    );
}

#[test]
fn json_pointer_addresses_must_exist_be_scalar_and_not_overlap() {
    let values = parameters(&[("v", ParameterValue::Integer(7))]);
    for pointers in [
        vec!["a"],
        vec!["/missing"],
        vec!["/a/01"],
        vec!["/a/-"],
        vec!["/a/1"],
        vec!["/a/+0"],
        vec!["/a/184467440737095516160"],
        vec!["/a/0/x"],
        vec!["/a"],
        vec![""],
        vec!["/a/0", "/a/0"],
        vec!["/a", "/a/0"],
        vec!["/a~2b"],
        vec!["/~"],
        vec!["/a/0", "/missing"],
    ] {
        let pairs: Vec<_> = pointers.iter().map(|pointer| ("v", *pointer)).collect();
        assert!(
            ordinary(&bindings(&pairs), br#"{"a":[null]}"#, &values).is_err(),
            "{pointers:?}"
        );
    }
    assert!(
        ordinary(
            &bindings(&[("missing", "/a/0")]),
            br#"{"a":[null]}"#,
            &values
        )
        .is_err()
    );
}

#[test]
fn json_input_retains_strict_lexical_and_output_budgets() {
    let renderer = bindings(&[]);
    for source in [
        br#"{"a":1,"a":2}"#.as_slice(),
        b"1.0",
        b"1e0",
        b"-0",
        b"NaN",
        b"{} {}",
        b"9007199254740992",
    ] {
        assert!(ordinary(&renderer, source, &BTreeMap::new()).is_err());
    }
    let values = parameters(&[("v", ParameterValue::Integer(9_007_199_254_740_992))]);
    assert!(ordinary(&bindings(&[("v", "/a")]), br#"{"a":null}"#, &values).is_err());
    let mut source = serde_json::Map::new();
    let mut entries = Vec::new();
    for i in 0..300 {
        let key = format!("f{i}");
        source.insert(key.clone(), Value::Null);
        entries.push(JsonBinding {
            parameter: id("v"),
            pointer: format!("/{key}"),
        });
    }
    let values = parameters(&[("v", ParameterValue::String("x".repeat(4096)))]);
    assert!(
        ordinary(
            &Renderer::JsonValues { bindings: entries },
            &serde_json::to_vec(&source).unwrap(),
            &values
        )
        .is_err()
    );
}

#[test]
fn ordinary_json_array_budget_applies_at_every_depth() {
    let renderer = bindings(&[]);
    for count in [4096, 4097] {
        let array = Value::Array(vec![Value::Null; count]);
        for value in [
            array.clone(),
            serde_json::json!({"outer": [{"nested": array}]}),
        ] {
            let result = ordinary(
                &renderer,
                &serde_json::to_vec(&value).unwrap(),
                &BTreeMap::new(),
            );
            assert_eq!(result.is_ok(), count == 4096);
        }
    }
}

#[test]
fn known_madb_json_requires_decoder_and_explicit_semantic_callback() {
    let source = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/contracts/project.json"
    ));
    let renderer = bindings(&[("name", "/project")]);
    let values = parameters(&[("name", ParameterValue::String("renamed".into()))]);
    let mut observed = 0;
    let result = render_bytes(&renderer, source, &values, registry(), |document| {
        observed += 1;
        assert_eq!(document.kind(), DocumentKind::Project);
        assert_eq!(document.value()["project"], "renamed");
        // This callback is a test substitute, not a project semantic certificate.
        Ok(())
    })
    .unwrap();
    assert_eq!(observed, 1);
    assert!(result.ends_with(b"\n"));
    let error = render_bytes(&renderer, source, &values, registry(), |_| {
        Err(AssetError {
            kind: AssetErrorKind::DocumentValidation,
            subject: "semantic-policy".into(),
            message: "independently rejected".into(),
        })
    })
    .unwrap_err();
    assert_eq!(error.subject, "semantic-policy");
    let invalid = parameters(&[("name", ParameterValue::Boolean(true))]);
    assert!(
        render_bytes(&renderer, source, &invalid, registry(), |_| panic!(
            "invalid schema must fail before callback"
        ))
        .is_err()
    );
    for replacement in ["business.payload/1", "madb.unknown/1"] {
        let values = parameters(&[("schema", ParameterValue::String(replacement.into()))]);
        assert!(
            render_bytes(
                &bindings(&[("schema", "/schema")]),
                source,
                &values,
                registry(),
                |_| panic!("schema cannot bypass validation")
            )
            .is_err()
        );
    }
    assert!(
        ordinary(
            &bindings(&[]),
            br#"{"schema":"madb.unknown/1"}"#,
            &BTreeMap::new()
        )
        .is_err()
    );
}

#[test]
fn identifiers_scan_only_source_tokens_and_match_declared_language() {
    let values = parameters(&[
        ("name", ParameterValue::String("Demo_7".into())),
        ("namespace", ParameterValue::String("meter".into())),
    ]);
    for (language, source, expected, names) in [
        (
            IdentifierLanguage::C,
            "#include \"@@MADB:namespace@@.h\"\nint @@MADB:namespace@@_run(void);",
            "#include \"meter.h\"\nint meter_run(void);",
            vec!["namespace"],
        ),
        (
            IdentifierLanguage::Toml,
            "name = \"@@MADB:name@@\"",
            "name = \"Demo_7\"",
            vec!["name"],
        ),
        (
            IdentifierLanguage::Cmake,
            "project(@@MADB:name@@ LANGUAGES C)",
            "project(Demo_7 LANGUAGES C)",
            vec!["name"],
        ),
    ] {
        assert_eq!(
            ordinary(&identifiers(language, &names), source.as_bytes(), &values).unwrap(),
            expected.as_bytes()
        );
    }
    let output = ordinary(
        &identifiers(IdentifierLanguage::C, &["name", "namespace"]),
        b"@@MADB:name@@@@MADB:namespace@@",
        &values,
    )
    .unwrap();
    assert_eq!(output, b"Demo_7meter");
}

#[test]
fn identifiers_reject_injection_reserved_names_and_malformed_markers() {
    let renderer = identifiers(IdentifierLanguage::C, &["name"]);
    for value in [
        "",
        "_private",
        "int",
        "Self",
        "async",
        "x;y",
        "x y",
        "x\ny",
        "x\"y",
        "x\\y",
        "${x}",
        "@@MADB:name@@",
        "中文",
        "1name",
    ] {
        let values = parameters(&[("name", ParameterValue::String(value.into()))]);
        assert!(
            ordinary(&renderer, b"@@MADB:name@@", &values).is_err(),
            "{value:?}"
        );
    }
    let values = parameters(&[("name", ParameterValue::String("Good".into()))]);
    for source in [
        b"@@MADB:name".as_slice(),
        b"@@MADB:unknown@@",
        b"@@MADB:@@",
        b"no token",
        b"@@MADB:name@@MADB:name@@",
        b"\xff@@MADB:name@@",
    ] {
        assert!(ordinary(&renderer, source, &values).is_err(), "{source:?}");
    }
    assert!(
        ordinary(
            &identifiers(IdentifierLanguage::C, &["name", "name"]),
            b"@@MADB:name@@",
            &values
        )
        .is_err()
    );
    assert!(
        ordinary(
            &identifiers(IdentifierLanguage::C, &[]),
            b"literal",
            &values
        )
        .is_err()
    );
    assert!(
        ordinary(
            &identifiers(IdentifierLanguage::Unknown(id("swift")), &["name"]),
            b"@@MADB:name@@",
            &values
        )
        .is_err()
    );
    assert!(
        ordinary(
            &Renderer::Unknown {
                id: id("other.renderer@1")
            },
            b"literal",
            &values
        )
        .is_err()
    );
}

#[test]
fn middle_double_underscore_is_not_a_cpp_restriction_on_c_or_rust() {
    // The approved languages are C11 and Rust. Their fixed reserved tables do
    // not import the C++ rule reserving a double underscore at any position.
    let values = parameters(&[("name", ParameterValue::String("A__B".into()))]);
    assert_eq!(
        ordinary(
            &identifiers(IdentifierLanguage::C, &["name"]),
            b"int @@MADB:name@@_run(void);",
            &values
        )
        .unwrap(),
        b"int A__B_run(void);"
    );
}

#[test]
fn assets_use_checked_bytes_and_local_state_is_only_a_reservation() {
    let source = b"unchanged";
    let bytes = archive(&[("asset.txt", source), ("manifest.json", b"{}")]);
    let package = parse(bytes)
        .unwrap()
        .check_inventory(&file("manifest.json", b"{}"), &[file("asset.txt", source)])
        .unwrap();
    let mut asset = TemplateAsset {
        id: id("asset"),
        module: id("root"),
        source: Some(file("asset.txt", source)),
        output: LogicalPath::new("output.txt").unwrap(),
        ownership: FileOwnership::User,
        renderer: Some(Renderer::Copy),
    };
    assert_eq!(
        render_asset(&asset, &package, &BTreeMap::new(), registry(), |_| panic!())
            .unwrap()
            .unwrap(),
        source
    );
    asset.source.as_mut().unwrap().size += 1;
    assert!(render_asset(&asset, &package, &BTreeMap::new(), registry(), |_| panic!()).is_err());
    asset.ownership = FileOwnership::LocalState;
    assert!(render_asset(&asset, &package, &BTreeMap::new(), registry(), |_| panic!()).is_err());
    asset.source = None;
    asset.renderer = None;
    assert_eq!(
        render_asset(&asset, &package, &BTreeMap::new(), registry(), |_| panic!()).unwrap(),
        None
    );
}
