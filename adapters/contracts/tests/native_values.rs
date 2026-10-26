use madb_contracts::native_values::read_envelope;
use madb_protocol::{FormatError, Value};
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const BASE: &str = "pub struct Reading { pub value: i32, pub label: String }\npub fn normalize(mut value: Reading) -> Reading { value.label = value.label.trim().to_owned(); value }\n";
const AUTHORS: [&str; 5] = [
    include_str!("native_values/author-0.rs"),
    include_str!("native_values/author-1.rs"),
    include_str!("native_values/author-2.rs"),
    include_str!("native_values/author-3.rs"),
    include_str!("native_values/author-4.rs"),
];
fn read(source: &str) -> Result<Value, FormatError> {
    read_namespace(source, "reading_demo")
}
fn read_namespace(source: &str, namespace: &str) -> Result<Value, FormatError> {
    read_envelope(
        &serde_json::to_vec(
            &json!({"source_path":"business/reading.rs", "source":source, "namespace":namespace}),
        )
        .unwrap(),
    )
    .map(|bytes| serde_json::from_slice(&bytes).unwrap())
}
fn rejected(source: &str) {
    let error = read(source).expect_err(source);
    assert_eq!(error.code, "NATIVE004", "{error}");
    assert!(error.path.starts_with("business/reading.rs:"), "{error}");
}

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "madb-value-reader-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn compile_witness(plan: &Value, source: &str) -> std::process::Output {
    let directory = Scratch::new();
    std::fs::write(directory.0.join("author.rs"), source).unwrap();
    std::fs::write(
        directory.0.join("witness.rs"),
        plan["witness"].as_str().unwrap(),
    )
    .unwrap();
    Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args([
            "--edition=2024",
            "--crate-type=lib",
            "--crate-name=madb_value_witness",
            "--emit=metadata",
            "-C",
            "panic=unwind",
            "witness.rs",
            "-o",
            "witness.rmeta",
        ])
        .current_dir(&directory.0)
        .output()
        .unwrap()
}
fn compiles(plan: &Value, source: &str) {
    let output = compile_witness(plan, source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn five_author_changes_are_read_from_source_and_witnessed_by_real_rustc() {
    let expected_names = [
        vec!["normalize"],
        vec!["normalize", "offset"],
        vec!["normalize", "offset"],
        vec!["canonicalize", "offset"],
        vec!["canonicalize"],
    ];
    for (index, source) in AUTHORS.iter().enumerate() {
        let plan = read(source).unwrap();
        let names: Vec<_> = plan["functions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|function| function["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, expected_names[index]);
        assert_eq!(
            plan["records"][0]["fields"].as_array().unwrap().len(),
            if index < 2 { 2 } else { 3 }
        );
        assert_eq!(plan["namespace"], "reading_demo");
        compiles(&plan, source);
        assert_eq!(plan, read(source).unwrap());
    }
}

#[test]
fn four_signature_shapes_are_not_business_name_specific() {
    let source = "#[derive(Clone)] pub struct Payload { pub text: ::std::string::String }\n#[repr(i32)] pub enum Failure { Full = 0x7_i32 }\npub fn first(x: Payload)->Payload{x}\npub fn second(x: Payload, amount:i32)->Payload{let _=amount; x}\npub fn third(x: Payload)->std::result::Result<Payload, Failure>{Ok(x)}\npub fn fourth(mut x: Payload, amount:i32)->Result<Payload, Failure>{x.text.push_str(&amount.to_string());Ok(x)}";
    let plan = read(source).unwrap();
    assert_eq!(plan["functions"].as_array().unwrap().len(), 4);
    assert_eq!(plan["errors"][0]["variants"][0]["value"], 7);
    assert_eq!(plan["functions"][0]["error"], Value::Null);
    assert_eq!(plan["functions"][3]["error"], "Failure");
    compiles(&plan, source);
}

#[test]
fn maximum_record_and_standard_derives_have_real_type_witnesses() {
    let fields = (0..16)
        .map(|i| format!("pub f{i}:u32"))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        "#[derive(Clone,Copy,Debug,PartialEq,Eq)] pub struct Sample{{{fields}}}\npub fn stable(input:Sample)->Sample{{input}}"
    );
    let plan = read(&source).unwrap();
    compiles(&plan, &source);
    rejected(&source.replace("f15:u32", "f15:u32,pub f16:u32"));
}

#[test]
fn locations_are_original_utf8_bytes_with_bom_crlf_and_unicode_comments() {
    let source = "\u{feff}/*温*/ pub struct Reading { pub value:i32, pub label:String }\r\n/*温度*/ pub fn normalize(value:Reading)->Reading{value}";
    let plan = read(source).unwrap();
    let record = source.find("Reading").unwrap();
    let function = source.rfind("normalize").unwrap();
    let line_start = source[..function].rfind('\n').unwrap() + 1;
    assert_eq!(
        plan["records"][0]["location"],
        json!({"path":"business/reading.rs","line":1,"column":record+1})
    );
    assert_eq!(
        plan["functions"][0]["location"],
        json!({"path":"business/reading.rs","line":2,"column":function-line_start+1})
    );
    compiles(&plan, source);
}

#[test]
fn envelope_is_exact_bounded_and_rejects_duplicate_keys() {
    for envelope in [
        json!({}),
        json!({"source_path":"a.rs","source":BASE,"namespace":"sample","extra":1}),
        json!({"source_path":"a.rs","source":1,"namespace":"sample"}),
    ] {
        assert_eq!(
            read_envelope(&serde_json::to_vec(&envelope).unwrap())
                .unwrap_err()
                .code,
            "NATIVE001"
        );
    }
    assert!(
        read_envelope(br#"{"source":"","source":"","source_path":"a.rs","namespace":"sample"}"#)
            .is_err()
    );
    assert!(
        read_envelope(&vec![
            b' ';
            madb_contracts::native_values::MAX_ENVELOPE_BYTES + 1
        ])
        .is_err()
    );
    for path in [
        "../a.rs",
        "/a.rs",
        "a\\b.rs",
        "a//b.rs",
        "a/./b.rs",
        "a.rs\0",
        "source.txt",
    ] {
        let error = read_envelope(
            &serde_json::to_vec(&json!({"source_path":path,"source":BASE,"namespace":"sample"}))
                .unwrap(),
        )
        .unwrap_err();
        assert_eq!(error.code, "NATIVE001");
    }
}

#[test]
fn unsupported_module_items_and_visibility_never_disappear() {
    for extra in [
        "type Alias = Reading;",
        "use std::string::String;",
        "pub use std::string::String as Other;",
        "mod external;",
        "mod inline {}",
        "pub trait Extra {}",
        "impl Reading { pub fn extra(&self) {} }",
        "unsafe extern \"C\" { fn extra(); }",
        "const HIDDEN:i32=1;",
        "macro_rules! exports {()=>{}}",
        "extern crate std;",
        "struct Hidden { field:i32 }",
        "fn private(value:Reading)->Reading{value}",
    ] {
        rejected(&format!("{BASE}\n{extra}"));
    }
    for variant in [
        BASE.replace("pub struct", "pub(crate) struct"),
        BASE.replace("pub label", "label"),
        BASE.replace("pub fn", "fn"),
        BASE.replace("pub fn", "pub(crate) fn"),
        BASE.replace("pub struct Reading", "pub struct Reading<T>"),
    ] {
        rejected(&variant);
    }
}

#[test]
fn attributes_are_checked_at_every_declaration_and_body_position() {
    for source in [
        format!("#![no_std]\n{BASE}"),
        format!("#[cfg(any())]\n{BASE}"),
        format!("#[repr(C)]\n{BASE}"),
        BASE.replace("pub label", "#[cfg(any())] pub label"),
        BASE.replace("pub fn", "#[unsafe(no_mangle)] pub fn"),
        BASE.replace("mut value:", "#[cfg(any())] mut value:"),
        BASE.replace("value.label =", "#[allow(unused)] value.label ="),
        BASE.replace("value.label =", "#[cfg(any())] value.label ="),
        BASE.replace("pub struct", "#[derive(Default)] pub struct"),
        BASE.replace("pub struct", "#[derive(custom::Clone)] pub struct"),
    ] {
        rejected(&source);
    }
    let error = "#[repr(i32)] pub enum E { #[cfg(any())] Fail=1 }";
    rejected(&format!("{BASE}\n{error}"));
}

#[test]
fn unsupported_record_shapes_types_and_shadowing_are_rejected() {
    for declaration in [
        "pub struct Reading;",
        "pub struct Reading(i32);",
        "pub struct Reading {}",
        "pub struct Reading{ pub value: Vec<i32> }",
        "pub struct Reading{ pub value: &'static str }",
        "pub struct Reading{ pub value: f64 }",
        "pub struct Reading{pub x:i32,pub x:u32}",
    ] {
        rejected(&format!(
            "{declaration}\npub fn normalize(value:Reading)->Reading{{value}}"
        ));
    }
    for shadow in [
        "pub struct String{pub x:i32}",
        "pub struct Result{pub x:i32}",
    ] {
        rejected(&format!("{BASE}\n{shadow}"));
    }
}

#[test]
fn function_qualifiers_types_patterns_and_arity_are_rejected() {
    for declaration in [
        "pub async fn f(value:Reading)->Reading{value}",
        "pub unsafe fn f(value:Reading)->Reading{value}",
        "pub extern \"C\" fn f(value:Reading)->Reading{value}",
        "pub fn f<T>(value:Reading)->Reading{value}",
        "pub fn f<'a>(value:Reading)->Reading{value}",
        "pub fn f(value:Reading)->Reading where Reading:Clone{value}",
        "pub fn f(ref value:Reading)->Reading{todo!()}",
        "pub fn f(value @ _:Reading)->Reading{value}",
        "pub fn f(_:Reading)->Reading{todo!()}",
        "pub fn f(Reading{value,label}:Reading)->Reading{Reading{value,label}}",
        "pub fn f(value:&Reading)->Reading{todo!()}",
        "pub fn f()->Reading{todo!()}",
        "pub fn f(a:Reading,b:i32,c:i32)->Reading{a}",
        "pub fn f(value:Reading,n:u32)->Reading{value}",
        "pub fn f(value:Reading){}",
        "pub fn f(value:Reading)->Option<Reading>{Some(value)}",
        "pub fn f(value:Reading)->Result<Reading,String>{Ok(value)}",
        "pub fn f(value:Reading,input:i32)->Reading{value}",
        "pub fn f(value:Reading,out_value:i32)->Reading{value}",
        "pub fn f(value:Reading,out_error:i32)->Reading{value}",
        "pub fn f(value:Reading,value:i32)->Reading{todo!()}",
    ] {
        rejected(&format!("{BASE}\n{declaration}"));
    }
}

#[test]
fn fieldless_error_tags_require_repr_and_explicit_distinct_positive_i32() {
    for declaration in [
        "pub enum E { Fail=1 }",
        "#[repr(u32)] pub enum E { Fail=1 }",
        "#[repr(i32)] enum E { Fail=1 }",
        "#[repr(i32)] pub enum E {}",
        "#[repr(i32)] pub enum E { Fail }",
        "#[repr(i32)] pub enum E { Fail=0 }",
        "#[repr(i32)] pub enum E { Fail=-1 }",
        "#[repr(i32)] pub enum E { Fail=1+1 }",
        "#[repr(i32)] pub enum E { Fail={1} }",
        "#[repr(i32)] pub enum E { Fail=2147483648 }",
        "#[repr(i32)] pub enum E { Fail=1u32 }",
        "#[repr(i32)] pub enum E { A=1,B=1 }",
        "#[repr(i32)] pub enum E { A=1,A=2 }",
        "#[repr(i32)] pub enum E { Fail(i32)=1 }",
        "#[repr(i32)] pub enum E { Fail{x:i32}=1 }",
        "#[repr(i32)] pub enum E<T> { Fail=1 }",
    ] {
        rejected(&format!("{BASE}\n{declaration}"));
    }
}

#[test]
fn source_names_are_not_silently_unraw_transliterated_or_renamed() {
    for source in [
        BASE.replace("value: i32", "r#type:i32"),
        BASE.replace("value: i32", "switch:i32"),
        BASE.replace("value: i32", "_Atomic:i32"),
        BASE.replace("Reading", "读数"),
        BASE.replace("normalize", "r#type"),
    ] {
        rejected(&source);
    }
    for namespace in ["", "_reserved", "int", "a.b", "a/b", "中文", "9prefix"] {
        assert_eq!(
            read_namespace(BASE, namespace).unwrap_err().code,
            "NATIVE004"
        );
    }
}

#[test]
fn all_generated_names_and_macros_share_a_collision_table() {
    for extra in [
        "pub struct ReadingResult {pub x:i32}",
        "pub struct reading_demo_Reading {pub x:i32}",
        "pub struct MadbValueError {pub x:i32}",
        "pub fn reading_demo_raw_normalize(x:Reading)->Reading{x}",
        "pub fn bootstrap_v1(x:Reading)->Reading{x}",
        "pub fn raw_normalize(x:Reading)->Reading{x}",
        "pub fn value_utf8_valid(x:Reading)->Reading{x}",
        "pub fn value_Reading_is_zero(x:Reading)->Reading{x}",
        "pub fn value_Reading_valid_output(x:Reading)->Reading{x}",
        "#[repr(i32)] pub enum A_B{C=1}\n#[repr(i32)] pub enum A{B_C=2}",
        "#[repr(i32)] pub enum Error{Issue=1}\npub fn Error_Issue(x:Reading)->Reading{x}",
        "pub fn f(x:Reading, MadbValueError:i32)->Reading{x}",
        "pub fn f(x:Reading, reading_demo_raw_normalize:i32)->Reading{x}",
    ] {
        rejected(&format!("{BASE}\n{extra}"));
    }
    let source = BASE.replace("normalize", "STATUS_OK");
    assert_eq!(
        read_namespace(&source, "MADB_VALUE").unwrap_err().code,
        "NATIVE004"
    );
    rejected(&BASE.replace("value: i32", "MADB_VALUE_STATUS_OK:i32"));
    // A field is a member, so sharing a typedef's spelling is not a collision.
    assert!(read(&BASE.replace("value: i32", "MadbValueError:i32")).is_ok());
}

#[test]
fn hidden_compilation_inputs_are_rejected_even_inside_macro_tokens() {
    for body in [
        "let _=include_str!(\"other.txt\");",
        "let _=std::include_bytes!(\"other.txt\");",
        "let _=env!(\"HOST_VALUE\");",
        "let _=option_env!(\"HOST_VALUE\");",
        "include!(\"other.rs\");",
        "let _=format!(\"{}\",include_str!(\"other.txt\"));",
        "let _=concat!(env!(\"HOST_VALUE\"));",
        "macro_rules! hidden {()=>{}}",
        "mod external;",
        "unsafe extern \"C\" {fn external();}",
        "extern crate std;",
        "something!(#[no_mangle] fn x(){});",
    ] {
        rejected(&BASE.replace("value.label =", &format!("{body} value.label =")));
    }
    let source = BASE.replace(
        "value.label =",
        "let _=format!(\"include! cfg env!\"); /* include!(\"unused\") */ value.label =",
    );
    compiles(&read(&source).unwrap(), &source);
    let panic_source = BASE.replace(
        "value.label = value.label.trim().to_owned(); value",
        "panic!(\"controlled unwind\")",
    );
    compiles(&read(&panic_source).unwrap(), &panic_source);
}

#[test]
fn witness_rejects_source_drift_and_types_that_syntax_cannot_resolve() {
    let plan = read(AUTHORS[2]).unwrap();
    for changed in [
        AUTHORS[2].replace("pub quality: u32", "pub quality: i32"),
        AUTHORS[2].replace("Overflow = 1", "Overflow = 2"),
        AUTHORS[2].replace("pub fn offset", "pub fn renamed"),
        AUTHORS[2].replace("delta: i32", "delta: u32"),
    ] {
        assert!(
            !compile_witness(&plan, &changed).status.success(),
            "source drift passed: {changed}"
        );
    }
    let invalid = BASE.replace("pub struct", "#[derive(Copy,Clone)] pub struct");
    let syntax = read(&invalid).unwrap();
    assert!(
        !compile_witness(&syntax, &invalid).status.success(),
        "String record cannot implement Copy"
    );
    let body = BASE.replace("value.label.trim().to_owned()", "999");
    assert!(
        !compile_witness(&read(&body).unwrap(), &body)
            .status
            .success(),
        "actual author body was not checked"
    );
}

#[test]
fn compiler_input_macros_cannot_hide_behind_imports_raw_names_or_assembly() {
    for body in [
        "use std::include_str as hidden; let _ = hidden!(\"unfrozen.txt\");",
        "use std::env as hidden; let _ = hidden!(\"HOST_MUTABLE\");",
        "use std::{include_bytes as hidden}; let _ = hidden!(\"unfrozen.txt\");",
        "use std as s; use s::{include_str as hidden}; let _ = hidden!(\"unfrozen.txt\");",
        "use std::*; use include_str as hidden; let _ = hidden!(\"unfrozen.txt\");",
        "let _ = format!(\"{}\", { use std::include_str as hidden; hidden!(\"unfrozen.txt\") });",
        "let _ = r#include_str!(\"unfrozen.txt\");",
        "use std::r#env as hidden; let _ = hidden!(\"HOST_MUTABLE\");",
        "std::arch::global_asm!(\".globl injected_export\");",
        "use std::arch::global_asm as hidden; hidden!(\".globl injected_export\");",
        "unsafe { std::arch::asm!(\".include unfrozen.s\"); }",
        "let _ = format!(\"{}\", { #![allow(unused)] 1 });",
    ] {
        rejected(&BASE.replace("value.label =", &format!("{body} value.label =")));
    }
    let ordinary = BASE.replace(
        "value.label =",
        "use std::string::String as Text; let _ = Text::new(); value.label =",
    );
    compiles(&read(&ordinary).unwrap(), &ordinary);
}

#[test]
fn const_synchronous_functions_keep_the_same_runtime_signature() {
    let source =
        "pub struct Number { pub value:i32 } pub const fn identity(value:Number)->Number{value}";
    let plan = read(source).unwrap();
    compiles(&plan, source);
}

#[test]
fn author_names_can_match_standard_names_without_replacing_required_types() {
    for (name, field_kind) in [
        ("u8", "i32"),
        ("i32", "u32"),
        ("u32", "i32"),
        ("std", "i32"),
    ] {
        let source = format!(
            "pub struct {name} {{ pub value:{field_kind} }} pub fn keep(x:{name})->{name}{{x}}"
        );
        let plan = read(&source).unwrap();
        assert_eq!(plan["records"][0]["name"], name);
        assert_eq!(plan["functions"][0]["parameters"][0]["kind"], name);
        compiles(&plan, &source);
    }
    let error = "pub struct R {pub value:u32} #[repr(i32)] pub enum i32 { Limit=7 } pub fn checked(x:R)->Result<R,i32>{let _=x;Err(i32::Limit)}";
    compiles(&read(error).unwrap(), error);
    for source in [
        "pub struct i32 {pub value:u32} pub struct R {pub value:i32} pub fn keep(x:R)->R{x}",
        "pub struct i32 {pub value:u32} pub fn keep(x:i32, delta:i32)->i32{let _=delta;x}",
        "#[repr(i32)] pub enum u32 { Limit=7 } pub struct R {pub value:u32} pub fn keep(x:R)->R{x}",
    ] {
        let plan = read(source).unwrap();
        let output = compile_witness(&plan, source);
        assert!(
            !output.status.success(),
            "user type replaced a required primitive: {source}"
        );
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostic.contains("mismatched types"), "{diagnostic}");
    }
}
