//! Version-matched UDL parser output. No regex or debug-format parsing establishes structure.
use serde_json::{json, Value};
use uniffi_bindgen::{
    interface::{Argument, AsType, Field, ObjectImpl, Type},
    ComponentInterface,
};
fn ty(t: &Type) -> Value {
    match t {
        Type::UInt8 => json!({"kind":"u8"}),
        Type::Int8 => json!({"kind":"i8"}),
        Type::UInt16 => json!({"kind":"u16"}),
        Type::Int16 => json!({"kind":"i16"}),
        Type::UInt32 => json!({"kind":"u32"}),
        Type::Int32 => json!({"kind":"i32"}),
        Type::UInt64 => json!({"kind":"u64"}),
        Type::Int64 => json!({"kind":"i64"}),
        Type::Float32 => json!({"kind":"f32"}),
        Type::Float64 => json!({"kind":"f64"}),
        Type::Boolean => json!({"kind":"bool"}),
        Type::String => json!({"kind":"string"}),
        Type::Bytes => json!({"kind":"bytes"}),
        Type::Optional { inner_type } => json!({"kind":"optional","inner":ty(inner_type)}),
        Type::Sequence { inner_type } => json!({"kind":"sequence","inner":ty(inner_type)}),
        #[cfg(not(feature = "gobley-029"))]
        Type::Box { inner_type } => json!({"kind":"box","inner":ty(inner_type)}),
        Type::Record { name, .. } => json!({"kind":"record","name":name}),
        Type::Enum { name, .. } => json!({"kind":"enum","name":name}),
        Type::Object { name, .. } => json!({"kind":"object","name":name}),
        Type::CallbackInterface { name, .. } => json!({"kind":"callback","name":name}),
        Type::Map {
            key_type,
            value_type,
        } => json!({"kind":"map","key":ty(key_type),"value":ty(value_type)}),
        #[cfg(not(feature = "gobley-029"))]
        Type::Set { inner_type } => json!({"kind":"set","inner":ty(inner_type)}),
        Type::Custom { name, builtin, .. } => {
            json!({"kind":"custom","name":name,"builtin":ty(builtin)})
        }
        Type::Timestamp => json!({"kind":"timestamp"}),
        Type::Duration => json!({"kind":"duration"}),
    }
}
fn implementation(implementation: &ObjectImpl) -> &'static str {
    #[cfg(not(feature = "gobley-029"))]
    {
        use uniffi_bindgen::interface::TraitKind;
        match implementation {
            ObjectImpl::Struct => "struct",
            ObjectImpl::Trait(TraitKind::RustOnly) => "rust-trait",
            ObjectImpl::Trait(TraitKind::Both) => "callback-trait",
            ObjectImpl::Trait(TraitKind::ForeignOnly) => "foreign-trait",
        }
    }
    #[cfg(feature = "gobley-029")]
    match implementation {
        ObjectImpl::Struct => "struct",
        ObjectImpl::Trait => "rust-trait",
        ObjectImpl::CallbackTrait => "callback-trait",
    }
}
fn args(a: Vec<&Argument>) -> Vec<Value> {
    a.into_iter()
        .map(|a| json!({"name":a.name(),"type":ty(&a.as_type()),"by_ref":a.by_ref(),"has_default":a.default_value().is_some()}))
        .collect()
}
fn fields(f: &[Field]) -> Vec<Value> {
    f.iter()
        .map(|f| json!({"name":f.name(),"type":ty(&f.as_type()),"has_default":f.default_value().is_some()}))
        .collect()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let argsv: Vec<_> = std::env::args().collect();
    if argsv.len() != 3 {
        return Err("usage: madb-uniffi-metadata <udl> <crate>".into());
    }
    let mut ci = ComponentInterface::from_webidl(&std::fs::read_to_string(&argsv[1])?, &argsv[2])?;
    ci.derive_ffi_funcs()?;
    let mut types = Vec::new();
    let mut functions = Vec::new();
    for r in ci.record_definitions() {
        types.push(json!({"kind":"record","name":r.name(),"fields":fields(r.fields())}));
    }
    for e in ci.enum_definitions() {
        types.push(json!({"kind":"enum","name":e.name(),"is_error":ci.is_name_used_as_error(e.name()),"variants":e.variants().iter().map(|v|json!({"name":v.name(),"fields":fields(v.fields())})).collect::<Vec<_>>()}));
    }
    for o in ci.object_definitions() {
        types.push(
            json!({"kind":"object","name":o.name(),"implementation":implementation(o.imp())}),
        );
        for c in o.constructors() {
            functions.push(json!({"kind":"constructor","owner":o.name(),"name":c.name(),"ffi_name":c.ffi_func().name(),"async":c.is_async(),"parameters":args(c.arguments()),"result":{"kind":"object","name":o.name(),"implementation":implementation(o.imp())},"throws":c.throws_type().map(ty),"ffi_result":format!("{:?}",c.ffi_func().return_type())}));
        }
        for m in o.methods() {
            functions.push(json!({"kind":"method","owner":o.name(),"name":m.name(),"ffi_name":m.ffi_func().name(),"async":m.is_async(),"parameters":args(m.arguments()),"result":m.return_type().map(ty),"throws":m.throws_type().map(ty),"ffi_result":format!("{:?}",m.ffi_func().return_type())}));
        }
    }
    for c in ci.callback_interface_definitions() {
        types.push(json!({"kind":"callback","name":c.name()}));
        for m in c.methods() {
            functions.push(json!({"kind":"callback_method","owner":c.name(),"name":m.name(),"async":m.is_async(),"parameters":args(m.arguments()),"result":m.return_type().map(ty),"throws":m.throws_type().map(ty)}));
        }
    }
    for f in ci.function_definitions() {
        functions.push(json!({"kind":"function","owner":null,"name":f.name(),"ffi_name":f.ffi_func().name(),"async":f.is_async(),"parameters":args(f.arguments()),"result":f.return_type().map(ty),"throws":f.throws_type().map(ty),"ffi_result":format!("{:?}",f.ffi_func().return_type())}));
    }
    let parser = if cfg!(feature = "gobley-029") {
        "uniffi-0.29.4"
    } else {
        "uniffi-0.32.2"
    };
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"schema_version":"1.0.0","parser":parser,"namespace":ci.namespace(),"crate":argsv[2],"source_positions":"not retained by ComponentInterface","types":types,"functions":functions})
        )?
    );
    Ok(())
}
